//! Integration tests for `canon status [--repo] [--json]` (0.14 D7),
//! invoking the built `canon` binary against offline git-tier fixtures
//! in a tmpdir. Records are planted through the same `GitTier` root the
//! CLI reads (`tests/subject.rs`'s shape), so each test controls exactly
//! which scenarios, evidence, reviews and findings exist.

use std::path::Path;
use std::process::{Command, Output};

use canon_model::{
    Actor, ChangeId, Envelope, EvidenceRecord, EvidenceVerdict, Finding, FindingSeverity, ProjectId, ProvenanceRef, RecordKind, Review, RoleId, Scenario,
    ScenarioId, SpecDigest, Subject, SubjectId, SubjectStatus,
};
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;
use chrono::Utc;
use serde_json::Value;
use tempfile::TempDir;

const CANON_YAML: &str = "\
tiers:
  local: { backend: git, root: .canon/ledger }
routing:
  subject: local
  change: local
  scenario: local
  evidence_record: local
";

/// Evidence, failure cases and an independent review, all required.
const FULL_POLICY: &str = "spec_coverage:\n  require_evidence: true\n  require_cases: [failure]\n  require_review: {}\n";

fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("canon.yaml"), CANON_YAML).unwrap();
    dir
}

fn write_policy(repo: &Path, yaml: &str) {
    std::fs::create_dir_all(repo.join(".canon")).unwrap();
    std::fs::write(repo.join(".canon/policy.yaml"), yaml).unwrap();
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

fn actor(id: &str, role: &str) -> Actor {
    Actor::new(id, RoleId::parse(role).unwrap())
}

fn seed_subject(repo: &Path, id: &str, status: SubjectStatus, changes: &[&str]) {
    let envelope = Envelope::new(1, RecordKind::Subject, Utc::now(), actor("canon", "implementer"));
    let subject = Subject::new(envelope, SubjectId::parse(id).unwrap(), "Seeded", "s", "dev", status, RoleId::parse("implementer").unwrap())
        .with_change_ids(changes.iter().map(|c| ChangeId::parse(*c).unwrap()).collect());
    ledger(repo).write(&subject).unwrap();
}

/// One scenario as `canon inventory sync` writes it from its tags.
fn seed_scenario(repo: &Path, id: &str, subject: Option<&str>, case: Option<&str>) {
    let envelope = Envelope::new(1, RecordKind::Scenario, Utc::now(), actor("canon", "implementer"));
    let mut scenario = Scenario::new(envelope, ProjectId::parse("demo").unwrap(), ScenarioId::parse(id).unwrap(), "seeded", "", SpecDigest::of(id.as_bytes()));
    scenario.subject_id = subject.map(|s| SubjectId::parse(s).unwrap());
    scenario.case = case.map(str::to_string);
    ledger(repo).write(&scenario).unwrap();
}

fn seed_evidence(repo: &Path, scenario: &str, verdict: EvidenceVerdict) {
    let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), actor("impl-agent", "implementer"));
    let record = EvidenceRecord::new(envelope, None, Some(ScenarioId::parse(scenario).unwrap()), None, verdict).with_project_id(ProjectId::parse("demo").unwrap());
    ledger(repo).write(&record).unwrap();
}

fn seed_review(repo: &Path, scenario: &str, reviewer: &str) {
    let envelope = Envelope::new(1, RecordKind::Review, Utc::now(), actor(reviewer, "reviewer"));
    let review = Review::new(
        envelope,
        ProjectId::parse("demo").unwrap(),
        ScenarioId::parse(scenario).unwrap(),
        reviewer,
        format!("pin-{reviewer}-{scenario}"),
        ProvenanceRef::OriginalSpecRef("specs/demo.feature".into()),
    );
    ledger(repo).write(&review).unwrap();
}

fn seed_blocker(repo: &Path, change: &str) {
    let envelope = Envelope::new(1, RecordKind::Finding, Utc::now(), actor("reviewer-2", "reviewer"));
    let finding = Finding::new(envelope, ChangeId::parse(change).unwrap(), 1, 1, FindingSeverity::Blocker, "reviewer-2", "unbounded subprocess wait");
    ledger(repo).write(&finding).unwrap();
}

/// The approval line the agent records once the human answers (0.16 D1).
const APPROVED: &str = "Approved: Journey 2026-10-11 — \"승인. 네 가정대로 진행\"";

/// The one `next` step status addresses to a human.
const BRIEF_STEP: &str = "get the brief approved (one human answer covers brief and policy)";

/// `proposal.md` for `change` at `<root>/openspec/changes/<change>/`,
/// with `extra` after the `## Why` paragraph. `root` is the repo for the
/// compat-default openspec source (`CANON_YAML` has no `plans:`).
fn write_proposal(root: &Path, change: &str, extra: &str) {
    let dir = root.join("openspec/changes").join(change);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("proposal.md"), format!("# {change}\n\n## Why\n\nSeeded.\n\n{extra}")).unwrap();
}

/// A subject adopting `c-<id>`, whose proposal.md carries an approval.
fn seed_approved_subject(repo: &Path, id: &str, status: SubjectStatus) {
    let change = format!("c-{id}");
    seed_subject(repo, id, status, &[&change]);
    write_proposal(repo, &change, &format!("{APPROVED}\n"));
}

fn status_json(repo: &Path) -> Value {
    let out = run(repo, &["status", "--json"]);
    assert_eq!(out.status.code(), Some(0), "status always exits 0; stderr: {}", stderr(&out));
    let report: Value = serde_json::from_str(&stdout(&out)).expect("status --json prints one JSON object");
    for step in report["next"].as_array().unwrap() {
        match step["actor"].as_str() {
            Some("agent") => {}
            Some("human") => assert_eq!(step["command"], BRIEF_STEP, "the brief is the only step status addresses to a human: {step}"),
            _ => panic!("every step names its actor: {step}"),
        }
    }
    report
}

