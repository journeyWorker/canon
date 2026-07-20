//! S8 (`retrieve-before-task`) guidance layer (design D1/D2/D3):
//! [`retrieve_guidance`], the fail-soft-at-the-type-level wrapper over S6's
//! [`crate::retrieve::retrieve`], and [`manifest_guidance_for_replay`], the
//! named replay/live-retrieval boundary that returns a recorded
//! [`canon_model::Run::injected_guidance`] snapshot unchanged. Generalizes
//! the donor tuning project's `evaluatePreSweepLookup`/`manifestGuidanceForReplay` pair
//! and the donor harness's fail-soft pre-edit pattern-lookup
//! PreToolUse hook into one role-agnostic library surface any dispatch
//! caller can use (`openspec/changes/s8-retrieve-before-task/design.md`).
//!
//! **Reconciles with S6's actual shipped surface, not the design doc's
//! aspirational one:** the design doc's decision 1 names
//! `search_similar_strategies`; S6 shipped exact-`regime_key`-match
//! [`crate::retrieve::retrieve`] instead (OQ2's parquet-first pivot
//! deferred vector/similarity search — `crate::store` module doc). This
//! module wraps the function S6 actually shipped, never a nonexistent
//! one, and reuses S6's `regime_key` grammar verbatim — no second key
//! derivation (design decision 1's own "the exact join-spine failure
//! design doc §1 exists to prevent").
//!
//! `canon retrieve` (the CLI surface) and the pre-dispatch hook wiring
//! that populates `injected_guidance` at real dispatch time are S8 part2
//! — deferred; this module is the library core only (fail-soft retrieve +
//! the replay boundary), consumed by whatever dispatch surface part2
//! wires up.
//!
//! # s36 hierarchical fallback + the subject→domain consolidation contract
//! [`retrieve_first_nonempty`] layers an ordered, fail-soft fallback on
//! top of [`retrieve_guidance`] for the subject-domain loop (s36
//! `subject-domain-loop`): it tries a caller-derived candidate list of
//! `regime_key`s in order and serves the FIRST non-empty one. The
//! candidate order the CLI builds encodes the hierarchy IN the fixed
//! four-segment grammar's `<area>` slot — `<domain>-<subject_id>` (the
//! subject-scoped namespace) tried before the bare `<domain>` (the
//! domain-scoped namespace) — never a fifth segment or a re-parsed
//! nested key (the grammar stays exactly `<role>/<repo>/<area>/<hash>`;
//! retrieval always DERIVES candidates from the structured `--domain`/
//! `--subject` inputs, so the in-segment encoding is written, never
//! parsed back).
//!
//! Why the domain fallback is load-bearing: a subject is finite, so its
//! lessons must outlive it. On a subject's `shipped`/`retired`
//! transition the s36 loop runs a CONSOLIDATION pass — an agent skill
//! re-distills the still-valid `<domain>-<subject_id>` strategies up to
//! the `<domain>` area through the EXISTING promote flow (`canon learn
//! promote`, [`crate::promotion::promote_strategy`]) — the L3→L5
//! promotion analog (proposal "on shipped/retired, a consolidation pass
//! promotes still-valid subject-scoped strategies to the domain
//! level"). This crate ships NO new code path for that pass: it is the
//! ordinary promote flow re-targeted at the `<domain>` regime, driven by
//! an agent through the CLI, never an LLM call inside canon. Once
//! consolidated, a later query for a retired subject finds nothing at
//! `<domain>-<subject_id>` and this helper's fallback serves the
//! consolidated `<domain>` guidance instead — the subject's lessons
//! survive its retirement.

use canon_model::ids::{RegimeKey, RoleId};
use canon_model::{Run, StrategyRef};

use crate::store::StrategyStore;

/// Default top-k cap when a caller supplies `k: None` — mirrors S6's own
/// search default (design doc Risk section: "cap `--k` (default matches
/// S6's search default, e.g. 5)").
pub const DEFAULT_K: usize = 5;

