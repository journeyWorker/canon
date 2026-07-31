//! The raw tier: [`Trajectory`] — a captured trace, generalizing
//! the donor harness's `PatternTrajectory`
//! (id/namespace/task/input/action/output/verdict/reward/recordedAt/tags,
//! deliberately single-step — "the donor's harness runs are one tool call →
//! one trajectory") onto canon's join spine: the namespace column
//! becomes the canonical `regime_key` (design decision 2), and the
//! bare `verdict: PatternVerdict` string becomes the actual
//! [`VerdictRow`](canon_ingest::verdict::VerdictRow)(s) S4 already
//! derived, carried verbatim rather than re-collapsed into a
//! `success|failure|pending` enum.

use canon_ingest::verdict::VerdictRow;
use canon_model::ids::{RegimeKey, RoleId, RunId};
use chrono::{DateTime, Utc};

use crate::error::LearnError;
use crate::ids::TrajectoryId;
use crate::verdict_outcome::TrajectoryVerdict;

/// One captured trace: the [`VerdictRow`]s a S4 artifact-ingest wave
/// derived for this outcome, plus the reasoning/context that produced
/// it — raw, immutable, cold tier (design decision 3). Keyed by
/// [`RegimeKey`] (design decision 2's `<role>/<repo>/<area>/<hash>`,
/// the SAME `canon_model::ids::regime_key` serialization every write
/// and read path in canon reuses).
#[derive(Debug, Clone, PartialEq)]
pub struct Trajectory {
    pub id: TrajectoryId,
    pub regime_key: RegimeKey,
    /// What was attempted — the short task description
    /// (`PatternTrajectory.task`'s analog; the distiller's `title`
    /// source for a successful strategy candidate).
    pub task: String,
    /// The reasoning/context narrative that produced the outcome
    /// (`PatternTrajectory.input`/`.output`'s analog, collapsed to one
    /// field per this crate's own scope — see module doc's
    /// single-step rationale).
    pub context: String,
    /// The VerdictRow(s) this trajectory carries — never empty
    /// ([`Trajectory::new`] rejects an empty list). One trajectory MAY
    /// carry more than one verdict (e.g. a code-review finding
    /// followed by its later remediation, both folded onto the same
    /// regime); [`crate::distill::distill_trajectory`] emits up to one
    /// distilled item per verdict.
    pub verdicts: Vec<VerdictRow>,
    pub recorded_at: DateTime<Utc>,
    pub tags: Vec<String>,
    /// The S7-level rolled-up outcome+reward (design D2) —
    /// `TrajectoryVerdict::pending()` until
    /// [`crate::mark_verdict::mark_trajectory_verdict`] writes a
    /// covering verdict. Not a [`Trajectory::new`] constructor
    /// parameter (every freshly-constructed trajectory starts
    /// `Pending`, matching the two-phase reward-write model
    /// the donor's dev-reward backfill documents) — use
    /// [`Trajectory::with_verdict_record`] to seed a non-default value
    /// (e.g. test fixtures).
    pub verdict_record: TrajectoryVerdict,
    /// The DISPATCHED RUN this trajectory was derived inside, when the
    /// deriving pass was told one — s42 (`close-the-open-loops`) task
    /// 3.1, closing s40 task 3.1, which asked
    /// `mart_flywheel_funnel`'s `applied` stage to count "a resolved
    /// trajectory joined to its own run" and could not be implemented
    /// because this type carried no run at all. (The `canon-model`
    /// record kind that does — `canon_model::records::Trajectory`, whose
    /// partition natural key IS its run id — has zero production
    /// writers, so conditioning the mart on it would have shipped a
    /// column structurally pinned at `0`.)
    ///
    /// `None` is the honest, and overwhelmingly common, value. It is
    /// NEVER inferred: `canon ingest artifacts` stamps this only from
    /// its own explicit `--run <RunId>`, never from a timestamp, a
    /// role, or the most recent dispatch manifest, because a wrong
    /// attribution is worse than none — the mart would then assert a
    /// relationship the corpus does not carry, the exact defect class
    /// s39/s40/s41 spent three changes removing.
    ///
    /// Deliberately OUTSIDE the trajectory identity this crate's
    /// callers digest. `crates/canon-cli/src/artifact_ingest.rs`'s
    /// `trajectory_content_digest` folds what a trajectory SAYS (its
    /// `regime_key`, ordered [`VerdictRow`]s and rendered
    /// `task`/`context`) — the bytes a distilled strategy is built
    /// from. A run id is provenance ABOUT the write, distils into
    /// nothing, and folding it in would break write-time idempotence:
    /// two passes over one unchanged corpus, one inside a dispatch and
    /// one outside, would stop recognizing each other. See that
    /// function's own doc comment.
    pub run_id: Option<RunId>,
}

