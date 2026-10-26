//! `canon inventory sync [--spec-root <dir>]` (s15 P3a,
//! `inventory-materialization` spec, tasks 3.1/3.4/3.5): the GENERAL
//! feature-corpus → ledger indexer. For EACH configured
//! `specs.roots[]` entry (`canon.yaml`, design D3):
//!
//! 1. Validate with `canon-fmt::check` (S11) — ANY violation ABORTS
//!    THE WHOLE ROOT, writing zero `Scenario` records for it and
//!    reporting the violation(s); never a partial sync.
//! 2. Scan `kind=feature/` via the extended `canon_fmt::gherkin::scan`
//!    (task 3.2) — each tag paired with its header's label as `title`,
//!    `source_digest` a sha256 over the `.feature` file's raw bytes, and
//!    `scenario_digest` a sha256 over that one scenario's meaning
//!    (`canon_fmt::gherkin::scenario_digests`, 0.15 D1).
//! 3. Materialize ONE `Scenario` index record per `(project_id,
//!    scenario_id)` via the normal append-only `GitTier::write`.
//!    Idempotence is LOGICAL (design D5): fold latest-per-key first
//!    (`canon_store::fold_latest_by_key`), no-op when the candidate's
//!    digests/title/tags already match the latest folded record.
//!
//! # General index — never `upstream`/`InventoryEntry.covered_by`
//! The index derives ONLY from the `.feature` corpus — this module
//! NEVER reads an `upstream`/`InventoryEntry.covered_by` inventory (a
//! donor porting concern, not core canon, which is a
//! general spec-planning tool). A root's `inventory/` directory, if
//! present, is validated by `canon-fmt::check` as ordinary S11 corpus
//! hygiene — orthogonal to what `sync` indexes; `covered`/`surface_ref`
//! were dropped from `canon_model::Scenario` (task 3.3) precisely
//! because no general populator exists for them — that enrichment is
//! plugin-extensible (a future s16 porting plugin's own
//! foreign-namespace overlay record), never a core sync-populated
//! field.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use canon_fmt::Violation;
use canon_gate::{GateContext, GateCtx};
use canon_model::ids::SpecDigest;
use canon_model::{paths, Actor, Envelope, ProjectId, RecordKind, Scenario, ScenarioId, SubjectId};
use canon_store::file_lock::{FileLock, FileLockError};
use canon_store::fold_latest_by_key;
use canon_store::git_tier::GitTier;
use canon_store::tier::{StoreError, Tier, TierQuery};
use chrono::Utc;

use crate::context::resolve_repo_root;
use crate::migrate_legacy_pins::SpecSources;
use crate::subject::enum_membership_in;

/// The stable literal id BOTH the absent-`specs:` default AND
/// `--spec-root`'s ad hoc override resolve to (design D3) — NEVER the
/// checkout directory name.
const DEFAULT_SPEC_ROOT_ID: &str = "root";
/// Where the default root, and an entry without `root:`, lives (0.17 D1).
const DEFAULT_SPEC_ROOT_DIR: &str = paths::SPECS_DIR;

fn default_root_id() -> ProjectId {
    ProjectId::parse(DEFAULT_SPEC_ROOT_ID).expect("literal default spec-root id is a valid ProjectId")
}

