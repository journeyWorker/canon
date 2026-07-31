//! `canon dispatch begin --role <r> --regime <k> [--repo <dir>]
//! [--agent-id <id>] [--task <task_id>] [--parent-run <run_id>]
//! [--json]` (S8 `retrieve-before-task`, task 2.3):
//! the LIVE run-manifest write seam. Everything else in S8 shipped
//! standalone (design.md Migration Plan Step 1 — `canon retrieve` + the
//! pre-dispatch hook "work with zero manifest integration"); this is
//! Step 2: at the moment a run is dispatched, retrieve the role+regime
//! guidance ONCE and record it verbatim into a `Run` manifest's
//! [`canon_model::records::Run::injected_guidance`], so a later replay
//! reproduces the run's inputs byte-for-byte even after the source
//! strategies are edited or demoted (the whole point of the snapshot
//! field, `Run::injected_guidance`'s own doc).
//!
//! # Why a private side-channel, not canon-store's git tier
//! `canon-ingest`'s own `Run` constructor (`normalize.rs`) is a
//! POST-HOC reconstruction from an already-completed session transcript,
//! written through `canon-store`'s `GitTier` at a canonical Hive-keyed
//! path — and a git-tier duplicate-path write is a HARD ERROR
//! (`canon-store::tier`'s own doc), not an idempotent dedup. A live
//! dispatch-time `Run` and the later post-hoc ingest `Run` for the same
//! session would therefore collide on that path. So the dispatch record
//! lands in a private, non-canonical side-channel
//! (`<repo>/.canon/dispatch/<run_id>.json`), keyed by the freshly-minted
//! `RunId` (unique per dispatch, never colliding), for a future
//! reconciliation step to fold into the canonical tier — never fed
//! through `GitTier`'s Hive scheme here. This is exactly the seam S8's
//! own tasks.md note called "a live run-manifest write seam that does
//! not exist yet".
//!
//! FAIL-SOFT retrieval, FAIL-LOUD write: the retrieval half reuses
//! `canon_learn::retrieve_guidance`'s own fail-soft contract (a store
//! outage yields empty guidance, never an error); only a `--role`/
//! `--regime` usage mismatch (exit `2`) or a filesystem write failure
//! (exit `1`) is surfaced.
//!
//! # s40 (`plan-vs-actual-diff`): the two edges, and closing a run
//! Until s40 this seam wrote `None, None` for `session_id, task_id` and
//! left `parent_run_id` at its `Run::new` default, so across this
//! repo's 5079 real runs `task_id` was set on ZERO of them — the
//! execution tree and the plan DAG both existed with no edge between
//! them, and s37's "a plan-vs-actual diff is now possible" claim was
//! false. `--task` and `--parent-run` ([`DispatchBinding`]) populate
//! exactly those two fields, and [`end`] supplies the terminal status
//! `begin` never could: `begin` mints `RunStatus::Running` and, before
//! s40, NOTHING in the CLI ever closed it, so every dispatched run
//! stayed `Running` forever.
//!
//! Both flags stay OPTIONAL and both keys stay
//! `skip_serializing_if = "Option::is_none"`, so the single-agent,
//! no-plan dispatch — the overwhelming and correct case — still writes
//! the byte-identical manifest it always did.
//!
//! `--task` joins the FAIL-LOUD half (exit `2`): a typo must never
//! persist a dangling `Run.task_id`, which is the very
//! relationship-asserted-without-data defect s40 exists to remove.
//! `--parent-run` deliberately does NOT — see
//! [`DispatchBinding::parent_run_id`].
//!
//! # `canon dispatch diff` — the reader those two edges finally feed
//! [`run_diff`] is the surface s37 claimed and could not deliver: the
//! DECLARED plan DAG (`Task.depends_on`) against the OBSERVED
//! execution graph (`Run.task_id` + `Run.parent_run_id`), classified
//! into satisfied / declared-not-observed / observed-not-declared. It
//! READS both halves and it never gates — see [`run_diff`]'s own doc
//! for why an undeclared edge is information rather than a violation.
//! It reads the private side-channel alongside the canonical tier,
//! because a dispatched run — still the only run that carries a
//! `task_id` — lives only in the former until the reconciliation step
//! this module's own doc anticipates exists.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use canon_ingest::{find_plan_adapter, WriteBackError};
use canon_learn::guidance::retrieve_guidance;
use canon_learn::{LearnConfig, ParquetStrategyStore};
use canon_model::envelope::{Actor, Envelope, RecordKind};
use canon_model::ids::{RunId, TaskId};
use canon_model::records::{Run, RunStatus};
use canon_model::{RegimeKey, RoleId};
use canon_store::write_atomic;
use serde_json::Value;

use crate::context::resolve_repo_root;

/// The private side-channel directory a dispatch record lands under,
/// relative to the repo root (module doc: never `canon-store`'s git
/// tier).
pub const DISPATCH_DIR: &str = ".canon/dispatch";

#[derive(Debug, thiserror::Error)]
pub enum DispatchError {
    /// `--role` disagrees with `--regime`'s own leading segment — the
    /// same caller-contract check `canon retrieve` makes (design
    /// decision 1: regime_key already embeds role as its first segment).
    #[error(
        "--role `{role}` does not match --regime `{regime_key}`'s own leading role segment `{regime_role}` — pass the SAME role to both"
    )]
    RoleRegimeMismatch { role: String, regime_key: String, regime_role: String },

    /// The dispatch record could not be written to the side-channel.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// The `Run` manifest could not be serialized (should never happen —
    /// `Run` is always `Serialize`).
    #[error("serializing the dispatch Run manifest: {0}")]
    Serialize(String),

    /// `--task <id>` (s40 task 1.2) names a task no configured plan
    /// source carries. Raised BEFORE the mint and the write:
    /// persisting a dangling `Run.task_id` would assert exactly the
    /// relationship s40 exists to stop asserting without data.
    #[error(
        "--task `{task_id}` names no task in this repo's plan corpus (consulted: {consulted}) — fix the id, or dispatch without --task rather than record a dangling binding"
    )]
    TaskNotFound { task_id: String, consulted: String },

    /// `--task` was given by a repo that configures no plan corpus at
    /// all. DISTINCT from [`DispatchError::TaskNotFound`] on purpose:
    /// "your id is wrong" and "there is nowhere for any id to be right"
    /// are different operator problems with different fixes, and
    /// collapsing them would send the caller hunting for a typo in a
    /// repo that has no plan sources to typo against.
    #[error(
        "--task `{task_id}` cannot be resolved: this repo configures no plan sources (canon.yaml has no `plans.sources` entries), so no plan corpus exists for a task id to live in — configure `plans:`, or dispatch without --task"
    )]
    NoPlanSources { task_id: String },

    /// The plan corpus could not be consulted at all — a malformed
    /// `canon.yaml` `plans:` section (which `crate::plans` already
    /// fails loud on) or an unreadable located plan document. Never
    /// collapsed into "task not found": a corpus that cannot be read
    /// has not answered the question either way.
    #[error("resolving --task `{task_id}` against the plan corpus: {detail}")]
    PlanCorpus { task_id: String, detail: String },

    /// `canon dispatch end --run <id>` found no manifest for `<id>`.
    /// Names the id AND the path consulted, so the usual cause — a run
    /// begun against a DIFFERENT repo root — is readable straight off
    /// the error.
    #[error("--run `{run_id}` has no dispatch manifest at {path} — only a run begun by `canon dispatch begin` on this repo can be ended")]
    NoSuchRun { run_id: String, path: String },

    /// `canon dispatch end` on a run that already carries `ended_at`.
    /// A LOUD failure, never a silent overwrite: `ended_at` and the
    /// terminal status are PROVENANCE — the observed close of a real
    /// run — so re-closing would rewrite that history in place, with a
    /// different status at a later instant, leaving no trace that the
    /// first close ever happened.
    #[error(
        "--run `{run_id}` already ended at {ended_at} with status `{status}` — ended_at is provenance and re-closing would silently rewrite it; remove the manifest deliberately if the first close was wrong"
    )]
    AlreadyEnded { run_id: String, ended_at: String, status: &'static str },

    /// The manifest exists but does not deserialize as a `Run`.
    #[error("the dispatch manifest at {path} is not a readable Run record: {detail}")]
    Unreadable { path: String, detail: String },
}

