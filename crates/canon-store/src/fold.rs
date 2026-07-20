//! Generic last-wins-by-`(at, digest)` fold (design D11, s21 D3): s15
//! needed this exact reduction in ≥4 places (sync's upsert-check, the
//! divergence fold, gate staleness, the plugin-overlay projection) —
//! rather than a fourth local copy, this is the one hoisted primitive
//! every caller reuses, generalizing `canon-gate::ledger::latest_verdicts`'s
//! pre-hoist local fold. s21 D3 closed the one remaining
//! non-determinism: the original tie-break ("iteration order") was a
//! function of the CALLER's construction/scan order — for a
//! `GitTier`-backed caller, ultimately host-filesystem `readdir` order,
//! unspecified by POSIX and empirically not byte-stable across
//! machines. The tie-break is now `(at, digest)`, compared as a total
//! order over DATA the item itself carries — never over how the caller
//! happened to iterate.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};

/// Fold `items` into one winner per `key(item)`: the item with the
/// greatest `(at(item), digest(item))` pair wins, compared as a total
/// order — a strictly greater `at` always wins regardless of digest; on
/// EQUAL `at`, the item whose `digest` sorts greater (lexicographic
/// string/byte comparison) wins. This is a pure function of each item's
/// own data: the result for a fixed input SET is identical regardless
/// of the order the caller constructs, iterates, or supplies that set
/// in (s21 spec `cross-tier-supersession`'s "machine-independent"
/// requirement) — unlike the pre-s21 "later-iterated item wins on a
/// tie" rule this replaces.
pub fn fold_latest_by_key<T, K>(items: impl IntoIterator<Item = T>, key: impl Fn(&T) -> K, at: impl Fn(&T) -> DateTime<Utc>, digest: impl Fn(&T) -> &str) -> BTreeMap<K, T>
where
    K: Ord,
{
    let mut latest: BTreeMap<K, T> = BTreeMap::new();
    for item in items {
        let item_order = (at(&item), digest(&item).to_string());
        let k = key(&item);
        let replace = match latest.get(&k) {
            Some(existing) => (at(existing), digest(existing).to_string()) < item_order,
            None => true,
        };
        if replace {
            latest.insert(k, item);
        }
    }
    latest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Item {
        key: &'static str,
        at: DateTime<Utc>,
        digest: &'static str,
        tag: &'static str,
    }

    fn at(offset_secs: i64) -> DateTime<Utc> {
        DateTime::UNIX_EPOCH + chrono::Duration::seconds(offset_secs)
    }

    #[test]
    fn latest_at_wins_per_key() {
        let items = vec![
            Item { key: "a", at: at(1), digest: "z", tag: "stale" },
            Item { key: "a", at: at(2), digest: "a", tag: "fresh" },
        ];
        let folded = fold_latest_by_key(items, |i| i.key, |i| i.at, |i| i.digest);
        assert_eq!(folded.len(), 1);
        assert_eq!(folded.get("a").unwrap().tag, "fresh", "a strictly greater `at` wins regardless of digest");
    }

    #[test]
    fn earlier_item_arriving_after_a_later_one_never_wins() {
        // Iteration order does NOT determine the winner when `at`
        // genuinely differs — only the latest `at` does.
        let items = vec![
            Item { key: "a", at: at(2), digest: "a", tag: "fresh" },
            Item { key: "a", at: at(1), digest: "z", tag: "stale" },
        ];
        let folded = fold_latest_by_key(items, |i| i.key, |i| i.at, |i| i.digest);
        assert_eq!(folded.get("a").unwrap().tag, "fresh");
    }

    #[test]
    fn ties_broken_by_the_greater_digest_never_by_iteration_order() {
        let same_at = at(5);
        let items = vec![
            Item { key: "a", at: same_at, digest: "zzz", tag: "greater-digest-first" },
            Item { key: "a", at: same_at, digest: "aaa", tag: "lesser-digest-second" },
        ];
        let folded = fold_latest_by_key(items, |i| i.key, |i| i.at, |i| i.digest);
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
        let greater = Item { key: "a", at: same_at, digest: "zzz", tag: "greater" };
        let lesser = Item { key: "a", at: same_at, digest: "aaa", tag: "lesser" };

        let greater_first = fold_latest_by_key(vec![greater.clone(), lesser.clone()], |i| i.key, |i| i.at, |i| i.digest);
        let lesser_first = fold_latest_by_key(vec![lesser, greater], |i| i.key, |i| i.at, |i| i.digest);

        assert_eq!(greater_first.get("a").unwrap().tag, "greater");
        assert_eq!(lesser_first.get("a").unwrap().tag, "greater", "construction order must never change the winner");
    }

    #[test]
    fn distinct_keys_are_kept_independently() {
        let items = vec![Item { key: "a", at: at(1), digest: "d1", tag: "a-only" }, Item { key: "b", at: at(1), digest: "d2", tag: "b-only" }];
        let folded = fold_latest_by_key(items, |i| i.key, |i| i.at, |i| i.digest);
        assert_eq!(folded.len(), 2);
        assert_eq!(folded.get("a").unwrap().tag, "a-only");
        assert_eq!(folded.get("b").unwrap().tag, "b-only");
    }

    #[test]
    fn empty_input_folds_to_an_empty_map() {
        let folded = fold_latest_by_key(Vec::<Item>::new(), |i| i.key, |i| i.at, |i| i.digest);
        assert!(folded.is_empty());
    }
}
