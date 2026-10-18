//! Integration tests for `canon status [--repo] [--json]` (0.14 D7),
//! invoking the built `canon` binary against offline git-tier fixtures
//! in a tmpdir. Records are planted through the same `GitTier` root the
//! CLI reads (`tests/subject.rs`'s shape), so each test controls exactly
//! which scenarios, evidence, reviews and findings exist.

use std::path::Path;
use std::process::{Command, Output};

use canon_model::{
    Actor, AuditResult, ChangeId, Envelope, EvidenceAudit, EvidenceRecord, EvidenceVerdict, Finding, FindingSeverity, ProjectId, ProvenanceRef, RecordKind, Review, RoleId, Scenario,
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
    ledger(repo).write(&pinned(repo, review)).unwrap();
}

fn seed_blocker(repo: &Path, change: &str) {
    let envelope = Envelope::new(1, RecordKind::Finding, Utc::now(), actor("reviewer-2", "reviewer"));
    let finding = Finding::new(envelope, ChangeId::parse(change).unwrap(), 1, 1, FindingSeverity::Blocker, "reviewer-2", "unbounded subprocess wait");
    ledger(repo).write(&finding).unwrap();
}

/// The approval line the agent records once the human answers (0.16 D1).
const APPROVED: &str = "Approved: Journey 2026-10-11 — \"승인. 네 가정대로 진행\"";

/// The one `next` step status addresses to a human, for subject `id`
/// whose newest adopted change is `change` (0.16 M7: stop and report).
fn brief_step(id: &str, change: &str) -> String {
    format!("stop and report the brief in .canon/changes/{change}/brief.md; build and attest nothing for {id} until the human answers (one answer approves brief and policy)")
}

/// The prefix every human step's command starts with.
const BRIEF_STEP_PREFIX: &str = "stop and report the brief in ";

/// Canon's own `brief.md` for `change` at
/// `<repo>/.canon/changes/<change>/` (0.17 D3), with `extra` after the
/// `## Why` paragraph.
fn write_brief(repo: &Path, change: &str, extra: &str) {
    let dir = repo.join(".canon/changes").join(change);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("brief.md"), format!("# {change}\n\n## Why\n\nSeeded.\n\n{extra}")).unwrap();
}

/// An external openspec `proposal.md` for `change` at
/// `<root>/openspec/changes/<change>/`, with `extra` after the `## Why`
/// paragraph.
fn write_external_proposal(root: &Path, change: &str, extra: &str) {
    let dir = root.join("openspec/changes").join(change);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("proposal.md"), format!("# {change}\n\n## Why\n\nSeeded.\n\n{extra}")).unwrap();
}

/// A subject adopting `c-<id>`, whose brief.md carries an approval.
fn seed_approved_subject(repo: &Path, id: &str, status: SubjectStatus) {
    let change = format!("c-{id}");
    seed_subject(repo, id, status, &[&change]);
    write_brief(repo, &change, &format!("{APPROVED}\n"));
}