impl DispatchError {
    /// Which exit class this error is: `2` when the INVOCATION is
    /// fixable (bad flags, a bad id, a corpus that cannot answer), `1`
    /// when the invocation was fine and the machine failed under it.
    /// The split the rest of canon-cli uses, kept in ONE exhaustive
    /// match so a variant added later has to declare its class here
    /// rather than silently inherit `1` from a catch-all arm in a
    /// wrapper.
    fn is_usage(&self) -> bool {
        match self {
            Self::RoleRegimeMismatch { .. }
            | Self::TaskNotFound { .. }
            | Self::NoPlanSources { .. }
            | Self::PlanCorpus { .. }
            | Self::NoSuchRun { .. }
            | Self::AlreadyEnded { .. }
            | Self::Unreadable { .. } => true,
            Self::Io(_) | Self::Serialize(_) => false,
        }
    }
}

/// What [`begin`] produced: the minted run id, the side-channel path the
/// manifest was written to, and the guidance snapshot recorded into it.
#[derive(Debug, Clone)]
pub struct Begun {
    pub run_id: RunId,
    pub manifest_path: PathBuf,
    pub run: Run,
}

/// Resolve `<repo>`'s configured learn root and open the `strategies`
/// parquet tier (same resolution `canon retrieve` uses — never a second
/// convention).
fn open_strategy_store(repo: &Path) -> ParquetStrategyStore {
    let canon_yaml = repo.join("canon.yaml");
    let learn_config =
        std::fs::read_to_string(&canon_yaml).ok().and_then(|text| LearnConfig::from_manifest(&text).ok()).unwrap_or_default();
    ParquetStrategyStore::open(repo.join(learn_config.root).join("strategies"))
}

/// The two OPTIONAL edges a dispatch may declare (s40
/// (`plan-vs-actual-diff`), task 1.1) — a named struct rather than two
/// adjacent `Option` positionals on [`begin`], so a caller cannot
/// transpose them and so [`DispatchBinding::default`] IS the pre-s40
/// "neither flag given" case by construction.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DispatchBinding {
    /// The plan task this run serves (`Run.task_id`). VALIDATED against
    /// the configured plan corpus before anything is written — see
    /// [`validate_task_binding`].
    pub task_id: Option<TaskId>,
    /// The run that dispatched this one (`Run.parent_run_id`).
    ///
    /// Deliberately NOT existence-checked. A parent may legitimately be
    /// dispatched by a process that has not yet flushed its own
    /// manifest, so verifying the parent here would make dispatch ORDER
    /// load-bearing — turning a benign race into a hard failure and
    /// forbidding the ordinary case of a child announcing its lineage
    /// first. The `RunId` grammar (a ULID) is the only check: a
    /// malformed value fails at clap parse time ([`parse_run_id`]), and
    /// a `parent_run_id` naming a run that never materializes simply
    /// contributes no observed edge to `canon dispatch diff`, which
    /// reports and never gates.
    pub parent_run_id: Option<RunId>,
}

/// Resolve `task_id` against the repo's configured plan corpus (s40
/// task 1.2), reusing the EXACT path `canon gate task` resolves a flip
/// through (`crate::gate::run_task`): `canon.yaml`'s `plans:` sources,
/// each dialect looked up in `canon_ingest::plan_registry`, and the
/// first source whose [`PlanWriteBack::locate_task`] finds the change's
/// document wins. Never a second resolution convention — a task
/// bindable here but unflippable by the gate (or the reverse) would be
/// its own divergence.
///
/// Two deliberate departures from `canon gate task`, both because this
/// is a READ:
///
/// 1. **No compat default.** `crate::plans::load_plan_sources_for_gate`
///    substitutes `[{ dialect: openspec, root: <repo> }]` when `plans:`
///    is absent, so pre-s35 gate consumers keep working. `--task` is
///    new in s40 and has no such consumer, and reporting "no task `x`
///    in openspec @ <repo>" against a source the operator never
///    configured is misleading. Zero configured sources is therefore
///    its own error, [`DispatchError::NoPlanSources`].
/// 2. **Row-level, not file-level.** `locate_task` is FILE existence
///    only (its own doc): a `<change_id>` resolves to a document
///    whether or not row `<n>` is inside it. So the winning dialect's
///    PURE [`PlanWriteBack::flip_task`] serves as the row probe and its
///    output is DISCARDED — it is the only row-existence check the
///    trait exposes, it takes the already-read document text and
///    returns a new `String`, and this function performs no write of
///    any kind. A dialect that cannot flip at all
///    ([`WriteBackError::Unsupported`], or no registered write-back)
///    can only answer at document granularity, and that answer is
///    accepted rather than inflated into a false "unknown task".
fn validate_task_binding(repo: &Path, task_id: &TaskId) -> Result<(), DispatchError> {
    let named = || task_id.as_str().to_string();
    let sources = crate::plans::load_plan_sources_from_config(&repo.join("canon.yaml"), repo)
        .map_err(|e| DispatchError::PlanCorpus { task_id: named(), detail: e.to_string() })?;
    if sources.is_empty() {
        return Err(DispatchError::NoPlanSources { task_id: named() });
    }

    let mut consulted: Vec<String> = Vec::new();
    for src in &sources {
        consulted.push(format!("{} @ {}", src.dialect(), src.root().display()));
        // `load_plan_sources_from_config` rejects an unregistered
        // dialect before returning, so this lookup cannot miss — the
        // same reasoning (and the same `expect`) `crate::plans`' own
        // scan loop already states.
        let entry = find_plan_adapter(src.dialect()).expect("dialect validated by load_plan_sources_from_config");
        // No write-back capability at all: this source cannot answer
        // the row question. A LATER source may still hold the task, so
        // move on rather than decide here.
        let Some(write_back) = entry.write_back else {
            continue;
        };
        let Some(location) = write_back.locate_task(src.root(), task_id) else {
            continue;
        };
        // First source locating the change wins, exactly as the gate's
        // resolution does — the decision is made HERE, never deferred
        // to a later source that happens to carry the same change id.
        let document = std::fs::read_to_string(&location.document_path).map_err(|e| DispatchError::PlanCorpus {
            task_id: named(),
            detail: format!("cannot read {}: {e}", location.document_path.display()),
        })?;
        return match write_back.flip_task(&document, task_id, "") {
            // The row exists (whether still `[ ]` or already `[x]`).
            // The rewritten document is dropped unread — probe only,
            // never a write.
            Ok(_) => Ok(()),
            Err(WriteBackError::RowNotFound(_)) => Err(DispatchError::TaskNotFound { task_id: named(), consulted: consulted.join("; ") }),
            Err(WriteBackError::Unsupported { .. }) => Ok(()),
        };
    }
    Err(DispatchError::TaskNotFound { task_id: named(), consulted: consulted.join("; ") })
}

/// Mint a `Run` (status `Running`), retrieve the role+regime guidance,
/// record it into the run's `injected_guidance`, and persist the
/// manifest to `<repo>/.canon/dispatch/<run_id>.json`. Returns the
/// [`Begun`] record (run id + path + the in-memory `Run`).
///
/// `binding` carries s40's two optional edges. `binding.task_id` is
/// validated FIRST — before the guidance retrieval, the mint, and the
/// write — so a rejected id leaves nothing at all on disk. A
/// [`DispatchBinding::default`] (neither flag given) reproduces the
/// pre-s40 manifest byte-for-byte: `Run::new` already starts
/// `parent_run_id: None`, and both fields are
/// `skip_serializing_if = "Option::is_none"`, so their keys stay ABSENT
/// rather than `null` — load-bearing beyond cosmetics, since canon's
/// write-time idempotence keys on a content digest over these bytes.
pub fn begin(repo: &Path, role: &RoleId, regime_key: &RegimeKey, agent_id: &str, binding: &DispatchBinding) -> Result<Begun, DispatchError> {
    if regime_key.role() != role.as_str() {
        return Err(DispatchError::RoleRegimeMismatch {
            role: role.as_str().to_string(),
            regime_key: regime_key.as_str().to_string(),
            regime_role: regime_key.role().to_string(),
        });
    }
    let repo = resolve_repo_root(repo);
    if let Some(task_id) = &binding.task_id {
        validate_task_binding(&repo, task_id)?;
    }
    let store = open_strategy_store(&repo);
    let guidance = retrieve_guidance(&store, role, regime_key, None);

    let run_id = RunId::new();
    let now = chrono::Utc::now();
    let actor = Actor::new(agent_id.to_string(), role.clone());
    let run =
        Run::new(Envelope::current(RecordKind::Run, now, actor), run_id, None, binding.task_id.clone(), RunStatus::Running, now, None)
            .with_injected_guidance(guidance);
    // The builder, not a field poke — `Run::with_parent_run_id`'s own
    // doc is that the edge is recorded exactly once, at the moment it
    // becomes known; a root dispatch must leave the field untouched so
    // its key stays absent from the manifest.
    let run = match binding.parent_run_id {
        Some(parent) => run.with_parent_run_id(parent),
        None => run,
    };

    let manifest_path = repo.join(DISPATCH_DIR).join(format!("{run_id}.json"));
    let json = serde_json::to_string_pretty(&run).map_err(|e| DispatchError::Serialize(e.to_string()))?;
    // Atomic write (canon-store's shared tempfile+rename primitive): a
    // replay depends on this manifest being complete, so a mid-write
    // kill must never leave a torn `.canon/dispatch/<run_id>.json`.
    write_atomic(&manifest_path, json.as_bytes())?;

    Ok(Begun { run_id, manifest_path, run })
}

