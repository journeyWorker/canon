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
//! # What the review saw is pinned, never typed (0.15 D1, 0.23 D5)
//! [`run_add`] reads the corpus the gate reads and records, on the
//! review, the scenario's current digest and what its latest evidence per
//! role says: the verdict and the identity of the checks it bound
//! ([`canon_gate::freshness::current_pins`]). The gate counts the review
//! only while both still match, so recording the suite again with the same
//! verdicts from the same checks leaves it current. A scenario with no record is
//! refused (exit `2`, nothing written): there is nothing to pin. A
//! scenario with no evidence pins an empty list.
//!
//! # A chore is reviewed as a change (0.23 D5)
//! `--change <slug>` writes one review of a chore change instead of one
//! per scenario: the reviewer signs the chore's invariants and checks
//! against the diff. It pins the brief ([`PinnedSpec::Brief`], the digest
//! [`crate::brief::brief_digest`] computes), so an edit to the brief
//! stales it, and it is what `--to shipped` asks of a chore
//! ([`canon_gate::review_gate::change_review_gap`]).
//!
//! # The reviewer's run can fail visibly (0.15 D4)
//! `--outcome` records `signed` (default), `indeterminate` or
//! `unavailable`; only `signed` is an approval. `--reviewer-provider`,
//! `--reviewer-model` and `--reviewer-harness` record the runtime that
//! produced the review, as the caller reports it.
//!
//! # What the reviewer looked at is stored (0.16 D5)
//! `--artifact <path>` (repeatable) binds a file the reviewer looked at —
//! a screenshot, a recording — through the same binding as `canon
//! evidence add --artifact` ([`crate::evidence_attach::bind_artifacts`]):
//! inside the repository, within `--max-artifact-mib`, its bytes copied
//! into `.canon/artifacts/sha256/<hex>` after every refusal has passed
//! and before the review is written. The review records each file's
//! `{path, sha256}`; `canon gate check` re-checks the files of every
//! current in-effect review by the evidence rule
//! (`canon_gate::artifact_store`). Canon never compares images.

use std::path::{Path, PathBuf};

use canon_gate::freshness::current_pins;
use canon_gate::spec_coverage::latest_scenarios;
use canon_gate::{GateContext, GateCtx};
use canon_model::{Actor, ChangeId, Envelope, PinnedSpec, ProjectId, ProvenanceRef, RecordKind, Review, ReviewArtifact, ReviewPins, ReviewOutcome, RoleId, ScenarioId, SessionId};
use canon_policy::SchemaRegistry;
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;
use chrono::Utc;

use crate::cli_error::CliError;
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
    /// `--scenario-id`, repeatable (0.18 D1): one review per scenario.
    pub scenario_ids: Vec<ScenarioId>,
    /// `--scenario-ids-from <file|->`: more ids, one per line, `#`
    /// starting a comment ([`parse_scenario_ids`]).
    pub scenario_ids_from: Option<String>,
    /// `--change <slug>`: a change-level review of a chore (module doc);
    /// excludes the scenario ids.
    pub change: Option<ChangeId>,
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
    /// Files the reviewer looked at (module doc), as given on the CLI.
    pub artifacts: Vec<PathBuf>,
    /// The artifact store's per-file size limit, in MiB.
    pub max_artifact_mib: u64,
    /// `--json`: one JSON document on stdout, every human line on stderr.
    pub json: bool,
}

/// The scenario ids of a `--scenario-ids-from` file: one per line, `#`
/// starting a comment that runs to the end of the line, blank lines
/// skipped. A line that is not a scenario id is an error naming it.
pub fn parse_scenario_ids(text: &str) -> Result<Vec<ScenarioId>, String> {
    text.lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let id = line.split('#').next().unwrap_or_default().trim();
            (!id.is_empty()).then(|| ScenarioId::parse(id).map_err(|e| format!("line {}: `{id}` is not a scenario id: {e}", index + 1)))
        })
        .collect()
}