fn status_json(repo: &Path) -> Value {
    let out = run(repo, &["status", "--json"]);
    assert_eq!(out.status.code(), Some(0), "status always exits 0; stderr: {}", stderr(&out));
    let report: Value = serde_json::from_str(&stdout(&out)).expect("status --json prints one JSON object");
    for step in report["next"].as_array().unwrap() {
        match step["actor"].as_str() {
            Some("agent") => {}
            Some("human") => assert!(
                step["command"].as_str().unwrap().starts_with(BRIEF_STEP_PREFIX),
                "the brief is the only step status addresses to a human: {step}"
            ),
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
    assert_eq!(report["statusVersion"], 3);
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
    // settings.
    let header = format!(
        "canon {}\npolicy: .canon/policy.yaml\n  spec_coverage: require_evidence=true scope=<every scenario>\n  require_cases: failure\n  require_review: scope=verifying, shipped distinct_actor=true block_on_findings=true\n  level: strict\n{UNAPPROVED}\n\n",
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
    // Attest world.demo.01 again after its review, with another verdict: the
    // evidence now says something the review did not see.
    seed_evidence(dir.path(), "world.demo.01", EvidenceVerdict::NotApplicable);

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
            "canon scenario new world.demo.03 --title \"<what happens on this path>\" --subject-id demo-subject --case failure --actor-id <unit>",
            "canon evidence add --scenario-id world.demo.02 --project-id demo --kind test-run --role implementer --actor-id <unit> --session-id <session> --verdict faithful --ref \"<test command>\"",
            "canon finding close --change-id c-demo-subject --round 1 --seq 1 --disposition fixed --resolution-sha <sha> --actor-id <unit> --session-id <session>",
            "canon review add --project-id demo --scenario-id world.demo.01 --reviewer <reviewer> --actor-id <reviewer> --session-id <review-session> --role reviewer --pin <sha> --original-spec-ref \"<feature file>\"",
            "canon scenario new <area>.<surface>.01 --title \"<behavior>\" --subject-id empty-subject --case happy --actor-id <unit>",
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
/// without `--project-id`. Status names the owning root exactly, and the
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
    let authored = run(dir.path(), &["scenario", "new", "world.demo.01", "--title", "seeded", "--subject-id", "demo-subject", "--case", "happy", "--project-id", "demo"]);
    assert!(authored.status.success(), "{}", String::from_utf8_lossy(&authored.stderr));

    let report = status_json(dir.path());
    let next = commands(&report);
    let case_step = "canon scenario new world.demo.02 --title \"<what happens on this path>\" --subject-id demo-subject --case failure --actor-id <unit> --project-id demo";
    assert_eq!(next[0], case_step, "{report}");
    assert!(next.contains(&"canon scenario new <area>.<surface>.01 --title \"<behavior>\" --subject-id empty-subject --case happy --actor-id <unit> --project-id <root-id>".to_string()), "{report}");
    let empty = report["next"].as_array().unwrap().iter().find(|n| n["command"].as_str().unwrap().contains("empty-subject")).unwrap();
    assert!(empty["why"].as_str().unwrap().ends_with("(spec roots: demo, other)"), "{empty}");

    let bin_dir = Path::new(env!("CARGO_BIN_EXE_canon")).parent().unwrap().to_path_buf();
    let path = format!("{}:{}", bin_dir.display(), std::env::var("PATH").unwrap_or_default());
    let out = Command::new("sh").arg("-c").arg(case_step.replace("<unit>", "unit-a")).env("PATH", path).current_dir(dir.path()).output().unwrap();
    assert!(out.status.success(), "the suggested command must run once its placeholder is filled: {}", stderr(&out));
    let written = stdout(&out);
    assert!(written.contains("wrote `@world.demo.02` to ") && written.contains("/specs/demo/"), "written under the demo root: {written}");
}

/// One spec root: no `--project-id`, which `canon scenario new` would not need.
#[test]
fn status_omits_the_spec_root_flag_with_one_root() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Building, &[]);
    seed_scenario(dir.path(), "world.demo.01", Some("demo-subject"), Some("happy"));
    let report = status_json(dir.path());
    assert!(commands(&report).iter().filter(|c| c.starts_with("canon scenario new") || c.starts_with("canon feature new")).all(|c| !c.contains("--project-id ")), "{report}");
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
    write_brief(dir.path(), "c-demo-subject", &format!("{APPROVED}\n"));

    let report = status_json(dir.path());
    assert_eq!(commands(&report), ["canon subject status demo-subject --to shipped --actor-id <unit>"]);
    let step = &report["next"][0];
    assert_eq!(step["actor"], "agent", "{step}");
    let why = step["why"].as_str().unwrap();
    assert!(why.contains("ship it yourself unless the brief says the human ships"), "{why}");
    assert!(!why.contains("ask"), "shipping asks nobody: {why}");
    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped"]);
    assert!(out.status.success(), "the suggested transition must pass its gates: {}", stderr(&out));
    let report = status_json(dir.path());
    assert_eq!(subject(&report, "demo-subject")["status"], "shipped");
    assert_eq!(subject(&report, "demo-subject")["brief"]["approved"], true, "a shipped subject still shows its brief");
    assert_eq!(report["next"], serde_json::json!([]));
    assert!(stdout(&run(dir.path(), &["status"])).contains("next: nothing"));
}

/// context.status.70 (0.18 D3): a shipped subject whose gate went red
/// after it shipped (an unattested, unreviewed new scenario and an open
/// blocker) is flagged with what the ship guard would refuse, per
/// failure class, and its one step is the reopen command, which parses
/// and, followed, reopens it; the flag and the reopen step are then gone.
#[test]
fn context_status_70_a_shipped_subject_whose_gate_is_red_is_flagged_and_its_step_is_the_reopen() {
    let dir = repo();
    ship_ready(dir.path());
    write_brief(dir.path(), "c-demo-subject", &format!("{APPROVED}\n"));
    assert!(run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped"]).status.success());
    assert!(subject(&status_json(dir.path()), "demo-subject").get("red").is_none(), "a green shipped subject is not flagged");

    seed_blocker(dir.path(), "c-demo-subject");
    seed_scenario(dir.path(), "world.demo.03", Some("demo-subject"), Some("failure"));
    let report = status_json(dir.path());
    let red = "1 uncovered-cell (first: world.demo.03), 1 unreviewed-promotion (first: world.demo.03), 1 open-blocker (first: c-demo-subject#1.1)";
    assert_eq!(subject(&report, "demo-subject")["red"], serde_json::json!(red.split(", ").collect::<Vec<_>>()));
    let reopen = "canon subject status demo-subject --to verifying --reason \"<why it reopens>\" --actor-id <unit>";
    assert_eq!(commands(&report), [reopen]);
    assert_eq!(
        report["next"][0]["why"],
        format!("demo-subject is shipped but its gate is red ({red}); reopen it, keep the follow-up in its approved change `c-demo-subject` (never `canon change new`), then scenario, evidence and review, and ship it again")
    );
    let human = stdout(&run(dir.path(), &["status"]));
    assert!(human.contains(&format!("      gate: red — {red}; reopen with `{reopen}`\n")), "{human}");

    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "verifying", "--reason", "follow-up: a blocker and a new failure path", "--actor-id", "impl-agent"]);
    assert!(out.status.success(), "the suggested reopen goes through: {}", stderr(&out));
    let report = status_json(dir.path());
    assert_eq!(subject(&report, "demo-subject")["status"], "verifying");
    assert!(subject(&report, "demo-subject").get("red").is_none());
    assert!(!commands(&report).iter().any(|c| c.contains("--reason")), "{report}");
}

