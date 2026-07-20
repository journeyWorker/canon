//! `canon inventory sync [--spec-root <dir>]` (s15 P3a,
//! `inventory-materialization` spec, tasks 3.1/3.4/3.5): the GENERAL
//! feature-corpus → ledger indexer. For EACH configured
//! `specs.roots[]` entry (`canon.yaml`, design D3):
//!
//! 1. Validate with `canon-fmt::check` (S11) — ANY violation ABORTS
//!    THE WHOLE ROOT, writing zero `Scenario` records for it and
//!    reporting the violation(s); never a partial sync.
//! 2. Scan `features/` via the extended `canon_fmt::gherkin::scan`
//!    (task 3.2) — each tag paired with its header's label as `title`,
//!    `source_digest` a sha256 over the `.feature` file's raw bytes.
//! 3. Materialize ONE `Scenario` index record per `(project_id,
//!    scenario_id)` via the normal append-only `GitTier::write`.
//!    Idempotence is LOGICAL (design D5): fold latest-per-key first
//!    (`canon_store::fold_latest_by_key`), no-op when the candidate's
//!    `source_digest`/`title` already match the latest folded record.
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

use std::path::{Path, PathBuf};

use canon_fmt::Violation;
use canon_gate::GateCtx;
use canon_model::ids::SpecDigest;
use canon_model::{Actor, Envelope, ProjectId, RecordKind, Scenario, ScenarioId, SubjectId};
use canon_store::fold_latest_by_key;
use canon_store::git_tier::GitTier;
use canon_store::tier::{StoreError, Tier, TierQuery};
use chrono::Utc;

use crate::context::resolve_repo_root;

/// The stable literal id BOTH the absent-`specs:` default AND
/// `--spec-root`'s ad hoc override resolve to (design D3) — NEVER the
/// checkout directory name.
const DEFAULT_SPEC_ROOT_ID: &str = "root";
const DEFAULT_SPEC_ROOT_DIR: &str = "specs";

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
    let repo_root = canon_yaml.parent().unwrap_or_else(|| Path::new("."));

    // Missing / unreadable canon.yaml: fail-soft (no config at all is
    // a legitimate first-run / minimal state) — same as
    // `IngestSourceConfig::load`.
    let Ok(text) = std::fs::read_to_string(canon_yaml) else {
        return Ok(vec![default_root(repo_root)]);
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
        return Ok(vec![default_root(repo_root)]);
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
        let root_dir = raw.root.as_deref().unwrap_or(DEFAULT_SPEC_ROOT_DIR);
        let root_path = Path::new(root_dir);
        let root = if root_path.is_absolute() { root_path.to_path_buf() } else { repo_root.join(root_path) };
        roots.push(SpecRoot { id, root });
    }
    Ok(roots)
}

/// Rebindable roots inventory-sync reads through (design D3,
/// spec-ledger-selftest Req 2) — composes over [`GateCtx`]'s own
/// two-constructor pattern (`from_repo`/`from_fixture`) rather than a
/// second, hand-rolled copy of its `<repo>/canon.yaml` `tiers.git.root`
/// resolution / default `canon/ledger` layout: inventory-sync and the
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
    /// second copy of that resolution).
    pub fn from_repo(repo: impl Into<PathBuf>) -> Self {
        let gate_ctx = GateCtx::from_repo(repo);
        Self { repo: gate_ctx.repo, ledger_root: gate_ctx.ledger_root }
    }

    /// Fixture binding (spec-ledger-selftest Req 2's "fixture constructor
    /// runs fully offline against a tempdir" scenario): every root under
    /// one fresh tempdir, the identical `canon/ledger` default layout
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
    pub written: usize,
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
pub fn run_sync(repo: &Path, spec_root: Option<&Path>) -> Result<SyncOutcome, InventoryError> {
    let repo = resolve_repo_root(repo);
    let ctx = SyncCtx::from_repo(&repo);
    run_sync_with_ctx(&ctx, spec_root)
}

