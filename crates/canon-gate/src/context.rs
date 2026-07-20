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

use canon_model::{validate_evidence_batch, EvidenceRecord, EvidenceViolation, RecordKind};
use canon_policy::SchemaRegistry;
use canon_store::git_tier::GitTier;
use canon_store::tier::{StoreError, Tier, TierQuery};
use chrono::{DateTime, Utc};

use crate::policy::PolicyResolution;
use crate::Violation;

/// The infra-layout doc's fixed ledger location relative to a repo
/// root (`docs/superpowers/specs/2026-07-10-canon-design.md`:
/// `<repo>/canon/ledger/ # Hive: kind=<kind>/area=<area>/*.json —
/// append-only`) — the [`canon_store::git_tier::GitTier`] root
/// [`GateCtx::from_repo`] defaults to when the canonical
/// `<repo>/canon.yaml` (S2's [`canon_store::policy::TierPolicy`]
/// source of truth) declares no `tiers.git.root` override.
pub const DEFAULT_LEDGER_RELATIVE_PATH: &str = "canon/ledger";

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
    pub fn from_repo(repo: impl Into<PathBuf>) -> Self {
        let repo = repo.into();
        let ledger_root = canon_yaml_git_root(&repo).unwrap_or_else(|| repo.join(DEFAULT_LEDGER_RELATIVE_PATH));
        Self { repo, ledger_root }
    }

    /// Fixture binding: every root under one fixture directory
    /// (fixtures-selftest.md §3.1's `fixture_ctx(fx)` — "binds EVERY
    /// `GateCtx` field into one fixture directory"). A fixture never
    /// reads `<repo>/canon.yaml`; `ledger_root` is always
    /// `fixture_dir/canon/ledger`, the identical layout
    /// [`GateCtx::from_repo`]'s default uses.
    pub fn from_fixture(fixture_dir: impl Into<PathBuf>) -> Self {
        let repo = fixture_dir.into();
        let ledger_root = repo.join(DEFAULT_LEDGER_RELATIVE_PATH);
        Self { repo, ledger_root }
    }
}

/// Best-effort canonical `<repo>/canon.yaml` → the `local` rung's
/// git root resolution — reuses
/// [`canon_store::policy::TierPolicy::from_yaml`] (S2's own parser,
/// never a second hand-rolled YAML reader) against the SAME on-disk
/// file S2's `TierPolicy`/`canon tier age`/`canon query` resolve the
/// local rung's `root` from (`crates/canon-cli/src/tiers.rs::
/// build_tiers`'s identical `<canon.yaml's own dir>.join(&cfg.root)`
/// semantics), so a consumer's override is honored identically for
/// `canon gate` and every S2 CLI path. `None` on any problem (file
/// absent, unparseable, no git-backed `local` rung) —
/// [`GateCtx::from_repo`]'s caller always has a usable fallback,
/// matching this crate's fail-soft-load discipline
/// ([`crate::policy`]'s module doc).
fn canon_yaml_git_root(repo: &Path) -> Option<PathBuf> {
    let canon_yaml_path = repo.join("canon.yaml");
    let content = std::fs::read_to_string(canon_yaml_path).ok()?;
    let tier_policy = canon_store::policy::TierPolicy::from_yaml(&content).ok()?;
    let root = tier_policy.local_git()?.root.clone();
    Some(if root.is_absolute() { root } else { repo.join(root) })
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
    pub violations: Vec<EvidenceViolation>,
    pub now: DateTime<Utc>,
}

impl GateContext {
    /// Load everything an S5 wave-2 check needs: resolve `policy.yaml`
    /// ([`PolicyResolution::resolve`]) and read every
    /// `EvidenceRecord` off `ctx.ledger_root`'s [`GitTier`]
    /// (canon-store, S2). Fails only on a [`StoreError`] the tier
    /// itself cannot recover from (e.g. an unreadable ledger root) —
    /// per-record malformed content is never an `Err` here, it lands
    /// in `violations` (module doc). `now` is REQUIRED, never
    /// defaulted to the live clock (s21 `deterministic-gate-clock`):
    /// the CLI dispatch boundary (`canon-cli/src/gate.rs`) is the ONE
    /// place `Utc::now()` is ever called for a gate run, exactly once,
    /// and threads the result in here.
    pub fn load(ctx: GateCtx, registry: &SchemaRegistry, now: DateTime<Utc>) -> Result<Self, GateContextError> {
        let policy = PolicyResolution::resolve(&ctx.repo, registry);

        let tier = GitTier::new(&ctx.ledger_root);
        let read = tier.read(&TierQuery::kind(RecordKind::EvidenceRecord))?;
        let (evidence, validation_violations) = validate_evidence_batch(&read.records);

        let mut violations = read.violations;
        violations.extend(validation_violations);

        Ok(Self { ctx, policy, evidence, violations, now })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GateContextError {
    #[error("canon-store: {0}")]
    Store(#[from] StoreError),
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
        let ctx = GateCtx::from_repo(dir.path());
        assert_eq!(ctx.repo, dir.path());
        assert_eq!(ctx.ledger_root, dir.path().join("canon").join("ledger"));
    }

    #[test]
    fn from_fixture_uses_identical_default_layout() {
        let dir = TempDir::new().unwrap();
        let ctx = GateCtx::from_fixture(dir.path());
        assert_eq!(ctx.ledger_root, dir.path().join("canon").join("ledger"));
    }

    #[test]
    fn from_repo_honors_local_git_root_override_from_repo_canon_yaml() {
        // The canonical config location is `<repo>/canon.yaml` — the
        // SAME file S2's `TierPolicy`/`canon tier age`/`canon query`
        // resolve the local rung's `root` from (never a
        // `.canon/canon.yaml` gate-only path, review finding).
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("canon.yaml"), "tiers:\n  local:\n    backend: git\n    root: custom/ledger-root\n").unwrap();

        let ctx = GateCtx::from_repo(dir.path());
        assert_eq!(ctx.ledger_root, dir.path().join("custom").join("ledger-root"));
    }

    #[test]
    fn from_repo_ignores_a_dot_canon_canon_yaml_the_wrong_legacy_path() {
        // A stray `.canon/canon.yaml` (the OLD, incorrect location this
        // fix removes) must never be read — only `<repo>/canon.yaml`
        // (S2's canonical `TierPolicy` source) may set `ledger_root`.
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".canon")).unwrap();
        std::fs::write(dir.path().join(".canon").join("canon.yaml"), "tiers:\n  local:\n    backend: git\n    root: custom/ledger-root\n").unwrap();

        let ctx = GateCtx::from_repo(dir.path());
        assert_eq!(ctx.ledger_root, dir.path().join(DEFAULT_LEDGER_RELATIVE_PATH), "a `.canon/canon.yaml` override must NOT be honored");
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
