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
//!
//! # What the review saw is pinned, never typed (0.15 D1)
//! [`run_add`] reads the corpus the gate reads and records, on the
//! review, the scenario's current `source_digest` and the content digest
//! of its latest evidence record per role
//! ([`canon_gate::review_gate::current_pins`]). The gate counts the
//! review only while both still match. A scenario with no record is
//! refused (exit `2`, nothing written): there is nothing to pin. A
//! scenario with no evidence pins an empty list.
//!
//! # The reviewer's run can fail visibly (0.15 D4)
//! `--outcome` records `signed` (default), `indeterminate` or
//! `unavailable`; only `signed` is an approval. `--reviewer-provider`,
//! `--reviewer-model` and `--reviewer-harness` record the runtime that
//! produced the review, as the caller reports it.

use std::path::Path;

use canon_gate::review_gate::current_pins;
use canon_gate::spec_coverage::latest_scenarios;
use canon_gate::{GateContext, GateCtx};
use canon_model::{Actor, Envelope, PinnedSpec, ProjectId, ProvenanceRef, RecordKind, Review, ReviewOutcome, RoleId, ScenarioId, SessionId};
use canon_policy::SchemaRegistry;
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

/// `--session-id`'s `clap` value parser, shared by every command that
/// records an actor (`evidence add`, `review add`, `finding add|close`).
/// The grammar is [`SessionId`]'s: one non-empty opaque token, no
/// surrounding whitespace, no control characters.
pub fn parse_session_id(s: &str) -> Result<SessionId, String> {
    SessionId::parse(s).map_err(|e| e.to_string())
}

/// `--outcome`'s `clap` value parser.
pub fn parse_outcome(s: &str) -> Result<ReviewOutcome, String> {
    ReviewOutcome::parse(s)
}

/// `--reviewer-provider|model|harness`'s `clap` value parser: one
/// non-empty label, no surrounding whitespace, no control characters.
pub fn parse_runtime_label(s: &str) -> Result<String, String> {
    if s.is_empty() || s.trim() != s || s.chars().any(char::is_control) {
        return Err(format!("`{s}` is not a runtime label: it must be non-empty, without surrounding whitespace or control characters"));
    }
    Ok(s.to_string())
}

/// `canon review add`'s arguments. `upstream_ref`/`original_spec_ref`
/// are the raw `--upstream-ref`/`--original-spec-ref` values — exactly
/// one MUST be `Some`.
pub struct ReviewAddArgs {
    pub project_id: ProjectId,
    pub scenario_id: ScenarioId,
    pub reviewer: String,
    pub pin: String,
    pub upstream_ref: Option<String>,
    pub original_spec_ref: Option<String>,
    pub actor_id: String,
    pub role: RoleId,
    pub session_id: Option<SessionId>,
    pub outcome: ReviewOutcome,
    pub reviewer_provider: Option<String>,
    pub reviewer_model: Option<String>,
    pub reviewer_harness: Option<String>,
}