/// The ONE downstream sync entry point (module doc / spec-ledger-selftest
/// Req 2's "no downstream sync code branches on which constructor built
/// its ctx" scenario): [`run_sync`] (production, [`SyncCtx::from_repo`])
/// and `canon selftest`'s inventory fixtures ([`SyncCtx::from_fixture`])
/// both call this exact function, unconditionally.
pub fn run_sync_with_ctx(ctx: &SyncCtx, spec_root_override: Option<&Path>) -> Result<SyncOutcome, InventoryError> {
    let roots = ctx.spec_roots(spec_root_override)?;
    let tier = GitTier::new(&ctx.ledger_root);
    // Never an agent-authored attestation — a deterministic
    // materializer over the `.feature` corpus, mirroring
    // `canon-gate`'s own `Actor::new_unattributed("canon-gate")` for
    // its typed-task compile path (never an agent-authored record).
    let actor = Actor::new_unattributed("canon-inventory-sync");

    let mut outcomes = Vec::with_capacity(roots.len());
    for spec_root in &roots {
        outcomes.push(sync_one_root(&tier, spec_root, &actor)?);
    }
    Ok(SyncOutcome { roots: outcomes })
}

/// Scan `root`'s `features/` corpus for every well-formed
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
/// `(scenario_id, title, source_digest)` plus the raw `@subject:<value>`
/// tag values the scan lexed for it (s36 task 6.2, `canon-fmt`'s
/// [`canon_fmt::gherkin::ScenarioScan::subject_tags`]). Grammar
/// validation of those values against [`SubjectId`] — and the fail-soft
/// diagnostics for a malformed or duplicated tag — is [`sync_one_root`]'s
/// job, not the pure lexer's. [`scan_feature_corpus`] is the thin
/// projection over this that drops the subject tags for callers (s16's
/// `plugin_sync`) that only need the `(project_id, scenario_id)` universe.
#[derive(Debug, Clone)]
pub struct ScannedScenario {
    pub scenario_id: ScenarioId,
    pub title: String,
    pub source_digest: SpecDigest,
    pub subject_tags: Vec<String>,
}