/// One resolved `specs.roots[]` entry (design D3): `id` is a stable,
/// author-declared literal — never derived from a checkout path;
/// `root` is already resolved to an absolute path (relative entries
/// joined against `canon.yaml`'s directory).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecRoot {
    pub id: ProjectId,
    pub root: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum InventoryError {
    /// A PRESENT `canon.yaml` `specs:` section is malformed, or an
    /// entry's `id` doesn't parse as a [`ProjectId`] — fail loud rather
    /// than silently falling back to the default root or the wrong
    /// corpus (spec: "config resolution ... does NOT fall back to the
    /// default root").
    #[error("{0}")]
    Config(String),
    #[error(transparent)]
    Store(#[from] StoreError),
    /// A present `canon.yaml` the tier parser rejects: the ledger this
    /// sync would write to cannot be resolved ([`GateCtx::from_repo`]).
    #[error(transparent)]
    CanonYaml(#[from] canon_gate::CanonYamlError),
    /// Another canon process held the repo's scenario-index lock for
    /// longer than a writer waits ([`lock_index`]).
    #[error("{0}")]
    Locked(String),
}

/// `canon.yaml`'s `specs:` section, parsed STRICTLY
/// (`deny_unknown_fields`) once PRESENT — an absent key never reaches
/// this type at all (the caller returns the single default root first).
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSpecs {
    #[serde(default)]
    roots: Vec<RawSpecRoot>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSpecRoot {
    /// REQUIRED, no default — an entry missing `id` fails the whole
    /// `specs:` parse loud (no `ProjectId` may ever be silently
    /// synthesized from a checkout path).
    id: String,
    #[serde(default)]
    root: Option<String>,
    /// Where this root lived before `canon migrate` moved it (0.17 D6),
    /// newest last. Read only by `canon migrate`'s review pinning
    /// ([`spec_sources`]): a pre-0.15 review pinned to a commit from before
    /// the move finds its scenario under these paths at that commit.
    #[serde(default)]
    former_roots: Vec<String>,
}

fn default_root(repo_root: &Path) -> SpecRoot {
    SpecRoot { id: default_root_id(), root: repo_root.join(DEFAULT_SPEC_ROOT_DIR) }
}

/// `canon.yaml`'s `specs.roots[]` (design D3, task 3.1). Reuses ONLY
/// `IngestSourceConfig::load`'s fail-soft/fail-loud SEMANTICS (missing
/// or unreadable `canon.yaml`, or an absent `specs:` key → the single
/// default root; a non-YAML `canon.yaml`, or a PRESENT-but-malformed
/// `specs:` section → fails LOUD) — the named multi-root LIST shape
/// itself is new here, not a copy of ingest's per-source `roots`
/// override.
pub fn load_spec_roots(canon_yaml: &Path) -> Result<Vec<SpecRoot>, InventoryError> {
    Ok(load_spec_roots_with_former(canon_yaml)?.into_iter().map(|(root, _)| root).collect())
}

/// [`load_spec_roots`], each root with its `former_roots` resolved the
/// same way as `root` (relative entries joined against `canon.yaml`'s
/// directory).
fn load_spec_roots_with_former(canon_yaml: &Path) -> Result<Vec<(SpecRoot, Vec<PathBuf>)>, InventoryError> {
    let repo_root = canon_yaml.parent().unwrap_or_else(|| Path::new("."));
    let resolve = |dir: &str| {
        let path = Path::new(dir);
        if path.is_absolute() { path.to_path_buf() } else { repo_root.join(path) }
    };

    // Missing / unreadable canon.yaml: fail-soft (no config at all is
    // a legitimate first-run / minimal state) — same as
    // `IngestSourceConfig::load`.
    let Ok(text) = std::fs::read_to_string(canon_yaml) else {
        return Ok(vec![(default_root(repo_root), Vec::new())]);
    };
    // A PRESENT but non-YAML canon.yaml fails LOUD: a syntax typo in a
    // canon.yaml meant to set `specs.roots[]` must never silently
    // resolve the default root and sync the wrong corpus.
    let doc: serde_yaml::Value = serde_yaml::from_str(&text).map_err(|e| {
        InventoryError::Config(format!(
            "canon.yaml is not valid YAML (fail-loud so an intended `specs.roots[]` override is never silently dropped to the default root): {e}"
        ))
    })?;
    let Some(specs_val) = doc.get("specs") else {
        return Ok(vec![(default_root(repo_root), Vec::new())]);
    };
    // PRESENT `specs:` — strict from here on.
    let specs: RawSpecs = serde_yaml::from_value(specs_val.clone())
        .map_err(|e| InventoryError::Config(format!("canon.yaml `specs:` section is malformed (fail-loud — a silent fallback would sync the wrong corpus): {e}")))?;

    // A PRESENT `specs:` section must declare at least one root — an
    // empty `specs.roots[]` (including a `specs: {}` whose `roots`
    // defaults to empty) is a present-but-incomplete config, NOT the
    // absent-key default. Fail loud (exit 2) so an intended-but-
    // mistyped override never silently syncs zero roots and reports a
    // hollow exit-0 success (only an ABSENT `specs:` key -> default).
    if specs.roots.is_empty() {
        return Err(InventoryError::Config(
            "canon.yaml has a `specs:` section but no `specs.roots[]` entries — fail-loud: only an ABSENT `specs:` key resolves the single default root; a present `specs:` must declare at least one root".to_string(),
        ));
    }

    let mut roots = Vec::with_capacity(specs.roots.len());
    for raw in specs.roots {
        let id = ProjectId::parse(&raw.id)
            .map_err(|e| InventoryError::Config(format!("canon.yaml `specs.roots[]` entry `id: {}` is not a valid ProjectId: {e}", raw.id)))?;
        let root = resolve(raw.root.as_deref().unwrap_or(DEFAULT_SPEC_ROOT_DIR));
        let former = raw.former_roots.iter().map(|dir| resolve(dir)).collect();
        roots.push((SpecRoot { id, root }, former));
    }
    Ok(roots)
}

/// The refusal every write command makes when its `--project-id` names
/// no spec root configured in `<repo>/canon.yaml` (0.26, #104). A record
/// written under such an id attests nothing the gate counts (a scenario
/// is keyed `(project_id, scenario_id)`), and no command removes one
/// record, so the id is checked before anything is written. The hint
/// names the configured roots, and with one root the exact flag to use.
/// An unreadable `canon.yaml` is `CONFIG_UNREADABLE`, as everywhere else.
/// Read-only queries are not checked: a filter on an id no root carries
/// matches nothing and writes nothing.
pub fn require_spec_root(repo: &Path, project_id: &ProjectId) -> Result<(), crate::cli_error::CliError> {
    use crate::cli_error::CliError;
    let roots = load_spec_roots(&repo.join("canon.yaml")).map_err(|e| {
        CliError::refused("CONFIG_UNREADABLE", e.to_string()).hint("fix canon.yaml so it parses (`canon init --check` reports what is wrong), then run the command again")
    })?;
    check_spec_root(&roots, project_id)
}

/// [`require_spec_root`] over roots already loaded.
pub fn check_spec_root(roots: &[SpecRoot], project_id: &ProjectId) -> Result<(), crate::cli_error::CliError> {
    use crate::cli_error::CliError;
    if roots.iter().any(|root| &root.id == project_id) {
        return Ok(());
    }
    let ids: Vec<&str> = roots.iter().map(|root| root.id.as_str()).collect();
    let hint = match ids.as_slice() {
        [one] => format!("the only spec root canon.yaml configures is `{one}`; pass `--project-id {one}`"),
        many => format!("canon.yaml configures the spec roots {}; pass `--project-id <one of them>`", many.iter().map(|id| format!("`{id}`")).collect::<Vec<_>>().join(", ")),
    };
    Err(CliError::refused(
        "UNKNOWN_SPEC_ROOT",
        format!("`--project-id {}` names no spec root configured in canon.yaml (`specs.roots[]`: {}); nothing was written", project_id.as_str(), ids.join(", ")),
    )
    .hint(hint)
    .field("--project-id")
    .expected(ids))
}

/// The spec root a leaf writes under when `--project-id` is optional (D1):
/// the one given, else the only root `canon.yaml` configures. With several
/// roots and none given the leaf refuses with `code`, naming the roots. The
/// id given is not checked here ([`require_spec_root`] does that).
pub fn resolve_project_id(repo: &Path, given: Option<&ProjectId>, code: &'static str, command: &str, json: bool) -> Result<ProjectId, u8> {
    use crate::cli_error::CliError;
    if let Some(id) = given {
        return Ok(id.clone());
    }
    let roots = match load_spec_roots(&repo.join("canon.yaml")) {
        Ok(roots) => roots,
        Err(e) => {
            return Err(CliError::refused("CONFIG_UNREADABLE", e.to_string())
                .hint("fix canon.yaml so it parses (`canon init --check` reports what is wrong), then run the command again")
                .emit(command, json));
        }
    };
    match roots.as_slice() {
        [one] => Ok(one.id.clone()),
        many => {
            let ids: Vec<&str> = many.iter().map(|root| root.id.as_str()).collect();
            Err(CliError::refused(code, format!("canon.yaml configures {} spec roots ({}); `--project-id` names the one to use", ids.len(), ids.join(", ")))
                .hint(format!("pass `--project-id <one of {}>`", ids.join(", ")))
                .field("--project-id")
                .expected(ids)
                .emit(command, json))
        }
    }
}

/// Rebindable roots inventory-sync reads through (design D3,
/// spec-ledger-selftest Req 2) — composes over [`GateCtx`]'s own
/// two-constructor pattern (`from_repo`/`from_fixture`) rather than a
/// second, hand-rolled copy of its `<repo>/canon.yaml` `tiers.git.root`
/// resolution / default `.canon/ledger` layout: inventory-sync and the
/// gate read the SAME ledger. [`SyncCtx::spec_roots`] is the ONE
/// `specs.roots[]` resolver both constructors run through —
/// [`run_sync_with_ctx`] is the ONE downstream sync entry point a
/// production `canon inventory sync` run (`SyncCtx::from_repo`) and a
/// fixture-corpus `canon selftest` run (`SyncCtx::from_fixture`) both
/// call; neither branches on which constructor built its `ctx`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncCtx {
    /// The repo root `canon.yaml`'s `specs:` section resolves against.
    pub repo: PathBuf,
    /// The `GitTier` root `Scenario` index records are read/written
    /// through — identical resolution [`GateCtx`] uses (S2's own
    /// `TierPolicy`, never a second gate-only config path).
    pub ledger_root: PathBuf,
}

impl SyncCtx {
    /// Production binding: `ledger_root` resolved off `<repo>/canon.yaml`
    /// exactly as [`GateCtx::from_repo`] does (reused directly, never a
    /// second copy of that resolution), including its refusal of a
    /// present but unusable `canon.yaml`.
    pub fn from_repo(repo: impl Into<PathBuf>) -> Result<Self, InventoryError> {
        let gate_ctx = GateCtx::from_repo(repo)?;
        Ok(Self { repo: gate_ctx.repo, ledger_root: gate_ctx.ledger_root })
    }

    /// Fixture binding (spec-ledger-selftest Req 2's "fixture constructor
    /// runs fully offline against a tempdir" scenario): every root under
    /// one fresh tempdir, the identical `.canon/ledger` default layout
    /// [`GateCtx::from_fixture`] uses. Never reads a `canon.yaml` at all
    /// for `ledger_root` (there is none in a fresh fixture dir);
    /// [`SyncCtx::spec_roots`] below still probes for one through the
    /// SAME resolver `from_repo` uses — absent, so it falls back to the
    /// single default root, the identical fail-soft semantics
    /// `load_spec_roots` always applies, never a fixture-only special
    /// case.
    pub fn from_fixture(fixture_dir: impl Into<PathBuf>) -> Self {
        let gate_ctx = GateCtx::from_fixture(fixture_dir);
        Self { repo: gate_ctx.repo, ledger_root: gate_ctx.ledger_root }
    }

    /// Resolve this ctx's `specs.roots[]` (design D3) — the SAME
    /// [`load_spec_roots`] fail-soft/fail-loud resolver for both
    /// constructors above; `spec_root_override` bypasses config
    /// resolution ENTIRELY (matching `canon inventory sync
    /// --spec-root`'s existing CLI contract) before it ever reads
    /// `canon.yaml`.
    pub fn spec_roots(&self, spec_root_override: Option<&Path>) -> Result<Vec<SpecRoot>, InventoryError> {
        match spec_root_override {
            Some(dir) => {
                let root = if dir.is_absolute() { dir.to_path_buf() } else { self.repo.join(dir) };
                Ok(vec![SpecRoot { id: default_root_id(), root }])
            }
            None => load_spec_roots(&self.repo.join("canon.yaml")),
        }
    }
}

/// One root's sync outcome. A non-empty `violations` (S11 fmt gaps) or
/// `sync_errors` (a duplicate-key corpus fault the frozen
/// `FmtFailureClass` can't express) means this root's materialization
/// was ABORTED WHOLE-ROOT (spec: "sync writes zero `Scenario` records
/// for it") — `written` is always `0` in that case (`scanned` reflects
/// what the pre-abort scan saw: `0` on an S11 abort that precedes the
/// scan, the scanned count on a post-scan duplicate abort).
#[derive(Debug, Clone)]
pub struct RootSyncOutcome {
    pub id: ProjectId,
    pub root: PathBuf,
    /// Scenarios found by the gherkin scan, whether or not they
    /// resulted in a new write (logical idempotence may no-op most of
    /// them).
    pub scanned: usize,
    /// Records this run wrote, tombstones included.
    pub written: usize,
    /// Of `written`: tombstones for scenarios the corpus no longer
    /// declares ([`sync_one_root`]).
    pub removed: usize,
    pub violations: Vec<Violation>,
    /// Per-root sync-level abort reasons the frozen
    /// [`canon_fmt::FmtFailureClass`] deliberately cannot express (its
    /// 11 classes are the audited S11 gap set, "no more, no less") —
    /// today, a duplicate `(project_id, scenario_id)` in this root's
    /// scan. Like `violations`, non-empty means WHOLE-ROOT abort (0
    /// writes); unlike a config fault it aborts only THIS root, so
    /// sibling roots still materialize and the CLI still exits `1`.
    pub sync_errors: Vec<String>,
    /// Fail-SOFT per-scenario tag diagnostics (s36 task 6.2): a
    /// malformed `@subject:<id>` value (rejected by [`SubjectId`]'s
    /// grammar) or two-or-more `@subject:` tags on one scenario. Unlike
    /// `violations`/`sync_errors`, these NEVER abort — the scenario is
    /// still indexed (a malformed/duplicate tag simply leaves
    /// `Scenario.subject_id` unset, first-tag-wins on a duplicate), so
    /// they do NOT flip [`RootSyncOutcome::is_clean`] and the sync still
    /// exits `0`; they are counted and printed so an author sees the
    /// dropped join. Mirrors how the gherkin scan treats an unrecognized
    /// tag as a soft skip, never a hard corpus error.
    pub tag_diagnostics: Vec<String>,
}

impl RootSyncOutcome {
    pub fn is_clean(&self) -> bool {
        self.violations.is_empty() && self.sync_errors.is_empty()
    }
}

#[derive(Debug, Clone, Default)]
pub struct SyncOutcome {
    pub roots: Vec<RootSyncOutcome>,
    /// A `--dry-run` preview: `written` counts what the sync would write.
    pub dry_run: bool,
}

impl SyncOutcome {
    pub fn is_clean(&self) -> bool {
        self.roots.iter().all(RootSyncOutcome::is_clean)
    }

    pub fn total_written(&self) -> usize {
        self.roots.iter().map(|r| r.written).sum()
    }
}

/// `canon inventory sync [--spec-root <dir>]` (module doc). `spec_root`
/// overrides config resolution ENTIRELY — syncs exactly the one ad hoc
/// root at that directory, under the SAME stable literal id the
/// absent-`specs:` default uses, ignoring `canon.yaml`'s `specs:`
/// section altogether. Builds a production [`SyncCtx::from_repo`] and
/// delegates to [`run_sync_with_ctx`] — the CLI's own behavior/exit
/// codes are unchanged, now routed through the same seam a
/// fixture-corpus `canon selftest` run drives.
///
/// Holds the repo's scenario-index lock ([`lock_index`]) for the whole
/// run, so it never interleaves with another sync or a reader's
/// [`refresh_stale_index`].
pub fn run_sync(repo: &Path, spec_root: Option<&Path>) -> Result<SyncOutcome, InventoryError> {
    let outcome = rebuild(repo, spec_root)?;
    if outcome.is_clean() && outcome.total_written() == 0 {
        crate::json_out::mark_unchanged();
    }
    Ok(outcome)
}

/// [`run_sync`] as one step of a larger command (`canon migrate`): the same
/// sync, without marking the whole run unchanged when this step wrote nothing.
pub fn rebuild(repo: &Path, spec_root: Option<&Path>) -> Result<SyncOutcome, InventoryError> {
    sync_repo(repo, spec_root, false)
}

/// [`run_sync`] under `--dry-run`: the same validation and the same
/// decisions, but no lock file and no record written.
pub fn preview_sync(repo: &Path, spec_root: Option<&Path>) -> Result<SyncOutcome, InventoryError> {
    sync_repo(repo, spec_root, true)
}

fn sync_repo(repo: &Path, spec_root: Option<&Path>, dry_run: bool) -> Result<SyncOutcome, InventoryError> {
    let repo = resolve_repo_root(repo);
    let ctx = SyncCtx::from_repo(&repo)?;
    let _lock = if dry_run { None } else { Some(lock_index(&ctx.repo)?) };
    sync_with_ctx(&ctx, spec_root, dry_run)
}

/// The ONE downstream sync entry point (module doc / spec-ledger-selftest
/// Req 2's "no downstream sync code branches on which constructor built
/// its ctx" scenario): [`run_sync`] (production, [`SyncCtx::from_repo`])
/// and `canon selftest`'s inventory fixtures ([`SyncCtx::from_fixture`])
/// both call this exact function, unconditionally. Takes no lock itself:
/// a fixture run owns its fresh scratch directory, and [`run_sync`]
/// holds the lock around this call.
pub fn run_sync_with_ctx(ctx: &SyncCtx, spec_root_override: Option<&Path>) -> Result<SyncOutcome, InventoryError> {
    sync_with_ctx(ctx, spec_root_override, false)
}

fn sync_with_ctx(ctx: &SyncCtx, spec_root_override: Option<&Path>, dry_run: bool) -> Result<SyncOutcome, InventoryError> {
    let roots = ctx.spec_roots(spec_root_override)?;
    let tier = GitTier::new(&ctx.ledger_root);
    // Never an agent-authored attestation — a deterministic
    // materializer over the `.feature` corpus, mirroring
    // `canon-gate`'s own `Actor::new_unattributed("canon-gate")` for
    // its typed-task compile path (never an agent-authored record).
    let actor = Actor::new_unattributed("canon-inventory-sync");
    let scans: Vec<Vec<ScannedScenario>> = roots.iter().map(|root| scan_feature_corpus_detailed(&root.root)).collect();
    let corpus = corpus_ids(&roots, &scans);

    let mut outcomes = Vec::with_capacity(roots.len());
    for (spec_root, scanned) in roots.iter().zip(&scans) {
        let outcome = if dry_run {
            plan_one_root(|| latest_index(&tier).map(std::borrow::Cow::Owned), &ctx.repo, spec_root, scanned, &corpus[&spec_root.id], &actor, None)?.outcome
        } else {
            sync_one_root(&tier, &ctx.repo, spec_root, scanned, &corpus[&spec_root.id], &actor, None)?
        };
        outcomes.push(outcome);
    }
    Ok(SyncOutcome { roots: outcomes, dry_run })
}

/// Every scenario id the `.feature` corpus declares, per project, over
/// all of `roots` — what a tombstone is decided against. A union across
/// roots that share one id, so two roots misconfigured under one
/// `project_id` never mark each other's scenarios removed. `scans[i]` is
/// `roots[i]`'s scan.
fn corpus_ids(roots: &[SpecRoot], scans: &[Vec<ScannedScenario>]) -> BTreeMap<ProjectId, BTreeSet<ScenarioId>> {
    let mut ids: BTreeMap<ProjectId, BTreeSet<ScenarioId>> = BTreeMap::new();
    for (root, scanned) in roots.iter().zip(scans) {
        ids.entry(root.id.clone()).or_default().extend(scanned.iter().map(|s| s.scenario_id.clone()));
    }
    ids
}

/// The latest generation of every indexed scenario, per `(project_id,
/// scenario_id)` — tombstones included, since a removed latest
/// generation is exactly what a re-added scenario must supersede. The
/// fold [`sync_one_root`] writes against and [`stale_roots`] compares
/// with, so the two never disagree about what the index holds.
///
/// Each record's OWN content digest and format generation ride along
/// into the fold (s21 D3: the tie-break needs a real digest, never just
/// `at`; `s38-evidence-bearing-memory`: an equal-`at` tie is decided by
/// `envelope.schema` FIRST) — `content_digest12` over the record's raw
/// JSON, the same digest `GitTier`/`PgTier` writes already compute.
fn latest_index(tier: &GitTier) -> Result<BTreeMap<(ProjectId, ScenarioId), Scenario>, InventoryError> {
    let existing = tier.read(&TierQuery::kind(RecordKind::Scenario))?;
    let indexed: Vec<(Scenario, String)> = existing
        .records
        .iter()
        .filter_map(|raw| serde_json::from_value::<Scenario>(raw.0.clone()).ok().map(|scenario| (scenario, canon_store::partition::content_digest12(&raw.0))))
        .collect();
    Ok(fold_latest_by_key(
        indexed,
        |(scenario, _)| (scenario.project_id.clone(), scenario.scenario_id.clone()),
        |(scenario, _)| scenario.envelope.at,
        |(scenario, _)| scenario.envelope.schema,
        |(_, digest)| digest.as_str(),
    )
    .into_iter()
    .map(|(key, (scenario, _))| (key, scenario))
    .collect())
}

/// The scenarios of project `id` the index still holds live (latest
/// generation not removed) but `corpus` no longer declares: what a sync
/// of that project marks removed.
fn removed_from_corpus<'a>(latest: &'a BTreeMap<(ProjectId, ScenarioId), Scenario>, id: &ProjectId, corpus: &BTreeSet<ScenarioId>) -> Vec<&'a Scenario> {
    latest.values().filter(|scenario| &scenario.project_id == id && !scenario.removed && !corpus.contains(&scenario.scenario_id)).collect()
}

