//! The gate-context seam (design decisions 1/9, S3's `SessionAdapter`
//! precedent for freezing a wave-2 contract,
//! `crates/canon-ingest/src/adapter.rs`'s module doc: "the
//! `SessionAdapter` trait + `UnifiedRow` normalization target ... Wave
//! 1, frozen for Wave 2's ... adapters"). [`GateCtx`] names every
//! rebindable root a check reads — the direct Rust port of
//! `tools/parity.py`'s `GateCtx` frozen dataclass
//! (the donor parity-harness audit's fixtures-selftest notes §3.1:
//! "the direct architectural ancestor of canon's own testing
//! requirement ... every `canon-gate`/`canon-check` crate needs an
//! equivalent typed 'roots' struct with a real-repo constructor and a
//! fixture-dir constructor"). [`GateContext`] is the LOADED bundle
//! (resolved policy + evidence corpus) every S5 wave-2 [`GateCheck`]
//! consumes — the shared input coverage/verdict-ledger/staleness/
//! trust-ladder/checkbox-grammar checks build against, mirroring how
//! `UnifiedRow` froze what every Wave-2 session adapter emits into.
//!
//! This module implements ONLY the loading seam (task 1.1's "Scaffold
//! `crates/canon-gate` consuming canon-model's ... types and
//! canon-store's git-tier adapter"). No [`GateCheck`] implementation
//! lives here — the static coverage check (task 1.2), the dynamic
//! verdict-ledger check (task 1.3), staleness (task 1.7), and `canon
//! gate check`'s dispatcher (task 1.9) are S5 wave-2.

use std::path::{Path, PathBuf};

use canon_model::{validate_evidence_batch, Divergence, EvidenceRecord, EvidenceViolation, RecordKind, Scenario, Subject};
use canon_policy::SchemaRegistry;
use canon_store::git_tier::GitTier;
use canon_store::tier::{StoreError, Tier, TierQuery};
use chrono::{DateTime, Utc};

use crate::policy::PolicyResolution;
use crate::Violation;

/// The infra-layout doc's fixed ledger location relative to a repo
/// root (`docs/superpowers/specs/2026-07-10-canon-design.md`:
/// `<repo>/.canon/ledger/ # Hive: kind=<kind>/area=<area>/*.json —
/// append-only`) — the [`canon_store::git_tier::GitTier`] root
/// [`GateCtx::from_repo`] defaults to when the canonical
/// `<repo>/canon.yaml` (S2's [`canon_store::policy::TierPolicy`]
/// source of truth) declares no `tiers.git.root` override.
pub const DEFAULT_LEDGER_RELATIVE_PATH: &str = canon_model::paths::LEDGER_DIR;

/// Rebindable roots every S5 wave-2 check reads through — the direct
/// Rust port of `tools/parity.py`'s `GateCtx` (module doc). Two
/// constructors, [`GateCtx::from_repo`]/[`GateCtx::from_fixture`], so
/// a production `canon gate check` run and a fixture-corpus `canon
/// gate selftest` run (S5 wave-2) share every downstream line — no
/// check function branches on which one built its `ctx`
/// (fixtures-selftest.md §3.1's own stated discipline).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateCtx {
    /// The repo root `policy.yaml` resolves against
    /// ([`PolicyResolution::resolve`]).
    pub repo: PathBuf,
    /// The [`GitTier`] root evidence records are read from.
    pub ledger_root: PathBuf,
}

impl GateCtx {
    /// Production binding: `repo` itself, with `ledger_root` resolved
    /// from the canonical `<repo>/canon.yaml` (the SAME file S2's
    /// `TierPolicy`/`canon tier age`/`canon query` resolve `tiers.git.root`
    /// from — never a second, gate-only config path) when that file
    /// exists and declares one (a relative `root` is joined against
    /// `repo`; an absolute one is used as-is), else the fixed
    /// [`DEFAULT_LEDGER_RELATIVE_PATH`]. An ABSENT `canon.yaml` is the
    /// default layout; a present one that cannot be read or parsed is an
    /// `Err` — falling back would read (or write) a ledger the repo's
    /// own config never named and report it as the truth.
    pub fn from_repo(repo: impl Into<PathBuf>) -> Result<Self, CanonYamlError> {
        let repo = repo.into();
        let configured_root = load_tier_policy(&repo)?.and_then(|policy| policy.local_git().map(|cfg| cfg.root.clone()));
        let ledger_root = match configured_root {
            Some(root) if root.is_absolute() => root,
            Some(root) => repo.join(root),
            None => repo.join(DEFAULT_LEDGER_RELATIVE_PATH),
        };
        Ok(Self { repo, ledger_root })
    }

