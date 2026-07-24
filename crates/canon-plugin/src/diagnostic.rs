//! canon-plugin's own small validation-finding type -- NOT
//! `canon_vocab::checker::Diagnostic` (this crate takes no canon-vocab
//! dependency; design.md D2/R4: the two manifest surfaces stay genuinely
//! separate, right down to their diagnostic types). Mirrors that type's
//! SHAPE (`{code, severity, message, subject}`) by inspiration only.
//!
//! Every diagnostic this crate emits carries one of the stable `E-PLUGIN-*`
//! codes below -- from the moment this change lands these are canon-plugin's
//! OWN stable failure-class strings, never renamed without migrating both
//! fixtures and any hook that greps them (mirrors `canon_vocab::checker`'s
//! own `E-*` code stability discipline, `crates/canon-vocab/src/
//! checker.rs:21-23`). [`DIAGNOSTIC_CODES`] is asserted complete by a unit
//! test so a new code added to [`crate::manifest::loader::LoadError`] or
//! [`crate::manifest::resolve`]'s resolution checks can never silently
//! diverge from this list.

/// A `plugin.yaml` manifest read from disk (`E-PLUGIN-MANIFEST`): the file
/// is absent, unreadable, or fails to parse as the `PluginManifest` schema
/// -- including a missing required field, which serde rejects outright
/// rather than defaulting.
pub const E_PLUGIN_MANIFEST: &str = "E-PLUGIN-MANIFEST";
/// Two packages under `.canon/plugins/` declare the same manifest `id`
/// (`E-PLUGIN-DUP-ID`) -- the later (directory-sort order) package is
/// dropped, never silently merged or overwritten.
pub const E_PLUGIN_DUP_ID: &str = "E-PLUGIN-DUP-ID";
/// A `namespace` or overlay `kind` fails the kebab-token grammar
/// `[a-z0-9]+(-[a-z0-9]+)*` (`E-PLUGIN-GRAMMAR`).
pub const E_PLUGIN_GRAMMAR: &str = "E-PLUGIN-GRAMMAR";
/// An overlay's `attaches_to.core_kind` names anything other than
/// `scenario` (`E-PLUGIN-CORE-KIND`) -- s16 supports `core_kind: scenario`
/// only; a generic projection over other core kinds is explicit FUTURE
/// work (`plugin-overlay-projection` spec).
pub const E_PLUGIN_CORE_KIND: &str = "E-PLUGIN-CORE-KIND";
/// An overlay identity (`<namespace>.<kind>`) equals a core
/// `RecordKind::as_str()` value (`E-PLUGIN-CORE-COLLISION`) -- resolution-
/// time half of design.md R5's defense in depth (the write-time half is
/// P2's `write_namespaced`, out of this change's scope).
pub const E_PLUGIN_CORE_COLLISION: &str = "E-PLUGIN-CORE-COLLISION";
/// Two overlays -- within one manifest, or across two different plugins --
/// declare the same `<namespace>.<kind>` identity (`E-PLUGIN-DUP-OVERLAY`):
/// an ambiguous write target neither declaration should silently win.
pub const E_PLUGIN_DUP_OVERLAY: &str = "E-PLUGIN-DUP-OVERLAY";
/// A required non-empty manifest list is empty (`E-PLUGIN-EMPTY`): a
/// manifest with `overlays: []`, or an overlay with an empty
/// `attaches_to.join_key`. The registry spec requires one-or-more
/// overlays and one-or-more join-key field(s); an empty join_key would
/// also leave P2 nothing to derive an overlay's `natural_key` from.
pub const E_PLUGIN_EMPTY: &str = "E-PLUGIN-EMPTY";
/// P2's `validate_overlay_body` (`E-PLUGIN-BODY-MISSING`): a candidate
/// overlay body omits a required field -- one of the `OverlayEnvelope`
/// fields (`schema`/`kind`/`at`/`actor`), a REQUIRED join-key field
/// named by `OverlayDecl.join_key`, or a manifest-declared field.
pub const E_PLUGIN_BODY_MISSING: &str = "E-PLUGIN-BODY-MISSING";
/// P2's `validate_overlay_body` (`E-PLUGIN-BODY-UNDECLARED`): a
/// candidate overlay body carries a field outside the closed union of
/// (a) `OverlayEnvelope` fields, (b) `OverlayDecl.join_key` fields, and
/// (c) `OverlayDecl.fields` -- an overlay kind's field set is closed,
/// exactly one level down from `RecordKind`'s own twelve-kind closure.
pub const E_PLUGIN_BODY_UNDECLARED: &str = "E-PLUGIN-BODY-UNDECLARED";
/// P2's `validate_overlay_body` (`E-PLUGIN-BODY-TYPE`): a present field
/// fails its expected structural shape -- an `at` that doesn't parse as
/// RFC3339, a join-key field that isn't a JSON string, or a
/// manifest-declared field whose value `type_accepts` rejects.
pub const E_PLUGIN_BODY_TYPE: &str = "E-PLUGIN-BODY-TYPE";
/// P2's `validate_overlay_body` (`E-PLUGIN-BODY-KIND`): a candidate
/// overlay body's own `OverlayEnvelope.kind` field disagrees with
/// `OverlayDecl.identity` (`<namespace>.<kind>`) -- the manifest-schema
/// half of the SAME directory/content-kind invariant
/// `canon_store::partition::validate_kind_matches_content` enforces for
/// the twelve core kinds; `GitTier::write_namespaced`/
/// `scan_namespaced_kind` enforce the store-layer half at write/scan
/// time (s16 design.md R5 defense in depth) -- an overlay record can
/// never live under `kind=<namespace>.<kind>/` while its own body
/// claims a different kind, at either layer.
pub const E_PLUGIN_BODY_KIND: &str = "E-PLUGIN-BODY-KIND";

