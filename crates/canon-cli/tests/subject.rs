//! Integration tests for `canon subject {new,adopt,status}` (s36
//! `subject-domain-loop`), invoking the actually-built `canon` binary
//! against an offline git-tier fixture in a tmpdir — zero network, no
//! credentials (mirrors `tests/gate.rs`/`tests/query.rs`'s shape). The
//! subject write path routes through `TierRegistry` (subject → `local`
//! rung → git tier at `.canon/ledger`); records seeded directly here go
//! through the SAME `GitTier` root, so the CLI reads back exactly what
//! the fixtures plant.

use std::path::Path;
use std::process::{Command, Output};

use canon_model::{
    Actor, Change, ChangeId, ChangeStatus, Envelope, EvidenceRecord, EvidenceVerdict, Finding, FindingSeverity, ProjectId, ProvenanceRef, RecordKind, Review,
    RoleId, Scenario, ScenarioId, Sha, SpecDigest, Subject, SubjectId, SubjectStatus,
};
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;
use chrono::Utc;
use serde_json::Value;
use tempfile::TempDir;

/// A minimal, WORKING `canon.yaml` routing every kind this suite
/// touches to the git-backed `local` rung at `.canon/ledger` — the same
/// root `GateCtx::from_repo` (the `verifying → shipped` evidence gate)
/// resolves from `tiers.local.root`.
const CANON_YAML: &str = "\
tiers:
  local: { backend: git, root: .canon/ledger }
routing:
  subject: local
  change: local
  scenario: local
  evidence_record: local
";

fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("canon.yaml"), CANON_YAML).unwrap();
    dir
}

fn run(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(repo).output().expect("spawn canon binary")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

fn ledger(repo: &Path) -> GitTier {
    GitTier::new(repo.join(".canon/ledger"))
}

fn seed_change(repo: &Path, change_id: &str) {
    let envelope = Envelope::new(1, RecordKind::Change, Utc::now(), Actor::new("importer", RoleId::parse("planner").unwrap()));
    let change = Change::new(envelope, ChangeId::parse(change_id).unwrap(), "Imported", "why", ChangeStatus::InProgress);
    ledger(repo).write(&change).unwrap();
}

/// Plant a `Subject` at an arbitrary lifecycle state, plus one `Scenario`
/// per entry in `scenarios` carrying `subject_id` exactly as `canon
/// inventory sync` writes it from an `@subject:<id>` tag — the only
/// way a subject owns scenarios, and the only way to reach the
/// `verifying → shipped` gate's per-scenario checks.
fn seed_subject(repo: &Path, id: &str, status: SubjectStatus, scenarios: &[&str]) {
    seed_subject_with_changes(repo, id, status, scenarios, &[]);
}

fn seed_subject_with_changes(repo: &Path, id: &str, status: SubjectStatus, scenarios: &[&str], changes: &[&str]) {
    let envelope = Envelope::new(1, RecordKind::Subject, Utc::now(), Actor::new("canon", RoleId::parse("implementer").unwrap()));
    let subject = Subject::new(envelope, SubjectId::parse(id).unwrap(), "Seeded", "s", "dev", status, RoleId::parse("implementer").unwrap())
        .with_change_ids(changes.iter().map(|c| ChangeId::parse(*c).unwrap()).collect());
    ledger(repo).write(&subject).unwrap();
    for scenario_id in scenarios {
        let envelope = Envelope::new(1, RecordKind::Scenario, Utc::now(), Actor::new("canon", RoleId::parse("implementer").unwrap()));
        let mut scenario = Scenario::new(
            envelope,
            ProjectId::parse("demo").unwrap(),
            ScenarioId::parse(*scenario_id).unwrap(),
            "seeded",
            "",
            SpecDigest::of(scenario_id.as_bytes()),
        );
        scenario.subject_id = Some(SubjectId::parse(id).unwrap());
        ledger(repo).write(&scenario).unwrap();
    }
}

fn seed_scenario_verdict(repo: &Path, scenario: &str, verdict: EvidenceVerdict) {
    let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), Actor::new("reviewer-1", RoleId::parse("reviewer").unwrap()));
    let record = EvidenceRecord::new(envelope, None, Some(ScenarioId::parse(scenario).unwrap()), None, verdict);
    ledger(repo).write(&record).unwrap();
}

