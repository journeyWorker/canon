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

use canon_ingest::{find_plan_adapter, PlanSourceConfig};
use canon_learn::guidance::retrieve_guidance;
use canon_learn::{LearnConfig, ParquetStrategyStore};
use canon_model::envelope::{Actor, Envelope, RecordKind};
use canon_model::ids::{ChangeId, RunId, TaskId};
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

    /// The plan corpus could not be CONSULTED at all — a malformed
    /// `canon.yaml` `plans:` section, which `crate::plans` already
    /// fails loud on. Never collapsed into "task not found": a
    /// configuration that cannot be read has not answered the question
    /// either way. An individual unreadable/broken document INSIDE a
    /// readable source is not this: the adapter names it in
    /// `PlanParseOutcome::malformed` and the scan continues
    /// ([`validate_task_binding`]).
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

    /// `canon dispatch end` on a manifest that is not in the ONE
    /// closeable lifecycle state ([`end`]'s transition predicate:
    /// `Running` with no `ended_at`). Distinct from
    /// [`DispatchError::AlreadyEnded`], which reports a close that
    /// genuinely happened and is dated: this reports a manifest whose
    /// recorded state cannot be the origin of the requested
    /// transition at all — a `pending` run that was never dispatched
    /// live, or a hand-edited manifest carrying a TERMINAL status with
    /// no `ended_at` to prove when it closed. Collapsing the two would
    /// let an internally inconsistent manifest be silently stamped
    /// with a fresh timestamp and a possibly different status, which
    /// is precisely the provenance rewrite `AlreadyEnded` exists to
    /// refuse.
    #[error(
        "--run `{run_id}` records status `{status}`, which is not a run this command can close — `dispatch end` performs exactly one transition, `running` (with no ended_at) -> a terminal status; repair or remove the manifest deliberately"
    )]
    NotRunning { run_id: String, status: &'static str },

    /// The manifest at `<run_id>.json` names a DIFFERENT `run_id`
    /// inside. The filename is the side-channel's only index, so a
    /// disagreement means the close would stamp a terminal status onto
    /// a run the operator did not name — a copied or hand-renamed
    /// manifest, never something to guess through.
    #[error(
        "the dispatch manifest at {path} records run_id `{found}` but is filed under `{expected}` — the filename is this side-channel's only index, so closing it would stamp a terminal status onto a run you did not name"
    )]
    RunIdMismatch { path: String, expected: String, found: String },

    /// Another `canon dispatch end` holds this run's exclusive lock
    /// sidecar. NOT a usage error (exit `1`): the invocation is
    /// perfectly well-formed and the correct response is to retry once
    /// the other close finishes. A sidecar left behind by a KILLED
    /// process is reported by path here, so an operator can remove it
    /// deliberately — the same posture [`DispatchError::AlreadyEnded`]
    /// takes toward a manifest the CLI will not guess about.
    #[error(
        "--run `{run_id}` is being closed by another process (lock held at {lock_path}) — retry once it finishes, or remove that sidecar if the holder was killed"
    )]
    EndInProgress { run_id: String, lock_path: String },

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
            | Self::NotRunning { .. }
            | Self::RunIdMismatch { .. }
            | Self::Unreadable { .. } => true,
            // A concurrent close and a filesystem failure are both
            // "the invocation was fine, the machine was busy or
            // broken" — exit `1`, retryable, never a flag to fix.
            Self::EndInProgress { .. } | Self::Io(_) | Self::Serialize(_) => false,
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
/// task 1.2), through the SAME `canon.yaml` `plans:` sources and the
/// SAME `canon_ingest::plan_registry` dialect lookup `canon gate task`
/// resolves a flip through (`crate::gate::run_task`). Never a second
/// source-resolution convention: both reach the operator's configured
/// corpus through the one registry, so neither can consult a plan tree
/// the other cannot see.
///
/// Three deliberate departures from `canon gate task`, all because
/// this is a READ:
///
/// 1. **No compat default.** `crate::plans::load_plan_sources_for_gate`
///    substitutes `[{ dialect: openspec, root: <repo> }]` when `plans:`
///    is absent, so pre-s35 gate consumers keep working. `--task` is
///    new in s40 and has no such consumer, and reporting "no task `x`
///    in openspec @ <repo>" against a source the operator never
///    configured is misleading. Zero configured sources is therefore
///    its own error, [`DispatchError::NoPlanSources`].
/// 2. **The PARSED task set, never the write-back.** Membership is
///    `task_id ∈ PlanParseOutcome::tasks` — the very `Task` candidates
///    `canon ingest plans` would persist for this source, so a
///    bindable id is by construction an id the corpus actually
///    carries. The write-back trait cannot answer this question:
///    [`canon_ingest::PlanWriteBack::locate_task`] is FILE existence
///    only (its own doc), and `flip_task` — the one row-level probe
///    the trait exposes — returns
///    [`canon_ingest::WriteBackError::Unsupported`] for EVERY id in
///    the shipped superpowers dialect, which would accept
///    `<real-change>#999` and persist exactly the dangling
///    `Run.task_id` this validation exists to prevent.
/// 3. **Every source is considered — under the IMPORTER's admission,
///    not "somebody parsed it".** A change id may legitimately appear
///    in more than one configured source (the plan-import driver's own
///    `duplicate_change_id` diagnostic exists because that happens),
///    so rejection happens only after the whole list is exhausted —
///    never on the first source that merely owns the change document.
///    But mere membership in SOME source's parsed set is the wrong
///    predicate: design D8 gives the FIRST configured occurrence of a
///    `change_id` ownership for the pass and drops every `Task` under a
///    later duplicate, so a row only a later duplicate carries is a row
///    `canon ingest plans` never persists. s41 (`review-hardening`):
///    that laxer rule accepted `shared-change#2.1` out of a second
///    source and wrote it into the dispatched `Run`, recreating exactly
///    the dangling binding this validation exists to prevent. The rule
///    now comes from ONE place —
///    [`crate::plans::admit_source_candidates`], the same call the
///    importer's own scan loop makes, threading one pass-wide
///    `seen_change_ids` across the sources in config order — so a
///    `--task` is bindable IFF an import pass would persist it, and the
///    two cannot silently drift again.
///
///    `canon gate task` deliberately keeps its own, laxer rule here and
///    flips the row in whichever source's document carries it: a flip
///    edits a plan DOCUMENT, which exists on disk regardless of which
///    source won the ownership contest, while a binding asserts a
///    `Task` RECORD — and only the winner's records are ever written.
///
/// The bar is therefore the IMPORTER's, not the flipper's: a
/// construct the dialect adapter rejects wholesale (an openspec change
/// dir with no `proposal.md`, a markdown file the superpowers dialect
/// reads as a docs README) yields no `Task` candidate, so no id under
/// it is bindable — which is the right answer, because no `Task`
/// record for that id exists in the ledger or ever will, and a
/// `Run.task_id` pointing at one is the dangling binding s40 exists to
/// refuse.
///
/// One deliberate asymmetry inside that shared rule: the importer's
/// watermark cursor may skip an UNCHANGED source before it claims its
/// `change_id`s, so an incremental pass can persist a row a full pass
/// disowns. This read holds no cursor state and therefore always
/// decides as a FULL re-import would — the conservative direction,
/// since a full pass is the durable authority on D8 ownership and the
/// cost of the disagreement is a loud, fixable exit `2` rather than a
/// silently dangling `Run.task_id`.
///
/// Read-only by construction: `PlanAdapter::parse` is the same pure
/// scan+parse `canon ingest plans` runs before its persist step, and
/// nothing here touches a tier or rewrites a plan document.
fn validate_task_binding(repo: &Path, task_id: &TaskId) -> Result<(), DispatchError> {
    let named = || task_id.as_str().to_string();
    let sources = crate::plans::load_plan_sources_from_config(&repo.join("canon.yaml"), repo)
        .map_err(|e| DispatchError::PlanCorpus { task_id: named(), detail: e.to_string() })?;
    if sources.is_empty() {
        return Err(DispatchError::NoPlanSources { task_id: named() });
    }

    let mut consulted: Vec<String> = Vec::new();
    // The pass-wide D8 accumulator, threaded across sources in config
    // order exactly as `crate::plans::run` threads its own: order IS
    // the ownership rule, so this may never be reset per source.
    let mut seen_change_ids: BTreeSet<ChangeId> = BTreeSet::new();
    for src in &sources {
        consulted.push(format!("{} @ {}", src.dialect(), src.root().display()));
        // `load_plan_sources_from_config` rejects an unregistered
        // dialect before returning, so this lookup cannot miss — the
        // same reasoning (and the same `expect`) `crate::plans`' own
        // scan loop already states.
        let entry = find_plan_adapter(src.dialect()).expect("dialect validated by load_plan_sources_from_config");
        // An unresolvable source (only possible for an unconfigured
        // root, which `load_plan_sources_from_config` cannot produce)
        // contributes no candidates AND claims no `change_id`, exactly
        // as it contributes nothing to an import pass — the same
        // `resolve_source(...).map(parse).unwrap_or_default()` shape
        // `crate::plans::run` uses, so the two agree even here. A
        // malformed construct INSIDE a resolvable source is
        // skipped-and-named by the adapter itself
        // (`PlanParseOutcome::malformed`), never a panic and never a
        // corpus-level failure — so an unreadable neighbouring
        // document cannot make a perfectly good task id unbindable.
        let handle = entry.adapter.resolve_source(&PlanSourceConfig { root: Some(src.root().to_path_buf()) });
        let parsed = handle.map(|h| entry.adapter.parse(&h)).unwrap_or_default();
        let admitted = crate::plans::admit_source_candidates(&mut seen_change_ids, parsed.changes, parsed.tasks);
        if admitted.tasks.iter().any(|task| task.task_id == *task_id) {
            return Ok(());
        }
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

/// The exclusive per-run sidecar [`end`] holds across its whole
/// read -> validate -> replace sequence: `<run_id>.json.lock`, beside
/// the manifest it guards.
///
/// # Why a lock at all, and why THIS one
/// Without it the close is a read/check/write TOCTOU: two concurrent
/// `canon dispatch end` processes both observe `ended_at: None`, both
/// pass the transition check, and both replace the manifest — the
/// later writer silently overwriting the first close's status and
/// timestamp. [`write_atomic`] cannot help: it makes ONE write
/// all-or-nothing, it does not make read-then-write exclusive.
///
/// `create_new` is the mechanism because it is the one exclusion
/// primitive `std::fs` already exposes — an `O_CREAT|O_EXCL` create,
/// atomic on every filesystem canon targets — so no new dependency
/// (`fs2`, `fd-lock`) joins the tree for a single call site. It is a
/// separate FILE rather than a lock on the manifest itself because
/// `write_atomic` replaces the manifest by rename: any handle-based
/// lock would be attached to the inode that rename discards.
///
/// The `.lock` suffix keeps the sidecar out of
/// [`read_dispatch_manifests`]' `*.json` scan by construction.
///
/// Released on Drop, so BOTH the success path and every rejection
/// return unlock. A sidecar surviving that only means the holder was
/// killed outright; [`DispatchError::EndInProgress`] names its path so
/// an operator removes it deliberately — never auto-reaped on a
/// guessed age, which would reintroduce exactly the race it exists to
/// close.
struct EndLock {
    path: PathBuf,
}

impl EndLock {
    /// Take the lock for `manifest_path`, or report who holds it.
    /// Callers MUST already have established the manifest's directory
    /// exists (a `dispatch end` that mints `.canon/dispatch/` for a run
    /// that was never begun would be its own defect).
    fn acquire(manifest_path: &Path, run_id: RunId) -> Result<Self, DispatchError> {
        // `<run_id>.json` -> `<run_id>.json.lock`. Total by
        // construction: a `RunId`'s Display is a ULID (26 alphanumeric
        // characters, never a dot), so the manifest's extension is
        // always exactly `json`.
        let path = manifest_path.with_extension("json.lock");
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => Ok(Self { path }),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                Err(DispatchError::EndInProgress { run_id: run_id.to_string(), lock_path: path.display().to_string() })
            }
            Err(e) => Err(DispatchError::Io(e)),
        }
    }
}