/// context.status.70: the red flag reads the ship guard's own inputs. A
/// refuted audit (`false-success`) makes a shipped subject red; an open
/// blocker under a `require_review` without `block_on_findings` does
/// not, as it does not refuse shipping.
#[test]
fn context_status_70_the_red_flag_follows_the_ship_guard_audits_in_blockers_only_when_they_block() {
    let dir = repo();
    ship_ready(dir.path());
    write_policy(dir.path(), "spec_coverage:\n  require_evidence: true\n  require_cases: [failure]\n  require_review:\n    block_on_findings: false\n");
    write_brief(dir.path(), "c-demo-subject", &format!("{APPROVED}\n"));
    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped"]);
    assert!(out.status.success(), "{}", stderr(&out));

    seed_blocker(dir.path(), "c-demo-subject");
    let report = status_json(dir.path());
    assert!(subject(&report, "demo-subject").get("red").is_none(), "a blocker that does not refuse shipping does not make the gate red: {report}");
    assert_eq!(subject(&report, "demo-subject")["openBlockers"], 1, "status still counts it");

    let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), actor("auditor-1", "auditor"));
    let audit = EvidenceRecord::new(envelope, None, Some(ScenarioId::parse("world.demo.01").unwrap()), None, EvidenceVerdict::Divergent)
        .with_project_id(ProjectId::parse("demo").unwrap())
        .with_audit(EvidenceAudit { evaluator_id: "auditor-1".into(), evaluator_version: "1.0.0".into(), evaluator_digest: "a".repeat(64), result: AuditResult::Refuted });
    ledger(dir.path()).write(&audit).unwrap();
    let report = status_json(dir.path());
    assert_eq!(subject(&report, "demo-subject")["red"], serde_json::json!(["1 false-success (first: world.demo.01)"]), "{report}");
    assert_eq!(commands(&report)[0], "canon subject status demo-subject --to verifying --reason \"<why it reopens>\" --actor-id <unit>");
    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "verifying", "--reason", "follow-up: the audit refuted run.01", "--actor-id", "impl-agent"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let out = run(dir.path(), &["subject", "status", "demo-subject", "--to", "shipped"]);
    assert_eq!(out.status.code(), Some(1), "the ship guard refuses on the same gap: {}", stderr(&out));
    assert!(stderr(&out).contains("false-success world.demo.01"), "{}", stderr(&out));
}

