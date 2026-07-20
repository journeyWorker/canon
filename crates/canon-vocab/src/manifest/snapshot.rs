//! Retargeted from the donor manifest layer's `CapabilitySnapshot`
//! (design.md D3's resolved-vocabulary output type), pruned to the fields
//! canon's directive/enum/evidence-kind domain needs — no `providers`/
//! `state_shapes`/`state_templates`/`asset_kinds`/`bridge_capabilities`/
//! `defs`/`frontmatter`/`events` (D2 Non-Goals, no scene-DSL analog).
//!
//! [`CapabilitySnapshot::evidence_kinds`] has no donor analog at all: D4's
//! `Type::Evidence` domain, resolved from S5's policy
//! ([`crate::policy_bridge`]), folded in by [`crate::resolve_snapshot`]
//! AFTER plugin assembly (unlike `enums`/`domains`, it is not a
//! manifest-declared vocabulary).
//!
//! # `capability_version` is a REAL content hash (the content-hash audit pattern)
//!
//! [`CapabilitySnapshot::capability_version`] is a SHA-256 hex digest folded
//! over `plugins`/`enums`/`directives`/`evidence_kinds`
//! ([`CapabilitySnapshot::compute_capability_version`]), set by
//! [`crate::resolve_snapshot::resolve_snapshot`] as the LAST step of every
//! resolution (after `evidence_kinds` itself is folded in — see that
//! function). Two resolutions of byte-identical manifests always fold to
//! the same digest; changing ANY directive attr, enum member, plugin
//! version, or the live evidence-kind domain changes it — genuine drift
//! detection, unlike `canon-cli::context::CURRENT_SCHEMA_VERSION` (a
//! separate, hand-maintained integer `canon-cli`'s own `AuthoringSurface`
//! uses for `canon-model` record-envelope schema bumps, an unrelated
//! concept this crate does not touch). A snapshot built directly via
//! [`CapabilitySnapshot::default`] (every in-crate unit test that hand-rolls
//! a snapshot rather than calling `resolve_snapshot`) carries the empty
//! string here — never a hash of nothing masquerading as "resolved".
//!
//! [`Serialize`] (only; no [`serde::Deserialize`] — a snapshot is a
//! RESOLUTION OUTPUT, never authored input) is derived here for exactly
//! one reason: S10 part2 wires `canon context`'s `AuthoringSurface`
//! (`crates/canon-cli/src/context.rs`) to embed the SAME
//! [`crate::resolve_snapshot::resolve_snapshot`] value the checker
//! resolves — design.md D3's "no consumer computes its own partial
//! vocabulary view" — literally, byte-for-byte, rather than canon-cli
//! hand-mapping a second, potentially-drifting projection of this
//! type's fields. This is the one narrow public-API surface S10 part2's
//! own territory rule permits touching in this crate.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::manifest::schema::DirectiveDecl;
use crate::manifest::types::Literal;

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilitySnapshot {
    /// SHA-256 hex content hash over the vocabulary this snapshot resolved
    /// (module doc) — set by [`crate::resolve_snapshot::resolve_snapshot`],
    /// empty on a hand-built [`CapabilitySnapshot::default`].
    pub capability_version: String,
    pub plugins: BTreeMap<String, ResolvedPlugin>,
    /// `enums.yaml`'s shared vocabulary, folded from every active plugin,
    /// resolved by `Type::Domain(name)`.
    pub enums: BTreeMap<String, Vec<String>>,
    pub directives: BTreeMap<String, DirectiveDecl>,
    /// `Type::Evidence`'s policy-resolved kind domain (D4) — see module doc.
    pub evidence_kinds: Vec<String>,
    /// Installed-but-inactive directive tag -> owning plugin id (plugin
    /// §11.2 fix-it parity; `crate::checker` surfaces this the same way
    /// `check_directive` does, minus the LSP fix-it object itself since no
    /// LSP consumes it yet).
    pub inactive: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPlugin {
    pub version: String,
    pub options: BTreeMap<String, Literal>,
}

impl CapabilitySnapshot {
    pub fn directive(&self, name: &str) -> Option<&DirectiveDecl> {
        self.directives.get(name)
    }

    /// Fold `plugins`/`enums`/`directives`/`evidence_kinds` — the resolved
    /// vocabulary proper (module doc) — into a deterministic SHA-256 hex
    /// digest. Deliberately excludes `capability_version` itself (no
    /// self-reference) and `inactive` (a diagnostic index of what did NOT
    /// activate, not vocabulary content — installing-then-deactivating a
    /// plugin must not perturb the version a consumer's typed atom checks
    /// against). `plugins`/`enums`/`directives` are already `BTreeMap`
    /// (key-sorted `Debug` iteration); `evidence_kinds` is explicitly
    /// sorted here so a differently-ordered-but-identical policy read still
    /// folds to the same digest (mirrors `canon-policy::bindings::
    /// fingerprint`'s identical "deterministic `Debug` of a sorted
    /// structure" technique), then hashed with the workspace `sha2`
    /// crate — the same SHA-256 primitive `canon-ingest::content_digest`
    /// uses, so canon has ONE hashing implementation, not a hand-rolled
    /// second one.
    pub fn compute_capability_version(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut evidence_kinds = self.evidence_kinds.clone();
        evidence_kinds.sort();
        let content = format!("{:?}|{:?}|{:?}|{:?}", self.plugins, self.enums, self.directives, evidence_kinds);
        Sha256::digest(content.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::schema::DirectiveDecl;
    use crate::manifest::types::{AttrDecl, Type};

    fn snapshot_with_task_directive() -> CapabilitySnapshot {
        let mut snap = CapabilitySnapshot::default();
        snap.directives.insert(
            "task".to_string(),
            DirectiveDecl { name: "task".into(), attrs: vec![AttrDecl { name: "desc".into(), required: true, ty: Type::Str, default: None }] },
        );
        snap.enums.insert("task-status".to_string(), vec!["open".to_string(), "done".to_string()]);
        snap.evidence_kinds = vec!["test-run".to_string(), "manual-review".to_string()];
        snap
    }

    #[test]
    fn identical_content_hashes_equal_regardless_of_evidence_kind_order() {
        let a = snapshot_with_task_directive();
        let mut b = snapshot_with_task_directive();
        b.evidence_kinds.reverse();
        assert_eq!(a.compute_capability_version(), b.compute_capability_version());
    }

    #[test]
    fn editing_a_directive_attr_flips_the_hash() {
        let before = snapshot_with_task_directive();
        let mut after = snapshot_with_task_directive();
        after.directives.get_mut("task").unwrap().attrs[0].required = false;
        assert_ne!(before.compute_capability_version(), after.compute_capability_version());
    }

    #[test]
    fn editing_an_enum_member_flips_the_hash() {
        let before = snapshot_with_task_directive();
        let mut after = snapshot_with_task_directive();
        after.enums.get_mut("task-status").unwrap().push("blocked".to_string());
        assert_ne!(before.compute_capability_version(), after.compute_capability_version());
    }

    #[test]
    fn editing_the_evidence_kind_domain_flips_the_hash() {
        let before = snapshot_with_task_directive();
        let mut after = snapshot_with_task_directive();
        after.evidence_kinds.push("new-kind".to_string());
        assert_ne!(before.compute_capability_version(), after.compute_capability_version());
    }

    #[test]
    fn deactivating_a_plugin_into_inactive_does_not_perturb_the_hash() {
        let before = snapshot_with_task_directive();
        let mut after = snapshot_with_task_directive();
        after.inactive.insert("some-directive".to_string(), "some.plugin".to_string());
        assert_eq!(before.compute_capability_version(), after.compute_capability_version());
    }

    #[test]
    fn a_default_snapshot_carries_no_capability_version() {
        assert_eq!(CapabilitySnapshot::default().capability_version, "");
    }
}