/// `canon review add` (module doc). Both provenance refs `None` or both
/// `Some`, and a scenario without a record, are refused before any write
/// is attempted. Returns the process exit code: `0` on a successful
/// write, `2` on a refused/malformed invocation.
pub fn run_add(repo: &Path, args: &ReviewAddArgs) -> i32 {
    let provenance_ref = match (&args.upstream_ref, &args.original_spec_ref) {
        (Some(r), None) => ProvenanceRef::UpstreamRef(r.clone()),
        (None, Some(r)) => ProvenanceRef::OriginalSpecRef(r.clone()),
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
    let gate_context = match GateCtx::from_repo(&repo).map_err(|e| e.to_string()).and_then(|ctx| GateContext::load(ctx, &SchemaRegistry::load(), Utc::now()).map_err(|e| e.to_string())) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("canon review add: {e}");
            return 2;
        }
    };
    let Some(scenario) = latest_scenarios(&gate_context).into_iter().find(|s| s.project_id == args.project_id && s.scenario_id == args.scenario_id) else {
        eprintln!(
            "canon review add: refused — no scenario record for `{}` in spec root `{}`; a review pins the scenario it saw, so index it first (`canon inventory sync`)",
            args.scenario_id.as_str(),
            args.project_id.as_str()
        );
        return 2;
    };
    let pins = current_pins(&gate_context, scenario);
    let committed = GitTier::new(&gate_context.ctx.ledger_root);

    let mut actor = Actor::new(&args.actor_id, args.role.clone());
    if let Some(session_id) = &args.session_id {
        actor = actor.with_session(session_id.clone());
    }
    let mut review = Review::new(
        Envelope::new(1, RecordKind::Review, Utc::now(), actor),
        args.project_id.clone(),
        args.scenario_id.clone(),
        &args.reviewer,
        &args.pin,
        provenance_ref,
    );
    review.outcome = args.outcome;
    review.reviewer_provider = args.reviewer_provider.clone();
    review.reviewer_model = args.reviewer_model.clone();
    review.reviewer_harness = args.reviewer_harness.clone();
    let pinned = format!(
        "canon review add: pinned {} at {} {} and {} latest evidence record(s); the review counts only while both are unchanged",
        args.scenario_id.as_str(),
        match pins.scenario_digest_scope {
            PinnedSpec::Scenario => "scenario digest",
            PinnedSpec::File => "source digest",
        },
        &pins.scenario_digest.as_str()[..12],
        pins.evidence.len()
    );
    review.pins = Some(pins);

    match committed.write(&review) {
        Ok(receipt) => {
            println!("{pinned}");
            if !args.outcome.is_signed() {
                println!("canon review add: outcome `{}` is recorded as a review gap, never as an approval", args.outcome.as_str());
            }
            println!(
                "canon review add: wrote {} ({}) — {}",
                receipt.location,
                if receipt.deduped { "deduped" } else { "new" },
                crate::write_mode::DIRECT
            );
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
    use canon_model::{EvidenceRecord, EvidenceVerdict, Scenario, SpecDigest};
    use canon_store::tier::TierQuery;
    use tempfile::TempDir;

    use super::*;

    fn args() -> ReviewAddArgs {
        ReviewAddArgs {
            project_id: ProjectId::parse("app-a").unwrap(),
            scenario_id: ScenarioId::parse("world.firstbuy-hotdeal.26").unwrap(),
            reviewer: "reviewer-1".into(),
            pin: "abc123pin".into(),
            upstream_ref: Some("routes/world.firstbuy-hotdeal.26#onReview".into()),
            original_spec_ref: None,
            actor_id: "agent-x".into(),
            role: RoleId::parse("reviewer").unwrap(),
            session_id: None,
            outcome: ReviewOutcome::Signed,
            reviewer_provider: None,
            reviewer_model: None,
            reviewer_harness: None,
        }
    }

    fn ledger(dir: &TempDir) -> GitTier {
        GitTier::new(GateCtx::from_repo(dir.path()).unwrap().ledger_root)
    }

    /// A repo whose ledger holds `args()`'s scenario record.
    fn repo_with_scenario() -> TempDir {
        let dir = TempDir::new().unwrap();
        let envelope = Envelope::new(1, RecordKind::Scenario, Utc::now(), Actor::new("canon", RoleId::parse("implementer").unwrap()));
        let a = args();
        ledger(&dir).write(&Scenario::new(envelope, a.project_id, a.scenario_id, "t", "", SpecDigest::of(b"feature bytes"))).unwrap();
        dir
    }

    fn reviews(dir: &TempDir) -> Vec<Review> {
        ledger(dir).read(&TierQuery::kind(RecordKind::Review)).unwrap().records.into_iter().map(|r| serde_json::from_value(r.0).unwrap()).collect()
    }

    #[test]
    fn writes_an_attributed_review_with_a_valid_upstream_ref() {
        let dir = repo_with_scenario();
        let session = SessionId::parse("review-session-1").unwrap();
        let code = run_add(dir.path(), &ReviewAddArgs { session_id: Some(session.clone()), ..args() });
        assert_eq!(code, 0);

        let reviews = reviews(&dir);
        assert_eq!(reviews.len(), 1);
        let review = &reviews[0];
        assert_eq!(review.envelope.actor.agent_id, "agent-x");
        assert_eq!(review.envelope.actor.role.as_ref().map(|r| r.as_str()), Some("reviewer"));
        assert_eq!(review.envelope.actor.session_id.as_ref(), Some(&session));
        assert_eq!(review.provenance_ref, ProvenanceRef::UpstreamRef("routes/world.firstbuy-hotdeal.26#onReview".to_string()));
        assert_eq!(review.outcome, ReviewOutcome::Signed);
    }

    /// review.add.20 (0.15 D1): the pins are computed from the corpus — the scenario's
    /// source digest, and an empty evidence list when it has none, then
    /// the latest evidence record once it does.
    #[test]
    fn pins_the_scenario_digest_and_its_latest_evidence() {
        let dir = repo_with_scenario();
        assert_eq!(run_add(dir.path(), &args()), 0);
        let pins = reviews(&dir)[0].pins.clone().expect("review add always pins");
        assert_eq!(pins.scenario_digest, SpecDigest::of(b"feature bytes"));
        assert!(pins.evidence.is_empty(), "no evidence yet: an empty list, still pinned");

        let a = args();
        let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), Actor::new("impl", RoleId::parse("implementer").unwrap()));
        let record = EvidenceRecord::new(envelope, None, Some(a.scenario_id.clone()), None, EvidenceVerdict::Faithful).with_project_id(a.project_id.clone());
        let receipt = ledger(&dir).write(&record).unwrap();
        assert_eq!(run_add(dir.path(), &ReviewAddArgs { reviewer: "reviewer-2".into(), ..args() }), 0);
        let second = reviews(&dir).into_iter().find(|r| r.reviewer == "reviewer-2").unwrap();
        let evidence = second.pins.unwrap().evidence;
        assert_eq!(evidence.len(), 1);
        assert_eq!(format!("kind=evidence_record/{}__{}.json", evidence[0].key, evidence[0].digest), receipt.location, "the pin names the ledger file");
    }

    /// review.add.26 (0.15 D4): the outcome and reviewer runtime land on the record.
    #[test]
    fn records_a_non_signed_outcome_and_the_reviewer_runtime() {
        let dir = repo_with_scenario();
        let code = run_add(
            dir.path(),
            &ReviewAddArgs {
                outcome: ReviewOutcome::Indeterminate,
                reviewer_provider: Some("openai".into()),
                reviewer_model: Some("gpt-x".into()),
                reviewer_harness: Some("codex".into()),
                ..args()
            },
        );
        assert_eq!(code, 0);
        let review = &reviews(&dir)[0];
        assert_eq!(review.outcome, ReviewOutcome::Indeterminate);
        assert_eq!(
            (review.reviewer_provider.as_deref(), review.reviewer_model.as_deref(), review.reviewer_harness.as_deref()),
            (Some("openai"), Some("gpt-x"), Some("codex"))
        );
        assert!(parse_runtime_label(" padded").is_err() && parse_runtime_label("").is_err() && parse_runtime_label("omp").is_ok());
    }

    /// review.add.21: no scenario record, nothing to pin — refused.
    #[test]
    fn refuses_a_scenario_without_a_record_and_writes_nothing() {
        let dir = TempDir::new().unwrap();
        assert_eq!(run_add(dir.path(), &args()), 2);
        assert!(reviews(&dir).is_empty(), "a refused review add must write nothing");
    }

    #[test]
    fn refuses_a_missing_provenance_ref_and_writes_nothing() {
        let dir = repo_with_scenario();
        assert_eq!(run_add(dir.path(), &ReviewAddArgs { upstream_ref: None, ..args() }), 2);
        assert!(reviews(&dir).is_empty(), "a refused review add must write nothing");
    }

    #[test]
    fn refuses_two_provenance_refs_at_once_and_writes_nothing() {
        let dir = repo_with_scenario();
        assert_eq!(run_add(dir.path(), &ReviewAddArgs { original_spec_ref: Some("spec/x.md".into()), ..args() }), 2);
        assert!(reviews(&dir).is_empty());
    }
}