/// context.status.61: a ship-ready subject whose newest adopted change's
/// brief.md carries no `Approved:` line gets the one human step and no
/// move-on; a subject with no change at all has no brief to answer, so it
/// gets no human step and its brief line is an advisory.
#[test]
fn context_status_61_an_unapproved_brief_is_the_only_human_step_and_blocks_the_move_on() {
    let dir = repo();
    ship_ready(dir.path());
    write_brief(dir.path(), "c-demo-subject", "Status: draft, awaiting the human.\n");
    seed_subject(dir.path(), "bare-subject", SubjectStatus::Building, &[]);
    seed_scenario(dir.path(), "world.bare.01", Some("bare-subject"), Some("failure"));
    seed_evidence(dir.path(), "world.bare.01", EvidenceVerdict::Faithful);

    let report = status_json(dir.path());
    assert_eq!(human_steps(&report), ["demo-subject"], "{report}");
    assert!(commands(&report).iter().all(|c| !c.starts_with("canon subject status demo-subject")), "no move-on without an approved brief: {report}");
    assert!(
        report["next"].as_array().unwrap().iter().filter(|n| n["why"].as_str().unwrap().starts_with("demo-subject")).all(|n| n["actor"] == "human"),
        "the human step is the subject's only step: nothing is built or attested before the answer: {report}"
    );
    assert!(
        report["next"].as_array().unwrap().iter().any(|n| n["actor"] == "agent" && n["why"].as_str().unwrap().starts_with("bare-subject")),
        "nothing to answer withholds nothing: the subject with no change gets the agent's next step: {report}"
    );
    let whys: Vec<&str> = report["next"].as_array().unwrap().iter().filter(|n| n["actor"] == "human").map(|n| n["why"].as_str().unwrap()).collect();
    assert!(whys[0].contains(".canon/changes/c-demo-subject/brief.md"), "{}", whys[0]);
    for why in &whys {
        assert!(why.contains("`canon brief approve --change-id ") && why.contains("--by <the human> --note"), "{why}");
        assert!(why.contains("`canon evidence add` refuses demo-subject's scenarios and tasks"), "{why}");
    }
    let demo = subject(&report, "demo-subject");
    assert_eq!(
        demo["brief"],
        serde_json::json!({"approved": false, "enforced": true, "change": "c-demo-subject", "brief": ".canon/changes/c-demo-subject/brief.md", "briefExists": true})
    );
    assert_eq!(subject(&report, "bare-subject")["brief"], serde_json::json!({"approved": false, "enforced": false, "briefExists": false}));

    let text = stdout(&run(dir.path(), &["status"]));
    assert!(text.contains("      brief: not approved (.canon/changes/c-demo-subject/brief.md has no `Approved:` line)\n"), "{text}");
    assert!(text.contains("      brief: not approved (no adopted change; advisory, nothing to approve yet)\n"), "{text}");
    assert!(text.contains(&format!("\n  human: {}\n", brief_step("demo-subject", "c-demo-subject"))), "{text}");
}

/// context.status.60: the brief is approved when the subject's newest
/// adopted change's canon brief.md has a line starting `Approved:` (indent
/// allowed); status shows that change, its brief.md and the recorded
/// words, for building, verifying and shipped subjects only.
#[test]
fn context_status_60_status_reads_the_approval_from_the_changes_brief() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Building, &["c-first", "c-second"]);
    write_brief(dir.path(), "c-first", "No approval here.\n");
    write_brief(dir.path(), "c-second", &format!("  {APPROVED}\n"));
    seed_approved_subject(dir.path(), "done-subject", SubjectStatus::Shipped);
    seed_subject(dir.path(), "early-subject", SubjectStatus::Specced, &[]);

    let report = status_json(dir.path());
    let words = APPROVED.strip_prefix("Approved: ").unwrap();
    assert_eq!(
        subject(&report, "demo-subject")["brief"],
        serde_json::json!({"approved": true, "enforced": false, "change": "c-second", "brief": ".canon/changes/c-second/brief.md", "briefExists": true, "approval": words, "legacyApproval": true})
    );
    assert_eq!(
        subject(&report, "done-subject")["brief"],
        serde_json::json!({"approved": true, "enforced": false, "change": "c-done-subject", "brief": ".canon/changes/c-done-subject/brief.md", "briefExists": true, "approval": words, "legacyApproval": true}),
        "a shipped subject shows its brief in full"
    );
    assert!(subject(&report, "early-subject").get("brief").is_none(), "a specced subject carries no brief field");
    assert!(human_steps(&report).is_empty(), "{report}");
    let text = stdout(&run(dir.path(), &["status"]));
    assert!(text.contains(&format!("      brief: approved in .canon/changes/c-second/brief.md — {words}\n")), "{text}");
    assert!(text.contains(&format!("      brief: approved in .canon/changes/c-done-subject/brief.md — {words}\n")), "{text}");
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
    // One building subject adopting `c-<case>`, whose brief.md is `body`.
    let report_for = |case: &str, body: &str| {
        let dir = repo();
        write_policy(dir.path(), FULL_POLICY);
        seed_subject(dir.path(), "demo-subject", SubjectStatus::Building, &[&format!("c-{case}")]);
        write_brief(dir.path(), &format!("c-{case}"), body);
        let report = status_json(dir.path());
        assert_eq!(report["nextOmitted"], 0, "{case}: the report stays under the next cap: {report}");
        report
    };

    for (case, body) in &refused {
        let report = report_for(case, body);
        assert_eq!(subject(&report, "demo-subject")["brief"]["approved"], false, "{case} must not approve:\n{body}");
        let humans: Vec<&Value> = report["next"].as_array().unwrap().iter().filter(|n| n["actor"] == "human").collect();
        assert_eq!(humans.len(), 1, "{case}: exactly one human step: {report}");
        assert_eq!(humans[0]["command"], brief_step("demo-subject", &format!("c-{case}")), "{case}: {report}");
        assert!(humans[0]["why"].as_str().unwrap().starts_with("demo-subject: "), "{case}: the step is the subject's: {report}");
        assert!(humans[0]["why"].as_str().unwrap().contains(&format!(".canon/changes/c-{case}/brief.md")), "{case}: {report}");
    }
    for (case, body) in &accepted {
        let report = report_for(case, body);
        assert_eq!(subject(&report, "demo-subject")["brief"]["approved"], true, "{case} must approve:\n{body}");
        assert!(human_steps(&report).is_empty(), "{case}: an approved brief asks no human: {report}");
    }
}

