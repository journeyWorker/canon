//! Evidence integrity (S1 spec `evidence-integrity`, design D6, tasks 4.*).
//!
//! "Malformed evidence is no evidence": [`validate_evidence`] either
//! accepts a raw candidate or returns a structured [`EvidenceViolation`]
//! — it never panics, and a caller's batch-read loop
//! ([`validate_evidence_batch`]) always continues past one bad record.
//! Mirrors `tools/parity.py`'s `_load_ledger`/`_ledger_problem`
//! skip-not-crash contract exactly (design D6).

use serde::{Deserialize, Serialize};

use crate::ids::ScenarioId;
use crate::records::EvidenceRecord;

/// A fixed, named set of failure classes (design D6) — every crate that
/// raises an [`EvidenceViolation`] reuses these, never inventing its own
/// ad hoc strings. `malformed` is seeded directly from
/// `tools/parity.py::FAILURE_CLASSES` (task 4.1); the others are new,
/// canon-specific classes for concerns `parity.py` doesn't have
/// (Handoff's closed state machine and per-domain template registry).
///
/// Renaming a variant's [`FailureClass::as_str`] value is a
/// `canon-model` `schema` version bump shipped together with updated
/// fixtures (evidence-integrity spec, "Renaming a failure class requires
/// a coordinated migration") — never a silent rename.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum FailureClass {
    /// Envelope or body content does not parse / is missing a required
    /// field. Seeded from `tools/parity.py::FAILURE_CLASSES`'s
    /// `"malformed"` (".feature does not parse").
    Malformed,
    /// A join-spine key field is present but fails its own grammar
    /// (e.g. `scenario_id: "Not Valid!!"`).
    InvalidJoinKey,
    /// A `Handoff` state transition was attempted from a terminal state
    /// (`done`/`abandoned`) or otherwise outside the closed state
    /// machine (handoff-state-machine spec).
    InvalidStateTransition,
    /// A `Handoff.body.domain` is not registered in the active
    /// `canon.yaml`'s `handoff_templates:` (handoff-state-machine spec).
    UnregisteredHandoffDomain,
    /// A registered domain's template rejected `Handoff.body.fields`
    /// (missing/invalid field).
    InvalidHandoffBody,
}

impl FailureClass {
    /// The stable, grep-able wire string (evidence-integrity spec:
    /// "byte-identical" across patch releases). Matches
    /// `#[serde(rename_all = "kebab-case")]` exactly — asserted by a
    /// test below, so the two representations cannot silently diverge.
    pub fn as_str(self) -> &'static str {
        match self {
            FailureClass::Malformed => "malformed",
            FailureClass::InvalidJoinKey => "invalid-join-key",
            FailureClass::InvalidStateTransition => "invalid-state-transition",
            FailureClass::UnregisteredHandoffDomain => "unregistered-handoff-domain",
            FailureClass::InvalidHandoffBody => "invalid-handoff-body",
        }
    }

    /// Parse a failure-class wire string back to its variant — used by
    /// the fixture corpus's `EXPECTED-violations.json` reader (task
    /// 6.2), never by production validation code.
    pub fn from_str_exact(s: &str) -> Option<Self> {
        [
            FailureClass::Malformed,
            FailureClass::InvalidJoinKey,
            FailureClass::InvalidStateTransition,
            FailureClass::UnregisteredHandoffDomain,
            FailureClass::InvalidHandoffBody,
        ]
        .into_iter()
        .find(|c| c.as_str() == s)
    }
}

/// Why a candidate record is malformed evidence — structured, never a
/// bare string (design D6). `subject` names the record/field at fault
/// (mirrors `Violation.subject()` in `tools/parity.py`); `detail` is a
/// human-readable explanation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("[{}] {subject}: {detail}", class.as_str())]
pub struct EvidenceViolation {
    pub class: FailureClass,
    pub subject: String,
    pub detail: String,
}

impl EvidenceViolation {
    pub fn new(class: FailureClass, subject: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { class, subject: subject.into(), detail: detail.into() }
    }
}