fn subject<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["subjects"].as_array().unwrap().iter().find(|s| s["id"] == id).unwrap_or_else(|| panic!("no subject `{id}` in {report}"))
}

/// The agent's steps: each a canon command line that parses as printed.
fn commands(report: &Value) -> Vec<String> {
    let commands: Vec<String> =
        report["next"].as_array().unwrap().iter().filter(|n| n["actor"] == "agent").map(|n| n["command"].as_str().unwrap().to_string()).collect();
    for command in &commands {
        assert_suggestion_parses_and_names_the_unit(command);
    }
    commands
}

/// The human steps' subjects, read off their reasons.
fn human_steps(report: &Value) -> Vec<String> {
    report["next"].as_array().unwrap().iter().filter(|n| n["actor"] == "human").map(|n| n["why"].as_str().unwrap().split(':').next().unwrap().to_string()).collect()
}

/// Splits a suggested command the way a POSIX shell would: whitespace
/// outside double quotes separates words, `\"` inside them is a quote.
fn shell_words(command: &str) -> Vec<String> {
    let (mut words, mut word, mut quoted, mut chars) = (Vec::new(), String::new(), false, command.chars());
    let mut started = false;
    while let Some(c) = chars.next() {
        match c {
            '"' => (quoted, started) = (!quoted, true),
            '\\' if quoted => word.extend(chars.next()),
            c if c.is_whitespace() && !quoted => {
                if started {
                    words.push(std::mem::take(&mut word));
                }
                started = false;
            }
            c => {
                word.push(c);
                started = true;
            }
        }
    }
    assert!(!quoted, "unbalanced quote in {command}");
    if started {
        words.push(word);
    }
    words
}

/// Fills each `<placeholder>` the way an operator would: `<sha>` with a
/// commit sha, any other with its name as a kebab-case slug, so typed
/// values parse.
fn fill_placeholders(word: &str) -> String {
    let mut out = String::new();
    let mut rest = word;
    while let Some((before, after)) = rest.split_once('<') {
        let (name, tail) = after.split_once('>').unwrap_or_else(|| panic!("unclosed placeholder in {word}"));
        out.push_str(before);
        out.push_str(&if name == "sha" { "a".repeat(40) } else { name.replace([' ', '_'], "-").to_lowercase() });
        rest = tail;
    }
    out.push_str(rest);
    out
}

/// context.status.14: every suggested command parses through clap as
/// printed (an unknown flag is a parse error before `--help` is reached),
/// and every write suggestion names the unit and its session wherever the
/// command takes `--actor-id` / `--session-id`.
fn assert_suggestion_parses_and_names_the_unit(command: &str) {
    let words = shell_words(command);
    assert_eq!(words.first().map(String::as_str), Some("canon"), "{command}");
    let filled: Vec<String> = words[1..].iter().map(|w| fill_placeholders(w)).collect();
    let out = Command::new(env!("CARGO_BIN_EXE_canon")).args(&filled).arg("--help").output().expect("spawn canon binary");
    assert!(out.status.success(), "`{command}` must parse as printed: {}", stderr(&out));
    let help = stdout(&out);
    for flag in ["--actor-id", "--session-id"] {
        if help.contains(&format!("{flag} <")) {
            assert!(words.iter().any(|w| w == flag), "`{command}` takes {flag} but the suggestion omits it");
        }
    }
}

/// Every file under `dir`, recursively, with its bytes.
fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                files.push((path.display().to_string(), std::fs::read(&path).unwrap()));
            }
        }
    }
    files.sort();
    files
}

/// A `building` subject with two surfaces: `world.demo` has two scenarios,
/// one evidenced and reviewed by someone else, one neither, and no failure
/// case; `world.edge` has one evidenced failure-case scenario reviewed only
/// by its own evidence actor. An adopted change carries an open blocker.
#[test]
fn status_counts_each_subject_through_the_gates_joins() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Building, &["c-demo"]);
    seed_scenario(dir.path(), "world.demo.01", Some("demo-subject"), Some("happy"));
    seed_scenario(dir.path(), "world.demo.02", Some("demo-subject"), Some("happy"));
    seed_scenario(dir.path(), "world.edge.01", Some("demo-subject"), Some("failure"));
    seed_evidence(dir.path(), "world.demo.01", EvidenceVerdict::Faithful);
    seed_evidence(dir.path(), "world.edge.01", EvidenceVerdict::Divergent);
    seed_review(dir.path(), "world.demo.01", "reviewer-2");
    seed_review(dir.path(), "world.edge.01", "impl-agent");
    seed_blocker(dir.path(), "c-demo");

    let report = status_json(dir.path());
    assert_eq!(report["statusVersion"], 2);
    assert_eq!(report["canonVersion"], env!("CARGO_PKG_VERSION"));
    assert_eq!(report["policy"]["present"], true);
    assert_eq!(report["policy"]["spec_coverage"]["require_cases"], serde_json::json!(["failure"]));
    let s = subject(&report, "demo-subject");
    assert_eq!(s["status"], "building");
    assert_eq!(s["scenarios"], 3);
    assert_eq!(s["evidenced"], 2);
    assert_eq!(s["divergent"], 1);
    // distinct_actor: the self-review of world.edge.01 does not count.
    assert_eq!(s["reviewed"], 1, "{s}");
    // building → verifying enters require_review's default scope.
    assert_eq!(s["reviewDue"], true);
    assert_eq!(s["openBlockers"], 1);
    assert_eq!(s["missingCases"], serde_json::json!([{"projectId": "demo", "surface": "world.demo", "case": "failure"}]));

    let text = stdout(&run(dir.path(), &["status"]));
    // The header: the version, then the effective coverage, case and review
    // settings, then the warning that both seeded reviews predate pinning
    // and, their pins being no commit of this repo, went unchecked.
    let header = format!(
        "canon {}\npolicy: .canon/policy.yaml\n  spec_coverage: require_evidence=true scope=<every scenario>\n  require_cases: failure\n  require_review: scope=verifying, shipped distinct_actor=true block_on_findings=true\n  level: strict\n{UNAPPROVED}\nwarning: 2 review(s) predate review pinning (0.15) and were checked against the commit each one pinned: 0 current, 0 stale, 2 undecidable; an undecidable one still counts unchecked, because its pinned commit or the scenario at it is not in this clone — check out full history (`fetch-depth: 0` in CI) to check it\n\n",
        env!("CARGO_PKG_VERSION")
    );
    assert!(text.starts_with(&header), "expected the header\n{header}\ngot\n{text}");
    assert!(text.contains("  building (1):\n    demo-subject: 3 scenarios, 2 evidenced, 1 divergent, 1 reviewed (2 due), 1 open blockers\n"), "{text}");
    assert!(text.contains("      missing @case:failure on world.demo\n"), "{text}");
}