/// Fail-soft-at-the-type-level retrieval (design decision 3): the public
/// signature returns `Vec<StrategyRef>` — NEVER a `Result` — so a store
/// outage, timeout, or malformed row can only ever produce an empty
/// guidance list, logged internally, never a typed failure a caller could
/// accidentally propagate into a blocking failure. Mirrors
/// `evaluatePreSweepLookup`'s "always resolves, never fails the enclosing
/// sweep" contract at the Rust type level instead of via an
/// `Effect.catchAllDefect`-equivalent combinator.
///
/// Internally wraps [`crate::retrieve::retrieve`] — S6's exact-
/// `regime_key`-match search (NOT a similarity search; see this module's
/// own doc comment) — fetching every stored item for `regime_key` (no
/// `limit`, since demoted items must be filtered out BEFORE `k` is
/// applied — passing `k` straight through as the store-level limit could
/// undercount when some of the top-`k` freshest rows are demoted),
/// EXCLUDES any [`crate::strategy::StrategyItem`] carrying
/// `demotion.is_some()` (S7's demotion contract restated as a hard
/// read-side requirement, design decision per the proposal's "Depends on
/// S7's demotion contract"), then caps the result at `k` (`k: None`
/// defaults to [`DEFAULT_K`]).
///
/// `role` is a caller-contract check, not a second filter:
/// `regime_key` already embeds `role` as its leading segment (S6 design
/// decision 2), so `regime_key` alone is the actual scoping mechanism —
/// `role` is `debug_assert_eq!`-checked against it (mirrors
/// [`canon_model::records::Run::new`]'s own `debug_assert_eq!` pattern),
/// catching a caller-side mismatch in debug builds while never adding a
/// panic path to a release binary this function's own contract forbids.
///
/// Every error path — [`crate::store::StrategyStore::query_by_regime_key`]
/// failing (store outage, I/O failure, a malformed on-disk row) — is
/// caught here and logged via `eprintln!` (this crate has no `tracing`
/// dependency; `eprintln!`-for-non-fatal-diagnostics is the established
/// convention across `canon-cli`/`canon-ingest`), then converted to an
/// empty `Vec` — never propagated, never a panic.
pub fn retrieve_guidance(store: &dyn StrategyStore, role: &RoleId, regime_key: &RegimeKey, k: Option<usize>) -> Vec<StrategyRef> {
    debug_assert_eq!(
        regime_key.role(),
        role.as_str(),
        "retrieve_guidance: role {:?} does not match regime_key {regime_key}'s own role segment",
        role
    );
    let k = k.unwrap_or(DEFAULT_K);

    let items = match crate::retrieve::retrieve(store, regime_key, None) {
        Ok(items) => items,
        Err(err) => {
            eprintln!(
                "canon-learn retrieve_guidance: store error for role {role} regime_key {regime_key} — returning empty guidance (fail-soft): {err}"
            );
            return Vec::new();
        }
    };

    items
        .into_iter()
        .filter(|item| item.demotion.is_none())
        .take(k)
        .map(|item| StrategyRef::new(item.id.to_string(), item.title, item.content))
        .collect()
}

/// Ordered, fail-soft fallback retrieval over a candidate list (s36
/// `subject-domain-loop`, module doc): tries each `regime_key` in
/// `candidates` in order via [`retrieve_guidance`] and returns the
/// FIRST non-empty guidance set together with the candidate that served
/// it. The candidate order IS the fallback hierarchy the caller derived
/// (`<domain>-<subject_id>` before `<domain>`) — this helper owns only
/// "try in order, stop at the first hit", never the derivation (the CLI
/// builds the candidates from structured `--domain`/`--subject` inputs;
/// module doc).
///
/// Deterministic and fail-soft to EXACTLY the degree [`retrieve_guidance`]
/// is: every per-candidate lookup rides that function's `Vec`-not-`Result`
/// contract, so a store outage, a malformed on-disk row, or an absent
/// namespace makes a candidate merely LOOK empty and the search advances
/// to the next — never an error, never a panic. When every candidate is
/// empty (or `candidates` is itself empty), the result is
/// `(Vec::new(), None)`: the same "no guidance" degrade
/// [`retrieve_guidance`] returns, with `None` distinguishing an
/// all-empty search from a first-candidate hit so a caller never
/// misreports which regime served.
///
/// `role` is the SAME caller-contract check [`retrieve_guidance`] makes
/// against each candidate's own leading segment (S6 design decision 2);
/// every candidate is expected to share `role`, since the CLI derives
/// them all from one `--role`.
pub fn retrieve_first_nonempty<'a>(
    store: &dyn StrategyStore,
    role: &RoleId,
    candidates: &'a [RegimeKey],
    k: Option<usize>,
) -> (Vec<StrategyRef>, Option<&'a RegimeKey>) {
    for candidate in candidates {
        let guidance = retrieve_guidance(store, role, candidate, k);
        if !guidance.is_empty() {
            return (guidance, Some(candidate));
        }
    }
    (Vec::new(), None)
}