    /// Fixture binding: every root under one fixture directory
    /// (fixtures-selftest.md §3.1's `fixture_ctx(fx)` — "binds EVERY
    /// `GateCtx` field into one fixture directory"). A fixture never
    /// reads `<repo>/canon.yaml`; `ledger_root` is always
    /// `fixture_dir/.canon/ledger`, the identical layout
    /// [`GateCtx::from_repo`]'s default uses.
    pub fn from_fixture(fixture_dir: impl Into<PathBuf>) -> Self {
        let repo = fixture_dir.into();
        let ledger_root = repo.join(DEFAULT_LEDGER_RELATIVE_PATH);
        Self { repo, ledger_root }
    }
}

/// `<repo>/canon.yaml` is present but unusable. Mirrors
/// `canon-cli`'s `TierCliError` wording so every verb that refuses the
/// file names it the same way.
#[derive(Debug, thiserror::Error)]
pub enum CanonYamlError {
    #[error("reading `{}`: {source}", path.display())]
    Read { path: PathBuf, source: std::io::Error },
    #[error("parsing `{}`: {source}", path.display())]
    Policy { path: PathBuf, source: canon_store::policy::PolicyError },
}

/// The canonical `<repo>/canon.yaml`'s [`TierPolicy`], parsed with S2's
/// own parser ([`TierPolicy::from_yaml_at`], resolved against `repo`
/// exactly as `crates/canon-cli/src/tiers.rs::build_tiers` does).
/// `Ok(None)` only when the file does not exist; any other read error,
/// or a file the parser rejects, is an `Err` naming the file.
///
/// [`TierPolicy`]: canon_store::policy::TierPolicy
/// [`TierPolicy::from_yaml_at`]: canon_store::policy::TierPolicy::from_yaml_at
fn load_tier_policy(repo: &Path) -> Result<Option<canon_store::policy::TierPolicy>, CanonYamlError> {
    let path = repo.join("canon.yaml");
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(CanonYamlError::Read { path, source }),
    };
    canon_store::policy::TierPolicy::from_yaml_at(&content, repo).map(Some).map_err(|source| CanonYamlError::Policy { path, source })
}

/// Every piece an S5 wave-2 [`GateCheck`] needs, loaded once per gate
/// run (module doc's `SessionAdapter`/`UnifiedRow` precedent).
/// `evidence` is every well-formed [`EvidenceRecord`]
/// [`GateCtx::ledger_root`]'s [`GitTier`] holds; `violations` is
/// everything the tier's own layout check or
/// [`canon_model::validate_evidence_batch`] rejected along the way —
/// §7's "malformed evidence is no evidence": skipped, counted, never a
/// crash and never silently dropped. `now` is the gate authority's
/// ONE injected clock reading (s21 `deterministic-gate-clock` D6):
/// every [`GateCheck`] that needs "the current instant" (staleness/
/// release-trust age checks, any time-bearing CEL `age_days(...)`
/// policy predicate) reads THIS field — never `Utc::now()` internally
/// — so every check in one gate run agrees on the identical instant.
pub struct GateContext {
    pub ctx: GateCtx,
    pub policy: PolicyResolution,
    pub evidence: Vec<EvidenceRecord>,
    /// The spec corpus `canon inventory sync` materializes — the
    /// enumeration side of a coverage question `evidence` structurally
    /// cannot answer. Grouping evidence can only ever surface artifacts
    /// that already HAVE evidence (`crate::coverage`'s own interface-gap
    /// note); a scenario nobody has attested to appears in no group, so
    /// the corpus itself has to be in the context for
    /// [`crate::spec_coverage`] to left-join against it.
    pub scenarios: Vec<Scenario>,
    /// Divergence lifecycle events, unfolded. `crate::spec_coverage`
    /// folds them through `canon_model::fold` rather than reading
    /// `status` off individual rows, so the gate and
    /// `canon divergence status` cannot disagree about which scenarios
    /// are open.
    pub divergences: Vec<Divergence>,
    /// Subjects, for the `spec_coverage.scope` status filter. Loaded
    /// HERE rather than read by the check, because this struct is the
    /// documented "loaded once per gate run" seam and a check that
    /// opened its own tier would fork that contract.
    pub subjects: Vec<Subject>,
    pub violations: Vec<EvidenceViolation>,
    /// Read problems from the three corpus kinds above, kept OUT of
    /// `violations` deliberately. [`crate::ledger`]'s `LedgerCheck`
    /// maps every `violations` entry to
    /// [`crate::FailureClass::MalformedEvidence`] and is
    /// unconditionally in `crate::dispatch::check_set`, so folding
    /// these in would turn a green repo red on upgrade with no policy
    /// change — the opposite of `spec_coverage` being opt-in. Only
    /// [`crate::spec_coverage`] reads this, and only when the policy
    /// section is present.
    pub corpus_violations: Vec<EvidenceViolation>,
    /// Kinds whose configured routing sends them somewhere this
    /// context's [`GitTier`] does not read, so their vector above is
    /// empty for a reason that is NOT "the corpus is empty".
    /// [`crate::spec_coverage`] refuses to run rather than reporting a
    /// silently inert pass — the failure mode that already made `Task`
    /// invisible to `canon report`.
    pub unreadable_kinds: Vec<RecordKind>,
    pub now: DateTime<Utc>,
}

