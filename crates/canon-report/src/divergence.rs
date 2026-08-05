//! S9 divergence burn-down CURRENT-STATE consumer (s15 P3b, task 4.5,
//! design D8): folds every committed `Divergence` record to its
//! CURRENT [`FoldedState`] per `(project_id, scenario_id)`, via
//! `canon_model::fold_to_current_state` — the pure validator this
//! crate supplies the live-binding re-check + `as_of` input to (design
//! D8: "one validator, two callers" — `canon-gate` is the other,
//! future caller; canon-model itself can never depend on canon-store
//! to fetch this input, so the caller owns fetching, no TOCTOU).
//!
//! # A SEPARATE surface from `mart_review_burndown`
//! `crates/canon-store/sql/views.sql`'s `mart_review_burndown` is a
//! per-day TREND: a running (opened − resolved) total over every raw
//! `Divergence.status` event, ever written. This module answers a
//! DIFFERENT question — "what is the CURRENT state of each
//! `(project_id, scenario_id)` divergence, right now" — which needs
//! `fold_to_current_state`'s `(run_seq, round)`-ranked winner-per-group
//! logic, its `ResolvedInvalid` live-binding re-check, and its
//! `Deferred`-`as_of`-expiry honoring: none of that is expressible as
//! a pure SQL window function, so `crate::marts`'s own "no aggregation
//! happens here, design D1" discipline (a thin wrapper over the SQL
//! view layer) does not apply here — this module IS the aggregation,
//! deliberately outside that constraint.
//!
//! # Fail-soft, mirroring `crate::digest::read_kind`
//! [`current_states`] never returns a `Result` — an unreadable/missing
//! git tier degrades to "zero divergences, zero live bindings" (the
//! identical fail-soft posture `crate::digest::read_kind` already
//! establishes in this crate for the same `GitTier` read), never a
//! panic or a propagated error that would abort report generation.

use std::collections::BTreeMap;
use std::path::Path;

use canon_model::envelope::RecordKind;
use canon_model::fold::{FoldedState, fold_to_current_state};
use canon_model::ids::{ProjectId, ScenarioId};
use canon_model::records::{Divergence, EvidenceRecord};
use canon_store::git_tier::GitTier;
use canon_store::tier::{Tier, TierQuery};
use chrono::{DateTime, Utc};

fn read_kind_fail_soft<T: for<'de> serde::Deserialize<'de>>(git_root: &Path, kind: RecordKind) -> Vec<T> {
    let tier = GitTier::new(git_root);
    let Ok(read) = tier.read(&TierQuery::kind(kind)) else {
        return Vec::new();
    };
    read.records.into_iter().filter_map(|raw| serde_json::from_value(raw.0).ok()).collect()
}

/// The live-binding re-check map [`fold_to_current_state`] needs.
///
/// s44 promoted the derivation into `canon-store` so `canon-gate`'s
/// `spec_coverage` check reads the SAME map — a second copy would let
/// the gate and `canon divergence status` disagree about which
/// scenarios are open, which is precisely what one shared fold exists
/// to prevent. See [`canon_store::fold::live_bindings_of`] for the
/// semantics.
use canon_store::fold::live_bindings_of;

/// Every current `(project_id, scenario_id)` divergence state, folded
/// from `ledger_root`'s committed `Divergence` records with a
/// live-binding re-check derived from its `EvidenceRecord`s (module
/// doc). `as_of` governs `Deferred` expiry ([`fold_to_current_state`]'s
/// own contract) — pass `Utc::now()` for "right now".
pub fn current_states(ledger_root: &Path, as_of: DateTime<Utc>) -> BTreeMap<(ProjectId, ScenarioId), FoldedState> {
    let divergences: Vec<Divergence> = read_kind_fail_soft(ledger_root, RecordKind::Divergence);
    let evidence: Vec<EvidenceRecord> = read_kind_fail_soft(ledger_root, RecordKind::EvidenceRecord);
    let live_bindings = live_bindings_of(evidence);
    fold_to_current_state(&divergences, &live_bindings, as_of)
}

