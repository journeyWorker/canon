//! The trust-ladder checks (design decision 2/D21) + the flag-clear
//! ratchet's core validation (design decision 2's risk mitigation).
//! Generalizes `tools/parity.py::_trust_level` + the one-way
//! `@flagged` ratchet in `tools/hooks/guard-spec-edit.py`
//! (the donor parity-harness audit's trust-ladder notes §3.1/§3.3)
//! over this crate's `EvidenceRecord`-shaped corpus.
//!
//! # s15 P3b: native fields, read directly off `ctx.evidence`
//! `lifecycle`/`flagged` used to be an interim, canon-gate-owned
//! `trust_ladder` raw-JSON companion (`crate::trust_ladder`'s own
//! migration note) — s15 P1 moved both onto `canon_model::EvidenceRecord`
//! natively as `lifecycle: Option<TrustLifecycle>`/`flagged:
//! Option<FlaggedOverlay>`; this module now reads them as plain field
//! accesses on the already-typed `ctx.evidence`, never a second raw
//! `GitTier` re-scan for them. The read stays THREE-way (unchanged
//! semantics, design D9/`gate-native-record-fields` spec): an ABSENT
//! field legitimately defaults to `draft`/unflagged —
//! `record.lifecycle.unwrap_or(TrustLifecycle::Draft)`/
//! `record.flagged.as_ref().is_some_and(|f| f.flagged)` below ARE that
//! default, not a special case; a PRESENT well-formed field reads
//! typed, exactly as `ctx.evidence` already deserialized it; a PRESENT
//! malformed field never reaches `ctx.evidence` at all —
//! `canon_model::EvidenceRecord`'s own `present_value` deserializer
//! fails the WHOLE record's `Deserialize` at
//! [`crate::context::GateContext::load`] time, landing it in
//! `ctx.violations` instead — `crate::ledger::LedgerCheck` (always in
//! `crate::dispatch::check_set`) already surfaces every one of those
//! as `malformed-evidence` through the NORMAL dispatcher. This module
//! therefore never re-implements that malformed-detection itself: by
//! the time a record reaches `ctx.evidence`, its five native fields are
//! either absent (documented default) or well-formed (typed) — never a
//! third "present but unreadable" case this module needs to branch on.
//!
//! # `ReleaseTrustCheck`'s `class` companion — NOT migrated
//! `class` (`policy.yaml`'s `trust_required` vocabulary key a record's
//! release-trust requirement is scoped to) is deliberately NOT one of
//! the five s15-native fields — it stays a canon-gate-owned raw-JSON
//! companion (a plain top-level `{"class": "p1"}` key,
//! `EvidenceRecord`'s strict `Deserialize` silently drops it as an
//! unknown key). [`ReleaseTrustCheck`] alone still independently
//! re-scans the ledger RAW ([`raw_evidence`]) to recover it —
//! [`TrustLadderCheck`] needs no such scan at all, since it reads
//! nothing companion-shaped anymore.
//!
//! # Two separate checks, one always-on, one release-scoped
//! [`TrustLadderCheck`] is static and always-on
//! (`unreviewed-promotion`/`flagged` — trust-ladder.md §3.2/§3.3: "a
//! **static, always-on** check"); [`ReleaseTrustCheck`] is a SEPARATE,
//! opt-in check (`trust-below-required`) a release profile registers
//! additionally (spec.md "it does not block ordinary (non-release)
//! evaluation", trust-ladder.md §3.4). Bundling both into one
//! `GateCheck` would make `trust-below-required` fire on every
//! ordinary gate run, which spec.md explicitly forbids.

use std::collections::HashSet;
use std::path::Path;

use canon_model::{Actor, EvidenceRecord, FlaggedOverlay, ProjectId, RecordKind, Review, ScenarioId, TrustLifecycle};
use canon_store::git_tier::GitTier;
use canon_store::tier::{Tier, TierQuery};

use crate::context::{GateCheck, GateContext};
use crate::failure_class::{FailureClass, Violation};
use crate::trust_ladder::{TrustLadderState, TrustRung};