/// The full walk both [`scan_feature_corpus`] and [`sync_one_root`]
/// derive from — one place that reads the `.feature` corpus, so the
/// scenario universe and the subject-tag pairing can never drift apart.
pub fn scan_feature_corpus_detailed(root: &Path) -> Vec<ScannedScenario> {
    let mut scanned = Vec::new();
    for path in canon_fmt::util::walk_files(root, "features") {
        if path.extension().and_then(|e| e.to_str()) != Some("feature") {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let Ok(text) = std::str::from_utf8(&bytes) else { continue };
        let scan = canon_fmt::gherkin::scan(text);
        let digest = canon_fmt::gherkin::source_digest(&bytes);
        for candidate in scan.scenarios {
            if let Ok(scenario_id) = ScenarioId::parse(&candidate.scenario_id) {
                scanned.push(ScannedScenario { scenario_id, title: candidate.title, source_digest: digest.clone(), subject_tags: candidate.subject_tags });
            }
        }
    }
    scanned
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

/// One configured root's validate → scan → materialize pass (module
/// doc). A fresh `existing` read per root keeps the fold correct even
/// if two roots were misconfigured to share a `project_id` — every
/// candidate is checked against the LATEST already-committed state,
/// never a snapshot taken before this run started.
fn sync_one_root(tier: &GitTier, spec_root: &SpecRoot, actor: &Actor) -> Result<RootSyncOutcome, InventoryError> {
    let report = canon_fmt::check(&spec_root.root);
    if !report.is_clean() {
        return Ok(RootSyncOutcome {
            id: spec_root.id.clone(),
            root: spec_root.root.clone(),
            scanned: 0,
            written: 0,
            violations: report.violations,
            sync_errors: Vec::new(),
            tag_diagnostics: Vec::new(),
        });
    }

    // Scan `features/` alone — NEVER `inventory/` (module doc: the
    // index is feature-corpus-derived only). The detailed scan also
    // carries each scenario's raw `@subject:` tag(s) (s36 task 6.2).
    let scanned: Vec<ScannedScenario> = scan_feature_corpus_detailed(&spec_root.root);
    // D5: exactly one index record per `(project_id, scenario_id)`. A
    // duplicate scenario_id WITHIN this root's scan can't pick a
    // winning title/source_digest, so abort THIS root (0 writes) — a
    // per-root corpus authoring fault, parallel to an S11 abort. NOT a
    // `FmtFailureClass` (frozen to the 11 audited gaps, never a 12th)
    // and NOT an `InventoryError` (that would short-circuit sibling
    // roots and mis-map to the exit-2 config lane instead of the
    // exit-1 root-abort lane).
    let mut seen: std::collections::BTreeSet<&ScenarioId> = std::collections::BTreeSet::new();
    let mut dups: std::collections::BTreeSet<&ScenarioId> = std::collections::BTreeSet::new();
    for s in &scanned {
        if !seen.insert(&s.scenario_id) {
            dups.insert(&s.scenario_id);
        }
    }
    if !dups.is_empty() {
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
        return Ok(RootSyncOutcome {
            id: spec_root.id.clone(),
            root: spec_root.root.clone(),
            scanned: scanned.len(),
            written: 0,
            violations: Vec::new(),
            sync_errors,
            tag_diagnostics: Vec::new(),
        });
    }

    let existing = tier.read(&TierQuery::kind(RecordKind::Scenario))?;
    // Keep each record's OWN content digest alongside its typed
    // `Scenario` (s21 D3: the fold's tie-break needs a real digest,
    // never just `at`) — `content_digest12` over the record's raw
    // JSON, the same digest `GitTier`/`PgTier` writes already compute.
    struct ExistingScenario {
        scenario: Scenario,
        digest: String,
    }
    let existing_scenarios: Vec<ExistingScenario> = existing
        .records
        .iter()
        .filter_map(|raw| serde_json::from_value::<Scenario>(raw.0.clone()).ok().map(|scenario| ExistingScenario { digest: canon_store::partition::content_digest12(&raw.0), scenario }))
        .collect();
    let folded = fold_latest_by_key(
        existing_scenarios,
        |e: &ExistingScenario| (e.scenario.project_id.clone(), e.scenario.scenario_id.clone()),
        |e: &ExistingScenario| e.scenario.envelope.at,
        |e: &ExistingScenario| e.digest.as_str(),
    );

    let mut written = 0;
    // Fail-soft `@subject:` diagnostics accumulate across the whole
    // root but never abort it (see `RootSyncOutcome::tag_diagnostics`).
    let mut tag_diagnostics = Vec::new();
    for candidate in &scanned {
        // Resolve BEFORE the idempotence check so a persistently
        // malformed/duplicate tag is reported on EVERY sync run, not
        // just the write that first introduced it.
        let subject_id = resolve_subject_tag(candidate, &spec_root.id, &mut tag_diagnostics);
        let key = (spec_root.id.clone(), candidate.scenario_id.clone());
        // The subject join joins the idempotence key: a record written
        // before this field existed (or before the tag was added) gets
        // rewritten to carry the join, even if title/digest match.
        let unchanged = folded
            .get(&key)
            .is_some_and(|latest| latest.scenario.source_digest == candidate.source_digest && latest.scenario.title == candidate.title && latest.scenario.subject_id == subject_id);
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
        record.subject_id = subject_id;
        tier.write(&record)?;
        written += 1;
    }

    Ok(RootSyncOutcome {
        id: spec_root.id.clone(),
        root: spec_root.root.clone(),
        scanned: scanned.len(),
        written,
        violations: Vec::new(),
        sync_errors: Vec::new(),
        tag_diagnostics,
    })
}

/// `canon inventory sync`'s stdout — one block per configured root.
pub fn format_human(outcome: &SyncOutcome) -> String {
    let mut out = String::new();
    for root in &outcome.roots {
        if root.is_clean() {
            out.push_str(&format!(
                "canon inventory sync: root `{}` ({}) — {} scenario(s) scanned, {} record(s) written\n",
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
        // Fail-soft `@subject:` diagnostics (s36 task 6.2): printed for
        // any root that reached materialization, whether or not it also
        // wrote records — the scenario was still indexed, only its
        // subject join was dropped.
        for d in &root.tag_diagnostics {
            out.push_str(&format!("  [subject-tag] {d}\n"));
        }
    }
    out
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

    /// A single well-formed `.feature` file under `<root>/features/`,
    /// with a provenance comment on both headers (so `canon-fmt::check`
    /// reports zero violations) and one scenario tag/header pair.
    fn write_clean_feature(root: &Path, area: &str, surface: &str, nn: &str, title: &str) {
        let text = format!(
            "Feature: {area} {surface}\n{prov}\n\n  @{area}.{surface}.{nn}\n  Scenario: {title}\n{prov}\n    Given a precondition\n",
            prov = provenance_comment()
        );
        write(root, &format!("features/kind=feature/area={area}/{surface}.feature"), &text);
    }

    /// Like [`write_clean_feature`] but appends `extra_tags` (e.g.
    /// `@subject:payments-core`) to the scenario's tag line (s36 task
    /// 6.2).
    fn write_feature_with_tags(root: &Path, area: &str, surface: &str, nn: &str, title: &str, extra_tags: &str) {
        let text = format!(
            "Feature: {area} {surface}\n{prov}\n\n  @{area}.{surface}.{nn} {extra_tags}\n  Scenario: {title}\n{prov}\n    Given a precondition\n",
            prov = provenance_comment()
        );
        write(root, &format!("features/kind=feature/area={area}/{surface}.feature"), &text);
    }

    fn only_scenario(repo: &Path) -> Scenario {
        let gate_ctx = GateCtx::from_repo(repo);
        let tier = GitTier::new(&gate_ctx.ledger_root);
        let result = tier.read(&TierQuery::kind(RecordKind::Scenario)).unwrap();
        assert_eq!(result.records.len(), 1, "expected exactly one Scenario record");
        serde_json::from_value(result.records[0].0.clone()).unwrap()
    }

    fn record_json(repo: &Path) -> serde_json::Value {
        let gate_ctx = GateCtx::from_repo(repo);
        let tier = GitTier::new(&gate_ctx.ledger_root);
        let result = tier.read(&TierQuery::kind(RecordKind::Scenario)).unwrap();
        assert_eq!(result.records.len(), 1, "expected exactly one Scenario record");
        result.records[0].0.clone()
    }

    // ---- specs.roots[] config resolution (task 3.1) ----

    #[test]
    fn missing_specs_key_resolves_the_default_single_root() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), "canon.yaml", "tiers:\n  git:\n    root: canon/ledger\n");
        let roots = load_spec_roots(&dir.path().join("canon.yaml")).unwrap();
        assert_eq!(roots, vec![SpecRoot { id: ProjectId::parse("root").unwrap(), root: dir.path().join("specs") }]);
    }

    #[test]
    fn missing_canon_yaml_also_resolves_the_default_single_root() {
        let dir = TempDir::new().unwrap();
        let roots = load_spec_roots(&dir.path().join("canon.yaml")).unwrap();
        assert_eq!(roots, vec![SpecRoot { id: ProjectId::parse("root").unwrap(), root: dir.path().join("specs") }]);
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
        assert_eq!(roots, vec![SpecRoot { id: ProjectId::parse("app-a").unwrap(), root: dir.path().join("specs") }]);
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
        write_clean_feature(&repo.path().join("specs"), "world", "hotdeal", "01", "Opening the hotdeal overlay");

        let outcome = run_sync(repo.path(), None).unwrap();
        assert!(outcome.is_clean());
        assert_eq!(outcome.total_written(), 1);

        let gate_ctx = GateCtx::from_repo(repo.path());
        let tier = GitTier::new(&gate_ctx.ledger_root);
        let result = tier.read(&TierQuery::kind(RecordKind::Scenario)).unwrap();
        assert_eq!(result.records.len(), 1);
        let record: Scenario = serde_json::from_value(result.records[0].0.clone()).unwrap();
        assert_eq!(record.project_id, ProjectId::parse("root").unwrap());
        assert_eq!(record.scenario_id, ScenarioId::parse("world.hotdeal.01").unwrap());
        assert_eq!(record.title, "Opening the hotdeal overlay");
        let bytes = std::fs::read(repo.path().join("specs/features/kind=feature/area=world/hotdeal.feature")).unwrap();
        assert_eq!(record.source_digest, SpecDigest::of(&bytes));
    }

    #[test]
    fn a_validation_violation_aborts_the_whole_root_zero_writes() {
        let repo = TempDir::new().unwrap();
        // No provenance comment on either header -> MissingProvenance
        // violation (S11 `canon-fmt::check`).
        write(&repo.path().join("specs"), "features/world/hotdeal.feature", "Feature: world hotdeal\n\n  @world.hotdeal.01\n  Scenario: Opening the hotdeal overlay\n    Given a precondition\n");

        let outcome = run_sync(repo.path(), None).unwrap();
        assert!(!outcome.is_clean());
        assert_eq!(outcome.total_written(), 0);
        assert!(!outcome.roots[0].violations.is_empty());

        let gate_ctx = GateCtx::from_repo(repo.path());
        let tier = GitTier::new(&gate_ctx.ledger_root);
        let result = tier.read(&TierQuery::kind(RecordKind::Scenario)).unwrap();
        assert!(result.records.is_empty(), "an aborted root must write zero Scenario records");
    }

    #[test]
    fn resync_of_an_unchanged_corpus_writes_zero_new_records() {
        let repo = TempDir::new().unwrap();
        write_clean_feature(&repo.path().join("specs"), "world", "hotdeal", "01", "Opening the hotdeal overlay");

        let first = run_sync(repo.path(), None).unwrap();
        assert_eq!(first.total_written(), 1);
        let second = run_sync(repo.path(), None).unwrap();
        assert_eq!(second.total_written(), 0, "an unchanged corpus must re-sync as a no-op");

        let gate_ctx = GateCtx::from_repo(repo.path());
        let tier = GitTier::new(&gate_ctx.ledger_root);
        let result = tier.read(&TierQuery::kind(RecordKind::Scenario)).unwrap();
        assert_eq!(result.records.len(), 1, "still exactly one record after two syncs");
    }

    #[test]
    fn a_changed_feature_file_produces_exactly_one_new_record() {
        let repo = TempDir::new().unwrap();
        write_clean_feature(&repo.path().join("specs"), "world", "hotdeal", "01", "Opening the hotdeal overlay");
        run_sync(repo.path(), None).unwrap();

        write_clean_feature(&repo.path().join("specs"), "world", "hotdeal", "01", "Opening the hotdeal overlay v2");
        let second = run_sync(repo.path(), None).unwrap();
        assert_eq!(second.total_written(), 1, "a changed .feature file must append exactly one new record");

        let gate_ctx = GateCtx::from_repo(repo.path());
        let tier = GitTier::new(&gate_ctx.ledger_root);
        let result = tier.read(&TierQuery::kind(RecordKind::Scenario)).unwrap();
        assert_eq!(result.records.len(), 2, "the prior record is appended alongside, never overwritten");
    }

    #[test]
    fn sync_derives_the_index_from_features_alone_ignoring_any_inventory_dir() {
        let repo = TempDir::new().unwrap();
        write_clean_feature(&repo.path().join("specs"), "world", "hotdeal", "01", "Opening the hotdeal overlay");
        // A present, WELL-FORMED `inventory/` dir carrying real
        // `covered_by` data -- sync must never read this to populate
        // the index.
        write(
            &repo.path().join("specs"),
            "inventory/kind=inventory/area=world/surface=hub/hub.yaml",
            "schema: 1\nkind: inventory\nat: \"2026-07-10T00:00:00Z\"\nactor:\n  agent_id: canon-fmt-test\nworld.hub.hub-header:\n  upstream:\n    pin: 9c93d024b\n    file: routes/world/hub/index.tsx\n    symbol: RouteComponent\n    lines: \"1-10\"\n  covered_by: [world.hotdeal.01]\n",
        );

        let outcome = run_sync(repo.path(), None).unwrap();
        assert!(outcome.is_clean());
        assert_eq!(outcome.total_written(), 1, "the inventory/ file must not itself produce a Scenario record");

        let gate_ctx = GateCtx::from_repo(repo.path());
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

        let gate_ctx = GateCtx::from_repo(repo.path());
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
        write(&repo.path().join("configured"), "features/world/broken.feature", "Feature: broken\n\n  @world.broken.01\n  Scenario: no provenance\n    Given x\n");
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
        write(&dup_root, "features/kind=feature/area=world/hotdeal.feature", &format!("Feature: world hotdeal\n{prov}\n\n  @world.hotdeal.01\n  Scenario: First\n{prov}\n    Given x\n"));
        write(&dup_root, "features/kind=feature/area=world/hotdeal-again.feature", &format!("Feature: world hotdeal again\n{prov}\n\n  @world.hotdeal.01\n  Scenario: Second\n{prov}\n    Given y\n"));
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
        let gate_ctx = GateCtx::from_repo(repo.path());
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
        write_feature_with_tags(&repo.path().join("specs"), "world", "hotdeal", "01", "Opening the hotdeal overlay", "@subject:payments-core");

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
        write_clean_feature(&repo.path().join("specs"), "world", "hotdeal", "01", "Opening the hotdeal overlay");

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
        write_feature_with_tags(&repo.path().join("specs"), "world", "hotdeal", "01", "Opening the hotdeal overlay", "@subject:Bad_Slug");

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
        write_feature_with_tags(&repo.path().join("specs"), "world", "hotdeal", "01", "Opening the hotdeal overlay", "@subject:first-subject @subject:second-subject");

        let outcome = run_sync(repo.path(), None).unwrap();
        assert!(outcome.is_clean(), "multiple tags are fail-soft, never a whole-root abort");
        assert_eq!(outcome.total_written(), 1);
        assert_eq!(outcome.roots[0].tag_diagnostics.len(), 1, "the duplicate is counted once");
        assert!(outcome.roots[0].tag_diagnostics[0].contains("first-subject"), "the diagnostic names the winning tag");

        let record = only_scenario(repo.path());
        assert_eq!(record.subject_id, Some(SubjectId::parse("first-subject").unwrap()), "the first @subject: tag wins");
    }

    #[test]
    fn a_resync_after_adding_a_subject_tag_rewrites_the_record_to_carry_the_join() {
        let repo = TempDir::new().unwrap();
        write_clean_feature(&repo.path().join("specs"), "world", "hotdeal", "01", "Opening the hotdeal overlay");
        run_sync(repo.path(), None).unwrap();

        write_feature_with_tags(&repo.path().join("specs"), "world", "hotdeal", "01", "Opening the hotdeal overlay", "@subject:payments-core");
        let second = run_sync(repo.path(), None).unwrap();
        assert_eq!(second.total_written(), 1, "adding a subject tag changes the file bytes, so a new record is appended");

        let gate_ctx = GateCtx::from_repo(repo.path());
        let tier = GitTier::new(&gate_ctx.ledger_root);
        let result = tier.read(&TierQuery::kind(RecordKind::Scenario)).unwrap();
        let latest = fold_latest_by_key(
            result.records.iter().filter_map(|r| serde_json::from_value::<Scenario>(r.0.clone()).ok()).collect::<Vec<_>>(),
            |s: &Scenario| (s.project_id.clone(), s.scenario_id.clone()),
            |s: &Scenario| s.envelope.at,
            |_: &Scenario| "",
        );
        let key = (ProjectId::parse("root").unwrap(), ScenarioId::parse("world.hotdeal.01").unwrap());
        assert_eq!(latest.get(&key).unwrap().subject_id, Some(SubjectId::parse("payments-core").unwrap()));
    }
}