/// context.status.65: a fix stays inside the approved change, so
/// a newer adopted change is a new scope. An approval on an older change
/// does not cover it: status names the newest change, asks for the human,
/// and withholds the ship step.
#[test]
fn context_status_65_an_older_changes_approval_does_not_cover_a_newer_change() {
    let dir = repo();
    ship_ready(dir.path());
    write_brief(dir.path(), "c-demo-subject", &format!("{APPROVED}\n"));
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Verifying, &["c-demo-subject", "c-scope-two"]);
    write_brief(dir.path(), "c-scope-two", "A new scope, not yet approved.\n");
    seed_subject(dir.path(), "other-subject", SubjectStatus::Building, &["c-other-old", "c-other-new"]);
    write_brief(dir.path(), "c-other-old", &format!("{APPROVED}\n"));

    let report = status_json(dir.path());
    assert_eq!(
        subject(&report, "demo-subject")["brief"],
        serde_json::json!({"approved": false, "enforced": true, "change": "c-scope-two", "brief": ".canon/changes/c-scope-two/brief.md", "briefExists": true})
    );
    assert_eq!(
        subject(&report, "other-subject")["brief"],
        serde_json::json!({"approved": false, "enforced": false, "change": "c-other-new", "briefExists": false}),
        "no brief for the newest change"
    );
    assert_eq!(human_steps(&report), ["demo-subject"], "a change with no brief has no brief to answer: {report}");
    assert!(!commands(&report).iter().any(|c| c.starts_with("canon subject status demo-subject")), "an older approval never unlocks the ship step: {report}");
    let why = |id: &str| report["next"].as_array().unwrap().iter().find(|n| n["actor"] == "human" && n["why"].as_str().unwrap().starts_with(id)).unwrap()["why"].as_str().unwrap().to_string();
    assert!(why("demo-subject").contains(".canon/changes/c-scope-two/brief.md"), "{}", why("demo-subject"));
    let text = stdout(&run(dir.path(), &["status"]));
    assert!(text.contains("      brief: not approved (.canon/changes/c-scope-two/brief.md has no `Approved:` line)\n"), "{text}");
    assert!(text.contains("      brief: not approved (change `c-other-new` has no brief; advisory, nothing to approve yet)\n"), "{text}");
}

