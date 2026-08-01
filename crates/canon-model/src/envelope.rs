//! The shared record envelope (S1 design D2, task 1.1).
//!
//! Every one of canon-model's fourteen record kinds composes [`Envelope`]
//! via `#[serde(flatten)]` — no record type defines its own ad hoc
//! actor/`by` field; the only path to attribution is `Envelope.actor`.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{RoleId, SessionId};

/// The fourteen closed record kinds `canon-model` recognizes (design D1;
/// `Subject` is the reviewed 13th kind, added by s36, and `Finding` the
/// reviewed 14th, added by s43 — the "a new kind is a reviewed, breaking
/// `canon-model` change" process design D1 mandates, exercised twice for
/// real). A fifteenth kind is again a reviewed, breaking
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
    // One code-review finding (s43): the durable home for what used to
    // live only in an agent transcript, so a release narrative's
    // "N issues, M of them fix-of-fix" is DERIVED from records rather
    // than hand-counted into a git tag. The reviewed 14th kind. Plain
    // line comment, NOT a doc comment — see the `Subject` note above.
    Finding,
}

impl RecordKind {
    /// All fourteen kinds, in the same order the proposal/design docs
    /// list them — the one iteration point schema export (task 3.2) and
    /// the fixture round-trip test (task 6.2) both walk, so "fourteen
    /// kinds" is asserted structurally (`RecordKind::ALL.len() == 14`)
    /// rather than by a comment that can drift from the enum.
    pub const ALL: [RecordKind; 14] = [
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
        RecordKind::Finding,
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
            RecordKind::Finding => "finding",
        }
    }

    /// This kind's CURRENT [`Envelope::schema`] format generation — the
    /// ONE value every writer of this kind stamps, and the
    /// `s38-evidence-bearing-memory` supersession discriminator
    /// `canon_store::fold::fold_latest_by_key` compares on an equal
    /// `at`. Before this registry the version lived as a hand-written
    /// `SCHEMA_VERSION` const inside each writer (`canon-ingest`'s
    /// normalizer, EACH plan adapter, `canon-cli`'s dispatch and gate
    /// paths), which could not even EXPRESS a per-kind bump — one
    /// adapter's single const stamped both its `Change` and its `Task`
    /// — and which `canon-cli::dispatch`'s own doc already flagged as a
    /// must-agree-by-hand drift hazard. `canon-model` owns the record
    /// formats, so it owns their generations; `canon context`'s
    /// per-kind `schema_version` reads this same function, so the
    /// authoring surface can never advertise a generation nothing
    /// writes.
    ///
    /// # When to bump
    /// Bump a kind IFF two generations of ONE of its records can carry
    /// an IDENTICAL `Envelope.at`, because that is exactly when the
    /// fold needs a generation signal to order them. The rule is NOT
    /// "bump on every field addition":
    /// - **`Task` is `2`** — it gained `depends_on`
    ///   (`s37-execution-graph-topology`) and its `at` is
    ///   `file_modified_at(<source plan doc>)` (s20 D7, byte-stable so
    ///   an unchanged plan re-imports idempotently). A canon PARSER
    ///   change does not move that mtime, so the pre-change and
    ///   post-change record for one `task_id` tie on `at` exactly, and
    ///   without this bump the fold resolved that tie by lexicographic
    ///   digest — arbitrarily, per row.
    /// - **`Run` and `Handoff` stay `1`** even though they ALSO gained
    ///   fields on the same branch (`Run.parent_run_id`,
    ///   `Run.injected_guidance`, the handoff role fields). Their `at`
    ///   is stamped at DERIVATION time (`Utc::now()` at the dispatch
    ///   boundary, or the ingested transcript's own instant), so a
    ///   re-derivation always advances `at` and the two generations can
    ///   never tie. No tie, no discriminator needed, no bump owed.
    ///
    /// A field addition that is additive on the wire (`#[serde(default,
    /// skip_serializing_if)]`) still leaves the pre-change corpus
    /// byte-identical and digest-stable — that is what makes this a
    /// generation MARKER for the fold, not a migration trigger.
    pub const fn schema_version(self) -> u32 {
        match self {
            RecordKind::Task => 2,
            RecordKind::Change
            | RecordKind::Scenario
            | RecordKind::Session
            | RecordKind::Run
            | RecordKind::Event
            | RecordKind::Handoff
            | RecordKind::Review
            | RecordKind::Divergence
            | RecordKind::Trajectory
            | RecordKind::StrategyItem
            | RecordKind::EvidenceRecord
            | RecordKind::Subject
            | RecordKind::Finding => 1,
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
    /// other eleven (including [`RecordKind::EvidenceRecord`], whose
    /// `scenario_id` is present-but-optional per S1 design — an
    /// evidence record without a scenario tie is still a flat, valid
    /// record, and [`RecordKind::Finding`], which is scoped to a
    /// reviewed COMMIT rather than to a scenario at all).
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
    /// Per-kind FORMAT GENERATION — the value
    /// [`RecordKind::schema_version`] defines and every writer stamps
    /// via [`Envelope::current`] (design D2; `canon fmt`/`canon
    /// migrate`, S11, key off it; `canon_store::fold::fold_latest_by_key`
    /// orders an equal-`at` supersession tie by it,
    /// `s38-evidence-bearing-memory`). See
    /// [`RecordKind::schema_version`] for the bump rule — it is "bump
    /// when two generations of one record can tie on `at`", not "bump on
    /// every field addition".
    pub schema: u32,
    pub kind: RecordKind,
    pub at: DateTime<Utc>,
    pub actor: Actor,
}

impl Envelope {
    /// Construct an envelope at an EXPLICIT generation — for a test or
    /// compat fixture that deliberately models an OLDER generation of a
    /// kind. Production writers use [`Envelope::current`] instead, so no
    /// writer can stamp a generation [`RecordKind::schema_version`] does
    /// not define.
    pub fn new(schema: u32, kind: RecordKind, at: DateTime<Utc>, actor: Actor) -> Self {
        Self { schema, kind, at, actor }
    }

    /// Construct an envelope at `kind`'s CURRENT generation
    /// ([`RecordKind::schema_version`]) — the one constructor every
    /// production writer uses (`s38-evidence-bearing-memory`). It
    /// replaces the per-writer `SCHEMA_VERSION` consts that previously
    /// each hand-maintained their own copy of this integer, which could
    /// not express a per-kind bump and had to agree by convention.
    pub fn current(kind: RecordKind, at: DateTime<Utc>, actor: Actor) -> Self {
        Self { schema: kind.schema_version(), kind, at, actor }
    }
}

/// Implemented by every one of the fourteen closed record kinds — the one
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
    fn all_fourteen_kinds_present_exactly_once() {
        assert_eq!(RecordKind::ALL.len(), 14);
        let mut seen = std::collections::HashSet::new();
        for kind in RecordKind::ALL {
            assert!(seen.insert(kind), "{kind:?} listed twice in RecordKind::ALL");
        }
    }

    /// The `s38-evidence-bearing-memory` bump, asserted structurally so
    /// a future field addition cannot quietly bump a kind whose `at` is
    /// derivation-time (and therefore cannot tie) — see
    /// [`RecordKind::schema_version`]'s bump rule. `Run` and `Handoff`
    /// are named explicitly because both DID gain fields on the same
    /// branch that bumped `Task` and both deliberately stayed at `1`.
    #[test]
    fn only_task_carries_the_bumped_schema_generation() {
        assert_eq!(RecordKind::Task.schema_version(), 2, "Task gained `depends_on` and its byte-stable `at` can tie");
        assert_eq!(RecordKind::Run.schema_version(), 1, "Run's `at` is derivation-time, so its generations can never tie on `at`");
        assert_eq!(RecordKind::Handoff.schema_version(), 1, "Handoff's `at` is derivation-time, so its generations can never tie on `at`");
        for kind in RecordKind::ALL {
            if kind == RecordKind::Task {
                continue;
            }
            assert_eq!(kind.schema_version(), 1, "{kind:?} must stay at generation 1 until its records can tie on `at`");
        }
    }

    #[test]
    fn envelope_current_stamps_the_kinds_own_generation() {
        for kind in RecordKind::ALL {
            let envelope = Envelope::current(kind, DateTime::UNIX_EPOCH, Actor::new_unattributed("test"));
            assert_eq!(envelope.schema, kind.schema_version());
            assert_eq!(envelope.kind, kind);
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