/// A per-[`FoldedState`]-variant count — the CURRENT-STATE burn-down
/// summary (module doc's contrast with `mart_review_burndown`'s
/// per-day TREND).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DivergenceBurndownSummary {
    pub open: usize,
    pub resolved: usize,
    pub still_divergent: usize,
    pub deferred: usize,
    pub resolved_invalid: usize,
}

impl DivergenceBurndownSummary {
    pub fn total(&self) -> usize {
        self.open + self.resolved + self.still_divergent + self.deferred + self.resolved_invalid
    }
}

/// Summarizes [`current_states`]'s output into per-variant counts.
pub fn summarize(states: &BTreeMap<(ProjectId, ScenarioId), FoldedState>) -> DivergenceBurndownSummary {
    let mut summary = DivergenceBurndownSummary::default();
    for state in states.values() {
        match state {
            FoldedState::Open => summary.open += 1,
            FoldedState::Resolved => summary.resolved += 1,
            FoldedState::StillDivergent => summary.still_divergent += 1,
            FoldedState::Deferred { .. } => summary.deferred += 1,
            FoldedState::ResolvedInvalid => summary.resolved_invalid += 1,
        }
    }
    summary
}

#[cfg(test)]
mod tests {
    use canon_model::envelope::{Actor, Envelope};
    use canon_model::ids::{RoleId, Sha, TotalOrder};
    use canon_model::records::{DivergenceStatus, EvidenceVerdict};
    use canon_store::tier::Tier;
    use tempfile::TempDir;

    use super::*;

    fn actor(role: &str) -> Actor {
        Actor::new("selftest-agent", RoleId::parse(role).unwrap())
    }

    // A `#[cfg(test)]` fixture builder mirroring `Divergence`'s own
    // field set — the arg count is inherent to the record it
    // constructs, not a design smell worth a params struct here.
    #[allow(clippy::too_many_arguments)]
    fn write_divergence(tier: &GitTier, project_id: &str, scenario_id: &str, sha: &str, status: DivergenceStatus, run_seq: u64, round: u32, at: DateTime<Utc>) {
        let divergence = Divergence::new(
            Envelope::new(1, RecordKind::Divergence, at, actor("reviewer")),
            ProjectId::parse(project_id).unwrap(),
            ScenarioId::parse(scenario_id).unwrap(),
            Sha::parse(sha).unwrap(),
            status,
            TotalOrder::new(run_seq),
            round,
            "reviewer-1",
            "",
        );
        tier.write(&divergence).expect("write fixture divergence");
    }

    fn write_evidence(tier: &GitTier, project_id: &str, scenario_id: &str, sha: &str, at: DateTime<Utc>) {
        let record = EvidenceRecord::new(Envelope::new(1, RecordKind::EvidenceRecord, at, actor("implementer")), None, Some(ScenarioId::parse(scenario_id).unwrap()), None, EvidenceVerdict::Faithful)
            .with_project_id(ProjectId::parse(project_id).unwrap())
            .with_evidence_sha(Sha::parse(sha).unwrap());
        tier.write(&record).expect("write fixture evidence");
    }

    #[test]
    fn a_stale_resolved_binding_downgrades_to_resolved_invalid() {
        let dir = TempDir::new().unwrap();
        let ledger_root = dir.path().join("canon").join("ledger");
        let tier = GitTier::new(&ledger_root);

        let resolved_at = Utc::now() - chrono::Duration::hours(2);
        write_divergence(&tier, "app-a", "world.firstbuy-hotdeal.14", &"a".repeat(40), DivergenceStatus::Resolved, 1, 1, resolved_at);
        // The LIVE evidence for this scenario now carries a DIFFERENT
        // sha than what the divergence resolved against — a genuine
        // mismatch the re-check must catch.
        write_evidence(&tier, "app-a", "world.firstbuy-hotdeal.14", &"b".repeat(40), Utc::now());

        let states = current_states(&ledger_root, Utc::now());
        let key = (ProjectId::parse("app-a").unwrap(), ScenarioId::parse("world.firstbuy-hotdeal.14").unwrap());
        assert_eq!(states.get(&key), Some(&FoldedState::ResolvedInvalid), "states: {states:?}");

        let summary = summarize(&states);
        assert_eq!(summary.resolved_invalid, 1);
        assert_eq!(summary.total(), 1);
    }