/// An untyped candidate record read from a raw evidence source (a ledger
/// file, an ingest stream, …) before it is known to be well-formed.
/// Thin wrapper over [`serde_json::Value`] — the untyped-candidate
/// analog of `tools/parity.py::_load_ledger`'s `dict` records.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawRecord(pub serde_json::Value);

impl std::str::FromStr for RawRecord {
    type Err = serde_json::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(serde_json::from_str(s)?))
    }
}

impl From<serde_json::Value> for RawRecord {
    fn from(v: serde_json::Value) -> Self {
        Self(v)
    }
}

/// Envelope-shape check every record kind's validation reuses (not just
/// `EvidenceRecord`'s): the candidate is a JSON object carrying
/// `schema` (number), `kind` (string), `at` (RFC3339 string), and
/// `actor` (`{agent_id: string, role?: string, ...}`). Returns the
/// first missing/invalid field as a [`FailureClass::Malformed`]
/// violation. `actor.role` is optional (S11 design D5: a
/// migration-backfilled actor may carry no role at all) — only a
/// *present but non-string* `role` is malformed, mirroring
/// [`check_optional_scenario_id`]'s "absence is not itself a
/// violation" discipline.
pub fn validate_envelope_shape(candidate: &RawRecord) -> Result<(), EvidenceViolation> {
    let malformed = |subject: &str, detail: &str| {
        Err(EvidenceViolation::new(FailureClass::Malformed, subject, detail))
    };

    let obj = match candidate.0.as_object() {
        Some(obj) => obj,
        None => return malformed("<candidate>", "not a JSON object"),
    };

    if !obj.get("schema").is_some_and(|v| v.is_u64() || v.is_i64()) {
        return malformed("schema", "missing or non-integer `schema` field");
    }
    if !obj.get("kind").is_some_and(|v| v.is_string()) {
        return malformed("kind", "missing or non-string `kind` field");
    }
    match obj.get("at") {
        Some(serde_json::Value::String(s)) if chrono::DateTime::parse_from_rfc3339(s).is_ok() => {}
        Some(serde_json::Value::String(_)) => return malformed("at", "`at` is not a valid RFC3339 timestamp"),
        _ => return malformed("at", "missing or non-string `at` field"),
    }

    let actor = match obj.get("actor").and_then(|v| v.as_object()) {
        Some(actor) => actor,
        None => return malformed("actor", "missing or non-object `actor` field"),
    };
    if !actor.get("agent_id").is_some_and(|v| v.is_string()) {
        return malformed("actor.agent_id", "missing or non-string `actor.agent_id` field");
    }
    if actor.get("role").is_some_and(|v| !v.is_string()) {
        return malformed("actor.role", "`actor.role` present but non-string");
    }

    Ok(())
}

/// If `field` is present in `candidate` as a string, it must parse as a
/// well-formed [`ScenarioId`]. Absence is not itself a violation (not
/// every evidence kind carries a `scenario_id`) — only a *present but
/// malformed* value is.
fn check_optional_scenario_id(candidate: &RawRecord, field: &str) -> Result<(), EvidenceViolation> {
    let Some(value) = candidate.0.get(field) else { return Ok(()) };
    let Some(s) = value.as_str() else {
        return Err(EvidenceViolation::new(FailureClass::InvalidJoinKey, field, "not a string"));
    };
    ScenarioId::parse(s)
        .map(|_| ())
        .map_err(|e| EvidenceViolation::new(FailureClass::InvalidJoinKey, field, e.to_string()))
}

/// `canon_model::validate_evidence` (tasks 4.2/4.3, evidence-integrity
/// spec): given a raw `EvidenceRecord` candidate, either accept it
/// (`Ok(())`) or return a structured [`EvidenceViolation`] — never
/// panics. Well-formed candidates round-trip through
/// [`EvidenceRecord`]'s own `Deserialize` impl (so any future field
/// this function doesn't special-case is still checked); malformed
/// candidates are reported with the most specific [`FailureClass`]
/// available (envelope-shape problems as `Malformed`, a present-but-bad
/// join key as `InvalidJoinKey`, anything else caught by the full
/// deserialize attempt as `Malformed`).
pub fn validate_evidence(candidate: &RawRecord) -> Result<(), EvidenceViolation> {
    validate_envelope_shape(candidate)?;
    check_optional_scenario_id(candidate, "scenario_id")?;

    serde_json::from_value::<EvidenceRecord>(candidate.0.clone())
        .map(|_| ())
        .map_err(|e| EvidenceViolation::new(FailureClass::Malformed, "<candidate>", e.to_string()))
}