/// The named replay/live-retrieval boundary (design decision 2): returns
/// `run.injected_guidance` UNCHANGED — never a fresh [`retrieve_guidance`]
/// call, ever. A replay of a run's manifest MUST call this function, never
/// `retrieve_guidance` directly, so a live store change (a strategy
/// added, edited, demoted, or removed after the original run) can never
/// perturb a replay's guidance input — the manifest's recorded snapshot
/// IS the replay input, unconditionally. Giving the replay path its own
/// name (rather than scattering `run.injected_guidance` reads at every
/// replay call site) keeps that call-site distinction from being silently
/// blurred — the same rationale the donor's own module doc gives for its
/// analogous split, quoted verbatim in the design doc.
///
/// Takes `&Run` (never `&dyn StrategyStore`) — there is no live lookup
/// this function COULD perform even if a caller wanted one; that absence
/// is itself the type-level proof of "replay never calls canon retrieve".
pub fn manifest_guidance_for_replay(run: &Run) -> Vec<StrategyRef> {
    run.injected_guidance.clone()
}

#[cfg(test)]
mod tests {
    use canon_model::envelope::{Actor, Envelope, RecordKind};
    use canon_model::ids::RunId;
    use canon_model::records::RunStatus;
    use chrono::{DateTime, Duration, Utc};

    use super::*;
    use crate::error::LearnError;
    use crate::ids::{StrategyId, TrajectoryId};
    use crate::store::ParquetStrategyStore;
    use crate::strategy::{DemotionEvidence, StrategyItem};

    fn role(r: &str) -> RoleId {
        RoleId::parse(r).unwrap()
    }

    fn regime(r: &str) -> RegimeKey {
        RegimeKey::parse(canon_model::ids::regime_key(r, "repo", "auth", "abc123")).unwrap()
    }

    fn strategy_at(r: &str, title: &str, at: DateTime<Utc>) -> StrategyItem {
        StrategyItem::new(StrategyId::new(), regime(r), role(r), title, "d", format!("content-{title}"), vec![TrajectoryId::new()], at)
    }

    /// A synthetic [`StrategyStore`] whose every read fails — simulates a
    /// store outage (design decision 3's fail-soft contract) without any
    /// real I/O.
    struct AlwaysFailsStore;

    impl StrategyStore for AlwaysFailsStore {
        fn append(&self, _item: &StrategyItem) -> Result<(), LearnError> {
            Err(LearnError::Parquet("simulated store outage".into()))
        }
        fn query_by_regime_key(&self, _regime_key: &RegimeKey) -> Result<Vec<StrategyItem>, LearnError> {
            Err(LearnError::Parquet("simulated store outage".into()))
        }
        fn delete_for_regime_key(&self, _regime_key: &RegimeKey) -> Result<usize, LearnError> {
            Err(LearnError::Parquet("simulated store outage".into()))
        }
        fn find_by_id(&self, _id: &StrategyId) -> Result<Option<StrategyItem>, LearnError> {
            Err(LearnError::Parquet("simulated store outage".into()))
        }
        fn mark_demoted(&self, _id: &StrategyId, _demotion: DemotionEvidence) -> Result<(), LearnError> {
            Err(LearnError::Parquet("simulated store outage".into()))
        }
    }

    #[test]
    fn retrieve_guidance_returns_top_k_role_and_regime_snapshots() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetStrategyStore::open(dir.path());
        let now = Utc::now();
        for i in 0..5i64 {
            store.append(&strategy_at("dev", &format!("s{i}"), now - Duration::minutes(4 - i))).unwrap();
        }
        // A different role/regime — must never leak into a `dev`-scoped result.
        store.append(&strategy_at("content", "other-role", now)).unwrap();