/// review.add.27 (0.15 D1/D4): a review whose scenario was re-attested after it is
/// counted stale, a scenario with only an indeterminate review is counted
/// unsigned, and the review step names both.
#[test]
fn status_counts_stale_and_unsigned_reviews() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Building, &[]);
    seed_scenario(dir.path(), "world.demo.01", Some("demo-subject"), Some("happy"));
    seed_scenario(dir.path(), "world.demo.02", Some("demo-subject"), Some("failure"));
    seed_evidence(dir.path(), "world.demo.01", EvidenceVerdict::Faithful);
    seed_evidence(dir.path(), "world.demo.02", EvidenceVerdict::Faithful);
    let review = |scenario: &str, outcome: &str| {
        let out = run(
            dir.path(),
            &[
                "review", "add", "--project-id", "demo", "--scenario-id", scenario, "--reviewer", "reviewer-2", "--actor-id", "reviewer-2", "--role", "reviewer",
                "--session-id", "sess-review", "--pin", "p1", "--original-spec-ref", "specs/demo.feature", "--outcome", outcome,
            ],
        );
        assert!(out.status.success(), "{}", stderr(&out));
    };
    review("world.demo.01", "signed");
    review("world.demo.02", "indeterminate");
    assert_eq!(subject(&status_json(dir.path()), "demo-subject")["reviewed"], 1, "the current signed review counts");
    // Re-attest world.demo.01 after its review.
    seed_evidence(dir.path(), "world.demo.01", EvidenceVerdict::Faithful);

    let report = status_json(dir.path());
    let s = subject(&report, "demo-subject");
    assert_eq!((s["reviewed"].as_u64(), s["staleReviews"].as_u64(), s["unsignedReviews"].as_u64()), (Some(0), Some(1), Some(1)), "{s}");
    assert_eq!(report["warnings"], serde_json::json!([UNAPPROVED.strip_prefix("warning: ").unwrap()]), "pinned reviews draw no legacy warning: {report}");
    let why = report["next"].as_array().unwrap().iter().find(|n| n["command"].as_str().unwrap().starts_with("canon review add")).expect("a review step")["why"].as_str().unwrap().to_string();
    assert!(
        why.contains("2 of 2 scenario(s) lack a qualifying review (1 with a stale review (spec or evidence changed since it was reviewed); 1 with only indeterminate or unavailable reviews)"),
        "{why}"
    );
    let text = stdout(&run(dir.path(), &["status"]));
    assert!(text.contains("demo-subject: 2 scenarios, 2 evidenced, 0 reviewed (2 due), 1 stale review(s), 1 unsigned review(s), 0 open blockers\n"), "{text}");
}

/// A policy whose settings differ from the defaults prints them as
/// resolved: a `spec_coverage.scope`, `exclude_lanes`, no required case,
/// and no review rule.
#[test]
fn status_header_prints_the_effective_settings() {
    let dir = repo();
    write_policy(dir.path(), "spec_coverage:\n  require_evidence: false\n  scope: [building]\n  exclude_lanes: [process]\n");
    let text = stdout(&run(dir.path(), &["status"]));
    let header = format!(
        "canon {}\npolicy: .canon/policy.yaml\n  spec_coverage: require_evidence=false scope=building exclude_lanes=process\n  require_cases: <none>\n  require_review: <absent>\n  level: custom\n{UNAPPROVED}\n",
        env!("CARGO_PKG_VERSION")
    );
    assert!(text.starts_with(&header), "expected the header\n{header}\ngot\n{text}");
}

/// The approval warning every fixture policy here carries: none of them
/// is approved (`canon policy approve`).
const UNAPPROVED: &str = "warning: policy not approved: .canon/policy.yaml has no approval record (.canon/policy.approval.json); a human reviews it and runs `canon policy approve --by <who>`";