/// Batch form mirroring `tools/parity.py::_load_ledger`'s skip-not-crash
/// loop: every candidate is validated independently; one malformed
/// record is reported and skipped, never aborting the batch (task 4.3's
/// "a batch of five records with one malformed entry processes the
/// other four without aborting").
pub fn validate_evidence_batch(candidates: &[RawRecord]) -> (Vec<EvidenceRecord>, Vec<EvidenceViolation>) {
    let mut accepted = Vec::new();
    let mut violations = Vec::new();
    for candidate in candidates {
        match validate_evidence(candidate) {
            Ok(()) => {
                // `validate_evidence` already proved this deserializes;
                // re-deserializing here (rather than threading the value
                // through) keeps the two functions independently
                // correct and independently testable.
                if let Ok(record) = serde_json::from_value::<EvidenceRecord>(candidate.0.clone()) {
                    accepted.push(record);
                }
            }
            Err(violation) => violations.push(violation),
        }
    }
    (accepted, violations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::{Actor, Envelope, RecordKind};
    use crate::ids::RoleId;
    use chrono::Utc;

    fn well_formed_evidence_json() -> serde_json::Value {
        let record = EvidenceRecord::new(
            Envelope::new(
                1,
                RecordKind::EvidenceRecord,
                Utc::now(),
                Actor::new("codex-cli", RoleId::parse("implementer").unwrap()),
            ),
            None,
            None,
            None,
            crate::records::EvidenceVerdict::Faithful,
        );
        serde_json::to_value(&record).unwrap()
    }

    #[test]
    fn well_formed_record_validates_with_no_violation() {
        let candidate = RawRecord(well_formed_evidence_json());
        assert!(validate_evidence(&candidate).is_ok());
    }

    #[test]
    fn record_missing_actor_is_skipped_and_reported() {
        let mut json = well_formed_evidence_json();
        json.as_object_mut().unwrap().remove("actor");
        let candidate = RawRecord(json);
        let violation = validate_evidence(&candidate).unwrap_err();
        assert_eq!(violation.class, FailureClass::Malformed);
        assert!(violation.subject.contains("actor"));
    }

    #[test]
    fn invalid_scenario_id_grammar_is_reported_as_invalid_join_key() {
        let mut json = well_formed_evidence_json();
        json.as_object_mut().unwrap().insert("scenario_id".into(), serde_json::json!("Not Valid!!"));
        let candidate = RawRecord(json);
        let violation = validate_evidence(&candidate).unwrap_err();
        assert_eq!(violation.class, FailureClass::InvalidJoinKey);
    }

    #[test]
    fn batch_of_five_with_one_malformed_processes_the_other_four() {
        let mut candidates = Vec::new();
        for _ in 0..4 {
            candidates.push(RawRecord(well_formed_evidence_json()));
        }
        let mut malformed = well_formed_evidence_json();
        malformed.as_object_mut().unwrap().remove("actor");
        candidates.push(RawRecord(malformed));

        let (accepted, violations) = validate_evidence_batch(&candidates);
        assert_eq!(accepted.len(), 4);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].class, FailureClass::Malformed);
    }

    #[test]
    fn failure_class_as_str_matches_serde_kebab_case() {
        for class in [
            FailureClass::Malformed,
            FailureClass::InvalidJoinKey,
            FailureClass::InvalidStateTransition,
            FailureClass::UnregisteredHandoffDomain,
            FailureClass::InvalidHandoffBody,
        ] {
            let json = serde_json::to_string(&class).unwrap();
            assert_eq!(json, format!("\"{}\"", class.as_str()));
            assert_eq!(FailureClass::from_str_exact(class.as_str()), Some(class));
        }
    }
}