/// This record's `Violation::subject` — `task_id` preferred, then
/// `scenario_id`, then `run_id` (join-spine table order), matching
/// `crate::coverage::CellSubject`'s own preference order.
fn subject_of(record: &EvidenceRecord) -> String {
    if let Some(task_id) = &record.task_id {
        task_id.to_string()
    } else if let Some(scenario_id) = &record.scenario_id {
        scenario_id.to_string()
    } else if let Some(run_id) = &record.run_id {
        run_id.to_string()
    } else {
        "<unscoped>".to_string()
    }
}

/// Independently re-scan `ledger_root` for every well-formed
/// `EvidenceRecord`, RAW — used ONLY by [`ReleaseTrustCheck`] to
/// recover its `class` companion (module doc; [`TrustLadderCheck`]
/// needs no raw scan at all now). Fail-soft (`GateCheck::run`'s own §7
/// contract: never panic) — an unreadable ledger degrades to "nothing
/// found" rather than aborting the whole check; `ctx.evidence` already
/// proves the root loaded successfully once for this same
/// `GateContext`, so this should not realistically fail independently.
fn raw_evidence(ledger_root: &Path) -> Vec<canon_model::RawRecord> {
    GitTier::new(ledger_root).read(&TierQuery::kind(RecordKind::EvidenceRecord)).map(|read| read.records).unwrap_or_default()
}

/// Every well-formed ledger [`Review`], indexed for [`TrustRung::classify`]'s
/// `has_review_record` input — project-aware (design D12 /
/// `gate-native-record-fields` spec's "review index is project-aware"
/// requirement): [`Self::is_reviewed`] matches by the COMPOSITE
/// `(project_id, scenario_id)` when the querying evidence carries
/// `Some(project_id)` (a review for one project never satisfies another
/// project's same-`scenario_id` evidence), falling back to the prior
/// bare-`scenario_id` match for `project_id = None` legacy evidence (no
/// regression). `bare` therefore indexes every reviewed `scenario_id`
/// across ALL projects — the pre-s15 `_review_index` shape exactly.
#[derive(Debug, Default)]
struct ReviewIndex {
    composite: HashSet<(ProjectId, ScenarioId)>,
    bare: HashSet<ScenarioId>,
}

impl ReviewIndex {
    fn is_reviewed(&self, project_id: Option<&ProjectId>, scenario_id: &ScenarioId) -> bool {
        match project_id {
            Some(project_id) => self.composite.contains(&(project_id.clone(), scenario_id.clone())),
            None => self.bare.contains(scenario_id),
        }
    }
}

fn review_index(ledger_root: &Path) -> ReviewIndex {
    let mut index = ReviewIndex::default();
    let reviews = GitTier::new(ledger_root).read(&TierQuery::kind(RecordKind::Review)).map(|read| read.records).unwrap_or_default();
    for raw in reviews {
        let Ok(review) = serde_json::from_value::<Review>(raw.0) else { continue };
        index.composite.insert((review.project_id.clone(), review.scenario_id.clone()));
        index.bare.insert(review.scenario_id);
    }
    index
}

/// This evidence record's [`TrustLadderState`], read three-way off its
/// own native fields (module doc): absent → the documented safe
/// default, present → typed. A present-malformed field never reaches
/// this function at all — it already failed [`GateContext::load`]'s
/// deserialize and lives in `ctx.violations` instead.
fn ladder_state_of(record: &EvidenceRecord) -> TrustLadderState {
    TrustLadderState { lifecycle: record.lifecycle.unwrap_or(TrustLifecycle::Draft), flagged: record.flagged.clone().unwrap_or_else(FlaggedOverlay::clear) }
}

/// The always-on trust-ladder check (module doc): `reviewed` with no
/// matching review-record is `unreviewed-promotion`
/// ([`TrustRung::UnreviewedPromotion`]); the human-only `flagged`
/// overlay is always `flagged` ([`TrustRung::Flagged`]), regardless of
/// any passing evidence. Plain `draft` is never itself a violation
/// here — spec.md's "draft is never green" is the coverage/ledger
/// checks' job to report as not-green, not a distinct
/// `FAILURE_CLASSES` entry this module raises.
pub struct TrustLadderCheck;

