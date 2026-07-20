//! Versioned JSON-schema export (S1 spec `canon-model-schema`, task
//! group 3; extended by S11's artifact-family schema registry, task
//! 1.1): one `.schema.json` per record kind, generated from the same
//! Rust type definitions used for serialization — no separate/manual
//! schema-authoring file to fall out of sync.

use schemars::schema_for;

use crate::envelope::RecordKind;
use crate::family::divergence::DivergenceEvent;
use crate::family::feature::FeatureProvenance;
use crate::family::inventory::{InventoryFile, InventoryLock};
use crate::family::ledger::{LedgerKind, LedgerReviewRecord, LedgerRunRecord};
use crate::family::policy::PolicyFile;
use crate::family::FamilyKind;
use crate::handoff::Handoff;
use crate::records::{Change, Divergence, Event, EvidenceRecord, Review, Run, Scenario, Session, StrategyItem, Subject, Task, Trajectory};

/// One `(kind, schema)` pair per one of the thirteen closed record kinds
/// (`RecordKind::ALL`'s own order). Every schema is produced by
/// `schemars::schema_for!` directly against that kind's Rust type — a
/// field added to the type changes only that kind's output (spec
/// scenario "A field addition is reflected without a second
/// registration site").
pub fn record_schemas() -> Vec<(RecordKind, schemars::Schema)> {
    vec![
        (RecordKind::Change, schema_for!(Change)),
        (RecordKind::Task, schema_for!(Task)),
        (RecordKind::Scenario, schema_for!(Scenario)),
        (RecordKind::Session, schema_for!(Session)),
        (RecordKind::Run, schema_for!(Run)),
        (RecordKind::Event, schema_for!(Event)),
        (RecordKind::Handoff, schema_for!(Handoff)),
        (RecordKind::Review, schema_for!(Review)),
        (RecordKind::Divergence, schema_for!(Divergence)),
        (RecordKind::Trajectory, schema_for!(Trajectory)),
        (RecordKind::StrategyItem, schema_for!(StrategyItem)),
        (RecordKind::EvidenceRecord, schema_for!(EvidenceRecord)),
        (RecordKind::Subject, schema_for!(Subject)),
    ]
}

/// One `(kind, schema)` pair per one of the eleven [`FamilyKind`]
/// entries (S11 task 1.1) — the artifact-family registry ALONGSIDE
/// `record_schemas()`'s closed twelve, never merged into it (see
/// `crate::family`'s module doc for why). Ledger's `run`/`drill` share
/// [`LedgerRunRecord`]'s Rust type; `review`/`clear`/`code-review`/
/// `design-review` share [`LedgerReviewRecord`]'s; `divergence` exports
/// the `type`-tagged [`DivergenceEvent`] enum (one schema covers all
/// three event shapes, matching how they actually coexist one-JSONL-
/// line-at-a-time); `feature` exports [`FeatureProvenance`] (the
/// comment payload, not a whole-file schema — `.feature` files are not
/// JSON).
pub fn family_schemas() -> Vec<(FamilyKind, schemars::Schema)> {
    vec![
        (FamilyKind::Ledger(LedgerKind::Run), schema_for!(LedgerRunRecord)),
        (FamilyKind::Ledger(LedgerKind::Drill), schema_for!(LedgerRunRecord)),
        (FamilyKind::Ledger(LedgerKind::Review), schema_for!(LedgerReviewRecord)),
        (FamilyKind::Ledger(LedgerKind::Clear), schema_for!(LedgerReviewRecord)),
        (FamilyKind::Ledger(LedgerKind::CodeReview), schema_for!(LedgerReviewRecord)),
        (FamilyKind::Ledger(LedgerKind::DesignReview), schema_for!(LedgerReviewRecord)),
        (FamilyKind::Divergence, schema_for!(DivergenceEvent)),
        (FamilyKind::Feature, schema_for!(FeatureProvenance)),
        (FamilyKind::Inventory, schema_for!(InventoryFile)),
        (FamilyKind::InventoryLock, schema_for!(InventoryLock)),
        (FamilyKind::Policy, schema_for!(PolicyFile)),
    ]
}

/// `record_schemas()`, pretty-printed with a trailing newline (stable,
/// diff-friendly), keyed by the output filename
/// (`schemas/<kind>.schema.json`) the generator writes/checks.
pub fn pretty_schemas() -> Vec<(String, String)> {
    record_schemas()
        .into_iter()
        .map(|(kind, schema)| {
            let filename = format!("{}.schema.json", kind.as_str());
            let mut text = serde_json::to_string_pretty(&schema).expect("Schema serializes to JSON");
            text.push('\n');
            (filename, text)
        })
        .collect()
}

/// `family_schemas()`, pretty-printed the same way, keyed by
/// `schemas/family-<kind>.schema.json` — the `family-` prefix keeps the
/// two registries' filenames from colliding (`family-run` next to
/// `run`, since `RecordKind::Run` and `FamilyKind::Ledger(LedgerKind::Run)`
/// both happen to name "run") in the SAME `schemas/` directory, so
/// `gen::check`'s single-directory drift scan covers both without a
/// second directory to keep in sync.
pub fn pretty_family_schemas() -> Vec<(String, String)> {
    let mut seen = std::collections::HashSet::new();
    family_schemas()
        .into_iter()
        .filter(|(kind, _)| seen.insert(kind.as_str()))
        .map(|(kind, schema)| {
            let filename = format!("family-{}.schema.json", kind.as_str());
            let mut text = serde_json::to_string_pretty(&schema).expect("Schema serializes to JSON");
            text.push('\n');
            (filename, text)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_exactly_one_schema_per_record_kind() {
        let schemas = record_schemas();
        assert_eq!(schemas.len(), 13);
        let mut seen = std::collections::HashSet::new();
        for (kind, _) in &schemas {
            assert!(seen.insert(*kind), "{kind:?} emitted twice");
        }
        for kind in RecordKind::ALL {
            assert!(seen.contains(&kind), "{kind:?} missing a schema");
        }
    }

    #[test]
    fn every_schema_declares_the_envelope_fields() {
        for (kind, schema) in record_schemas() {
            let value = serde_json::to_value(&schema).unwrap();
            let properties = value.pointer("/properties").unwrap_or_else(|| panic!("{kind:?} schema has no properties"));
            for field in ["schema", "kind", "at", "actor"] {
                assert!(properties.get(field).is_some(), "{kind:?} schema missing envelope field `{field}`");
            }
        }
    }
}