impl Drop for EndLock {
    fn drop(&mut self) {
        // Best-effort by necessity: a destructor cannot report, and
        // panicking here would replace a recoverable stale sidecar
        // with an abort. A removal that fails leaves a lock the NEXT
        // `end` names by path, which is strictly the better failure.
        let _ = std::fs::remove_file(&self.path);
    }
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
/// # Exactly ONE transition, taken under an exclusive lock
/// The predicate is `(RunStatus::Running, ended_at: None)` -> the
/// requested terminal status, and nothing else — the status is
/// checked, not merely the timestamp. Reading `ended_at` alone would
/// accept an internally inconsistent manifest whose status is already
/// `succeeded`/`failed` but whose `ended_at` is absent (a hand edit, a
/// partial restore) and stamp it with a fresh instant, inventing a
/// close time for a run that did not close then. Every other state is
/// rejected: an already-dated close as
/// [`DispatchError::AlreadyEnded`], any other origin status as
/// [`DispatchError::NotRunning`], and a manifest whose own `run_id`
/// disagrees with the filename it is filed under as
/// [`DispatchError::RunIdMismatch`].
///
/// The read, the validation, and the replacement all happen while
/// [`EndLock`] is held, so the whole sequence is exclusive per run:
/// the second of two concurrent closes fails on the lock
/// ([`DispatchError::EndInProgress`]) or, if it arrives after the
/// first released, on `AlreadyEnded` — never by silently overwriting.
///
/// # A rejected end leaves the manifest BYTE-IDENTICAL
/// Every rejection returns before the single [`write_atomic`] call,
/// which is the only write in this function; nothing is opened for
/// writing and no field is mutated in a `Run` that is ever serialized
/// on a rejection path. `ended_at` and the terminal status are
/// PROVENANCE — the observed close of a real run — so rewriting them
/// at a later instant under a possibly different status would leave
/// no trace the first close happened. An operator who genuinely closed
/// the wrong run removes the manifest deliberately; the CLI never
/// guesses that for them.
pub fn end(repo: &Path, run_id: RunId, status: RunStatus) -> Result<Ended, DispatchError> {
    let repo = resolve_repo_root(repo);
    let manifest_path = repo.join(DISPATCH_DIR).join(format!("{run_id}.json"));
    // Probe BEFORE taking the lock, purely so an unknown run never
    // causes `.canon/dispatch/` (and a sidecar inside it) to be minted
    // for a run that was never begun. Only ABSENCE is "no such run": a
    // permission or I/O failure on a manifest that does exist stays a
    // real failure (exit `1`), never reported as a run the operator
    // never began.
    match std::fs::metadata(&manifest_path) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(DispatchError::NoSuchRun { run_id: run_id.to_string(), path: manifest_path.display().to_string() });
        }
        Err(e) => return Err(DispatchError::Io(e)),
    }
    let _lock = EndLock::acquire(&manifest_path, run_id)?;

    let text = match std::fs::read_to_string(&manifest_path) {
        Ok(text) => text,
        // Still reachable under the lock: the manifest can be removed
        // between the probe and here by something that is not another
        // `dispatch end` (an operator deleting a wrongly-closed run,
        // this module's documented remedy).
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(DispatchError::NoSuchRun { run_id: run_id.to_string(), path: manifest_path.display().to_string() });
        }
        Err(e) => return Err(DispatchError::Io(e)),
    };
    let mut run: Run = serde_json::from_str(&text)
        .map_err(|e| DispatchError::Unreadable { path: manifest_path.display().to_string(), detail: e.to_string() })?;
    if run.run_id != run_id {
        return Err(DispatchError::RunIdMismatch {
            path: manifest_path.display().to_string(),
            expected: run_id.to_string(),
            found: run.run_id.to_string(),
        });
    }
    match (run.status, run.ended_at) {
        // The one closeable state.
        (RunStatus::Running, None) => {}
        // A dated close already exists — the provenance case.
        (recorded, Some(ended_at)) => {
            return Err(DispatchError::AlreadyEnded {
                run_id: run_id.to_string(),
                ended_at: ended_at.to_rfc3339(),
                status: status_slug(recorded),
            });
        }
        // Undated, but not a running run: `pending`, or a terminal
        // status with no timestamp to prove when it closed.
        (recorded, None) => {
            return Err(DispatchError::NotRunning { run_id: run_id.to_string(), status: status_slug(recorded) });
        }
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
/// on a usage failure (unknown run id, an already-ended run, a
/// manifest that is not in the one closeable state or is filed under
/// the wrong run id, an unreadable manifest), `1` on a write/serialize
/// failure or a close another process is already holding — the same
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
/// # Every degrade is NAMED, and only `Run` bytes get in
/// Paths are sorted so the scan order is data-derived rather than
/// directory order, and so are the notes this produces.
///
/// An ABSENT directory is the ordinary never-dispatched-here case and
/// produces no note. Any OTHER `read_dir` failure (a permission
/// change, a mount that went away) does produce one: silently
/// returning empty there is indistinguishable from "nothing was ever
/// dispatched", so every live observed edge would vanish from the
/// report with no diagnostic at all.
///
/// Each file is deserialized as a TYPED [`Run`], never as a bare
/// `serde_json::Value`. [`observed_edges`] reads four strings off a
/// record; against untyped JSON, any hand-written or stray file in
/// this directory that happens to carry `run_id`/`task_id`/
/// `parent_run_id` would invent an edge indistinguishable from a real
/// dispatch. The typed parse makes the whole `Run` shape the
/// admission bar. The manifest's own `run_id` must ALSO agree with the
/// filename it is filed under — the filename is this side-channel's
/// only index ([`end`] resolves a close through it), so a copied or
/// renamed manifest is ambiguous evidence, not extra evidence.
///
/// Anything skipped — an unreadable entry, a non-`Run` body, a
/// misfiled `run_id` — is skipped WITH a note. Notes reach stderr via
/// [`run_diff`]; the report itself still succeeds, because this is a
/// read-only surface that degrades rather than gates.
fn read_dispatch_manifests(repo: &Path, notes: &mut Vec<String>) -> Vec<Value> {
    let dir = repo.join(DISPATCH_DIR);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            notes.push(format!(
                "dispatch side-channel {} could not be listed ({e}) — no live dispatch run contributes an observed edge",
                dir.display()
            ));
            return Vec::new();
        }
    };

    let mut paths: Vec<PathBuf> = Vec::new();
    // Collected and sorted rather than pushed straight through: a
    // dirent error carries no path, so directory-iteration order is
    // the only order available and it is not data-derived.
    let mut entry_errors: Vec<String> = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "json") {
                    paths.push(path);
                }
            }
            Err(e) => entry_errors.push(format!("a dispatch side-channel entry under {} could not be read ({e}) — skipped", dir.display())),
        }
    }
    paths.sort();
    entry_errors.sort();
    notes.append(&mut entry_errors);

    let mut runs = Vec::with_capacity(paths.len());
    for path in paths {
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) => {
                notes.push(format!("unreadable dispatch manifest {} ({e}) — skipped", path.display()));
                continue;
            }
        };
        let run: Run = match serde_json::from_str(&text) {
            Ok(run) => run,
            Err(e) => {
                notes.push(format!("dispatch manifest {} is not a readable Run record ({e}) — skipped", path.display()));
                continue;
            }
        };
        let filed_under = path.file_stem().and_then(|stem| stem.to_str()).and_then(|stem| RunId::parse(stem).ok());
        if filed_under != Some(run.run_id) {
            notes.push(format!(
                "dispatch manifest {} records run_id `{}` but is not filed under that id — skipped rather than counted as an observed run",
                path.display(),
                run.run_id
            ));
            continue;
        }
        match serde_json::to_value(&run) {
            Ok(value) => runs.push(value),
            // Structurally unreachable (`Run`'s fields are all
            // string-keyed), but a `.expect()` on artifact-derived
            // content is exactly what this read-only surface must not
            // do — degrade with a note instead.
            Err(e) => notes.push(format!("dispatch manifest {} will not re-serialize ({e}) — skipped", path.display())),
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

    /// A FULL `Run` manifest, exactly as [`begin`] writes one — the
    /// shape [`read_dispatch_manifests`] now demands, since a bare
    /// four-string object is precisely the invented-edge input its
    /// typed parse exists to refuse.
    fn run_manifest(run_id: RunId, task_id: Option<&str>, parent_run_id: Option<RunId>) -> Run {
        let actor = Actor::new("canon".to_string(), RoleId::parse("implementer").expect("a literal role slug"));
        let now = chrono::Utc::now();
        let run = Run::new(
            Envelope::current(RecordKind::Run, now, actor),
            run_id,
            None,
            task_id.map(|id| TaskId::parse(id).expect("test task id")),
            RunStatus::Running,
            now,
            None,
        );
        match parent_run_id {
            Some(parent) => run.with_parent_run_id(parent),
            None => run,
        }
    }

    /// Write `run` to the side-channel under `filename`, creating the
    /// directory on first use.
    fn write_manifest(repo: &Path, filename: &str, body: &str) {
        let manifests = repo.join(DISPATCH_DIR);
        std::fs::create_dir_all(&manifests).expect("side-channel dir");
        std::fs::write(manifests.join(filename), body).expect("write manifest");
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
        let (parent, child) = (RunId::new(), RunId::new());
        for run in [run_manifest(parent, Some("c#6.1"), None), run_manifest(child, Some("c#6.2"), Some(parent))] {
            write_manifest(dir.path(), &format!("{}.json", run.run_id), &serde_json::to_string(&run).expect("manifest"));
        }
        // A file that is not JSON at all is skipped WITH a note, never
        // aborting the read.
        write_manifest(dir.path(), "torn.json", "{not json");

        let mut notes = Vec::new();
        let runs = read_dispatch_manifests(dir.path(), &mut notes);
        assert_eq!(runs.len(), 2);
        assert_eq!(notes.len(), 1, "the torn manifest is the only note: {notes:?}");
        assert_eq!(observed_edges(&runs), BTreeSet::from([edge("c#6.1", "c#6.2")]));
    }

    /// [`observed_edges`] reads four strings off a record, so untyped
    /// admission let ANY syntactically valid JSON object carrying them
    /// mint an execution edge — a hand-written note, a stray export, a
    /// half-written tool scratch file — indistinguishable in the
    /// report from a real dispatch and with no diagnostic at all. The
    /// typed `Run` parse is the admission bar.
    #[test]
    fn non_run_json_in_the_side_channel_cannot_invent_an_observed_edge() {
        let dir = TempDir::new().expect("tempdir");
        let (parent, child) = (RunId::new().to_string(), RunId::new().to_string());
        // Exactly the four strings the edge builder reads, in valid
        // JSON — and nothing else a `Run` requires.
        for (run_id, task_id, parent_run_id) in [(&parent, "c#7.1", None), (&child, "c#7.2", Some(parent.as_str()))] {
            let record = run_record(run_id, Some(task_id), parent_run_id);
            write_manifest(dir.path(), &format!("{run_id}.json"), &serde_json::to_string(&record).expect("record"));
        }

        let mut notes = Vec::new();
        let runs = read_dispatch_manifests(dir.path(), &mut notes);
        assert!(runs.is_empty(), "a bare four-string object is not a Run: {runs:?}");
        assert!(observed_edges(&runs).is_empty(), "non-Run JSON must not mint an edge");
        assert_eq!(notes.len(), 2, "each rejected file is named: {notes:?}");
        assert!(notes.iter().all(|note| note.contains("not a readable Run record")), "{notes:?}");
    }

    /// The filename is this side-channel's only index ([`end`] resolves
    /// a close through it), so a manifest copied or renamed under
    /// another id is ambiguous evidence — counted, it would attribute
    /// one run's task binding to a second run id and mint an edge that
    /// never ran.
    #[test]
    fn a_manifest_filed_under_the_wrong_run_id_is_skipped_with_a_note() {
        let dir = TempDir::new().expect("tempdir");
        let (parent, child) = (RunId::new(), RunId::new());
        let parent_run = run_manifest(parent, Some("c#8.1"), None);
        write_manifest(dir.path(), &format!("{parent}.json"), &serde_json::to_string(&parent_run).expect("manifest"));
        // A real `Run`, but copied under a run id that is not its own.
        let impostor = run_manifest(child, Some("c#8.2"), Some(parent));
        write_manifest(dir.path(), &format!("{}.json", RunId::new()), &serde_json::to_string(&impostor).expect("manifest"));

        let mut notes = Vec::new();
        let runs = read_dispatch_manifests(dir.path(), &mut notes);
        assert_eq!(runs.len(), 1, "only the correctly-filed manifest is admitted: {runs:?}");
        assert!(observed_edges(&runs).is_empty(), "the misfiled child must contribute no edge");
        assert_eq!(notes.len(), 1, "the misfiled manifest is named: {notes:?}");
        assert!(notes[0].contains("is not filed under that id"), "{}", notes[0]);
    }

    /// A directory that cannot be listed is NOT the same fact as a
    /// directory that was never created: the first makes every live
    /// observed edge vanish, and reporting it as a clean zero-edge
    /// diff is a silent, unattributable data loss.
    ///
    /// The unlistable case here is a `.canon/dispatch` that exists as
    /// a regular FILE — the one `read_dir` failure reproducible on
    /// every platform and under every uid (a dropped read bit is a
    /// no-op for root, so it would silently stop testing anything in a
    /// container).
    #[test]
    fn an_unlistable_side_channel_degrades_with_a_note_not_in_silence() {
        // An ABSENT directory stays the silent normal case.
        let absent = TempDir::new().expect("tempdir");
        let mut absent_notes = Vec::new();
        assert!(read_dispatch_manifests(absent.path(), &mut absent_notes).is_empty());
        assert!(absent_notes.is_empty(), "never-dispatched-here is not a degrade: {absent_notes:?}");

        let dir = TempDir::new().expect("tempdir");
        std::fs::create_dir_all(dir.path().join(".canon")).expect("the .canon dir");
        std::fs::write(dir.path().join(DISPATCH_DIR), b"not a directory").expect("occupy the side-channel path");

        let mut notes = Vec::new();
        let runs = read_dispatch_manifests(dir.path(), &mut notes);
        assert!(runs.is_empty());
        assert_eq!(notes.len(), 1, "an unlistable side-channel is reported, never silent: {notes:?}");
        assert!(notes[0].contains("could not be listed"), "{}", notes[0]);
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

    /// One openspec change dir under `root`: a `proposal.md` (the
    /// openspec dialect's own admission bar — a change dir without one
    /// is not a change the importer recognizes at all) plus the given
    /// `tasks.md` rows.
    fn write_openspec_change(root: &Path, change_id: &str, rows: &str) {
        let change_dir = root.join(change_id);
        std::fs::create_dir_all(&change_dir).expect("creating the change dir");
        std::fs::write(change_dir.join("proposal.md"), format!("# {change_id}\n\n## Why\n\nTo exercise --task resolution.\n"))
            .expect("writing proposal.md");
        std::fs::write(change_dir.join("tasks.md"), rows).expect("writing tasks.md");
    }

    /// A repo configuring ONE openspec plan source that carries one
    /// change with an open row (`1.1`) and an already-done row (`1.2`)
    /// — the smallest tree [`validate_task_binding`] can actually
    /// resolve a task through.
    fn repo_with_plan_corpus() -> TempDir {
        let tmp = tempfile::tempdir().expect("a temp dir");
        std::fs::write(tmp.path().join("canon.yaml"), "plans:\n  sources:\n    - dialect: openspec\n      root: plans\n")
            .expect("writing canon.yaml");
        write_openspec_change(
            &tmp.path().join("plans"),
            "demo-change",
            "# demo-change — tasks\n\n- [ ] 1.1 Bind a dispatched run to its plan task\n- [x] 1.2 Already flipped\n",
        );
        tmp
    }

    /// A repo configuring ONE `superpowers` plan source holding a
    /// single `writing-plans`-shaped doc with exactly one
    /// `### Task 1:` section — the dialect whose `flip_task` is
    /// `Unsupported`, so it is the one where a write-back-based probe
    /// cannot tell a real row from an invented one.
    fn repo_with_superpowers_corpus() -> TempDir {
        let tmp = tempfile::tempdir().expect("a temp dir");
        std::fs::write(tmp.path().join("canon.yaml"), "plans:\n  sources:\n    - dialect: superpowers\n      root: plans\n")
            .expect("writing canon.yaml");
        std::fs::create_dir_all(tmp.path().join("plans")).expect("creating the plans dir");
        std::fs::write(
            tmp.path().join("plans").join("sp-plan.md"),
            "# SP Implementation Plan\n\n**Goal:** Prove superpowers row membership.\n\n### Task 1: Adapter\n- [x] wire it up\n",
        )
        .expect("writing the plan doc");
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

    /// Membership asks whether the row EXISTS, never whether it is
    /// still open: a run may legitimately be dispatched against an
    /// already-flipped task (a follow-up, a re-run). The parsed `Task`
    /// set carries both states, so a `[x]` row is as bindable as a
    /// `[ ]` one.
    #[test]
    fn an_already_flipped_row_is_still_a_bindable_task() {
        let tmp = repo_with_plan_corpus();
        let task_id = TaskId::parse("demo-change#1.2").expect("a literal task id");
        let binding = DispatchBinding { task_id: Some(task_id.clone()), parent_run_id: None };
        let begun = begin(tmp.path(), &role(), &regime(), "canon", &binding).expect("an already-done task is still a real task");
        assert_eq!(begun.run.task_id, Some(task_id));
    }

    /// Resolution must not write: it is a pure `PlanAdapter::parse`,
    /// the same scan `canon ingest plans` performs before its persist
    /// step, and nothing here touches a plan document.
    #[test]
    fn validating_a_task_never_mutates_the_plan_document() {
        let tmp = repo_with_plan_corpus();
        let tasks_md = tmp.path().join("plans").join("demo-change").join("tasks.md");
        let before = std::fs::read_to_string(&tasks_md).expect("the fixture tasks.md");

        let binding = DispatchBinding { task_id: Some(TaskId::parse("demo-change#1.1").expect("a literal task id")), parent_run_id: None };
        begin(tmp.path(), &role(), &regime(), "canon", &binding).expect("the dispatch succeeds");

        assert_eq!(std::fs::read_to_string(&tasks_md).expect("tasks.md still readable"), before, "resolving a task must leave the document byte-identical");
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

    /// BLOCKER regression (`ReviewDispatch`): the superpowers dialect's
    /// `flip_task` returns `WriteBackError::Unsupported` for EVERY id,
    /// so a write-back-based probe accepted `<real-change>#<anything>`
    /// the moment `locate_task` found the change's document — and
    /// persisted exactly the dangling `Run.task_id` this validation
    /// exists to prevent. Membership now comes from the parsed task
    /// set, which knows `sp-plan` has a `Task 1` and no `Task 9`.
    #[test]
    fn a_nonexistent_superpowers_row_is_rejected_even_though_its_change_doc_exists() {
        let tmp = repo_with_superpowers_corpus();
        let binding = DispatchBinding { task_id: Some(TaskId::parse("sp-plan#9").expect("a literal task id")), parent_run_id: None };
        let err = begin(tmp.path(), &role(), &regime(), "canon", &binding)
            .expect_err("a section the plan doc does not have is not a bindable task");

        assert!(matches!(err, DispatchError::TaskNotFound { .. }), "expected TaskNotFound, got {err:?}");
        let message = err.to_string();
        assert!(message.contains("sp-plan#9"), "the message must name the rejected id: {message}");
        assert!(message.contains("superpowers @ "), "the message must name the sources consulted: {message}");
        assert!(!tmp.path().join(DISPATCH_DIR).exists(), "a rejected binding must leave no dispatch record behind");
    }

    /// The other half of the same blocker: tightening the check must
    /// not cost the dialect its genuinely valid ids. A `### Task 1:`
    /// section IS a `Task` candidate, so `sp-plan#1` binds — in the
    /// dialect that cannot flip at all.
    #[test]
    fn a_real_superpowers_row_is_bindable_in_the_dialect_that_cannot_flip() {
        let tmp = repo_with_superpowers_corpus();
        let task_id = TaskId::parse("sp-plan#1").expect("a literal task id");
        let binding = DispatchBinding { task_id: Some(task_id.clone()), parent_run_id: None };
        let begun = begin(tmp.path(), &role(), &regime(), "canon", &binding).expect("a section the plan doc DOES have is bindable");
        assert_eq!(begun.run.task_id, Some(task_id));
    }

    /// Two configured openspec sources that BOTH carry a
    /// `shared-change` document, each with a row the other lacks — the
    /// exact shape design D8's `duplicate_change_id` diagnostic exists
    /// for, and the shape s41 (`review-hardening`) caught `--task`
    /// admitting wrongly.
    fn repo_with_a_duplicated_change_id() -> TempDir {
        let tmp = tempfile::tempdir().expect("a temp dir");
        std::fs::write(
            tmp.path().join("canon.yaml"),
            "plans:\n  sources:\n    - dialect: openspec\n      root: first\n    - dialect: openspec\n      root: second\n",
        )
        .expect("writing canon.yaml");
        write_openspec_change(&tmp.path().join("first"), "shared-change", "- [ ] 1.1 Only in the first source\n");
        write_openspec_change(&tmp.path().join("second"), "shared-change", "- [ ] 2.1 Only in the second source\n");
        tmp
    }

    /// Every `TaskId` the openspec dialect ACTUALLY parses out of one
    /// source root, with no cross-source admission applied — the raw
    /// `PlanAdapter::parse` set, so a test can tell "the row is not
    /// there" apart from "the row is there and D8 disowns it".
    fn parsed_task_ids(root: &Path) -> Vec<TaskId> {
        let entry = find_plan_adapter("openspec").expect("the openspec dialect is registered");
        let handle =
            entry.adapter.resolve_source(&PlanSourceConfig { root: Some(root.to_path_buf()) }).expect("a configured root resolves");
        entry.adapter.parse(&handle).tasks.into_iter().map(|task| task.task_id).collect()
    }

    /// BLOCKER regression (`ReReviewDispatch`, s41 `review-hardening`)
    /// — and the INVERSION of what this case asserted in s40, when it
    /// was named for a later source's row STILL BINDING. Searching every
    /// configured source is the right shape for the NOT-FOUND message
    /// and the wrong rule for admission: design D8 gives the FIRST
    /// configured occurrence of `shared-change` ownership of the whole
    /// pass, so a real `canon ingest plans` persists `shared-change#1.1`
    /// and drops `shared-change#2.1` along with its disowned `Change`.
    /// Binding the dropped row wrote precisely the dangling
    /// `Run.task_id` this validation exists to refuse — a task edge
    /// pointing at a `Task` record that exists nowhere and never will.
    #[test]
    fn a_row_only_a_later_duplicate_of_the_change_id_carries_is_refused_as_the_importer_skips_it() {
        let tmp = repo_with_a_duplicated_change_id();
        // The row is unambiguously THERE in the second source: what
        // follows is an admission refusal, not a parse miss.
        let refused = TaskId::parse("shared-change#2.1").expect("a literal task id");
        assert!(
            parsed_task_ids(&tmp.path().join("second")).contains(&refused),
            "the fixture's second source must really carry the row this test expects to be DISOWNED, not missing"
        );

        let binding = DispatchBinding { task_id: Some(refused), parent_run_id: None };
        let err =
            begin(tmp.path(), &role(), &regime(), "canon", &binding).expect_err("a row no import pass would persist is not bindable");
        assert!(matches!(err, DispatchError::TaskNotFound { .. }), "expected TaskNotFound, got {err:?}");
        let message = err.to_string();
        assert!(message.contains("shared-change#2.1"), "the message must name the rejected id: {message}");
        assert!(err.is_usage(), "an unbindable --task is a fixable invocation, exit 2");
        assert!(!tmp.path().join(DISPATCH_DIR).exists(), "a rejected binding must leave no dispatch record behind");
    }

    /// The other half of that inversion: mirroring the importer must
    /// not cost the FIRST-configured source its rows. `shared-change#1.1`
    /// is exactly what a real pass persists out of this corpus, so it
    /// stays bindable — the predicate is "would `canon ingest plans`
    /// persist this task", never the blunter "is its change id
    /// duplicated".
    #[test]
    fn the_first_sources_row_still_binds_when_a_later_source_duplicates_its_change_id() {
        let tmp = repo_with_a_duplicated_change_id();
        let task_id = TaskId::parse("shared-change#1.1").expect("a literal task id");
        let binding = DispatchBinding { task_id: Some(task_id.clone()), parent_run_id: None };
        let begun =
            begin(tmp.path(), &role(), &regime(), "canon", &binding).expect("the row an import pass DOES persist is bindable");
        assert_eq!(begun.run.task_id, Some(task_id));
    }

    /// Rejection still happens only once the WHOLE source list has been
    /// considered: a row neither source carries names both, even though
    /// the second source lost its `shared-change` ownership contest and
    /// contributed no candidate at all.
    #[test]
    fn a_row_no_source_carries_names_every_source_consulted_across_a_duplicate() {
        let tmp = repo_with_a_duplicated_change_id();
        let missing = DispatchBinding { task_id: Some(TaskId::parse("shared-change#9.9").expect("a literal task id")), parent_run_id: None };
        let err = begin(tmp.path(), &role(), &regime(), "canon", &missing).expect_err("a row no source carries must be rejected");
        let message = err.to_string();
        for root in ["first", "second"] {
            let consulted = format!("openspec @ {}", tmp.path().join(root).display());
            assert!(message.contains(&consulted), "every source must be named before rejecting; missing `{consulted}` in: {message}");
        }
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

    /// The exclusive sidecar [`end`] holds, by the same construction
    /// [`EndLock::acquire`] uses.
    fn lock_path_for(repo: &Path, run_id: RunId) -> PathBuf {
        repo.join(DISPATCH_DIR).join(format!("{run_id}.json.lock"))
    }

    /// BLOCKER regression (`ReviewDispatch`): the close was a
    /// read/check/write TOCTOU, so two concurrent `dispatch end` runs
    /// could both observe `ended_at: None` and both write, the later
    /// silently replacing the first terminal status and timestamp.
    /// A close in flight is now visible to the second process, which
    /// refuses rather than overwrites — and refuses without touching a
    /// byte of the manifest.
    #[test]
    fn a_close_already_in_flight_blocks_a_second_end_instead_of_overwriting_it() {
        let tmp = repo_without_plan_sources();
        let begun = begin(tmp.path(), &role(), &regime(), "canon", &DispatchBinding::default()).expect("the dispatch begins");
        let before = std::fs::read(&begun.manifest_path).expect("the begun manifest");

        // Stand in for the other process: it holds the sidecar across
        // its own read -> validate -> replace.
        let lock = lock_path_for(tmp.path(), begun.run_id);
        std::fs::write(&lock, b"").expect("simulate a held lock");

        let err = end(tmp.path(), begun.run_id, RunStatus::Failed).expect_err("a close already in flight must not be raced");
        assert!(matches!(err, DispatchError::EndInProgress { .. }), "expected EndInProgress, got {err:?}");
        assert!(err.to_string().contains(&lock.display().to_string()), "the message must name the sidecar to remove: {err}");
        assert!(!err.is_usage(), "a concurrent close is retryable machinery, exit 1 — never a flag to fix");
        assert_eq!(std::fs::read(&begun.manifest_path).expect("the manifest survives"), before, "a blocked close must not touch a byte");

        // Once the holder is gone the close proceeds normally, and
        // leaves no sidecar of its own behind.
        std::fs::remove_file(&lock).expect("release the simulated lock");
        let ended = end(tmp.path(), begun.run_id, RunStatus::Failed).expect("the close succeeds once the lock is free");
        assert_eq!(ended.run.status, RunStatus::Failed);
        assert!(!lock.exists(), "the lock is released on the success path");
    }

    /// A rejected close must release the lock too — otherwise one bad
    /// `dispatch end` would wedge that run's manifest permanently.
    #[test]
    fn a_rejected_close_still_releases_the_lock() {
        let tmp = repo_without_plan_sources();
        let begun = begin(tmp.path(), &role(), &regime(), "canon", &DispatchBinding::default()).expect("the dispatch begins");
        end(tmp.path(), begun.run_id, RunStatus::Succeeded).expect("the first close succeeds");

        end(tmp.path(), begun.run_id, RunStatus::Failed).expect_err("the second close is rejected");
        assert!(!lock_path_for(tmp.path(), begun.run_id).exists(), "a rejected close must not leave its sidecar behind");
    }

    /// BLOCKER regression (`ReviewDispatch`): checking `ended_at`
    /// ALONE accepted an internally inconsistent manifest whose status
    /// was already terminal but whose timestamp was absent, stamping it
    /// with a fresh instant and inventing a close time for a run that
    /// did not close then. The predicate is the STATUS: only
    /// `running` (undated) is closeable.
    #[test]
    fn a_terminal_status_with_no_ended_at_is_not_a_closeable_run() {
        let tmp = repo_without_plan_sources();
        let begun = begin(tmp.path(), &role(), &regime(), "canon", &DispatchBinding::default()).expect("the dispatch begins");
        // A hand edit / partial restore: terminal status, no timestamp.
        let mut inconsistent = begun.run.clone();
        inconsistent.status = RunStatus::Succeeded;
        inconsistent.ended_at = None;
        let body = serde_json::to_string_pretty(&inconsistent).expect("a Run is always serializable");
        std::fs::write(&begun.manifest_path, &body).expect("write the inconsistent manifest");

        let err = end(tmp.path(), begun.run_id, RunStatus::Failed).expect_err("a terminal manifest is not a running run");
        assert!(matches!(err, DispatchError::NotRunning { .. }), "expected NotRunning, got {err:?}");
        assert!(err.to_string().contains("succeeded"), "the message must name the status recorded: {err}");
        assert!(err.is_usage(), "a manifest to repair is a fixable invocation, exit 2");
        assert_eq!(std::fs::read_to_string(&begun.manifest_path).expect("the manifest survives"), body, "a rejected close must not touch a byte");
    }

    /// A `pending` run was never dispatched live by this CLI, so it has
    /// no close to record either — the same one-transition predicate,
    /// exercised from the other side of `running`.
    #[test]
    fn a_pending_run_is_not_a_closeable_run() {
        let tmp = repo_without_plan_sources();
        let begun = begin(tmp.path(), &role(), &regime(), "canon", &DispatchBinding::default()).expect("the dispatch begins");
        let mut pending = begun.run.clone();
        pending.status = RunStatus::Pending;
        std::fs::write(&begun.manifest_path, serde_json::to_string_pretty(&pending).expect("serializable")).expect("write");

        let err = end(tmp.path(), begun.run_id, RunStatus::Succeeded).expect_err("a pending run has no live dispatch to close");
        assert!(matches!(err, DispatchError::NotRunning { .. }), "expected NotRunning, got {err:?}");
        assert!(err.to_string().contains("pending"), "{err}");
    }

    /// The filename is this side-channel's only index, so a manifest
    /// whose own `run_id` disagrees with it would have a terminal
    /// status stamped onto a run the operator never named.
    #[test]
    fn a_manifest_filed_under_another_run_id_is_not_closeable() {
        let tmp = repo_without_plan_sources();
        let begun = begin(tmp.path(), &role(), &regime(), "canon", &DispatchBinding::default()).expect("the dispatch begins");
        let mut impostor = begun.run.clone();
        impostor.run_id = RunId::new();
        let body = serde_json::to_string_pretty(&impostor).expect("a Run is always serializable");
        std::fs::write(&begun.manifest_path, &body).expect("write the misfiled manifest");

        let err = end(tmp.path(), begun.run_id, RunStatus::Succeeded).expect_err("a misfiled manifest must not be closed");
        assert!(matches!(err, DispatchError::RunIdMismatch { .. }), "expected RunIdMismatch, got {err:?}");
        let message = err.to_string();
        assert!(message.contains(&begun.run_id.to_string()), "the message must name the id asked for: {message}");
        assert!(message.contains(&impostor.run_id.to_string()), "the message must name the id found: {message}");
        assert!(err.is_usage());
        assert_eq!(std::fs::read_to_string(&begun.manifest_path).expect("the manifest survives"), body, "a rejected close must not touch a byte");
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
