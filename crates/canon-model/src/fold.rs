//! `fold_to_current_state` (design D8, s15 `native-verdict-lifecycle`
//! spec, task 1.6) — the PURE `Divergence` fold-to-current-state
//! function. Groups records by `(project_id, scenario_id)`, ranks
//! within a group by `run_seq: TotalOrder` as the SOLE primary key
//! (`round` is a tiebreak ONLY among equal `run_seq` values, never an
//! independent `Ord` axis), and derives [`FoldedState::ResolvedInvalid`]
//! from a caller-supplied live-binding re-check — never by re-fetching
//! internally, since `canon-model` cannot depend on `canon-store`
//! (design D8: "one validator, two callers", `canon-gate`/`canon-report`
//! own fetching).

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};

use crate::ids::{ProjectId, ScenarioId, Sha};
use crate::records::{Divergence, DivergenceStatus};

/// A live re-check snapshot for one `(project_id, scenario_id)` fold
/// group — "what app state the ledger reflects RIGHT NOW" for the
/// scenario a `Divergence` resolution is bound to. Fetched by the CALLER
/// (`canon-gate`/`canon-report` own I/O) and passed into
/// [`fold_to_current_state`] as an INPUT — never re-fetched internally,
/// so there is no time-of-check/time-of-use gap.
///
/// The ONLY live-checkable axis is the app state (`app_sha`): a
/// `Resolved` divergence is bound to the app state (`Divergence.sha`) it
/// resolved against, and becomes stale iff the scenario's CURRENT app
/// state has moved off that sha. WHO resolved it and WHEN
/// (`Divergence.reviewer`/`envelope.at`) are the divergence record's own
/// IMMUTABLE provenance — not live-checkable, and a superseding
/// resolution is already handled by `run_seq` ranking, so they are NOT
/// part of this comparison. `reserved_digest` is a RESERVED, non-semantic
/// field (default `None`, never consulted today) for a future binding
/// source (e.g. a paired `Review` content digest) — setting it alone
/// NEVER triggers `ResolvedInvalid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingSnapshot {
    pub app_sha: Sha,
    pub reserved_digest: Option<String>,
}

impl BindingSnapshot {
    /// Whether this LIVE snapshot still agrees with what `resolved` (a
    /// `Divergence` whose `status` is `Resolved`) bound to — the
    /// scenario's current app state (`app_sha`) must still equal the app
    /// state the divergence resolved against (`resolved.sha`), or the
    /// resolution is stale (its app state moved on).
    fn still_matches(&self, resolved: &Divergence) -> bool {
        self.app_sha == resolved.sha
    }
}

/// The fold's OUTPUT state for one `(project_id, scenario_id)` group —
/// deliberately a SEPARATE type from [`DivergenceStatus`] (design D8/D9):
/// `ResolvedInvalid` is fold-DERIVED from a live-binding re-check and
/// must never be reachable as a persisted `DivergenceStatus` variant —
/// the on-disk `Divergence` record is never rewritten to reflect a
/// downgrade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FoldedState {
    Open,
    Resolved,
    StillDivergent,
    Deferred { reason: String, expiry: DateTime<Utc> },
    /// The group's winning record claims `Resolved`, but the supplied
    /// `live_bindings` re-check shows the CURRENT ledger's binding no
    /// longer matches what was resolved (design D8's no-TOCTOU
    /// re-check) — derivable ONLY at fold time, from the caller's own
    /// input, never by this function re-fetching state itself.
    ResolvedInvalid,
}

/// Groups `records` by `(project_id, scenario_id)`, and for each group
/// derives its current [`FoldedState`] from the WINNING record — ranked
/// by `run_seq: TotalOrder` as the sole primary ordering key, `round`
/// consulted ONLY to break a tie among equal `run_seq` values (a plain
/// `(run_seq, round)` tuple comparison gives exactly this: `round` never
/// participates unless `run_seq` is already equal). PURE: no I/O, no
/// `canon-store` dependency — `live_bindings` and `as_of` are the
/// function's ENTIRE view of "now".
pub fn fold_to_current_state(
    records: &[Divergence],
    live_bindings: &BTreeMap<(ProjectId, ScenarioId), BindingSnapshot>,
    as_of: DateTime<Utc>,
) -> BTreeMap<(ProjectId, ScenarioId), FoldedState> {
    let mut groups: BTreeMap<(ProjectId, ScenarioId), Vec<&Divergence>> = BTreeMap::new();
    for record in records {
        groups.entry((record.project_id.clone(), record.scenario_id.clone())).or_default().push(record);
    }

    groups
        .into_iter()
        .map(|(key, members)| {
            let winner = winning_record(members);
            let state = fold_one(winner, live_bindings.get(&key), as_of);
            (key, state)
        })
        .collect()
}

