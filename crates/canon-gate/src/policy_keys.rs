//! `policy-keys`: an unknown top-level `policy.yaml` key fails the gate.
//!
//! `PolicyResolution::resolve` records each key outside
//! [`KNOWN_TOP_LEVEL_KEYS`] as a [`PolicyDiagnostic::UnknownKey`] and
//! keeps resolving every known key. This check is that diagnostic's
//! owning check, the same pattern `spec_coverage`/`evidence_binding`/
//! `risk_tiers` use for a present-but-invalid section: an
//! `uncovered-cell` violation naming the key, so a typo like
//! `spec_coverag:` never reads as "not opted in".

use crate::context::{GateCheck, GateContext};
use crate::failure_class::{FailureClass, Violation};
use crate::policy::{PolicyDiagnostic, KNOWN_TOP_LEVEL_KEYS};

/// One `uncovered-cell` violation per unknown top-level key, in file order.
pub struct PolicyKeysCheck;

impl GateCheck for PolicyKeysCheck {
    fn name(&self) -> &'static str {
        "policy-keys"
    }

    fn run(&self, ctx: &GateContext) -> Vec<Violation> {
        ctx.policy
            .diagnostics
            .iter()
            .filter_map(|diagnostic| match diagnostic {
                PolicyDiagnostic::UnknownKey { key } => Some(Violation::new(
                    FailureClass::UncoveredCell,
                    key.clone(),
                    format!(
                        "`{key}` is not a known top-level policy.yaml key (known keys: {}); fix it or remove it — an unknown key is never silently ignored",
                        KNOWN_TOP_LEVEL_KEYS.join(", ")
                    ),
                )),
                _ => None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use canon_policy::SchemaRegistry;
    use tempfile::TempDir;

    use super::*;
    use crate::context::GateCtx;

    fn run_with(policy: &str) -> Vec<Violation> {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".canon")).unwrap();
        std::fs::write(dir.path().join(".canon/policy.yaml"), policy).unwrap();
        let now = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").unwrap().with_timezone(&chrono::Utc);
        let ctx = GateContext::load(GateCtx::from_fixture(dir.path()), &SchemaRegistry::load(), now).unwrap();
        PolicyKeysCheck.run(&ctx)
    }

    #[test]
    fn an_unknown_top_level_key_is_an_uncovered_cell_naming_it_and_the_known_keys() {
        let violations = run_with("trust_required:\n  test-run: agent\nspec_coverag:\n  require_evidence: true\n");
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert_eq!(violations[0].class, FailureClass::UncoveredCell);
        assert_eq!(violations[0].subject, "spec_coverag");
        assert!(violations[0].detail.contains("`spec_coverag` is not a known top-level policy.yaml key"), "{}", violations[0].detail);
        assert!(violations[0].detail.contains(&KNOWN_TOP_LEVEL_KEYS.join(", ")), "{}", violations[0].detail);
    }

    #[test]
    fn every_known_key_passes() {
        let policy: String = KNOWN_TOP_LEVEL_KEYS.iter().map(|key| format!("{key}: {{}}\n")).collect();
        // `schema` is a number, not a mapping.
        let policy = policy.replace("schema: {}", "schema: 1");
        assert_eq!(run_with(&policy), Vec::new());
    }
}
