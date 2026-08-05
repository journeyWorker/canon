//! Generic last-wins-by-`(at, schema, digest)` fold (design D11, s21 D3,
//! s38 `s38-evidence-bearing-memory`): s15 needed this exact reduction
//! in ≥4 places (sync's upsert-check, the divergence fold, gate
//! staleness, the plugin-overlay projection) — rather than a fourth
//! local copy, this is the one hoisted primitive every caller reuses,
//! generalizing `canon-gate::ledger::latest_verdicts`'s pre-hoist local
//! fold. s21 D3 closed the first non-determinism: the original
//! tie-break ("iteration order") was a function of the CALLER's
//! construction/scan order — for a `GitTier`-backed caller, ultimately
//! host-filesystem `readdir` order, unspecified by POSIX and
//! empirically not byte-stable across machines.
//!
//! `s38-evidence-bearing-memory` closes the second one, which s21 D3's
//! digest tie-break papered over rather than answered. A
//! plan-derived record's `Envelope.at` is deliberately BYTE-STABLE, not
//! wall-clock: it is `file_modified_at(<source doc>)` (s20 D7), which
//! is what makes re-importing an unchanged plan idempotent. But a canon
//! CODE change — a parser that now extracts a field it previously
//! dropped — does not advance the source document's mtime, so the stale
//! (pre-change) and fresh (post-change) record for ONE natural key
//! carry an IDENTICAL `at`. Under a bare `(at, digest)` order the
//! lexicographic digest then decides, and a content digest is
//! uncorrelated with generation — so the winner is arbitrary PER ROW:
//! some rows surface the new field, some do not, from one corpus, with
//! no diagnostic. That is indistinguishable from a parser bug by
//! inspection.
//!
//! `Envelope.schema` is the correct discriminator to sit BETWEEN them,
//! because it is precisely the per-kind FORMAT GENERATION: a kind's
//! records gain a field, the kind's `schema` is bumped, and every
//! record written by the new code carries a strictly greater `schema`
//! than every record written by the old code. Ordering by it turns
//! "which generation is this?" from an accident of hashing into an
//! answer the record states. And it costs nothing s21 D3 bought:
//! `schema` is DATA the item itself carries, so the fold stays a pure
//! function of the input SET — machine-independent, never a function of
//! how the caller iterated. `digest` remains the final tie-break, for
//! two records of the same generation at the same `at`.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use canon_model::fold::BindingSnapshot;
use canon_model::{EvidenceRecord, ProjectId, ScenarioId};

/// Fold `items` into one winner per `key(item)`: the item with the
/// greatest `(at(item), schema(item), digest(item))` triple wins,
/// compared as a total order — a strictly greater `at` always wins
/// regardless of schema or digest; on EQUAL `at`, the item carrying the
/// greater `Envelope.schema` (the per-kind format generation) wins; only
/// on equal `at` AND equal `schema` does the item whose `digest` sorts
/// greater (lexicographic string/byte comparison) win.
///
/// The `schema` rung exists because a byte-stable `at` makes ties
/// ROUTINE, not theoretical: plan-derived records stamp `at` from the
/// source file's mtime (s20 D7), so a canon parser change produces a
/// fresh record whose `at` is identical to the stale one it supersedes,
/// and a digest tie-break then picks arbitrarily per row (see this
/// module's doc for the full diagnosis).
///
/// All three rungs are the item's OWN data: the result for a fixed input
/// SET is identical regardless of the order the caller constructs,
/// iterates, or supplies that set in (s21 spec `cross-tier-supersession`'s
/// "machine-independent" requirement) — unlike the pre-s21
/// "later-iterated item wins on a tie" rule this replaces.
pub fn fold_latest_by_key<T, K>(
    items: impl IntoIterator<Item = T>,
    key: impl Fn(&T) -> K,
    at: impl Fn(&T) -> DateTime<Utc>,
    schema: impl Fn(&T) -> u32,
    digest: impl Fn(&T) -> &str,
) -> BTreeMap<K, T>
where
    K: Ord,
{
    let mut latest: BTreeMap<K, T> = BTreeMap::new();
    for item in items {
        let item_order = (at(&item), schema(&item), digest(&item).to_string());
        let k = key(&item);
        let replace = match latest.get(&k) {
            Some(existing) => (at(existing), schema(existing), digest(existing).to_string()) < item_order,
            None => true,
        };
        if replace {
            latest.insert(k, item);
        }
    }
    latest
}