impl GateCheck for TrustLadderCheck {
    fn name(&self) -> &'static str {
        "trust-ladder"
    }

    fn run(&self, ctx: &GateContext) -> Vec<Violation> {
        let reviewed = review_index(&ctx.ctx.ledger_root);

        let mut violations = Vec::new();
        for record in &ctx.evidence {
            let subject = subject_of(record);
            let state = ladder_state_of(record);
            let has_review = record.scenario_id.as_ref().is_some_and(|sid| reviewed.is_reviewed(record.project_id.as_ref(), sid));

            match state.rung(has_review) {
                TrustRung::UnreviewedPromotion => violations.push(Violation::new(
                    FailureClass::UnreviewedPromotion,
                    subject,
                    "`reviewed` lifecycle tag with no matching ledger review-record",
                )),
                TrustRung::Flagged => violations.push(Violation::new(
                    FailureClass::Flagged,
                    subject,
                    "human-only `flagged` overlay is set — never green regardless of any passing evidence",
                )),
                TrustRung::Draft | TrustRung::Agent | TrustRung::Human => {}
            }
        }
        violations
    }
}

/// The release-scoped `trust-below-required` check (design D2/D7,
/// spec.md "Severity below the required trust level at release") — a
/// SEPARATE, opt-in [`GateCheck`] from [`TrustLadderCheck`] (module
/// doc): a `class`-tagged artifact sitting below `policy.yaml`'s
/// `trust_required` for that class is fine mid-campaign and becomes a
/// finding only when a release profile additionally registers this
/// check (a future `canon gate check --profile release`, task 1.9,
/// not implemented here).
pub struct ReleaseTrustCheck;

impl GateCheck for ReleaseTrustCheck {
    fn name(&self) -> &'static str {
        "release-trust-required"
    }

    fn run(&self, ctx: &GateContext) -> Vec<Violation> {
        let raw_records = raw_evidence(&ctx.ctx.ledger_root);
        let reviewed = review_index(&ctx.ctx.ledger_root);
        let now = ctx.now;

        let mut violations = Vec::new();
        for raw in &raw_records {
            // `class` is a raw companion, never migrated (module doc)
            // — checked FIRST so a record with no `trust_required`
            // policy applying to it is skipped before even attempting
            // the (potentially malformed-at-the-whole-record-level)
            // native-field deserialize below.
            let Some(class) = raw.0.get("class").and_then(|v| v.as_str()) else { continue };
            let required = match ctx.policy.trust_required_for(class, &raw.0, now) {
                Ok(Some(level)) => level,
                Ok(None) | Err(_) => continue,
            };

            // A present-malformed native field fails this whole
            // record's deserialize (module doc) — already reported
            // once as `malformed-evidence` by `crate::ledger::LedgerCheck`;
            // this check silently skips it rather than reporting it a
            // second time under a different class.
            let Ok(record) = serde_json::from_value::<EvidenceRecord>(raw.0.clone()) else { continue };
            let subject = subject_of(&record);
            let state = ladder_state_of(&record);
            let has_review = record.scenario_id.as_ref().is_some_and(|sid| reviewed.is_reviewed(record.project_id.as_ref(), sid));
            let rung = state.rung(has_review);
            let achieved = rung.green();

            if achieved.is_none_or(|level| level < required) {
                let achieved_str = achieved.map(|level| level.as_str()).unwrap_or_else(|| rung.as_str());
                violations.push(Violation::new(
                    FailureClass::TrustBelowRequired,
                    subject,
                    format!("class `{class}` requires `{}` trust; record achieves `{achieved_str}`", required.as_str()),
                ));
            }
        }
        violations
    }
}