/// How long a writer of the scenario index waits for another one to
/// finish before giving up.
const INDEX_LOCK_WAIT: std::time::Duration = std::time::Duration::from_secs(30);

/// The repo's scenario-index lock ([`paths::SCENARIO_INDEX_LOCK_FILE`]),
/// held until this value is dropped.
///
/// An OS advisory lock on the file ([`canon_store::file_lock`]: `flock`
/// on Unix, `LockFileEx` on Windows), not the file's existence: closing
/// the file releases it, and so does the holder's death, however it
/// died, so a killed sync never leaves a stale lock behind and the file
/// itself is never deleted. Waiting is bounded ([`INDEX_LOCK_WAIT`],
/// polled), and running out is [`InventoryError::Locked`], naming the
/// file.
struct IndexLock {
    _lock: FileLock,
}

/// Take [`IndexLock`] for the repo rooted at `repo`.
fn lock_index(repo: &Path) -> Result<IndexLock, InventoryError> {
    match FileLock::acquire(&repo.join(paths::SCENARIO_INDEX_LOCK_FILE), INDEX_LOCK_WAIT) {
        Ok(lock) => Ok(IndexLock { _lock: lock }),
        Err(FileLockError::Busy(waited)) => Err(InventoryError::Locked(format!(
            "another canon process has been writing the scenario index for {}s (it holds `{}`); retry once it finishes",
            waited.as_secs(),
            paths::SCENARIO_INDEX_LOCK_FILE
        ))),
        Err(FileLockError::Io(e)) => Err(StoreError::from(e).into()),
    }
}

/// Scan `root`'s `kind=feature/` corpus for every well-formed
/// `(scenario_id, title, source_digest)` triple the `.feature` files
/// there declare — the SAME walk [`sync_one_root`] performs to decide
/// what to materialize, factored out so a plugin overlay source (s16
/// P4, `canon_cli::plugin_sync::PortingOverlaySource`) can derive the
/// IDENTICAL `(project_id, scenario_id)` universe `canon inventory
/// sync` would index from the SAME root, without a second,
/// independently-maintained copy of this walk (`plugin_sync`'s own
/// module doc: "for every `(project_id, scenario_id)` `canon
/// inventory sync` would index").
pub fn scan_feature_corpus(root: &Path) -> Vec<(ScenarioId, String, SpecDigest)> {
    scan_feature_corpus_detailed(root)
        .into_iter()
        .map(|s| (s.scenario_id, s.title, s.source_digest))
        .collect()
}
/// One scanned `.feature` scenario before materialization: the paired
/// `(scenario_id, title, source_digest)` plus raw axis/namespace tags
/// the scan lexed for it (s36/s49).
#[derive(Debug, Clone)]
pub struct ScannedScenario {
    /// The `.feature` file the scenario was scanned from.
    pub path: PathBuf,
    pub scenario_id: ScenarioId,
    pub title: String,
    pub source_digest: SpecDigest,
    /// This scenario's own digest (0.15 D1); `None` only when the digest
    /// walk found no block for a tag the scan paired.
    pub scenario_digest: Option<SpecDigest>,
    pub subject_tags: Vec<String>,
    pub lane_tags: Vec<String>,
    pub case_tags: Vec<String>,
    pub unknown_namespace_tags: Vec<String>,
}

/// The full walk both [`scan_feature_corpus`] and [`sync_one_root`]
/// derive from — one place that reads the `.feature` corpus, so the
/// scenario universe and the subject-tag pairing can never drift apart.
pub fn scan_feature_corpus_detailed(root: &Path) -> Vec<ScannedScenario> {
    let mut scanned = Vec::new();
    for path in canon_fmt::util::feature_files(root) {
        let Some(file) = feature_file_scan(&path) else { continue };
        for candidate in &file.scan.scenarios {
            if let Ok(scenario_id) = ScenarioId::parse(&candidate.scenario_id) {
                scanned.push(ScannedScenario {
                    path: path.clone(),
                    scenario_id,
                    title: candidate.title.clone(),
                    source_digest: file.source_digest.clone(),
                    scenario_digest: file.scenario_digests.get(&candidate.scenario_id).cloned(),
                    subject_tags: candidate.subject_tags.clone(),
                    lane_tags: candidate.lane_tags.clone(),
                    case_tags: candidate.case_tags.clone(),
                    unknown_namespace_tags: candidate.unknown_namespace_tags.clone(),
                });
            }
        }
    }
    scanned
}

/// One `.feature` file as the Gherkin scan reads it.
#[derive(Debug)]
pub struct FeatureFileScan {
    pub scan: canon_fmt::gherkin::FeatureScan,
    pub source_digest: SpecDigest,
    pub scenario_digests: BTreeMap<String, SpecDigest>,
}

/// What each `.feature` file scanned to, by path, while its size and
/// modification time are unchanged ([`canon_store::git_tier::FileStamp`]).
/// One command scans the corpus for the staleness check, the refresh,
/// the status, the gate and the brief's flow coverage; an unchanged file
/// is read and lexed once, and an edited or new one is read again.
static FEATURE_MEMO: std::sync::LazyLock<std::sync::Mutex<BTreeMap<PathBuf, MemoEntry>>> =
    std::sync::LazyLock::new(Default::default);

type MemoEntry = (canon_store::git_tier::FileStamp, std::sync::Arc<FeatureFileScan>);

/// The scan of the `.feature` file at `path`; `None` when it cannot be
/// read or is not UTF-8 (the file holds no scenario).
pub fn feature_file_scan(path: &Path) -> Option<std::sync::Arc<FeatureFileScan>> {
    let stamp = std::fs::metadata(path).ok().and_then(|metadata| canon_store::git_tier::FileStamp::of(&metadata));
    if let Some(stamp) = stamp {
        let memo = FEATURE_MEMO.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((_, file)) = memo.get(path).filter(|(seen, _)| *seen == stamp) {
            return Some(std::sync::Arc::clone(file));
        }
    }
    let bytes = std::fs::read(path).ok()?;
    let text = std::str::from_utf8(&bytes).ok()?;
    let file = std::sync::Arc::new(FeatureFileScan {
        scan: canon_fmt::gherkin::scan(text),
        source_digest: canon_fmt::gherkin::source_digest(&bytes),
        scenario_digests: canon_fmt::gherkin::scenario_digests(text),
    });
    if let Some(stamp) = stamp {
        FEATURE_MEMO.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(path.to_path_buf(), (stamp, std::sync::Arc::clone(&file)));
    }
    Some(file)
}

/// Each scenario's current `.feature` file and the directories each spec
/// root's history lives under, relative to `repo`: the root now, then its
/// `former_roots` (where `canon migrate` moved it from). A root outside
/// the repo has no history to read and is left out, and so is a former
/// root outside it; an unusable `specs:` section leaves everything out,
/// so every pre-0.15 review stays undecidable rather than misjudged. Read
/// only by `canon migrate`'s review pinning ([`crate::migrate_pins`]).
pub(crate) fn spec_sources(repo: &Path) -> SpecSources {
    let mut sources = SpecSources::default();
    let Ok(roots) = load_spec_roots_with_former(&repo.join("canon.yaml")) else { return sources };
    for (root, former) in roots {
        let Ok(relative) = root.root.strip_prefix(repo) else { continue };
        let dirs = std::iter::once(relative.to_path_buf()).chain(former.iter().filter_map(|dir| dir.strip_prefix(repo).ok().map(Path::to_path_buf)));
        sources.roots.insert(root.id.clone(), dirs.collect());
        for scanned in scan_feature_corpus_detailed(&root.root) {
            if let Ok(file) = scanned.path.strip_prefix(repo) {
                sources.files.insert((root.id.clone(), scanned.scenario_id), file.to_path_buf());
            }
        }
    }
    sources
}

/// Resolve a scanned scenario's `@subject:` tag(s) into the optional
/// [`SubjectId`] join, FAIL-SOFT (s36 task 6.2). Zero tags → `None`
/// (the untagged common case). A single tag → `Some` when it parses as
/// a kebab-case subject slug; a malformed value pushes ONE counted
/// diagnostic and yields `None` — the scenario is still indexed, just
/// without the join. Two-or-more tags → a counted "first wins"
/// diagnostic, then the FIRST tag is resolved as above. Never aborts;
/// only the join is dropped.
fn resolve_subject_tag(scanned: &ScannedScenario, root_id: &ProjectId, diagnostics: &mut Vec<String>) -> Option<SubjectId> {
    let first = scanned.subject_tags.first()?;
    if scanned.subject_tags.len() > 1 {
        diagnostics.push(format!(
            "scenario `{}` under root `{}` carries {} `@subject:` tags — a scenario joins at most one subject, so the first (`{}`) wins and the rest are ignored",
            scanned.scenario_id.as_str(),
            root_id.as_str(),
            scanned.subject_tags.len(),
            first
        ));
    }
    match SubjectId::parse(first) {
        Ok(id) => Some(id),
        Err(_) => {
            diagnostics.push(format!(
                "scenario `{}` under root `{}` has a malformed `@subject:{}` tag (not a kebab-case subject slug) — indexed without the subject join",
                scanned.scenario_id.as_str(),
                root_id.as_str(),
                first
            ));
            None
        }
    }
}

/// The vocabulary's enums, resolved on first need and once per plan: a
/// root with no `@lane:`/`@case:` tag never loads the vocabulary, and a
/// root with many loads it once, not per tag.
struct VocabEnums<'a> {
    repo: &'a Path,
    enums: std::cell::OnceCell<BTreeMap<String, Vec<String>>>,
}

impl<'a> VocabEnums<'a> {
    fn lazy(repo: &'a Path) -> Self {
        Self { repo, enums: std::cell::OnceCell::new() }
    }

    fn get(&self) -> &BTreeMap<String, Vec<String>> {
        self.enums.get_or_init(|| canon_vocab::resolve_snapshot(self.repo, None).0.enums)
    }
}