#[test]
fn status_next_names_the_command_for_each_gap() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    seed_approved_subject(dir.path(), "demo-subject", SubjectStatus::Verifying);
    seed_approved_subject(dir.path(), "empty-subject", SubjectStatus::Building);
    seed_scenario(dir.path(), "world.demo.01", Some("demo-subject"), Some("happy"));
    seed_scenario(dir.path(), "world.demo.02", Some("demo-subject"), Some("happy"));
    seed_evidence(dir.path(), "world.demo.01", EvidenceVerdict::Faithful);
    seed_blocker(dir.path(), "c-demo-subject");

    let report = status_json(dir.path());
    assert_eq!(
        commands(&report),
        [
            "canon scenario new world.demo.03 --title \"<what happens on this path>\" --subject demo-subject --case failure",
            "canon evidence add --scenario-id world.demo.02 --project-id demo --kind test-run --role implementer --actor-id <unit> --session-id <session> --verdict faithful --ref \"<test command>\"",
            "canon finding close --change-id c-demo-subject --round 1 --seq 1 --disposition fixed --resolution-sha <sha> --actor-id <unit> --session-id <session>",
            "canon review add --project-id demo --scenario-id world.demo.01 --reviewer <reviewer> --actor-id <reviewer> --session-id <review-session> --role reviewer --pin <sha> --original-spec-ref \"<feature file>\"",
            "canon scenario new <area>.<surface>.01 --title \"<behavior>\" --subject empty-subject --case happy",
        ]
    );
    assert_eq!(report["nextOmitted"], 0);
    for step in report["next"].as_array().unwrap() {
        let why = step["why"].as_str().unwrap();
        assert!(!why.trim().is_empty() && !why.contains('\n'), "every step carries a one-line reason: {step}");
    }

    // The human form: each command line directly under its `# why` line.
    let text = stdout(&run(dir.path(), &["status"]));
    let block = text.split_once("\nnext:\n").map(|(_, rest)| rest).unwrap_or_else(|| panic!("no next: block\n{text}"));
    let lines: Vec<&str> = block.lines().collect();
    assert_eq!(lines.len(), 10, "five `# why` + command pairs:\n{block}");
    for (pair, step) in lines.chunks(2).zip(report["next"].as_array().unwrap()) {
        assert_eq!(pair[0], format!("  # {}", step["why"].as_str().unwrap()), "{block}");
        assert_eq!(pair[1], format!("  {}", step["command"].as_str().unwrap()), "{block}");
    }
    assert_eq!(lines[0], "  # demo-subject: 1 surface(s) lack a @case:failure scenario (first: world.demo); add one, never retag an existing scenario");
}

/// Finding 1: a corpus kind routed away from the rung status reads would
/// read as empty. Status fails closed: only the routing repair is listed,
/// never a step derived from the empty corpus.
#[test]
fn status_fails_closed_when_scenarios_route_away() {
    let dir = repo();
    std::fs::write(
        dir.path().join("canon.yaml"),
        "tiers:\n  local: { backend: git, root: .canon/ledger }\n  hot: { backend: sqlite, path: .canon/hot.db }\nrouting:\n  subject: local\n  scenario: hot\n  evidence_record: local\n",
    )
    .unwrap();
    write_policy(dir.path(), FULL_POLICY);
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Verifying, &[]);

    let report = status_json(dir.path());
    assert_eq!(commands(&report), ["canon init --check-config"], "{report}");
    assert!(report["next"][0]["why"].as_str().unwrap().starts_with("set `routing.scenario: local` in canon.yaml"), "{report}");
    assert!(
        report["warnings"].as_array().unwrap().iter().any(|w| w == "`scenario` routes away from the `local` rung status reads, so its records are not counted"),
        "{report}"
    );
    assert_eq!(report["nextOmitted"], 0);
}

/// Finding 2: an unusable `spec_coverage` makes the gate refuse, so status
/// lists the policy repair first and suggests no status move, even for a
/// subject whose scenarios are all evidenced.
#[test]
fn status_treats_an_invalid_coverage_policy_as_a_blocking_gap() {
    let dir = repo();
    write_policy(dir.path(), "spec_coverage:\n  require_cases: [Not-A-Slug]\n");
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Verifying, &[]);
    seed_scenario(dir.path(), "world.demo.01", Some("demo-subject"), Some("failure"));
    seed_evidence(dir.path(), "world.demo.01", EvidenceVerdict::Faithful);

    let report = status_json(dir.path());
    assert_eq!(commands(&report), ["canon gate check"], "{report}");
    assert!(report["next"][0]["why"].as_str().unwrap().starts_with("fix `spec_coverage` in .canon/policy.yaml ("), "{report}");
    assert!(report["policy"]["spec_coverage"]["invalid"].is_string(), "{report}");
    assert!(report["warnings"][0].as_str().unwrap().starts_with("policy's `spec_coverage` section is unusable"), "{report}");
    assert_eq!(run(dir.path(), &["gate", "check"]).status.code(), Some(1), "the gate this mirrors refuses the invalid section");
}

/// Finding 3: with several spec roots, `canon scenario new` refuses a call
/// without `--project`. Status names the owning root exactly, and the
/// suggested command runs as printed.
#[test]
fn status_names_the_spec_root_when_several_are_configured() {
    let dir = repo();
    std::fs::write(
        dir.path().join("canon.yaml"),
        format!("{CANON_YAML}specs:\n  roots:\n    - id: demo\n      root: specs/demo\n    - id: other\n      root: specs/other\n"),
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("specs/demo")).unwrap();
    std::fs::create_dir_all(dir.path().join("specs/other")).unwrap();
    write_policy(dir.path(), FULL_POLICY);
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Building, &[]);
    seed_subject(dir.path(), "empty-subject", SubjectStatus::Building, &[]);
    // A real feature file under the configured `demo` root: an index
    // record no feature file declares is marked removed by the refresh.
    let authored = run(dir.path(), &["scenario", "new", "world.demo.01", "--title", "seeded", "--subject", "demo-subject", "--case", "happy", "--project", "demo"]);
    assert!(authored.status.success(), "{}", String::from_utf8_lossy(&authored.stderr));

    let report = status_json(dir.path());
    let next = commands(&report);
    let case_step = "canon scenario new world.demo.02 --title \"<what happens on this path>\" --subject demo-subject --case failure --project demo";
    assert_eq!(next[0], case_step, "{report}");
    assert!(next.contains(&"canon scenario new <area>.<surface>.01 --title \"<behavior>\" --subject empty-subject --case happy --project <root-id>".to_string()), "{report}");
    let empty = report["next"].as_array().unwrap().iter().find(|n| n["command"].as_str().unwrap().contains("empty-subject")).unwrap();
    assert!(empty["why"].as_str().unwrap().ends_with("(spec roots: demo, other)"), "{empty}");

    let bin_dir = Path::new(env!("CARGO_BIN_EXE_canon")).parent().unwrap().to_path_buf();
    let path = format!("{}:{}", bin_dir.display(), std::env::var("PATH").unwrap_or_default());
    let out = Command::new("sh").arg("-c").arg(case_step).env("PATH", path).current_dir(dir.path()).output().unwrap();
    assert!(out.status.success(), "the suggested command must run as printed: {}", stderr(&out));
    let written = stdout(&out);
    assert!(written.contains("wrote `@world.demo.02` to ") && written.contains("/specs/demo/"), "written under the demo root: {written}");
}