        let guidance = retrieve_guidance(&store, &role("dev"), &regime("dev"), Some(3));

        assert_eq!(guidance.len(), 3, "k=3 must cap the result at 3");
        assert_eq!(
            guidance.iter().map(|g| g.title.as_str()).collect::<Vec<_>>(),
            vec!["s4", "s3", "s2"],
            "must be the top-3 newest dev-role strategies, most-recent first"
        );
        for g in &guidance {
            assert_eq!(g.content, format!("content-{}", g.title), "StrategyRef must be a full content snapshot, not just an id");
            assert!(!g.strategy_id.is_empty());
        }
        assert!(guidance.iter().all(|g| g.title != "other-role"), "content-role strategy must never appear in a dev-role retrieval");
    }

    #[test]
    fn retrieve_guidance_defaults_k_to_five_when_none_is_given() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetStrategyStore::open(dir.path());
        let now = Utc::now();
        for i in 0..7i64 {
            store.append(&strategy_at("dev", &format!("s{i}"), now - Duration::minutes(i))).unwrap();
        }

        let guidance = retrieve_guidance(&store, &role("dev"), &regime("dev"), None);

        assert_eq!(guidance.len(), DEFAULT_K);
    }

    #[test]
    fn retrieve_guidance_excludes_a_demoted_strategy() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetStrategyStore::open(dir.path());
        let now = Utc::now();
        let s0 = strategy_at("dev", "s0", now - Duration::minutes(2));
        let s1 = strategy_at("dev", "s1", now - Duration::minutes(1));
        let s2 = strategy_at("dev", "s2", now);
        store.append(&s0).unwrap();
        store.append(&s1).unwrap();
        store.append(&s2).unwrap();

        store.mark_demoted(&s1.id, DemotionEvidence::new(TrajectoryId::new(), "contradicting failure", now)).unwrap();

        let guidance = retrieve_guidance(&store, &role("dev"), &regime("dev"), None);

        assert_eq!(guidance.iter().map(|g| g.title.as_str()).collect::<Vec<_>>(), vec!["s2", "s0"], "the demoted s1 must be excluded");
    }

    #[test]
    fn retrieve_guidance_on_a_store_outage_returns_an_empty_vec_never_panics_or_errors() {
        // The signature itself (`Vec<StrategyRef>`, no `Result`) already
        // proves this at the type level — this test proves the runtime
        // behavior on top of that: a failing store degrades to empty,
        // not a panic or a propagated error.
        let guidance = retrieve_guidance(&AlwaysFailsStore, &role("dev"), &regime("dev"), None);
        assert!(guidance.is_empty());
    }

    fn envelope(kind: RecordKind) -> Envelope {
        Envelope::new(1, kind, Utc::now(), Actor::new("codex-cli", canon_model::ids::RoleId::parse("implementer").unwrap()))
    }

    #[test]
    fn manifest_guidance_for_replay_returns_the_recorded_snapshot_verbatim_even_after_the_source_is_demoted() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetStrategyStore::open(dir.path());
        let now = Utc::now();
        let s0 = strategy_at("dev", "s0", now);
        store.append(&s0).unwrap();

        // Dispatch time: retrieve, then record into the manifest.
        let guidance_at_dispatch = retrieve_guidance(&store, &role("dev"), &regime("dev"), None);
        assert_eq!(guidance_at_dispatch.len(), 1);
        let run = Run::new(envelope(RecordKind::Run), RunId::new(), None, None, RunStatus::Succeeded, now, Some(now))
            .with_injected_guidance(guidance_at_dispatch.clone());

        // Later: the source strategy is demoted.
        store.mark_demoted(&s0.id, DemotionEvidence::new(TrajectoryId::new(), "contradicting failure", now)).unwrap();

        // A NEW retrieval no longer includes it (live half of the guarantee).
        let guidance_after_demotion = retrieve_guidance(&store, &role("dev"), &regime("dev"), None);
        assert!(guidance_after_demotion.is_empty(), "a fresh retrieval must exclude the now-demoted strategy");

        // The manifest's replay still reproduces the ORIGINAL snapshot,
        // byte-identically, unaffected by the demotion (replay half).
        let replayed = manifest_guidance_for_replay(&run);
        assert_eq!(replayed, guidance_at_dispatch, "replay must reproduce the originally-recorded snapshot verbatim, demotion notwithstanding");
    }

    #[test]
    fn an_old_manifest_without_injected_guidance_replays_empty() {
        let now = Utc::now();
        let run = Run::new(envelope(RecordKind::Run), RunId::new(), None, None, RunStatus::Succeeded, now, Some(now));
        assert_eq!(manifest_guidance_for_replay(&run), Vec::new());
    }

    /// A `regime_key` for `role` in a named `area` (the s36 fallback
    /// candidates differ ONLY in their `<area>` segment).
    fn regime_in(r: &str, area: &str) -> RegimeKey {
        RegimeKey::parse(canon_model::ids::regime_key(r, "repo", area, "abc123")).unwrap()
    }

    fn strategy_in(rk: &RegimeKey, title: &str, at: DateTime<Utc>) -> StrategyItem {
        StrategyItem::new(StrategyId::new(), rk.clone(), role(rk.role()), title, "d", format!("content-{title}"), vec![TrajectoryId::new()], at)
    }

    #[test]
    fn retrieve_first_nonempty_serves_the_first_candidate_when_it_is_nonempty() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetStrategyStore::open(dir.path());
        let now = Utc::now();
        let subject = regime_in("dev", "planning-my-subject");
        let domain = regime_in("dev", "planning");
        // BOTH namespaces are populated: order must pick the subject one.
        store.append(&strategy_in(&subject, "subject-scoped", now)).unwrap();
        store.append(&strategy_in(&domain, "domain-scoped", now)).unwrap();

        let candidates = [subject.clone(), domain];
        let (guidance, serving) = retrieve_first_nonempty(&store, &role("dev"), &candidates, None);

        assert_eq!(guidance.iter().map(|g| g.title.as_str()).collect::<Vec<_>>(), vec!["subject-scoped"]);
        assert_eq!(serving, Some(&subject), "the first non-empty candidate must serve, never a later one");
    }

    #[test]
    fn retrieve_first_nonempty_falls_back_to_the_next_candidate_when_the_first_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetStrategyStore::open(dir.path());
        let now = Utc::now();
        let subject = regime_in("dev", "planning-my-subject");
        let domain = regime_in("dev", "planning");
        // Only the DOMAIN namespace is populated — the subject one is empty.
        store.append(&strategy_in(&domain, "domain-scoped", now)).unwrap();

        let candidates = [subject, domain.clone()];
        let (guidance, serving) = retrieve_first_nonempty(&store, &role("dev"), &candidates, None);

        assert_eq!(guidance.iter().map(|g| g.title.as_str()).collect::<Vec<_>>(), vec!["domain-scoped"]);
        assert_eq!(serving, Some(&domain), "an empty subject namespace must fall back to the domain candidate");
    }

    #[test]
    fn retrieve_first_nonempty_returns_empty_and_none_when_every_candidate_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetStrategyStore::open(dir.path());
        let candidates = [regime_in("dev", "planning-my-subject"), regime_in("dev", "planning")];

        let (guidance, serving) = retrieve_first_nonempty(&store, &role("dev"), &candidates, None);

        assert!(guidance.is_empty());
        assert_eq!(serving, None, "an all-empty search reports None, never a spurious first-candidate hit");
    }

    #[test]
    fn retrieve_first_nonempty_on_a_store_outage_returns_empty_and_none_never_panics() {
        // Every candidate lookup fails (store outage) — fail-soft to the
        // same degree `retrieve_guidance` is: empty result, `None`
        // serving, no panic, no propagated error.
        let candidates = [regime_in("dev", "planning-my-subject"), regime_in("dev", "planning")];
        let (guidance, serving) = retrieve_first_nonempty(&AlwaysFailsStore, &role("dev"), &candidates, None);
        assert!(guidance.is_empty());
        assert_eq!(serving, None);
    }

    #[test]
    fn retrieve_first_nonempty_over_no_candidates_is_empty_and_none() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetStrategyStore::open(dir.path());
        let (guidance, serving) = retrieve_first_nonempty(&store, &role("dev"), &[], None);
        assert!(guidance.is_empty());
        assert_eq!(serving, None);
    }
}