impl GateContext {
    /// Load everything an S5 wave-2 check needs: resolve `policy.yaml`
    /// ([`PolicyResolution::resolve`]) and read the evidence corpus
    /// plus the three spec-corpus kinds
    /// ([`GateContext::scenarios`]/[`GateContext::divergences`]/
    /// [`GateContext::subjects`]) off `ctx.ledger_root`'s [`GitTier`]
    /// (canon-store, S2). Fails only on a [`StoreError`] the tier
    /// itself cannot recover from (e.g. an unreadable ledger root) —
    /// per-record malformed content is never an `Err` here, it lands
    /// in `violations` (module doc). `now` is REQUIRED, never
    /// defaulted to the live clock (s21 `deterministic-gate-clock`):
    /// the CLI dispatch boundary (`canon-cli/src/gate.rs`) is the ONE
    /// place `Utc::now()` is ever called for a gate run, exactly once,
    /// and threads the result in here.
    ///
    /// The three corpus reads are UNCONDITIONAL — not gated on
    /// `spec_coverage` being configured. That is deliberate and is the
    /// same property `risk_routing` already documents
    /// (`crate::coverage`: "A policy diff alone ... tightens coverage
    /// for every existing artifact with zero corpus edits"): adding the
    /// policy section must be sufficient by itself, with no second
    /// switch and no corpus edit. Their read problems land in
    /// [`GateContext::corpus_violations`], never `violations`, so the
    /// widened read cannot change any existing check's verdict.
    pub fn load(ctx: GateCtx, registry: &SchemaRegistry, now: DateTime<Utc>) -> Result<Self, GateContextError> {
        let policy = PolicyResolution::resolve(&ctx.repo, registry);

        let tier = GitTier::new(&ctx.ledger_root);
        let read = tier.read(&TierQuery::kind(RecordKind::EvidenceRecord))?;
        let (evidence, validation_violations) = validate_evidence_batch(&read.records);

        let mut violations = read.violations;
        violations.extend(validation_violations);

        let mut corpus_violations = Vec::new();
        let scenarios = read_corpus_kind(&tier, RecordKind::Scenario, &mut corpus_violations)?;
        let divergences = read_corpus_kind(&tier, RecordKind::Divergence, &mut corpus_violations)?;
        let subjects = read_corpus_kind(&tier, RecordKind::Subject, &mut corpus_violations)?;
        let unreadable_kinds = unreadable_corpus_kinds(&ctx.repo)?;

        Ok(Self { ctx, policy, evidence, scenarios, divergences, subjects, violations, corpus_violations, unreadable_kinds, now })
    }
}

/// Read one spec-corpus kind off `tier`, deserializing each row into
/// `T`. A row that fails to deserialize is recorded in
/// `corpus_violations` and SKIPPED — never an `Err`, mirroring
/// `validate_evidence_batch`'s "malformed evidence is no evidence"
/// discipline for the evidence corpus. canon-model ships no
/// `validate_<kind>_batch` analog (`validate_evidence_batch` is the only
/// one), so this is deserialization plus the tier's own layout check,
/// not a semantic validation pass.
fn read_corpus_kind<T: serde::de::DeserializeOwned>(
    tier: &GitTier,
    kind: RecordKind,
    corpus_violations: &mut Vec<EvidenceViolation>,
) -> Result<Vec<T>, GateContextError> {
    let read = tier.read(&TierQuery::kind(kind))?;
    corpus_violations.extend(read.violations);
    let mut out = Vec::with_capacity(read.records.len());
    for raw in &read.records {
        match serde_json::from_value::<T>(raw.0.clone()) {
            Ok(record) => out.push(record),
            // `RawRecord` is a bare body with no path, so the subject is
            // the kind itself — enough for an operator to know WHICH
            // corpus is malformed, and this vector is diagnostic input
            // to one check rather than a reported violation set.
            Err(e) => corpus_violations.push(EvidenceViolation::new(
                canon_model::FailureClass::Malformed,
                kind.as_str(),
                format!("{} row does not deserialize: {e}", kind.as_str()),
            )),
        }
    }
    Ok(out)
}

