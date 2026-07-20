//! The distill step: folds raw [`Trajectory`]s into distilled
//! [`StrategyItem`]s — a deterministic, non-LLM distiller (design
//! decision 6: "distillation is fail-soft and decoupled from the
//! primary write"; this crate ships the deterministic reference
//! distiller, the same role the donor's reasoning-bank stub
//! distiller plays — an LLM-backed
//! distiller is a future, separately-injected concrete impl, never a
//! dependency this crate itself takes on).
//!
//! One trajectory MAY fold into more than one item — one per
//! [`VerdictRow`] it carries (the donor's reasoning-bank stub comment:
//! "real distillers MAY emit multiple; see spec scenario 'a single
//! trajectory may distill into multiple items'"). A `Success` verdict
//! distills into a validated-strategy item (title = the trajectory's
//! `task`, content = its `context`); `Failure`/`Corrective` distill
//! into a guardrail item (title prefixed `avoid:`, content prefixed
//! `Pitfall:` — `makeStubStrategyDistiller`'s exact branching,
//! generalized from `PatternVerdict`'s two-way split to `Polarity`'s
//! three-way one).

use canon_ingest::verdict::Polarity;
use canon_model::ids::RegimeKey;
use chrono::Utc;

use crate::ids::StrategyId;
use crate::strategy::StrategyItem;
use crate::trajectory::Trajectory;

/// Distills one trajectory into zero-or-more strategy items — one per
/// `VerdictRow` it carries (never zero in practice, since
/// [`Trajectory::new`](crate::trajectory::Trajectory::new) rejects an
/// empty verdict list).
pub fn distill_trajectory(trajectory: &Trajectory) -> Vec<StrategyItem> {
    trajectory
        .verdicts
        .iter()
        .map(|verdict| {
            let is_success = matches!(verdict.polarity, Polarity::Success);
            let title = if is_success { trajectory.task.clone() } else { format!("avoid: {}", trajectory.task) };
            let description = if is_success {
                format!("Validated strategy distilled from trajectory {} ({}).", trajectory.id, verdict.becomes.as_str())
            } else {
                format!(
                    "Guardrail distilled from a {} trajectory {} ({}).",
                    verdict.polarity.as_str(),
                    trajectory.id,
                    verdict.becomes.as_str()
                )
            };
            let content = if is_success { trajectory.context.clone() } else { format!("Pitfall: {}", trajectory.context) };
            StrategyItem::new(
                StrategyId::new(),
                trajectory.regime_key.clone(),
                verdict.role.clone(),
                title,
                description,
                content,
                vec![trajectory.id],
                Utc::now(),
            )
        })
        .collect()
}

/// Folds every trajectory recorded under `regime_key` into strategy
/// items (design decision 3's "a distill step that folds a
/// namespace's Trajectories into StrategyItems") — the read-then-fold
/// half of [`crate::rebuild::rebuild_namespace`]. A trajectory whose
/// own `regime_key` does not match (a defensive check; a caller that
/// already queried by `regime_key` never triggers this) is skipped
/// rather than distilled under the wrong namespace, mirroring
/// `rebuildStrategies`'s own `if (trajectory.namespace !== input.namespace) continue;`.
pub fn distill_namespace(regime_key: &RegimeKey, trajectories: &[Trajectory]) -> Vec<StrategyItem> {
    trajectories.iter().filter(|t| &t.regime_key == regime_key).flat_map(distill_trajectory).collect()
}

#[cfg(test)]
mod tests {
    use canon_ingest::verdict::{Becomes, VerdictRow};
    use canon_model::ids::RoleId;

    use super::*;
    use crate::ids::TrajectoryId;

    fn regime(role: &str) -> RegimeKey {
        RegimeKey::parse(canon_model::ids::regime_key(role, "repo", "auth", "abc123")).unwrap()
    }

    fn trajectory(role: &str, task: &str, polarity: Polarity, becomes: Becomes) -> Trajectory {
        let verdict = VerdictRow { role: RoleId::parse(role).unwrap(), polarity, becomes };
        Trajectory::new(TrajectoryId::new(), regime(role), task, "ctx text", vec![verdict], Utc::now(), vec![]).unwrap()
    }

    #[test]
    fn a_success_verdict_distills_into_a_validated_strategy_item() {
        let t = trajectory("dev", "batch the writes", Polarity::Success, Becomes::StrategyCandidate);
        let items = distill_trajectory(&t);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "batch the writes");
        assert_eq!(items[0].content, "ctx text");
        assert_eq!(items[0].source_trajectory_ids, vec![t.id]);
    }

    #[test]
    fn a_failure_verdict_distills_into_a_guardrail_item() {
        let t = trajectory("dev", "skip the null check", Polarity::Failure, Becomes::GuardrailCandidate);
        let items = distill_trajectory(&t);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "avoid: skip the null check");
        assert_eq!(items[0].content, "Pitfall: ctx text");
    }

    #[test]
    fn a_trajectory_with_multiple_verdicts_distills_into_multiple_items() {
        let verdicts = vec![
            VerdictRow { role: RoleId::parse("dev").unwrap(), polarity: Polarity::Failure, becomes: Becomes::GuardrailCandidate },
            VerdictRow { role: RoleId::parse("dev").unwrap(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate },
        ];
        let t = Trajectory::new(TrajectoryId::new(), regime("dev"), "task", "ctx", verdicts, Utc::now(), vec![]).unwrap();
        assert_eq!(distill_trajectory(&t).len(), 2);
    }

    #[test]
    fn distill_namespace_skips_trajectories_outside_the_regime_key() {
        let dev = trajectory("dev", "dev task", Polarity::Success, Becomes::StrategyCandidate);
        let content = trajectory("content", "content task", Polarity::Success, Becomes::StrategyCandidate);
        let items = distill_namespace(&regime("dev"), &[dev, content]);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "dev task");
    }
}
