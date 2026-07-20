//! The `GateCheck` dispatcher (task 1.9's own naming, S5 wave-2-part2):
//! the ONE place that assembles which [`crate::GateCheck`]s run together
//! over one [`crate::GateContext`] — reused by `canon-cli`'s
//! `canon gate check` AND this crate's own [`crate::selftest`] module, so
//! "the dispatcher ALWAYS includes [`crate::TrustLadderCheck`] alongside
//! any release-profile [`crate::ReleaseTrustCheck`]" is a structural
//! property of one function, never independently re-assembled (and
//! potentially drifted) per caller.
//!
//! # Why this matters: malformed evidence is caught ONCE, upstream
//! `crate::context::GateContext::load` validates every ledger record's
//! native fields at load time (s15 P3b/D9) — a present-malformed
//! `lifecycle`/`flagged`/`evidence_sha`/`surface_ref`/`run_seq` fails
//! the WHOLE record's deserialize there, landing it in `ctx.violations`
//! rather than `ctx.evidence`. [`crate::LedgerCheck`] (always in
//! [`check_set`]) is what surfaces every one of those as
//! `malformed-evidence` — never a per-check bespoke re-validation.
//! [`check_set`] still closes a SEPARATE concern at the dispatch level
//! (never trusting each call site to remember the pairing): a caller
//! that engaged [`crate::ReleaseTrustCheck`] WITHOUT
//! [`crate::TrustLadderCheck`] could see a `class`-tagged-but-
//! `reviewed`-without-a-review-record artifact pass release evaluation
//! simply because nothing in that reduced check set was watching for
//! `unreviewed-promotion` at all — [`check_set`] makes "both or just
//! the ordinary four" the only two shapes this function can ever
//! produce.
//!
//! # Ordinary vs release-scoped (design decision 2/D7, spec.md "Severity
//! below the required trust level at release ... does not block ordinary
//! (non-release) evaluation")
//! [`check_set`]'s `release` parameter is the only switch: `false` (every
//! ordinary, non-release `canon gate check` invocation) runs coverage +
//! ledger + staleness + the always-on trust ladder; `true` (a release
//! profile — `canon gate check --release`, and this crate's own
//! [`crate::selftest`] corpus, which must prove `trust-below-required`
//! itself fires) additionally engages [`crate::ReleaseTrustCheck`], with
//! [`crate::TrustLadderCheck`] present either way.

use crate::context::GateCheck;
use crate::coverage::CoverageCheck;
use crate::ledger::LedgerCheck;
use crate::staleness::StalenessCheck;
use crate::trust::{ReleaseTrustCheck, TrustLadderCheck};

/// The assembled `GateCheck` set (module doc). `release = false` is
/// ordinary evaluation's set; `release = true` additionally engages
/// [`ReleaseTrustCheck`] — [`TrustLadderCheck`] is present in BOTH,
/// never dropped when a release profile is engaged.
pub fn check_set(release: bool) -> Vec<Box<dyn GateCheck>> {
    let mut checks: Vec<Box<dyn GateCheck>> = vec![Box::new(CoverageCheck), Box::new(LedgerCheck), Box::new(StalenessCheck), Box::new(TrustLadderCheck)];
    if release {
        checks.push(Box::new(ReleaseTrustCheck));
    }
    checks
}

#[cfg(test)]
mod tests {
    use canon_policy::SchemaRegistry;
    use canon_store::git_tier::GitTier;
    use canon_store::tier::{RawWrite, Tier};
    use tempfile::TempDir;

    use super::*;
    use crate::context::{GateContext, GateCtx};
    use crate::failure_class::FailureClass;

    #[test]
    fn ordinary_check_set_always_includes_trust_ladder_check_but_never_release_trust_check() {
        let checks = check_set(false);
        assert!(checks.iter().any(|c| c.name() == "trust-ladder"));
        assert!(!checks.iter().any(|c| c.name() == "release-trust-required"));
        assert_eq!(checks.len(), 4);
    }

    #[test]
    fn release_check_set_includes_both_trust_ladder_check_and_release_trust_check() {
        let checks = check_set(true);
        assert!(checks.iter().any(|c| c.name() == "trust-ladder"), "a release profile must never drop the always-on trust-ladder check");
        assert!(checks.iter().any(|c| c.name() == "release-trust-required"));
        assert_eq!(checks.len(), 5);
    }

    #[test]
    fn a_present_malformed_native_field_surfaces_as_malformed_evidence_through_the_normal_dispatcher() {
        // s15 P3b acceptance (a): a present-malformed native field
        // (`flagged`'s inner `flagged: bool` here holding a string)
        // fails `EvidenceRecord`'s whole-record `Deserialize` at
        // `GateContext::load` time — surfaced as `malformed-evidence`
        // by the NORMAL `check_set` dispatcher (via `LedgerCheck`),
        // never a bespoke re-check any individual check implements.
        let dir = TempDir::new().unwrap();
        let gate_ctx = GateCtx::from_fixture(dir.path());
        let tier = GitTier::new(&gate_ctx.ledger_root);

        let body = serde_json::json!({
            "schema": 1,
            "kind": "evidence_record",
            "at": chrono::Utc::now().to_rfc3339(),
            "actor": {"agent_id": "test-agent", "role": "implementer"},
            "scenario_id": "world.firstbuy-hotdeal.90",
            "verdict": "faithful",
            "flagged": {"flagged": "not-a-bool"},
        });
        tier.write(&RawWrite(canon_model::RawRecord(body))).expect("write one malformed-native-field record");

        let registry = SchemaRegistry::load();
        let gate_context = GateContext::load(gate_ctx, &registry, chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").unwrap().with_timezone(&chrono::Utc)).unwrap();

        let checks = check_set(false);
        let violations: Vec<_> = checks.iter().flat_map(|c| c.run(&gate_context)).collect();
        assert!(violations.iter().any(|v| v.class == FailureClass::MalformedEvidence), "violations: {violations:?}");
    }
}