/// Which of the three spec-corpus kinds this repo routes somewhere the
/// gate's [`GitTier`] does not read.
///
/// `GateContext` reads ONE tier — the `local` rung's git root — which is
/// safe for `EvidenceRecord` only by convention. `routing` is per-repo
/// configurable, so a consumer sending `scenario` to `hot` would hand
/// [`crate::spec_coverage`] an empty corpus and get a check that passes
/// because it saw nothing. That is precisely the failure that hid
/// `Task` from `canon report` (`task: hot` routed to a rung with no SQL
/// view), and an inert gate that reports clean is worse than one that
/// refuses.
///
/// A repo with no `canon.yaml`, or one whose routing is absent for a
/// kind, yields nothing here: [`GateCtx::from_repo`] uses the default
/// ledger path in that case, so the git tier IS where those records
/// live. A present but unusable `canon.yaml` is an `Err`, as in
/// [`GateCtx::from_repo`].
fn unreadable_corpus_kinds(repo: &Path) -> Result<Vec<RecordKind>, CanonYamlError> {
    const CORPUS_KINDS: [RecordKind; 3] = [RecordKind::Scenario, RecordKind::Divergence, RecordKind::Subject];
    let Some(tier_policy) = load_tier_policy(repo)? else {
        return Ok(Vec::new());
    };
    Ok(CORPUS_KINDS
        .into_iter()
        .filter(|kind| matches!(tier_policy.routing.get(kind), Some(rung) if *rung != canon_store::policy::Rung::Local))
        .collect())
}

#[derive(Debug, thiserror::Error)]
pub enum GateContextError {
    #[error("canon-store: {0}")]
    Store(#[from] StoreError),
    #[error(transparent)]
    CanonYaml(#[from] CanonYamlError),
}

/// One S5 wave-2 check (static coverage/D3a, dynamic verdict-ledger/
/// D3b, staleness, trust-ladder promotion enforcement, the flag
/// ratchet, checkbox-grammar's evidence gate, …) — a pure function
/// over a loaded [`GateContext`], producing zero or more
/// [`Violation`]s. `canon gate check` (task 1.9) runs every registered
/// `GateCheck` over one production [`GateContext`] and flattens the
/// results; `canon gate selftest` (task 5.2) runs the IDENTICAL trait
/// over a [`GateContext`] loaded from
/// [`GateCtx::from_fixture`] instead of a real repo — no separate
/// check path, matching [`GateCtx`]'s own two-constructor discipline.
///
/// Not implemented against here — S5 wave-2 supplies every concrete
/// `GateCheck` (module doc).
pub trait GateCheck: Send + Sync {
    /// A stable identity for this check (diagnostics, a future `--only
    /// <name>` filter) — distinct from any [`crate::FailureClass`]
    /// string; one check may emit several failure classes.
    fn name(&self) -> &'static str;

    /// Run this check over `ctx`, returning every violation found.
    /// Implementations MUST NOT panic on malformed/unexpected input —
    /// an unexpected shape is itself a violation to report (design
    /// §7), never a crash that takes down the whole gate run.
    fn run(&self, ctx: &GateContext) -> Vec<Violation>;
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    /// A named, fixed UTC constant (s21 design.md R5: never
    /// `Utc::now()` in a test call site of `GateContext::load`) — this
    /// module's tests never assert on time-bearing behavior, so any
    /// fixed instant is equally valid; what matters is that it is NOT
    /// the live clock.
    fn fixed_now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").unwrap().with_timezone(&Utc)
    }

    #[test]
    fn from_repo_defaults_ledger_root_when_no_canon_yaml() {
        let dir = TempDir::new().unwrap();
        let ctx = GateCtx::from_repo(dir.path()).unwrap();
        assert_eq!(ctx.repo, dir.path());
        assert_eq!(ctx.ledger_root, dir.path().join(".canon").join("ledger"));
    }

    #[test]
    fn from_fixture_uses_identical_default_layout() {
        let dir = TempDir::new().unwrap();
        let ctx = GateCtx::from_fixture(dir.path());
        assert_eq!(ctx.ledger_root, dir.path().join(".canon").join("ledger"));
    }

    #[test]
    fn from_repo_honors_local_git_root_override_from_repo_canon_yaml() {
        // The canonical config location is `<repo>/canon.yaml` — the
        // SAME file S2's `TierPolicy`/`canon tier age`/`canon query`
        // resolve the local rung's `root` from (never a
        // `.canon/canon.yaml` gate-only path, review finding).
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("canon.yaml"), "tiers:\n  local:\n    backend: git\n    root: custom/ledger-root\n").unwrap();

        let ctx = GateCtx::from_repo(dir.path()).unwrap();
        assert_eq!(ctx.ledger_root, dir.path().join("custom").join("ledger-root"));
    }