/// One spec root: no `--project`, which `canon scenario new` would not need.
#[test]
fn status_omits_the_spec_root_flag_with_one_root() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Building, &[]);
    seed_scenario(dir.path(), "world.demo.01", Some("demo-subject"), Some("happy"));
    let report = status_json(dir.path());
    assert!(commands(&report).iter().all(|c| !c.contains("--project ")), "{report}");
}

#[test]
fn status_on_a_repo_with_no_subject_points_at_subject_new() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    let report = status_json(dir.path());
    assert_eq!(report["subjects"], serde_json::json!([]));
    assert_eq!(commands(&report), ["canon subject new <id> --domain <domain> --title \"<title>\" --actor-id <unit>"]);
}

/// A verifying subject whose scenarios are all evidenced and reviewed.
fn ship_ready(dir: &Path) {
    write_policy(dir, FULL_POLICY);
    seed_subject(dir, "demo-subject", SubjectStatus::Verifying, &["c-demo-subject"]);
    seed_scenario(dir, "world.demo.01", Some("demo-subject"), Some("happy"));
    seed_scenario(dir, "world.demo.02", Some("demo-subject"), Some("failure"));
    for id in ["world.demo.01", "world.demo.02"] {
        seed_evidence(dir, id, EvidenceVerdict::Faithful);
        seed_review(dir, id, "reviewer-2");
    }
}

/// context.status.62: the advance rule fires only for a subject the
/// transition gates accept and whose brief is approved. The ship step is
/// the agent's own command, with no ask of the human, and following it
/// succeeds.
#[test]
fn context_status_62_ship_is_the_agents_step_once_the_brief_is_approved() {
    let dir = repo();
    ship_ready(dir.path());
    write_proposal(dir.path(), "c-demo-subject", &format!("{APPROVED}\n"));

    let report = status_json(dir.path());
    assert_eq!(commands(&report), ["canon subject status demo-subject shipped --actor-id <unit>"]);
    let step = &report["next"][0];
    assert_eq!(step["actor"], "agent", "{step}");
    let why = step["why"].as_str().unwrap();
    assert!(why.contains("ship it yourself unless the brief says the human ships"), "{why}");
    assert!(!why.contains("ask"), "shipping asks nobody: {why}");
    let out = run(dir.path(), &["subject", "status", "demo-subject", "shipped"]);
    assert!(out.status.success(), "the suggested transition must pass its gates: {}", stderr(&out));
    let report = status_json(dir.path());
    assert_eq!(subject(&report, "demo-subject")["status"], "shipped");
    assert_eq!(subject(&report, "demo-subject")["brief"]["approved"], true, "a shipped subject still shows its brief");
    assert_eq!(report["next"], serde_json::json!([]));
    assert!(stdout(&run(dir.path(), &["status"])).contains("next: nothing"));
}

/// context.status.61: a ship-ready subject whose newest adopted change's
/// proposal.md carries no `Approved:` line gets the one human step and no
/// move-on; a subject with no change at all is told where the brief goes.
#[test]
fn context_status_61_an_unapproved_brief_is_the_only_human_step_and_blocks_the_move_on() {
    let dir = repo();
    ship_ready(dir.path());
    write_proposal(dir.path(), "c-demo-subject", "Status: draft, awaiting the human.\n");
    seed_subject(dir.path(), "bare-subject", SubjectStatus::Building, &[]);
    seed_scenario(dir.path(), "world.bare.01", Some("bare-subject"), Some("failure"));
    seed_evidence(dir.path(), "world.bare.01", EvidenceVerdict::Faithful);

    let report = status_json(dir.path());
    assert_eq!(human_steps(&report), ["demo-subject", "bare-subject"], "{report}");
    assert!(commands(&report).iter().all(|c| !c.starts_with("canon subject status")), "no move-on without an approved brief: {report}");
    let whys: Vec<&str> = report["next"].as_array().unwrap().iter().filter(|n| n["actor"] == "human").map(|n| n["why"].as_str().unwrap()).collect();
    assert!(whys[0].contains("openspec/changes/c-demo-subject/proposal.md"), "{}", whys[0]);
    for why in &whys {
        assert!(why.contains("Approved: <who> <when>") && why.contains("canon policy approve --by <the human> --note"), "{why}");
    }
    assert!(whys[1].contains("canon change new <slug> --subject bare-subject"), "{}", whys[1]);
    let demo = subject(&report, "demo-subject");
    assert_eq!(demo["brief"], serde_json::json!({"approved": false, "change": "c-demo-subject", "proposal": "openspec/changes/c-demo-subject/proposal.md"}));
    assert_eq!(subject(&report, "bare-subject")["brief"], serde_json::json!({"approved": false}));

    let text = stdout(&run(dir.path(), &["status"]));
    assert!(text.contains("      brief: not approved (openspec/changes/c-demo-subject/proposal.md has no `Approved:` line)\n"), "{text}");
    assert!(text.contains("      brief: not approved (no adopted change)\n"), "{text}");
    assert!(text.contains(&format!("\n  human: {BRIEF_STEP}\n")), "{text}");
}