/// context.status.64 (0.17 D3): an external openspec change with no
/// canon brief is enforced, its own `Approved:` line is never read, and
/// status names the canon file to create with its `Source:` pointer; the
/// external proposal is found under canon.yaml's openspec sources in
/// config order (a changes dir passed directly, an archived change
/// included, other dialects not read). A canon brief pointing at an
/// external change carries its approval, with or without plan sources,
/// and status never edits the external document.
#[test]
fn context_status_64_an_external_change_is_answered_in_a_canon_brief_that_points_at_it() {
    let dir = repo();
    let plans = "plans:\n  sources:\n    - dialect: superpowers\n      root: docs/plans\n    - dialect: openspec\n      root: plans/changes\n    - dialect: openspec\n      root: later\n";
    std::fs::write(dir.path().join("canon.yaml"), format!("{CANON_YAML}{plans}")).unwrap();
    std::fs::create_dir_all(dir.path().join("docs/plans")).unwrap();
    write_policy(dir.path(), FULL_POLICY);
    seed_subject(dir.path(), "live-subject", SubjectStatus::Building, &["c-live"]);
    seed_subject(dir.path(), "old-subject", SubjectStatus::Shipped, &["c-old"]);
    // `plans/changes` is a changes dir passed directly; `c-old` is
    // archived. Both external proposals carry a 0.16-style approval,
    // which is not read.
    let direct = dir.path().join("plans/changes");
    for change in ["c-live", "archive/c-old"] {
        std::fs::create_dir_all(direct.join(change)).unwrap();
        std::fs::write(direct.join(change).join("proposal.md"), format!("## Why\n\nSeeded.\n\n{APPROVED}\n")).unwrap();
    }
    write_external_proposal(&dir.path().join("later"), "c-live", &format!("{APPROVED}\n"));
    let external_before = std::fs::read(direct.join("c-live/proposal.md")).unwrap();
    // The old change's canon brief points at its external plan and holds
    // the approval.
    write_brief(dir.path(), "c-old", &format!("Source: plans/changes/archive/c-old/proposal.md\n\n{APPROVED}\n"));

    let report = status_json(dir.path());
    assert_eq!(
        subject(&report, "live-subject")["brief"],
        serde_json::json!({"approved": false, "enforced": true, "change": "c-live", "brief": ".canon/changes/c-live/brief.md", "briefExists": false, "source": "plans/changes/c-live/proposal.md"}),
        "{report}"
    );
    let words = APPROVED.strip_prefix("Approved: ").unwrap();
    assert_eq!(
        subject(&report, "old-subject")["brief"],
        serde_json::json!({"approved": true, "enforced": false, "change": "c-old", "brief": ".canon/changes/c-old/brief.md", "briefExists": true, "source": "plans/changes/archive/c-old/proposal.md", "approval": words, "legacyApproval": true}),
        "{report}"
    );
    let human: Vec<&Value> = report["next"].as_array().unwrap().iter().filter(|n| n["actor"] == "human").collect();
    assert_eq!(human.len(), 1, "{report}");
    assert_eq!(
        human[0]["command"],
        "stop and report the brief in plans/changes/c-live/proposal.md; build and attest nothing for live-subject until the human answers (one answer approves brief and policy)"
    );
    let why = human[0]["why"].as_str().unwrap();
    assert!(
        why.contains(".canon/changes/c-live/brief.md does not exist yet")
            && why.contains("create .canon/changes/c-live/brief.md holding a `Source: plans/changes/c-live/proposal.md` line")
            && why.contains("`canon brief approve --change-id c-live --by <the human> --note"),
        "{why}"
    );
    let text = stdout(&run(dir.path(), &["status"]));
    assert!(text.contains("      brief: not approved (.canon/changes/c-live/brief.md does not exist yet: change `c-live` comes from the external plan plans/changes/c-live/proposal.md"), "{text}");
    assert!(text.contains(&format!("      brief: approved in .canon/changes/c-old/brief.md — {words}\n")), "{text}");
    assert_eq!(std::fs::read(direct.join("c-live/proposal.md")).unwrap(), external_before, "status never edits an external plan");

    std::fs::write(dir.path().join("canon.yaml"), format!("{CANON_YAML}plans:\n  sources: []\n")).unwrap();
    let report = status_json(dir.path());
    assert_eq!(subject(&report, "live-subject")["brief"], serde_json::json!({"approved": false, "enforced": false, "change": "c-live", "briefExists": false}), "{report}");
    assert_eq!(subject(&report, "old-subject")["brief"]["approved"], true, "the canon brief needs no plan source: {report}");
}

