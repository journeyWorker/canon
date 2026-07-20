//! The shared record envelope (S1 design D2, task 1.1).
//!
//! Every one of canon-model's thirteen record kinds composes [`Envelope`]
//! via `#[serde(flatten)]` — no record type defines its own ad hoc
//! actor/`by` field; the only path to attribution is `Envelope.actor`.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{RoleId, SessionId};

/// The thirteen closed record kinds `canon-model` recognizes (design D1;
/// `Subject` is the reviewed 13th kind, added by s36 — the "a new kind is
/// a reviewed, breaking `canon-model` change" process design D1 mandates,
/// exercised for real). A fourteenth kind is again a reviewed, breaking
/// change — never a `kind: String` escape hatch (design D1's explicitly
/// rejected alternative: an untyped `payload: serde_json::Value` would
/// silently recreate the "no documented join key" problem inside canon
/// itself).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecordKind {
    Change,
    Task,
    Scenario,
    Session,
    Run,
    Event,
    Handoff,
    Review,
    Divergence,
    Trajectory,
    StrategyItem,
    EvidenceRecord,
    // The product/management unit (s36): the durable subject a team
    // plans, designs, builds, and measures across many changes — the
    // reviewed 13th kind (design D1's process). Plain line comment, NOT
    // a doc comment: a per-variant doc comment makes schemars split the
    // exported `RecordKind` schema into a `oneOf` (one arm per
    // documented variant) instead of the flat `enum` canon-policy's
    // schema walker resolves into a CEL enum domain.
    Subject,
}

impl RecordKind {
    /// All thirteen kinds, in the same order the proposal/design docs
    /// list them — the one iteration point schema export (task 3.2) and
    /// the fixture round-trip test (task 6.2) both walk, so "thirteen
    /// kinds" is asserted structurally (`RecordKind::ALL.len() == 13`)
    /// rather than by a comment that can drift from the enum.
    pub const ALL: [RecordKind; 13] = [
        RecordKind::Change,
        RecordKind::Task,
        RecordKind::Scenario,
        RecordKind::Session,
        RecordKind::Run,
        RecordKind::Event,
        RecordKind::Handoff,
        RecordKind::Review,
        RecordKind::Divergence,
        RecordKind::Trajectory,
        RecordKind::StrategyItem,
        RecordKind::EvidenceRecord,
        RecordKind::Subject,
    ];

    /// The wire string this kind serializes to (the `kind` field's
    /// value) — stable, snake_case, matches
    /// `#[serde(rename_all = "snake_case")]` exactly (asserted by a
    /// test, so the two can never silently disagree).
    pub fn as_str(self) -> &'static str {
        match self {
            RecordKind::Change => "change",
            RecordKind::Task => "task",
            RecordKind::Scenario => "scenario",
            RecordKind::Session => "session",
            RecordKind::Run => "run",
            RecordKind::Event => "event",
            RecordKind::Handoff => "handoff",
            RecordKind::Review => "review",
            RecordKind::Divergence => "divergence",
            RecordKind::Trajectory => "trajectory",
            RecordKind::StrategyItem => "strategy_item",
            RecordKind::EvidenceRecord => "evidence_record",
            RecordKind::Subject => "subject",
        }
    }

    /// The Hive-style path TEMPLATE this kind's git-tier files follow
    /// (S2 design D2, task 1.2) — a pure, storage-agnostic path
    /// template string; `canon-model` never resolves `{area}`/`{id}`
    /// itself and never imports `canon-store` — only `canon-store`'s
    /// `GitTier` interprets this template against a filesystem
    /// (design D2's Risk-section mitigation). Exactly two shapes,
    /// mirroring `tools/parity.py::_ledger_layout_problem`'s `run`/
    /// `drill` (flat) vs. `review`/`design-review`/`code-review`/
    /// `clear` (nested) split:
    /// - flat: `"kind={kind}/{id}.json"` — every kind without a
    ///   mandatory `scenario_id`.
    /// - area-scoped: `"kind={kind}/area={area}/{id}.json"` — the
    ///   three kinds whose `scenario_id` field is non-`Option`
    ///   ([`crate::records::Scenario`], [`crate::records::Review`],
    ///   [`crate::records::Divergence`]; see [`Self::is_area_scoped`]).
    ///   `{area}` MUST be resolved via `ScenarioId::area()`, never
    ///   trusted from a source directory (`tools/parity.py::_area_of`,
    ///   six documented mismatch cases).
    pub fn partition_template(self) -> &'static str {
        if self.is_area_scoped() {
            "kind={kind}/area={area}/{id}.json"
        } else {
            "kind={kind}/{id}.json"
        }
    }

    /// Whether this kind's [`Self::partition_template`] requires the
    /// Hive `area={area}/` segment — true for exactly the kinds whose
    /// `scenario_id` field is mandatory (non-`Option`), false for the
    /// other nine (including [`RecordKind::EvidenceRecord`], whose
    /// `scenario_id` is present-but-optional per S1 design — an
    /// evidence record without a scenario tie is still a flat, valid
    /// record).
    pub fn is_area_scoped(self) -> bool {
        matches!(self, RecordKind::Scenario | RecordKind::Review | RecordKind::Divergence)
    }
}