/// context.status.60: the brief is approved when the subject's newest
/// adopted change's proposal.md has a line starting `Approved:` (indent
/// allowed); status shows that change, its proposal.md and the recorded
/// words, for building, verifying and shipped subjects only.
#[test]
fn context_status_60_status_reads_the_approval_from_the_changes_proposal() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Building, &["c-first", "c-second"]);
    write_proposal(dir.path(), "c-first", "No approval here.\n");
    write_proposal(dir.path(), "c-second", &format!("  {APPROVED}\n"));
    seed_approved_subject(dir.path(), "done-subject", SubjectStatus::Shipped);
    seed_subject(dir.path(), "early-subject", SubjectStatus::Specced, &[]);

    let report = status_json(dir.path());
    let words = APPROVED.strip_prefix("Approved: ").unwrap();
    assert_eq!(
        subject(&report, "demo-subject")["brief"],
        serde_json::json!({"approved": true, "change": "c-second", "proposal": "openspec/changes/c-second/proposal.md", "approval": words})
    );
    assert_eq!(
        subject(&report, "done-subject")["brief"],
        serde_json::json!({"approved": true, "change": "c-done-subject", "proposal": "openspec/changes/c-done-subject/proposal.md", "approval": words}),
        "a shipped subject shows its brief in full"
    );
    assert!(subject(&report, "early-subject").get("brief").is_none(), "a specced subject carries no brief field");
    assert!(human_steps(&report).is_empty(), "{report}");
    let text = stdout(&run(dir.path(), &["status"]));
    assert!(text.contains(&format!("      brief: approved in openspec/changes/c-second/proposal.md — {words}\n")), "{text}");
    assert!(text.contains(&format!("      brief: approved in openspec/changes/c-done-subject/proposal.md — {words}\n")), "{text}");
}

/// context.status.63: only a line that starts with `Approved:`, says
/// something, and sits outside HTML comments and fenced code blocks
/// approves. Each case runs in its own repo with one subject, so its
/// report stays under the `next` cap and its human step is checked
/// exactly; the controls prove a comment or fence that has closed does
/// not hide a later approval.
#[test]
fn context_status_63_a_line_that_only_mentions_approval_does_not_approve() {
    let refused = [
        ("empty", "Approved:\n".to_string()),
        ("mid-line", "Status: Approved: by the agent\n".to_string()),
        ("comment-one-line", "<!-- Approved: Journey 2026-10-11 -->\n".to_string()),
        ("comment-multi-line", format!("<!--\n{APPROVED}\n-->\n")),
        ("comment-opened-mid-line", format!("Draft notes <!-- not yet:\n{APPROVED}\n-->\n")),
        ("fence-backtick", format!("```\n{APPROVED}\n```\n")),
        ("fence-tilde", format!("~~~md\n{APPROVED}\n~~~\n")),
        ("fence-nested", format!("````\n```\n{APPROVED}\n```\n````\n")),
        ("fence-unclosed", format!("```\n{APPROVED}\n")),
    ];
    let accepted = [
        ("after-comment", format!("<!-- a note -->\n<!--\nmore\n-->\n{APPROVED}\n")),
        ("after-fence", format!("```\nApproved: in code\n```\n{APPROVED}\n")),
    ];
    // One building subject adopting `c-<case>`, whose proposal.md is `body`.
    let report_for = |case: &str, body: &str| {
        let dir = repo();
        write_policy(dir.path(), FULL_POLICY);
        seed_subject(dir.path(), "demo-subject", SubjectStatus::Building, &[&format!("c-{case}")]);
        write_proposal(dir.path(), &format!("c-{case}"), body);
        let report = status_json(dir.path());
        assert_eq!(report["nextOmitted"], 0, "{case}: the report stays under the next cap: {report}");
        report
    };

    for (case, body) in &refused {
        let report = report_for(case, body);
        assert_eq!(subject(&report, "demo-subject")["brief"]["approved"], false, "{case} must not approve:\n{body}");
        let humans: Vec<&Value> = report["next"].as_array().unwrap().iter().filter(|n| n["actor"] == "human").collect();
        assert_eq!(humans.len(), 1, "{case}: exactly one human step: {report}");
        assert_eq!(humans[0]["command"], BRIEF_STEP, "{case}: {report}");
        assert!(humans[0]["why"].as_str().unwrap().starts_with("demo-subject: "), "{case}: the step is the subject's: {report}");
        assert!(humans[0]["why"].as_str().unwrap().contains(&format!("openspec/changes/c-{case}/proposal.md")), "{case}: {report}");
    }
    for (case, body) in &accepted {
        let report = report_for(case, body);
        assert_eq!(subject(&report, "demo-subject")["brief"]["approved"], true, "{case} must approve:\n{body}");
        assert!(human_steps(&report).is_empty(), "{case}: an approved brief asks no human: {report}");
    }
}