/// Resolve one vocabulary-classified axis tag (`@lane:`, `@case:`) into
/// the scenario's optional classification, fail-soft like
/// [`resolve_subject_tag`]. `axis` names both the tag namespace and the
/// vocabulary enum it is checked against. The first tag wins; a
/// malformed shape or a value outside the repo's activated enum drops
/// only that classification, with a counted diagnostic.
fn resolve_axis_tag(axis: &str, tags: &[String], scenario_id: &ScenarioId, root_id: &ProjectId, enums: &VocabEnums, diagnostics: &mut Vec<String>) -> Option<String> {
    let first = tags.first()?;
    if tags.len() > 1 {
        diagnostics.push(format!(
            "scenario `{}` under root `{}` carries {} `@{axis}:` tags — a scenario has at most one {axis}, so the first (`{}`) wins and the rest are ignored",
            scenario_id.as_str(),
            root_id.as_str(),
            tags.len(),
            first
        ));
    }
    if SubjectId::parse(first).is_err() {
        diagnostics.push(format!(
            "scenario `{}` under root `{}` has a malformed `@{axis}:{}` tag (not a kebab-case {axis} slug) — indexed without the {axis}",
            scenario_id.as_str(),
            root_id.as_str(),
            first
        ));
        return None;
    }
    if let Some(detail) = enum_membership_in(enums.get(), axis, first) {
        diagnostics.push(format!(
            "scenario `{}` under root `{}` has an invalid `@{axis}:{}` tag ({detail}) — indexed without the {axis}",
            scenario_id.as_str(),
            root_id.as_str(),
            first
        ));
        return None;
    }
    Some(first.clone())
}

/// Why a root refuses to sync, as `(format violations, sync errors)`;
/// both empty when it syncs. A `canon format` violation anywhere in the
/// root aborts it; so does a `scenario_id` scanned more than once WITHIN
/// its scan (D5: exactly one index record per `(project_id,
/// scenario_id)`, so no winning title/source_digest can be picked).
/// That is a per-root corpus authoring fault, parallel to a format
/// abort: NOT a `FmtFailureClass` (frozen to the 11 audited gaps, never
/// a 12th) and NOT an `InventoryError` (that would short-circuit sibling
/// roots and mis-map to the exit-2 config lane instead of the exit-1
/// root-abort lane). [`sync_one_root`] and [`unrefreshable_roots`] both
/// decide through this one function.
fn root_refusal(spec_root: &SpecRoot, scanned: &[ScannedScenario]) -> (Vec<Violation>, Vec<String>) {
    let report = canon_fmt::check(&spec_root.root);
    if !report.is_clean() {
        return (report.violations, Vec::new());
    }
    let mut seen: BTreeSet<&ScenarioId> = BTreeSet::new();
    let mut dups: BTreeSet<&ScenarioId> = BTreeSet::new();
    for s in scanned {
        if !seen.insert(&s.scenario_id) {
            dups.insert(&s.scenario_id);
        }
    }
    let sync_errors = dups
        .iter()
        .map(|id| {
            format!(
                "scenario_id `{}` is scanned more than once under root `{}` — deduplicate the `.feature` corpus (D5: exactly one index record per (project_id, scenario_id))",
                id.as_str(),
                spec_root.id.as_str()
            )
        })
        .collect();
    (Vec::new(), sync_errors)
}

/// What syncing one root decides: its outcome and the records it would write.
struct RootPlan {
    outcome: RootSyncOutcome,
    records: Vec<Scenario>,
}

/// One configured root's validate → materialize pass over its `scanned`
/// corpus (module doc). A fresh index read per root keeps the fold
/// correct even if two roots were misconfigured to share a
/// `project_id` — every candidate is checked against the LATEST
/// already-committed state, never a snapshot taken before this run
/// started.
///
/// A scenario the index still holds live for this project but
/// `project_corpus` (every id this run's roots under that project
/// declare, [`corpus_ids`]) no longer carries gets a tombstone: a new
/// generation of its latest record with `removed: true`, so every
/// reader stops grading and counting it. A root that aborts writes no
/// tombstone either.
///
/// `only` scopes the MATERIALIZATION of scanned scenarios to those
/// `.feature` files (a reader's stale-index refresh,
/// [`refresh_stale_index`]). Validation, the duplicate check and the
/// tombstones still cover the whole root, so a scoped run refuses and
/// removes exactly what a full `canon inventory sync` would.
///
/// Decide what syncing `spec_root` writes against the index as it stands
/// (`folded`, read only when the root is not refused), without writing.
/// [`sync_one_root`] writes it; [`overlay_stale_index`] only reads it.
fn plan_one_root<'f>(
    folded: impl FnOnce() -> Result<std::borrow::Cow<'f, BTreeMap<(ProjectId, ScenarioId), Scenario>>, InventoryError>,
    repo: &Path,
    spec_root: &SpecRoot,
    scanned: &[ScannedScenario],
    project_corpus: &BTreeSet<ScenarioId>,
    actor: &Actor,
    only: Option<&BTreeSet<PathBuf>>,
) -> Result<RootPlan, InventoryError> {
    let (violations, sync_errors) = root_refusal(spec_root, scanned);
    if !violations.is_empty() || !sync_errors.is_empty() {
        let outcome = RootSyncOutcome {
            id: spec_root.id.clone(),
            root: spec_root.root.clone(),
            scanned: if violations.is_empty() { scanned.len() } else { 0 },
            written: 0,
            removed: 0,
            violations,
            sync_errors,
            tag_diagnostics: Vec::new(),
        };
        return Ok(RootPlan { outcome, records: Vec::new() });
    }

    let folded = folded()?;

    let mut records = Vec::new();
    // Fail-soft tag diagnostics accumulate across the whole root but
    // never abort it (see `RootSyncOutcome::tag_diagnostics`).
    let mut tag_diagnostics = Vec::new();
    let enums = VocabEnums::lazy(repo);
    for candidate in scanned.iter().filter(|candidate| only.is_none_or(|only| only.contains(&candidate.path))) {
        // Resolve BEFORE the idempotence check so a persistently
        // malformed/duplicate tag is reported on EVERY sync run, not
        // just the write that first introduced it.
        let subject_id = resolve_subject_tag(candidate, &spec_root.id, &mut tag_diagnostics);
        let lane = resolve_axis_tag("lane", &candidate.lane_tags, &candidate.scenario_id, &spec_root.id, &enums, &mut tag_diagnostics);
        let case = resolve_axis_tag("case", &candidate.case_tags, &candidate.scenario_id, &spec_root.id, &enums, &mut tag_diagnostics);
        let mut namespaces = std::collections::BTreeSet::new();
        for namespace in &candidate.unknown_namespace_tags {
            namespaces.insert(namespace);
        }
        for namespace in namespaces {
            tag_diagnostics.push(format!(
                "scenario `{}` under root `{}` carries an unrecognized `@{}:` tag namespace — canon ignores it; use a recognized axis or a plain runner tag",
                candidate.scenario_id.as_str(),
                spec_root.id.as_str(),
                namespace
            ));
        }
        let key = (spec_root.id.clone(), candidate.scenario_id.clone());
        // The subject join joins the idempotence key: a record written
        // before this field existed (or before the tag was added) gets
        // rewritten to carry the join, even if title/digest match.
        let unchanged = folded.get(&key).is_some_and(|latest| {
            !latest.removed
                && latest.source_digest == candidate.source_digest
                && latest.scenario_digest == candidate.scenario_digest
                && latest.title == candidate.title
                && latest.subject_id == subject_id
                && latest.lane.as_deref() == lane.as_deref()
                && latest.case.as_deref() == case.as_deref()
        });
        if unchanged {
            continue;
        }
        let mut record = Scenario::new(
            Envelope::new(1, RecordKind::Scenario, Utc::now(), actor.clone()),
            spec_root.id.clone(),
            candidate.scenario_id.clone(),
            candidate.title.clone(),
            "",
            candidate.source_digest.clone(),
        );
        record.scenario_digest = candidate.scenario_digest.clone();
        record.lane = lane;
        record.case = case;
        record.subject_id = subject_id;
        records.push(record);
    }

    let mut removed = 0;
    for gone in removed_from_corpus(&folded, &spec_root.id, project_corpus) {
        let mut tombstone = gone.clone();
        tombstone.envelope = Envelope::new(1, RecordKind::Scenario, Utc::now(), actor.clone());
        tombstone.removed = true;
        records.push(tombstone);
        removed += 1;
    }

    let outcome = RootSyncOutcome {
        id: spec_root.id.clone(),
        root: spec_root.root.clone(),
        scanned: scanned.len(),
        written: records.len(),
        removed,
        violations: Vec::new(),
        sync_errors: Vec::new(),
        tag_diagnostics,
    };
    Ok(RootPlan { outcome, records })
}

fn sync_one_root(
    tier: &GitTier,
    repo: &Path,
    spec_root: &SpecRoot,
    scanned: &[ScannedScenario],
    project_corpus: &BTreeSet<ScenarioId>,
    actor: &Actor,
    only: Option<&BTreeSet<PathBuf>>,
) -> Result<RootSyncOutcome, InventoryError> {
    let plan = plan_one_root(|| latest_index(tier).map(std::borrow::Cow::Owned), repo, spec_root, scanned, project_corpus, actor, only)?;
    for record in &plan.records {
        tier.write(record)?;
    }
    Ok(plan.outcome)
}

/// Lay the scenario index a refresh would write over `ctx`, in memory:
/// what a reader sees after [`refresh_stale_index`], for a command that
/// must leave the ledger as it found it (`canon scenario new` writes only
/// a `.feature` file, and its `meta.next` is read from the specs it just
/// wrote). A root the refresh would refuse is left as it is: the readers
/// fail closed on it ([`unrefreshable_roots`]).
pub fn overlay_stale_index(ctx: &mut GateContext) {
    let repo = ctx.ctx.repo.clone();
    let Ok(sync) = SyncCtx::from_repo(&repo) else { return };
    let Ok(roots) = sync.spec_roots(None) else { return };
    let (_, roots) = split_pre_0_17(&sync.repo, roots);
    let tier = GitTier::new(&sync.ledger_root);
    let Ok(folded) = latest_index(&tier) else { return };
    let (stale, corpus) = stale_roots_against(&folded, &roots);
    let actor = Actor::new_unattributed("canon-inventory-sync");
    let mut written: Vec<Scenario> = Vec::new();
    for StaleRoot { root, scanned, files, .. } in &stale {
        let Ok(plan) = plan_one_root(|| Ok(std::borrow::Cow::Borrowed(&folded)), &repo, root, scanned, &corpus[&root.id], &actor, Some(files)) else { continue };
        if !plan.outcome.is_clean() {
            continue;
        }
        written.extend(plan.records);
    }
    // One pass: a plan holds one record per `(project_id, scenario_id)`
    // (D5), so replacing every replaced key at once equals replacing them
    // one record at a time, and the pushes keep the plans' order.
    let replaced: BTreeSet<(&ProjectId, &ScenarioId)> = written.iter().map(|r| (&r.project_id, &r.scenario_id)).collect();
    ctx.scenarios.retain(|s| !replaced.contains(&(&s.project_id, &s.scenario_id)));
    ctx.scenarios.extend(written.into_iter().filter(|r| !r.removed));
    // The overlay normally runs before any derived read; invalidating keeps
    // that an optimisation, not a precondition (0.29 D2).
    ctx.invalidate_derived();
}

