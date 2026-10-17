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
    RoleId, Scenario, ScenarioId, SessionId, Sha, SpecDigest, Subject, SubjectId, SubjectStatus,
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

/// A write command's stdout without its closing `next:` line (0.21 D2).
fn without_next(text: &str) -> String {
    text.lines().filter(|line| !line.starts_with("next: ")).map(|line| format!("{line}\n")).collect()
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

/// The approval line the agent records once the human answers (0.16 D1).
const APPROVED: &str = "Approved: Journey 2026-10-12 — \"승인. 네 가정대로 진행\"";

/// Write `change`'s brief as canon's own
/// `.canon/changes/<change>/brief.md` (0.17 D3), the one file the
/// approval is read from.
fn write_brief(repo: &Path, change: &str, body: &str) {
    let dir = repo.join(".canon/changes").join(change);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("brief.md"), format!("## Why\n\nSeeded brief.\n\n{body}")).unwrap();
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

    // Change side: some version carries the stamped `subject_ids`.
    let cq = run(dir.path(), &["query", "--kind", "change", "--json"]);
    assert!(cq.status.success(), "stderr: {}", stderr(&cq));
    let change_payload: Value = serde_json::from_str(&stdout(&cq)).unwrap();
    let changes = change_payload["records"].as_array().unwrap();
    assert!(
        changes.iter().any(|c| c["subject_ids"] == serde_json::json!(["demo-subject"]) && c["schema"] == 2),
        "an adopted change must carry subject_ids=[demo-subject], written at the current generation: {changes:?}"
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
    assert!(changes["records"].as_array().unwrap().iter().any(|c| c["subject_ids"] == serde_json::json!(["demo-subject"])), "{changes}");
    let again = run(dir.path(), &["subject", "adopt", "s36-demo", "--subject", "demo-subject"]);
    assert!(stdout(&again).contains("nothing written"), "{}", stdout(&again));
}

#[test]
fn the_forward_status_chain_advances_and_folds_to_one_row() {
    let dir = repo();
    assert!(run(dir.path(), &["subject", "new", "demo-subject", "--domain", "dev", "--title", "Demo"]).status.success());

    for state in ["specced", "building", "verifying"] {
        let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", state]);
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
    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("invalid transition"), "stderr: {}", stderr(&out));

    let payload = query_subjects(dir.path(), &[]);
    assert_eq!(payload["records"][0]["status"], "proposed");
}

#[test]
fn shipped_is_blocked_when_a_linked_scenario_has_no_verdict() {
    let dir = repo();
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Verifying, &["world.demo.01"]);

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped"]);
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

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped"]);
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

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped"]);
    assert_eq!(out.status.code(), Some(1), "a divergent latest verdict must fail closed");
    assert!(stderr(&out).contains("divergent"), "stderr: {}", stderr(&out));

    let payload = query_subjects(dir.path(), &[]);
    assert_eq!(payload["records"][0]["status"], "verifying");
}

#[test]
fn shipped_is_allowed_with_a_faithful_verdict_for_every_linked_scenario() {
    let dir = repo();
    seed_subject_with_changes(dir.path(), "demo-subject", SubjectStatus::Verifying, &["world.demo.01"], &["c-demo"]);
    seed_scenario_verdict(dir.path(), "world.demo.01", EvidenceVerdict::Faithful);
    write_brief(dir.path(), "c-demo", &format!("{APPROVED}\n"));

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped"]);
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
    seed_subject_with_changes(dir.path(), "demo-subject", SubjectStatus::Verifying, &["world.demo.01"], &["c-demo"]);
    write_brief(dir.path(), "c-demo", &format!("{APPROVED}\n"));
    seed_scenario_verdict(dir.path(), "world.demo.01", EvidenceVerdict::Faithful);

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped"]);
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

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped"]);
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
    seed_attested_subject_in_session(repo, None);
}

/// [`seed_attested_subject`], with the evidence attested from `session`.
fn seed_attested_subject_in_session(repo: &Path, session: Option<&str>) {
    seed_subject_with_changes(repo, "demo-subject", SubjectStatus::Building, &["world.demo.01"], &["c-demo"]);
    let mut actor = Actor::new("impl-agent", RoleId::parse("implementer").unwrap());
    if let Some(session) = session {
        actor = actor.with_session(SessionId::parse(session).unwrap());
    }
    let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), actor);
    let record = EvidenceRecord::new(envelope, None, Some(ScenarioId::parse("world.demo.01").unwrap()), None, EvidenceVerdict::Faithful)
        .with_project_id(ProjectId::parse("demo").unwrap());
    ledger(repo).write(&record).unwrap();
}

/// `canon review add` for `world.demo.01` by `actor`, from `session`.
fn review_via_cli(repo: &Path, actor: &str, session: Option<&str>) -> Output {
    let pin = format!("pin-{actor}-{}", session.unwrap_or("none"));
    let mut args = vec![
        "review",
        "add",
        "--project-id",
        "demo",
        "--scenario-id",
        "world.demo.01",
        "--reviewer",
        actor,
        "--pin",
        pin.as_str(),
        "--original-spec-ref",
        "specs/demo.feature",
        "--actor-id",
        actor,
        "--role",
        "reviewer",
    ];
    if let Some(session) = session {
        args.extend(["--session-id", session]);
    }
    run(repo, &args)
}

/// A review as `canon review add` writes it: pinned to the scenario and
/// what its evidence says in `repo`'s ledger right now.
fn pinned(repo: &Path, mut review: Review) -> Review {
    let ctx = canon_gate::GateContext::load(canon_gate::GateCtx::from_repo(repo).unwrap(), &canon_policy::SchemaRegistry::load(), Utc::now()).unwrap();
    let scenario = canon_gate::spec_coverage::latest_scenarios(&ctx)
        .into_iter()
        .find(|s| Some(&s.scenario_id) == review.scenario_id.as_ref() && s.project_id == review.project_id)
        .expect("the reviewed scenario is indexed");
    review.pins = Some(canon_gate::freshness::current_pins(&ctx, scenario));
    review
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
    ledger(repo).write(&pinned(repo, review)).unwrap();
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

/// What a fix is followed by (0.22 D2): the suite run again, then a new
/// review of it, both after the blocker was raised and closed.
fn seed_reverification(repo: &Path, scenario: &str) {
    let later = |seconds: i64| Utc::now() + chrono::Duration::seconds(seconds);
    let envelope = Envelope::new(1, RecordKind::EvidenceRecord, later(10), Actor::new("impl-agent", RoleId::parse("implementer").unwrap()));
    let evidence = EvidenceRecord::new(envelope, None, Some(ScenarioId::parse(scenario).unwrap()), None, EvidenceVerdict::Faithful).with_project_id(ProjectId::parse("demo").unwrap());
    ledger(repo).write(&evidence).unwrap();
    let envelope = Envelope::new(1, RecordKind::Review, later(11), Actor::new("reviewer-2", RoleId::parse("reviewer").unwrap()));
    let review = Review::new(
        envelope,
        ProjectId::parse("demo").unwrap(),
        ScenarioId::parse(scenario).unwrap(),
        "reviewer-2",
        format!("pin-re-review-{scenario}"),
        ProvenanceRef::OriginalSpecRef("specs/demo.feature".into()),
    );
    ledger(repo).write(&pinned(repo, review)).unwrap();
}

fn current_subject(repo: &Path) -> Value {
    query_subjects(repo, &[])["records"][0].clone()
}

#[test]
fn require_review_refuses_entering_its_scope_without_a_review() {
    let dir = repo();
    write_policy(dir.path(), REQUIRE_REVIEW);
    seed_attested_subject(dir.path());

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "verifying"]);
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

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "specced"]);
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

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "verifying"]);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("`impl-agent`") && err.contains("distinct_actor"), "the detail must name the rule and the actor: {err}");
    assert_eq!(current_subject(dir.path())["status"], "building");
}