/// context.status.65: a fix lives as tasks inside the approved change, so
/// a newer adopted change is a new scope. An approval on an older change
/// does not cover it: status names the newest change, asks for the human,
/// and withholds the ship step.
#[test]
fn context_status_65_an_older_changes_approval_does_not_cover_a_newer_change() {
    let dir = repo();
    ship_ready(dir.path());
    write_proposal(dir.path(), "c-demo-subject", &format!("{APPROVED}\n"));
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Verifying, &["c-demo-subject", "c-scope-two"]);
    write_proposal(dir.path(), "c-scope-two", "A new scope, not yet approved.\n");
    seed_subject(dir.path(), "other-subject", SubjectStatus::Building, &["c-other-old", "c-other-new"]);
    write_proposal(dir.path(), "c-other-old", &format!("{APPROVED}\n"));

    let report = status_json(dir.path());
    assert_eq!(
        subject(&report, "demo-subject")["brief"],
        serde_json::json!({"approved": false, "change": "c-scope-two", "proposal": "openspec/changes/c-scope-two/proposal.md"})
    );
    assert_eq!(subject(&report, "other-subject")["brief"], serde_json::json!({"approved": false, "change": "c-other-new"}), "no proposal.md for the newest change");
    let mut humans = human_steps(&report);
    humans.sort();
    assert_eq!(humans, ["demo-subject", "other-subject"], "{report}");
    assert!(commands(&report).iter().all(|c| !c.starts_with("canon subject status")), "an older approval never unlocks the ship step: {report}");
    let why = |id: &str| report["next"].as_array().unwrap().iter().find(|n| n["actor"] == "human" && n["why"].as_str().unwrap().starts_with(id)).unwrap()["why"].as_str().unwrap().to_string();
    assert!(why("demo-subject").contains("openspec/changes/c-scope-two/proposal.md"), "{}", why("demo-subject"));
    assert!(why("other-subject").contains("`c-other-new`"), "{}", why("other-subject"));
    let text = stdout(&run(dir.path(), &["status"]));
    assert!(text.contains("      brief: not approved (openspec/changes/c-scope-two/proposal.md has no `Approved:` line)\n"), "{text}");
    assert!(text.contains("      brief: not approved (change `c-other-new` has no proposal.md)\n"), "{text}");
}

/// context.status.64: the brief is read under canon.yaml's openspec plan
/// sources in config order, a changes dir passed directly and an archived
/// change included; other dialects are not read, and with `sources: []`
/// no proposal.md is found, so no brief reads as approved.
#[test]
fn context_status_64_status_reads_briefs_under_the_configured_openspec_sources() {
    let dir = repo();
    let plans = "plans:\n  sources:\n    - dialect: superpowers\n      root: docs/plans\n    - dialect: openspec\n      root: plans/changes\n    - dialect: openspec\n      root: later\n";
    std::fs::write(dir.path().join("canon.yaml"), format!("{CANON_YAML}{plans}")).unwrap();
    std::fs::create_dir_all(dir.path().join("docs/plans")).unwrap();
    write_policy(dir.path(), FULL_POLICY);
    seed_subject(dir.path(), "live-subject", SubjectStatus::Building, &["c-live"]);
    seed_subject(dir.path(), "old-subject", SubjectStatus::Shipped, &["c-old"]);
    // `plans/changes` is a changes dir passed directly; `c-old` is archived.
    let direct = dir.path().join("plans/changes");
    for (change, body) in [("c-live", "No approval in the first source.\n"), ("archive/c-old", "Approved: Journey 2026-10-01 — \"ok\"\n")] {
        std::fs::create_dir_all(direct.join(change)).unwrap();
        std::fs::write(direct.join(change).join("proposal.md"), format!("## Why\n\nSeeded.\n\n{body}")).unwrap();
    }
    // A later source's approval of the same change does not count: the
    // first source holding the change owns it.
    write_proposal(&dir.path().join("later"), "c-live", &format!("{APPROVED}\n"));
    // The compat-default location is not a configured source here.
    write_proposal(dir.path(), "c-live", &format!("{APPROVED}\n"));

    let report = status_json(dir.path());
    assert_eq!(subject(&report, "live-subject")["brief"]["proposal"], "plans/changes/c-live/proposal.md", "{report}");
    assert_eq!(subject(&report, "live-subject")["brief"]["approved"], false, "{report}");
    assert_eq!(subject(&report, "old-subject")["brief"]["proposal"], "plans/changes/archive/c-old/proposal.md", "{report}");
    assert_eq!(subject(&report, "old-subject")["brief"]["approved"], true, "{report}");

    std::fs::write(dir.path().join("canon.yaml"), format!("{CANON_YAML}plans:\n  sources: []\n")).unwrap();
    let report = status_json(dir.path());
    assert_eq!(subject(&report, "old-subject")["brief"], serde_json::json!({"approved": false, "change": "c-old"}), "{report}");
}

#[test]
fn status_lists_scenarios_no_subject_owns() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Proposed, &[]);
    seed_scenario(dir.path(), "world.demo.01", Some("demo-subject"), Some("failure"));
    seed_scenario(dir.path(), "world.loose.01", None, None);
    seed_scenario(dir.path(), "world.loose.02", None, None);

    let report = status_json(dir.path());
    assert_eq!(report["unowned"], serde_json::json!(["world.loose.01", "world.loose.02"]));
    let next = report["next"].as_array().unwrap();
    let last = next.last().unwrap();
    assert_eq!(last["command"], "canon inventory sync");
    assert!(last["why"].as_str().unwrap().contains("2 scenario(s) carry no @subject tag (first: world.loose.01)"), "{last}");
    assert!(stdout(&run(dir.path(), &["status"])).contains("unowned (2, no @subject): world.loose.01, world.loose.02\n"));
}