/// `canon inventory sync`'s stdout — one block per configured root.
pub fn format_human(outcome: &SyncOutcome) -> String {
    let mut out = String::new();
    for root in &outcome.roots {
        if root.is_clean() {
            let removed = if root.removed > 0 { format!(" ({} marking scenario(s) no longer in any feature file as removed)", root.removed) } else { String::new() };
            let verb = if outcome.dry_run { "would be written" } else { "written" };
            out.push_str(&format!(
                "canon inventory sync: root `{}` ({}) — {} scenario(s) scanned, {} record(s) {verb}{removed}\n",
                root.id,
                root.root.display(),
                root.scanned,
                root.written
            ));
        } else {
            out.push_str(&format!(
                "canon inventory sync: root `{}` ({}) — ABORTED, {} violation(s), {} sync error(s), 0 record(s) written\n",
                root.id,
                root.root.display(),
                root.violations.len(),
                root.sync_errors.len()
            ));
            for v in &root.violations {
                out.push_str(&format!("  [{}] {} — {}\n", v.class.as_str(), v.path.display(), v.detail));
            }
            for e in &root.sync_errors {
                out.push_str(&format!("  [duplicate-scenario] {e}\n"));
            }
        }
        // Fail-soft tag diagnostics are printed for any root that reached
        // materialization, whether or not it also wrote records.
        for d in &root.tag_diagnostics {
            out.push_str(&format!("  [tag] {d}\n"));
        }
    }
    out
}

/// One spec root of `canon inventory sync`'s report.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InventoryRootReport<'a> {
    pub id: &'a str,
    pub root: String,
    pub clean: bool,
    pub scanned: usize,
    pub written: usize,
    pub removed: usize,
    /// The format violations that aborted the root.
    pub violations: Vec<crate::fmt::ViolationEntry<'a>>,
    pub sync_errors: Vec<String>,
    pub tag_diagnostics: Vec<String>,
}

/// `canon inventory sync`'s report (`InventorySyncReport`).
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InventorySyncReport<'a> {
    /// True when `--dry-run` previewed the run without writing.
    pub dry_run: bool,
    pub clean: bool,
    pub total_written: usize,
    pub roots: Vec<InventoryRootReport<'a>>,
}

impl crate::json_out::Report for InventorySyncReport<'_> {}

/// The `--json` data for [`SyncOutcome`]: one entry per root, with the
/// violations and sync errors that aborted it.
pub fn format_json(outcome: &SyncOutcome) -> InventorySyncReport<'_> {
    let roots = outcome
        .roots
        .iter()
        .map(|root| InventoryRootReport {
            id: root.id.as_str(),
            root: root.root.display().to_string(),
            clean: root.is_clean(),
            scanned: root.scanned,
            written: root.written,
            removed: root.removed,
            violations: root.violations.iter().map(crate::fmt::ViolationEntry::of).collect(),
            sync_errors: root.sync_errors.clone(),
            tag_diagnostics: root.tag_diagnostics.clone(),
        })
        .collect();
    InventorySyncReport { dry_run: outcome.dry_run, clean: outcome.is_clean(), total_written: outcome.total_written(), roots }
}

/// One root's stale part of the scenario index ([`stale_roots`]).
struct StaleRoot {
    root: SpecRoot,
    /// The root's scan, reused by the sync that refreshes it.
    scanned: Vec<ScannedScenario>,
    /// Every `.feature` file holding a scenario whose latest `Scenario`
    /// generation is missing, removed, or carries another
    /// `source_digest` or `scenario_digest` than the file has now. A
    /// generation written before 0.15 has no `scenario_digest`, so its
    /// file is stale even though its bytes are unchanged: the sync
    /// rewrites it, as [`sync_one_root`] would. A file with no scenario
    /// has nothing to index and is never stale.
    files: BTreeSet<PathBuf>,
    /// Scenarios the index still holds live for this root's project
    /// that no `.feature` file declares any more — what a full sync
    /// marks removed ([`removed_from_corpus`]).
    removed: Vec<ScenarioId>,
}

/// Every root whose index differs from what `canon inventory sync` would
/// leave, with the per-project corpus ids ([`corpus_ids`]) that was
/// judged against. Two differences count: a scanned scenario the index
/// lacks or holds at another `source_digest` or `scenario_digest`, and
/// an indexed scenario the corpus no longer declares — the indexed id
/// set and the corpus id set of a project must be equal.
fn stale_roots(tier: &GitTier, roots: &[SpecRoot]) -> Result<StaleScan, InventoryError> {
    Ok(stale_roots_against(&latest_index(tier)?, roots))
}

type StaleScan = (Vec<StaleRoot>, BTreeMap<ProjectId, BTreeSet<ScenarioId>>);

/// [`stale_roots`] against an index already folded ([`latest_index`]).
fn stale_roots_against(latest: &BTreeMap<(ProjectId, ScenarioId), Scenario>, roots: &[SpecRoot]) -> (Vec<StaleRoot>, BTreeMap<ProjectId, BTreeSet<ScenarioId>>) {
    let scans: Vec<Vec<ScannedScenario>> = roots.iter().map(|root| scan_feature_corpus_detailed(&root.root)).collect();
    let corpus = corpus_ids(roots, &scans);

    let mut stale = Vec::new();
    for (root, scanned) in roots.iter().zip(scans) {
        let files: BTreeSet<PathBuf> = scanned
            .iter()
            .filter(|candidate| {
                !latest
                    .get(&(root.id.clone(), candidate.scenario_id.clone()))
                    .is_some_and(|record| !record.removed && record.source_digest == candidate.source_digest && record.scenario_digest == candidate.scenario_digest)
            })
            .map(|candidate| candidate.path.clone())
            .collect();
        let removed: Vec<ScenarioId> = removed_from_corpus(latest, &root.id, &corpus[&root.id]).into_iter().map(|scenario| scenario.scenario_id.clone()).collect();
        if !files.is_empty() || !removed.is_empty() {
            stale.push(StaleRoot { root: root.clone(), scanned, files, removed });
        }
    }
    (stale, corpus)
}

/// At most three of `items`, then how many more.
fn first_three(items: &[String]) -> String {
    match items.len() {
        0..=3 => items.join(", "),
        n => format!("{}, +{} more", items[..3].join(", "), n - 3),
    }
}

/// A writer's stale-index refresh (0.16 D2): `canon subject move`,
/// `canon review add`, `canon evidence add` and the other commands that
/// record something call this before they read the scenario index, so an
/// edited, added or deleted scenario never needs a separate `canon
/// inventory sync` step. The read commands (`canon gate check`, `canon
/// status`) do not call it: a `read` command writes nothing (0.27), so
/// they grade the index as this refresh would leave it, in memory
/// ([`overlay_stale_index`]).
///
/// It runs the same materialization `canon inventory sync` runs
/// ([`sync_one_root`]), scoped to the changed files, plus the same
/// tombstones for scenarios no feature file declares any more, and says
/// what it did in ONE stderr line prefixed with `command` (stderr, so a
/// `--json` stdout stays parseable). Silent when nothing is stale. Never
/// writes into a directory without `canon.yaml`, and never fails the
/// caller: a config error is the caller's own to report, and a root
/// that refuses to sync (format violations, a duplicate id) is named
/// with the command that shows why. The commands that grade or advance
/// (`gate check`, the ship guard, `status`) then fail closed on
/// [`unrefreshable_roots`].
///
/// The staleness check runs without a lock, so a fresh index costs no
/// waiting. A stale one is rewritten under the repo's scenario-index
/// lock ([`lock_index`]), shared with `canon inventory sync`, and
/// staleness is checked again once the lock is held: a refresh that
/// waited for another writer finds its work done and stays silent
/// instead of indexing the same change twice.
///
/// A root still on the pre-0.17 layout is not refreshed at all: one
/// stderr line names it and `canon migrate` ([`pre_0_17_detail`]), and
/// the readers that grade fail closed on it ([`unrefreshable_roots`]).
///
/// The index is derived from the `.feature` files, so rebuilding it is
/// not the kind of write a read-only command must avoid: it changes no
/// authored record, and grading a stale index would answer about specs
/// that no longer exist.
pub fn refresh_stale_index(repo: &Path, command: &str) {
    let repo = resolve_repo_root(repo);
    if !repo.join("canon.yaml").is_file() {
        return;
    }
    let Ok(ctx) = SyncCtx::from_repo(&repo) else { return };
    let Ok(roots) = ctx.spec_roots(None) else { return };
    let (old, roots) = split_pre_0_17(&ctx.repo, roots);
    for (_, detail) in &old {
        crate::warn_err!("{command}: {detail}; its scenario index is NOT refreshed");
    }
    let tier = GitTier::new(&ctx.ledger_root);
    let checked = |tier: &GitTier| match stale_roots(tier, &roots) {
        Ok(stale) => Some(stale),
        Err(e) => {
            crate::warn_err!("{command}: could not check the scenario index for staleness: {e}");
            None
        }
    };
    match checked(&tier) {
        Some((stale, _)) if !stale.is_empty() => {}
        _ => return,
    }
    let _lock = match lock_index(&ctx.repo) {
        Ok(lock) => lock,
        Err(e) => {
            crate::warn_err!("{command}: the scenario index is stale but was not refreshed: {e}");
            return;
        }
    };
    let Some((stale, corpus)) = checked(&tier) else { return };
    if stale.is_empty() {
        return;
    }

    let files: Vec<String> = stale
        .iter()
        .flat_map(|stale| stale.files.iter())
        .map(|path| path.strip_prefix(&ctx.repo).unwrap_or(path).display().to_string())
        .collect();
    let removed: Vec<String> =
        stale.iter().flat_map(|stale| stale.removed.iter().map(|id| id.as_str().to_string())).collect::<BTreeSet<_>>().into_iter().collect();

    let actor = Actor::new_unattributed("canon-inventory-sync");
    let (mut written, mut diagnostics, mut refused) = (0usize, 0usize, Vec::new());
    for StaleRoot { root, scanned, files, .. } in &stale {
        match sync_one_root(&tier, &ctx.repo, root, scanned, &corpus[&root.id], &actor, Some(files)) {
            Ok(outcome) if outcome.is_clean() => {
                written += outcome.written;
                diagnostics += outcome.tag_diagnostics.len();
            }
            Ok(outcome) => refused.push(format!("root `{}` ({} format violation(s), {} sync error(s))", outcome.id, outcome.violations.len(), outcome.sync_errors.len())),
            Err(e) => refused.push(format!("root `{}` ({e})", root.id)),
        }
    }

    let mut what = Vec::new();
    if !files.is_empty() {
        what.push(format!("{} changed feature file(s) ({})", files.len(), first_three(&files)));
    }
    if !removed.is_empty() {
        what.push(format!("{} scenario(s) no longer in any feature file ({})", removed.len(), first_three(&removed)));
    }
    let verb = if refused.len() == stale.len() { "found the scenario index stale for" } else { "refreshed the scenario index from" };
    let mut line = format!("{command}: {verb} {}: {written} record(s) written", what.join(" and "));
    if diagnostics > 0 {
        line.push_str(&format!(", {diagnostics} tag diagnostic(s)"));
    }
    if !refused.is_empty() {
        line.push_str(&format!("; NOT refreshed: {}", refused.join(", ")));
    }
    if diagnostics > 0 || !refused.is_empty() {
        line.push_str(" — `canon inventory sync` lists them");
    }
    crate::warn_err!("{line}");
}

/// A configured spec root every grading reader must refuse: its scenario
/// index is stale and cannot be refreshed (the `NOT refreshed` case of
/// [`refresh_stale_index`]), because its `.feature` files changed but
/// the root has format violations or a duplicate id, or because the
/// root is still on the pre-0.17 layout (`pre_0_17`), which is never
/// read. Either way every reader would be reading an index the corpus
/// is not in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnrefreshableRoot {
    pub id: ProjectId,
    pub violations: usize,
    pub sync_errors: usize,
    /// Why the root is refused for its layout ([`pre_0_17_detail`]):
    /// `Some` for a root still holding `.feature` files under the
    /// pre-0.17 `features/` directory, whatever the index holds.
    pub pre_0_17: Option<String>,
}