/// D6 (F11): a second actor id is a label the implementer can pick. The
/// session is harder to fake by accident, so a review written from a
/// session that also attested the evidence does not count, and the
/// refusal names the session rule.
#[test]
fn a_review_from_the_evidence_session_does_not_count_under_distinct_actor() {
    let dir = repo();
    write_policy(dir.path(), REQUIRE_REVIEW);
    seed_attested_subject_in_session(dir.path(), Some("sess-impl"));

    let out = review_via_cli(dir.path(), "reviewer-2", Some("sess-impl"));
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("— written directly; nothing to promote"), "review add must say it committed directly: {}", stdout(&out));
    let review: Value = serde_json::from_slice(&std::fs::read(review_files(dir.path())[0].clone()).unwrap()).unwrap();
    assert_eq!(review["actor"]["session_id"], "sess-impl", "--session-id fills actor.session_id: {review}");

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "verifying"]);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    let err = stderr(&out);
    assert!(
        err.contains("every owned scenario needs a signed review, current with the spec and evidence it pinned, by an actor, and from a session, other than its evidence's"),
        "{err}"
    );
    assert!(err.contains("(`sess-impl`)") && err.contains("in a session other than any evidence session"), "the detail must name the session rule: {err}");
    assert_eq!(current_subject(dir.path())["status"], "building");

    // The same reviewer from its own session counts.
    assert!(review_via_cli(dir.path(), "reviewer-2", Some("sess-review")).status.success());
    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "verifying"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
}

/// Records written before sessions existed keep counting by actor: a
/// session on one side only never matches.
#[test]
fn a_session_on_one_side_only_leaves_the_actor_rule_in_charge() {
    for (evidence_session, review_session) in [(Some("sess-impl"), None), (None, Some("sess-impl"))] {
        let dir = repo();
        write_policy(dir.path(), REQUIRE_REVIEW);
        seed_attested_subject_in_session(dir.path(), evidence_session);
        assert!(review_via_cli(dir.path(), "reviewer-2", review_session).status.success());

        let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "verifying"]);
        assert!(out.status.success(), "evidence {evidence_session:?}, review {review_session:?}: {}", stderr(&out));
    }
}

/// D6 (F12): the author records a defect it found in its own work. No
/// guard treats that finding as illegitimate, and an open blocker the
/// author raised blocks exactly like one a reviewer raised.
#[test]
fn a_blocker_the_author_found_in_its_own_work_is_accepted_and_still_blocks() {
    let dir = repo();
    write_policy(dir.path(), REQUIRE_REVIEW);
    seed_attested_subject(dir.path());
    seed_review(dir.path(), "reviewer-2", "reviewer-2");

    let out = run(
        dir.path(),
        &[
            "finding",
            "add",
            "--change-id",
            "c-demo",
            "--round",
            "1",
            "--seq",
            "1",
            "--severity",
            "blocker",
            "--reviewer",
            "impl-agent",
            "--summary",
            "restart leaves the old timer running",
            "--actor-id",
            "impl-agent",
            "--role",
            "implementer",
            "--session-id",
            "sess-impl",
        ],
    );
    assert!(out.status.success(), "an author-raised finding must be accepted: {}", stderr(&out));
    assert!(stdout(&out).contains("— run `canon gate promote` to commit it"), "{}", stdout(&out));
    let out = run(dir.path(), &["gate", "promote"]);
    assert!(out.status.success(), "the author's finding must promote: {}{}", stdout(&out), stderr(&out));

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "verifying"]);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains("open-blocker c-demo#1.1 — building → verifying: open blocker finding by `impl-agent`"), "{}", stderr(&out));
    assert_eq!(current_subject(dir.path())["status"], "building");

    let gate = run(dir.path(), &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(0), "the subject never reached review scope, so the gate has nothing to report: {}", stdout(&gate));
}

fn review_files(repo: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![repo.join(".canon/ledger/kind=review")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files
}

#[test]
fn a_distinct_review_lets_the_subject_through_with_no_waiver() {
    let dir = repo();
    write_policy(dir.path(), REQUIRE_REVIEW);
    seed_attested_subject(dir.path());
    seed_review(dir.path(), "reviewer-2", "reviewer-2");

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "verifying"]);
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

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "verifying"]);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains("open-blocker c-demo#1.1"), "stderr: {}", stderr(&out));
    assert_eq!(current_subject(dir.path())["status"], "building");

    // The same natural key, closed later as fixed: the fold reads the
    // latest version, so the blocker no longer counts as open, but it
    // keeps blocking until the suite ran again (0.22 D2).
    seed_blocker(dir.path(), Some(5));
    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "verifying"]);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    seed_reverification(dir.path(), "world.demo.01");
    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "verifying"]);
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

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "verifying", "--override-reason", "reviewer out until Monday", "--actor-id", "lead"]);
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
    assert!(run(dir.path(), &["subject", "status", "demo-subject", "--to", "retired"]).status.success());
    assert!(current_subject(dir.path()).get("status_override").is_none());
}

#[test]
fn an_override_never_waives_the_ship_gate_and_a_blank_reason_is_refused() {
    let dir = repo();
    write_policy(dir.path(), REQUIRE_REVIEW);
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Verifying, &["world.demo.01"]);

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped", "--override-reason", "ship it"]);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("no ledger verdict") && err.contains("waives only the review checks"), "{err}");
    assert_eq!(current_subject(dir.path())["status"], "verifying");

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped", "--override-reason", "  "]);
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
        assert!(run(dir.path(), &["policy", "approve", "--by", "alice"]).status.success());
        seed_attested_subject(dir.path());
        if with_review_records {
            seed_blocker(dir.path(), None);
            seed_review(dir.path(), "impl-agent", "impl-agent");
        }
        let status = run(dir.path(), &["subject", "status", "demo-subject", "--to", "verifying"]);
        let gate = run(dir.path(), &["gate", "check"]);
        let subject = current_subject(dir.path());
        (status.status.code(), without_next(&stdout(&status)), stderr(&status), gate.status.code(), stdout(&gate), subject.get("status_override").is_some())
    };
    let plain = outputs(false);
    assert_eq!(plain.0, Some(0));
    assert_eq!(plain.1, "canon subject status: demo-subject → verifying — written directly; nothing to promote\n");
    assert_eq!(plain.2, "", "no guard output without the policy");
    assert_eq!(plain.4, "canon gate check: clean (0 violations)\n");
    assert!(!plain.5);
    assert_eq!(outputs(true), plain, "review records must not change anything while require_review is absent");
}