/// context.status.04: untagged scenarios are listed as unowned, and the
/// last step is tagging them and running `canon status` again (it
/// re-indexes the edit), never `canon inventory sync`.
#[test]
fn context_status_04_status_lists_scenarios_no_subject_owns() {
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
    assert_eq!(last["command"], "canon status");
    let why = last["why"].as_str().unwrap();
    assert!(why.contains("2 scenario(s) carry no @subject tag (first: world.loose.01); add `@subject:<id>` above each in its .feature file"), "{last}");
    assert!(!why.contains("sync"), "status re-indexes the edit itself: {last}");
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

/// context.status.09: a corpus the refresh cannot index (here, headers
/// with no provenance comment) leaves the index empty; status says so
/// once and names `canon inventory sync` for the reasons.
#[test]
fn context_status_09_status_on_an_unindexable_corpus_points_at_inventory_sync() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Verifying, &[]);
    seed_subject(dir.path(), "other-subject", SubjectStatus::Building, &[]);
    let feature = dir.path().join(".canon/specs/kind=feature/area=world/demo.feature");
    std::fs::create_dir_all(feature.parent().unwrap()).unwrap();
    std::fs::write(&feature, "Feature: demo\n\n  @subject:demo-subject\n  @world.demo.01\n  Scenario: A demo\n    Given a step\n").unwrap();

    let out = run(dir.path(), &["status", "--json"]);
    assert!(stderr(&out).contains("NOT refreshed"), "status's own refresh refused the corpus: {}", stderr(&out));
    let report = status_json(dir.path());
    assert_eq!(commands(&report), ["canon inventory sync"]);
    let why = report["next"][0]["why"].as_str().unwrap();
    assert!(why.contains("the automatic refresh could not index them") && why.contains("format violation or duplicate id"), "{why}");
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
        ["canon feature new <area>.<surface> --title \"<feature>\" --actor-id <unit>", "canon scenario new <area>.<surface>.01 --title \"<behavior>\" --subject-id demo-subject --case happy --actor-id <unit>"],
        "{report}"
    );
    assert!(!commands.iter().any(|c| c.contains("inventory sync")), "{report}");
    assert!(!report["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("no scenario records")), "{report}");
    for command in &commands {
        assert_suggestion_parses_and_names_the_unit(command);
    }
}

/// context.status.66: a `proposed` subject whose newest adopted change
/// has an unapproved brief.md shows its brief, and the human step is
/// its only step: nothing starts the work before the human answers.
/// Approving the brief brings the feature and scenario steps back.
#[test]
fn context_status_66_a_proposed_subject_with_an_unapproved_brief_asks_only_the_human() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Proposed, &["c-demo-subject"]);
    write_brief(dir.path(), "c-demo-subject", "Status: draft, awaiting the human.\n");

    let report = status_json(dir.path());
    assert_eq!(
        subject(&report, "demo-subject")["brief"],
        serde_json::json!({"approved": false, "enforced": true, "change": "c-demo-subject", "brief": ".canon/changes/c-demo-subject/brief.md", "briefExists": true})
    );
    assert_eq!(human_steps(&report), ["demo-subject"], "{report}");
    assert_eq!(report["next"].as_array().unwrap().len(), 1, "the human step is the only step: {report}");
    let text = stdout(&run(dir.path(), &["status"]));
    assert!(text.contains("      brief: not approved (.canon/changes/c-demo-subject/brief.md has no `Approved:` line)\n"), "{text}");
    assert!(text.contains(&format!("\n  human: {}\n", brief_step("demo-subject", "c-demo-subject"))), "{text}");
    assert!(!text.contains("canon feature new") && !text.contains("canon scenario new"), "{text}");

    write_brief(dir.path(), "c-demo-subject", &format!("{APPROVED}\n"));
    let report = status_json(dir.path());
    assert_eq!(subject(&report, "demo-subject")["brief"]["approved"], true, "{report}");
    assert!(human_steps(&report).is_empty(), "{report}");
    assert_eq!(
        commands(&report),
        ["canon feature new <area>.<surface> --title \"<feature>\" --actor-id <unit>", "canon scenario new <area>.<surface>.01 --title \"<behavior>\" --subject-id demo-subject --case happy --actor-id <unit>"],
        "{report}"
    );
}