/// `canon dispatch begin`'s CLI wrapper: `0` on a written manifest, `2`
/// on a usage failure (a `--role`/`--regime` mismatch, or a `--task`
/// naming no task / no plan corpus to name one in), `1` on a
/// write/serialize failure.
pub fn run_begin(repo: &Path, role: &RoleId, regime_key: &RegimeKey, agent_id: &str, binding: &DispatchBinding, json: bool) -> ExitCode {
    match begin(repo, role, regime_key, agent_id, binding) {
        Ok(begun) => {
            if json {
                // The binding keys are OMITTED when unset, mirroring
                // the manifest's own `skip_serializing_if` discipline:
                // an unbound dispatch's `--json` shape stays exactly
                // what it was before s40, rather than gaining two
                // permanently-`null` keys every consumer must ignore.
                let mut summary = serde_json::Map::new();
                summary.insert("run_id".to_string(), serde_json::json!(begun.run_id.to_string()));
                summary.insert("manifest".to_string(), serde_json::json!(begun.manifest_path.display().to_string()));
                summary.insert("injected_guidance".to_string(), serde_json::json!(begun.run.injected_guidance));
                if let Some(task_id) = &begun.run.task_id {
                    summary.insert("task_id".to_string(), serde_json::json!(task_id.as_str()));
                }
                if let Some(parent) = begun.run.parent_run_id {
                    summary.insert("parent_run_id".to_string(), serde_json::json!(parent.to_string()));
                }
                println!("{}", serde_json::to_string_pretty(&summary).expect("summary is always serializable"));
            } else {
                println!(
                    "dispatch {} begun for {} — recorded {} guidance item(s) -> {}",
                    begun.run_id,
                    regime_key.as_str(),
                    begun.run.injected_guidance.len(),
                    begun.manifest_path.display()
                );
                if let Some(task_id) = &begun.run.task_id {
                    println!("  bound to plan task {task_id}");
                }
                if let Some(parent) = begun.run.parent_run_id {
                    println!("  dispatched by run {parent}");
                }
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("canon dispatch begin: {err}");
            ExitCode::from(if err.is_usage() { 2 } else { 1 })
        }
    }
}

// ─── s40 (`plan-vs-actual-diff`), task 1: value parsers + the close ───

/// `--task`'s clap `value_parser` (s40 task 1.1), mirroring
/// `crate::retrieve::parse_role`'s established shape: the `TaskId`
/// GRAMMAR check only — `<change_id>#<n>`. Whether such a task EXISTS
/// is [`validate_task_binding`]'s job at dispatch time, against the
/// live plan corpus, because clap has not resolved `--repo` yet.
pub fn parse_task_id(s: &str) -> Result<TaskId, String> {
    TaskId::parse(s).map_err(|e| e.to_string())
}

/// `--parent-run`'s and `--run`'s clap `value_parser`: the `RunId`
/// grammar (a ULID). For `--parent-run` this is the ONLY check there
/// is — see [`DispatchBinding::parent_run_id`] for why the parent's
/// existence is deliberately not verified.
pub fn parse_run_id(s: &str) -> Result<RunId, String> {
    RunId::parse(s).map_err(|e| e.to_string())
}

/// `canon dispatch end --status`'s clap `value_parser`. The domain is
/// exactly the two TERMINAL outcomes a dispatcher can report about its
/// own run: `pending`/`running` are not terminal, and `aborted` is an
/// ingest-time classification of a transcript that stopped, not
/// something a closing dispatcher observes about itself.
pub fn parse_run_status(s: &str) -> Result<RunStatus, String> {
    match s {
        "succeeded" => Ok(RunStatus::Succeeded),
        "failed" => Ok(RunStatus::Failed),
        other => Err(format!("`{other}` is not a terminal run status — expected one of: succeeded, failed")),
    }
}

/// A `RunStatus`'s on-disk spelling, mirroring the enum's own
/// `#[serde(rename_all = "snake_case")]`. An exhaustive match rather
/// than a `Debug` render, so a variant added later has to choose its
/// wording here instead of silently leaking a Rust identifier into an
/// operator-facing message.
fn status_slug(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Pending => "pending",
        RunStatus::Running => "running",
        RunStatus::Succeeded => "succeeded",
        RunStatus::Failed => "failed",
        RunStatus::Aborted => "aborted",
    }
}

/// What [`end`] produced: the closed run id, the manifest rewritten in
/// place, and the updated `Run`.
#[derive(Debug, Clone)]
pub struct Ended {
    pub run_id: RunId,
    pub manifest_path: PathBuf,
    pub run: Run,
}

/// Close a dispatched run: read `<repo>/.canon/dispatch/<run_id>.json`,
/// stamp a terminal [`RunStatus`] and `ended_at`, and rewrite the
/// manifest through the SAME atomic write [`begin`] used to create it
/// (s40). Before this, `begin` minted `RunStatus::Running` and nothing
/// in the CLI ever closed it — only session ingest, reconstructing a
/// finished transcript post hoc, ever wrote a terminal status — so
/// every dispatched run stayed `Running` forever and the flywheel
/// funnel's last stage had nothing to compute from.
///
/// # Re-closing is a LOUD failure, never a silent overwrite
/// A run already carrying `ended_at` is rejected
/// ([`DispatchError::AlreadyEnded`]). `ended_at` and the terminal
/// status are PROVENANCE — the observed close of a real run — so a
/// second `end` would rewrite that history in place, at a later
/// instant and possibly under a different status, leaving no trace the
/// first close happened. An operator who genuinely closed the wrong
/// run removes the manifest deliberately; the CLI never guesses that
/// for them.
pub fn end(repo: &Path, run_id: RunId, status: RunStatus) -> Result<Ended, DispatchError> {
    let repo = resolve_repo_root(repo);
    let manifest_path = repo.join(DISPATCH_DIR).join(format!("{run_id}.json"));
    let text = match std::fs::read_to_string(&manifest_path) {
        Ok(text) => text,
        // Only ABSENCE is "no such run". A permission or I/O failure on
        // a manifest that does exist stays a real failure (exit `1`),
        // never reported as a run the operator never began.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(DispatchError::NoSuchRun { run_id: run_id.to_string(), path: manifest_path.display().to_string() });
        }
        Err(e) => return Err(DispatchError::Io(e)),
    };
    let mut run: Run = serde_json::from_str(&text)
        .map_err(|e| DispatchError::Unreadable { path: manifest_path.display().to_string(), detail: e.to_string() })?;
    if let Some(ended_at) = run.ended_at {
        return Err(DispatchError::AlreadyEnded {
            run_id: run_id.to_string(),
            ended_at: ended_at.to_rfc3339(),
            status: status_slug(run.status),
        });
    }

    run.status = status;
    run.ended_at = Some(chrono::Utc::now());
    let json = serde_json::to_string_pretty(&run).map_err(|e| DispatchError::Serialize(e.to_string()))?;
    // Same atomicity argument `begin` makes: a mid-write kill must
    // never leave a torn manifest, and here it would additionally
    // destroy the only record of a run that DID happen.
    write_atomic(&manifest_path, json.as_bytes())?;

    Ok(Ended { run_id, manifest_path, run })
}