/// review.add.21: a review of a scenario no record carries is refused
/// with exit 2, the refusal names `canon inventory sync`, and nothing is
/// written.
#[test]
fn review_add_refuses_an_unsynced_scenario_and_names_inventory_sync() {
    let dir = repo();
    let out = run(
        dir.path(),
        &[
            "review", "add", "--project-id", "demo", "--scenario-id", "world.demo.99", "--reviewer", "reviewer-2", "--pin", "p1", "--original-spec-ref",
            "specs/demo.feature", "--actor-id", "reviewer-2", "--role", "reviewer",
        ],
    );
    assert_eq!(out.status.code(), Some(2), "stderr: {}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("REVIEW_SCENARIO_NOT_INDEXED — no scenario record for `world.demo.99` in spec root `demo`"), "{err}");
    assert!(err.contains("(`canon inventory sync`)"), "the refusal must name the repair: {err}");
    assert!(!dir.path().join(".canon/ledger/kind=review").exists(), "a refused review add writes nothing");
}

/// review.add.24: an `unavailable` reviewer run is recorded but is a gap,
/// end to end: `canon gate check` reports it (exit 1), the status guard
/// refuses the move (exit 1), both naming the reviewer and the outcome.
#[test]
fn an_unavailable_review_is_a_gap_at_the_gate_and_the_status_guard() {
    let dir = repo();
    write_policy(dir.path(), "spec_coverage:\n  require_evidence: true\n  require_review:\n    scope: []\n");
    seed_attested_subject(dir.path());
    let out = run(
        dir.path(),
        &[
            "review", "add", "--project-id", "demo", "--scenario-id", "world.demo.01", "--reviewer", "reviewer-2", "--pin", "p1", "--original-spec-ref",
            "specs/demo.feature", "--actor-id", "reviewer-2", "--role", "reviewer", "--session-id", "sess-review", "--outcome", "unavailable",
        ],
    );
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("outcome `unavailable` is recorded as a review gap, never as an approval"), "{}", stdout(&out));

    let detail = "unreviewed-promotion world.demo.01 — no signed review: the latest review recorded no sign-off (`reviewer-2` unavailable)";
    let gate = run(dir.path(), &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(1), "{}", stdout(&gate));
    assert!(stdout(&gate).contains(detail), "{}", stdout(&gate));

    let guard = run(dir.path(), &["subject", "status", "demo-subject", "--to", "verifying"]);
    assert_eq!(guard.status.code(), Some(1), "stderr: {}", stderr(&guard));
    assert!(stderr(&guard).contains("no signed review: the latest review recorded no sign-off (`reviewer-2` unavailable)"), "{}", stderr(&guard));
    assert_eq!(current_subject(dir.path())["status"], "building", "an unavailable review approves nothing");
}

/// A review of `world.demo.01` as written before 0.23 pinning (planted by
/// hand, no `pins` or `outcome` keys).
fn plant_unpinned_review(repo: &Path) {
    let legacy = serde_json::json!({
        "schema": 1,
        "kind": "review",
        "at": "2026-10-01T00:00:00Z",
        "actor": {"agent_id": "reviewer-2", "role": "reviewer"},
        "project_id": "demo",
        "scenario_id": "world.demo.01",
        "reviewer": "reviewer-2",
        "pin": "r1",
        "provenance_ref": {"original_spec_ref": "specs/demo.feature"},
    });
    let relative = canon_store::partition::expected_relative_path(RecordKind::Review, &legacy).unwrap();
    let path = repo.join(".canon/ledger").join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, serde_json::to_vec_pretty(&legacy).unwrap()).unwrap();
}

const UNMIGRATED_ADVISORY: &str = "1 scenario(s) have a review that predates 0.23 review pinning (world.demo.01); such a review does not count until `canon migrate` rewrites it to what the evidence says";

/// gate.check.40 (0.23): a review written before 0.23 pinning (planted by
/// hand, no `pins` or `outcome` keys) does not count: the ship guard and
/// `canon gate check` refuse naming `canon migrate`, and `canon gate check`,
/// `canon subject status` and `canon status` each print the one advisory
/// line counting the scenarios whose review waits for it.
#[test]
fn an_unmigrated_review_does_not_count_and_every_surface_names_migrate() {
    let dir = repo();
    write_policy(dir.path(), REQUIRE_REVIEW);
    seed_attested_subject(dir.path());
    plant_unpinned_review(dir.path());

    let advisory = UNMIGRATED_ADVISORY;
    let guard = run(dir.path(), &["subject", "status", "demo-subject", "--to", "verifying"]);
    assert_eq!(guard.status.code(), Some(1), "the unpinned review does not count: {}", stderr(&guard));
    assert!(stderr(&guard).contains(&format!("canon subject status: advisory: {advisory}\n")), "{}", stderr(&guard));
    assert!(stderr(&guard).contains("predates 0.23 review pinning") && stderr(&guard).contains("`canon migrate`"), "{}", stderr(&guard));
    assert_eq!(current_subject(dir.path())["status"], "building");

    let gate = run(dir.path(), &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(0), "a building subject is not yet in review scope: {}", stdout(&gate));
    assert!(stdout(&gate).contains(&format!("advisory: {advisory}\n")), "{}", stdout(&gate));

    let status = run(dir.path(), &["status"]);
    assert_eq!(status.status.code(), Some(0));
    assert!(stdout(&status).contains(&format!("warning: {advisory}\n")), "{}", stdout(&status));
}

/// review.add.61 (0.23): the status guard judges a review that predates
/// 0.23 pinning as the gate does: it refuses with the gate's detail naming
/// `canon migrate` and prints the advisory, `canon status` counts the
/// scenario as a stale review, and once the scenario is reviewed again the
/// same move goes through.
#[test]
fn review_add_61_the_guard_and_status_judge_a_review_that_predates_pinning_as_the_gate_does() {
    let dir = repo();
    write_policy(dir.path(), REQUIRE_REVIEW);
    seed_attested_subject(dir.path());
    plant_unpinned_review(dir.path());

    let guard = run(dir.path(), &["subject", "status", "demo-subject", "--to", "verifying"]);
    assert_eq!(guard.status.code(), Some(1), "{}", stderr(&guard));
    let err = stderr(&guard);
    assert!(err.contains("unreviewed-promotion world.demo.01 — building → verifying:") && err.contains("predates 0.23 review pinning") && err.contains("`canon migrate`"), "{err}");
    assert!(err.contains(&format!("canon subject status: advisory: {UNMIGRATED_ADVISORY}\n")), "{err}");
    assert_eq!(current_subject(dir.path())["status"], "building");

    let report: Value = serde_json::from_str(&stdout(&run(dir.path(), &["status", "--json"]))).unwrap();
    let subject = report["subjects"].as_array().unwrap().iter().find(|s| s["id"] == "demo-subject").unwrap().clone();
    assert_eq!((subject["reviewed"].as_u64(), subject["staleReviews"].as_u64()), (Some(0), Some(1)), "counted as a stale review: {subject}");
    let text = stdout(&run(dir.path(), &["status"]));
    assert!(text.contains("1 stale review(s)"), "{text}");

    let review = review_via_cli(dir.path(), "reviewer-2", Some("sess-review"));
    assert!(review.status.success(), "{}", stderr(&review));
    let moved = run(dir.path(), &["subject", "status", "demo-subject", "--to", "verifying"]);
    assert!(moved.status.success(), "reviewed again, the move goes through: {}", stderr(&moved));
    assert_eq!(current_subject(dir.path())["status"], "verifying");
}