/// Every scenario id the invocation names, in order and each once:
/// `--scenario-id` first, then `--scenario-ids-from` (`-` reads stdin).
fn requested_scenarios(args: &ReviewAddArgs) -> Result<Vec<ScenarioId>, String> {
    let mut ids = args.scenario_ids.clone();
    if let Some(source) = &args.scenario_ids_from {
        let text = if source == "-" {
            let mut text = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut text).map_err(|e| format!("--scenario-ids-from -: {e}"))?;
            text
        } else {
            std::fs::read_to_string(source).map_err(|e| format!("--scenario-ids-from {source}: {e}"))?
        };
        ids.extend(parse_scenario_ids(&text).map_err(|e| format!("--scenario-ids-from {source}: {e}"))?);
    }
    let mut unique: Vec<ScenarioId> = Vec::new();
    for id in ids {
        if !unique.contains(&id) {
            unique.push(id);
        }
    }
    if unique.is_empty() {
        return Err("no scenario to review: give --scenario-id <id> (repeatable), --scenario-ids-from <file|->, or --change <slug> for a chore".to_string());
    }
    Ok(unique)
}

/// `canon review add` (module doc). Both provenance refs `None` or both
/// `Some`, no scenario, and any scenario without a record are refused
/// before any write is attempted. With several scenarios (0.18 D1) it
/// writes one review per scenario with the same reviewer, pin and
/// artifacts, each pinning its own scenario, all or nothing. Returns the
/// process exit code: `0` on a successful write, `2` on a
/// refused/malformed invocation.
pub fn run_add(repo: &Path, args: &ReviewAddArgs) -> i32 {
    let provenance_ref = match (&args.upstream_ref, &args.original_spec_ref) {
        (Some(r), None) => ProvenanceRef::UpstreamRef(r.clone()),
        (None, Some(r)) => ProvenanceRef::OriginalSpecRef(r.clone()),
        (None, None) => {
            return CliError::refused("REVIEW_INVALID_ARGUMENT", "neither --upstream-ref nor --original-spec-ref given; a review must carry a resolvable provenance ref").hint("pass exactly one of --upstream-ref <ref> or --original-spec-ref <ref>").emit("canon review add", args.json);
        }
        (Some(_), Some(_)) => {
            return CliError::refused("REVIEW_INVALID_ARGUMENT", "--upstream-ref and --original-spec-ref are mutually exclusive; exactly one provenance ref is required").hint("pass exactly one of --upstream-ref <ref> or --original-spec-ref <ref>").emit("canon review add", args.json);
        }
    };
    let scenario_ids = if args.change.is_some() {
        Vec::new()
    } else {
        match requested_scenarios(args) {
            Ok(ids) => ids,
            Err(e) => {
                return CliError::refused("REVIEW_INVALID_ARGUMENT", e.to_string()).hint("`canon schema review add` lists the accepted arguments and their forms").emit("canon review add", args.json);
            }
        }
    };

    let repo = resolve_repo_root(repo);
    let gate_context = match GateCtx::from_repo(&repo).map_err(|e| e.to_string()).and_then(|ctx| GateContext::load(ctx, &SchemaRegistry::load(), Utc::now()).map_err(|e| e.to_string())) {
        Ok(ctx) => ctx,
        Err(e) => {
            return CliError::refused("CONFIG_UNREADABLE", e.to_string()).hint("fix canon.yaml so it parses (`canon init --check-config` reports what is wrong), then run the command again").emit("canon review add", args.json);
        }
    };
    let brief_digest = match &args.change {
        Some(change) => match chore_brief_digest(&repo, change) {
            Ok(digest) => Some(digest),
            Err(message) => {
                return CliError::refused("REVIEW_INVALID_ARGUMENT", message).field("--change").hint("a change-level review is for a chore change whose brief is in the repository; review feature scenarios with --scenario-id").emit("canon review add", args.json);
            }
        },
        None => None,
    };
    let indexed = latest_scenarios(&gate_context);
    let (found, missing): (Vec<_>, Vec<_>) =
        scenario_ids.iter().map(|id| (id, indexed.iter().find(|s| s.project_id == args.project_id && &s.scenario_id == id))).partition(|(_, s)| s.is_some());
    if !missing.is_empty() {
        let ids = missing.iter().map(|(id, _)| format!("`{}`", id.as_str())).collect::<Vec<_>>().join(", ");
        let nothing = if scenario_ids.len() > 1 { " (nothing is written for any of the scenarios)" } else { "" };
        return CliError::refused("REVIEW_SCENARIO_NOT_INDEXED", format!("no scenario record for {ids} in spec root `{}`; a review pins the scenario it saw, so index it first (`canon inventory sync`){nothing}",
            args.project_id.as_str()
        )).hint("run `canon inventory sync`, then run `canon review add` again").emit("canon review add", args.json);
    }
    let bound = match crate::evidence_attach::bind_artifacts(&repo, &args.artifacts, args.max_artifact_mib.saturating_mul(canon_gate::artifact_store::MIB)) {
        Ok(bound) => bound,
        Err(e) => {
            let refusal = CliError::refused("REVIEW_ARTIFACT_REFUSED", e.message().to_string()).hint("`canon schema review add` lists the artifact options and their limits");
            let refusal = match e.hint() {
                Some(hint) => refusal.hint(hint),
                None => refusal,
            };
            let refusal = match e.field() {
                Some(flag) => refusal.field(flag),
                None => refusal,
            };
            return refusal.emit("canon review add", args.json);
        }
    };
    let mut artifacts: Vec<ReviewArtifact> = Vec::new();
    let mut files: Vec<crate::evidence_attach::BoundFile> = Vec::new();
    for (path, file) in bound {
        if !artifacts.iter().any(|a| a.path == path && a.sha256 == file.sha256) {
            artifacts.push(ReviewArtifact { path, sha256: file.sha256.clone() });
        }
        if !files.iter().any(|f| f.sha256 == file.sha256) {
            files.push(file);
        }
    }
    let mut actor = Actor::new(&args.actor_id, args.role.clone());
    if let Some(session_id) = &args.session_id {
        actor = actor.with_session(session_id.clone());
    }
    let at = Utc::now();
    let mut reviews: Vec<(Review, String)> = Vec::new();
    if let (Some(change), Some(digest)) = (&args.change, brief_digest) {
        let mut review = Review::for_change(Envelope::new(1, RecordKind::Review, at, actor.clone()), args.project_id.clone(), change.clone(), &args.reviewer, &args.pin, provenance_ref.clone());
        review.outcome = args.outcome;
        review.reviewer_provider = args.reviewer_provider.clone();
        review.reviewer_model = args.reviewer_model.clone();
        review.reviewer_harness = args.reviewer_harness.clone();
        review.artifacts = artifacts.clone();
        let pinned = format!("canon review add: pinned change {} at brief digest {}; the review counts only while the brief is unchanged", change.as_str(), &digest.as_str()[..12]);
        review.pins = Some(ReviewPins { scenario_digest: digest, scenario_digest_scope: PinnedSpec::Brief, checks: Some(Vec::new()), evidence: Vec::new() });
        reviews.push((review, pinned));
    }
    reviews.extend(found.into_iter().filter_map(|(_, scenario)| scenario.copied()).map(|scenario| {
        let pins = current_pins(&gate_context, scenario);
        let mut review = Review::new(
            Envelope::new(1, RecordKind::Review, at, actor.clone()),
            args.project_id.clone(),
            scenario.scenario_id.clone(),
            &args.reviewer,
            &args.pin,
            provenance_ref.clone(),
        );
        review.outcome = args.outcome;
        review.reviewer_provider = args.reviewer_provider.clone();
        review.reviewer_model = args.reviewer_model.clone();
        review.reviewer_harness = args.reviewer_harness.clone();
        review.artifacts = artifacts.clone();
        let pinned = format!(
            "canon review add: pinned {} at {} {} and the verdict and checks of its {} role(s) of evidence; the review counts only while they are unchanged",
            scenario.scenario_id.as_str(),
            match pins.scenario_digest_scope {
                PinnedSpec::Scenario => "scenario digest",
                PinnedSpec::File => "source digest",
                PinnedSpec::Brief => "brief digest",
            },
            &pins.scenario_digest.as_str()[..12],
            pins.checks.as_ref().map_or(0, Vec::len)
        );
        review.pins = Some(pins);
        (review, pinned)
    }));

    // A review that repeats its scenario's latest one by this reviewer is not
    // written again (`crate::retry`); the others are.
    let ledger_root = &gate_context.ctx.ledger_root;
    let repeats: Vec<Option<crate::retry::Existing>> = reviews
        .iter()
        .map(|(review, _)| {
            let body = serde_json::to_value(review).expect("a Review always serializes");
            crate::retry::repeats_latest(ledger_root, None, RecordKind::Review.as_str(), &body, |stored| {
                ["project_id", "scenario_id", "change_id", "reviewer"].iter().all(|key| stored.get(key) == body.get(key))
            }, None)
        })
        .collect();
    let unchanged: Vec<(String, crate::retry::Existing)> =
        reviews.iter().zip(&repeats).filter_map(|((review, _), repeat)| repeat.clone().map(|existing| (review.target().to_string(), existing))).collect();
    for (scenario, existing) in &unchanged {
        crate::say!(args.json, "canon review add: unchanged — an identical review of {scenario} is already {}; nothing written", existing.describe());
    }
    let reviews: Vec<(Review, String)> = reviews.into_iter().zip(&repeats).filter(|(_, repeat)| repeat.is_none()).map(|(review, _)| review).collect();
    if reviews.is_empty() {
        if args.json {
            let wrote: Vec<serde_json::Value> = unchanged.iter().map(|(scenario, existing)| serde_json::json!({ "location": existing.committed, "scenarioId": scenario, "unchanged": true })).collect();
            crate::json_out::data_with_meta("review add", serde_json::json!({ "outcome": args.outcome.as_str(), "reviews": wrote, "blobsStored": 0 }), serde_json::json!({ "unchanged": true }));
        }
        return 0;
    }

    // Store the bytes before any review is written (module doc): every
    // refusal is behind us, and a review must never name a blob the
    // store lacks. All or nothing (0.18 D1): a failed write removes what
    // this call wrote and stored.
    let only_reviews: Vec<Review> = reviews.iter().map(|(review, _)| review.clone()).collect();
    let (stored, written) = match write_reviews(&repo, &gate_context.ctx.ledger_root, &files, &only_reviews, &mut |tier, review| tier.write(review)) {
        Ok(landed) => landed,
        Err(error) => {
            let mut error = CliError::refused("WRITE_FAILED", error.to_string()).hint("check that the path is writable and the disk has room, then run the command again");
            if reviews.len() > 1 {
                error = error.hint("no review is written: every review and blob this run wrote or stored was removed");
            }
            return error.emit("canon review add", args.json);
        }
    };
    for (_, pinned) in &reviews {
        crate::say!(args.json, "{pinned}");
    }
    if !files.is_empty() {
        crate::say!(args.json, "{}", crate::evidence_attach::stored_line("canon review add", files.len(), stored));
    }
    if !args.outcome.is_signed() {
        crate::say!(args.json, "canon review add: outcome `{}` is recorded as a review gap, never as an approval", args.outcome.as_str());
    }
    for (location, deduped) in &written {
        crate::say!(args.json, "canon review add: wrote {location} ({}) — {}", if *deduped { "deduped" } else { "new" }, crate::write_mode::DIRECT);
    }
    let mut wrote: Vec<serde_json::Value> = written.iter().map(|(location, deduped)| serde_json::json!({ "location": location, "deduped": deduped })).collect();
    wrote.extend(unchanged.iter().map(|(scenario, existing)| serde_json::json!({ "location": existing.committed, "scenarioId": scenario, "unchanged": true })));
    let scope = match &args.change {
        Some(change) => crate::next::Scope::Change(change.as_str().to_string()),
        None => crate::next::Scope::Scenarios(scenario_ids.iter().map(|id| id.as_str().to_string()).collect()),
    };
    crate::next::finish(&repo, "review add", &scope, false, args.json, serde_json::json!({ "outcome": args.outcome.as_str(), "reviews": wrote, "blobsStored": stored }));
    0
}