/// `canon dispatch end`'s CLI wrapper: `0` on a rewritten manifest, `2`
/// on a usage failure (unknown run id, an already-ended run, an
/// unreadable manifest), `1` on a write/serialize failure — the same
/// three-way contract [`run_begin`] and `canon gate` use.
pub fn run_end(repo: &Path, run_id: RunId, status: RunStatus, json: bool) -> ExitCode {
    match end(repo, run_id, status) {
        Ok(ended) => {
            if json {
                let summary = serde_json::json!({
                    "run_id": ended.run_id.to_string(),
                    "manifest": ended.manifest_path.display().to_string(),
                    "status": status_slug(ended.run.status),
                    "ended_at": ended.run.ended_at,
                });
                println!("{}", serde_json::to_string_pretty(&summary).expect("summary is always serializable"));
            } else {
                println!("dispatch {} ended `{}` -> {}", ended.run_id, status_slug(ended.run.status), ended.manifest_path.display());
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("canon dispatch end: {err}");
            ExitCode::from(if err.is_usage() { 2 } else { 1 })
        }
    }
}

// ─── s40 (`plan-vs-actual-diff`), task 2: `canon dispatch diff` ───

/// One directed edge between two plan tasks: `from` is the antecedent,
/// `to` the dependent — `from` is meant to precede `to` (s40
/// (`plan-vs-actual-diff`), task 2.1).
///
/// Both edge sets name the ANTECEDENT first, which is the only reason
/// they are comparable at all:
/// - DECLARED reads [`canon_model::records::Task::depends_on`]: a task
///   listing `A` among its own dependencies yields `A -> task`.
/// - OBSERVED reads a dispatch lineage hop: a child run's
///   [`Run::parent_run_id`] resolves to the parent run's own
///   [`Run::task_id`], yielding `parent.task_id -> child.task_id`.
///
/// The DERIVED [`Ord`] is a lexicographic compare of the two id
/// strings and exists ONLY to give [`BTreeSet`] its identity and
/// dedup. It is never the order anything is rendered in — rendering
/// sorts by `edge_sort_key`, which reads `#1.2` before `#1.10`
/// exactly as `canon query --kind task` already does.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TaskEdge {
    pub from: TaskId,
    pub to: TaskId,
}

/// Which of the two edge sets an edge turned up in (task 2.2). A named
/// class carried BY each edge, rather than three positionally-
/// meaningful lists, so the class travels with the edge into both
/// renderers and the `--json` `class` field can never disagree with
/// the heading an edge was printed under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeClass {
    /// Declared by `Task.depends_on` AND observed in dispatch lineage:
    /// the plan held.
    Satisfied,
    /// The plan declares the ordering; nothing ever ran that way.
    DeclaredNotObserved,
    /// It ran that way; the plan never declared it. INFORMATION, never
    /// a violation — see [`run_diff`]'s never-gates contract.
    ObservedNotDeclared,
}

impl EdgeClass {
    /// Every class, in the order both renderers list them: the plan
    /// holding first, then the two ways it does not.
    pub const ALL: [EdgeClass; 3] = [EdgeClass::Satisfied, EdgeClass::DeclaredNotObserved, EdgeClass::ObservedNotDeclared];

    /// The report vocabulary — ONE set of strings shared by the human
    /// list headings, the `--json` per-edge `class` field, AND the
    /// `--json` `counts` object's own keys, so a consumer can index
    /// `counts[edge.class]` and the two renderings can never drift
    /// into two names for one class.
    pub fn as_str(self) -> &'static str {
        match self {
            EdgeClass::Satisfied => "satisfied",
            EdgeClass::DeclaredNotObserved => "declared-not-observed",
            EdgeClass::ObservedNotDeclared => "observed-not-declared",
        }
    }
}

/// One edge plus the class it fell into (task 2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedEdge {
    pub edge: TaskEdge,
    pub class: EdgeClass,
}

/// The whole plan-vs-actual comparison (tasks 2.2/2.3) — what
/// [`diff`] produces and both renderers consume.
#[derive(Debug, Clone)]
pub struct PlanActualDiff {
    /// How many DISTINCT edges the declared side carried.
    pub declared: usize,
    /// How many DISTINCT edges the observed side carried.
    pub observed: usize,
    /// The UNION of the two sides: one entry per distinct edge, each
    /// carrying its own [`EdgeClass`], in ONE total, data-derived
    /// order (`edge_sort_key`). Filtering this by class PRESERVES
    /// that order, so every rendered per-class list is sorted by
    /// `(from, to)` without a second sort that could drift from it.
    pub edges: Vec<ClassifiedEdge>,
    /// Degrade diagnostics: a tier that could not be read
    /// (`read_kind`), a dispatch manifest that would not parse
    /// (`read_dispatch_manifests`). Each one only NARROWS a side;
    /// none is an error, and [`run_diff`] prints them on stderr and
    /// still exits `0`.
    pub notes: Vec<String>,
}

impl PlanActualDiff {
    fn new(declared: BTreeSet<TaskEdge>, observed: BTreeSet<TaskEdge>, notes: Vec<String>) -> Self {
        Self { declared: declared.len(), observed: observed.len(), edges: classify(&declared, &observed), notes }
    }

    /// Every edge in `class`, in [`Self::edges`]' own total order.
    pub fn in_class(&self, class: EdgeClass) -> impl Iterator<Item = &TaskEdge> + '_ {
        self.edges.iter().filter(move |classified| classified.class == class).map(|classified| &classified.edge)
    }

    /// How many edges fell into `class`.
    pub fn count(&self, class: EdgeClass) -> usize {
        self.edges.iter().filter(|classified| classified.class == class).count()
    }
}

/// One task id's ordering key — named solely to keep
/// [`edge_sort_key`]'s signature under clippy's `type_complexity`
/// threshold, the same reason `crate::tiers::LenientTiers` exists.
type TaskSortKey = (String, Vec<u64>, String);

/// A task id's ordering key: `(owning change, task-number segments,
/// full id)`. The first two components are
/// [`crate::query::task_number_key`]'s — the SAME decomposition
/// `canon query --kind task` orders its rows by, extracted there
/// rather than re-derived here so a plan-graph edge and a plan-task
/// row can never sort by two different readings of one id.
///
/// The trailing full id makes the order TOTAL rather than merely
/// well-defined: the shared key drops any segment too large for a
/// `u64`, so two distinct ids can in principle tie on
/// `(change, segments)`, and a report whose row order then depends on
/// which of two tied edges happened to be visited first is not
/// reproducible.
fn task_sort_key(task: &TaskId) -> TaskSortKey {
    let (change, segments) = crate::query::task_number_key(task.as_str());
    (change, segments, task.as_str().to_string())
}

/// [`PlanActualDiff::edges`]' total order: `from`'s key, then `to`'s.
fn edge_sort_key(edge: &TaskEdge) -> (TaskSortKey, TaskSortKey) {
    (task_sort_key(&edge.from), task_sort_key(&edge.to))
}

/// Task 2.2: fold the two edge sets into ONE classified, totally
/// ordered list. The union is taken first and each class derived from
/// membership, so every distinct edge appears exactly once and no edge
/// can be double-counted as both satisfied and observed-only.
fn classify(declared: &BTreeSet<TaskEdge>, observed: &BTreeSet<TaskEdge>) -> Vec<ClassifiedEdge> {
    let mut edges: Vec<ClassifiedEdge> = declared
        .union(observed)
        .map(|edge| {
            let class = if !observed.contains(edge) {
                EdgeClass::DeclaredNotObserved
            } else if declared.contains(edge) {
                EdgeClass::Satisfied
            } else {
                EdgeClass::ObservedNotDeclared
            };
            ClassifiedEdge { edge: edge.clone(), class }
        })
        .collect();
    edges.sort_by_cached_key(|classified| edge_sort_key(&classified.edge));
    edges
}

fn str_field<'a>(record: &'a Value, field: &str) -> Option<&'a str> {
    record.get(field).and_then(Value::as_str)
}

/// `record`'s `field` as a [`TaskId`], or `None`. An id that does not
/// parse is DROPPED, never `unwrap()`ed: these records come off disk
/// or out of a foreign plan import, so a malformed id is a corpus fact
/// the report must survive, not a caller-contract violation.
fn task_id_field(record: &Value, field: &str) -> Option<TaskId> {
    str_field(record, field).and_then(|s| TaskId::parse(s).ok())
}

/// The DECLARED edge set (task 2.1): one `dep -> task` edge for every
/// entry in every task's own `depends_on`.
///
/// A self-edge (`dep == task`) is dropped. A task cannot be its own
/// antecedent, so such an entry declares nothing; dropping it here as
/// well as on the observed side keeps the two sides SYMMETRIC — were
/// it kept only here, a plan's self-dependency would be reported
/// forever as declared-not-observed, since [`observed_edges`] can
/// never produce the matching edge to satisfy it.
fn declared_edges(tasks: &[Value]) -> BTreeSet<TaskEdge> {
    let mut edges = BTreeSet::new();
    for task in tasks {
        let Some(to) = task_id_field(task, "task_id") else { continue };
        let Some(depends_on) = task.get("depends_on").and_then(Value::as_array) else { continue };
        for dep in depends_on {
            let Some(from) = dep.as_str().and_then(|s| TaskId::parse(s).ok()) else { continue };
            if from == to {
                continue;
            }
            edges.insert(TaskEdge { from, to: to.clone() });
        }
    }
    edges
}

