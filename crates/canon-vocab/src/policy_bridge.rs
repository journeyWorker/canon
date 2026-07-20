//! `Type::Evidence`'s policy-resolved kind domain (design.md D4): "an
//! `evidence` type, whose domain is resolved from S5's own parsed
//! `policy.yaml` (the same parse S5's gate uses, not a duplicate parser)".
//!
//! # Where "S5's own parsed `policy.yaml`" actually lives
//!
//! This change's own assignment names the dependency `canon-policy`, but
//! that crate (S13) is canon's shared CEL expression-language ENGINE
//! (`canon_policy::{compile, evaluate, SchemaRegistry}`) — it has no
//! `policy.yaml`-file parser of its own (verified: `crates/canon-policy/src/
//! lib.rs`'s own module doc lists S5 among CEL's "intended consumers", not
//! its policy loader). S5's ACTUAL `<repo>/canon/policy.yaml` parser is
//! [`canon_gate::PolicyResolution`] (`crates/canon-gate/src/policy.rs`),
//! which is BUILT ON `canon-policy`'s CEL engine (compiles each field's
//! `{cel: "…"}` predicate through it) but lives in the `canon-gate` crate.
//! Depending on `canon-gate` for this one read-only call is therefore the
//! literal, non-duplicating fulfillment of D4's instruction — canon-vocab
//! never edits `canon-gate`'s source (this change's own territory rule) and
//! never re-parses `policy.yaml` itself.
//!
//! # No dependency cycle with a future `canon gate task` typed path
//!
//! Task 4.4 (S10 part2, deferred by this change) has `canon gate task`
//! consume a typed atom's compiled `evidence.kind`/`ref`. That does NOT
//! require `canon-gate` to depend on `canon-vocab` — a mediating layer
//! (`canon-cli`, which can depend on both) can pass the compiled
//! `kind`/`ref` strings into a plain `canon-gate` function that takes them as
//! parameters, never importing a `canon-vocab` type. `canon-vocab ->
//! canon-gate` (this module) stays a one-directional edge.
//!
//! # What "the evidence-kind domain" resolves to today
//!
//! `policy.yaml` has no dedicated `evidence_kinds:` section (verified against
//! `canon_gate::policy::RawPolicy`'s closed field set: `schema`,
//! `trust_required`, `trust_sample`, `staleness`, `risk_routing` — none of
//! which is an evidence-kind list). [`PolicyResolution::trust_required`]'s
//! key set is the one already-existing "open, repo-declared vocabulary" S5's
//! own policy exposes (its doc comment: "whatever key vocabulary a repo's
//! `policy.yaml` declares"), so this module reuses THAT set as the
//! policy-recognized evidence-kind domain — literally S5's parse, zero new
//! parsing logic. If a future `policy.yaml` schema change adds a dedicated
//! evidence-kind section, only [`evidence_kind_domain`]'s body changes; no
//! caller of [`crate::resolve_snapshot`] does.

use std::path::Path;

/// The policy-recognized evidence-kind domain for `project_dir` — S5's
/// `trust_required` key set (module doc), resolved fresh every call (never
/// cached), so a repo's live policy is always the source of truth (design.md
/// Risks section: "`canon gate task <task_id>` re-resolves the snapshot ...
/// at gate time, not authoring time"). Never panics — `PolicyResolution::
/// resolve` is itself documented infallible.
pub fn evidence_kind_domain(project_dir: &Path) -> Vec<String> {
    let registry = canon_policy::SchemaRegistry::load();
    let policy = canon_gate::PolicyResolution::resolve(project_dir, &registry);
    policy.trust_required.keys().cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_policy_yaml_resolves_to_an_empty_domain_not_a_panic() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(evidence_kind_domain(tmp.path()).is_empty());
    }

    #[test]
    fn trust_required_keys_become_the_evidence_kind_domain() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("canon")).unwrap();
        std::fs::write(tmp.path().join("canon/policy.yaml"), "trust_required:\n  test-run: agent\n  manual-review: human\n").unwrap();
        let mut kinds = evidence_kind_domain(tmp.path());
        kinds.sort();
        assert_eq!(kinds, vec!["manual-review".to_string(), "test-run".to_string()]);
    }
}