// ── the ship guard needs the approved brief (0.16 D1) ──

/// A `verifying` subject past every technical gate: `world.demo.01`
/// evidenced by `impl-agent`, reviewed by `reviewer-2`, no blocker, the
/// review guard satisfied, and adopted change `c-demo` holding `brief`
/// as its brief.md.
fn seed_technically_ship_ready(repo: &Path, brief: &str) {
    write_policy(repo, REQUIRE_REVIEW);
    seed_attested_subject(repo);
    seed_review(repo, "reviewer-2", "reviewer-2");
    let out = run(repo, &["subject", "status", "demo-subject", "--to", "verifying"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    write_brief(repo, "c-demo", brief);
}

/// The brief as `canon status --json` reports it for `demo-subject`.
fn status_brief(repo: &Path) -> Value {
    let out = run(repo, &["status", "--json"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let report: Value = serde_json::from_str(&stdout(&out)).unwrap();
    report["subjects"].as_array().unwrap().iter().find(|s| s["id"] == "demo-subject").unwrap()["brief"].clone()
}

const BRIEF_REFUSAL: &str = "canon subject status: SUBJECT_GATE_VIOLATION — unreviewed-promotion demo-subject — verifying → shipped: brief not approved — .canon/changes/c-demo/brief.md has no `Approved:` line; a human must answer the brief, and only from that answer the agent will record their words with `canon brief approve --change c-demo --by <the human> --note \"<their words>\"`\n";

/// subject.lifecycle.24: with every technical gate passing, a brief
/// with no `Approved:` line refuses `verifying → shipped`, naming the
/// brief.md and saying a human must answer; status reads the same
/// brief as not approved; recording the line lets the same move ship.
#[test]
fn subject_lifecycle_24_shipping_is_refused_while_the_brief_has_no_approved_line() {
    let dir = repo();
    seed_technically_ship_ready(dir.path(), "Status: draft, awaiting the human.\n");
    let gate = run(dir.path(), &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(0), "every technical gate passes: {}", stdout(&gate));

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped"]);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains(BRIEF_REFUSAL), "{err}");
    assert!(
        err.contains("hint: get the human's answer to the brief and record it with `canon brief approve`, or pass --override-reason <text> to move anyway; the waiver is recorded on the subject\n"),
        "{err}"
    );
    assert!(!err.contains("uncovered-cell") && !err.contains("world.demo.01"), "only the brief refuses: {err}");
    assert_eq!(current_subject(dir.path())["status"], "verifying", "a refusal leaves the record unchanged");
    assert_eq!(
        status_brief(dir.path()),
        serde_json::json!({"approved": false, "enforced": true, "change": "c-demo", "brief": ".canon/changes/c-demo/brief.md", "briefExists": true})
    );

    write_brief(dir.path(), "c-demo", &format!("Status: approved.\n\n{APPROVED}\n"));
    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let subject = current_subject(dir.path());
    assert_eq!(subject["status"], "shipped");
    assert!(subject.get("status_override").is_none(), "nothing was waived: {subject}");
}

/// subject.lifecycle.25: an `Approved:` line that sits only inside an
/// HTML comment or a fenced code block does not approve; the guard
/// refuses naming the brief.md, and status agrees.
#[test]
fn subject_lifecycle_25_an_approved_line_in_a_comment_or_fence_does_not_ship() {
    for (case, body) in [
        ("comment", format!("<!-- draft:\n{APPROVED}\n-->\n")),
        ("comment-one-line", format!("<!-- {APPROVED} -->\n")),
        ("fence-backtick", format!("```\n{APPROVED}\n```\n")),
        ("fence-tilde", format!("~~~md\n{APPROVED}\n~~~\n")),
    ] {
        let dir = repo();
        seed_technically_ship_ready(dir.path(), &body);
        let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped"]);
        assert_eq!(out.status.code(), Some(1), "{case}: stderr: {}", stderr(&out));
        assert!(stderr(&out).contains(BRIEF_REFUSAL), "{case}: {}", stderr(&out));
        assert_eq!(current_subject(dir.path())["status"], "verifying", "{case}");
        assert_eq!(status_brief(dir.path())["approved"], false, "{case}: status and the guard agree");
    }
}

/// subject.lifecycle.26: `--override-reason` ships a subject whose brief
/// is not approved, names the waived brief check, and records the
/// reason, the actor and the waived `unreviewed-promotion` naming the
/// subject on the new subject record.
#[test]
fn subject_lifecycle_26_an_override_ships_an_unapproved_brief_and_records_the_waiver() {
    let dir = repo();
    seed_technically_ship_ready(dir.path(), "Status: draft, awaiting the human.\n");

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped", "--override-reason", "the human approved in chat; line to follow", "--actor-id", "lead"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains(&BRIEF_REFUSAL.replace("canon subject status: SUBJECT_GATE_VIOLATION — unreviewed-promotion", "canon subject status: waived unreviewed-promotion")), "{err}");
    assert!(err.contains("override recorded by `lead` for the 1 violation(s) above: the human approved in chat; line to follow"), "{err}");

    let subject = current_subject(dir.path());
    assert_eq!(subject["status"], "shipped");
    assert_eq!(
        subject["status_override"],
        serde_json::json!({
            "to": "shipped",
            "reason": "the human approved in chat; line to follow",
            "waived": [{"class": "unreviewed-promotion", "subject": "demo-subject"}],
            "actor": {"agent_id": "lead"}
        })
    );
}

/// subject.lifecycle.27: a subject that adopts no change has no brief a
/// human could answer, so `verifying → shipped` passes its gates with an
/// advisory that the brief is not approved and no refusal; status reads
/// the same brief as not approved and not enforced, with no human step.
#[test]
fn subject_lifecycle_27_a_subject_with_no_adopted_change_ships_with_an_advisory() {
    let dir = repo();
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Verifying, &["world.demo.01"]);
    seed_scenario_verdict(dir.path(), "world.demo.01", EvidenceVerdict::Faithful);
    assert_eq!(status_brief(dir.path()), serde_json::json!({"approved": false, "enforced": false, "briefExists": false}));
    let status = run(dir.path(), &["status"]);
    assert!(!stdout(&status).contains("human:"), "nothing to answer, so no human step: {}", stdout(&status));
    assert!(stdout(&status).contains("      brief: not approved (no adopted change; advisory, nothing to approve yet)\n"), "{}", stdout(&status));

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(
        stderr(&out),
        "canon subject status: advisory: demo-subject's brief is not approved — demo-subject adopts no change, so there is no brief to answer and shipping is not refused\n"
    );
    let subject = current_subject(dir.path());
    assert_eq!(subject["status"], "shipped");
    assert!(subject.get("status_override").is_none(), "nothing was waived: {subject}");
}

// ── nothing is attested under an unapproved brief (0.16 M7) ──

/// Every evidence record file under `.canon/`, staged or committed.
fn evidence_files(repo: &Path) -> Vec<std::path::PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.to_string_lossy().contains("evidence_record") {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&repo.join(".canon"), &mut out);
    out.sort();
    out
}

/// A `building` subject owning `world.demo.01` and adopting `c-demo`,
/// whose brief.md is `brief` and whose tasks.md holds task `1.1`, both
/// in canon's own `.canon/changes/c-demo/`, read with no `plans:` entry.
fn seed_briefed_subject(repo: &Path, brief: &str) {
    seed_subject_with_changes(repo, "demo-subject", SubjectStatus::Building, &["world.demo.01"], &["c-demo"]);
    write_brief(repo, "c-demo", brief);
    std::fs::write(repo.join(".canon/changes/c-demo/tasks.md"), "## 1. Build\n\n- [ ] 1.1 Build the demo\n").unwrap();
}

/// `canon evidence add` for `world.demo.01` (and `extra`), promoted.
fn attest_scenario(repo: &Path, extra: &[&str]) -> Output {
    let mut args = vec![
        "evidence", "add", "--project-id", "demo", "--scenario-id", "world.demo.01", "--kind", "test-run", "--ref", "cargo test", "--role", "implementer",
        "--actor-id", "impl-agent", "--promote",
    ];
    args.extend_from_slice(extra);
    run(repo, &args)
}

/// `canon evidence add` for task `c-demo#1.1` (and `extra`), promoted.
fn attest_task(repo: &Path, extra: &[&str]) -> Output {
    let mut args = vec!["evidence", "add", "--task", "c-demo#1.1", "--kind", "test-run", "--ref", "cargo test", "--role", "implementer", "--actor-id", "impl-agent", "--promote"];
    args.extend_from_slice(extra);
    run(repo, &args)
}

const EVIDENCE_REFUSAL: &str = "hint: stop, report the brief to the human and wait for their answer: nothing is built or attested for a subject before its brief is approved. Record their answer with `canon brief approve --by <the human> --note \"<their words>\"` only from that answer; to attest anyway, pass --override-reason <text>, which is recorded on the evidence record\n";

/// The violation line `canon evidence add` prints for `demo-subject`'s
/// unapproved brief, attesting `what`.
fn evidence_brief_line(what: &str) -> String {
    format!("canon evidence add: BRIEF_UNAPPROVED — unreviewed-promotion demo-subject — evidence for {what}: brief not approved — .canon/changes/c-demo/brief.md has no `Approved:` line, so the human has not answered it; once they do, record their words with `canon brief approve --change c-demo --by <the human> --note \"<their words>\"`\n")
}

/// evidence.add.68: while the subject's brief is enforced and not
/// approved, `canon evidence add` refuses with exit 1 and stages nothing,
/// for the scenario (through its `@subject` owner) and for the task
/// (through the subject adopting its change), naming the brief.md and
/// saying to stop, report the brief and wait for the human's answer.
/// `review add` and `finding add` are not refused.
#[test]
fn evidence_add_68_an_unapproved_brief_refuses_the_attestation_and_stages_nothing() {
    let dir = repo();
    seed_briefed_subject(dir.path(), "Status: draft, awaiting the human.\n");
    let before = evidence_files(dir.path());

    let out = attest_scenario(dir.path(), &[]);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert_eq!(stderr(&out), format!("{}{EVIDENCE_REFUSAL}", evidence_brief_line("world.demo.01")));
    assert_eq!(evidence_files(dir.path()), before, "a refused add stages and commits nothing");

    let out = attest_task(dir.path(), &[]);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert_eq!(stderr(&out), format!("{}{EVIDENCE_REFUSAL}", evidence_brief_line("c-demo#1.1")));
    assert_eq!(evidence_files(dir.path()), before, "a refused add stages and commits nothing");

    let review = review_via_cli(dir.path(), "reviewer-2", None);
    assert!(review.status.success(), "review add is not refused: {}", stderr(&review));
    let finding = run(
        dir.path(),
        &["finding", "add", "--change-id", "c-demo", "--round", "1", "--seq", "1", "--severity", "note", "--reviewer", "reviewer-2", "--summary", "brief pending", "--promote"],
    );
    assert!(finding.status.success(), "finding add is not refused: {}", stderr(&finding));
}

/// evidence.add.69: a subject with no brief to answer (no adopted change,
/// or a newest change with no brief) and a scenario no subject
/// owns are attested as before: no refusal, no override, no warning.
#[test]
fn evidence_add_69_no_refusal_when_the_brief_is_not_enforced() {
    for case in ["no-change", "no-brief", "unowned"] {
        let dir = repo();
        match case {
            "no-change" => seed_subject(dir.path(), "demo-subject", SubjectStatus::Building, &["world.demo.01"]),
            "no-brief" => seed_subject_with_changes(dir.path(), "demo-subject", SubjectStatus::Building, &["world.demo.01"], &["c-demo"]),
            _ => {
                seed_subject_with_changes(dir.path(), "demo-subject", SubjectStatus::Building, &[], &["c-demo"]);
                write_brief(dir.path(), "c-demo", "Status: draft, awaiting the human.\n");
                let envelope = Envelope::new(1, RecordKind::Scenario, Utc::now(), Actor::new("canon", RoleId::parse("implementer").unwrap()));
                let scenario = Scenario::new(envelope, ProjectId::parse("demo").unwrap(), ScenarioId::parse("world.demo.01").unwrap(), "seeded", "", SpecDigest::of(b"x"));
                ledger(dir.path()).write(&scenario).unwrap();
            }
        }
        let out = attest_scenario(dir.path(), &[]);
        assert!(out.status.success(), "{case}: stderr: {}", stderr(&out));
        assert!(!stderr(&out).contains("brief"), "{case}: {}", stderr(&out));
        let records: Vec<Value> = evidence_files(dir.path()).iter().map(|p| serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()).collect();
        assert_eq!(records.len(), 1, "{case}");
        assert!(records[0].get("brief_override").is_none(), "{case}: {}", records[0]);
        assert!(!stdout(&run(dir.path(), &["status"])).contains("was attested by"), "{case}");
    }
}

/// evidence.add.70: `--override-reason` attests anyway, prints the waived
/// refusal and who recorded the override, records the reason, the waived
/// `unreviewed-promotion` naming the subject and the actor on the record,
/// and `canon status` and `canon gate check` list it. A blank reason is
/// refused (exit 2) and an override nothing needed is not recorded.
#[test]
fn evidence_add_70_an_override_attests_anyway_and_records_the_waiver() {
    let dir = repo();
    seed_briefed_subject(dir.path(), "Status: draft, awaiting the human.\n");

    let out = attest_scenario(dir.path(), &["--override-reason", " "]);
    assert_eq!(out.status.code(), Some(2), "stderr: {}", stderr(&out));
    assert!(evidence_files(dir.path()).is_empty());

    let out = attest_scenario(dir.path(), &["--override-reason", "the human approved in chat; line to follow"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains(&evidence_brief_line("world.demo.01").replace("canon evidence add: BRIEF_UNAPPROVED — unreviewed-promotion", "canon evidence add: waived unreviewed-promotion")), "{err}");
    assert!(err.contains("canon evidence add: override recorded by `impl-agent` for the 1 brief refusal(s) above: the human approved in chat; line to follow\n"), "{err}");
    let files = evidence_files(dir.path());
    assert_eq!(files.len(), 1);
    let record: Value = serde_json::from_slice(&std::fs::read(&files[0]).unwrap()).unwrap();
    assert_eq!(
        record["brief_override"],
        serde_json::json!({
            "reason": "the human approved in chat; line to follow",
            "waived": [{"class": "unreviewed-promotion", "subject": "demo-subject"}],
            "actor": {"agent_id": "impl-agent"}
        })
    );

    let listed = "evidence for world.demo.01 was attested by `impl-agent` at ";
    let status = stdout(&run(dir.path(), &["status"]));
    assert!(status.contains(&format!("warning: {listed}")) && status.contains("while the brief of demo-subject was not approved (override: the human approved in chat; line to follow)"), "{status}");
    let gate = stdout(&run(dir.path(), &["gate", "check"]));
    assert!(gate.contains("brief overrides: 1 evidence record(s) attested before their brief was approved — not failing the gate:\n") && gate.contains(&format!("  {listed}")), "{gate}");

    write_brief(dir.path(), "c-demo", &format!("{APPROVED}\n"));
    let out = attest_scenario(dir.path(), &["--override-reason", "habit"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains("canon evidence add: --override-reason not recorded — no unapproved brief refused this attestation\n"), "{}", stderr(&out));
    let latest = evidence_files(dir.path()).into_iter().map(|p| serde_json::from_slice::<Value>(&std::fs::read(p).unwrap()).unwrap()).find(|r| r.get("brief_override").is_none());
    assert!(latest.is_some(), "the unneeded override is not recorded");
}

/// evidence.add.71: once the human's answer is recorded as the brief's
/// `Approved:` line, the same scenario and task attestations that were
/// refused go through, with no override.
#[test]
fn evidence_add_71_the_approval_lifts_the_refusal() {
    let dir = repo();
    seed_briefed_subject(dir.path(), "Status: draft, awaiting the human.\n");
    assert_eq!(attest_scenario(dir.path(), &[]).status.code(), Some(1));
    assert_eq!(attest_task(dir.path(), &[]).status.code(), Some(1));

    write_brief(dir.path(), "c-demo", &format!("Status: approved.\n\n{APPROVED}\n"));
    for out in [attest_scenario(dir.path(), &[]), attest_task(dir.path(), &[])] {
        assert!(out.status.success(), "stderr: {}", stderr(&out));
        assert!(!stderr(&out).contains("brief"), "{}", stderr(&out));
    }
    let records: Vec<Value> = evidence_files(dir.path()).iter().map(|p| serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()).collect();
    assert_eq!(records.len(), 2);
    assert!(records.iter().all(|r| r.get("brief_override").is_none()), "{records:?}");
}

// ── a change from an external plan is answered in a canon brief (0.17 D3) ──

/// The external openspec proposal of `change`, holding a 0.16-style
/// `Approved:` line that is never read, under an explicit openspec plan
/// source at the repo root.
fn write_external_proposal(repo: &Path, change: &str) -> std::path::PathBuf {
    std::fs::write(repo.join("canon.yaml"), format!("{CANON_YAML}plans:\n  sources:\n    - dialect: openspec\n      root: .\n")).unwrap();
    let dir = repo.join("openspec/changes").join(change);
    std::fs::create_dir_all(&dir).unwrap();
    let proposal = dir.join("proposal.md");
    std::fs::write(&proposal, format!("## Why\n\nA human's plan.\n\n{APPROVED}\n")).unwrap();
    proposal
}

/// The canon brief a change from an external plan is answered in: a
/// `Source:` pointer to the plan and, from the human's answer, the
/// approval.
fn write_pointer_brief(repo: &Path, change: &str) {
    write_brief(repo, change, &format!("Source: openspec/changes/{change}/proposal.md\n\n{APPROVED}\n"));
}

/// What the refusals ask for when `c-ext`'s canon brief does not exist.
const EXTERNAL_UNANSWERED: &str = ".canon/changes/c-ext/brief.md does not exist yet: change `c-ext` comes from the external plan openspec/changes/c-ext/proposal.md, which canon reads and never edits, so its approval lives in that canon file";
const EXTERNAL_RECORD: &str = "create .canon/changes/c-ext/brief.md holding a `Source: openspec/changes/c-ext/proposal.md` line, then record their words with `canon brief approve --change c-ext --by <the human> --note \"<their words>\"`";

/// evidence.add.72: a subject adopting a change from an external openspec
/// plan with no canon brief has a brief to answer. `canon evidence add`
/// refuses its scenario and its task, naming the canon `brief.md` to
/// create with its `Source:` pointer; the `Approved:` line inside the
/// external proposal is not read. Once the canon brief records the
/// approval both attest, the external proposal is never edited, and
/// `canon gate task` still flips the external task row (0.17 D4).
#[test]
fn evidence_add_72_an_external_change_is_answered_in_a_canon_brief_and_its_rows_still_flip() {
    let dir = repo();
    seed_subject_with_changes(dir.path(), "demo-subject", SubjectStatus::Building, &["world.demo.01"], &["c-ext"]);
    let proposal = write_external_proposal(dir.path(), "c-ext");
    let tasks = dir.path().join("openspec/changes/c-ext/tasks.md");
    std::fs::write(&tasks, "## 1. Build\n\n- [ ] 1.1 Build the demo\n").unwrap();
    let proposal_before = std::fs::read(&proposal).unwrap();
    let attest_ext_task = |repo: &Path| {
        run(repo, &["evidence", "add", "--task", "c-ext#1.1", "--kind", "test-run", "--ref", "cargo test", "--role", "implementer", "--actor-id", "impl-agent", "--promote"])
    };

    for (what, out) in [("world.demo.01", attest_scenario(dir.path(), &[])), ("c-ext#1.1", attest_ext_task(dir.path()))] {
        assert_eq!(out.status.code(), Some(1), "{what}: stderr: {}", stderr(&out));
        assert_eq!(
            stderr(&out),
            format!(
                "canon evidence add: BRIEF_UNAPPROVED — unreviewed-promotion demo-subject — evidence for {what}: brief not approved — {EXTERNAL_UNANSWERED}, so the human has not answered it; once they do, {EXTERNAL_RECORD}\n{EVIDENCE_REFUSAL}"
            )
        );
    }
    assert!(evidence_files(dir.path()).is_empty(), "a refused add stages and commits nothing");

    write_pointer_brief(dir.path(), "c-ext");
    for out in [attest_scenario(dir.path(), &[]), attest_ext_task(dir.path())] {
        assert!(out.status.success(), "stderr: {}", stderr(&out));
        assert!(!stderr(&out).contains("brief"), "{}", stderr(&out));
    }
    assert_eq!(std::fs::read(&proposal).unwrap(), proposal_before, "canon never edits the external plan's proposal");

    let flip = run(dir.path(), &["gate", "task", "c-ext#1.1"]);
    assert!(flip.status.success(), "{}{}", stdout(&flip), stderr(&flip));
    assert!(std::fs::read_to_string(&tasks).unwrap().contains("- [x] 1.1 Build the demo"), "the external task row flips");
}

/// subject.lifecycle.28: the ship guard refuses a subject whose newest
/// change comes from an external openspec plan with no canon brief,
/// naming the canon `brief.md` to create with its `Source:` pointer;
/// status agrees. Once the canon brief records the approval the same
/// move ships, and the external proposal is never edited.
#[test]
fn subject_lifecycle_28_shipping_an_external_change_needs_its_canon_brief() {
    let dir = repo();
    seed_technically_ship_ready(dir.path(), "unused\n");
    std::fs::remove_dir_all(dir.path().join(".canon/changes")).unwrap();
    // The subject's newest change is the external one.
    seed_subject_with_changes(dir.path(), "demo-subject", SubjectStatus::Verifying, &[], &["c-demo", "c-ext"]);
    let proposal = write_external_proposal(dir.path(), "c-ext");
    let proposal_before = std::fs::read(&proposal).unwrap();

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped"]);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert!(
        stderr(&out).contains(&format!(
            "canon subject status: SUBJECT_GATE_VIOLATION — unreviewed-promotion demo-subject — verifying → shipped: brief not approved — {EXTERNAL_UNANSWERED}; a human must answer the brief, and only from that answer the agent will {EXTERNAL_RECORD}\n"
        )),
        "{}",
        stderr(&out)
    );
    assert_eq!(current_subject(dir.path())["status"], "verifying");
    assert_eq!(
        status_brief(dir.path()),
        serde_json::json!({"approved": false, "enforced": true, "change": "c-ext", "brief": ".canon/changes/c-ext/brief.md", "briefExists": false, "source": "openspec/changes/c-ext/proposal.md"})
    );

    write_pointer_brief(dir.path(), "c-ext");
    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(current_subject(dir.path())["status"], "shipped");
    assert_eq!(std::fs::read(&proposal).unwrap(), proposal_before, "canon never edits the external plan's proposal");
}

// ── reopening a shipped subject (0.18 D3) ──

/// [`seed_technically_ship_ready`] with an approved brief, shipped.
fn seed_shipped(repo: &Path) {
    seed_technically_ship_ready(repo, &format!("Status: approved.\n\n{APPROVED}\n"));
    let out = run(repo, &["subject", "status", "demo-subject", "--to", "shipped"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
}

const REOPEN_REASON: &str = "follow-up: the retry path leaks a file handle";

fn reopen(repo: &Path, extra: &[&str]) -> Output {
    let mut args = vec!["subject", "status", "demo-subject", "--to", "verifying", "--reason", REOPEN_REASON, "--actor-id", "impl-agent"];
    args.extend_from_slice(extra);
    run(repo, &args)
}

/// subject.lifecycle.29: inside its approved scope a shipped subject
/// reopens with no human, the review guard skipped and said so; the new
/// record carries the reason, the actor and the change the follow-up goes
/// into, and the next status write clears it.
#[test]
fn subject_lifecycle_29_a_shipped_subject_reopens_inside_its_approved_scope_and_records_why() {
    let dir = repo();
    seed_shipped(dir.path());

    let out = reopen(dir.path(), &[]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).starts_with("canon subject status: demo-subject → verifying — "), "{}", stdout(&out));
    let err = stderr(&out);
    assert!(
        err.contains(&format!(
            "canon subject status: reopened `demo-subject` from shipped, recorded by `impl-agent`: {REOPEN_REASON}; keep the follow-up in its approved change `c-demo`, then scenario, evidence and review; shipping again runs every guard\n"
        )),
        "{err}"
    );
    assert!(err.contains("skipped unreviewed-promotion — a reopen moves the subject back to fix its gaps; shipping again runs it"), "{err}");
    let subject = current_subject(dir.path());
    assert_eq!(subject["status"], "verifying");
    assert_eq!(subject["reopen"], serde_json::json!({"reason": REOPEN_REASON, "change": "c-demo", "actor": {"agent_id": "impl-agent"}}));

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let subject = current_subject(dir.path());
    assert_eq!(subject["status"], "shipped");
    assert!(subject.get("reopen").is_none(), "the note belongs to the reopen that wrote it: {subject}");
}

/// subject.lifecycle.30: a newer adopted change is a new scope. While
/// its brief has no `Approved:` line the reopen is refused naming that
/// brief, `--override-reason` does not waive it, and the subject stays
/// shipped; once the human answers, the same reopen goes through.
#[test]
fn subject_lifecycle_30_reopening_outside_an_approved_scope_is_refused_naming_the_brief() {
    let dir = repo();
    seed_shipped(dir.path());
    seed_change(dir.path(), "c-next");
    assert!(run(dir.path(), &["subject", "adopt", "c-next", "--subject", "demo-subject"]).status.success());
    write_brief(dir.path(), "c-next", "Status: draft, awaiting the human.\n");

    let refusal = "canon subject status: SUBJECT_GATE_VIOLATION — unreviewed-promotion demo-subject — shipped → verifying: reopening needs the approved scope, and the brief is not approved — .canon/changes/c-next/brief.md has no `Approved:` line; a human must answer the brief, and only from that answer the agent will record their words with `canon brief approve --change c-next --by <the human> --note \"<their words>\"`\n";
    let out = reopen(dir.path(), &[]);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains(refusal), "{}", stderr(&out));
    assert_eq!(current_subject(dir.path())["status"], "shipped", "a refusal leaves the record unchanged");

    let out = reopen(dir.path(), &["--override-reason", "the human said go"]);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains(refusal) && stderr(&out).contains("waives only the review checks"), "{}", stderr(&out));
    assert_eq!(current_subject(dir.path())["status"], "shipped");

    write_brief(dir.path(), "c-next", &format!("{APPROVED}\n"));
    let out = reopen(dir.path(), &[]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(current_subject(dir.path())["reopen"]["change"], "c-next");
}

/// subject.lifecycle.31: a reopen without `--reason`, or with a blank
/// one, is a usage refusal, with a hint, that leaves the subject
/// unchanged.
#[test]
fn subject_lifecycle_31_a_reopen_needs_a_reason() {
    let dir = repo();
    seed_shipped(dir.path());

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "verifying"]);
    assert_eq!(out.status.code(), Some(2), "stderr: {}", stderr(&out));
    assert!(
        stderr(&out).contains("SUBJECT_REASON_REQUIRED — reopening demo-subject (shipped → verifying) needs --reason <text>: why it reopens, recorded on the subject with the actor"),
        "{}",
        stderr(&out)
    );
    assert!(stderr(&out).contains("hint: canon subject status demo-subject --to verifying --reason"), "the refusal says how to retry: {}", stderr(&out));
    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "verifying", "--reason", "  "]);
    assert_eq!(out.status.code(), Some(2), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains("--reason must be one non-empty line"), "{}", stderr(&out));
    assert_eq!(current_subject(dir.path())["status"], "shipped");
}

