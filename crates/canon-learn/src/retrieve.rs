//! `retrieve`: the read side of the trace→verdict→distill→store→
//! retrieve→apply loop (module doc). Wraps
//! [`StrategyStore::query_by_regime_key`] with a deterministic
//! ordering (most-recently-distilled first) and an optional top-N cap
//! — S7 (reward-statistical-promotion) owns RANKING by reward; this
//! function only owns "return this namespace's strategies in a stable
//! order", never a quality judgment.

use canon_model::ids::RegimeKey;

use crate::error::LearnError;
use crate::store::StrategyStore;
use crate::strategy::StrategyItem;

/// Every strategy item recorded for `regime_key`, most-recent first.
/// `limit` caps the result to the first `limit` items when `Some`
/// ("the top strategies for a role/repo/area" — "top" here means
/// "most recently distilled", the only ordering this change owns;
/// S7/S8 may layer reward-weighted ranking on top without changing
/// this function's contract).
pub fn retrieve(strategy_store: &dyn StrategyStore, regime_key: &RegimeKey, limit: Option<usize>) -> Result<Vec<StrategyItem>, LearnError> {
    let mut items = strategy_store.query_by_regime_key(regime_key)?;
    items.sort_by_key(|item| std::cmp::Reverse(item.recorded_at));
    if let Some(limit) = limit {
        items.truncate(limit);
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use canon_model::ids::RoleId;
    use chrono::{DateTime, Duration, Utc};

    use super::*;
    use crate::ids::{StrategyId, TrajectoryId};
    use crate::store::ParquetStrategyStore;

    fn regime(role: &str) -> RegimeKey {
        RegimeKey::parse(canon_model::ids::regime_key(role, "repo", "auth", "abc123")).unwrap()
    }

    fn strategy_at(role: &str, title: &str, at: DateTime<Utc>) -> StrategyItem {
        StrategyItem::new(StrategyId::new(), regime(role), RoleId::parse(role).unwrap(), title, "d", "c", vec![TrajectoryId::new()], at)
    }

    #[test]
    fn retrieve_returns_only_the_requested_namespaces_strategies_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetStrategyStore::open(dir.path());
        let now = Utc::now();
        store.append(&strategy_at("dev", "older", now - Duration::hours(1))).unwrap();
        store.append(&strategy_at("dev", "newer", now)).unwrap();
        store.append(&strategy_at("content", "other namespace", now)).unwrap();

        let items = retrieve(&store, &regime("dev"), None).unwrap();
        assert_eq!(items.iter().map(|i| i.title.as_str()).collect::<Vec<_>>(), vec!["newer", "older"]);
    }

    #[test]
    fn retrieve_respects_a_limit() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetStrategyStore::open(dir.path());
        let now = Utc::now();
        for i in 0..5i64 {
            store.append(&strategy_at("dev", &format!("s{i}"), now - Duration::minutes(i))).unwrap();
        }
        let items = retrieve(&store, &regime("dev"), Some(2)).unwrap();
        assert_eq!(items.len(), 2);
    }
}