#[test]
fn status_warns_when_no_policy_enforces_coverage() {
    let dir = repo();
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Building, &[]);
    seed_scenario(dir.path(), "world.demo.01", Some("demo-subject"), None);

    let report = status_json(dir.path());
    assert_eq!(report["policy"]["present"], false);
    assert!(report["policy"]["spec_coverage"].is_null());
    let warnings = report["warnings"].as_array().unwrap();
    assert!(warnings.iter().any(|w| w.as_str().unwrap().starts_with("no policy: .canon/policy.yaml not found")), "{report}");

    let text = stdout(&run(dir.path(), &["status"]));
    assert!(text.contains("policy: absent\nwarning: no policy: .canon/policy.yaml not found, so `canon gate check` requires no evidence, failure cases or review here\n"), "{text}");

    write_policy(dir.path(), "trust_required:\n  p1: human\n");
    let report = status_json(dir.path());
    assert_eq!(report["policy"]["present"], true);
    assert!(report["warnings"][0].as_str().unwrap().starts_with("policy has no `spec_coverage` section"), "{report}");
}

#[test]
fn status_reports_an_unreadable_canon_yaml_and_still_exits_zero() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("canon.yaml"), "tiers: [not, a, map]\n").unwrap();

    let report = status_json(dir.path());
    assert!(report["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().starts_with("cannot read this repo's ledger: parsing `")), "{report}");
    assert_eq!(commands(&report), ["canon init --check-config"]);
}

#[test]
fn status_is_a_read_that_writes_nothing() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Verifying, &["c-demo"]);
    seed_scenario(dir.path(), "world.demo.01", Some("demo-subject"), None);
    seed_blocker(dir.path(), "c-demo");

    let before = snapshot(dir.path());
    for args in [&["status"][..], &["status", "--json"][..]] {
        let out = run(dir.path(), args);
        assert_eq!(out.status.code(), Some(0), "gaps never fail a read: {}", stderr(&out));
    }
    assert_eq!(snapshot(dir.path()), before, "status must not write any file, including .canon/audit");
}

#[test]
fn status_caps_next_at_five() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    for n in 1..=7 {
        seed_approved_subject(dir.path(), &format!("subject-{n}"), SubjectStatus::Building);
    }
    seed_scenario(dir.path(), "world.demo.01", None, None);

    let report = status_json(dir.path());
    assert_eq!(report["next"].as_array().unwrap().len(), 5);
    assert_eq!(report["nextOmitted"], 3, "7 subject steps + 1 unowned step, 5 shown");
    assert!(stdout(&run(dir.path(), &["status"])).contains("  … 3 more once these are done\n"));
}

#[test]
fn status_on_an_unsynced_corpus_points_at_inventory_sync() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Verifying, &[]);
    seed_subject(dir.path(), "other-subject", SubjectStatus::Building, &[]);
    let feature = dir.path().join("specs/features/kind=feature/area=world/demo.feature");
    std::fs::create_dir_all(feature.parent().unwrap()).unwrap();
    std::fs::write(&feature, "Feature: demo\n\n  @subject:demo-subject\n  @world.demo.01\n  Scenario: A demo\n    Given a step\n").unwrap();

    let report = status_json(dir.path());
    assert_eq!(commands(&report), ["canon inventory sync"]);
    assert!(report["warnings"].as_array().unwrap().iter().any(|w| w == "the ledger holds no scenario records, so every subject reads as owning none"), "{report}");
}

/// 0.14 acceptance rerun G10: with no `.feature` scenario at all, a sync
/// has nothing to index, so status says to write features and each
/// subject's scenarios instead of `canon inventory sync`.
#[test]
fn status_with_no_feature_scenarios_points_at_writing_them() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Building, &[]);

    let report = status_json(dir.path());
    let commands = commands(&report);
    assert_eq!(
        commands,
        ["canon feature new <area>.<surface> --title \"<feature>\"", "canon scenario new <area>.<surface>.01 --title \"<behavior>\" --subject demo-subject --case happy"],
        "{report}"
    );
    assert!(!commands.iter().any(|c| c.contains("inventory sync")), "{report}");
    assert!(!report["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("no scenario records")), "{report}");
    for command in &commands {
        assert_suggestion_parses_and_names_the_unit(command);
    }
}

/// context.status.80: the policy level is the level line's level when it
/// names a known one, else the level `spec_coverage` matches exactly,
/// else `custom`; `null` without a policy.
#[test]
fn context_status_80_status_names_the_policy_level_from_its_line_else_its_sections_else_custom() {
    let light = "spec_coverage:\n  require_evidence: true\n";
    let standard = "spec_coverage:\n  require_evidence: true\n  require_cases: [failure]\n";
    for (policy, level) in [
        (format!("# canon policy level: light\n{FULL_POLICY}"), "light"),
        (format!("# a note\n#\n# canon policy level: standard\n{light}"), "standard"),
        (format!("# canon policy level: extreme\n{FULL_POLICY}"), "strict"),
        (FULL_POLICY.to_string(), "strict"),
        (standard.to_string(), "standard"),
        (light.to_string(), "light"),
        (format!("{light}  scope: [building]\n"), "custom"),
        (format!("{FULL_POLICY}  require_audit: {{rate: 0.5}}\n"), "custom"),
        (format!("{standard}  require_review: {{distinct_actor: false}}\n"), "custom"),
        ("spec_coverage:\n  require_evidence: false\n".to_string(), "custom"),
        (format!("{light}# canon policy level: strict\n"), "light"),
    ] {
        let dir = repo();
        write_policy(dir.path(), &policy);
        assert_eq!(status_json(dir.path())["policy"]["level"], level, "{policy}");
        let human = stdout(&run(dir.path(), &["status"]));
        assert!(human.contains(&format!("\n  level: {level}")), "{policy}\n{human}");
    }
    let dir = repo();
    assert_eq!(status_json(dir.path())["policy"]["level"], Value::Null);
    assert!(!stdout(&run(dir.path(), &["status"])).contains("level:"));
}