/// The digest a change-level review of `change` pins: its chore brief as
/// the gate reads it now. A change with no brief in the repository, or a
/// brief that is not a chore's, is refused with the reason.
fn chore_brief_digest(repo: &Path, change: &ChangeId) -> Result<canon_model::SpecDigest, String> {
    let path = crate::brief::canon_brief(repo, change).ok_or_else(|| format!("change `{}` has no brief in the repository", change.as_str()))?;
    let text = std::fs::read_to_string(&path).map_err(|e| format!("cannot read the brief of change `{}` ({}): {e}", change.as_str(), path.display()))?;
    if !crate::brief_check::is_chore(&text) {
        return Err(format!("change `{}` is not a chore: its brief does not mark it one, and a feature is reviewed per scenario", change.as_str()));
    }
    Ok(crate::brief::brief_spec_digest(&text))
}

/// Write `reviews` all or nothing (0.18 D1): check the committed review
/// directory can be written, store `files` under the store lock, then
/// write each review through `write` (`GitTier::write` outside tests).
/// A failed write removes every review this call wrote (a deduped one
/// already existed and stays) and every blob it newly stored, and is
/// returned as the error. `Ok` is the blobs newly stored and each
/// review's `(location, deduped)`.
fn write_reviews(
    repo: &Path,
    ledger_root: &Path,
    files: &[crate::evidence_attach::BoundFile],
    reviews: &[Review],
    write: &mut dyn FnMut(&GitTier, &Review) -> Result<canon_store::tier::WriteReceipt, canon_store::tier::StoreError>,
) -> Result<(usize, Vec<(String, bool)>), String> {
    let dir = ledger_root.join(format!("kind={}", RecordKind::Review.as_str()));
    match std::fs::create_dir_all(&dir).and_then(|()| std::fs::metadata(&dir)) {
        Ok(meta) if meta.permissions().readonly() => return Err(format!("{} is read-only", dir.display())),
        Ok(_) => {}
        Err(e) => return Err(format!("cannot prepare {}: {e}", dir.display())),
    }
    let held = crate::evidence_attach::store_files_held(repo, files)
        .map_err(|(file, e)| format!("could not store {} in the artifact store: {e}", file.resolved.display()))?;
    let committed = GitTier::new(ledger_root);
    let mut written: Vec<(String, bool)> = Vec::new();
    for review in reviews {
        match write(&committed, review) {
            Ok(receipt) => written.push((receipt.location, receipt.deduped)),
            Err(e) => {
                for (location, deduped) in &written {
                    if !deduped {
                        let _ = std::fs::remove_file(ledger_root.join(location));
                    }
                }
                if !written.is_empty() {
                    crate::ledger::restore_after_rollback(ledger_root, RecordKind::Review);
                }
                held.rollback();
                if reviews.len() == 1 {
                    return Err(e.to_string());
                }
                return Err(format!("writing review {} of {} failed: {e}", written.len() + 1, reviews.len()));
            }
        }
    }
    Ok((held.stored(), written))
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
            scenario_ids: vec![ScenarioId::parse("world.firstbuy-hotdeal.26").unwrap()],
            scenario_ids_from: None,
            change: None,
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
            artifacts: Vec::new(),
            max_artifact_mib: canon_gate::DEFAULT_MAX_ARTIFACT_MIB,
            json: false,
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
        ledger(&dir).write(&Scenario::new(envelope, a.project_id, a.scenario_ids[0].clone(), "t", "", SpecDigest::of(b"feature bytes"))).unwrap();
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

    /// review.add.20 (0.15 D1, 0.23 D5): the pins are computed from the
    /// corpus: the scenario's digest, and an empty list of checks when it
    /// has no evidence, then the latest evidence's verdict (and the identity
    /// of its checks) once it does.
    #[test]
    fn pins_the_scenario_digest_and_what_its_latest_evidence_says() {
        let dir = repo_with_scenario();
        assert_eq!(run_add(dir.path(), &args()), 0);
        let pins = reviews(&dir)[0].pins.clone().expect("review add always pins");
        assert_eq!(pins.scenario_digest, SpecDigest::of(b"feature bytes"));
        assert_eq!(pins.checks, Some(Vec::new()), "no evidence yet: an empty list, still pinned");
        assert!(pins.evidence.is_empty(), "a new review never pins evidence record ids");

        let a = args();
        let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), Actor::new("impl", RoleId::parse("implementer").unwrap()));
        let record = EvidenceRecord::new(envelope, None, Some(a.scenario_ids[0].clone()), None, EvidenceVerdict::Faithful).with_project_id(a.project_id.clone());
        ledger(&dir).write(&record).unwrap();
        assert_eq!(run_add(dir.path(), &ReviewAddArgs { reviewer: "reviewer-2".into(), ..args() }), 0);
        let second = reviews(&dir).into_iter().find(|r| r.reviewer == "reviewer-2").unwrap();
        let checks = second.pins.unwrap().checks.unwrap();
        assert_eq!(checks.len(), 1);
        assert_eq!((checks[0].role.as_deref(), checks[0].verdict), (Some("implementer"), EvidenceVerdict::Faithful));
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

    /// review.add.81: a write failure on the second review of a batch
    /// removes the first and every blob the call stored; a read-only
    /// review directory is refused before anything is written.
    #[test]
    fn review_add_81_a_batch_whose_second_write_fails_leaves_nothing() {
        let dir = repo_with_scenario();
        let ledger_root = GateCtx::from_repo(dir.path()).unwrap().ledger_root;
        std::fs::write(dir.path().join("shot.png"), b"png").unwrap();
        let files: Vec<crate::evidence_attach::BoundFile> =
            crate::evidence_attach::bind_artifacts(dir.path(), &[dir.path().join("shot.png")], 1024).unwrap().into_iter().map(|(_, file)| file).collect();
        let store = canon_gate::artifact_store::store_dir(dir.path());
        let blobs = || std::fs::read_dir(&store).map(|d| d.count()).unwrap_or(0);
        let review = |scenario: &str| {
            let a = args();
            Review::new(
                Envelope::new(1, RecordKind::Review, Utc::now(), Actor::new("agent-x", RoleId::parse("reviewer").unwrap())),
                a.project_id,
                ScenarioId::parse(scenario).unwrap(),
                "reviewer-1",
                "pin",
                ProvenanceRef::UpstreamRef("r".into()),
            )
        };
        let batch = vec![review("cart.add.01"), review("cart.add.02")];

        let mut calls = 0;
        let mut failing_second = |tier: &GitTier, review: &Review| {
            calls += 1;
            if calls == 2 {
                return Err(canon_store::tier::StoreError::Io(std::io::Error::other("disk full")));
            }
            tier.write(review)
        };
        let error = write_reviews(dir.path(), &ledger_root, &files, &batch, &mut failing_second).unwrap_err();
        assert!(error.contains("writing review 2 of 2 failed: ") && error.contains("disk full"), "{error}");
        assert!(reviews(&dir).is_empty(), "the first review is removed");
        assert_eq!(blobs(), 0, "the blob this call stored is removed");

        // One scenario: the error is the write's own, as before 0.18,
        // so `run_add` prints `canon review add: <error>` unchanged.
        let disk_full = || canon_store::tier::StoreError::Io(std::io::Error::other("disk full"));
        let single = write_reviews(dir.path(), &ledger_root, &files, &batch[..1], &mut |_, _| Err(disk_full())).unwrap_err();
        assert_eq!(single, disk_full().to_string(), "a single review's failure keeps its pre-0.18 wording");
        assert_eq!((reviews(&dir).len(), blobs()), (0, 0));

        let review_dir = ledger_root.join("kind=review");
        let mut readonly = std::fs::metadata(&review_dir).unwrap().permissions();
        readonly.set_readonly(true);
        std::fs::set_permissions(&review_dir, readonly.clone()).unwrap();
        let refused = write_reviews(dir.path(), &ledger_root, &files, &batch, &mut |tier, review| tier.write(review));
        readonly.set_readonly(false);
        std::fs::set_permissions(&review_dir, readonly).unwrap();
        assert!(refused.unwrap_err().contains("is read-only"));
        assert_eq!(blobs(), 0);

        let (stored, written) = write_reviews(dir.path(), &ledger_root, &files, &batch, &mut |tier, review| tier.write(review)).unwrap();
        assert_eq!((stored, written.len(), reviews(&dir).len(), blobs()), (1, 2, 2, 1));
    }
}
