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
//!
//! # Determinism is a contract here, not a nicety
//! Every value this module produces is a pure function of the RETAINED
//! raw trajectory: the id comes from
//! [`crate::ids::StrategyId::derive`] over the distilled row's own
//! content, and `recorded_at` is the SOURCE
//! trajectory's own timestamp, never `Utc::now()`. That makes
//! [`crate::rebuild::rebuild_namespace`] a fixpoint — re-distilling
//! unchanged evidence rewrites byte-identical rows under identical
//! filenames — which is what keeps a recorded
//! `Run.injected_guidance` `StrategyRef`, a promoted
//! `.canon/strategies/<role>/<id>.md`, and `mart_flywheel_funnel`'s
//! `retrieved`/`applied` stages resolving across an ordinary `canon
//! ingest artifacts` (the three joins `crate::ids`' module doc walks
//! through). Reading the wall clock here would defeat the derived id
//! outright: `recorded_at` feeds the ULID's own time component.

use canon_ingest::verdict::Polarity;
use canon_model::ids::RegimeKey;

use crate::ids::StrategyIdentity;
use crate::strategy::StrategyItem;
use crate::trajectory::Trajectory;

/// Distills one trajectory into zero-or-more strategy items — one per
/// DISTINCT `VerdictRow` it carries (never zero in practice, since
/// [`Trajectory::new`](crate::trajectory::Trajectory::new) rejects an
/// empty verdict list; fewer than one-per-verdict only when a
/// trajectory repeats a byte-identical verdict, see the dedupe comment
/// in the loop). Deterministic in every field — same trajectory in,
/// byte-identical items out, no clock and no fresh id (module doc).
pub fn distill_trajectory(trajectory: &Trajectory) -> Vec<StrategyItem> {
    let source_trajectory_ids = [trajectory.id];
    let mut items: Vec<StrategyItem> = Vec::with_capacity(trajectory.verdicts.len());
    for verdict in &trajectory.verdicts {
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
        let item = StrategyItem::from_identity(StrategyIdentity {
            regime_key: &trajectory.regime_key,
            role: &verdict.role,
            title: &title,
            description: &description,
            content: &content,
            source_trajectory_ids: &source_trajectory_ids,
            recorded_at: trajectory.recorded_at,
        });
        // Two byte-identical `VerdictRow`s on one trajectory distill
        // into one strategy, not two indistinguishable copies of it:
        // under a content-derived id they share a `<id>.parquet`
        // filename, so storing both would silently write one row and
        // leave `rebuild_namespace`'s returned count disagreeing with
        // the store. Collapsing here keeps the two in step, and costs a
        // linear scan over a list that holds a handful of verdicts —
        // cheaper than allocating a set to dedupe it.
        if items.iter().any(|existing| existing.id == item.id) {
            continue;
        }
        items.push(item);
    }
    items
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
    use chrono::Utc;

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

    /// The regression the whole derived-id change exists for: distilling
    /// the SAME retained trajectory twice — what every
    /// `rebuild_namespace` does — must reproduce the same ids. Under
    /// the previous `StrategyId::new()` mint this failed on every call,
    /// silently orphaning each recorded `Run.injected_guidance`
    /// `StrategyRef` and each promoted `.canon/strategies/<role>/
    /// <id>.md`.
    #[test]
    fn distilling_the_same_trajectory_twice_reproduces_the_same_ids() {
        let t = trajectory("dev", "batch the writes", Polarity::Success, Becomes::StrategyCandidate);
        let first = distill_trajectory(&t);
        let second = distill_trajectory(&t);
        assert_eq!(first, second, "distillation must be a pure function of the trajectory, id and recorded_at included");
    }

    /// `recorded_at` is the SOURCE trajectory's timestamp, not the
    /// distiller's wall clock — the property that lets `recorded_at`
    /// feed the derived id's own time component without breaking it.
    #[test]
    fn a_distilled_item_carries_its_source_trajectorys_recorded_at() {
        let t = trajectory("dev", "batch the writes", Polarity::Success, Becomes::StrategyCandidate);
        assert_eq!(distill_trajectory(&t)[0].recorded_at, t.recorded_at);
    }

    /// Two DIFFERENT trajectories never alias onto one id even when
    /// they carry the same task text: the distilled `description` and
    /// `source_trajectory_ids` both name the source trajectory, so the
    /// content-derived id separates them. Without this, one
    /// `<id>.parquet` filename would hold two strategies and `distilled`
    /// would undercount.
    #[test]
    fn two_trajectories_with_identical_task_text_distill_to_distinct_ids() {
        let a = trajectory("dev", "batch the writes", Polarity::Success, Becomes::StrategyCandidate);
        let b = trajectory("dev", "batch the writes", Polarity::Success, Becomes::StrategyCandidate);
        assert_ne!(distill_trajectory(&a)[0].id, distill_trajectory(&b)[0].id);
    }

    /// A trajectory repeating a byte-identical verdict distills into ONE
    /// item: the two would share a content-derived id and therefore a
    /// `<id>.parquet` filename, so emitting both would make
    /// `rebuild_namespace`'s returned count disagree with the store's
    /// actual row count.
    #[test]
    fn a_repeated_identical_verdict_distills_into_one_item() {
        let verdict = || VerdictRow { role: RoleId::parse("dev").unwrap(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate };
        let t = Trajectory::new(TrajectoryId::new(), regime("dev"), "task", "ctx", vec![verdict(), verdict()], Utc::now(), vec![]).unwrap();
        assert_eq!(distill_trajectory(&t).len(), 1);
    }
}