impl UnrefreshableRoot {
    /// The gate's violation for this root (0.16 M6, fail closed): an
    /// `uncovered-cell` naming the root, the class the ship gate already
    /// uses for "this subject's scenarios cannot be counted" (`scenario`
    /// routed away), because the root's scenarios cannot be graded at
    /// all. `prefix` is prepended to the detail (the transition, for the
    /// ship guard).
    pub fn violation(&self, prefix: &str) -> canon_gate::Violation {
        let detail = match &self.pre_0_17 {
            Some(detail) => format!("{prefix}{detail}; until then its scenarios are not graded"),
            None => format!(
                "{prefix}the scenario index for spec root `{}` is stale and could not be refreshed ({} format violation(s), {} sync error(s)), so its changed .feature files are not graded; `canon inventory sync` lists the problems to fix",
                self.id.as_str(),
                self.violations,
                self.sync_errors
            ),
        };
        canon_gate::Violation::new(canon_gate::FailureClass::UncoveredCell, self.id.as_str(), detail)
    }
}

/// Why `root` is refused for its layout, or `None` when it holds no
/// `.feature` file under the pre-0.17 `features/` directory (0.17 D1,
/// [`canon_fmt::util::pre_0_17_feature_file`]). Names the root by id and
/// repo-relative path, and `canon migrate`.
pub fn pre_0_17_detail(repo: &Path, root: &SpecRoot) -> Option<String> {
    let first = canon_fmt::util::pre_0_17_feature_file(&root.root)?;
    let shown = root.root.strip_prefix(repo).unwrap_or(&root.root);
    Some(canon_fmt::util::pre_0_17_layout_detail(&format!("`{}` (`{}`)", root.id.as_str(), shown.display()), &first))
}

/// `roots` split into those on the pre-0.17 layout (with
/// [`pre_0_17_detail`]) and the rest, each in configured order.
fn split_pre_0_17(repo: &Path, roots: Vec<SpecRoot>) -> (Vec<(SpecRoot, String)>, Vec<SpecRoot>) {
    let mut old = Vec::new();
    let mut current = Vec::new();
    for root in roots {
        match pre_0_17_detail(repo, &root) {
            Some(detail) => old.push((root, detail)),
            None => current.push(root),
        }
    }
    (old, current)
}

