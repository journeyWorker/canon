//! `canon ingest plans [--dialect <id> --source <path>] [--repo <dir>]
//! [--json]` (s17 P3 `s17-plan-import`): the plan-import connector
//! meets canon-store — the THIRD instance of `crate::ingest`/
//! `crate::artifact_ingest`'s adapter -> normalize -> persist shape,
//! generalized from `SessionAdapter`/`ArtifactAdapter` to
//! `canon_ingest::plan_registry`'s `PlanAdapter`s (`openspec`, s17's
//! reference dialect). `canon-ingest` itself has no `canon-store`
//! dependency (pure scan/parse/normalize domain logic, P1/P2); this
//! module is the one place a `PlanAdapter`'s `Change`/`Task`
//! candidates meet canon-store's validated tiered write
//! (`canon_store::registry::TierRegistry::persist`, the SAME entry
//! point `canon query`/session/artifact ingest already share).
//!
//! # Config: `canon.yaml` `plans:` (design D2, task 3.2)
//! `plans: { sources: [{dialect, root}] }`, roots resolved relative to
//! the `canon.yaml` directory. An ABSENT `plans:` section resolves to
//! ZERO sources — a clean, explicit no-op, never a hardcoded default
//! root ([`load_plan_sources_from_config`]). A PRESENT section parses
//! STRICTLY (`deny_unknown_fields`): a typo'd key, an unregistered
//! dialect id, or (checked separately, before any scan) a nonexistent
//! source root all fail the command loud, naming the offender.
//!
//! # One-shot override (design D2, task 3.3)
//! `--dialect <id> --source <path>` bypasses `canon.yaml` entirely for
//! a single ad-hoc import; either flag without the other fails loud
//! ([`resolve_sources`]).
//!
//! # Watermark cursor (S3 §3 generalized, task 3.4)
//! One [`SourceCursor`] per configured `(dialect, root)` source, under
//! the SAME `<repo>/.canon/ingest/cursors/` root session-ingest already
//! uses (a distinct filename per source, [`plan_source_cursor_id`], so
//! the two families never collide). The gate digests exactly the files
//! the source's dialect reads ([`canon_ingest::PlanAdapter::source_files`];
//! for `openspec`, the change dirs under `openspec/changes/`), never
//! a walk of the whole root: `canon init` writes `root: .`, and a
//! source rooted at a JS repository must not read `node_modules` on
//! every pass (0.14 acceptance rerun G1, where it cost ~10 s per
//! `canon gate task`). The importer's OWN repo-local write surface (the
//! git ledger root under `tiers.git.root`, and the `<repo>/.canon/
//! ingest` cursor tree) is still excluded by a canonicalized
//! `starts_with` check computed once per run, so a dialect whose input
//! set could reach them never self-churns. The
//! predicate is content-digest ONLY (never mtime), so a `git checkout`
//! / `touch` that doesn't change bytes never reaches parse. A source
//! is (re-)parsed as a COMPLETE set whenever anything in it changed —
//! never partially, so a multi-file change dir is never re-derived
//! from a partial read. `canon gate task` re-imports only the change it
//! flipped ([`refresh_change`]) and leaves the cursor alone.
//!
//! # Adoption survives a re-import (0.14 acceptance rerun G9)
//! A plan document never names its subject; `canon change new` and
//! `canon subject adopt` stamp `subject_id` on a newer change version.
//! Every imported `Change` with no `subject_id` of its own carries the
//! one its latest stored version has ([`Adoptions`]), so a re-import
//! never unlinks an adopted change.
//!
//! # Cross-source `change_id` collision (design D8, task 3.7)
//! Sources are processed in strict config/one-shot order. A
//! `change_id` a LATER source also produces, after an EARLIER source
//! already produced it THIS PASS, is skipped — its `Change` AND every
//! `Task` under that `change_id` — and counted under the later
//! source's `duplicate_change_id` diagnostic. The first-configured
//! occurrence always wins in full; across separate passes, fold-latest
//! governs as usual.
//!
//! # Persistence + the `unwritten` seam (spec, task 3.5)
//! Every accepted candidate persists ONLY through
//! [`TierRegistry::persist`] — this driver never writes a record file
//! directly, bypasses validation, or introduces a second write path
//! (connector-never-authority). A `StoreError::DuplicatePath` (an
//! already-identical git-tier resubmission) is treated as a successful
//! no-op, mirroring `crate::ingest`'s own `persist_idempotent`
//! discipline. Unlike `crate::ingest`'s whole-batch unwritten
//! fallback (which triggers on ANY `canon.yaml`/tier-build failure),
//! this driver degrades PER RECORD: a `StoreError::TierUnavailable`
//! (e.g. `task: pg` routed but `CANON_PG_DSN` unset) or
//! `StoreError::UnroutedKind` (a kind with no `routing` entry at all)
//! is non-fatal — that ONE candidate degrades into
//! [`PlansOutcome::unwritten_changes`]/[`PlansOutcome::unwritten_tasks`]
//! while every sibling candidate (including a DIFFERENT kind routed to
//! a reachable tier) still persists normally.
//! [`crate::tiers::build_lenient_tiers`] is what makes this possible:
//! unlike `crate::tiers::build_tiers`'s all-or-nothing "a
//! declared-but-unreachable tier is a startup hard error" contract
//! (appropriate for `canon tier age`'s destructive move+delete, which
//! has no partial-success story), it attaches whichever tiers it CAN
//! reach and leaves the rest `None` — but ONLY for genuine
//! unreachability (an unset `dsn_env`, an absent/unreachable r2
//! bucket credential); a MALFORMED `tiers.*` configuration (e.g. a
//! `tiers.pg.schema` that fails validation) still fails this whole
//! command loud, exactly like `build_tiers` — "lenient" describes
//! per-tier reachability, never config correctness. `canon query`
//! (`crate::query`) shares this SAME degrade-or-propagate core via
//! `crate::tiers::build_lenient_tiers_for_kind`, a kind-scoped sibling
//! (s22 `query-tier-degradation`) — never a second, independently
//! drifting copy. This is what lets
//! a git-routed `Change` and a pg-routed-but-unreachable `Task` be
//! decided independently at persist time. A source with ANY unwritten
//! candidate this pass does NOT advance its cursor (the pass was not
//! fully durable, spec "An unreachable pg tier degrades to the
//! unwritten seam"); a collision-skip alone does NOT block the cursor
//! (a deliberate, successful business outcome, not a failure). A
//! source whose pass is malformed-nonzero and zero-persisted --
//! wholly-unproductive, no `Change`/`Task` reached any tier --
//! likewise withholds its cursor (s23 `durable-import-diagnostics`),
//! reusing the SAME `malformed_zero_persisted` local that also drives
//! `PlansOutcome::non_clean_sources`/s18's unconditional stderr WARN:
//! every subsequent run against an unchanged, still-broken source
//! re-scans, re-parses, and re-warns until the source becomes clean or
//! partially successful, at which point a fresh cursor is finally
//! earned and the source goes quiet again -- no new `SourceCursor`
//! field, no persisted marker (design.md rejected that shape).
//!
//! # Never an authority (design R1)
//! This module (and `canon-ingest`'s plan family) carries zero
//! reference to `canon-gate`/`canon-learn`; imported `Change`/`Task`
//! rows are ordinary records indistinguishable to any reader except by
//! their fixed per-dialect actor provenance
//! (`canon-plan-import-<dialect>`, set by the adapter, never here).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use canon_ingest::plan_adapter::{MalformedEntry, PlanParseOutcome, PlanSourceConfig};
use canon_ingest::plan_registry;
use canon_model::envelope::{CanonRecord, RecordKind};
use canon_model::ids::{ChangeId, SubjectId};
use canon_model::records::{Change, Task};
use canon_store::cursor::{file_digest, CursorStore, SourceCursor};
use canon_store::policy::{PolicyError, TierPolicy};
use canon_store::registry::TierRegistry;
use canon_store::tier::StoreError;
use serde::{Deserialize, Serialize};