/// The OBSERVED edge set (task 2.1): one `parent.task_id ->
/// child.task_id` edge per run carrying BOTH a `task_id` and a
/// `parent_run_id` whose parent itself names a task.
///
/// A run missing EITHER field contributes no edge as a child — which,
/// on this repo today, is every one of its 5079 ingested runs
/// (`task_id` on 0 of them). That is a legitimately EMPTY observed
/// side and a normal zero-edge report, never an error: session ingest
/// reconstructs a run from a transcript that cannot know a plan task,
/// and only `canon dispatch begin --task` binds one. A run missing
/// only `parent_run_id` is still usable as a PARENT — a root dispatch
/// bound to a task is exactly the far end of the first edge.
///
/// A self-edge (parent and child on the same task) is dropped: that is
/// one task spawning a helper agent, execution detail rather than a
/// dependency. See [`declared_edges`] for why the declared side drops
/// it too.
fn observed_edges(runs: &[Value]) -> BTreeSet<TaskEdge> {
    // run_id -> the task that run served, for every run that names
    // one. FIRST writer wins, and `diff` reads the canonical tier
    // before the dispatch side-channel, so a run_id present in both
    // resolves to its canonical binding rather than to whichever
    // source happened to be scanned last.
    let mut task_of: BTreeMap<&str, TaskId> = BTreeMap::new();
    for run in runs {
        let (Some(run_id), Some(task)) = (str_field(run, "run_id"), task_id_field(run, "task_id")) else { continue };
        task_of.entry(run_id).or_insert(task);
    }

    let mut edges = BTreeSet::new();
    for run in runs {
        let Some(to) = task_id_field(run, "task_id") else { continue };
        let Some(parent_run_id) = str_field(run, "parent_run_id") else { continue };
        let Some(from) = task_of.get(parent_run_id) else { continue };
        if *from == to {
            continue;
        }
        edges.insert(TaskEdge { from: from.clone(), to });
    }
    edges
}

/// Read `kind`'s records through the SAME path `canon query --kind <k>`
/// uses ([`crate::query::run`]: the tier-registry fan-out, its lenient
/// per-rung build and its hot-rung supersession fold), never a second
/// read convention for one kind.
///
/// An unroutable or unreachable tier degrades THIS SIDE ONLY to empty,
/// with a note — the same per-tier degrade `canon query`'s own lenient
/// build already performs, not a new error path. A diff over half the
/// corpus is still a useful report, and aborting would turn a
/// read-only surface into a failure mode over a condition it does not
/// own.
fn read_kind(repo: &Path, kind: RecordKind, notes: &mut Vec<String>) -> Vec<Value> {
    match crate::query::run(repo, None, kind, None, None, None, None) {
        Ok(outcome) => outcome.records.into_iter().map(|record| record.0).collect(),
        Err(err) => {
            notes.push(format!("`{}` records unavailable ({err}) — that side of the diff is empty", kind.as_str()));
            Vec::new()
        }
    }
}

/// Read `<repo>/.canon/dispatch/*.json` (this module's own private
/// side-channel, [`DISPATCH_DIR`]) as additional `Run` manifests.
///
/// Necessary, not belt-and-braces: [`begin`] deliberately writes a
/// live dispatch manifest OUTSIDE `canon-store`'s git tier (module
/// doc — a hive-path collision with the later post-hoc ingest `Run`
/// would be a hard error), and the reconciliation step that would fold
/// it into the canonical tier does not exist yet. So a freshly
/// dispatched run — the ONLY kind of run that carries a `task_id` at
/// all — is invisible to [`read_kind`], and a diff that read only the
/// tier would report zero observed edges no matter how many runs were
/// dispatched with `--task`/`--parent-run`.
///
/// Paths are sorted so the scan order is data-derived rather than
/// directory order. An absent directory is the ordinary
/// never-dispatched-here case and produces no note; a manifest that
/// will not read or parse is skipped WITH one.
fn read_dispatch_manifests(repo: &Path, notes: &mut Vec<String>) -> Vec<Value> {
    let Ok(entries) = std::fs::read_dir(repo.join(DISPATCH_DIR)) else { return Vec::new() };
    let mut paths: Vec<PathBuf> =
        entries.filter_map(Result::ok).map(|entry| entry.path()).filter(|path| path.extension().is_some_and(|ext| ext == "json")).collect();
    paths.sort();

    let mut runs = Vec::with_capacity(paths.len());
    for path in paths {
        match std::fs::read_to_string(&path).ok().and_then(|text| serde_json::from_str::<Value>(&text).ok()) {
            Some(manifest) => runs.push(manifest),
            None => notes.push(format!("unreadable dispatch manifest {} — skipped", path.display())),
        }
    }
    runs
}

/// Build the plan-vs-actual comparison for `repo` (tasks 2.1/2.2):
/// DECLARED from every `Task`'s `depends_on`, OBSERVED from every
/// `Run` the canonical tier holds PLUS every live dispatch manifest
/// the side-channel holds. Every read degrades rather than fails, so
/// this function is total — it always produces a report.
pub fn diff(repo: &Path) -> PlanActualDiff {
    let repo = resolve_repo_root(repo);
    let mut notes = Vec::new();
    let tasks = read_kind(&repo, RecordKind::Task, &mut notes);
    let mut runs = read_kind(&repo, RecordKind::Run, &mut notes);
    runs.extend(read_dispatch_manifests(&repo, &mut notes));
    PlanActualDiff::new(declared_edges(&tasks), observed_edges(&runs), notes)
}

/// The default human report: one counts line, then every class with
/// its own count and its edges (`(none)` when a class is empty, so the
/// three headings are always present and the shape never depends on
/// the data). [`PlanActualDiff::notes`] are NOT in here — [`run_diff`]
/// puts them on stderr, keeping stdout a clean report.
fn format_human(diff: &PlanActualDiff) -> String {
    let mut out = format!("canon dispatch diff: declared={} observed={}", diff.declared, diff.observed);
    for class in EdgeClass::ALL {
        out.push_str(&format!(" {}={}", class.as_str(), diff.count(class)));
    }
    for class in EdgeClass::ALL {
        out.push_str(&format!("\n  {} ({}):", class.as_str(), diff.count(class)));
        let mut listed = false;
        for edge in diff.in_class(class) {
            listed = true;
            out.push_str(&format!("\n    {} -> {}", edge.from, edge.to));
        }
        if !listed {
            out.push_str("\n    (none)");
        }
    }
    out
}

/// `--json`: the same data as [`format_human`], in this stable shape.
/// Keys appear here in the order they actually serialize —
/// `serde_json::Map` is a `BTreeMap` in this workspace (no
/// `preserve_order` feature anywhere in the lockfile), so every object
/// is emitted in sorted key order, which is one more thing a consumer
/// diffing two reports can rely on:
///
/// ```text
/// {
///   "counts":   { "<class>": <count>, ... },   // every class, always
///   "declared": <count of distinct declared edges>,
///   "edges":    [ { "class": "<class>",
///                   "from": "<task_id>",
///                   "to": "<task_id>" }, ... ],
///   "notes":    [ "<degrade diagnostic>", ... ],
///   "observed": <count of distinct observed edges>
/// }
/// ```
///
/// `edges` is the UNION of both sides, one entry per distinct edge, in
/// [`PlanActualDiff::edges`]' own total order; grouping by class is a
/// filter, which is why no per-class array is duplicated here.
/// `counts` is keyed by the SAME [`EdgeClass::as_str`] vocabulary each
/// edge's `class` carries, so `counts[edge.class]` is a valid lookup.
fn format_json(diff: &PlanActualDiff) -> String {
    let mut counts = serde_json::Map::new();
    for class in EdgeClass::ALL {
        counts.insert(class.as_str().to_string(), serde_json::json!(diff.count(class)));
    }
    let payload = serde_json::json!({
        "declared": diff.declared,
        "observed": diff.observed,
        "counts": counts,
        "edges": diff
            .edges
            .iter()
            .map(|classified| serde_json::json!({
                "from": classified.edge.from.as_str(),
                "to": classified.edge.to.as_str(),
                "class": classified.class.as_str(),
            }))
            .collect::<Vec<_>>(),
        "notes": diff.notes,
    });
    serde_json::to_string_pretty(&payload).expect("serde_json::Value always serializes")
}