/// The `human` role a flag-clear's actor must carry (design decision
/// 2's risk mitigation: "clear-records require an attested actor
/// field the gate itself validates as never agent-originated") —
/// matches [`crate::trust_ladder::TrustLevel::Human`]'s own wire
/// string.
pub const HUMAN_ROLE: &str = "human";

/// Whether `actor` is structurally human-attributed — `role` must be
/// PRESENT and equal exactly [`HUMAN_ROLE`]. An absent role (an
/// unattributed/backfilled actor, `Actor::new_unattributed`) or any
/// other role string (an agent's own role slug, or free text an agent
/// process could set on `agent_id`) is never human, by construction —
/// an agent cannot mint a human-attributed clear no matter what it
/// names itself (design decision 2's risk section: "clear-records
/// require an attested actor field").
pub fn is_human_actor(actor: &Actor) -> bool {
    actor.role.as_ref().is_some_and(|role| role.as_str() == HUMAN_ROLE)
}

/// A flag-clear attempt was rejected — `clearing_actor` was not
/// human-attributed (spec.md "agent-originated clear-record is
/// rejected"). The overlay this attempt targeted remains flagged.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("flag-clear rejected: actor `{actor_agent_id}` is not human-attributed (role = {actor_role:?}) — the flagged overlay remains set")]
pub struct FlagClearRejected {
    pub actor_agent_id: String,
    pub actor_role: Option<String>,
}