/// Every configured spec root a grading reader must refuse
/// ([`UnrefreshableRoot`]): the ONE decision `canon gate check`, the
/// `verifying → shipped` guard of `canon subject move`, and `canon
/// status` fail closed on (0.16 M6, 0.17 D1), so none of them grades or
/// advances on an index the `.feature` corpus is not in. Readers that
/// only report (`query`, `context`) keep the stderr line of
/// [`refresh_stale_index`] instead. It reads the same layout check, the
/// same staleness and the same refusal ([`root_refusal`]) the refresh
/// acts on, and writes nothing. A pre-0.17 root is listed whatever the
/// index holds. Otherwise empty without `canon.yaml`, or when the config
/// or the index cannot be read: the caller reports those itself.
pub fn unrefreshable_roots(repo: &Path) -> Vec<UnrefreshableRoot> {
    let repo = resolve_repo_root(repo);
    if !repo.join("canon.yaml").is_file() {
        return Vec::new();
    }
    let Ok(ctx) = SyncCtx::from_repo(&repo) else { return Vec::new() };
    let Ok(roots) = ctx.spec_roots(None) else { return Vec::new() };
    let (old, roots) = split_pre_0_17(&ctx.repo, roots);
    let mut refused: Vec<UnrefreshableRoot> =
        old.into_iter().map(|(root, detail)| UnrefreshableRoot { id: root.id, violations: 0, sync_errors: 0, pre_0_17: Some(detail) }).collect();
    let Ok((stale, _)) = stale_roots(&GitTier::new(&ctx.ledger_root), &roots) else { return refused };
    refused.extend(stale.iter().filter_map(|stale| {
        let (violations, sync_errors) = root_refusal(&stale.root, &stale.scanned);
        (!violations.is_empty() || !sync_errors.is_empty())
            .then(|| UnrefreshableRoot { id: stale.root.id.clone(), violations: violations.len(), sync_errors: sync_errors.len(), pre_0_17: None })
    }));
    refused
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn write(dir: &Path, rel: &str, content: &str) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    fn provenance_comment() -> String {
        "  # canon: {\"schema\":1,\"at\":\"2026-07-10T00:00:00Z\",\"actor\":{\"agent_id\":\"canon-fmt\"}}".to_string()
    }

    /// A single well-formed `.feature` file under `<root>/kind=feature/`,
    /// with a provenance comment on both headers (so `canon-fmt::check`
    /// reports zero violations) and one scenario tag/header pair.
    fn write_clean_feature(root: &Path, area: &str, surface: &str, nn: &str, title: &str) {
        let text = format!(
            "Feature: {area} {surface}\n{prov}\n\n  @{area}.{surface}.{nn}\n  Scenario: {title}\n{prov}\n    Given a precondition\n",
            prov = provenance_comment()
        );
        write(root, &format!("kind=feature/area={area}/{surface}.feature"), &text);
    }

    /// Like [`write_clean_feature`] but appends `extra_tags` (e.g.
    /// `@subject:payments-core`) to the scenario's tag line (s36 task
    /// 6.2).
    fn write_feature_with_tags(root: &Path, area: &str, surface: &str, nn: &str, title: &str, extra_tags: &str) {
        let text = format!(
            "Feature: {area} {surface}\n{prov}\n\n  @{area}.{surface}.{nn} {extra_tags}\n  Scenario: {title}\n{prov}\n    Given a precondition\n",
            prov = provenance_comment()
        );
        write(root, &format!("kind=feature/area={area}/{surface}.feature"), &text);
    }

    /// A `.feature` file scanned twice in one process is read again when
    /// it changed, even under a settled timestamp.
    #[test]
    fn a_repeated_feature_scan_sees_an_edited_file() {
        let dir = TempDir::new().unwrap();
        write_clean_feature(dir.path(), "game", "art", "01", "First title");
        let path = dir.path().join("kind=feature/area=game/art.feature");
        let age = |secs: u64| std::fs::OpenOptions::new().write(true).open(&path).unwrap().set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(secs)).unwrap();
        age(3600);
        let before = scan_feature_corpus_detailed(dir.path());
        assert_eq!(before.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(), ["First title"]);

        write_clean_feature(dir.path(), "game", "art", "01", "Other title");
        age(3595);
        let after = scan_feature_corpus_detailed(dir.path());
        assert_eq!(after.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(), ["Other title"]);
    }

    fn only_scenario(repo: &Path) -> Scenario {
        let gate_ctx = GateCtx::from_repo(repo).unwrap();
        let tier = GitTier::new(&gate_ctx.ledger_root);
        let result = tier.read(&TierQuery::kind(RecordKind::Scenario)).unwrap();
        assert_eq!(result.records.len(), 1, "expected exactly one Scenario record");
        serde_json::from_value(result.records[0].0.clone()).unwrap()
    }

    fn record_json(repo: &Path) -> serde_json::Value {
        let gate_ctx = GateCtx::from_repo(repo).unwrap();
        let tier = GitTier::new(&gate_ctx.ledger_root);
        let result = tier.read(&TierQuery::kind(RecordKind::Scenario)).unwrap();
        assert_eq!(result.records.len(), 1, "expected exactly one Scenario record");
        result.records[0].0.clone()
    }

    // ---- specs.roots[] config resolution (task 3.1) ----

    #[test]
    fn missing_specs_key_resolves_the_default_single_root() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), "canon.yaml", "tiers:\n  local:\n    backend: git\n    root: .canon/ledger\n");
        let roots = load_spec_roots(&dir.path().join("canon.yaml")).unwrap();
        assert_eq!(roots, vec![SpecRoot { id: ProjectId::parse("root").unwrap(), root: dir.path().join(paths::SPECS_DIR) }]);
    }

    #[test]
    fn missing_canon_yaml_also_resolves_the_default_single_root() {
        let dir = TempDir::new().unwrap();
        let roots = load_spec_roots(&dir.path().join("canon.yaml")).unwrap();
        assert_eq!(roots, vec![SpecRoot { id: ProjectId::parse("root").unwrap(), root: dir.path().join(paths::SPECS_DIR) }]);
    }

    #[test]
    fn a_malformed_specs_roots_entry_fails_loud_never_defaults() {
        let dir = TempDir::new().unwrap();
        // A `roots` entry missing its required `id` field.
        write(dir.path(), "canon.yaml", "specs:\n  roots:\n    - root: apps/a/specs\n");
        let err = load_spec_roots(&dir.path().join("canon.yaml")).unwrap_err();
        assert!(matches!(err, InventoryError::Config(_)));
    }

    #[test]
    fn roots_not_a_list_fails_loud() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), "canon.yaml", "specs:\n  roots: not-a-list\n");
        let err = load_spec_roots(&dir.path().join("canon.yaml")).unwrap_err();
        assert!(matches!(err, InventoryError::Config(_)));
    }

    #[test]
    fn a_present_but_empty_specs_section_fails_loud_never_defaults() {
        // `specs: {}` (or `specs:\n  roots: []`) is a present-but-
        // incomplete config, NOT the absent-key default — it must fail
        // loud rather than silently syncing zero roots at exit 0.
        for yaml in ["specs: {}\n", "specs:\n  roots: []\n"] {
            let dir = TempDir::new().unwrap();
            write(dir.path(), "canon.yaml", yaml);
            let err = load_spec_roots(&dir.path().join("canon.yaml")).unwrap_err();
            assert!(matches!(err, InventoryError::Config(_)), "present-but-empty `specs:` (`{yaml}`) must fail loud, not default");
        }
    }

    #[test]
    fn a_monorepo_declares_multiple_named_roots() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), "canon.yaml", "specs:\n  roots:\n    - id: app-a\n      root: apps/a/specs\n    - id: app-b\n      root: apps/b/specs\n");
        let roots = load_spec_roots(&dir.path().join("canon.yaml")).unwrap();
        assert_eq!(
            roots,
            vec![
                SpecRoot { id: ProjectId::parse("app-a").unwrap(), root: dir.path().join("apps/a/specs") },
                SpecRoot { id: ProjectId::parse("app-b").unwrap(), root: dir.path().join("apps/b/specs") },
            ]
        );
    }

    #[test]
    fn per_entry_root_defaults_to_specs_when_omitted() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), "canon.yaml", "specs:\n  roots:\n    - id: app-a\n");
        let roots = load_spec_roots(&dir.path().join("canon.yaml")).unwrap();
        assert_eq!(roots, vec![SpecRoot { id: ProjectId::parse("app-a").unwrap(), root: dir.path().join(paths::SPECS_DIR) }]);
    }

    #[test]
    fn id_is_never_derived_from_the_checkout_directory() {
        let clone_one = TempDir::new().unwrap();
        let clone_two = TempDir::new().unwrap();
        let yaml = "specs:\n  roots:\n    - id: app-a\n      root: specs\n";
        write(clone_one.path(), "canon.yaml", yaml);
        write(clone_two.path(), "canon.yaml", yaml);

        let roots_one = load_spec_roots(&clone_one.path().join("canon.yaml")).unwrap();
        let roots_two = load_spec_roots(&clone_two.path().join("canon.yaml")).unwrap();
        assert_eq!(roots_one[0].id, roots_two[0].id, "the SAME canon.yaml content must resolve the SAME id regardless of checkout path");
        assert_ne!(roots_one[0].root, roots_two[0].root, "only the resolved directory differs across checkouts, never the id");
    }

    // ---- sync (tasks 3.4/3.5) ----

    #[test]
    fn a_clean_root_materializes_one_scenario_per_scenario_id_with_correct_digest_and_title() {
        let repo = TempDir::new().unwrap();
        write_clean_feature(&repo.path().join(paths::SPECS_DIR), "world", "hotdeal", "01", "Opening the hotdeal overlay");

        let outcome = run_sync(repo.path(), None).unwrap();
        assert!(outcome.is_clean());
        assert_eq!(outcome.total_written(), 1);

        let gate_ctx = GateCtx::from_repo(repo.path()).unwrap();
        let tier = GitTier::new(&gate_ctx.ledger_root);
        let result = tier.read(&TierQuery::kind(RecordKind::Scenario)).unwrap();
        assert_eq!(result.records.len(), 1);
        let record: Scenario = serde_json::from_value(result.records[0].0.clone()).unwrap();
        assert_eq!(record.project_id, ProjectId::parse("root").unwrap());
        assert_eq!(record.scenario_id, ScenarioId::parse("world.hotdeal.01").unwrap());
        assert_eq!(record.title, "Opening the hotdeal overlay");
        let bytes = std::fs::read(repo.path().join(".canon/specs/kind=feature/area=world/hotdeal.feature")).unwrap();
        assert_eq!(record.source_digest, SpecDigest::of(&bytes));
    }

    #[test]
    fn a_validation_violation_aborts_the_whole_root_zero_writes() {
        let repo = TempDir::new().unwrap();
        // No provenance comment on either header -> MissingProvenance
        // violation (S11 `canon-fmt::check`).
        write(&repo.path().join(paths::SPECS_DIR), "kind=feature/world/hotdeal.feature", "Feature: world hotdeal\n\n  @world.hotdeal.01\n  Scenario: Opening the hotdeal overlay\n    Given a precondition\n");

        let outcome = run_sync(repo.path(), None).unwrap();
        assert!(!outcome.is_clean());
        assert_eq!(outcome.total_written(), 0);
        assert!(!outcome.roots[0].violations.is_empty());

        let gate_ctx = GateCtx::from_repo(repo.path()).unwrap();
        let tier = GitTier::new(&gate_ctx.ledger_root);
        let result = tier.read(&TierQuery::kind(RecordKind::Scenario)).unwrap();
        assert!(result.records.is_empty(), "an aborted root must write zero Scenario records");
    }

    /// Write `<repo>/canon.yaml` naming one root `root: specs`, with a
    /// clean feature under its pre-0.17 `features/kind=feature/` dir.
    fn pre_0_17_repo() -> TempDir {
        let repo = TempDir::new().unwrap();
        write(repo.path(), "canon.yaml", "specs:\n  roots:\n    - id: app\n      root: specs\n");
        write_clean_feature(&repo.path().join("specs/features"), "world", "hotdeal", "01", "Opening the hotdeal overlay");
        repo
    }

    #[test]
    fn a_pre_0_17_root_aborts_sync_naming_the_root_and_canon_migrate() {
        let repo = pre_0_17_repo();
        let outcome = run_sync(repo.path(), None).unwrap();
        assert!(!outcome.is_clean());
        assert_eq!(outcome.total_written(), 0);
        let violations = &outcome.roots[0].violations;
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert_eq!(violations[0].class, canon_fmt::FmtFailureClass::LayoutGrammar);
        assert!(violations[0].detail.contains("specs`") && violations[0].detail.contains("`canon migrate`"), "{}", violations[0].detail);
    }

    #[test]
    fn a_pre_0_17_root_is_unrefreshable_even_with_an_empty_index() {
        let repo = pre_0_17_repo();
        let refused = unrefreshable_roots(repo.path());
        assert_eq!(refused.len(), 1, "{refused:?}");
        let detail = refused[0].pre_0_17.as_deref().expect("refused for its layout");
        assert!(detail.starts_with("spec root `app` (`specs`) is on the pre-0.17 layout"), "{detail}");
        let violation = refused[0].violation("");
        assert_eq!(violation.class, canon_gate::FailureClass::UncoveredCell);
        assert!(violation.detail.contains("`canon migrate`"), "{}", violation.detail);
    }

    #[test]
    fn a_root_moved_back_to_the_pre_0_17_layout_is_never_tombstoned_by_a_refresh() {
        let repo = TempDir::new().unwrap();
        write(repo.path(), "canon.yaml", "specs:\n  roots:\n    - id: app\n      root: specs\n");
        write_clean_feature(&repo.path().join("specs"), "world", "hotdeal", "01", "Opening the hotdeal overlay");
        assert_eq!(run_sync(repo.path(), None).unwrap().total_written(), 1);
        std::fs::create_dir_all(repo.path().join("specs/features")).unwrap();
        std::fs::rename(repo.path().join("specs/kind=feature"), repo.path().join("specs/features/kind=feature")).unwrap();

        refresh_stale_index(repo.path(), "canon test");
        let tier = GitTier::new(&GateCtx::from_repo(repo.path()).unwrap().ledger_root);
        let latest = latest_index(&tier).unwrap();
        assert_eq!(latest.len(), 1);
        assert!(latest.values().all(|scenario| !scenario.removed), "a refused root writes no tombstone");
    }

    #[test]
    fn resync_of_an_unchanged_corpus_writes_zero_new_records() {
        let repo = TempDir::new().unwrap();
        write_clean_feature(&repo.path().join(paths::SPECS_DIR), "world", "hotdeal", "01", "Opening the hotdeal overlay");

        let first = run_sync(repo.path(), None).unwrap();
        assert_eq!(first.total_written(), 1);
        let second = run_sync(repo.path(), None).unwrap();
        assert_eq!(second.total_written(), 0, "an unchanged corpus must re-sync as a no-op");

        let gate_ctx = GateCtx::from_repo(repo.path()).unwrap();
        let tier = GitTier::new(&gate_ctx.ledger_root);
        let result = tier.read(&TierQuery::kind(RecordKind::Scenario)).unwrap();
        assert_eq!(result.records.len(), 1, "still exactly one record after two syncs");
    }

    #[test]
    fn a_changed_feature_file_produces_exactly_one_new_record() {
        let repo = TempDir::new().unwrap();
        write_clean_feature(&repo.path().join(paths::SPECS_DIR), "world", "hotdeal", "01", "Opening the hotdeal overlay");
        run_sync(repo.path(), None).unwrap();

        write_clean_feature(&repo.path().join(paths::SPECS_DIR), "world", "hotdeal", "01", "Opening the hotdeal overlay v2");
        let second = run_sync(repo.path(), None).unwrap();
        assert_eq!(second.total_written(), 1, "a changed .feature file must append exactly one new record");

        let gate_ctx = GateCtx::from_repo(repo.path()).unwrap();
        let tier = GitTier::new(&gate_ctx.ledger_root);
        let result = tier.read(&TierQuery::kind(RecordKind::Scenario)).unwrap();
        assert_eq!(result.records.len(), 1, "the ledger holds one record per scenario: the new generation");
        assert_eq!(tier.read_history(RecordKind::Scenario).unwrap().records.len(), 2, "the prior record is moved to the archive alongside, never overwritten");
    }

    #[test]
    fn sync_derives_the_index_from_features_alone_ignoring_any_inventory_dir() {
        let repo = TempDir::new().unwrap();
        write_clean_feature(&repo.path().join(paths::SPECS_DIR), "world", "hotdeal", "01", "Opening the hotdeal overlay");
        // A present, WELL-FORMED `inventory/` dir carrying real
        // `covered_by` data -- sync must never read this to populate
        // the index.
        write(
            &repo.path().join(paths::SPECS_DIR),
            "inventory/kind=inventory/area=world/surface=hub/hub.yaml",
            "schema: 1\nkind: inventory\nat: \"2026-07-10T00:00:00Z\"\nactor:\n  agent_id: canon-fmt-test\nworld.hub.hub-header:\n  upstream:\n    pin: 9c93d024b\n    file: routes/world/hub/index.tsx\n    symbol: RouteComponent\n    lines: \"1-10\"\n  covered_by: [world.hotdeal.01]\n",
        );

        let outcome = run_sync(repo.path(), None).unwrap();
        assert!(outcome.is_clean());
        assert_eq!(outcome.total_written(), 1, "the inventory/ file must not itself produce a Scenario record");

        let gate_ctx = GateCtx::from_repo(repo.path()).unwrap();
        let tier = GitTier::new(&gate_ctx.ledger_root);
        let result = tier.read(&TierQuery::kind(RecordKind::Scenario)).unwrap();
        assert_eq!(result.records.len(), 1);
        let obj = result.records[0].0.as_object().unwrap();
        assert!(!obj.contains_key("covered"), "the core index must never carry a covered field");
        assert!(!obj.contains_key("surface_ref"), "the core index must never carry a surface_ref field");
    }

    #[test]
    fn two_roots_sharing_a_scenario_id_stay_distinct() {
        let repo = TempDir::new().unwrap();
        write_clean_feature(&repo.path().join("apps/a/specs"), "world", "hotdeal", "01", "App A's hotdeal");
        write_clean_feature(&repo.path().join("apps/b/specs"), "world", "hotdeal", "01", "App B's hotdeal");
        write(
            repo.path(),
            "canon.yaml",
            "specs:\n  roots:\n    - id: app-a\n      root: apps/a/specs\n    - id: app-b\n      root: apps/b/specs\n",
        );

        let outcome = run_sync(repo.path(), None).unwrap();
        assert!(outcome.is_clean());
        assert_eq!(outcome.total_written(), 2);

        let gate_ctx = GateCtx::from_repo(repo.path()).unwrap();
        let tier = GitTier::new(&gate_ctx.ledger_root);
        let result = tier.read(&TierQuery::kind(RecordKind::Scenario)).unwrap();
        assert_eq!(result.records.len(), 2);
        let project_ids: std::collections::BTreeSet<String> = result.records.iter().map(|r| r.0["project_id"].as_str().unwrap().to_string()).collect();
        assert_eq!(project_ids, std::collections::BTreeSet::from(["app-a".to_string(), "app-b".to_string()]));
    }

    #[test]
    fn spec_root_override_bypasses_config_and_syncs_exactly_that_directory() {
        let repo = TempDir::new().unwrap();
        // A configured (but unused, since --spec-root overrides it)
        // root that would fail validation if it were ever read.
        write(&repo.path().join("configured"), "kind=feature/world/broken.feature", "Feature: broken\n\n  @world.broken.01\n  Scenario: no provenance\n    Given x\n");
        write(repo.path(), "canon.yaml", "specs:\n  roots:\n    - id: configured\n      root: configured\n");
        write_clean_feature(&repo.path().join("ad-hoc"), "world", "hotdeal", "01", "Opening the hotdeal overlay");

        let outcome = run_sync(repo.path(), Some(Path::new("ad-hoc"))).unwrap();
        assert!(outcome.is_clean(), "the configured (broken) root must never be read when --spec-root overrides it");
        assert_eq!(outcome.roots.len(), 1);
        assert_eq!(outcome.roots[0].id, ProjectId::parse("root").unwrap(), "the override uses the same stable literal default id");
        assert_eq!(outcome.total_written(), 1);
    }

    #[test]
    fn a_duplicate_scenario_id_in_one_root_aborts_that_root_but_siblings_continue() {
        let repo = TempDir::new().unwrap();
        let prov = provenance_comment();
        // Root `app-dup`: two well-formed features declaring the SAME
        // scenario_id `world.hotdeal.01` — passes S11 `canon-fmt::check`
        // (which never dedups scenario_ids) but violates D5's
        // one-record-per-key contract, so this root aborts (0 writes)
        // via `sync_errors`, NOT a frozen `FmtFailureClass` violation.
        let dup_root = repo.path().join("apps/dup/specs");
        write(&dup_root, "kind=feature/area=world/hotdeal.feature", &format!("Feature: world hotdeal\n{prov}\n\n  @world.hotdeal.01\n  Scenario: First\n{prov}\n    Given x\n"));
        write(&dup_root, "kind=feature/area=world/hotdeal-again.feature", &format!("Feature: world hotdeal again\n{prov}\n\n  @world.hotdeal.01\n  Scenario: Second\n{prov}\n    Given y\n"));
        // Root `app-ok`: a clean, distinct root that must still materialize.
        write_clean_feature(&repo.path().join("apps/ok/specs"), "world", "coupon", "02", "Applying the coupon");
        write(repo.path(), "canon.yaml", "specs:\n  roots:\n    - id: app-dup\n      root: apps/dup/specs\n    - id: app-ok\n      root: apps/ok/specs\n");

        // run_sync does NOT short-circuit (returns Ok, not Err): the
        // duplicate is a per-root abort, never a whole-sync InventoryError.
        let outcome = run_sync(repo.path(), None).unwrap();
        assert!(!outcome.is_clean(), "a duplicate-key root makes the whole outcome unclean (CLI exit 1)");

        let dup = outcome.roots.iter().find(|r| r.id == ProjectId::parse("app-dup").unwrap()).unwrap();
        assert!(!dup.is_clean());
        assert!(dup.violations.is_empty(), "a duplicate scenario_id is NOT a frozen FmtFailureClass violation");
        assert!(!dup.sync_errors.is_empty(), "the duplicate is reported as a per-root sync-level error");
        assert_eq!(dup.written, 0, "the aborted root writes zero records");

        let ok = outcome.roots.iter().find(|r| r.id == ProjectId::parse("app-ok").unwrap()).unwrap();
        assert!(ok.is_clean(), "the sibling clean root still materializes despite the other root's abort");
        assert_eq!(ok.written, 1);

        // Only the clean sibling's single record persists in the tier.
        let gate_ctx = GateCtx::from_repo(repo.path()).unwrap();
        let tier = GitTier::new(&gate_ctx.ledger_root);
        let result = tier.read(&TierQuery::kind(RecordKind::Scenario)).unwrap();
        assert_eq!(result.records.len(), 1, "only the clean sibling root's record is written");
        let record: Scenario = serde_json::from_value(result.records[0].0.clone()).unwrap();
        assert_eq!(record.project_id, ProjectId::parse("app-ok").unwrap());
    }

    // ---- @subject: tag → Scenario.subject_id join (s36 task 6.2) ----

    #[test]
    fn a_tagged_scenario_round_trips_the_subject_join() {
        let repo = TempDir::new().unwrap();
        write_feature_with_tags(&repo.path().join(paths::SPECS_DIR), "world", "hotdeal", "01", "Opening the hotdeal overlay", "@subject:payments-core");

        let outcome = run_sync(repo.path(), None).unwrap();
        assert!(outcome.is_clean(), "a well-formed subject tag is never a violation");
        assert!(outcome.roots[0].tag_diagnostics.is_empty(), "a well-formed single tag emits no diagnostic");
        assert_eq!(outcome.total_written(), 1);

        let record = only_scenario(repo.path());
        assert_eq!(record.subject_id, Some(SubjectId::parse("payments-core").unwrap()), "the @subject: tag populates the join on the materialized record");
    }

    #[test]
    fn an_untagged_scenario_leaves_the_subject_join_unset() {
        let repo = TempDir::new().unwrap();
        write_clean_feature(&repo.path().join(paths::SPECS_DIR), "world", "hotdeal", "01", "Opening the hotdeal overlay");

        let outcome = run_sync(repo.path(), None).unwrap();
        assert!(outcome.is_clean());
        assert!(outcome.roots[0].tag_diagnostics.is_empty());
        assert_eq!(outcome.total_written(), 1);

        let record = only_scenario(repo.path());
        assert_eq!(record.subject_id, None, "an untagged scenario carries no subject join");
        // Additive-field guarantee: no spurious key on the wire.
        assert!(!record_json(repo.path()).as_object().unwrap().contains_key("subject_id"), "an unset subject_id never reserializes a spurious key");
    }

    #[test]
    fn a_malformed_subject_tag_is_counted_but_the_scenario_is_still_indexed_without_the_join() {
        let repo = TempDir::new().unwrap();
        // `Bad_Slug` is a single whitespace token (so the scan pairs it)
        // but not a kebab-case subject slug (underscore + uppercase).
        write_feature_with_tags(&repo.path().join(paths::SPECS_DIR), "world", "hotdeal", "01", "Opening the hotdeal overlay", "@subject:Bad_Slug");

        let outcome = run_sync(repo.path(), None).unwrap();
        assert!(outcome.is_clean(), "a malformed tag is fail-soft, never a whole-root abort");
        assert_eq!(outcome.total_written(), 1, "the scenario is still indexed");
        assert_eq!(outcome.roots[0].tag_diagnostics.len(), 1, "the malformed tag is counted as one named diagnostic");
        assert!(outcome.roots[0].tag_diagnostics[0].contains("Bad_Slug"));

        let record = only_scenario(repo.path());
        assert_eq!(record.subject_id, None, "a malformed tag drops the join but keeps the record");
    }

    #[test]
    fn two_subject_tags_on_one_scenario_first_wins_with_a_counted_diagnostic() {
        let repo = TempDir::new().unwrap();
        write_feature_with_tags(&repo.path().join(paths::SPECS_DIR), "world", "hotdeal", "01", "Opening the hotdeal overlay", "@subject:first-subject @subject:second-subject");

        let outcome = run_sync(repo.path(), None).unwrap();
        assert!(outcome.is_clean(), "multiple tags are fail-soft, never a whole-root abort");
        assert_eq!(outcome.total_written(), 1);
        assert_eq!(outcome.roots[0].tag_diagnostics.len(), 1, "the duplicate is counted once");
        assert!(outcome.roots[0].tag_diagnostics[0].contains("first-subject"), "the diagnostic names the winning tag");

        let record = only_scenario(repo.path());
        assert_eq!(record.subject_id, Some(SubjectId::parse("first-subject").unwrap()), "the first @subject: tag wins");
    }

    #[test]
    fn a_lane_tag_round_trips_the_lane_join() {
        let repo = TempDir::new().unwrap();
        write_feature_with_tags(&repo.path().join(paths::SPECS_DIR), "world", "hotdeal", "01", "Opening the hotdeal overlay", "@lane:process");

        let outcome = run_sync(repo.path(), None).unwrap();
        assert!(outcome.is_clean());
        assert!(outcome.roots[0].tag_diagnostics.is_empty());
        let record = only_scenario(repo.path());
        assert_eq!(record.lane.as_deref(), Some("process"));
    }

    #[test]
    fn two_lane_tags_first_wins_with_a_counted_diagnostic() {
        let repo = TempDir::new().unwrap();
        write_feature_with_tags(&repo.path().join(paths::SPECS_DIR), "world", "hotdeal", "01", "Opening the hotdeal overlay", "@lane:behavior @lane:design");

        let outcome = run_sync(repo.path(), None).unwrap();
        assert_eq!(outcome.roots[0].tag_diagnostics.len(), 1);
        assert!(outcome.roots[0].tag_diagnostics[0].contains("behavior"));
        assert_eq!(only_scenario(repo.path()).lane.as_deref(), Some("behavior"));
    }

    #[test]
    fn malformed_lane_is_dropped_with_a_counted_diagnostic() {
        let repo = TempDir::new().unwrap();
        write_feature_with_tags(&repo.path().join(paths::SPECS_DIR), "world", "hotdeal", "01", "Opening the hotdeal overlay", "@lane:Bad_Lane");

        let outcome = run_sync(repo.path(), None).unwrap();
        assert_eq!(outcome.roots[0].tag_diagnostics.len(), 1);
        assert!(outcome.roots[0].tag_diagnostics[0].contains("Bad_Lane"));
        assert_eq!(only_scenario(repo.path()).lane, None);
    }

    #[test]
    fn a_case_tag_round_trips_and_a_value_outside_the_vocabulary_is_dropped() {
        let repo = TempDir::new().unwrap();
        std::fs::create_dir_all(repo.path().join(".canon/vocab/canon.core")).unwrap();
        std::fs::write(
            repo.path().join(".canon/vocab/canon.core/plugin.yaml"),
            "id: canon.core\nversion: \"0.1.0\"\nkind: core\nexports:\n  enums: enums.yaml\n",
        )
        .unwrap();
        std::fs::write(repo.path().join(".canon/vocab/canon.core/enums.yaml"), "enums:\n  case: [happy, failure, edge]\n").unwrap();

        write_feature_with_tags(&repo.path().join(paths::SPECS_DIR), "world", "hotdeal", "01", "Opening the hotdeal overlay", "@case:failure");
        let outcome = run_sync(repo.path(), None).unwrap();
        assert!(outcome.roots[0].tag_diagnostics.is_empty(), "{:?}", outcome.roots[0].tag_diagnostics);
        assert_eq!(only_scenario(repo.path()).case.as_deref(), Some("failure"));

        // A case the repo never declared must not count toward
        // `require_cases`: dropped, with a named diagnostic, and the
        // re-synced record no longer carries the old case.
        write_feature_with_tags(&repo.path().join(paths::SPECS_DIR), "world", "hotdeal", "01", "Opening the hotdeal overlay", "@case:sad");
        let outcome = run_sync(repo.path(), None).unwrap();
        assert_eq!(outcome.roots[0].tag_diagnostics.len(), 1);
        assert!(outcome.roots[0].tag_diagnostics[0].contains("@case:sad"), "{}", outcome.roots[0].tag_diagnostics[0]);
        let gate_ctx = GateCtx::from_repo(repo.path()).unwrap();
        let result = GitTier::new(&gate_ctx.ledger_root).read(&TierQuery::kind(RecordKind::Scenario)).unwrap();
        let latest = fold_latest_by_key(
            result.records.iter().filter_map(|r| serde_json::from_value::<Scenario>(r.0.clone()).ok()).collect::<Vec<_>>(),
            |s: &Scenario| (s.project_id.clone(), s.scenario_id.clone()),
            |s: &Scenario| s.envelope.at,
            |s: &Scenario| s.envelope.schema,
            |_: &Scenario| "",
        );
        let key = (ProjectId::parse("root").unwrap(), ScenarioId::parse("world.hotdeal.01").unwrap());
        assert_eq!(latest.get(&key).unwrap().case, None);
    }

    #[test]
    fn unknown_namespace_is_counted_once_per_scenario_and_namespace() {
        let repo = TempDir::new().unwrap();
        write_feature_with_tags(&repo.path().join(paths::SPECS_DIR), "world", "hotdeal", "01", "Opening the hotdeal overlay", "@foo:bar @foo:baz @p2");

        let outcome = run_sync(repo.path(), None).unwrap();
        assert_eq!(outcome.roots[0].tag_diagnostics.len(), 1);
        assert!(outcome.roots[0].tag_diagnostics[0].contains("@foo:"));
    }

    #[test]
    fn a_resync_after_adding_a_subject_tag_rewrites_the_record_to_carry_the_join() {
        let repo = TempDir::new().unwrap();
        write_clean_feature(&repo.path().join(paths::SPECS_DIR), "world", "hotdeal", "01", "Opening the hotdeal overlay");
        run_sync(repo.path(), None).unwrap();

        write_feature_with_tags(&repo.path().join(paths::SPECS_DIR), "world", "hotdeal", "01", "Opening the hotdeal overlay", "@subject:payments-core");
        let second = run_sync(repo.path(), None).unwrap();
        assert_eq!(second.total_written(), 1, "adding a subject tag changes the file bytes, so a new record is appended");

        let gate_ctx = GateCtx::from_repo(repo.path()).unwrap();
        let tier = GitTier::new(&gate_ctx.ledger_root);
        let result = tier.read(&TierQuery::kind(RecordKind::Scenario)).unwrap();
        let latest = fold_latest_by_key(
            result.records.iter().filter_map(|r| serde_json::from_value::<Scenario>(r.0.clone()).ok()).collect::<Vec<_>>(),
            |s: &Scenario| (s.project_id.clone(), s.scenario_id.clone()),
            |s: &Scenario| s.envelope.at,
            |s: &Scenario| s.envelope.schema,
            |_: &Scenario| "",
        );
        let key = (ProjectId::parse("root").unwrap(), ScenarioId::parse("world.hotdeal.01").unwrap());
        assert_eq!(latest.get(&key).unwrap().subject_id, Some(SubjectId::parse("payments-core").unwrap()));
    }
}