/// The group's fold winner: highest `(run_seq, round)` — `round` a
/// tiebreak ONLY among equal `run_seq` (module doc). A non-empty
/// `members` (guaranteed by `fold_to_current_state`'s grouping loop,
/// which only ever creates a group by pushing at least one record)
/// always has a winner.
fn winning_record(members: Vec<&Divergence>) -> &Divergence {
    members.into_iter().max_by_key(|d| (d.run_seq, d.round)).expect("a fold group is never empty")
}

fn fold_one(winner: &Divergence, live: Option<&BindingSnapshot>, as_of: DateTime<Utc>) -> FoldedState {
    match &winner.status {
        DivergenceStatus::Open => FoldedState::Open,
        DivergenceStatus::StillDivergent => FoldedState::StillDivergent,
        DivergenceStatus::Deferred { reason, expiry } => {
            if as_of >= *expiry {
                // The deferral has lapsed as of `as_of` — the divergence
                // resurfaces as still needing review, never silently
                // treated as resolved by expiry alone.
                FoldedState::StillDivergent
            } else {
                FoldedState::Deferred { reason: reason.clone(), expiry: *expiry }
            }
        }
        DivergenceStatus::Resolved => match live {
            // No live-binding re-check was supplied for this group —
            // there is no EVIDENCE of a mismatch, so the resolution is
            // trusted as-is (`ResolvedInvalid` is derivable ONLY from
            // an actual mismatch in the caller's input, never from the
            // absence of a re-check).
            None => FoldedState::Resolved,
            Some(live) if live.still_matches(winner) => FoldedState::Resolved,
            Some(_) => FoldedState::ResolvedInvalid,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::{Actor, Envelope, RecordKind};
    use crate::ids::{RoleId, TotalOrder};

    fn envelope_at(at: DateTime<Utc>) -> Envelope {
        Envelope::new(1, RecordKind::Divergence, at, Actor::new("reviewer-1", RoleId::parse("reviewer").unwrap()))
    }

    fn project_id() -> ProjectId {
        ProjectId::parse("root").unwrap()
    }

    fn scenario_id() -> ScenarioId {
        ScenarioId::parse("world.place-lock.01").unwrap()
    }

    fn sha(byte: char) -> Sha {
        Sha::parse(byte.to_string().repeat(40)).unwrap()
    }

    fn divergence(run_seq: u64, round: u32, status: DivergenceStatus, at: DateTime<Utc>) -> Divergence {
        Divergence::new(envelope_at(at), project_id(), scenario_id(), sha('a'), status, TotalOrder::new(run_seq), round, "reviewer-1", "detail")
    }

    #[test]
    fn a_lower_run_seq_at_a_higher_round_still_folds_before_a_higher_run_seq() {
        let now = Utc::now();
        let records = vec![
            divergence(3, 9, DivergenceStatus::StillDivergent, now),
            divergence(4, 1, DivergenceStatus::Open, now),
        ];
        let folded = fold_to_current_state(&records, &BTreeMap::new(), now);
        // run_seq 4 wins despite its lower round — `Open` (its status),
        // never `StillDivergent` (run_seq 3's status).
        assert_eq!(folded.get(&(project_id(), scenario_id())), Some(&FoldedState::Open));
    }

    #[test]
    fn round_is_a_tiebreak_only_among_equal_run_seq_values() {
        let now = Utc::now();
        let records = vec![
            divergence(5, 1, DivergenceStatus::StillDivergent, now),
            divergence(5, 2, DivergenceStatus::Open, now),
        ];
        let folded = fold_to_current_state(&records, &BTreeMap::new(), now);
        // Equal run_seq: round 2 breaks the tie over round 1.
        assert_eq!(folded.get(&(project_id(), scenario_id())), Some(&FoldedState::Open));
    }

    #[test]
    fn as_of_governs_deferred_expiry() {
        let opened = Utc::now();
        let expiry = opened + chrono::Duration::days(7);
        let records = vec![divergence(1, 1, DivergenceStatus::Deferred { reason: "waiting on design".into(), expiry }, opened)];

        let before = fold_to_current_state(&records, &BTreeMap::new(), expiry - chrono::Duration::hours(1));
        assert_eq!(
            before.get(&(project_id(), scenario_id())),
            Some(&FoldedState::Deferred { reason: "waiting on design".into(), expiry })
        );

        let after = fold_to_current_state(&records, &BTreeMap::new(), expiry + chrono::Duration::hours(1));
        assert_eq!(after.get(&(project_id(), scenario_id())), Some(&FoldedState::StillDivergent));
    }

    #[test]
    fn a_resolved_winner_whose_live_app_sha_moved_folds_to_resolved_invalid() {
        let resolved_at = Utc::now();
        let records = vec![divergence(2, 1, DivergenceStatus::Resolved, resolved_at)];

        // The scenario's CURRENT app state has moved off the sha the
        // `Divergence` resolved against (`sha('a')`) — a stale resolution.
        let mut live_bindings = BTreeMap::new();
        live_bindings.insert(
            (project_id(), scenario_id()),
            BindingSnapshot { app_sha: sha('b'), reserved_digest: None },
        );

        let folded = fold_to_current_state(&records, &live_bindings, Utc::now());
        assert_eq!(folded.get(&(project_id(), scenario_id())), Some(&FoldedState::ResolvedInvalid));
    }

    #[test]
    fn a_resolved_winner_whose_live_app_sha_matches_stays_resolved() {
        let resolved_at = Utc::now();
        let records = vec![divergence(2, 1, DivergenceStatus::Resolved, resolved_at)];

        let mut live_bindings = BTreeMap::new();
        live_bindings.insert(
            (project_id(), scenario_id()),
            BindingSnapshot { app_sha: sha('a'), reserved_digest: None },
        );

        let folded = fold_to_current_state(&records, &live_bindings, Utc::now());
        assert_eq!(folded.get(&(project_id(), scenario_id())), Some(&FoldedState::Resolved));
    }

    #[test]
    fn a_reserved_digest_only_difference_does_not_invalidate_a_resolved_binding() {
        // `reserved_digest` is non-semantic today: a resolution whose
        // `app_sha` still matches stays `Resolved` regardless of the
        // snapshot's `reserved_digest` value (it is not consulted).
        let resolved_at = Utc::now();
        let records = vec![divergence(2, 1, DivergenceStatus::Resolved, resolved_at)];
        let mut live_bindings = BTreeMap::new();
        live_bindings.insert(
            (project_id(), scenario_id()),
            BindingSnapshot { app_sha: sha('a'), reserved_digest: Some("a-different-digest".into()) },
        );
        let folded = fold_to_current_state(&records, &live_bindings, Utc::now());
        assert_eq!(folded.get(&(project_id(), scenario_id())), Some(&FoldedState::Resolved));
    }

    #[test]
    fn resolved_invalid_is_not_a_divergence_status_variant() {
        // Structural, not just a runtime probe: `DivergenceStatus` only
        // ever deserializes to Open/Resolved/StillDivergent/Deferred —
        // there is no wire string that produces `ResolvedInvalid`,
        // because the variant does not exist on that type at all (it
        // only exists on `FoldedState`, checked by the match above
        // needing no `ResolvedInvalid` arm).
        match DivergenceStatus::Open {
            DivergenceStatus::Open | DivergenceStatus::Resolved | DivergenceStatus::StillDivergent | DivergenceStatus::Deferred { .. } => {}
        }
    }

    #[test]
    fn groups_are_isolated_by_project_id_even_with_the_same_scenario_id() {
        let now = Utc::now();
        let app_a = ProjectId::parse("app-a").unwrap();
        let app_b = ProjectId::parse("app-b").unwrap();
        let mut a = divergence(1, 1, DivergenceStatus::Open, now);
        a.project_id = app_a.clone();
        let mut b = divergence(1, 1, DivergenceStatus::StillDivergent, now);
        b.project_id = app_b.clone();

        let folded = fold_to_current_state(&[a, b], &BTreeMap::new(), now);
        assert_eq!(folded.get(&(app_a, scenario_id())), Some(&FoldedState::Open));
        assert_eq!(folded.get(&(app_b, scenario_id())), Some(&FoldedState::StillDivergent));
    }
}