/// `canon dispatch diff [--repo <dir>] [--json]` (task 2.3): render the
/// DECLARED plan DAG against the OBSERVED execution graph.
///
/// # This REPORTS; it never gates
/// ALWAYS exits `0` on a successful read — there is exactly one return
/// value in this function and it is [`ExitCode::SUCCESS`]. A reader
/// will otherwise assume a `diff` command is a gate, so, explicitly:
/// an observed-not-declared edge is INFORMATION, not a violation.
/// canon records topology; it still does not schedule or execute from
/// it, so a run that took an undeclared path broke no rule canon
/// enforces. This is the same posture `canon divergence status` holds
/// — a read-only capability query over whatever is currently on the
/// ledger. `canon gate check` is where evidence is enforced.
///
/// An empty corpus is therefore a zero-edge report, not an error; so
/// is a corpus whose tiers cannot be read at all (`read_kind`
/// degrades each side independently and [`diff`] carries the reason
/// into [`PlanActualDiff::notes`], printed here on stderr so stdout
/// stays a clean report in both render modes).
pub fn run_diff(repo: &Path, json: bool) -> ExitCode {
    let report = diff(repo);
    for note in &report.notes {
        eprintln!("canon dispatch diff: {note}");
    }
    println!("{}", if json { format_json(&report) } else { format_human(&report) });
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    /// A plan `Task` record's diff-relevant shape: its own id plus the
    /// ids it declares as antecedents.
    fn task(id: &str, depends_on: &[&str]) -> Value {
        serde_json::json!({ "task_id": id, "depends_on": depends_on })
    }

    /// A `Run` record's diff-relevant shape. Both `task_id` and
    /// `parent_run_id` are `Option` on the model AND
    /// `skip_serializing_if = "Option::is_none"`, so a run without one
    /// carries no key at all — which is exactly what passing `None`
    /// here reproduces, rather than a JSON `null` the real corpus
    /// never contains.
    fn run_record(run_id: &str, task_id: Option<&str>, parent_run_id: Option<&str>) -> Value {
        let mut record = serde_json::Map::new();
        record.insert("run_id".to_string(), Value::String(run_id.to_string()));
        if let Some(task_id) = task_id {
            record.insert("task_id".to_string(), Value::String(task_id.to_string()));
        }
        if let Some(parent_run_id) = parent_run_id {
            record.insert("parent_run_id".to_string(), Value::String(parent_run_id.to_string()));
        }
        Value::Object(record)
    }

    fn edge(from: &str, to: &str) -> TaskEdge {
        TaskEdge { from: TaskId::parse(from).expect("test task id"), to: TaskId::parse(to).expect("test task id") }
    }

    /// A corpus carrying exactly one edge of each class: `c#1.1 ->
    /// c#1.2` declared and run, `c#2.1 -> c#2.2` declared only, and
    /// `c#3.1 -> c#3.2` run only.
    fn one_of_each_class() -> PlanActualDiff {
        let tasks = vec![task("c#1.2", &["c#1.1"]), task("c#2.2", &["c#2.1"])];
        let (satisfied_parent, satisfied_child) = (RunId::new().to_string(), RunId::new().to_string());
        let (undeclared_parent, undeclared_child) = (RunId::new().to_string(), RunId::new().to_string());
        let runs = vec![
            run_record(&satisfied_parent, Some("c#1.1"), None),
            run_record(&satisfied_child, Some("c#1.2"), Some(satisfied_parent.as_str())),
            run_record(&undeclared_parent, Some("c#3.1"), None),
            run_record(&undeclared_child, Some("c#3.2"), Some(undeclared_parent.as_str())),
        ];
        PlanActualDiff::new(declared_edges(&tasks), observed_edges(&runs), Vec::new())
    }

    #[test]
    fn a_declared_dependency_that_actually_ran_is_satisfied() {
        let tasks = vec![task("c#1.1", &[]), task("c#1.2", &["c#1.1"])];
        let (parent, child) = (RunId::new().to_string(), RunId::new().to_string());
        let runs = vec![run_record(&parent, Some("c#1.1"), None), run_record(&child, Some("c#1.2"), Some(parent.as_str()))];

        let report = PlanActualDiff::new(declared_edges(&tasks), observed_edges(&runs), Vec::new());
        assert_eq!(report.edges, vec![ClassifiedEdge { edge: edge("c#1.1", "c#1.2"), class: EdgeClass::Satisfied }]);
        // One DISTINCT edge per side, and one union entry — a
        // satisfied edge is never double-counted.
        assert_eq!((report.declared, report.observed), (1, 1));
    }

    #[test]
    fn a_plan_dependency_nothing_ever_ran_is_declared_not_observed() {
        let tasks = vec![task("c#2.2", &["c#2.1"])];

        let report = PlanActualDiff::new(declared_edges(&tasks), observed_edges(&[]), Vec::new());
        assert_eq!(report.edges, vec![ClassifiedEdge { edge: edge("c#2.1", "c#2.2"), class: EdgeClass::DeclaredNotObserved }]);
        assert_eq!((report.declared, report.observed), (1, 0));
    }

    #[test]
    fn a_lineage_hop_the_plan_never_declared_is_observed_not_declared() {
        let (parent, child) = (RunId::new().to_string(), RunId::new().to_string());
        let runs = vec![run_record(&parent, Some("c#3.1"), None), run_record(&child, Some("c#3.2"), Some(parent.as_str()))];

        let report = PlanActualDiff::new(declared_edges(&[]), observed_edges(&runs), Vec::new());
        assert_eq!(report.edges, vec![ClassifiedEdge { edge: edge("c#3.1", "c#3.2"), class: EdgeClass::ObservedNotDeclared }]);
        assert_eq!((report.declared, report.observed), (0, 1));
    }

    /// A parent and child run BOTH serving one task is a helper-agent
    /// spawn, not a dependency, and a task cannot be its own
    /// antecedent. Dropped on BOTH sides — kept on the declared side
    /// only, a plan's self-dependency would be reported forever as
    /// declared-not-observed.
    #[test]
    fn a_self_edge_is_dropped_by_both_sides() {
        let tasks = vec![task("c#4.1", &["c#4.1"])];
        let (parent, child) = (RunId::new().to_string(), RunId::new().to_string());
        let runs = vec![run_record(&parent, Some("c#4.1"), None), run_record(&child, Some("c#4.1"), Some(parent.as_str()))];

        let report = PlanActualDiff::new(declared_edges(&tasks), observed_edges(&runs), Vec::new());
        assert!(report.edges.is_empty(), "a task is never its own antecedent: {:?}", report.edges);
        assert_eq!((report.declared, report.observed), (0, 0));
    }

    /// The shape of every run this repo currently holds: 5073 roots
    /// with neither field, 6 children with a parent but still no
    /// `task_id`. None of them can contribute an edge, and that is a
    /// normal empty observed side.
    #[test]
    fn runs_missing_either_field_contribute_no_observed_edge() {
        let unbound_root = RunId::new().to_string();
        let bound_root = RunId::new().to_string();
        let absent_parent = RunId::new().to_string();
        let runs = vec![
            // Neither field — the overwhelming single-agent case.
            run_record(&unbound_root, None, None),
            // Bound to a task, but its parent names no task, so the
            // FROM end of the edge is unknowable.
            run_record(&RunId::new().to_string(), Some("c#5.2"), Some(unbound_root.as_str())),
            // Bound to a task with no parent: a legitimate root
            // dispatch, usable as a parent but never a child.
            run_record(&bound_root, Some("c#5.1"), None),
            // A bound parent, but this child names no task, so the TO
            // end is unknowable.
            run_record(&RunId::new().to_string(), None, Some(bound_root.as_str())),
            // A parent_run_id naming a run that is not in the corpus
            // at all.
            run_record(&RunId::new().to_string(), Some("c#5.3"), Some(absent_parent.as_str())),
        ];

        assert!(observed_edges(&runs).is_empty());
    }

    #[test]
    fn an_empty_corpus_reports_zero_edges_and_still_succeeds() {
        // No `canon.yaml` at all: BOTH tier reads fail, which is the
        // harshest read this command can meet, and it still reports.
        let dir = TempDir::new().expect("tempdir");

        let report = diff(dir.path());
        assert!(report.edges.is_empty());
        assert_eq!((report.declared, report.observed), (0, 0));
        assert_eq!(report.notes.len(), 2, "one degrade note per side, never an abort: {:?}", report.notes);
        assert_eq!(
            format_human(&report),
            "canon dispatch diff: declared=0 observed=0 satisfied=0 declared-not-observed=0 observed-not-declared=0\
             \n  satisfied (0):\
             \n    (none)\
             \n  declared-not-observed (0):\
             \n    (none)\
             \n  observed-not-declared (0):\
             \n    (none)"
        );
        // `run_diff` has exactly ONE return value and it is
        // `ExitCode::SUCCESS` (`ExitCode` is not comparable, so the
        // never-gates contract is structural); exercise both renderers
        // to prove neither panics on the all-empty case.
        let _ = run_diff(dir.path(), false);
        let _ = run_diff(dir.path(), true);
    }

    #[test]
    fn the_human_report_leads_with_counts_then_lists_every_class() {
        assert_eq!(
            format_human(&one_of_each_class()),
            "canon dispatch diff: declared=2 observed=2 satisfied=1 declared-not-observed=1 observed-not-declared=1\
             \n  satisfied (1):\
             \n    c#1.1 -> c#1.2\
             \n  declared-not-observed (1):\
             \n    c#2.1 -> c#2.2\
             \n  observed-not-declared (1):\
             \n    c#3.1 -> c#3.2"
        );
    }

    #[test]
    fn the_json_shape_keys_counts_by_the_same_class_vocabulary_each_edge_carries() {
        let payload: Value = serde_json::from_str(&format_json(&one_of_each_class())).expect("--json emits valid JSON");

        assert_eq!(payload["declared"], 2);
        assert_eq!(payload["observed"], 2);
        assert_eq!(payload["notes"], serde_json::json!([]));
        assert_eq!(
            payload["edges"],
            serde_json::json!([
                { "from": "c#1.1", "to": "c#1.2", "class": "satisfied" },
                { "from": "c#2.1", "to": "c#2.2", "class": "declared-not-observed" },
                { "from": "c#3.1", "to": "c#3.2", "class": "observed-not-declared" },
            ])
        );
        // `counts[edge.class]` is a valid lookup: the counts object is
        // keyed by the SAME strings the per-edge `class` field carries.
        for edge in payload["edges"].as_array().expect("edges is an array") {
            let class = edge["class"].as_str().expect("class is a string");
            assert_eq!(payload["counts"][class], 1, "counts is not keyed by `{class}`");
        }
    }

    /// Determinism is not "a sort ran": it is that the SAME logical
    /// corpus, constructed in two different input orders, renders
    /// byte-identically — and in the task-NUMBER order `canon query
    /// --kind task` uses, never a lexicographic one.
    #[test]
    fn ordering_is_total_and_independent_of_input_order() {
        let (root, child) = (RunId::new().to_string(), RunId::new().to_string());
        let forward_tasks = vec![task("c#1.2", &["c#1.1"]), task("c#1.10", &["c#1.2"]), task("c#2.1", &["c#1.10"])];
        let forward_runs = vec![run_record(&root, Some("c#1.1"), None), run_record(&child, Some("c#1.2"), Some(root.as_str()))];
        let reverse_tasks: Vec<Value> = forward_tasks.iter().rev().cloned().collect();
        let reverse_runs: Vec<Value> = forward_runs.iter().rev().cloned().collect();

        let forward = PlanActualDiff::new(declared_edges(&forward_tasks), observed_edges(&forward_runs), Vec::new());
        let reverse = PlanActualDiff::new(declared_edges(&reverse_tasks), observed_edges(&reverse_runs), Vec::new());
        assert_eq!(format_human(&forward), format_human(&reverse));
        assert_eq!(format_json(&forward), format_json(&reverse));

        let rendered: Vec<String> = forward.edges.iter().map(|classified| format!("{} -> {}", classified.edge.from, classified.edge.to)).collect();
        assert_eq!(
            rendered,
            vec!["c#1.1 -> c#1.2".to_string(), "c#1.2 -> c#1.10".to_string(), "c#1.10 -> c#2.1".to_string()],
            "`#1.2` sorts before `#1.10`; a lexicographic compare would order them the other way"
        );
    }

    /// A live `canon dispatch begin` manifest lands in the private
    /// side-channel, NOT the canonical tier ([`begin`]'s own module
    /// doc), and no reconciliation step folds it in yet — so a diff
    /// that read only the tier would report zero observed edges no
    /// matter how many runs were dispatched with `--task`.
    #[test]
    fn dispatch_side_channel_manifests_are_read_as_runs() {
        let dir = TempDir::new().expect("tempdir");
        let manifests = dir.path().join(DISPATCH_DIR);
        std::fs::create_dir_all(&manifests).expect("side-channel dir");
        let (parent, child) = (RunId::new().to_string(), RunId::new().to_string());
        for (run_id, task_id, parent_run_id) in [(&parent, "c#6.1", None), (&child, "c#6.2", Some(parent.as_str()))] {
            let record = run_record(run_id, Some(task_id), parent_run_id);
            std::fs::write(manifests.join(format!("{run_id}.json")), serde_json::to_string(&record).expect("manifest")).expect("write manifest");
        }
        // A file that is not JSON at all is skipped WITH a note, never
        // aborting the read.
        std::fs::write(manifests.join("torn.json"), b"{not json").expect("write torn manifest");

        let mut notes = Vec::new();
        let runs = read_dispatch_manifests(dir.path(), &mut notes);
        assert_eq!(runs.len(), 2);
        assert_eq!(notes.len(), 1, "the torn manifest is the only note: {notes:?}");
        assert_eq!(observed_edges(&runs), BTreeSet::from([edge("c#6.1", "c#6.2")]));
    }
}

