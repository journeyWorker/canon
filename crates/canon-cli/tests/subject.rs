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
    Actor, Change, ChangeId, ChangeStatus, Envelope, EvidenceRecord, EvidenceVerdict, ProjectId, RecordKind, RoleId, Scenario,
    ScenarioId, SpecDigest, Subject, SubjectId, SubjectStatus,
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
    let envelope = Envelope::new(1, RecordKind::Subject, Utc::now(), Actor::new("canon", RoleId::parse("implementer").unwrap()));
    let subject = Subject::new(envelope, SubjectId::parse(id).unwrap(), "Seeded", "s", "dev", status, RoleId::parse("implementer").unwrap());
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