impl Trajectory {
    /// Constructs a trajectory, validating the two invariants a
    /// well-formed regime-keyed trace must hold:
    ///
    /// - at least one [`VerdictRow`] ([`LearnError::EmptyVerdicts`]);
    /// - every verdict's `role` agrees with `regime_key`'s own `role`
    ///   segment ([`LearnError::VerdictRoleMismatch`]) — `regime_key`'s
    ///   role is the single retrieval axis (design decision 2: "a
    ///   `dev` trajectory must never surface as a similar-regime hit
    ///   for a `content` role"), so a trajectory whose verdicts
    ///   disagree with its own key would silently violate that at
    ///   read time.
    pub fn new(
        id: TrajectoryId,
        regime_key: RegimeKey,
        task: impl Into<String>,
        context: impl Into<String>,
        verdicts: Vec<VerdictRow>,
        recorded_at: DateTime<Utc>,
        tags: Vec<String>,
    ) -> Result<Self, LearnError> {
        if verdicts.is_empty() {
            return Err(LearnError::EmptyVerdicts);
        }
        for v in &verdicts {
            if v.role.as_str() != regime_key.role() {
                return Err(LearnError::VerdictRoleMismatch {
                    verdict_role: v.role.as_str().to_string(),
                    regime_role: regime_key.role().to_string(),
                });
            }
        }
        Ok(Self {
            id,
            regime_key,
            task: task.into(),
            context: context.into(),
            verdicts,
            recorded_at,
            tags,
            verdict_record: TrajectoryVerdict::pending(),
            run_id: None,
        })
    }

    pub fn role(&self) -> Result<RoleId, LearnError> {
        RoleId::parse(self.regime_key.role()).map_err(LearnError::from)
    }

    /// Builder-style override for [`Trajectory::verdict_record`] — the
    /// constructor always seeds `Pending`; this is the escape hatch for
    /// a caller (typically a test fixture) that wants a pre-resolved
    /// trajectory without a separate `mark_trajectory_verdict` round
    /// trip.
    pub fn with_verdict_record(mut self, verdict_record: TrajectoryVerdict) -> Self {
        self.verdict_record = verdict_record;
        self
    }

    /// Builder-style setter for [`Trajectory::run_id`] (s42
    /// (`close-the-open-loops`) task 3.2) — the constructor always seeds
    /// `None`, because attribution is knowledge the DERIVING PASS has
    /// and the trace itself does not.
    ///
    /// Takes the caller's own `Option` verbatim rather than a bare
    /// [`RunId`], deliberately. Every caller's attribution is itself
    /// optional (`canon ingest artifacts` has a run only when invoked
    /// with `--run`), so a `RunId`-taking setter would push an
    /// `if let`/`map` branch onto each call site — and a branch that
    /// has to decide "is there a run here" is exactly where a `None`
    /// becomes a fabricated `Some`. Passing the `Option` through makes
    /// absence the trivial path.
    pub fn with_run_id(mut self, run_id: Option<RunId>) -> Self {
        self.run_id = run_id;
        self
    }
}

#[cfg(test)]
mod tests {
    use canon_ingest::verdict::{Becomes, Polarity};

    use super::*;

    fn verdict(role: &str) -> VerdictRow {
        VerdictRow { role: RoleId::parse(role).unwrap(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate }
    }

    fn regime(role: &str) -> RegimeKey {
        RegimeKey::parse(canon_model::ids::regime_key(role, "repo", "auth", "abc123")).unwrap()
    }

    #[test]
    fn rejects_empty_verdicts() {
        let err = Trajectory::new(TrajectoryId::new(), regime("dev"), "t", "c", vec![], Utc::now(), vec![]).unwrap_err();
        assert!(matches!(err, LearnError::EmptyVerdicts));
    }

    #[test]
    fn rejects_a_verdict_role_disagreeing_with_the_regime_key_role() {
        let err =
            Trajectory::new(TrajectoryId::new(), regime("dev"), "t", "c", vec![verdict("content")], Utc::now(), vec![])
                .unwrap_err();
        assert!(matches!(err, LearnError::VerdictRoleMismatch { .. }));
    }

    #[test]
    fn accepts_verdicts_agreeing_with_the_regime_key_role() {
        let trajectory =
            Trajectory::new(TrajectoryId::new(), regime("dev"), "t", "c", vec![verdict("dev")], Utc::now(), vec![]).unwrap();
        assert_eq!(trajectory.role().unwrap(), RoleId::parse("dev").unwrap());
    }

    /// s42 (`close-the-open-loops`) task 3.1: the field is ADDITIVE and
    /// ABSENT when unset. `new` must never invent a run — the whole
    /// point of the field is that an unattributed trajectory says so.
    #[test]
    fn a_freshly_constructed_trajectory_carries_no_run() {
        let trajectory =
            Trajectory::new(TrajectoryId::new(), regime("dev"), "t", "c", vec![verdict("dev")], Utc::now(), vec![]).unwrap();
        assert_eq!(trajectory.run_id, None, "attribution is never minted by the constructor");
    }

    /// The setter records exactly what it was handed, both ways — a
    /// `None` handed in stays `None` (the ingest-outside-a-dispatch
    /// case) rather than degrading into a fabricated id.
    #[test]
    fn with_run_id_records_the_callers_option_verbatim() {
        let run_id = RunId::new();
        let build = || Trajectory::new(TrajectoryId::new(), regime("dev"), "t", "c", vec![verdict("dev")], Utc::now(), vec![]).unwrap();

        assert_eq!(build().with_run_id(Some(run_id)).run_id, Some(run_id));
        assert_eq!(build().with_run_id(None).run_id, None);
    }
}