/// #63: `--reason` is an audit note on every transition, not only the
/// reopen: `building → verifying` accepts it and the record carries it
/// with the actor, and the next write without one clears it.
#[test]
fn subject_lifecycle_31b_any_transition_accepts_a_reason_and_records_it() {
    let dir = repo();
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Building, &[]);

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "verifying", "--reason", "story complete", "--actor-id", "impl-agent"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let subject = current_subject(dir.path());
    assert_eq!(subject["status"], "verifying");
    assert_eq!(subject["status_reason"]["reason"], "story complete", "{subject}");
    assert_eq!(subject["status_reason"]["actor"]["agent_id"], "impl-agent", "{subject}");
    assert!(subject.get("reopen").is_none(), "only a reopen records `reopen`: {subject}");

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "retired"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(current_subject(dir.path()).get("status_reason").is_none(), "a reason belongs to the transition that gave it");
}

/// subject.lifecycle.32: a reopened subject ships through every guard
/// again: an open blocker found in the follow-up and a new owned
/// scenario with no evidence each refuse, and once they are fixed,
/// evidenced and reviewed it ships.
#[test]
fn subject_lifecycle_32_shipping_a_reopened_subject_runs_every_guard_again() {
    let dir = repo();
    seed_shipped(dir.path());
    assert!(reopen(dir.path(), &[]).status.success());

    seed_blocker(dir.path(), None);
    let envelope = Envelope::new(1, RecordKind::Scenario, Utc::now(), Actor::new("canon", RoleId::parse("implementer").unwrap()));
    let mut follow_up = Scenario::new(envelope, ProjectId::parse("demo").unwrap(), ScenarioId::parse("world.demo.02").unwrap(), "follow-up", "", SpecDigest::of(b"02"));
    follow_up.subject_id = Some(SubjectId::parse("demo-subject").unwrap());
    ledger(dir.path()).write(&follow_up).unwrap();

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped"]);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("uncovered-cell world.demo.02 — verifying → shipped: no ledger verdict"), "the evidence gate ran: {err}");
    assert!(err.contains("open-blocker c-demo#1.1 — verifying → shipped"), "the review guard ran: {err}");
    assert!(err.contains("unreviewed-promotion world.demo.02 — verifying → shipped"), "the review guard ran: {err}");
    assert_eq!(current_subject(dir.path())["status"], "verifying");

    seed_blocker(dir.path(), Some(5));
    seed_reverification(dir.path(), "world.demo.01");
    let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now() + chrono::Duration::seconds(10), Actor::new("impl-agent", RoleId::parse("implementer").unwrap()));
    let evidence = EvidenceRecord::new(envelope, None, Some(ScenarioId::parse("world.demo.02").unwrap()), None, EvidenceVerdict::Faithful)
        .with_project_id(ProjectId::parse("demo").unwrap());
    ledger(dir.path()).write(&evidence).unwrap();
    let envelope = Envelope::new(1, RecordKind::Review, Utc::now(), Actor::new("reviewer-2", RoleId::parse("reviewer").unwrap()));
    let review = Review::new(
        envelope,
        ProjectId::parse("demo").unwrap(),
        ScenarioId::parse("world.demo.02").unwrap(),
        "reviewer-2",
        "pin-reviewer-2-02",
        ProvenanceRef::OriginalSpecRef("specs/demo.feature".into()),
    );
    ledger(dir.path()).write(&pinned(dir.path(), review)).unwrap();
    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(current_subject(dir.path())["status"], "shipped");
}