    #[test]
    fn from_repo_ignores_a_dot_canon_canon_yaml_the_wrong_legacy_path() {
        // The config anchor is ALWAYS `<repo>/canon.yaml` at the repo
        // root — `.canon/` holds canon's PRODUCTS, never its config.
        // A stray `.canon/canon.yaml` must never be read; only the root
        // `<repo>/canon.yaml` (S2's canonical `TierPolicy` source) may
        // set `ledger_root`.
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".canon")).unwrap();
        std::fs::write(dir.path().join(".canon").join("canon.yaml"), "tiers:\n  local:\n    backend: git\n    root: custom/ledger-root\n").unwrap();

        let ctx = GateCtx::from_repo(dir.path()).unwrap();
        assert_eq!(ctx.ledger_root, dir.path().join(DEFAULT_LEDGER_RELATIVE_PATH), "a `.canon/canon.yaml` override must NOT be honored");
    }

    /// A present `canon.yaml` the tier parser rejects is refused, naming
    /// the file — never silently resolved to the default ledger.
    #[test]
    fn from_repo_refuses_an_unparseable_canon_yaml() {
        let dir = TempDir::new().unwrap();
        let canon_yaml = dir.path().join("canon.yaml");
        for broken in ["tiers: [unclosed\n", "tiers:\n  git: { root: .canon/ledger }\n"] {
            std::fs::write(&canon_yaml, broken).unwrap();
            let err = GateCtx::from_repo(dir.path()).expect_err(broken);
            assert!(matches!(err, CanonYamlError::Policy { ref path, .. } if *path == canon_yaml), "{err:?}");
            assert!(err.to_string().contains(&canon_yaml.display().to_string()), "{err}");
        }
    }

    /// Even a context not built by `from_repo` refuses the load when the
    /// repo's `canon.yaml` is unusable: the corpus routing check reads it.
    #[test]
    fn load_refuses_an_unparseable_canon_yaml() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("canon.yaml"), "tiers: [unclosed\n").unwrap();
        let result = GateContext::load(GateCtx::from_fixture(dir.path()), &SchemaRegistry::load(), fixed_now());
        assert!(matches!(result, Err(GateContextError::CanonYaml(_))), "an unusable canon.yaml must not load");
    }

    #[test]
    fn load_succeeds_over_an_empty_fixture_with_no_evidence_records() {
        let dir = TempDir::new().unwrap();
        let ctx = GateCtx::from_fixture(dir.path());
        let registry = SchemaRegistry::load();

        let gate_context = GateContext::load(ctx, &registry, fixed_now()).expect("load over an empty fixture must succeed");
        assert!(gate_context.evidence.is_empty());
        assert!(gate_context.violations.is_empty());
        // policy.yaml is also absent from this fixture — resolve()
        // degrades to defaults + a diagnostic, never a load failure.
        assert!(!gate_context.policy.is_clean());
    }

    #[test]
    fn load_reads_a_real_evidence_record_written_through_git_tier() {
        use canon_model::{Actor, Envelope, EvidenceVerdict, RoleId};

        let dir = TempDir::new().unwrap();
        let ctx = GateCtx::from_fixture(dir.path());
        let tier = GitTier::new(&ctx.ledger_root);

        let record = EvidenceRecord::new(
            Envelope::new(1, RecordKind::EvidenceRecord, chrono::Utc::now(), Actor::new("test-agent", RoleId::parse("implementer").unwrap())),
            None,
            None,
            None,
            EvidenceVerdict::Faithful,
        );
        tier.write(&record).expect("write one evidence record through GitTier");

        let registry = SchemaRegistry::load();
        let gate_context = GateContext::load(ctx, &registry, fixed_now()).expect("load over a fixture with one real record");
        assert_eq!(gate_context.evidence.len(), 1);
        assert!(gate_context.violations.is_empty());
        assert_eq!(gate_context.evidence[0].verdict, EvidenceVerdict::Faithful);
    }
}