use crate::context::resolve_repo_root;
use crate::tiers::TierCliError;

#[derive(Debug, thiserror::Error)]
pub enum PlansError {
    /// A malformed `plans:` config, an unregistered dialect id, a
    /// `--dialect`/`--source` given without its pair, or a configured
    /// source root that does not exist — an operator error, always
    /// caught BEFORE any scan (spec "Malformed plan sources fail soft
    /// per construct, loud per configuration").
    #[error("{0}")]
    Config(String),
    #[error(transparent)]
    Policy(#[from] PolicyError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// [`crate::tiers::build_lenient_tiers`]'s relocated error type
/// (design.md D1) converts into `PlansError` here: the ONLY variant it
/// can actually produce for this call site is `TierCliError::Store`
/// (this driver already parses `canon.yaml` itself, so
/// `ReadCanonYaml`/`Policy` never occur from a `build_lenient_tiers`
/// call) -- mapped straight onto `PlansError::Store`, so `ingest
/// plans`'s own propagated-error text is byte-identical to before the
/// relocation (spec "canon ingest plans's own observable behavior is
/// unchanged"). The other variants are handled too, via `Display`,
/// purely for exhaustiveness -- they are structurally unreachable here.
impl From<TierCliError> for PlansError {
    fn from(err: TierCliError) -> Self {
        match err {
            TierCliError::Store(store_err) => PlansError::Store(store_err),
            other => PlansError::Config(other.to_string()),
        }
    }
}

/// One resolved `(dialect, root)` source, from EITHER `canon.yaml`
/// `plans.sources[]` or the `--dialect`/`--source` one-shot override —
/// the driver treats both identically from here on. `pub(crate)`
/// accessors let `crate::gate` (s35 `gate-plan-dialect-seam`) resolve a
/// `canon gate task` flip through the SAME configured plan sources
/// `canon ingest plans` reads, rather than a second hardcoded path.
pub(crate) struct PlanSource {
    pub(crate) dialect: String,
    pub(crate) root: PathBuf,
}

impl PlanSource {
    pub(crate) fn dialect(&self) -> &str {
        &self.dialect
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
}

/// One configured source's contribution to a pass (human + `--json`
/// output, task 3.5: "per-source counts + drop diagnostics + malformed
/// tallies").
#[derive(Debug, Clone, Serialize)]
pub struct PlanSourceSummary {
    pub dialect: String,
    pub root: String,
    /// `true` iff the S3-generalized watermark gate skipped this
    /// source wholesale (byte-identical to its cursor) — every other
    /// field is `0`/empty in that case.
    pub skipped_unchanged: bool,
    pub changes_parsed: usize,
    pub tasks_parsed: usize,
    pub changes_persisted: usize,
    pub tasks_persisted: usize,
    /// Degraded to the `unwritten` seam (a routed tier unreachable/
    /// unrouted) — the bodies themselves are in
    /// [`PlansOutcome::unwritten_changes`]/`unwritten_tasks`.
    pub changes_unwritten: usize,
    pub tasks_unwritten: usize,
    /// `change_id`s this source lost to an EARLIER-configured source
    /// this pass (design D8) — that `Change` and every `Task` under it
    /// were skipped entirely, never persisted.
    pub duplicate_change_id: usize,
    /// Per-construct NAMED unmapped-drop counts straight from the
    /// adapter's [`PlanParseOutcome`] (design D3).
    pub unmapped: BTreeMap<String, usize>,
    /// Structurally-broken constructs the adapter itself skipped
    /// (design D3) — distinct from `duplicate_change_id`, which this
    /// driver (not the adapter) detects. Each one NAMED by path +
    /// reason [+ hint] (s18 `loud-plan-import-diagnostics` spec) —
    /// `.len()` is the scalar tally a caller that only needs the count
    /// reaches for.
    pub malformed: Vec<MalformedEntry>,
    /// `true` iff this source's watermark cursor was refreshed this
    /// pass — `false` for a skipped-unchanged source (nothing to
    /// refresh) AND for a (re-)parsed source with any unwritten
    /// candidate (the pass was not fully durable).
    pub cursor_advanced: bool,
}

/// One configured source this pass found `malformed > 0` AND persisted
/// ZERO records for (`changes_persisted == 0 && tasks_persisted == 0`)
/// — s18 `loud-plan-import-diagnostics` spec's "A malformed-nonzero,
/// zero-persisted source makes canon ingest plans non-clean at the
/// process level". Never flagged for a legitimately empty/fresh source
/// (`malformed == 0`) or a partial success (`persisted > 0`) — see
/// [`run`]'s own flagging logic.
#[derive(Debug, Clone, Serialize)]
pub struct NonCleanSource {
    pub dialect: String,
    pub root: String,
    pub malformed: usize,
}

/// One `canon ingest plans` pass's outcome.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PlansOutcome {
    pub sources: Vec<PlanSourceSummary>,
    pub changes_persisted: usize,
    pub tasks_persisted: usize,
    pub duplicate_change_id: usize,
    /// Every `Change`/`Task` a routed-but-unreachable (or wholly
    /// unrouted) tier could not accept this pass — printed, never
    /// silently dropped, never fatal to a sibling record whose tier
    /// write already succeeded (spec "An unreachable pg tier degrades
    /// to the unwritten seam").
    pub unwritten_changes: Vec<Change>,
    pub unwritten_tasks: Vec<Task>,
    /// Every source this pass flagged non-clean (s18 spec) — the CLI
    /// layer (`main.rs::run_ingest_plans`) prints an unconditional
    /// stderr WARN per entry and exits non-zero whenever this is
    /// non-empty. Empty for an ordinary clean/partial-success pass.
    pub non_clean_sources: Vec<NonCleanSource>,
}

/// `canon.yaml`'s top-level `plans:` section — see module doc.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPlans {
    #[serde(default)]
    sources: Vec<RawPlanSource>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPlanSource {
    dialect: String,
    root: PathBuf,
}

/// One `scan(digest) -> gate -> parse -> collision-filter -> persist`
/// pass over every configured plan source, in config/one-shot order
/// (module doc).
pub fn run(repo: &Path, dialect: Option<&str>, source: Option<&Path>) -> Result<PlansOutcome, PlansError> {
    let repo = resolve_repo_root(repo);
    let canon_yaml_path = repo.join("canon.yaml");

    let sources = resolve_sources(&repo, &canon_yaml_path, dialect, source)?;
    if sources.is_empty() {
        // Absent `plans:` (config-driven path only — the one-shot pair
        // always yields exactly one source) — a clean, explicit
        // no-op; never a hardcoded default root (task 3.2).
        return Ok(PlansOutcome::default());
    }
    validate_source_roots(&sources)?;

    let (store, git_root) = open_store(&repo, &canon_yaml_path)?;

    // F2 / design note: the digest below must never include this
    // driver's OWN repo-local write surface (the git ledger root, the
    // `.canon/ingest` cursor tree) — computed ONCE per run, as an
    // absolute/canonicalized dir per [`canonicalize_or`], so the
    // per-file exclusion check is a real-filesystem `starts_with`,
    // never a text-level "any path segment named canon" match. A
    // no-op whenever a dialect's input set (the common case) never
    // nests either dir.
    let mut excluded_dirs: Vec<PathBuf> = Vec::new();
    if let Some(root) = &git_root {
        excluded_dirs.push(canonicalize_or(root));
    }
    excluded_dirs.push(canonicalize_or(&repo.join(".canon/ingest")));

    let cursors = CursorStore::open(repo.join(".canon/ingest/cursors"));

    let mut outcome = PlansOutcome::default();
    let mut seen_change_ids: BTreeSet<ChangeId> = BTreeSet::new();
    let mut pending_cursors: Vec<SourceCursor> = Vec::new();
    let mut adoptions = Adoptions::default();

    for src in &sources {
        // Validated at `resolve_sources`/`load_plan_sources_from_config`
        // time — every `src.dialect` is a registered id by construction.
        let entry = plan_registry::find(&src.dialect).expect("dialect validated before the scan loop");
        let cursor_id = plan_source_cursor_id(&src.dialect, entry.adapter.parse_version(), &src.root);
        let handle = entry.adapter.resolve_source(&PlanSourceConfig { root: Some(src.root.clone()) });

        // The dialect's own input set (`PlanAdapter::source_files`),
        // never a walk of the whole root (module doc).
        let files: Vec<PathBuf> = handle
            .as_ref()
            .map(|h| entry.adapter.source_files(h))
            .unwrap_or_default()
            .into_iter()
            .filter(|path| !excluded_dirs.iter().any(|dir| canonicalize_or(path).starts_with(dir)))
            .collect();
        let mut present_digests: BTreeMap<String, String> = BTreeMap::new();
        let mut readable: Vec<(PathBuf, i64, u64, String)> = Vec::new();
        let mut read_errors = 0usize;
        for path in &files {
            match std::fs::read(path) {
                Ok(bytes) => {
                    let digest = file_digest(&bytes);
                    let (mtime_ms, size) = file_stat(path);
                    present_digests.insert(path.to_string_lossy().into_owned(), digest.clone());
                    readable.push((path.clone(), mtime_ms, size, digest));
                }
                Err(_) => read_errors += 1,
            }
        }

        let cursor = cursors.read(&cursor_id);
        let unchanged = read_errors == 0 && cursor.as_ref().is_some_and(|c| c.source_unchanged(&present_digests));

        if unchanged {
            outcome.sources.push(PlanSourceSummary {
                dialect: src.dialect.clone(),
                root: src.root.display().to_string(),
                skipped_unchanged: true,
                changes_parsed: 0,
                tasks_parsed: 0,
                changes_persisted: 0,
                tasks_persisted: 0,
                changes_unwritten: 0,
                tasks_unwritten: 0,
                duplicate_change_id: 0,
                unmapped: BTreeMap::new(),
                malformed: Vec::new(),
                cursor_advanced: false,
            });
            continue;
        }

        let PlanParseOutcome { changes, tasks, unmapped, malformed } = handle.map(|h| entry.adapter.parse(&h)).unwrap_or_default();
        let changes_parsed = changes.len();
        let tasks_parsed = tasks.len();

        // Design D8's first-source-wins rule, applied by the ONE
        // shared decision `crate::dispatch`'s `--task` admission also
        // calls ([`admit_source_candidates`], extracted in s41
        // (`review-hardening`)) — never a second local copy of it.
        let SourceAdmission { changes: accepted_changes, tasks: accepted_tasks, duplicate_change_ids } =
            admit_source_candidates(&mut seen_change_ids, changes, tasks);
        let duplicate_change_id = duplicate_change_ids.len();

        let Persisted { changes_persisted, changes_unwritten, tasks_persisted, tasks_unwritten } =
            persist_candidates(&store, &mut adoptions, accepted_changes, accepted_tasks, &mut outcome)?;

        // s23 durable-import-diagnostics: a malformed-nonzero,
        // zero-persisted pass produced no durable evidence at all --
        // ONE shared local, read by both the cursor-eligibility check
        // below AND the non-clean-source flag further down, so the
        // two never independently drift (design.md R2).
        let malformed_zero_persisted = !malformed.is_empty() && changes_persisted == 0 && tasks_persisted == 0;

        // Cursor advances ONLY when this source's pass was fully
        // durable (task 3.4/spec) — a collision-skip alone never
        // blocks it (a deliberate, successful outcome, not a failure).
        let fully_durable = changes_unwritten == 0 && tasks_unwritten == 0 && !malformed_zero_persisted;
        if fully_durable {
            let mut fresh = SourceCursor::empty(cursor_id);
            for (path, mtime_ms, size, digest) in &readable {
                fresh.record(path, *mtime_ms, *size, digest.clone());
            }
            fresh.refresh_summary();
            pending_cursors.push(fresh);
        }

        outcome.changes_persisted += changes_persisted;
        outcome.tasks_persisted += tasks_persisted;
        outcome.duplicate_change_id += duplicate_change_id;

        // s18 `loud-plan-import-diagnostics` spec: "A malformed-nonzero,
        // zero-persisted source makes canon ingest plans non-clean at
        // the process level" -- a source with SOME malformed dirs but
        // ANY persisted record stays clean (partial success is not the
        // targeted near-miss); a legitimately empty source
        // (`malformed` empty) is never flagged. Reads the shared
        // `malformed_zero_persisted` local above (s23 design.md R2).
        if malformed_zero_persisted {
            outcome.non_clean_sources.push(NonCleanSource { dialect: src.dialect.clone(), root: src.root.display().to_string(), malformed: malformed.len() });
        }

        outcome.sources.push(PlanSourceSummary {
            dialect: src.dialect.clone(),
            root: src.root.display().to_string(),
            skipped_unchanged: false,
            changes_parsed,
            tasks_parsed,
            changes_persisted,
            tasks_persisted,
            changes_unwritten,
            tasks_unwritten,
            duplicate_change_id,
            unmapped,
            malformed,
            cursor_advanced: fully_durable,
        });
    }

    // Durable-write succeeded for these sources: advance their
    // cursors. Best-effort, mirrors `crate::ingest::run` — a cursor
    // write failure only costs a re-parse next pass, never the
    // records this pass already persisted.
    for cursor in pending_cursors {
        let _ = cursors.write(&cursor);
    }

    Ok(outcome)
}

/// The routed store an import persists through, plus the git tier's
/// root (the importer's own write surface [`run`] excludes from its
/// digest). A missing/unreadable canon.yaml degrades to an empty policy
/// (every candidate lands in the `unwritten` seam) rather than a hard
/// failure — a genuinely malformed (present but unparseable) canon.yaml
/// still fails loud via `TierPolicy::from_yaml`.
fn open_store(repo: &Path, canon_yaml_path: &Path) -> Result<(TierRegistry, Option<PathBuf>), PlansError> {
    let canon_yaml_text = std::fs::read_to_string(canon_yaml_path).unwrap_or_default();
    let policy = if canon_yaml_text.trim().is_empty() {
        TierPolicy { tiers: HashMap::new(), routing: HashMap::new(), aging: HashMap::new() }
    } else {
        TierPolicy::from_yaml_at(&canon_yaml_text, repo)?
    };
    let (git, pg, r2, sqlite) = crate::tiers::build_lenient_tiers(&policy, repo)?;
    let git_root = git.as_ref().map(|tier| tier.root().to_path_buf());
    Ok((TierRegistry::new(policy, git, pg, r2, sqlite), git_root))
}

/// One source's persisted/unwritten tallies.
struct Persisted {
    changes_persisted: usize,
    changes_unwritten: usize,
    tasks_persisted: usize,
    tasks_unwritten: usize,
}

/// Persist admitted candidates (module doc "Persistence + the
/// `unwritten` seam"), each `Change` first carrying its adoption
/// ([`Adoptions::carry`]). Unwritten bodies land in `outcome`.
fn persist_candidates(store: &TierRegistry, adoptions: &mut Adoptions, changes: Vec<Change>, tasks: Vec<Task>, outcome: &mut PlansOutcome) -> Result<Persisted, PlansError> {
    let mut persisted = Persisted { changes_persisted: 0, changes_unwritten: 0, tasks_persisted: 0, tasks_unwritten: 0 };
    for mut change in changes {
        adoptions.carry(store, &mut change)?;
        match persist_or_unwritten(store, change)? {
            None => persisted.changes_persisted += 1,
            Some(unwritten) => {
                persisted.changes_unwritten += 1;
                outcome.unwritten_changes.push(unwritten);
            }
        }
    }
    for task in tasks {
        match persist_or_unwritten(store, task)? {
            None => persisted.tasks_persisted += 1,
            Some(unwritten) => {
                persisted.tasks_unwritten += 1;
                outcome.unwritten_tasks.push(unwritten);
            }
        }
    }
    Ok(persisted)
}

/// The subject each change was adopted into (`canon change new`,
/// `canon subject adopt`), read from the latest change records the
/// first time an import has a change to persist (0.14 acceptance rerun
/// G9). A plan document never names its subject, so without this every
/// re-import (a `canon gate task` flip, a ticked checkbox) appended a
/// newer change version with `subject_id: null` that superseded the
/// adoption in every folded read.
#[derive(Default)]
struct Adoptions(Option<BTreeMap<ChangeId, SubjectId>>);

impl Adoptions {
    /// Give `change` the subject its latest stored version carries, when
    /// the plan source named none. A change tier this run cannot reach
    /// reads as no adoptions: the change itself then degrades to the
    /// `unwritten` seam at persist time, so nothing is lost.
    fn carry(&mut self, store: &TierRegistry, change: &mut Change) -> Result<(), PlansError> {
        if change.subject_id.is_some() {
            return Ok(());
        }
        if self.0.is_none() {
            let latest = match crate::subject::latest_records(store, RecordKind::Change) {
                Ok(records) => records,
                Err(StoreError::TierUnavailable { .. }) | Err(StoreError::UnroutedKind { .. }) => Vec::new(),
                Err(err) => return Err(PlansError::Store(err)),
            };
            let adopted = latest
                .iter()
                .filter_map(|raw| {
                    let change_id = ChangeId::parse(raw.0.get("change_id")?.as_str()?).ok()?;
                    let subject_id = SubjectId::parse(raw.0.get("subject_id")?.as_str()?).ok()?;
                    Some((change_id, subject_id))
                })
                .collect();
            self.0 = Some(adopted);
        }
        change.subject_id = self.0.as_ref().and_then(|adopted| adopted.get(&change.change_id)).cloned();
        Ok(())
    }
}

/// What [`refresh_change`] found for its change across the configured
/// sources.
pub(crate) struct ChangeRefresh {
    /// `Change` candidates parsed for the id (`0`: no source holds it).
    pub(crate) changes_parsed: usize,
    /// Constructs the adapters skipped while reading it.
    pub(crate) malformed: usize,
}

/// Re-import ONE change, `change_id`, from the configured plan sources:
/// `canon gate task`'s refresh after a flip (0.14 D5). Each source in
/// config order reads only that change ([`PlanAdapter::parse_change`]),
/// the first source carrying it owns it ([`admit_source_candidates`],
/// exactly as in [`run`]), and its candidates persist exactly as [`run`]
/// persists them, adoption included. No source is digested and no
/// cursor moves: the next `canon ingest plans` re-reads the edited
/// source and finds these records already written.
///
/// [`PlanAdapter::parse_change`]: canon_ingest::PlanAdapter::parse_change
pub(crate) fn refresh_change(repo: &Path, change_id: &ChangeId) -> Result<ChangeRefresh, PlansError> {
    let repo = resolve_repo_root(repo);
    let sources = load_plan_sources_for_gate(&repo)?;
    let (store, _) = open_store(&repo, &repo.join("canon.yaml"))?;
    let mut adoptions = Adoptions::default();
    let mut seen_change_ids: BTreeSet<ChangeId> = BTreeSet::new();
    let mut unwritten = PlansOutcome::default();
    let mut refresh = ChangeRefresh { changes_parsed: 0, malformed: 0 };
    for src in &sources {
        let entry = plan_registry::find(&src.dialect).ok_or_else(|| PlansError::Config(format!("`{}` is not a registered plan dialect", src.dialect)))?;
        let Some(handle) = entry.adapter.resolve_source(&PlanSourceConfig { root: Some(src.root.clone()) }) else {
            continue;
        };
        let PlanParseOutcome { changes, tasks, malformed, .. } = entry.adapter.parse_change(&handle, change_id);
        refresh.changes_parsed += changes.len();
        refresh.malformed += malformed.len();
        let admitted = admit_source_candidates(&mut seen_change_ids, changes, tasks);
        persist_candidates(&store, &mut adoptions, admitted.changes, admitted.tasks, &mut unwritten)?;
    }
    Ok(refresh)
}

/// One plan source's [`admit_source_candidates`] verdict. Named, not a
/// positional `(Vec<Change>, Vec<Task>, BTreeSet<ChangeId>)` triple:
/// two of the three are same-shaped collections whose swap at a call
/// site would compile and silently persist the wrong half.
#[derive(Debug)]
pub(crate) struct SourceAdmission {
    /// The `Change`s this source is the FIRST to carry — exactly the
    /// ones an import pass offers to a tier.
    pub(crate) changes: Vec<Change>,
    /// The `Task`s that survive the rule — exactly the ones an import
    /// pass offers to a tier, and therefore exactly the ids a
    /// `Run.task_id` binding may name (`crate::dispatch`).
    pub(crate) tasks: Vec<Task>,
    /// The `change_id`s this source re-declared after an
    /// earlier-configured source already owned them, deduplicated (one
    /// source may repeat an id across documents) — the
    /// [`PlanSourceSummary::duplicate_change_id`] diagnostic's count.
    pub(crate) duplicate_change_ids: BTreeSet<ChangeId>,
}

/// Design D8's cross-source `change_id` admission (module doc), for ONE
/// source's parsed candidates: the first configured occurrence of a
/// `change_id` owns it for the whole pass, and every later occurrence
/// is skipped — its `Change` AND every `Task` sitting under that
/// `change_id`.
///
/// # Why this is a shared function and not an inline loop
/// s41 (`review-hardening`): this rule had a SECOND, laxer copy.
/// `crate::dispatch`'s `--task` validation admitted a row as soon as
/// ANY configured source parsed it, so a row under a later duplicate
/// of a change id — a row this driver drops on the floor and never
/// persists — was written into a dispatched `Run.task_id` as precisely
/// the dangling edge s40 exists to refuse. A `--task` is bindable IFF
/// `canon ingest plans` would persist that task, so both callers must
/// decide by the same predicate; two copies of an admission rule is
/// how that defect arose in the first place.
///
/// # Contract
/// `seen_change_ids` is the PASS-WIDE accumulator, threaded across
/// sources in strict config order by the caller — order is the rule
/// here, so this is deliberately not commutative — and is extended with
/// every `change_id` this source is the first to carry.
///
/// A `Task` whose `change_id` NO source produced a `Change` for is
/// never skipped: only an id observed twice as a `Change` can enter
/// [`SourceAdmission::duplicate_change_ids`]. Ownership keys on the
/// change document, and a change id with no earlier owner has nothing
/// to lose to.
pub(crate) fn admit_source_candidates(
    seen_change_ids: &mut BTreeSet<ChangeId>,
    changes: Vec<Change>,
    tasks: Vec<Task>,
) -> SourceAdmission {
    let mut duplicate_change_ids: BTreeSet<ChangeId> = BTreeSet::new();
    let mut accepted_changes = Vec::with_capacity(changes.len());
    for change in changes {
        if seen_change_ids.contains(&change.change_id) {
            duplicate_change_ids.insert(change.change_id.clone());
        } else {
            seen_change_ids.insert(change.change_id.clone());
            accepted_changes.push(change);
        }
    }
    // Filtered only AFTER the whole change list is classified: a
    // source's tasks may precede the duplicate `Change` that disowns
    // them, so a streaming filter would admit half of them by ordering
    // accident.
    let accepted_tasks: Vec<Task> = tasks.into_iter().filter(|t| !duplicate_change_ids.contains(&t.task_id.change_id())).collect();
    SourceAdmission { changes: accepted_changes, tasks: accepted_tasks, duplicate_change_ids }
}

/// `(--dialect, --source)` one-shot override (design D2, task 3.3) or
/// `canon.yaml` `plans:` (task 3.2) — never both attempted, never a
/// silent fallback between them.
fn resolve_sources(repo: &Path, canon_yaml_path: &Path, dialect: Option<&str>, source: Option<&Path>) -> Result<Vec<PlanSource>, PlansError> {
    match (dialect, source) {
        (Some(d), Some(s)) => {
            ensure_dialect_registered(d)?;
            Ok(vec![PlanSource { dialect: d.to_string(), root: s.to_path_buf() }])
        }
        (Some(_), None) | (None, Some(_)) => Err(PlansError::Config(
            "`--dialect` and `--source` must be given together (a one-shot plan-import override needs both; give neither for the config-driven canon.yaml `plans:` scan)".to_string(),
        )),
        (None, None) => load_plan_sources_from_config(canon_yaml_path, repo),
    }
}

fn ensure_dialect_registered(dialect: &str) -> Result<(), PlansError> {
    if plan_registry::find(dialect).is_some() {
        return Ok(());
    }
    Err(PlansError::Config(format!("`{dialect}` is not a registered plan dialect (registered: {}); fix the dialect id", registered_dialect_ids().join(", "))))
}

fn registered_dialect_ids() -> Vec<&'static str> {
    let mut ids: Vec<&str> = plan_registry::registry().iter().map(|e| e.dialect_id()).collect();
    ids.sort_unstable();
    ids
}

/// Parse `canon.yaml`'s `plans:` section (task 3.2; see module doc for
/// the fail-soft-on-absent vs fail-loud-on-present split, mirroring
/// `crate::ingest::IngestSourceConfig::load`'s established pattern).
pub(crate) fn load_plan_sources_from_config(canon_yaml_path: &Path, repo: &Path) -> Result<Vec<PlanSource>, PlansError> {
    // Missing/unreadable canon.yaml: fail-soft (no config at all is a
    // legitimate first-run state) -- zero sources, same as an absent
    // `plans:` section.
    let Ok(text) = std::fs::read_to_string(canon_yaml_path) else {
        return Ok(Vec::new());
    };
    // A PRESENT but non-YAML canon.yaml fails LOUD -- a syntax typo in
    // a canon.yaml meant to set `plans.sources` must never silently
    // resolve to zero sources (mirrors `crate::ingest`'s identical
    // discipline).
    let doc: serde_yaml::Value = serde_yaml::from_str(&text)
        .map_err(|e| PlansError::Config(format!("canon.yaml is not valid YAML (fail-loud so an intended `plans:` section is never silently dropped to zero sources): {e}")))?;
    let Some(plans_val) = doc.get("plans") else {
        // Absent `plans:` -- clean, explicit no-op (task 3.2).
        return Ok(Vec::new());
    };
    // PRESENT `plans:` -- strict from here on.
    let raw: RawPlans = serde_yaml::from_value(plans_val.clone())
        .map_err(|e| PlansError::Config(format!("canon.yaml `plans:` section is malformed (fail-loud -- a silent fallback would scan zero sources while appearing configured): {e}")))?;

    let mut sources = Vec::with_capacity(raw.sources.len());
    for (i, s) in raw.sources.into_iter().enumerate() {
        if plan_registry::find(&s.dialect).is_none() {
            return Err(PlansError::Config(format!(
                "canon.yaml `plans.sources[{i}]` names unregistered dialect `{}` (registered: {}); fix the dialect id or remove the entry",
                s.dialect,
                registered_dialect_ids().join(", ")
            )));
        }
        let root = if s.root.is_absolute() { s.root } else { repo.join(&s.root) };
        sources.push(PlanSource { dialect: s.dialect, root });
    }
    Ok(sources)
}

/// Whether `canon.yaml` carries a `plans:` key AT ALL — the signal
/// [`load_plan_sources_for_gate`] needs to tell "a pre-s35 repo that
/// never knew about `plans:`" apart from "a repo that configured
/// `plans.sources: []` on purpose". Both resolve to zero sources
/// through [`load_plan_sources_from_config`], but only the former
/// earns the openspec compat default. Unreadable/non-YAML both answer
/// `false`; the loud parse error is
/// [`load_plan_sources_from_config`]'s job, and this is only ever
/// consulted after that call already returned `Ok`.
fn plans_section_present(canon_yaml_path: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(canon_yaml_path) else {
        return false;
    };
    serde_yaml::from_str::<serde_yaml::Value>(&text).is_ok_and(|doc| doc.get("plans").is_some())
}

/// The plan sources `canon gate task` resolves a flip through (s35
/// `gate-plan-dialect-seam`, design "Compat default"). Loads
/// `canon.yaml`'s `plans:` sources exactly like `canon ingest plans`
/// ([`load_plan_sources_from_config`]) — but where that command treats
/// an ABSENT `plans:` section as zero sources (a clean no-op scan),
/// `canon gate task` instead falls back to the documented compat
/// default `[{ dialect: openspec, root: <repo> }]` so every consumer
/// that worked before s35 (a repo with no `plans:` section, whose
/// `tasks.md` lives at `<repo>/openspec/changes/<change_id>/`) keeps
/// working — the dependence moves from hardcoded to configured-default,
/// never removed. A PRESENT-but-malformed `plans:` section still fails
/// loud here, exactly as for `canon ingest plans`.
///
/// The fallback is keyed on the section's ABSENCE
/// ([`plans_section_present`]), never on "zero sources resolved". A
/// repo that writes `plans:\n  sources: []` — which is exactly what
/// `canon init` scaffolds — has configured a plan corpus and declared
/// it empty, so synthesizing an openspec source under it would make
/// `canon gate task` report `consulted: openspec @ <repo>` against a
/// dialect the operator never named. That is the same misleading shape
/// `crate::dispatch`'s `--task` validation already refuses via
/// `DispatchError::NoPlanSources`; the two commands now agree.
pub(crate) fn load_plan_sources_for_gate(repo: &Path) -> Result<Vec<PlanSource>, PlansError> {
    let canon_yaml_path = repo.join("canon.yaml");
    let mut sources = load_plan_sources_from_config(&canon_yaml_path, repo)?;
    if sources.is_empty() && !plans_section_present(&canon_yaml_path) {
        sources.push(PlanSource { dialect: "openspec".to_string(), root: repo.to_path_buf() });
    }
    Ok(sources)
}

/// A malformed CONFIGURATION fails loud BEFORE any scan (spec
/// "Malformed plan sources fail soft per construct, loud per
/// configuration") -- a nonexistent source root is exactly that, never
/// a silent empty scan indistinguishable from an empty plan tree.
fn validate_source_roots(sources: &[PlanSource]) -> Result<(), PlansError> {
    for src in sources {
        if !src.root.exists() {
            return Err(PlansError::Config(format!("plan source (dialect `{}`) root `{}` does not exist", src.dialect, src.root.display())));
        }
    }
    Ok(())
}

/// One `SourceCursor` id per configured `(dialect, parse_version, root)`
/// source (task 3.4) -- `plan-<dialect>-v<parse_version>-<digest12>`,
/// where `<digest12>` is `canon_ingest::normalize::content_digest` of
/// the root's own path string: two sources naming the same dialect at
/// DIFFERENT roots (or vice versa) never collide on one cursor file, and
/// the id stays a safe bare filename component regardless of what the
/// root path itself contains.
///
/// `parse_version` ([`canon_ingest::PlanAdapter::parse_version`]) joins
/// the identity because of what this cursor's gate actually compares
/// (`s38-evidence-bearing-memory`): per-file CONTENT digests. Those are
/// byte-stable by design — an unchanged plan re-imports idempotently —
/// which means a change to the PARSER is invisible to the gate and the
/// source is reported `skipped unchanged` even though its records would
/// now parse differently. Folding the dialect's parse generation into
/// the cursor ID rather than into its per-file digests is what makes a
/// bump behave EXACTLY like editing every file in the source: the
/// lookup finds no cursor at the new id at all, so every file is
/// re-read, re-parsed, and re-persisted, and the stale cursor is simply
/// orphaned rather than mutated.
fn plan_source_cursor_id(dialect: &str, parse_version: u32, root: &Path) -> String {
    let digest = canon_ingest::normalize::content_digest(&serde_json::json!(root.to_string_lossy()));
    format!("plan-{dialect}-v{parse_version}-{digest}")
}

/// A file's `(mtime_ms, size)` for the cursor's informational summary
/// fields -- best-effort, mirrors `crate::ingest::file_stat` (the gate
/// itself decides on the content digest alone, never these).
fn file_stat(path: &Path) -> (i64, u64) {
    match std::fs::metadata(path) {
        Ok(meta) => {
            let mtime_ms = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64).unwrap_or(0);
            (mtime_ms, meta.len())
        }
        Err(_) => (0, 0),
    }
}