/// Who/what produced a record — structured, never a bare `by: String`
/// (design D2, the donor audit's biggest cross-family gap).
///
/// `role` is `Option` (S11 design D5, "actor backfill is best-effort
/// from adjacent fields; absent stays absent"): a migrated legacy
/// record whose only source field is a bare `by: "legacy-ci-machine"`
/// string carries no role information at all — `canon migrate` maps
/// that string to `agent_id` and leaves `role: null` rather than
/// guessing, so the field must be able to express "unknown", not just
/// "known". Every record canon itself originates still supplies a role
/// via [`Actor::new`]; only backfilled/migrated records use
/// [`Actor::new_unattributed`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Actor {
    pub agent_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<RoleId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<SessionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl Actor {
    pub fn new(agent_id: impl Into<String>, role: RoleId) -> Self {
        Self { agent_id: agent_id.into(), role: Some(role), session_id: None, model: None }
    }

    /// A best-effort-backfilled actor whose source data carried no role
    /// (S11 design D5) — e.g. a ledger `run` record's bare
    /// `by: "legacy-ci-machine"` string, which names only an
    /// `agent_id`. Never used for a record canon itself originates.
    pub fn new_unattributed(agent_id: impl Into<String>) -> Self {
        Self { agent_id: agent_id.into(), role: None, session_id: None, model: None }
    }

    pub fn with_session(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }
}

/// The envelope every canon-model record carries (design D2):
/// `{schema, kind, at, actor}`. Composed via `#[serde(flatten)]` into
/// every record struct, never duplicated per type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Envelope {
    /// Per-kind schema version, bumped on any breaking field change to
    /// that kind (design D2; `canon fmt`/`canon migrate`, S11, key off
    /// it).
    pub schema: u32,
    pub kind: RecordKind,
    pub at: DateTime<Utc>,
    pub actor: Actor,
}

impl Envelope {
    pub fn new(schema: u32, kind: RecordKind, at: DateTime<Utc>, actor: Actor) -> Self {
        Self { schema, kind, at, actor }
    }
}

/// Implemented by every one of the thirteen closed record kinds — the one
/// dispatch point schema export, the fixture loader, and the round-trip
/// tests use instead of re-deriving a kind ↔ type mapping per caller.
pub trait CanonRecord: Serialize + for<'de> Deserialize<'de> + JsonSchema {
    /// This type's fixed [`RecordKind`]. Every constructor sets
    /// `envelope().kind` to this value; the round-trip tests assert the
    /// two never disagree.
    const KIND: RecordKind;

    fn envelope(&self) -> &Envelope;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_thirteen_kinds_present_exactly_once() {
        assert_eq!(RecordKind::ALL.len(), 13);
        let mut seen = std::collections::HashSet::new();
        for kind in RecordKind::ALL {
            assert!(seen.insert(kind), "{kind:?} listed twice in RecordKind::ALL");
        }
    }

    #[test]
    fn as_str_matches_serde_rename_all_snake_case() {
        for kind in RecordKind::ALL {
            let json = serde_json::to_string(&kind).unwrap();
            assert_eq!(json, format!("\"{}\"", kind.as_str()), "{kind:?} as_str() disagrees with its own serde encoding");
        }
    }

    #[test]
    fn actor_never_has_a_bare_by_field() {
        let actor = Actor::new("codex-cli", RoleId::parse("implementer").unwrap());
        let json = serde_json::to_value(&actor).unwrap();
        assert!(json.get("by").is_none());
        assert!(json.get("agent_id").is_some());
        assert!(json.get("role").is_some());
    }

    /// S11 design D5: a migration-backfilled actor whose source data
    /// named only an agent (never a role) omits `role` from the wire
    /// form entirely — `skip_serializing_if` on `None`, not a literal
    /// `"role": null`, so an old reader that only checks
    /// `actor.get("role").is_some()` sees an honestly-absent field.
    #[test]
    fn unattributed_actor_omits_role() {
        let actor = Actor::new_unattributed("legacy-ci-machine");
        let json = serde_json::to_value(&actor).unwrap();
        assert_eq!(json.get("agent_id").and_then(|v| v.as_str()), Some("legacy-ci-machine"));
        assert!(json.get("role").is_none());
        let round_tripped: Actor = serde_json::from_value(json).unwrap();
        assert_eq!(round_tripped, actor);
    }
}