    #[test]
    fn a_matching_app_sha_stays_resolved_despite_a_different_evidence_author_and_time() {
        let dir = TempDir::new().unwrap();
        let ledger_root = dir.path().join("canon").join("ledger");
        let tier = GitTier::new(&ledger_root);

        let resolved_at = Utc::now() - chrono::Duration::hours(2);
        write_divergence(&tier, "app-a", "world.firstbuy-hotdeal.14", &"a".repeat(40), DivergenceStatus::Resolved, 1, 1, resolved_at);
        // Live evidence carries the SAME app sha the divergence resolved
        // against, but a DIFFERENT author (`selftest-agent` vs the
        // divergence's `reviewer-1`) at a LATER time — neither may trigger
        // a false ResolvedInvalid: app sha is the sole live-checkable axis.
        write_evidence(&tier, "app-a", "world.firstbuy-hotdeal.14", &"a".repeat(40), Utc::now());

        let states = current_states(&ledger_root, Utc::now());
        let key = (ProjectId::parse("app-a").unwrap(), ScenarioId::parse("world.firstbuy-hotdeal.14").unwrap());
        assert_eq!(states.get(&key), Some(&FoldedState::Resolved), "same app sha must stay Resolved: {states:?}");
    }

    #[test]
    fn a_resolved_binding_with_no_live_recheck_input_is_trusted_as_is() {
        let dir = TempDir::new().unwrap();
        let ledger_root = dir.path().join("canon").join("ledger");
        let tier = GitTier::new(&ledger_root);

        write_divergence(&tier, "app-a", "world.firstbuy-hotdeal.26", &"a".repeat(40), DivergenceStatus::Resolved, 1, 1, Utc::now());
        // No EvidenceRecord at all for this scenario -> no live-binding
        // re-check entry -> Resolved is trusted as-is.

        let states = current_states(&ledger_root, Utc::now());
        let key = (ProjectId::parse("app-a").unwrap(), ScenarioId::parse("world.firstbuy-hotdeal.26").unwrap());
        assert_eq!(states.get(&key), Some(&FoldedState::Resolved));
    }

    #[test]
    fn deferred_honors_as_of_expiry() {
        let dir = TempDir::new().unwrap();
        let ledger_root = dir.path().join("canon").join("ledger");
        let tier = GitTier::new(&ledger_root);

        let expiry = Utc::now() + chrono::Duration::days(1);
        let divergence = Divergence::new(
            Envelope::new(1, RecordKind::Divergence, Utc::now(), actor("reviewer")),
            ProjectId::parse("app-a").unwrap(),
            ScenarioId::parse("world.firstbuy-hotdeal.33").unwrap(),
            Sha::parse("a".repeat(40)).unwrap(),
            DivergenceStatus::Deferred { reason: "waiting on upstream".to_string(), expiry },
            TotalOrder::new(1),
            1,
            "reviewer-1",
            "",
        );
        tier.write(&divergence).unwrap();

        let key = (ProjectId::parse("app-a").unwrap(), ScenarioId::parse("world.firstbuy-hotdeal.33").unwrap());

        // Before expiry: still deferred.
        let before = current_states(&ledger_root, expiry - chrono::Duration::hours(1));
        assert!(matches!(before.get(&key), Some(FoldedState::Deferred { .. })), "before: {before:?}");

        // After expiry: resurfaces as still-divergent (never silently resolved).
        let after = current_states(&ledger_root, expiry + chrono::Duration::hours(1));
        assert_eq!(after.get(&key), Some(&FoldedState::StillDivergent), "after: {after:?}");
    }
}