/// context.status.67: a `proposed` subject with no brief to answer (no
/// adopted change, or a change with no brief) keeps the steps it
/// had, with no brief field; and showing a `proposed` subject's brief
/// adds no transition check: its `proposed → specced` move succeeds
/// with an unapproved brief and prints nothing about it.
#[test]
fn context_status_67_a_proposed_subject_with_no_brief_keeps_its_steps_and_its_brief_adds_no_transition_check() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    seed_subject(dir.path(), "bare-subject", SubjectStatus::Proposed, &[]);
    seed_subject(dir.path(), "draft-subject", SubjectStatus::Proposed, &["c-draft"]);

    let report = status_json(dir.path());
    for id in ["bare-subject", "draft-subject"] {
        assert!(subject(&report, id).get("brief").is_none(), "{id} has no brief to show: {report}");
    }
    assert!(human_steps(&report).is_empty(), "{report}");
    assert_eq!(
        commands(&report),
        [
            "canon feature new <area>.<surface> --title \"<feature>\" --actor-id <unit>",
            "canon scenario new <area>.<surface>.01 --title \"<behavior>\" --subject-id bare-subject --case happy --actor-id <unit>",
            "canon scenario new <area>.<surface>.01 --title \"<behavior>\" --subject-id draft-subject --case happy --actor-id <unit>",
        ],
        "{report}"
    );
    assert!(!stdout(&run(dir.path(), &["status"])).contains("brief:"));

    seed_subject(dir.path(), "held-subject", SubjectStatus::Proposed, &["c-held"]);
    write_brief(dir.path(), "c-held", "Status: draft, awaiting the human.\n");
    seed_scenario(dir.path(), "world.held.01", Some("held-subject"), Some("failure"));
    let out = run(dir.path(), &["subject", "status", "held-subject", "--to", "specced", "--actor-id", "lead"]);
    assert!(out.status.success(), "the move to specced does not read the brief: {}", stderr(&out));
    assert!(!stderr(&out).contains("brief"), "{}", stderr(&out));
}

/// context.status.68: readers refresh the scenario index themselves, so
/// no step names `canon inventory sync`, `canon format` or `canon ingest
/// plans`: the empty subject's step just says to specify it, and the
/// unowned step is `canon status`, after tagging.
#[test]
fn context_status_68_no_step_names_a_sync_while_readers_refresh() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    seed_subject(dir.path(), "demo-subject", SubjectStatus::Building, &[]);
    seed_subject(dir.path(), "empty-subject", SubjectStatus::Building, &[]);
    seed_scenario(dir.path(), "world.demo.01", Some("demo-subject"), Some("failure"));
    seed_scenario(dir.path(), "world.loose.01", None, None);

    let report = status_json(dir.path());
    assert_eq!(report["nextOmitted"], 0, "{report}");
    let steps = report["next"].as_array().unwrap();
    let empty = steps.iter().find(|n| n["command"].as_str().unwrap().contains("--subject-id empty-subject")).unwrap_or_else(|| panic!("{report}"));
    assert_eq!(empty["why"], "empty-subject owns no scenario; specify its behavior, one scenario per behavior", "{report}");
    let unowned = steps.last().unwrap();
    assert_eq!(unowned["command"], "canon status", "{report}");
    assert!(unowned["why"].as_str().unwrap().contains("add `@subject:<id>` above each in its .feature file"), "{unowned}");
    for step in steps {
        let line = format!("{} {}", step["command"].as_str().unwrap(), step["why"].as_str().unwrap());
        for bookkeeping in ["inventory sync", "canon format", "ingest plans"] {
            assert!(!line.contains(bookkeeping), "no step names `{bookkeeping}`: {step}");
        }
    }
}

/// context.status.69: the evidence step's reason names `canon gate
/// promote` only when `canon.yaml` keeps writes staged (`manual`, or no
/// `write:` key); under `auto` the attestation is final.
#[test]
fn context_status_69_the_evidence_step_names_gate_promote_only_when_writes_stay_staged() {
    let dir = repo();
    write_policy(dir.path(), FULL_POLICY);
    seed_approved_subject(dir.path(), "demo-subject", SubjectStatus::Building);
    seed_scenario(dir.path(), "world.demo.01", Some("demo-subject"), Some("failure"));
    let evidence_why = |dir: &Path| {
        let report = status_json(dir);
        let step = report["next"].as_array().unwrap().iter().find(|n| n["command"].as_str().unwrap().starts_with("canon evidence add")).cloned();
        step.unwrap_or_else(|| panic!("{report}"))["why"].as_str().unwrap().to_string()
    };

    let staged = "demo-subject: 1 of 1 scenario(s) have no evidence; attest each, then `canon gate promote` (`write.promote: manual`)";
    assert_eq!(evidence_why(dir.path()), staged, "no write section is manual");
    std::fs::write(dir.path().join("canon.yaml"), format!("{CANON_YAML}write:\n  promote: manual\n")).unwrap();
    assert_eq!(evidence_why(dir.path()), staged);
    std::fs::write(dir.path().join("canon.yaml"), format!("{CANON_YAML}write:\n  promote: auto\n")).unwrap();
    assert_eq!(evidence_why(dir.path()), "demo-subject: 1 of 1 scenario(s) have no evidence; attest each");
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