fn query_subjects(repo: &Path, extra: &[&str]) -> Value {
    let mut args = vec!["query", "--kind", "subject", "--json"];
    args.extend_from_slice(extra);
    let out = run(repo, &args);
    assert!(out.status.success(), "query failed: {}", stderr(&out));
    serde_json::from_str(&stdout(&out)).expect("valid JSON on stdout")
}

#[test]
fn new_then_query_round_trips_the_authored_subject() {
    let dir = repo();
    let out = run(dir.path(), &["subject", "new", "demo-subject", "--domain", "dev", "--title", "Demo"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    let payload = query_subjects(dir.path(), &[]);
    assert_eq!(payload["count"], 1);
    let records = payload["records"].as_array().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["subject_id"], "demo-subject");
    assert_eq!(records[0]["domain"], "dev");
    assert_eq!(records[0]["status"], "proposed");
}

#[test]
fn duplicate_new_is_refused_and_leaves_the_store_unchanged() {
    let dir = repo();
    assert!(run(dir.path(), &["subject", "new", "demo-subject", "--domain", "dev", "--title", "First"]).status.success());

    let out = run(dir.path(), &["subject", "new", "demo-subject", "--domain", "planning", "--title", "Second"]);
    assert_eq!(out.status.code(), Some(2), "a duplicate id must be refused");
    assert!(stderr(&out).contains("already exists"), "stderr: {}", stderr(&out));

    // Still exactly one row, still the original domain (the second
    // write never happened).
    let payload = query_subjects(dir.path(), &[]);
    assert_eq!(payload["count"], 1);
    assert_eq!(payload["records"][0]["domain"], "dev");
}

#[test]
fn adopt_links_the_change_and_the_subject_on_both_sides() {
    let dir = repo();
    seed_change(dir.path(), "s36-demo");
    assert!(run(dir.path(), &["subject", "new", "demo-subject", "--domain", "dev", "--title", "Demo"]).status.success());

    let out = run(dir.path(), &["subject", "adopt", "s36-demo", "--subject", "demo-subject"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    // Subject side: `change_ids` gained the change (folded to one row).
    let subjects = query_subjects(dir.path(), &[]);
    assert_eq!(subjects["count"], 1);
    let change_ids = subjects["records"][0]["change_ids"].as_array().unwrap();
    assert!(change_ids.iter().any(|c| c == "s36-demo"), "subject.change_ids must include the adopted change: {change_ids:?}");

    // Change side: some version carries the stamped `subject_id`.
    let cq = run(dir.path(), &["query", "--kind", "change", "--json"]);
    assert!(cq.status.success(), "stderr: {}", stderr(&cq));
    let change_payload: Value = serde_json::from_str(&stdout(&cq)).unwrap();
    let changes = change_payload["records"].as_array().unwrap();
    assert!(
        changes.iter().any(|c| c["subject_id"] == "demo-subject"),
        "an adopted change must carry subject_id=demo-subject: {changes:?}"
    );
}

#[test]
fn adopt_refuses_an_unknown_change() {
    let dir = repo();
    assert!(run(dir.path(), &["subject", "new", "demo-subject", "--domain", "dev", "--title", "Demo"]).status.success());
    let out = run(dir.path(), &["subject", "adopt", "no-such-change", "--subject", "demo-subject"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("does not exist"), "stderr: {}", stderr(&out));
}

fn ledger_file_count(repo: &Path) -> usize {
    fn walk(dir: &Path) -> usize {
        std::fs::read_dir(dir).map(|entries| entries.filter_map(Result::ok).map(|e| if e.path().is_dir() { walk(&e.path()) } else { 1 }).sum()).unwrap_or(0)
    }
    walk(&repo.join(".canon/ledger"))
}

/// Adopting a pair that already carries the link writes nothing, so the
/// command is safe to rerun.
#[test]
fn adopt_is_a_no_op_on_an_already_linked_pair() {
    let dir = repo();
    seed_change(dir.path(), "s36-demo");
    assert!(run(dir.path(), &["subject", "new", "demo-subject", "--domain", "dev", "--title", "Demo"]).status.success());
    assert!(run(dir.path(), &["subject", "adopt", "s36-demo", "--subject", "demo-subject"]).status.success());

    let before = ledger_file_count(dir.path());
    let again = run(dir.path(), &["subject", "adopt", "s36-demo", "--subject", "demo-subject"]);
    assert_eq!(again.status.code(), Some(0), "{}", stderr(&again));
    assert!(stdout(&again).contains("already linked to subject `demo-subject`; nothing written"), "{}", stdout(&again));
    assert_eq!(ledger_file_count(dir.path()), before);
}

/// The subject record is written first; when the change write after it
/// fails, the refusal names the half-written state and the command that
/// completes it, and that command — rerunning adopt — does.
#[cfg(unix)]
#[test]
fn adopt_failing_between_its_two_writes_prints_a_repair_that_completes_the_link() {
    use std::os::unix::fs::PermissionsExt;

    let dir = repo();
    seed_change(dir.path(), "s36-demo");
    assert!(run(dir.path(), &["subject", "new", "demo-subject", "--domain", "dev", "--title", "Demo"]).status.success());
    let ledger_change = dir.path().join(".canon/ledger/kind=change");
    std::fs::set_permissions(&ledger_change, std::fs::Permissions::from_mode(0o555)).unwrap();
    if std::fs::write(ledger_change.join("probe"), "").is_ok() {
        // Root ignores directory permissions; there is no failure to force.
        std::fs::remove_file(ledger_change.join("probe")).unwrap();
        std::fs::set_permissions(&ledger_change, std::fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }

    let out = run(dir.path(), &["subject", "adopt", "s36-demo", "--subject", "demo-subject"]);
    std::fs::set_permissions(&ledger_change, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(out.status.code(), Some(2), "{}", stdout(&out));
    assert!(
        stderr(&out).contains("subject `demo-subject` now lists change `s36-demo`, but the change record could not be written")
            && stderr(&out).contains("complete the link with `canon subject adopt s36-demo --subject demo-subject`"),
        "{}",
        stderr(&out)
    );
    let subjects = query_subjects(dir.path(), &[]);
    assert!(subjects["records"][0]["change_ids"].as_array().unwrap().iter().any(|c| c == "s36-demo"));

    let repair = run(dir.path(), &["subject", "adopt", "s36-demo", "--subject", "demo-subject"]);
    assert_eq!(repair.status.code(), Some(0), "{}", stderr(&repair));
    let changes: Value = serde_json::from_str(&stdout(&run(dir.path(), &["query", "--kind", "change", "--json"]))).unwrap();
    assert!(changes["records"].as_array().unwrap().iter().any(|c| c["subject_id"] == "demo-subject"), "{changes}");
    let again = run(dir.path(), &["subject", "adopt", "s36-demo", "--subject", "demo-subject"]);
    assert!(stdout(&again).contains("nothing written"), "{}", stdout(&again));
}

#[test]
fn the_forward_status_chain_advances_and_folds_to_one_row() {
    let dir = repo();
    assert!(run(dir.path(), &["subject", "new", "demo-subject", "--domain", "dev", "--title", "Demo"]).status.success());

    for state in ["specced", "building", "verifying"] {
        let out = run(dir.path(), &["subject", "status", "demo-subject", state]);
        assert!(out.status.success(), "transition to {state} failed: {}", stderr(&out));
    }

    // Four writes (new + three transitions) fold to ONE current row.
    let payload = query_subjects(dir.path(), &[]);
    assert_eq!(payload["count"], 1, "adopt/status re-writes must read back as one latest row");
    assert_eq!(payload["records"][0]["status"], "verifying");
}

#[test]
fn an_off_chain_transition_is_refused_and_the_record_is_unchanged() {
    let dir = repo();
    assert!(run(dir.path(), &["subject", "new", "demo-subject", "--domain", "dev", "--title", "Demo"]).status.success());

    // proposed → shipped skips the chain.
    let out = run(dir.path(), &["subject", "status", "demo-subject", "shipped"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("invalid transition"), "stderr: {}", stderr(&out));

    let payload = query_subjects(dir.path(), &[]);
    assert_eq!(payload["records"][0]["status"], "proposed");
}

#[test]
fn shipped_is_blocked_when_a_linked_scenario_has_no_verdict() {
    let dir = repo();
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Verifying, &["world.demo.01"]);

    let out = run(dir.path(), &["subject", "status", "demo-subject", "shipped"]);
    assert_eq!(out.status.code(), Some(1), "verifying → shipped must fail closed without evidence");
    let err = stderr(&out);
    assert!(err.contains("uncovered-cell"), "must print by failure class: {err}");
    assert!(err.contains("world.demo.01"), "must name the uncovered scenario: {err}");

    // Record unchanged — still verifying.
    let payload = query_subjects(dir.path(), &[]);
    assert_eq!(payload["records"][0]["status"], "verifying");
}

/// The defect this closes: the gate used to read a subject-side link
/// list no command ever wrote, so it was always empty and `shipped`
/// passed with zero evidence. A subject that owns no tagged scenario
/// has nothing to ship on.
#[test]
fn shipped_is_blocked_when_no_scenario_is_tagged_to_the_subject() {
    let dir = repo();
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Verifying, &[]);

    let out = run(dir.path(), &["subject", "status", "demo-subject", "shipped"]);
    assert_eq!(out.status.code(), Some(1), "an empty owned set must fail closed, not pass vacuously");
    let err = stderr(&out);
    assert!(err.contains("uncovered-cell") && err.contains("@subject:demo-subject"), "must name the missing tag: {err}");

    let payload = query_subjects(dir.path(), &[]);
    assert_eq!(payload["records"][0]["status"], "verifying");
}

#[test]
fn shipped_is_blocked_when_a_linked_scenario_is_divergent() {
    let dir = repo();
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Verifying, &["world.demo.01"]);
    seed_scenario_verdict(dir.path(), "world.demo.01", EvidenceVerdict::Divergent);

    let out = run(dir.path(), &["subject", "status", "demo-subject", "shipped"]);
    assert_eq!(out.status.code(), Some(1), "a divergent latest verdict must fail closed");
    assert!(stderr(&out).contains("divergent"), "stderr: {}", stderr(&out));

    let payload = query_subjects(dir.path(), &[]);
    assert_eq!(payload["records"][0]["status"], "verifying");
}

#[test]
fn shipped_is_allowed_with_a_faithful_verdict_for_every_linked_scenario() {
    let dir = repo();
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Verifying, &["world.demo.01"]);
    seed_scenario_verdict(dir.path(), "world.demo.01", EvidenceVerdict::Faithful);

    let out = run(dir.path(), &["subject", "status", "demo-subject", "shipped"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));

    let payload = query_subjects(dir.path(), &[]);
    assert_eq!(payload["records"][0]["status"], "shipped");
}

/// With `spec_coverage.require_cases`, attested golden-path scenarios
/// are not enough to ship: the surface must also specify a failure
/// path. Adding one (attested) on the same surface clears the refusal.
#[test]
fn shipped_is_blocked_until_each_owned_surface_specifies_a_required_case() {
    let dir = repo();
    std::fs::create_dir_all(dir.path().join(".canon")).unwrap();
    std::fs::write(dir.path().join(".canon/policy.yaml"), "spec_coverage:\n  require_evidence: true\n  require_cases: [failure]\n").unwrap();
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Verifying, &["world.demo.01"]);
    seed_scenario_verdict(dir.path(), "world.demo.01", EvidenceVerdict::Faithful);

    let out = run(dir.path(), &["subject", "status", "demo-subject", "shipped"]);
    assert_eq!(out.status.code(), Some(1), "a golden-path-only surface must fail closed: {}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("world.demo") && err.contains("@case:failure"), "must name the surface and the missing case: {err}");
    assert_eq!(query_subjects(dir.path(), &[])["records"][0]["status"], "verifying");

    let envelope = Envelope::new(1, RecordKind::Scenario, Utc::now(), Actor::new("canon", RoleId::parse("implementer").unwrap()));
    let mut failure = Scenario::new(envelope, ProjectId::parse("demo").unwrap(), ScenarioId::parse("world.demo.02").unwrap(), "refused", "", SpecDigest::of(b"02"));
    failure.subject_id = Some(SubjectId::parse("demo-subject").unwrap());
    failure.case = Some("failure".to_string());
    ledger(dir.path()).write(&failure).unwrap();
    seed_scenario_verdict(dir.path(), "world.demo.02", EvidenceVerdict::Faithful);

    let out = run(dir.path(), &["subject", "status", "demo-subject", "shipped"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(query_subjects(dir.path(), &[])["records"][0]["status"], "shipped");
}

#[test]
fn domain_and_status_filters_scope_the_subject_view() {
    let dir = repo();
    assert!(run(dir.path(), &["subject", "new", "alpha", "--domain", "dev", "--title", "A"]).status.success());
    assert!(run(dir.path(), &["subject", "new", "beta", "--domain", "planning", "--title", "B"]).status.success());

    // --domain filters to one row.
    let dev = query_subjects(dir.path(), &["--domain", "dev"]);
    assert_eq!(dev["count"], 1);
    assert_eq!(dev["records"][0]["subject_id"], "alpha");

    // --status filters by the subject's own status domain.
    let proposed = query_subjects(dir.path(), &["--status", "proposed"]);
    assert_eq!(proposed["count"], 2);
    let shipped = query_subjects(dir.path(), &["--status", "shipped"]);
    assert_eq!(shipped["count"], 0);
}

#[test]
fn domain_filter_is_rejected_on_a_non_subject_kind() {
    let dir = repo();
    let out = run(dir.path(), &["query", "--kind", "change", "--domain", "dev"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("--domain"), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains("--kind subject"), "must name the one supported kind: {}", stderr(&out));
}

// ── `--domain` membership against the repo's activated vocabulary ──

/// Declare a `domain` enum in a consumer vocabulary plugin and activate
/// it. A `kind: project` plugin is inert until a profile in
/// `canon.project.yaml` names it — `canon.core` is the only plugin
/// scanned unconditionally — so both files are required for the enum to
/// reach the resolved snapshot.
fn activate_domain_vocab(repo: &Path, members: &[&str]) {
    std::fs::write(repo.join("canon.project.yaml"), "defaultProfile: default\nprofiles:\n  default:\n    plugins:\n      studio.game: {}\n")
        .unwrap();
    let plugin = repo.join(".canon/vocab/studio.game");
    std::fs::create_dir_all(&plugin).unwrap();
    std::fs::write(plugin.join("plugin.yaml"), "id: studio.game\nversion: \"0.1.0\"\nkind: project\nexports:\n  enums: enums.yaml\n").unwrap();
    std::fs::write(plugin.join("enums.yaml"), format!("enums:\n  domain: [{}]\n", members.join(", "))).unwrap();
}

/// The team's own cut is what gets enforced — canon ships no opinion
/// about how a repo slices its work. A studio declaring `combat`/
/// `live-ops` authors against those and nothing else.
#[test]
fn a_domain_the_repos_own_vocabulary_declares_is_accepted() {
    let dir = repo();
    activate_domain_vocab(dir.path(), &["combat", "economy", "live-ops"]);

    let out = run(dir.path(), &["subject", "new", "boss-rework", "--domain", "live-ops", "--title", "Boss rework"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("live-ops"), "stdout: {}", stdout(&out));
}

/// The defect this closes: a typo used to mint a new category silently,
/// so every `--domain`-filtered read disagreed about how many the repo
/// has. Refusal is exit `2` and names the legal set, the same
/// "expected one of: …" grammar the typed-atom checker uses.
#[test]
fn a_domain_outside_the_activated_vocabulary_is_refused_naming_the_legal_set() {
    let dir = repo();
    activate_domain_vocab(dir.path(), &["combat", "economy", "live-ops"]);

    let out = run(dir.path(), &["subject", "new", "typo", "--domain", "combatt", "--title", "Typo"]);
    assert_eq!(out.status.code(), Some(2), "stdout: {}", stdout(&out));
    let err = stderr(&out);
    assert!(err.contains("expected one of: combat, economy, live-ops"), "must name the legal set: {err}");
    assert!(err.contains("combatt"), "must name the offender: {err}");

    let listed = run(dir.path(), &["query", "--kind", "subject"]);
    assert!(!stdout(&listed).contains("typo"), "the refused subject must not have been written: {}", stdout(&listed));
}

/// canon's OWN base vocabulary carries no authority in a repo that
/// declared its own set — the enum is resolved per repo, never
/// hardcoded. `dev` is a `canon.core` member and still refused here.
#[test]
fn canons_base_vocabulary_does_not_leak_into_a_repo_with_its_own() {
    let dir = repo();
    activate_domain_vocab(dir.path(), &["combat", "economy"]);

    let out = run(dir.path(), &["subject", "new", "x", "--domain", "dev", "--title", "x"]);
    assert_eq!(out.status.code(), Some(2), "stdout: {}", stdout(&out));
    assert!(stderr(&out).contains("expected one of: combat, economy"), "stderr: {}", stderr(&out));
}

/// Fail-soft when nothing is declared: `canon init` scaffolds no
/// `.canon/vocab`, so a repo that never opted into a vocabulary must
/// still be able to author its first Subject. Absent enum = no
/// constraint, mirroring canon-gate's empty-`risk_routing` default.
#[test]
fn a_repo_declaring_no_domain_vocabulary_accepts_any_kebab_slug() {
    let dir = repo();

    let out = run(dir.path(), &["subject", "new", "loot", "--domain", "game-economy", "--title", "Loot"]);
    assert!(out.status.success(), "an undeclared vocabulary must not block authoring: {}", stderr(&out));
}

/// Shape is a separate, earlier question: a non-slug is refused on its
/// own grammar even where a vocabulary is active, so the two checks
/// never collapse into one message.
#[test]
fn a_malformed_domain_is_refused_on_shape_before_membership() {
    let dir = repo();
    activate_domain_vocab(dir.path(), &["combat"]);

    let out = run(dir.path(), &["subject", "new", "x", "--domain", "Combat Systems", "--title", "x"]);
    assert_eq!(out.status.code(), Some(2), "stdout: {}", stdout(&out));
    let err = stderr(&out);
    assert!(err.contains("kebab-case slug"), "shape must be the reported cause: {err}");
    assert!(!err.contains("expected one of"), "membership must not also fire: {err}");
}

// ── spec_coverage.require_review (issue #2) ──

const REQUIRE_REVIEW: &str = "spec_coverage:\n  require_evidence: true\n  require_review: {}\n";

fn write_policy(repo: &Path, yaml: &str) {
    std::fs::create_dir_all(repo.join(".canon")).unwrap();
    std::fs::write(repo.join(".canon/policy.yaml"), yaml).unwrap();
}

/// A `building` subject owning `world.demo.01`, adopted change `c-demo`,
/// with implementer evidence by `impl-agent` — the issue's own starting
/// point: attested by its author, never reviewed.
fn seed_attested_subject(repo: &Path) {
    seed_subject_with_changes(repo, "demo-subject", SubjectStatus::Building, &["world.demo.01"], &["c-demo"]);
    let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), Actor::new("impl-agent", RoleId::parse("implementer").unwrap()));
    let record = EvidenceRecord::new(envelope, None, Some(ScenarioId::parse("world.demo.01").unwrap()), None, EvidenceVerdict::Faithful)
        .with_project_id(ProjectId::parse("demo").unwrap());
    ledger(repo).write(&record).unwrap();
}

fn seed_review(repo: &Path, reviewer: &str, actor: &str) {
    let envelope = Envelope::new(1, RecordKind::Review, Utc::now(), Actor::new(actor, RoleId::parse("reviewer").unwrap()));
    let review = Review::new(
        envelope,
        ProjectId::parse("demo").unwrap(),
        ScenarioId::parse("world.demo.01").unwrap(),
        reviewer,
        format!("pin-{reviewer}-{actor}"),
        ProvenanceRef::OriginalSpecRef("specs/demo.feature".into()),
    );
    ledger(repo).write(&review).unwrap();
}

fn seed_blocker(repo: &Path, fixed_after: Option<i64>) {
    let envelope = Envelope::new(1, RecordKind::Finding, Utc::now(), Actor::new("reviewer-2", RoleId::parse("reviewer").unwrap()));
    let finding = Finding::new(envelope, ChangeId::parse("c-demo").unwrap(), 1, 1, FindingSeverity::Blocker, "reviewer-2", "unbounded subprocess wait");
    let finding = match fixed_after {
        None => finding,
        Some(seconds) => {
            let mut fixed = finding.fixed_by(Sha::parse("c".repeat(40)).unwrap());
            fixed.envelope.at += chrono::Duration::seconds(seconds);
            fixed
        }
    };
    ledger(repo).write(&finding).unwrap();
}

fn current_subject(repo: &Path) -> Value {
    query_subjects(repo, &[])["records"][0].clone()
}

#[test]
fn require_review_refuses_entering_its_scope_without_a_review() {
    let dir = repo();
    write_policy(dir.path(), REQUIRE_REVIEW);
    seed_attested_subject(dir.path());

    let out = run(dir.path(), &["subject", "status", "demo-subject", "verifying"]);
    assert_eq!(out.status.code(), Some(1), "an unreviewed subject must not reach verifying: {}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("ran unreviewed-promotion"), "the guard must say which checks it ran: {err}");
    assert!(err.contains("ran open-blocker"), "{err}");
    assert!(err.contains("unreviewed-promotion world.demo.01 — building → verifying: no review record"), "{err}");
    assert!(err.contains("--override-reason"), "the refusal must name the way through: {err}");
    assert_eq!(current_subject(dir.path())["status"], "building", "a refusal leaves the record unchanged");
}

#[test]
fn require_review_skips_and_says_so_outside_its_scope() {
    let dir = repo();
    write_policy(dir.path(), REQUIRE_REVIEW);
    assert!(run(dir.path(), &["subject", "new", "demo-subject", "--domain", "dev", "--title", "Demo"]).status.success());

    let out = run(dir.path(), &["subject", "status", "demo-subject", "specced"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("skipped unreviewed-promotion — `specced` is not in require_review.scope"), "{err}");
    assert!(err.contains("skipped open-blocker"), "{err}");
}

#[test]
fn a_self_review_does_not_count_under_distinct_actor() {
    let dir = repo();
    write_policy(dir.path(), REQUIRE_REVIEW);
    seed_attested_subject(dir.path());
    // Authored by the evidence actor, whatever reviewer it names.
    seed_review(dir.path(), "someone-else", "impl-agent");

    let out = run(dir.path(), &["subject", "status", "demo-subject", "verifying"]);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("`impl-agent`") && err.contains("distinct_actor"), "the detail must name the rule and the actor: {err}");
    assert_eq!(current_subject(dir.path())["status"], "building");
}

#[test]
fn a_distinct_review_lets_the_subject_through_with_no_waiver() {
    let dir = repo();
    write_policy(dir.path(), REQUIRE_REVIEW);
    seed_attested_subject(dir.path());
    seed_review(dir.path(), "reviewer-2", "reviewer-2");

    let out = run(dir.path(), &["subject", "status", "demo-subject", "verifying"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let subject = current_subject(dir.path());
    assert_eq!(subject["status"], "verifying");
    assert!(subject.get("status_override").is_none(), "nothing was waived: {subject}");

    let gate = run(dir.path(), &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(0), "stdout: {}", stdout(&gate));
}

#[test]
fn an_open_blocker_refuses_and_a_fixed_one_clears() {
    let dir = repo();
    write_policy(dir.path(), REQUIRE_REVIEW);
    seed_attested_subject(dir.path());
    seed_review(dir.path(), "reviewer-2", "reviewer-2");
    seed_blocker(dir.path(), None);

    let out = run(dir.path(), &["subject", "status", "demo-subject", "verifying"]);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains("open-blocker c-demo#1.1"), "stderr: {}", stderr(&out));
    assert_eq!(current_subject(dir.path())["status"], "building");

    // The same natural key, closed later as fixed: the fold reads the
    // latest version, so the blocker no longer counts.
    seed_blocker(dir.path(), Some(5));
    let out = run(dir.path(), &["subject", "status", "demo-subject", "verifying"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(current_subject(dir.path())["status"], "verifying");
}

#[test]
fn gate_check_reports_unreviewed_and_open_blocker_for_an_in_scope_subject() {
    let dir = repo();
    write_policy(dir.path(), REQUIRE_REVIEW);
    seed_attested_subject(dir.path());
    seed_blocker(dir.path(), None);
    // Reached `verifying` with no guard in the way (e.g. before the
    // policy existed): the gate must still see it.
    let envelope = Envelope::new(1, RecordKind::Subject, Utc::now() + chrono::Duration::seconds(2), Actor::new("canon", RoleId::parse("implementer").unwrap()));
    let subject = Subject::new(envelope, SubjectId::parse("demo-subject").unwrap(), "Seeded", "s", "dev", SubjectStatus::Verifying, RoleId::parse("implementer").unwrap())
        .with_change_ids(vec![ChangeId::parse("c-demo").unwrap()]);
    ledger(dir.path()).write(&subject).unwrap();

    let out = run(dir.path(), &["gate", "check"]);
    assert_eq!(out.status.code(), Some(1), "stdout: {}", stdout(&out));
    let text = stdout(&out);
    assert!(text.contains("unreviewed-promotion (1):\n  unreviewed-promotion world.demo.01 — no review record"), "{text}");
    assert!(text.contains("open-blocker (1):\n  open-blocker c-demo#1.1 — open blocker finding by `reviewer-2`"), "{text}");
}

#[test]
fn an_override_records_the_reason_and_gate_check_lists_an_advisory() {
    let dir = repo();
    write_policy(dir.path(), REQUIRE_REVIEW);
    seed_attested_subject(dir.path());

    let out = run(dir.path(), &["subject", "status", "demo-subject", "verifying", "--override-reason", "reviewer out until Monday", "--actor-id", "lead"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("waived unreviewed-promotion world.demo.01"), "{err}");
    assert!(err.contains("override recorded by `lead` for the 1 violation(s) above: reviewer out until Monday"), "{err}");

    let subject = current_subject(dir.path());
    assert_eq!(subject["status"], "verifying");
    assert_eq!(
        subject["status_override"],
        serde_json::json!({
            "to": "verifying",
            "reason": "reviewer out until Monday",
            "waived": [{"class": "unreviewed-promotion", "subject": "world.demo.01"}],
            "actor": {"agent_id": "lead"}
        })
    );

    let gate = run(dir.path(), &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(0), "a waived gap is an advisory, never a violation: {}", stdout(&gate));
    let text = stdout(&gate);
    assert!(text.contains("review waivers: 1 advisory(ies) — not failing the gate:"), "{text}");
    assert!(
        text.contains("  waived unreviewed-promotion world.demo.01 — no review record") && text.contains("[waiver: subject `demo-subject` moved to verifying by `lead`: reviewer out until Monday]"),
        "{text}"
    );

    // The waiver named one scenario. A blocker raised afterwards on the
    // adopted change was never waived, so the gate goes red on it while
    // the recorded gap stays an advisory.
    seed_blocker(dir.path(), None);
    let gate = run(dir.path(), &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(1), "a gap the waiver did not record must block: {}", stdout(&gate));
    let text = stdout(&gate);
    assert!(text.contains("open-blocker (1):\n  open-blocker c-demo#1.1"), "{text}");
    assert!(text.contains("review waivers: 1 advisory(ies)"), "{text}");

    // The waiver belongs to the transition that needed it: the next
    // write (retired is outside the default scope) drops it.
    assert!(run(dir.path(), &["subject", "status", "demo-subject", "retired"]).status.success());
    assert!(current_subject(dir.path()).get("status_override").is_none());
}

#[test]
fn an_override_never_waives_the_ship_gate_and_a_blank_reason_is_refused() {
    let dir = repo();
    write_policy(dir.path(), REQUIRE_REVIEW);
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Verifying, &["world.demo.01"]);

    let out = run(dir.path(), &["subject", "status", "demo-subject", "shipped", "--override-reason", "ship it"]);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("no ledger verdict") && err.contains("waives only the review checks"), "{err}");
    assert_eq!(current_subject(dir.path())["status"], "verifying");

    let out = run(dir.path(), &["subject", "status", "demo-subject", "shipped", "--override-reason", "  "]);
    assert_eq!(out.status.code(), Some(2), "stderr: {}", stderr(&out));
}

/// R1: without `require_review`, reviews and findings change nothing —
/// `subject status` and `gate check` output is byte-identical to a
/// corpus that has none, and no subject gains a `status_override` key.
#[test]
fn an_absent_require_review_is_byte_identical() {
    let outputs = |with_review_records: bool| {
        let dir = repo();
        write_policy(dir.path(), "spec_coverage:\n  require_evidence: true\n");
        seed_attested_subject(dir.path());
        if with_review_records {
            seed_blocker(dir.path(), None);
            seed_review(dir.path(), "impl-agent", "impl-agent");
        }
        let status = run(dir.path(), &["subject", "status", "demo-subject", "verifying"]);
        let gate = run(dir.path(), &["gate", "check"]);
        let subject = current_subject(dir.path());
        (status.status.code(), stdout(&status), stderr(&status), gate.status.code(), stdout(&gate), subject.get("status_override").is_some())
    };
    let plain = outputs(false);
    assert_eq!(plain.0, Some(0));
    assert_eq!(plain.1, "canon subject status: demo-subject → verifying\n");
    assert_eq!(plain.2, "", "no guard output without the policy");
    assert_eq!(plain.4, "canon gate check: clean (0 violations)\n");
    assert!(!plain.5);
    assert_eq!(outputs(true), plain, "review records must not change anything while require_review is absent");
}
