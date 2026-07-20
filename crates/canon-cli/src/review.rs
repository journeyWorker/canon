//! `canon review add` (s15 P3b, native-verdict-lifecycle spec): the
//! ONLY producer of `canon_model::Review` records — without it, S1's
//! `Review`/`Divergence` kinds have no way to exist on the ledger at
//! all and `canon-gate`'s review index (`crate::trust::review_index`)
//! reads an eternally-empty set (every reviewed cell
//! `unreviewed-promotion` forever, design D10). Writes ONE `Review`,
//! attributed to the invoking actor via the envelope, directly
//! through the committed `GitTier` — no staging/promote step, unlike
//! `EvidenceRecord`/`Divergence`: a `Review` is a simple attestation,
//! never subject to `run_seq` assignment.
//!
//! # `provenance_ref` is ENFORCED, never synthesized
//! `canon_model::ProvenanceRef` is `UpstreamRef(String) |
//! OriginalSpecRef(String)` — exactly one of `--upstream-ref`/
//! `--original-spec-ref` is REQUIRED (spec.md "a review lacking a
//! resolvable provenance ref SHALL be refused, never written with an
//! empty or synthesized ref"). [`run_add`] refuses (exit `2`, nothing
//! written) on neither-given OR both-given — this validation happens
//! HERE, in the library function itself (not a clap `ArgGroup`), so
//! the refusal path is directly unit-testable without spawning the
//! binary.

use std::path::Path;

use canon_model::{Actor, Envelope, ProjectId, ProvenanceRef, RecordKind, Review, RoleId, ScenarioId};
use canon_gate::GateCtx;
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;
use chrono::Utc;

use crate::context::resolve_repo_root;

/// `--project-id`'s `clap` value parser.
pub fn parse_project_id(s: &str) -> Result<ProjectId, String> {
    ProjectId::parse(s).map_err(|e| e.to_string())
}

/// `--scenario-id`'s `clap` value parser.
pub fn parse_scenario_id(s: &str) -> Result<ScenarioId, String> {
    ScenarioId::parse(s).map_err(|e| e.to_string())
}

/// `canon review add` (module doc). `upstream_ref`/`original_spec_ref`
/// are the raw `--upstream-ref`/`--original-spec-ref` values — exactly
/// one MUST be `Some`; both `None` or both `Some` is refused before
/// any write is attempted. Returns the process exit code: `0` on a
/// successful write, `2` on a refused/malformed invocation.
#[allow(clippy::too_many_arguments)]
pub fn run_add(
    repo: &Path,
    project_id: &ProjectId,
    scenario_id: &ScenarioId,
    reviewer: &str,
    pin: &str,
    upstream_ref: Option<&str>,
    original_spec_ref: Option<&str>,
    actor_id: &str,
    role: &RoleId,
) -> i32 {
    let provenance_ref = match (upstream_ref, original_spec_ref) {
        (Some(r), None) => ProvenanceRef::UpstreamRef(r.to_string()),
        (None, Some(r)) => ProvenanceRef::OriginalSpecRef(r.to_string()),
        (None, None) => {
            eprintln!("canon review add: refused — neither --upstream-ref nor --original-spec-ref given; a review must carry a resolvable provenance ref");
            return 2;
        }
        (Some(_), Some(_)) => {
            eprintln!("canon review add: refused — --upstream-ref and --original-spec-ref are mutually exclusive; exactly one provenance ref is required");
            return 2;
        }
    };

    let repo = resolve_repo_root(repo);
    let gate_ctx = GateCtx::from_repo(&repo);
    let committed = GitTier::new(&gate_ctx.ledger_root);

    let review = Review::new(
        Envelope::new(1, RecordKind::Review, Utc::now(), Actor::new(actor_id, role.clone())),
        project_id.clone(),
        scenario_id.clone(),
        reviewer,
        pin,
        provenance_ref,
    );

    match committed.write(&review) {
        Ok(receipt) => {
            println!("canon review add: wrote {} ({})", receipt.location, if receipt.deduped { "deduped" } else { "new" });
            0
        }
        Err(e) => {
            eprintln!("canon review add: {e}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use canon_store::tier::TierQuery;
    use tempfile::TempDir;

    use super::*;

    fn args() -> (ProjectId, ScenarioId, RoleId) {
        (ProjectId::parse("app-a").unwrap(), ScenarioId::parse("world.firstbuy-hotdeal.26").unwrap(), RoleId::parse("reviewer").unwrap())
    }

    #[test]
    fn writes_an_attributed_review_with_a_valid_upstream_ref() {
        let dir = TempDir::new().unwrap();
        let (project_id, scenario_id, role) = args();

        let code = run_add(dir.path(), &project_id, &scenario_id, "reviewer-1", "abc123pin", Some("routes/world.firstbuy-hotdeal.26#onReview"), None, "agent-x", &role);
        assert_eq!(code, 0);

        let committed = GitTier::new(GateCtx::from_repo(dir.path()).ledger_root);
        let read = committed.read(&TierQuery::kind(RecordKind::Review)).unwrap();
        assert_eq!(read.records.len(), 1);
        let review: Review = serde_json::from_value(read.records[0].0.clone()).unwrap();
        assert_eq!(review.envelope.actor.agent_id, "agent-x");
        assert_eq!(review.envelope.actor.role.as_ref().map(|r| r.as_str()), Some("reviewer"));
        assert_eq!(review.provenance_ref, ProvenanceRef::UpstreamRef("routes/world.firstbuy-hotdeal.26#onReview".to_string()));
    }

    #[test]
    fn refuses_a_missing_provenance_ref_and_writes_nothing() {
        let dir = TempDir::new().unwrap();
        let (project_id, scenario_id, role) = args();

        let code = run_add(dir.path(), &project_id, &scenario_id, "reviewer-1", "abc123pin", None, None, "agent-x", &role);
        assert_eq!(code, 2);

        let committed = GitTier::new(GateCtx::from_repo(dir.path()).ledger_root);
        let read = committed.read(&TierQuery::kind(RecordKind::Review)).unwrap();
        assert!(read.records.is_empty(), "a refused review add must write nothing");
    }

    #[test]
    fn refuses_two_provenance_refs_at_once_and_writes_nothing() {
        let dir = TempDir::new().unwrap();
        let (project_id, scenario_id, role) = args();

        let code = run_add(dir.path(), &project_id, &scenario_id, "reviewer-1", "abc123pin", Some("routes/x#onReview"), Some("spec/x.md"), "agent-x", &role);
        assert_eq!(code, 2);

        let committed = GitTier::new(GateCtx::from_repo(dir.path()).ledger_root);
        let read = committed.read(&TierQuery::kind(RecordKind::Review)).unwrap();
        assert!(read.records.is_empty());
    }
}