/// Every diagnostic code this crate emits, for a stability test (mirrors
/// `canon_vocab::checker::DIAGNOSTIC_CODES`).
pub const DIAGNOSTIC_CODES: [&str; 11] = [
    E_PLUGIN_MANIFEST,
    E_PLUGIN_DUP_ID,
    E_PLUGIN_GRAMMAR,
    E_PLUGIN_CORE_KIND,
    E_PLUGIN_CORE_COLLISION,
    E_PLUGIN_DUP_OVERLAY,
    E_PLUGIN_EMPTY,
    E_PLUGIN_BODY_MISSING,
    E_PLUGIN_BODY_UNDECLARED,
    E_PLUGIN_BODY_TYPE,
    E_PLUGIN_BODY_KIND,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// One resolution finding. `subject` identifies WHAT the diagnostic is
/// about -- a `plugin.yaml` path, a plugin id, or a `<namespace>.<kind>`
/// overlay identity -- canon-plugin's anchor in place of a byte span (no
/// `plugin.yaml` field carries per-value source position information once
/// parsed by `serde_yaml`, same reasoning as `canon_vocab::checker::
/// Diagnostic`'s own doc comment).
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub subject: String,
}

impl Diagnostic {
    pub fn error(code: &str, message: impl Into<String>, subject: impl Into<String>) -> Self {
        Diagnostic { code: code.to_string(), severity: Severity::Error, message: message.into(), subject: subject.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_codes_list_has_no_duplicates() {
        let mut sorted = DIAGNOSTIC_CODES.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), DIAGNOSTIC_CODES.len());
    }

    #[test]
    fn error_helper_sets_error_severity() {
        let d = Diagnostic::error(E_PLUGIN_MANIFEST, "boom", "some/subject");
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.code, E_PLUGIN_MANIFEST);
        assert_eq!(d.message, "boom");
        assert_eq!(d.subject, "some/subject");
    }
}