/// s40 (`plan-vs-actual-diff`) tasks 1.1–1.3 plus the run-lifecycle
/// close. Named apart from this module's `tests` so the binding half
/// and the diff half of s40 can be read (and edited) independently.
#[cfg(test)]
mod begin_tests {
    use super::*;
    use tempfile::TempDir;

    fn role() -> RoleId {
        RoleId::parse("implementer").expect("a literal role slug")
    }

    fn regime() -> RegimeKey {
        RegimeKey::parse("implementer/canon/dispatch/abc123").expect("a literal regime key")
    }

    /// A repo configuring ONE openspec plan source that carries one
    /// change with an open row (`1.1`) and an already-done row (`1.2`)
    /// — the smallest tree [`validate_task_binding`] can actually
    /// resolve a task through.
    fn repo_with_plan_corpus() -> TempDir {
        let tmp = tempfile::tempdir().expect("a temp dir");
        std::fs::write(tmp.path().join("canon.yaml"), "plans:\n  sources:\n    - dialect: openspec\n      root: plans\n")
            .expect("writing canon.yaml");
        let change_dir = tmp.path().join("plans").join("demo-change");
        std::fs::create_dir_all(&change_dir).expect("creating the change dir");
        std::fs::write(
            change_dir.join("tasks.md"),
            "# demo-change — tasks\n\n- [ ] 1.1 Bind a dispatched run to its plan task\n- [x] 1.2 Already flipped\n",
        )
        .expect("writing tasks.md");
        tmp
    }

    /// A repo whose canon.yaml configures no `plans:` section at all —
    /// the DISTINCT failure `--task` must report separately from an
    /// unknown id.
    fn repo_without_plan_sources() -> TempDir {
        let tmp = tempfile::tempdir().expect("a temp dir");
        std::fs::write(tmp.path().join("canon.yaml"), "project: demo\n").expect("writing canon.yaml");
        tmp
    }

    /// Task 1.3's byte-identity half. The comparison is against a `Run`
    /// built the PRE-s40 way — `Run::new` with a literal `None` task_id
    /// and no `with_parent_run_id` call — so this fails the moment
    /// either key starts serializing as `null` instead of vanishing.
    /// Load-bearing beyond cosmetics: canon's write-time idempotence
    /// keys on a content digest over exactly these bytes.
    #[test]
    fn a_dispatch_with_neither_flag_writes_todays_exact_manifest() {
        let tmp = repo_without_plan_sources();
        let begun =
            begin(tmp.path(), &role(), &regime(), "canon", &DispatchBinding::default()).expect("a dispatch with no binding always succeeds");

        let manifest = std::fs::read_to_string(&begun.manifest_path).expect("the manifest was written");
        assert!(!manifest.contains("task_id"), "a no-flag manifest must carry NO task_id key at all:\n{manifest}");
        assert!(!manifest.contains("parent_run_id"), "a no-flag manifest must carry NO parent_run_id key at all:\n{manifest}");

        let pre_s40 = Run::new(
            begun.run.envelope.clone(),
            begun.run.run_id,
            None,
            None,
            begun.run.status,
            begun.run.started_at,
            begun.run.ended_at,
        )
        .with_injected_guidance(begun.run.injected_guidance.clone());
        let expected = serde_json::to_string_pretty(&pre_s40).expect("a Run is always serializable");
        assert_eq!(manifest, expected, "s40 must not perturb the no-binding manifest by a single byte");
    }

    /// Task 1.3's round-trip half. The parent is a freshly minted
    /// `RunId` with no manifest of its own — deliberately, since
    /// `--parent-run` is documented NOT to verify the parent exists
    /// (a parent may not have flushed yet).
    #[test]
    fn a_dispatch_with_both_flags_round_trips_both_fields() {
        let tmp = repo_with_plan_corpus();
        let task_id = TaskId::parse("demo-change#1.1").expect("a literal task id");
        let parent = RunId::new();

        let binding = DispatchBinding { task_id: Some(task_id.clone()), parent_run_id: Some(parent) };
        let begun = begin(tmp.path(), &role(), &regime(), "canon", &binding).expect("a task that exists in the corpus dispatches");

        let manifest = std::fs::read_to_string(&begun.manifest_path).expect("the manifest was written");
        let round_tripped: Run = serde_json::from_str(&manifest).expect("the manifest deserializes as a Run");
        assert_eq!(round_tripped.task_id, Some(task_id));
        assert_eq!(round_tripped.parent_run_id, Some(parent));
    }