// ── a change belongs to several subjects (0.18 D4) ──

/// The latest stored version of change `id`, as `canon query` shows it.
fn latest_change(repo: &Path, id: &str) -> Value {
    let changes: Value = serde_json::from_str(&stdout(&run(repo, &["query", "--kind", "change", "--json"]))).unwrap();
    changes["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["change_id"] == id)
        .max_by_key(|c| chrono::DateTime::parse_from_rfc3339(c["at"].as_str().unwrap()).unwrap())
        .cloned()
        .expect("the change is in the store")
}

/// subject.lifecycle.34: a change written before 0.18 names its one
/// subject as `subject_id`. It reads, and `canon query` shows it, as a
/// one-element `subject_ids`; adopting it under a second subject appends
/// rather than re-points, and the new version is written as the list.
#[test]
fn subject_lifecycle_34_a_pre_0_18_change_reads_as_a_one_element_list_and_a_second_adopt_appends() {
    let dir = repo();
    for id in ["perception", "editing"] {
        assert!(run(dir.path(), &["subject", "new", id, "--domain", "dev", "--title", id]).status.success());
    }
    let old = serde_json::json!({
        "schema": 1,
        "kind": "change",
        "at": "2026-10-01T00:00:00.000000Z",
        "actor": {"agent_id": "canon-plan-import-canon"},
        "change_id": "refactor-core",
        "title": "refactor-core",
        "summary": "Refactor core",
        "status": "in_progress",
        "subject_id": "perception"
    });
    let path = dir.path().join(".canon/ledger").join(canon_store::partition::expected_relative_path(RecordKind::Change, &old).unwrap());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, serde_json::to_vec_pretty(&old).unwrap()).unwrap();

    assert_eq!(latest_change(dir.path(), "refactor-core")["subject_ids"], serde_json::json!(["perception"]), "the old key reads as a list");

    let out = run(dir.path(), &["subject", "adopt", "refactor-core", "--subject", "editing"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(stdout(&out).contains("the change now belongs to `perception` and `editing`"), "{}", stdout(&out));
    let change = latest_change(dir.path(), "refactor-core");
    assert_eq!(change["subject_ids"], serde_json::json!(["perception", "editing"]), "appended, not re-pointed: {change}");
    assert_eq!(change["schema"], 2, "written at the current generation: {change}");
    let changes: Value = serde_json::from_str(&stdout(&run(dir.path(), &["query", "--kind", "change", "--json"]))).unwrap();
    assert_eq!(changes["count"], 1, "the old and the new version fold to one row: {changes}");
    assert_eq!(changes["records"][0]["subject_ids"], serde_json::json!(["perception", "editing"]), "{changes}");
}

// ── the ship guard runs the brief's blocker rules (0.19 D2) ──

/// A PRD brief with one flow step, `F1`, approved by a human.
fn prd_brief_with(extra: &str) -> String {
    format!(
        "# Demo\n\n<!-- canon:section intent role=context -->\n## Intent\n\nDemo\n\nWhy.\n\n<!-- canon:section outcome role=normative -->\n## Desired outcome\n\nIt works. ==**It works.**==\n\n<!-- canon:section flow role=normative -->\n## User flow\n\n1. F1 The user sees → does → becomes. ==**One step.**==\n\n<!-- canon:section detail role=normative -->\n## Step detail\n\n### F1 Step\n\nRules. ==**Rules.**==\n\n<!-- canon:section acceptance role=normative -->\n## Acceptance\n\nF1 observable, failure covered. ==**Covered.**==\n\n<!-- canon:section questions role=question -->\n## Questions and assumptions\n\nNone.\n\n<!-- canon:section out-of-scope role=context -->\n## Out of scope\n\nNone.\n\n{APPROVED}\n{extra}"
    )
}

/// 0.19 D2: an approved PRD brief whose flow step no scenario points at
/// refuses the ship, `--override-reason` does not waive it, and the brief's
/// own waiver (one rule, one section, a reason) is what clears it.
#[test]
fn the_ship_guard_refuses_brief_blockers_and_only_the_briefs_own_waiver_clears_them() {
    let dir = repo();
    seed_technically_ship_ready(dir.path(), "");
    std::fs::write(dir.path().join(".canon/changes/c-demo/brief.md"), prd_brief_with("")).unwrap();

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped"]);
    assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains("brief.flow-uncovered [flow]"), "{}", stderr(&out));
    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped", "--override-reason", "ship it"]);
    assert_eq!(out.status.code(), Some(1), "an override never waives brief blockers: {}", stderr(&out));
    assert_eq!(query_subjects(dir.path(), &[])["records"][0]["status"], "verifying");

    let waiver = "<!-- canon:waive brief.flow-uncovered flow \"world.demo.01 predates flow tags\" -->\n";
    std::fs::write(dir.path().join(".canon/changes/c-demo/brief.md"), prd_brief_with(waiver)).unwrap();
    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped"]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert_eq!(query_subjects(dir.path(), &[])["records"][0]["status"], "shipped");
}