/// Attempt to clear `current` via `clearing_actor` (design decision 2,
/// spec.md "clearing flagged requires a human-attributed clear-record"
/// and "agent-originated clear-record is rejected", trust-ladder.md
/// §3.3's one-way ratchet). Only [`is_human_actor`] ever succeeds;
/// every other actor is refused (`Err`, `current` untouched by the
/// caller) — the ratchet has no bypass. Clearing an already-unflagged
/// overlay is a harmless no-op, matching [`FlaggedOverlay::clear`]'s
/// own idempotent shape.
pub fn attempt_clear(current: &FlaggedOverlay, clearing_actor: &Actor) -> Result<FlaggedOverlay, FlagClearRejected> {
    if !current.flagged {
        return Ok(current.clone());
    }
    if is_human_actor(clearing_actor) {
        Ok(FlaggedOverlay::clear())
    } else {
        Err(FlagClearRejected {
            actor_agent_id: clearing_actor.agent_id.clone(),
            actor_role: clearing_actor.role.as_ref().map(|role| role.as_str().to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use canon_model::{Envelope, EvidenceVerdict, ProjectId, ProvenanceRef, RoleId};
    use chrono::Utc;
    use canon_policy::SchemaRegistry;
    use tempfile::TempDir;

    use super::*;
    use crate::context::GateCtx;

    /// A named, fixed UTC constant (s21 design.md R5: never
    /// `Utc::now()` in a test call site of `GateContext::load`).
    fn fixed_now() -> chrono::DateTime<Utc> {
        chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").unwrap().with_timezone(&Utc)
    }

    /// Write one `EvidenceRecord` through `tier` with its NATIVE
    /// `lifecycle`/`flagged` fields set (s15 P3b: no longer a raw
    /// `trust_ladder` companion, module doc). `class` (module doc:
    /// NOT migrated, `ReleaseTrustCheck`'s own raw companion) is
    /// merged onto the record's own serialized body as a plain
    /// top-level key when given — `EvidenceRecord`'s strict
    /// `Deserialize` silently drops it as an unknown field, exactly
    /// as a production write would look.
    #[allow(clippy::too_many_arguments)]
    fn write_tagged_evidence(tier: &GitTier, project_id: Option<&str>, scenario_id: &str, actor_role: &str, lifecycle: TrustLifecycle, flagged: bool, class: Option<&str>) {
        let actor = Actor::new("test-agent", RoleId::parse(actor_role).unwrap());
        let mut record = EvidenceRecord::new(
            Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), actor.clone()),
            None,
            Some(ScenarioId::parse(scenario_id).unwrap()),
            None,
            EvidenceVerdict::Faithful,
        )
        .with_lifecycle(lifecycle)
        .with_flagged(if flagged { FlaggedOverlay::set(actor, Utc::now()) } else { FlaggedOverlay::clear() });
        if let Some(project_id) = project_id {
            record = record.with_project_id(ProjectId::parse(project_id).unwrap());
        }

        let mut body = serde_json::to_value(&record).unwrap();
        if let Some(class) = class {
            body["class"] = serde_json::json!(class);
        }
        tier.write(&canon_store::tier::RawWrite(canon_model::RawRecord(body))).expect("write one tagged evidence record");
    }

    fn write_review(tier: &GitTier, project_id: &str, scenario_id: &str, pin: &str) {
        let review = Review::new(
            Envelope::new(1, RecordKind::Review, Utc::now(), Actor::new("reviewer-1", RoleId::parse("reviewer").unwrap())),
            ProjectId::parse(project_id).unwrap(),
            ScenarioId::parse(scenario_id).unwrap(),
            "reviewer-1",
            pin,
            ProvenanceRef::UpstreamRef(format!("routes/{scenario_id}#onReview")),
        );
        tier.write(&review).expect("write one review record");
    }

    #[test]
    fn reviewed_lifecycle_without_review_record_emits_unreviewed_promotion() {
        let dir = TempDir::new().unwrap();
        let gate_ctx = GateCtx::from_fixture(dir.path());
        let tier = GitTier::new(&gate_ctx.ledger_root);
        write_tagged_evidence(&tier, None, "world.firstbuy-hotdeal.26", "implementer", TrustLifecycle::Reviewed, false, None);

        let registry = SchemaRegistry::load();
        let ctx = GateContext::load(gate_ctx, &registry, fixed_now()).unwrap();

        let violations = TrustLadderCheck.run(&ctx);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].class, FailureClass::UnreviewedPromotion);
        assert_eq!(violations[0].subject, "world.firstbuy-hotdeal.26");
    }

    #[test]
    fn reviewed_with_matching_review_record_is_not_a_violation() {
        let dir = TempDir::new().unwrap();
        let gate_ctx = GateCtx::from_fixture(dir.path());
        let tier = GitTier::new(&gate_ctx.ledger_root);
        write_tagged_evidence(&tier, None, "world.firstbuy-hotdeal.26", "implementer", TrustLifecycle::Reviewed, false, None);
        write_review(&tier, "root", "world.firstbuy-hotdeal.26", "abc123pin");

        let registry = SchemaRegistry::load();
        let ctx = GateContext::load(gate_ctx, &registry, fixed_now()).unwrap();
        assert!(TrustLadderCheck.run(&ctx).is_empty());
    }

    #[test]
    fn flagged_overlay_emits_flagged_violation_even_when_ratified() {
        let dir = TempDir::new().unwrap();
        let gate_ctx = GateCtx::from_fixture(dir.path());
        let tier = GitTier::new(&gate_ctx.ledger_root);
        write_tagged_evidence(&tier, None, "world.firstbuy-hotdeal.14", "implementer", TrustLifecycle::Ratified, true, None);

        let registry = SchemaRegistry::load();
        let ctx = GateContext::load(gate_ctx, &registry, fixed_now()).unwrap();

        let violations = TrustLadderCheck.run(&ctx);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].class, FailureClass::Flagged);
    }

    #[test]
    fn absent_native_trust_fields_default_to_draft_and_are_not_themselves_a_violation() {
        let dir = TempDir::new().unwrap();
        let gate_ctx = GateCtx::from_fixture(dir.path());
        let tier = GitTier::new(&gate_ctx.ledger_root);
        let record = EvidenceRecord::new(
            Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), Actor::new("test-agent", RoleId::parse("implementer").unwrap())),
            None,
            None,
            None,
            EvidenceVerdict::Faithful,
        );
        tier.write(&record).unwrap();

        let registry = SchemaRegistry::load();
        let ctx = GateContext::load(gate_ctx, &registry, fixed_now()).unwrap();
        assert!(TrustLadderCheck.run(&ctx).is_empty());
    }

    #[test]
    fn review_for_one_project_does_not_satisfy_another_projects_same_scenario_id_evidence() {
        // design D12 / `gate-native-record-fields` spec: a `Review`
        // for `(app-a, X)` must not satisfy `(app-b, X)` evidence.
        let dir = TempDir::new().unwrap();
        let gate_ctx = GateCtx::from_fixture(dir.path());
        let tier = GitTier::new(&gate_ctx.ledger_root);
        write_review(&tier, "app-a", "world.firstbuy-hotdeal.26", "abc123pin");
        write_tagged_evidence(&tier, Some("app-b"), "world.firstbuy-hotdeal.26", "implementer", TrustLifecycle::Reviewed, false, None);

        let registry = SchemaRegistry::load();
        let ctx = GateContext::load(gate_ctx, &registry, fixed_now()).unwrap();

        let violations = TrustLadderCheck.run(&ctx);
        assert_eq!(violations.len(), 1, "violations: {violations:?}");
        assert_eq!(violations[0].class, FailureClass::UnreviewedPromotion);
    }

    #[test]
    fn a_matching_project_review_satisfies_the_same_projects_evidence() {
        let dir = TempDir::new().unwrap();
        let gate_ctx = GateCtx::from_fixture(dir.path());
        let tier = GitTier::new(&gate_ctx.ledger_root);
        write_review(&tier, "app-a", "world.firstbuy-hotdeal.26", "abc123pin");
        write_tagged_evidence(&tier, Some("app-a"), "world.firstbuy-hotdeal.26", "implementer", TrustLifecycle::Reviewed, false, None);

        let registry = SchemaRegistry::load();
        let ctx = GateContext::load(gate_ctx, &registry, fixed_now()).unwrap();
        assert!(TrustLadderCheck.run(&ctx).is_empty());
    }

    #[test]
    fn legacy_none_project_evidence_still_matches_by_bare_scenario_id() {
        let dir = TempDir::new().unwrap();
        let gate_ctx = GateCtx::from_fixture(dir.path());
        let tier = GitTier::new(&gate_ctx.ledger_root);
        write_review(&tier, "app-a", "world.firstbuy-hotdeal.26", "abc123pin");
        // No `project_id` at all — pre-s15/legacy evidence.
        write_tagged_evidence(&tier, None, "world.firstbuy-hotdeal.26", "implementer", TrustLifecycle::Reviewed, false, None);

        let registry = SchemaRegistry::load();
        let ctx = GateContext::load(gate_ctx, &registry, fixed_now()).unwrap();
        assert!(TrustLadderCheck.run(&ctx).is_empty());
    }

    #[test]
    fn class_below_required_release_trust_level_emits_trust_below_required() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".canon")).unwrap();
        std::fs::write(dir.path().join(".canon").join("policy.yaml"), "trust_required:\n  p1: human\n  p2: agent\n").unwrap();

        let gate_ctx = GateCtx::from_fixture(dir.path());
        let tier = GitTier::new(&gate_ctx.ledger_root);

        // class "p1" requires human; this record only achieves agent (reviewed + review-record) -> violation.
        write_tagged_evidence(&tier, None, "world.firstbuy-hotdeal.26", "implementer", TrustLifecycle::Reviewed, false, Some("p1"));
        write_review(&tier, "root", "world.firstbuy-hotdeal.26", "abc123pin");

        // class "p2" requires agent; this record ALSO achieves agent -> no violation (contrast case).
        write_tagged_evidence(&tier, None, "world.firstbuy-hotdeal.27", "implementer", TrustLifecycle::Reviewed, false, Some("p2"));
        write_review(&tier, "root", "world.firstbuy-hotdeal.27", "def456pin");

        let registry = SchemaRegistry::load();
        let ctx = GateContext::load(gate_ctx, &registry, fixed_now()).unwrap();
        assert!(ctx.policy.is_clean(), "diagnostics: {:?}", ctx.policy.diagnostics);

        let violations = ReleaseTrustCheck.run(&ctx);
        assert_eq!(violations.len(), 1, "violations: {violations:?}");
        assert_eq!(violations[0].class, FailureClass::TrustBelowRequired);
        assert_eq!(violations[0].subject, "world.firstbuy-hotdeal.26");

        // the always-on check must stay silent on both — they both carry review-records.
        assert!(TrustLadderCheck.run(&ctx).is_empty());
    }

    #[test]
    fn release_trust_check_silently_skips_a_present_malformed_native_field_record() {
        // A present-malformed native field fails the WHOLE record's
        // deserialize at load time now (module doc) — it never
        // reaches `ctx.evidence`, and `raw_evidence`'s own
        // `GitTier::read` (the identical validation path) excludes it
        // too. `ReleaseTrustCheck` alone therefore finds nothing to
        // report for it — `crate::ledger::LedgerCheck`/`check_set` is
        // what surfaces `malformed-evidence` now (`dispatch.rs`'s own
        // test proves that, through the NORMAL dispatcher).
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".canon")).unwrap();
        std::fs::write(dir.path().join(".canon").join("policy.yaml"), "trust_required:\n  p1: human\n").unwrap();

        let gate_ctx = GateCtx::from_fixture(dir.path());
        let tier = GitTier::new(&gate_ctx.ledger_root);

        let body = serde_json::json!({
            "schema": 1,
            "kind": "evidence_record",
            "at": Utc::now().to_rfc3339(),
            "actor": {"agent_id": "test-agent", "role": "implementer"},
            "scenario_id": "world.firstbuy-hotdeal.91",
            "verdict": "faithful",
            "lifecycle": "bogus-lifecycle",
            "class": "p1",
        });
        tier.write(&canon_store::tier::RawWrite(canon_model::RawRecord(body))).expect("write one malformed-native-field record");

        let registry = SchemaRegistry::load();
        let ctx = GateContext::load(gate_ctx, &registry, fixed_now()).unwrap();
        assert_eq!(ctx.violations.len(), 1, "violations: {:?}", ctx.violations);

        assert!(ReleaseTrustCheck.run(&ctx).is_empty());
        assert!(TrustLadderCheck.run(&ctx).is_empty());
    }

    #[test]
    fn attempt_clear_rejects_an_agent_originated_actor() {
        let overlay = FlaggedOverlay::set(Actor::new("agent-007", RoleId::parse("implementer").unwrap()), Utc::now());
        let forger = Actor::new("agent-007", RoleId::parse("implementer").unwrap());

        let err = attempt_clear(&overlay, &forger).unwrap_err();
        assert_eq!(err.actor_agent_id, "agent-007");
        assert_eq!(err.actor_role.as_deref(), Some("implementer"));
    }

    #[test]
    fn attempt_clear_rejects_an_unattributed_actor_even_if_it_names_itself_human() {
        let overlay = FlaggedOverlay::set(Actor::new("agent-007", RoleId::parse("implementer").unwrap()), Utc::now());
        // `agent_id` free text SAYS "human-operator", but `role` is absent
        // (`Actor::new_unattributed`) — only a structured `role == "human"`
        // counts; free text never does.
        let forger = Actor::new_unattributed("human-operator");

        assert!(attempt_clear(&overlay, &forger).is_err());
    }

    #[test]
    fn attempt_clear_honors_a_genuinely_human_attributed_actor() {
        let overlay = FlaggedOverlay::set(Actor::new("agent-007", RoleId::parse("implementer").unwrap()), Utc::now());
        let operator = Actor::new("jane", RoleId::parse(HUMAN_ROLE).unwrap());

        let cleared = attempt_clear(&overlay, &operator).unwrap();
        assert!(!cleared.flagged);
    }

    #[test]
    fn attempt_clear_on_an_already_clear_overlay_is_a_harmless_no_op() {
        let overlay = FlaggedOverlay::clear();
        let anyone = Actor::new("agent-007", RoleId::parse("implementer").unwrap());

        let result = attempt_clear(&overlay, &anyone).unwrap();
        assert!(!result.flagged);
    }
}