/// Derive the live-binding re-check map
/// [`canon_model::fold::fold_to_current_state`] needs, from the LATEST
/// `EvidenceRecord` per `(project_id, scenario_id)` carrying BOTH a
/// concrete `project_id` AND `evidence_sha`.
///
/// A scenario's CURRENT app state is that latest evidence's
/// `evidence_sha` — the sole live-checkable axis. The fold downgrades a
/// `Resolved` divergence to `ResolvedInvalid` iff this current sha has
/// moved off the sha the divergence resolved against. WHO/WHEN the
/// evidence was authored is deliberately NOT in the snapshot: a
/// divergence's reviewer/`at` are immutable provenance, and a
/// superseding resolution is handled by `run_seq` ranking. A group with
/// no qualifying evidence gets no entry, so `fold_to_current_state`
/// trusts an existing `Resolved` claim as-is — no evidence of a
/// mismatch is not evidence OF one.
///
/// # Why this lives here
/// It was private to `canon-report`, which `canon-gate` may not depend
/// on (`canon-report/tests/gate_independence.rs` forbids it by name).
/// s44's `spec_coverage` check needs the identical map, and a second
/// copy in canon-gate would be exactly the divergent derivation the
/// shared fold exists to prevent — the two surfaces would disagree
/// about which scenarios are open, which is the one thing
/// `canon divergence status` and the gate must never do. `canon-store`
/// is the lowest crate both callers already depend on, and the only one
/// holding both `fold_latest_by_key` and
/// [`crate::partition::content_digest12`].
pub fn live_bindings_of(evidence: Vec<EvidenceRecord>) -> BTreeMap<(ProjectId, ScenarioId), BindingSnapshot> {
    struct Candidate {
        key: (ProjectId, ScenarioId),
        at: DateTime<Utc>,
        /// The record's own `envelope.schema` — the fold's equal-`at`
        /// generation discriminator (`s38-evidence-bearing-memory`),
        /// threaded from the record rather than assumed, so a schema
        /// bump supersedes deterministically instead of by digest luck.
        schema: u32,
        digest: String,
        snapshot: BindingSnapshot,
    }

    let candidates = evidence.into_iter().filter_map(|record| {
        let project_id = record.project_id.clone()?;
        let scenario_id = record.scenario_id.clone()?;
        let app_sha = record.evidence_sha.clone()?;
        let at = record.envelope.at;
        let schema = record.envelope.schema;
        let digest = crate::partition::content_digest12(&serde_json::to_value(&record).unwrap_or_default());
        Some(Candidate { key: (project_id, scenario_id), at, schema, digest, snapshot: BindingSnapshot { app_sha, reserved_digest: None } })
    });

    fold_latest_by_key(candidates, |c| c.key.clone(), |c| c.at, |c| c.schema, |c| c.digest.as_str()).into_values().map(|c| (c.key, c.snapshot)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Item {
        key: &'static str,
        at: DateTime<Utc>,
        schema: u32,
        digest: &'static str,
        tag: &'static str,
    }

    fn at(offset_secs: i64) -> DateTime<Utc> {
        DateTime::UNIX_EPOCH + chrono::Duration::seconds(offset_secs)
    }

    #[test]
    fn latest_at_wins_per_key() {
        let items = vec![
            Item { key: "a", at: at(1), schema: 1, digest: "z", tag: "stale" },
            Item { key: "a", at: at(2), schema: 1, digest: "a", tag: "fresh" },
        ];
        let folded = fold_latest_by_key(items, |i| i.key, |i| i.at, |i| i.schema, |i| i.digest);
        assert_eq!(folded.len(), 1);
        assert_eq!(folded.get("a").unwrap().tag, "fresh", "a strictly greater `at` wins regardless of digest");
    }

    #[test]
    fn earlier_item_arriving_after_a_later_one_never_wins() {
        // Iteration order does NOT determine the winner when `at`
        // genuinely differs — only the latest `at` does.
        let items = vec![
            Item { key: "a", at: at(2), schema: 1, digest: "a", tag: "fresh" },
            Item { key: "a", at: at(1), schema: 1, digest: "z", tag: "stale" },
        ];
        let folded = fold_latest_by_key(items, |i| i.key, |i| i.at, |i| i.schema, |i| i.digest);
        assert_eq!(folded.get("a").unwrap().tag, "fresh");
    }

    #[test]
    fn a_strictly_greater_at_wins_even_when_it_carries_a_lower_schema() {
        // `at` stays the OUTERMOST rung: the schema discriminator only
        // ever resolves a tie, it never lets an older-`at` record with a
        // newer format generation supersede a genuinely newer record.
        let items = vec![
            Item { key: "a", at: at(9), schema: 1, digest: "aaa", tag: "newer-at-older-schema" },
            Item { key: "a", at: at(1), schema: 7, digest: "zzz", tag: "older-at-newer-schema" },
        ];
        let folded = fold_latest_by_key(items, |i| i.key, |i| i.at, |i| i.schema, |i| i.digest);
        assert_eq!(folded.get("a").unwrap().tag, "newer-at-older-schema", "a strictly greater `at` must win regardless of schema or digest");
    }

    #[test]
    fn an_equal_at_tie_goes_to_the_greater_schema_even_when_the_lower_schema_has_the_greater_digest() {
        // The `s38-evidence-bearing-memory` defect in miniature: two
        // generations of ONE natural key sharing a byte-stable `at`
        // (s20 D7's `file_modified_at`, unmoved by a canon code
        // change). The STALE record deliberately carries the
        // lexicographically GREATER digest, so under s21 D3's bare
        // `(at, digest)` rule it would win — proving the schema rung,
        // not luck, decides.
        let same_at = at(5);
        let items = vec![
            Item { key: "a", at: same_at, schema: 1, digest: "zzz", tag: "stale-generation" },
            Item { key: "a", at: same_at, schema: 2, digest: "aaa", tag: "fresh-generation" },
        ];
        let folded = fold_latest_by_key(items, |i| i.key, |i| i.at, |i| i.schema, |i| i.digest);
        assert_eq!(folded.get("a").unwrap().tag, "fresh-generation", "an equal-`at` tie must go to the greater `Envelope.schema`, never to the greater digest of an older format generation");
    }

    #[test]
    fn the_greater_schema_wins_an_equal_at_tie_regardless_of_construction_order() {
        // Same machine-independence property s21 D3 established for the
        // digest rung, now for the schema rung: `schema` is data the
        // item carries, so neither iteration order nor which generation
        // the caller happened to scan first can change the winner.
        let same_at = at(5);
        let stale = Item { key: "a", at: same_at, schema: 1, digest: "zzz", tag: "stale-generation" };
        let fresh = Item { key: "a", at: same_at, schema: 2, digest: "aaa", tag: "fresh-generation" };

        let stale_first = fold_latest_by_key(vec![stale.clone(), fresh.clone()], |i| i.key, |i| i.at, |i| i.schema, |i| i.digest);
        let fresh_first = fold_latest_by_key(vec![fresh, stale], |i| i.key, |i| i.at, |i| i.schema, |i| i.digest);

        assert_eq!(stale_first.get("a").unwrap().tag, "fresh-generation");
        assert_eq!(fresh_first.get("a").unwrap().tag, "fresh-generation", "construction order must never change the winner");
    }

    #[test]
    fn ties_broken_by_the_greater_digest_never_by_iteration_order() {
        // Equal `at` AND equal `schema` — the digest rung is still the
        // final, total tie-break (s21 D3, unweakened).
        let same_at = at(5);
        let items = vec![
            Item { key: "a", at: same_at, schema: 1, digest: "zzz", tag: "greater-digest-first" },
            Item { key: "a", at: same_at, schema: 1, digest: "aaa", tag: "lesser-digest-second" },
        ];
        let folded = fold_latest_by_key(items, |i| i.key, |i| i.at, |i| i.schema, |i| i.digest);
        assert_eq!(folded.get("a").unwrap().tag, "greater-digest-first", "an equal-`at` tie must go to the item with the greater digest, not the later-iterated one");
    }

    #[test]
    fn same_at_tie_folds_to_the_identical_winner_regardless_of_construction_order() {
        // The actual machine-independence property (s21 spec
        // `cross-tier-supersession`, "Two same-`at` items fold to the
        // same winner regardless of iteration order"): the SAME two
        // items, folded once with the greater-digest item iterated
        // first and once iterated second, must produce the SAME
        // winner both times.
        let same_at = at(5);
        let greater = Item { key: "a", at: same_at, schema: 1, digest: "zzz", tag: "greater" };
        let lesser = Item { key: "a", at: same_at, schema: 1, digest: "aaa", tag: "lesser" };

        let greater_first = fold_latest_by_key(vec![greater.clone(), lesser.clone()], |i| i.key, |i| i.at, |i| i.schema, |i| i.digest);
        let lesser_first = fold_latest_by_key(vec![lesser, greater], |i| i.key, |i| i.at, |i| i.schema, |i| i.digest);

        assert_eq!(greater_first.get("a").unwrap().tag, "greater");
        assert_eq!(lesser_first.get("a").unwrap().tag, "greater", "construction order must never change the winner");
    }

    #[test]
    fn distinct_keys_are_kept_independently() {
        let items = vec![
            Item { key: "a", at: at(1), schema: 1, digest: "d1", tag: "a-only" },
            Item { key: "b", at: at(1), schema: 1, digest: "d2", tag: "b-only" },
        ];
        let folded = fold_latest_by_key(items, |i| i.key, |i| i.at, |i| i.schema, |i| i.digest);
        assert_eq!(folded.len(), 2);
        assert_eq!(folded.get("a").unwrap().tag, "a-only");
        assert_eq!(folded.get("b").unwrap().tag, "b-only");
    }

    #[test]
    fn empty_input_folds_to_an_empty_map() {
        let folded = fold_latest_by_key(Vec::<Item>::new(), |i| i.key, |i| i.at, |i| i.schema, |i| i.digest);
        assert!(folded.is_empty());
    }
}