    /// The row probe asks whether the row EXISTS, never whether it is
    /// still open: a run may legitimately be dispatched against an
    /// already-flipped task (a follow-up, a re-run).
    #[test]
    fn an_already_flipped_row_is_still_a_bindable_task() {
        let tmp = repo_with_plan_corpus();
        let task_id = TaskId::parse("demo-change#1.2").expect("a literal task id");
        let binding = DispatchBinding { task_id: Some(task_id.clone()), parent_run_id: None };
        let begun = begin(tmp.path(), &role(), &regime(), "canon", &binding).expect("an already-done task is still a real task");
        assert_eq!(begun.run.task_id, Some(task_id));
    }

    /// The probe must not write: `flip_task` is used purely as a
    /// row-existence oracle and its rewritten document is discarded.
    #[test]
    fn validating_a_task_never_mutates_the_plan_document() {
        let tmp = repo_with_plan_corpus();
        let tasks_md = tmp.path().join("plans").join("demo-change").join("tasks.md");
        let before = std::fs::read_to_string(&tasks_md).expect("the fixture tasks.md");

        let binding = DispatchBinding { task_id: Some(TaskId::parse("demo-change#1.1").expect("a literal task id")), parent_run_id: None };
        begin(tmp.path(), &role(), &regime(), "canon", &binding).expect("the dispatch succeeds");

        assert_eq!(std::fs::read_to_string(&tasks_md).expect("tasks.md still readable"), before, "the row probe must leave the document byte-identical");
    }

    /// Task 1.2: loud, naming the id, and — the point of validating
    /// BEFORE the mint — nothing at all on disk.
    #[test]
    fn an_unknown_row_fails_loud_naming_the_id_and_persists_no_manifest() {
        let tmp = repo_with_plan_corpus();
        let binding = DispatchBinding { task_id: Some(TaskId::parse("demo-change#9.9").expect("a literal task id")), parent_run_id: None };
        let err = begin(tmp.path(), &role(), &regime(), "canon", &binding).expect_err("a row that is not in the document must be rejected");

        assert!(matches!(err, DispatchError::TaskNotFound { .. }), "expected TaskNotFound, got {err:?}");
        assert!(err.to_string().contains("demo-change#9.9"), "the message must name the rejected id: {err}");
        assert!(err.is_usage(), "an unknown --task is a fixable invocation, exit 2");
        assert!(!tmp.path().join(DISPATCH_DIR).exists(), "a rejected binding must leave no dispatch record behind");
    }

    /// The loop-exhaustion path: no configured source locates the
    /// CHANGE at all, so the message names every source consulted.
    #[test]
    fn a_change_no_source_carries_fails_loud_naming_the_sources_consulted() {
        let tmp = repo_with_plan_corpus();
        let binding = DispatchBinding { task_id: Some(TaskId::parse("no-such-change#1.1").expect("a literal task id")), parent_run_id: None };
        let err = begin(tmp.path(), &role(), &regime(), "canon", &binding).expect_err("an unknown change must be rejected");

        assert!(matches!(err, DispatchError::TaskNotFound { .. }), "expected TaskNotFound, got {err:?}");
        let message = err.to_string();
        assert!(message.contains("no-such-change#1.1"), "the message must name the rejected id: {message}");
        assert!(message.contains("openspec @ "), "the message must name the sources consulted: {message}");
    }

    /// Task 1.2's second half: "there is nowhere for any id to be
    /// right" must read differently from "your id is wrong".
    #[test]
    fn a_repo_with_no_plan_sources_reports_distinctly_from_an_unknown_task() {
        let tmp = repo_without_plan_sources();
        let binding = DispatchBinding { task_id: Some(TaskId::parse("demo-change#1.1").expect("a literal task id")), parent_run_id: None };
        let err = begin(tmp.path(), &role(), &regime(), "canon", &binding).expect_err("--task against no corpus must be rejected");

        assert!(matches!(err, DispatchError::NoPlanSources { .. }), "expected NoPlanSources, got {err:?}");
        let message = err.to_string();
        assert!(message.contains("configures no plan sources"), "{message}");
        assert!(!message.contains("names no task"), "must not be confusable with the unknown-task message: {message}");
        assert!(err.is_usage());
        assert!(!tmp.path().join(DISPATCH_DIR).exists(), "a rejected binding must leave no dispatch record behind");
    }

    #[test]
    fn beginning_then_ending_lands_a_terminal_status_and_an_ended_at() {
        let tmp = repo_without_plan_sources();
        let begun = begin(tmp.path(), &role(), &regime(), "canon", &DispatchBinding::default()).expect("the dispatch begins");
        assert_eq!(begun.run.status, RunStatus::Running, "begin mints a Running run");
        assert!(begun.run.ended_at.is_none(), "a begun run is not closed");

        let ended = end(tmp.path(), begun.run_id, RunStatus::Succeeded).expect("a running run closes");
        assert_eq!(ended.run.status, RunStatus::Succeeded);
        let ended_at = ended.run.ended_at.expect("end always stamps ended_at");
        assert!(ended_at >= begun.run.started_at, "a run cannot end before it started");

        let manifest = std::fs::read_to_string(&ended.manifest_path).expect("the manifest was rewritten in place");
        let round_tripped: Run = serde_json::from_str(&manifest).expect("the manifest deserializes as a Run");
        assert_eq!(round_tripped.status, RunStatus::Succeeded);
        assert_eq!(round_tripped.ended_at, Some(ended_at));
        assert_eq!(round_tripped.run_id, begun.run_id, "end rewrites the SAME manifest, never a new one");
    }

    #[test]
    fn ending_an_unknown_run_fails_loud_naming_the_id_and_the_path() {
        let tmp = repo_without_plan_sources();
        let missing = RunId::new();
        let err = end(tmp.path(), missing, RunStatus::Succeeded).expect_err("a run that was never begun cannot be ended");

        assert!(matches!(err, DispatchError::NoSuchRun { .. }), "expected NoSuchRun, got {err:?}");
        let message = err.to_string();
        assert!(message.contains(&missing.to_string()), "the message must name the id: {message}");
        assert!(message.contains(DISPATCH_DIR), "the message must name the path consulted: {message}");
        assert!(err.is_usage());
    }

    /// `ended_at` is provenance: the second close must fail AND the
    /// first close must survive it untouched.
    #[test]
    fn ending_an_already_ended_run_fails_loud_and_preserves_the_first_close() {
        let tmp = repo_without_plan_sources();
        let begun = begin(tmp.path(), &role(), &regime(), "canon", &DispatchBinding::default()).expect("the dispatch begins");
        let first = end(tmp.path(), begun.run_id, RunStatus::Succeeded).expect("the first close succeeds");

        let err = end(tmp.path(), begun.run_id, RunStatus::Failed).expect_err("a second close must be rejected");
        assert!(matches!(err, DispatchError::AlreadyEnded { .. }), "expected AlreadyEnded, got {err:?}");
        assert!(err.to_string().contains("succeeded"), "the message must name the status already recorded: {err}");
        assert!(err.is_usage());

        let manifest = std::fs::read_to_string(&first.manifest_path).expect("the manifest survives the rejected close");
        let round_tripped: Run = serde_json::from_str(&manifest).expect("the manifest deserializes as a Run");
        assert_eq!(round_tripped.status, RunStatus::Succeeded, "the rejected close must not have rewritten the status");
        assert_eq!(round_tripped.ended_at, first.run.ended_at, "the rejected close must not have rewritten ended_at");
    }

    #[test]
    fn only_the_two_terminal_outcomes_parse_as_an_end_status() {
        assert_eq!(parse_run_status("succeeded"), Ok(RunStatus::Succeeded));
        assert_eq!(parse_run_status("failed"), Ok(RunStatus::Failed));
        for rejected in ["running", "pending", "aborted", "Succeeded", ""] {
            assert!(parse_run_status(rejected).is_err(), "{rejected:?} must not close a run");
        }
    }

    #[test]
    fn the_binding_value_parsers_check_grammar_only() {
        assert!(parse_task_id("demo-change#1.1").is_ok());
        assert!(parse_task_id("no-such-change#9.9").is_ok(), "grammar, not existence — existence is checked at dispatch time");
        assert!(parse_task_id("missing-the-separator").is_err());
        assert!(parse_run_id("01ARZ3NDEKTSV4RRFFQ69G5FAV").is_ok());
        assert!(parse_run_id("not-a-ulid").is_err());
    }
}