/// Canonicalize `path` (resolving symlinks/`.`/`..` so a
/// `starts_with` check against it is a real-filesystem comparison,
/// never a text-level one) when it exists; falls back to `path`
/// as-is when it doesn't (F2: this only ever happens for a tier this
/// run hasn't written anything to yet, so no scanned file can be
/// "under" it regardless).
fn canonicalize_or(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Persist one candidate through [`TierRegistry::persist`] --
/// connector-never-authority's ONE write path, never bypassed. `Ok(None)`
/// = durably persisted, OR an idempotent `StoreError::DuplicatePath`
/// no-op (an already-identical git-tier resubmission -- mirrors
/// `crate::ingest::persist_idempotent`'s established discipline).
/// `Ok(Some(record))` = the routed tier is unreachable
/// (`TierUnavailable`) or the kind has no routing entry at all
/// (`UnroutedKind`) -- task 3.5's documented `unwritten` seam,
/// non-fatal; the caller reports the record and does not advance this
/// source's cursor. Any OTHER `StoreError` (Io/Json/Layout/…) is a
/// genuine failure and propagates.
fn persist_or_unwritten<T: CanonRecord>(store: &TierRegistry, record: T) -> Result<Option<T>, PlansError> {
    match store.persist(&record) {
        Ok(_) => Ok(None),
        Err(StoreError::DuplicatePath { .. }) => Ok(None),
        Err(StoreError::TierUnavailable { .. }) | Err(StoreError::UnroutedKind { .. }) => Ok(Some(record)),
        Err(err) => Err(PlansError::Store(err)),
    }
}

/// Human-readable run summary (task 3.5: "per-source counts + drop
/// diagnostics + malformed tallies").
pub fn format_human(outcome: &PlansOutcome) -> String {
    let mut out = String::new();
    for src in &outcome.sources {
        if src.skipped_unchanged {
            out.push_str(&format!("{} ({}): skipped unchanged (watermark)\n", src.dialect, src.root));
            continue;
        }
        out.push_str(&format!(
            "{} ({}): {} change(s) parsed / {} persisted / {} unwritten; {} task(s) parsed / {} persisted / {} unwritten; {} duplicate-change-id; {} malformed\n",
            src.dialect,
            src.root,
            src.changes_parsed,
            src.changes_persisted,
            src.changes_unwritten,
            src.tasks_parsed,
            src.tasks_persisted,
            src.tasks_unwritten,
            src.duplicate_change_id,
            src.malformed.len(),
        ));
        for entry in &src.malformed {
            match &entry.hint {
                Some(hint) => out.push_str(&format!("  malformed ({}): {} — {hint}\n", entry.path, entry.reason)),
                None => out.push_str(&format!("  malformed ({}): {}\n", entry.path, entry.reason)),
            }
        }
        for (construct, count) in &src.unmapped {
            out.push_str(&format!("  dropped ({construct}): {count}\n"));
        }
        if !src.cursor_advanced {
            out.push_str("  cursor NOT advanced (pass not fully durable)\n");
        }
    }
    out.push_str(&format!(
        "total: {} change(s) persisted, {} task(s) persisted, {} duplicate-change-id skipped\n",
        outcome.changes_persisted, outcome.tasks_persisted, outcome.duplicate_change_id
    ));
    if !outcome.unwritten_changes.is_empty() || !outcome.unwritten_tasks.is_empty() {
        out.push_str(&format!(
            "unwritten (routed tier unreachable/unrouted, non-fatal, cursor not advanced for the affected source): {} change(s), {} task(s); printing JSON below\n",
            outcome.unwritten_changes.len(),
            outcome.unwritten_tasks.len()
        ));
    }
    out
}

/// `--json`: the full structured outcome.
pub fn format_json(outcome: &PlansOutcome) -> String {
    serde_json::to_string_pretty(outcome).expect("PlansOutcome always serializes")
}

/// The documented `unwritten` seam's own body (module doc), printed by
/// default regardless of `--json` -- mirrors `crate::ingest`'s
/// "the only copy of the normalized output must never be silently
/// discarded" fix (ReviewS3Full finding 4). `None` when nothing
/// degraded this pass.
pub fn format_unwritten_json(outcome: &PlansOutcome) -> Option<String> {
    if outcome.unwritten_changes.is_empty() && outcome.unwritten_tasks.is_empty() {
        return None;
    }
    #[derive(Serialize)]
    struct UnwrittenBody<'a> {
        changes: &'a [Change],
        tasks: &'a [Task],
    }
    let body = UnwrittenBody { changes: &outcome.unwritten_changes, tasks: &outcome.unwritten_tasks };
    Some(serde_json::to_string_pretty(&body).expect("unwritten body always serializes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `s38-evidence-bearing-memory` fix: the cursor's own gate
    /// compares byte-stable per-file content digests, so a PARSER change
    /// is invisible to it. Folding the dialect's parse generation into
    /// the cursor ID is what forces a full re-parse — a bumped
    /// `parse_version` must land on a DIFFERENT id, so `cursors.read`
    /// finds nothing and every file is re-read.
    #[test]
    fn a_bumped_parse_version_yields_a_different_cursor_id_for_the_same_source() {
        let root = Path::new("/tmp/plans/openspec");
        let before = plan_source_cursor_id("openspec", 1, root);
        let after = plan_source_cursor_id("openspec", 2, root);
        assert_ne!(before, after, "a parse-version bump must not reuse the prior cursor id, or the source is reported `skipped unchanged`");
    }

    /// The pre-existing collision guarantee (task 3.4), unweakened by
    /// the added version segment: two roots under one dialect, and two
    /// dialects over one root, still never share a cursor file.
    #[test]
    fn cursor_ids_stay_distinct_per_dialect_and_per_root() {
        let one = plan_source_cursor_id("openspec", 2, Path::new("/tmp/plans/a"));
        let other_root = plan_source_cursor_id("openspec", 2, Path::new("/tmp/plans/b"));
        let other_dialect = plan_source_cursor_id("superpowers", 2, Path::new("/tmp/plans/a"));
        assert_ne!(one, other_root);
        assert_ne!(one, other_dialect);
    }

    /// A cursor id is used as a bare filename component
    /// ([`plan_source_cursor_id`]'s own contract), so the version
    /// segment must not introduce a path separator or any other
    /// filename-hostile character.
    #[test]
    fn a_cursor_id_stays_a_safe_bare_filename_component() {
        let id = plan_source_cursor_id("openspec", 2, Path::new("/tmp/plans/../weird path/openspec"));
        assert!(id.starts_with("plan-openspec-v2-"), "expected the dialect and version segments up front, got `{id}`");
        assert!(id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'), "cursor id `{id}` must stay filename-safe");
    }
}
