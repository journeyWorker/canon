//! `spec/policy.yaml` (S11 task 1.1): the required-cell derivation
//! policy. Path unchanged by S11 (design D3 only calls out inventory/
//! `assets.lock` moving; policy is envelope-only) — gains the schema
//! envelope as additive top-level YAML keys, same discipline as
//! [`crate::family::inventory`]. The policy's own rich content
//! (`platforms_active`, `risk_platforms`, `severity_rigor`, …) is
//! donor-specific derivation logic S11 does not need to
//! reinterpret to be the format authority over its envelope — kept as
//! an open flatten map, round-tripped losslessly.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::family::FamilyEnvelope;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyKind {
    Policy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PolicyFile {
    #[serde(flatten)]
    pub envelope: FamilyEnvelope<PolicyKind>,
    #[serde(flatten)]
    pub content: BTreeMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::Actor;
    use chrono::Utc;

    #[test]
    fn policy_file_keeps_arbitrary_content_alongside_envelope() {
        let mut content = BTreeMap::new();
        content.insert("platforms_active".to_string(), serde_json::json!(["macos", "web-chrome"]));
        let file = PolicyFile {
            envelope: FamilyEnvelope::new(1, PolicyKind::Policy, Utc::now(), Actor::new_unattributed("canon-fmt")),
            content,
        };
        let json = serde_json::to_value(&file).unwrap();
        assert_eq!(json.get("kind").and_then(|v| v.as_str()), Some("policy"));
        assert!(json.get("platforms_active").is_some());
        let back: PolicyFile = serde_json::from_value(json).unwrap();
        assert_eq!(back, file);
    }
}
