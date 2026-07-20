//! `spec/inventory/` (S11 task 1.1/3.3-3.5, design D3): per-surface
//! reference-keyed behavior inventories, plus the single generated
//! `assets.lock` lockfile as its own kind. `InventoryEntry`'s own shape
//! (`{upstream: {pin, file, symbol, lines}, covered_by | out_of_scope}`) is
//! ALREADY well-structured in the donor's real inventory corpus (read
//! directly from that corpus) — the
//! audit's inventory gap is FILE-level (no schema envelope, no
//! at/actor, partition-key-smeared filenames), not entry-level, so this
//! module preserves the entry shape verbatim and only adds the file's
//! envelope + Hive lift.
//!
//! `area`/`surface` are deliberately NOT stored as file content —
//! design D3 lifts the file INTO `area=<area>/[surface=<surface>/]`
//! path segments; `canon-fmt` derives both from the file's OWN entry
//! keys (every entry across a real inventory file shares one
//! `<area>.<surface>` dot-prefix — verified against the entire live
//! corpus, S11 design §Context), never from the pre-migration filename
//! (`world-map.yaml` vs `world-place-map.yaml`'s drifted, ambiguous
//! hyphenation the audit calls out).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::family::FamilyEnvelope;
use crate::ids::ScenarioId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum InventoryKind {
    Inventory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum InventoryLockKind {
    InventoryLock,
}

/// The donor's existing 4-field reference (`pin`/`file`/`symbol`/
/// `lines`) — kept as its own type (not [`crate::family::refs::Ref`]):
/// it carries `pin` (the shared type does not) and `lines` stays the
/// donor's own `"262-270"` hyphen-string, never reparsed into
/// [`crate::family::refs::LineRange`] since nothing about it is
/// ambiguous or in need of upgrading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InventoryEntryRef {
    pub pin: String,
    pub file: String,
    pub symbol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<String>,
}

/// One inventory entry — `covered_by` XOR `out_of_scope` (donor
/// convention; `out_of_scope`'s own shape varies by entry — a bare
/// `true` or a reason string — kept open rather than guessed).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InventoryEntry {
    pub upstream: InventoryEntryRef,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub covered_by: Vec<ScenarioId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out_of_scope: Option<serde_json::Value>,
}

/// One `spec/inventory/kind=inventory/area=<area>/[surface=<surface>/]<key>.yaml`
/// file: the schema envelope (additive top-level YAML keys, design D3)
/// flattened alongside the file's own entries (also flattened, keyed by
/// their dotted entry id — `idolive.hub.hub-header`, …).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InventoryFile {
    #[serde(flatten)]
    pub envelope: FamilyEnvelope<InventoryKind>,
    #[serde(flatten)]
    pub entries: BTreeMap<String, InventoryEntry>,
}

/// One `assets.lock` line (`<asset-id>\t<source>\t<upstream-ref>`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LockAsset {
    pub id: String,
    pub source: String,
    pub upstream_ref: String,
}

/// One `[port_only]` section line (`<id-or-path>\t<reason>`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PortOnlyEntry {
    pub id: String,
    pub reason: String,
}

/// `spec/inventory/kind=inventory-lock/assets.lock.yaml` (design D3):
/// the donor's hand-rolled TSV-with-comments `assets.lock` format,
/// converted to enveloped YAML. Generated-only (D16 pattern) — canon
/// never expects a human-authored instance of this type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InventoryLock {
    #[serde(flatten)]
    pub envelope: FamilyEnvelope<InventoryLockKind>,
    pub assets: Vec<LockAsset>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub port_only: Vec<PortOnlyEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::Actor;
    use chrono::Utc;

    #[test]
    fn inventory_file_flattens_envelope_and_entries_together() {
        let file = InventoryFile {
            envelope: FamilyEnvelope::new(1, InventoryKind::Inventory, Utc::now(), Actor::new_unattributed("canon-fmt")),
            entries: BTreeMap::from([(
                "idolive.hub.hub-header".to_string(),
                InventoryEntry {
                    upstream: InventoryEntryRef {
                        pin: "9c93d024b".to_string(),
                        file: "routes/idolive/replays/index.tsx".to_string(),
                        symbol: "RouteComponent".to_string(),
                        lines: Some("262-270".to_string()),
                    },
                    covered_by: vec![ScenarioId::parse("idolive.hub.01").unwrap()],
                    out_of_scope: None,
                },
            )]),
        };
        let json = serde_json::to_value(&file).unwrap();
        assert_eq!(json.get("kind").and_then(|v| v.as_str()), Some("inventory"));
        assert!(json.get("idolive.hub.hub-header").is_some(), "entry keys must sit alongside envelope keys");
        let back: InventoryFile = serde_json::from_value(json).unwrap();
        assert_eq!(back, file);
    }

    #[test]
    fn inventory_lock_round_trips() {
        let lock = InventoryLock {
            envelope: FamilyEnvelope::new(1, InventoryLockKind::InventoryLock, Utc::now(), Actor::new_unattributed("gen_assets_lock")),
            assets: vec![LockAsset {
                id: "BG.space.1f_beauty_zone".to_string(),
                source: "lab_assets".to_string(),
                upstream_ref: "upstream/backgrounds/space-1f".to_string(),
            }],
            port_only: vec![],
        };
        let json = serde_json::to_value(&lock).unwrap();
        assert_eq!(json.get("kind").and_then(|v| v.as_str()), Some("inventory-lock"));
        let back: InventoryLock = serde_json::from_value(json).unwrap();
        assert_eq!(back, lock);
    }
}
