//! Integration tests for `canon init [--repo <dir>]` + `canon init
//! --check-config [--repo <dir>]` (s19 `canon-init-scaffold` spec) —
//! invokes the actually-built `canon` binary (`env!("CARGO_BIN_EXE_canon")`),
//! zero network, no credentials.
//!
//! Covers every scenario spec.md names: a fresh repo's `init` writes a
//! working skeleton plus the 0.14 starter policy, AGENTS.md block and
//! commented plans examples (0.17 D2: no `openspec/` dir, no plans
//! source); a rerun keeps `canon.yaml`/`policy.yaml` and refreshes only
//! the AGENTS.md block (refusing when `--no-agents-md` leaves nothing to
//! do);
//! `init` immediately followed by `inventory sync` (with one added
//! `.feature` file) succeeds with zero further edits; `init` immediately
//! followed by `ingest plans` exits `0` as a clean no-op; `check-config`
//! on the fresh skeleton reports all sections PASS; `check-config` on a
//! missing file fails loud; `check-config` surfaces one malformed
//! section while still reporting the other two PASS; `check-config`
//! treats an absent `plans:` key as "not configured", not FAIL.

use std::path::Path;
use std::process::{Command, Output};

fn run_canon(args: &[&str], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(cwd).output().expect("spawn canon binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn write(dir: &Path, rel: &str, content: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

// ── canon init scaffolds a working config, refuses to overwrite ──

#[test]
fn init_scaffolds_a_working_config_in_a_fresh_repo() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_canon(&["init", "--repo", "."], dir.path());
    assert!(out.status.success(), "canon init failed: {}", stderr(&out));
    assert!(stdout(&out).contains("canon skills install"), "init must name the skills install next step: {}", stdout(&out));

    let text = std::fs::read_to_string(dir.path().join("canon.yaml")).unwrap();
    assert!(text.contains("tiers:"), "{text}");
    assert!(text.contains("root: .canon/ledger"), "{text}");
    assert!(text.contains("backend: sqlite"), "{text}");
    assert!(text.contains("path: .canon/hot.db"), "{text}");
    assert!(text.contains("#   backend: postgres"), "postgres must stay present as a commented same-class swap: {text}");
    assert!(text.contains("dsn_env: CANON_PG_DSN"), "the commented postgres swap must keep its dsn_env line intact: {text}");
    for kind in ["task", "handoff", "session", "run", "event"] {
        assert!(text.contains(&format!("{kind}: hot")), "missing hot-routing line for `{kind}`: {text}");
    }
    for kind in ["change", "scenario", "review", "divergence", "trajectory", "strategy_item", "evidence_record"] {
        assert!(text.contains(&format!("{kind}: local")), "missing local-routing line for `{kind}`: {text}");
    }
    assert!(text.contains("specs:"), "{text}");
    assert!(text.contains("root: .canon/specs"), "{text}");
    assert!(text.contains("plans:\n"), "{text}");
    assert!(text.contains("  sources: []\n"), "init configures no plans source: {text}");
    assert!(
        text.contains("  #   - dialect: openspec") && text.contains("  #   - dialect: superpowers") && text.contains(".canon/changes/<slug>/"),
        "the plans block shows commented external examples and names canon's own changes dir: {text}"
    );
    assert!(!dir.path().join("openspec").exists(), "init creates no openspec directory");

    let policy = std::fs::read_to_string(dir.path().join(".canon/policy.yaml")).unwrap();
    for line in ["spec_coverage:", "require_evidence: true", "require_cases: [failure]", "require_review: {}"] {
        assert!(policy.contains(line), "starter policy is missing `{line}`: {policy}");
    }
    assert!(policy.contains("A human"), "the starter policy must say a human approves it: {policy}");
    assert!(stdout(&out).contains("a human reviews it and approves it with `canon policy approve --by <who>`"), "{}", stdout(&out));

    let agents = std::fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
    assert!(agents.starts_with("<!-- canon:begin -->\n") && agents.ends_with("<!-- canon:end -->\n"), "{agents}");
    for needle in [
        "managed with canon",
        "canon status",
        "canon skill",
        "failure",
        "Independent review",
        "canon brief check",
        "canon brief approve --change <slug> --by <the human> --note",
        "Inside an approved scope, never stop for a human",
        "unless the brief says the human ships",
        "a new change is a new scope",
    ] {
        assert!(agents.contains(needle), "the AGENTS.md block is missing `{needle}`: {agents}");
    }

    let gitignore = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    assert!(gitignore.lines().any(|line| line.trim() == ".canon/hot.db*"), "expected the hot tier's db+WAL+SHM glob in .gitignore: {gitignore}");
}

#[test]
fn init_rerun_keeps_canon_yaml_and_policy_and_refreshes_only_the_agents_md_block() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "canon.yaml", "# a hand-authored config\ntiers:\n  git:\n    root: .canon/ledger\n");
    write(dir.path(), ".canon/policy.yaml", "# hand-written\ntrust_required:\n  test-run: agent\n");
    write(dir.path(), "AGENTS.md", "# Rules\n\n<!-- canon:begin -->\nold block\n<!-- canon:end -->\n\nTrailing text.\n");
    let canon_yaml = std::fs::read(dir.path().join("canon.yaml")).unwrap();
    let policy = std::fs::read(dir.path().join(".canon/policy.yaml")).unwrap();

    let out = run_canon(&["init", "--repo", "."], dir.path());
    assert!(out.status.success(), "a rerun refreshes the AGENTS.md block: {}", stderr(&out));
    assert!(stdout(&out).contains("canon.yaml already exists — left unchanged"), "{}", stdout(&out));
    assert!(stdout(&out).contains("refreshed the canon block"), "{}", stdout(&out));
    assert_eq!(std::fs::read(dir.path().join("canon.yaml")).unwrap(), canon_yaml, "canon.yaml must be byte-identical");
    assert_eq!(std::fs::read(dir.path().join(".canon/policy.yaml")).unwrap(), policy, "policy.yaml must be byte-identical");
    assert!(!dir.path().join("openspec").exists(), "a rerun leaves everything but the AGENTS.md block alone");

    let agents = std::fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
    assert!(agents.starts_with("# Rules\n\n<!-- canon:begin -->\n## Canon\n"), "bytes before the block must be kept: {agents}");
    assert!(agents.ends_with("<!-- canon:end -->\n\nTrailing text.\n"), "bytes after the block must be kept: {agents}");
    assert!(!agents.contains("old block"), "{agents}");

    let again = run_canon(&["init", "--repo", "."], dir.path());
    assert!(again.status.success(), "{}", stderr(&again));
    assert!(stdout(&again).contains("already current"), "{}", stdout(&again));
    assert_eq!(std::fs::read_to_string(dir.path().join("AGENTS.md")).unwrap(), agents, "a second rerun must be byte-identical");
}

#[test]
fn init_rerun_with_no_agents_md_refuses_to_overwrite_an_existing_canon_yaml() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "canon.yaml", "# a hand-authored config\ntiers:\n  git:\n    root: .canon/ledger\n");
    write(dir.path(), ".gitignore", CANON_GITIGNORE);
    let before = std::fs::read(dir.path().join("canon.yaml")).unwrap();

    let out = run_canon(&["init", "--repo", ".", "--no-agents-md"], dir.path());
    assert_eq!(out.status.code(), Some(2), "with nothing to refresh, a rerun is the overwrite refusal");
    assert!(stderr(&out).contains("canon.yaml"), "{}", stderr(&out));
    assert_eq!(std::fs::read(dir.path().join("canon.yaml")).unwrap(), before, "the existing file's bytes must be byte-identical before and after the refused init");
    assert!(!dir.path().join("AGENTS.md").exists());
    assert_eq!(std::fs::read_to_string(dir.path().join(".gitignore")).unwrap(), CANON_GITIGNORE);
}

/// Every canon `.gitignore` line, as a repo set up by `canon init` 0.17 has them.
const CANON_GITIGNORE: &str = ".canon/hot.db*\n.canon/audit/\n.canon/ledger/kind=scenario/\n.canon/scenario-index.lock\n.canon/artifact-store.lock\n";

/// authoring.init.97 (#35): `canon init` re-run on a repo an older canon
/// set up adds the canon `.gitignore` lines it lacks, each once, and keeps
/// the rest of the file and `canon.yaml` byte-identical; with
/// `--no-agents-md` that is all it does, and a second run with nothing
/// missing is the old overwrite refusal.
#[test]
fn authoring_init_97_a_rerun_adds_the_missing_canon_gitignore_lines() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "canon.yaml", "# a 0.15 config\ntiers:\n  git:\n    root: .canon/ledger\n");
    write(dir.path(), ".gitignore", "target/\n.canon/hot.db*");
    let canon_yaml = std::fs::read(dir.path().join("canon.yaml")).unwrap();

    let out = run_canon(&["init", "--repo", "."], dir.path());
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("canon.yaml already exists — left unchanged; the AGENTS.md block and the canon .gitignore lines are refreshed\n"), "{}", stdout(&out));
    assert!(
        stdout(&out).contains("canon init: added to .gitignore: .canon/audit/, .canon/ledger/kind=scenario/, .canon/scenario-index.lock, .canon/artifact-store.lock\n"),
        "{}",
        stdout(&out)
    );
    let gitignore = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    assert!(gitignore.starts_with("target/\n.canon/hot.db*\n# canon's read-access audit log"), "the existing lines are kept: {gitignore}");
    for line in CANON_GITIGNORE.lines() {
        assert_eq!(gitignore.lines().filter(|have| have.trim() == line).count(), 1, "{line} once: {gitignore}");
    }
    assert_eq!(std::fs::read(dir.path().join("canon.yaml")).unwrap(), canon_yaml);

    let other = tempfile::tempdir().unwrap();
    write(other.path(), "canon.yaml", "# a 0.15 config\ntiers:\n  git:\n    root: .canon/ledger\n");
    let out = run_canon(&["init", "--repo", ".", "--no-agents-md"], other.path());
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("left unchanged; the canon .gitignore lines are refreshed\n"), "{}", stdout(&out));
    assert!(!other.path().join("AGENTS.md").exists());
    assert_eq!(std::fs::read_to_string(other.path().join(".gitignore")).unwrap().lines().filter(|line| !line.starts_with('#')).collect::<Vec<_>>(), CANON_GITIGNORE.lines().collect::<Vec<_>>());
    let again = run_canon(&["init", "--repo", ".", "--no-agents-md"], other.path());
    assert_eq!(again.status.code(), Some(2), "nothing left to refresh: {}", stdout(&again));
}

#[test]
fn init_appends_the_block_to_an_existing_agents_md_without_touching_its_text() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "AGENTS.md", "# House rules\n\nRun the formatter.");
    let out = run_canon(&["init", "--repo", "."], dir.path());
    assert!(out.status.success(), "{}", stderr(&out));
    let agents = std::fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
    assert!(agents.starts_with("# House rules\n\nRun the formatter.\n\n<!-- canon:begin -->\n"), "{agents}");
    assert!(agents.ends_with("<!-- canon:end -->\n"), "{agents}");
}

#[test]
fn init_refuses_an_agents_md_with_an_unclosed_canon_block_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "AGENTS.md", "<!-- canon:begin -->\nno end marker\n");
    let out = run_canon(&["init", "--repo", "."], dir.path());
    assert_eq!(out.status.code(), Some(2), "{}", stdout(&out));
    assert!(stderr(&out).contains("<!-- canon:end -->"), "{}", stderr(&out));
    assert!(!dir.path().join("canon.yaml").exists(), "a refused init writes nothing");
    assert_eq!(std::fs::read_to_string(dir.path().join("AGENTS.md")).unwrap(), "<!-- canon:begin -->\nno end marker\n");
}

#[test]
fn init_refuses_a_duplicate_begin_marker_and_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let agents = "<!-- canon:begin -->\nfirst\n<!-- canon:begin -->\nsecond\n<!-- canon:end -->\n";
    write(dir.path(), "AGENTS.md", agents);
    let out = run_canon(&["init", "--repo", "."], dir.path());
    assert_eq!(out.status.code(), Some(2), "{}", stdout(&out));
    assert!(stderr(&out).contains("found 2 begin, 1 end"), "{}", stderr(&out));
    assert!(!dir.path().join("canon.yaml").exists(), "a refused init writes nothing");
    assert!(!dir.path().join(".canon").exists(), "a refused init writes nothing");
    assert_eq!(std::fs::read_to_string(dir.path().join("AGENTS.md")).unwrap(), agents);

    // The same refusal on a rerun leaves the existing canon.yaml alone too.
    write(dir.path(), "canon.yaml", "# hand-authored\n");
    let rerun = run_canon(&["init", "--repo", "."], dir.path());
    assert_eq!(rerun.status.code(), Some(2), "{}", stdout(&rerun));
    assert_eq!(std::fs::read_to_string(dir.path().join("AGENTS.md")).unwrap(), agents);
    assert_eq!(std::fs::read_to_string(dir.path().join("canon.yaml")).unwrap(), "# hand-authored\n");
}

#[test]
fn init_treats_markers_inside_a_code_fence_as_text() {
    let dir = tempfile::tempdir().unwrap();
    let agents = "# Docs\n\nCanon owns this block:\n\n```md\n<!-- canon:begin -->\nexample\n<!-- canon:end -->\n```\n";
    write(dir.path(), "AGENTS.md", agents);
    let out = run_canon(&["init", "--repo", "."], dir.path());
    assert!(out.status.success(), "{}", stderr(&out));
    let merged = std::fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
    assert!(merged.starts_with(&format!("{agents}\n<!-- canon:begin -->\n## Canon\n")), "the fenced sample must be kept and the block appended: {merged}");
    assert!(merged.ends_with("<!-- canon:end -->\n"), "{merged}");

    let rerun = run_canon(&["init", "--repo", "."], dir.path());
    assert!(stdout(&rerun).contains("already current"), "{}", stdout(&rerun));
    assert_eq!(std::fs::read_to_string(dir.path().join("AGENTS.md")).unwrap(), merged);
}

#[test]
fn init_no_agents_md_and_no_policy_skip_those_files() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_canon(&["init", "--repo", ".", "--no-agents-md", "--no-policy"], dir.path());
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(dir.path().join("canon.yaml").is_file());
    assert!(!dir.path().join("AGENTS.md").exists());
    assert!(!dir.path().join(".canon/policy.yaml").exists());
}

#[test]
fn init_keeps_an_existing_policy_yaml_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), ".canon/policy.yaml", "trust_required:\n  test-run: agent\n");
    let out = run_canon(&["init", "--repo", "."], dir.path());
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("policy.yaml already exists — left unchanged"), "{}", stdout(&out));
    assert_eq!(std::fs::read_to_string(dir.path().join(".canon/policy.yaml")).unwrap(), "trust_required:\n  test-run: agent\n");
}

/// The starter policy is live: on a fresh init the gate is clean on an
/// empty corpus and red on an unevidenced scenario, with no advisory
/// that coverage is off.
#[test]
fn init_starter_policy_makes_the_gate_report_an_unevidenced_scenario() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run_canon(&["init", "--repo", "."], dir.path()).status.success());
    let clean = run_canon(&["gate", "check"], dir.path());
    assert_eq!(clean.status.code(), Some(0), "{}{}", stdout(&clean), stderr(&clean));

    let scenario = run_canon(&["scenario", "new", "cart.add.01", "--title", "An out-of-stock item is refused", "--case", "failure"], dir.path());
    assert!(scenario.status.success(), "{}", stderr(&scenario));
    assert!(run_canon(&["inventory", "sync"], dir.path()).status.success());
    let red = run_canon(&["gate", "check"], dir.path());
    assert_eq!(red.status.code(), Some(1), "{}", stdout(&red));
    assert!(stdout(&red).contains("cart.add.01"), "{}", stdout(&red));
    assert!(!stdout(&red).contains("spec_coverage is off"), "{}", stdout(&red));
}

// ── starter policy levels (0.16 D3) ──

fn status_level(dir: &Path) -> serde_json::Value {
    let out = run_canon(&["status", "--json"], dir);
    assert!(out.status.success(), "{}", stderr(&out));
    let report: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    report["policy"]["level"].clone()
}

/// The policy's keys, comments dropped.
fn policy_keys(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(dir.join(".canon/policy.yaml"))
        .unwrap()
        .lines()
        .filter(|line| !line.trim_start().starts_with('#') && !line.trim().is_empty())
        .map(str::to_string)
        .collect()
}

/// authoring.init.30: the default level is strict; its bytes are the
/// 0.15 starter under one level line (pinned byte for byte by
/// `init::tests::strict_starter_policy_is_the_pre_level_starter_under_one_level_line`).
#[test]
fn authoring_init_30_default_init_writes_the_strict_starter_under_a_level_line() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_canon(&["init"], dir.path());
    assert!(out.status.success(), "{}", stderr(&out));
    let policy = std::fs::read_to_string(dir.path().join(".canon/policy.yaml")).unwrap();
    assert!(policy.starts_with("# canon policy level: strict\n# .canon/policy.yaml -- scaffolded by `canon init`.\n"), "{policy}");
    assert_eq!(policy_keys(dir.path()), ["spec_coverage:", "  require_evidence: true", "  require_cases: [failure]", "  require_review: {}"]);
    assert_eq!(status_level(dir.path()), "strict");
    let human = stdout(&run_canon(&["status"], dir.path()));
    assert!(human.contains("\n  level: strict\n"), "{human}");

    let explicit = tempfile::tempdir().unwrap();
    assert!(run_canon(&["init", "--policy", "strict"], explicit.path()).status.success());
    assert_eq!(std::fs::read_to_string(explicit.path().join(".canon/policy.yaml")).unwrap(), policy, "--policy strict is the default");
}

/// authoring.init.31
#[test]
fn authoring_init_31_light_starter_requires_evidence_only_and_says_how_to_raise_it() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_canon(&["init", "--policy", "light"], dir.path());
    assert!(out.status.success(), "{}", stderr(&out));
    let policy = std::fs::read_to_string(dir.path().join(".canon/policy.yaml")).unwrap();
    assert!(policy.starts_with("# canon policy level: light\n"), "{policy}");
    assert_eq!(policy_keys(dir.path()), ["spec_coverage:", "  require_evidence: true"]);
    for line in ["  # require_cases: [failure]", "  # require_review: {}", "To raise the level", "canon policy approve --by <who>"] {
        assert!(policy.contains(line), "the light starter is missing `{line}`: {policy}");
    }

    let scenario = run_canon(&["scenario", "new", "cart.add.01", "--title", "An item is added", "--case", "happy"], dir.path());
    assert!(scenario.status.success(), "{}", stderr(&scenario));
    assert!(run_canon(&["inventory", "sync"], dir.path()).status.success());
    let gate = stdout(&run_canon(&["gate", "check"], dir.path()));
    assert!(gate.contains("cart.add.01 — spec scenario has no evidence record"), "{gate}");
    assert!(!gate.contains("@case:failure"), "light asks no surface for a failure case: {gate}");

    assert_eq!(status_level(dir.path()), "light");
    let human = stdout(&run_canon(&["status"], dir.path()));
    assert!(
        human.contains("\n  level: light — raise it by uncommenting `require_cases: [failure]` (standard) and `require_review: {}` (strict) in .canon/policy.yaml"),
        "{human}"
    );
}

/// authoring.init.32
#[test]
fn authoring_init_32_standard_starter_adds_a_failure_case_and_says_how_to_reach_strict() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_canon(&["init", "--policy", "standard"], dir.path());
    assert!(out.status.success(), "{}", stderr(&out));
    let policy = std::fs::read_to_string(dir.path().join(".canon/policy.yaml")).unwrap();
    assert!(policy.starts_with("# canon policy level: standard\n"), "{policy}");
    assert_eq!(policy_keys(dir.path()), ["spec_coverage:", "  require_evidence: true", "  require_cases: [failure]"]);
    for line in [
        "  # To raise the level, uncomment what the next level adds below and\n  # change the level line at the top of this file; a human approves the\n  # edit with `canon policy approve --by <who>`.\n",
        "  # strict:\n",
        "  # require_review: {}\n",
    ] {
        assert!(policy.contains(line), "the standard starter must say how to raise it to strict and that a human approves the edit; missing {line:?}: {policy}");
    }
    let raise = policy.find("To raise the level").unwrap();
    assert!(raise > policy.find("  require_cases: [failure]").unwrap() && raise < policy.find("  # require_review: {}").unwrap(), "the raise instructions sit between the active keys and the commented strict key: {policy}");

    assert_eq!(status_level(dir.path()), "standard");
    let human = stdout(&run_canon(&["status"], dir.path()));
    assert!(human.contains("\n  level: standard — raise it by uncommenting `require_review: {}` (strict)"), "{human}");
}

/// authoring.init.33
#[test]
fn authoring_init_33_an_unknown_policy_level_is_refused_and_nothing_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_canon(&["init", "--policy", "extreme"], dir.path());
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("light, standard, strict"), "{}", stderr(&out));
    assert!(!dir.path().join("canon.yaml").exists());
    assert!(!dir.path().join(".canon").exists());
}

/// authoring.init.34
#[test]
fn authoring_init_34_a_policy_level_with_no_policy_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_canon(&["init", "--policy", "light", "--no-policy"], dir.path());
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("cannot be used with"), "{}", stderr(&out));
    assert!(!dir.path().join("canon.yaml").exists());
    assert!(!dir.path().join(".canon").exists());
}

// ── the scaffolded config resolves cleanly through every existing loader ──

#[test]
fn init_then_inventory_sync_succeeds_with_zero_further_edits() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run_canon(&["init", "--repo", "."], dir.path()).status.success());

    let prov = "  # canon: {\"schema\":1,\"at\":\"2026-07-10T00:00:00Z\",\"actor\":{\"agent_id\":\"a-human\"}}";
    let feature_text = format!("Feature: world hotdeal\n{prov}\n\n  @world.hotdeal.01\n  Scenario: Apply a hotdeal coupon\n{prov}\n    Given a step\n");
    write(dir.path(), ".canon/specs/kind=feature/area=world/hotdeal.feature", &feature_text);

    let out = run_canon(&["inventory", "sync", "--repo", "."], dir.path());
    assert!(out.status.success(), "canon inventory sync must succeed against a freshly-init'd repo with zero further config edits: {}", stderr(&out));
}

#[test]
fn init_then_ingest_plans_exits_0_as_a_clean_no_op() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run_canon(&["init", "--repo", "."], dir.path()).status.success());
    // Top-level dirs no plan source may mistake for changes.
    write(dir.path(), "specs/README.md", "specs\n");
    write(dir.path(), "src/main.rs", "fn main() {}\n");

    let out = run_canon(&["ingest", "plans", "--repo", ".", "--json"], dir.path());
    assert!(out.status.success(), "canon ingest plans right after init must be a clean no-op: {}", stderr(&out));
    let payload: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(payload["sources"].as_array().map(Vec::len), Some(0), "no external source and no .canon/changes yet: {payload}");
    assert_eq!(payload["changes_persisted"], 0, "{payload}");
}

// ── canon init --check-config ──

#[test]
fn check_config_on_the_freshly_scaffolded_config_reports_all_sections_pass() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run_canon(&["init", "--repo", "."], dir.path()).status.success());

    let out = run_canon(&["init", "--check-config", "--repo", "."], dir.path());
    assert!(out.status.success(), "check-config on the fresh skeleton must exit 0: {}", stderr(&out));
    let report = stdout(&out);
    assert!(report.contains("[PASS]") && report.contains("tiers"), "{report}");
    assert!(report.contains("[PASS] specs"), "{report}");
    assert!(report.contains("[PASS] plans"), "{report}");
}

#[test]
fn check_config_on_a_missing_canon_yaml_fails_loud() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_canon(&["init", "--check-config", "--repo", "."], dir.path());
    assert!(!out.status.success());
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("canon.yaml"), "{}", stderr(&out));
    assert!(stdout(&out).is_empty(), "no per-section report on a missing file: {}", stdout(&out));
}

#[test]
fn check_config_surfaces_a_malformed_section_without_hiding_the_others() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "canon.yaml",
        "tiers:\n  local:\n    backend: git\n    root: .canon/ledger\nrouting:\n  change: local\nspecs:\n  roots:\n    - id: root\n      root: specs\nplans:\n  sources:\n    - dialect: not-a-real-dialect\n      root: plans-src\n",
    );

    let out = run_canon(&["init", "--check-config", "--repo", "."], dir.path());
    assert!(!out.status.success(), "a malformed plans section must fail the whole check nonzero");
    let report = stdout(&out);
    assert!(report.contains("[PASS]") && report.contains("tiers"), "{report}");
    assert!(report.contains("[PASS] specs"), "{report}");
    assert!(report.contains("[FAIL] plans"), "{report}");
    assert!(report.contains("not-a-real-dialect"), "must name the unregistered dialect id: {report}");
}

#[test]
fn check_config_treats_an_absent_plans_key_as_not_configured_not_a_failure() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "canon.yaml",
        "tiers:\n  local:\n    backend: git\n    root: .canon/ledger\nrouting:\n  change: local\nspecs:\n  roots:\n    - id: root\n      root: specs\n",
    );

    let out = run_canon(&["init", "--check-config", "--repo", "."], dir.path());
    assert!(out.status.success(), "an absent `plans:` key must not fail the check: {}", stderr(&out));
    let report = stdout(&out);
    assert!(report.contains("[not configured] plans"), "{report}");
}

/// s29 design D9 / spec scenario "check-config catches a malformed
/// schema": `TierPolicy::from_yaml` itself has no `canon-store`
/// dependency to call `validate_schema_ident`, so a `tiers.<rung>.schema`
/// `PgTier::connect` would reject at attach time must be caught by
/// `check-config` itself — the command fails naming the schema and
/// must NEVER print `[PASS] tiers/routing/aging` over it.
#[test]
fn check_config_fails_on_a_malformed_pg_schema_and_never_prints_pass_for_tiers() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "canon.yaml",
        "tiers:\n  local: { backend: git, root: .canon/ledger }\n  hot: { backend: postgres, dsn_env: CANON_PG_DSN, schema: Bad-Schema }\nrouting:\n  task: hot\n",
    );

    let out = run_canon(&["init", "--check-config", "--repo", "."], dir.path());
    assert!(!out.status.success(), "a malformed pg schema must fail check-config nonzero");
    let report = stdout(&out);
    assert!(report.contains("[FAIL] tiers/routing/aging"), "{report}");
    assert!(report.contains("Bad-Schema"), "must name the offending schema: {report}");
    assert!(!report.contains("[PASS] tiers/routing/aging"), "must never print PASS over a malformed schema: {report}");
}

// ── s32 `sqlite-hot-backend` ──

#[test]
fn check_config_validates_a_sqlite_hot_config_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "canon.yaml", "tiers:\n  local: { backend: git, root: .canon/ledger }\n  hot: { backend: sqlite, path: .canon/hot.db }\nrouting:\n  task: hot\n");

    let out = run_canon(&["init", "--check-config", "--repo", "."], dir.path());
    assert!(out.status.success(), "check-config on a valid sqlite hot config must exit 0: {}", stderr(&out));
    let report = stdout(&out);
    assert!(report.contains("[PASS] tiers/routing/aging"), "{report}");
}

#[test]
fn check_config_fails_loud_on_a_sqlite_entry_missing_path() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "canon.yaml", "tiers:\n  local: { backend: git, root: .canon/ledger }\n  hot: { backend: sqlite }\nrouting:\n  task: hot\n");

    let out = run_canon(&["init", "--check-config", "--repo", "."], dir.path());
    assert!(!out.status.success(), "a sqlite entry missing `path` must fail check-config nonzero");
    let report = stdout(&out);
    assert!(report.contains("[FAIL] tiers/routing/aging"), "{report}");
    assert!(report.contains("path"), "the parse error must name the missing `path` field: {report}");
}

/// spec.md "Fresh init ingests without docker" — the s32 headline
/// scenario: a bare `canon init` repo (no docker, no `CANON_*` env)
/// immediately `canon ingest sessions`-es cleanly, persisting into the
/// scaffolded sqlite hot tier. Ambient `CODEX_HOME`/
/// `CANON_INGEST_*_DIR`/`HERMES_HOME` are cleared for the child
/// process (mirroring `tests/ingest.rs`'s own isolation) so the empty
/// `--home` fixture is the ONLY thing scanned — this asserts the
/// PERSIST path, not merely a clean empty scan.
#[test]
fn init_then_ingest_sessions_writes_the_sqlite_hot_tier_with_zero_env_or_services() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run_canon(&["init", "--repo", "."], dir.path()).status.success());
    assert!(!dir.path().join(".canon/hot.db").exists(), "canon init itself must not eagerly create the hot tier's db file");

    let home = tempfile::tempdir().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(["ingest", "sessions", "--home", &home.path().display().to_string()])
        .current_dir(dir.path())
        .env_remove("CODEX_HOME")
        .env_remove("CANON_INGEST_CLAUDE_SESSIONS_DIR")
        .env_remove("CANON_INGEST_OMP_SESSIONS_DIR")
        .env_remove("HERMES_HOME")
        .output()
        .expect("spawn canon binary");

    assert!(out.status.success(), "canon ingest sessions against a fresh sqlite-hot init must exit 0 with zero services: {}", stderr(&out));
    assert!(dir.path().join(".canon/hot.db").exists(), "expected the sqlite hot tier's db file to be written by the ingest pass");
}

// ── harness choice (0.17 D5) ──

/// A canonical skill source under `dir/source`.
fn skill_source(dir: &Path) -> std::path::PathBuf {
    let source = dir.join("source");
    write(&source, "SKILL.src.md", "---\nname: canon\ndescription: umbrella\n---\n\n# Canon\n");
    write(&source, "reference/topic.md", "# topic\n");
    source
}

/// `canon init` in `repo` (piped stdin, so never a terminal) with the
/// default skill source at `source`.
fn init_with_source(args: &[&str], repo: &Path, source: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).arg("init").args(args).current_dir(repo).env("CANON_SKILLS_SOURCE", source).output().expect("spawn canon binary")
}

const PROJECTIONS: [&str; 4] = [".claude/skills/canon", ".agents/skills/canon", ".omp/skills/canon", ".pi/skills/canon"];

fn projected(repo: &Path) -> Vec<&'static str> {
    PROJECTIONS.into_iter().filter(|dir| repo.join(dir).join("SKILL.md").is_file()).collect()
}

/// authoring.init.90: without a terminal or `--providers`, init installs
/// for the detected harnesses only and records them.
#[test]
fn authoring_init_90_non_tty_init_installs_only_the_detected_harness_and_records_it() {
    let tmp = tempfile::tempdir().unwrap();
    let source = skill_source(tmp.path());
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(repo.join(".omp")).unwrap();
    std::fs::create_dir_all(repo.join(".agents")).unwrap();

    let out = init_with_source(&[], &repo, &source);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(stdout(&out).ends_with("canon init: installed the canon skill for omp (./.omp/skills/canon)\n"), "{}", stdout(&out));
    assert_eq!(projected(&repo), vec![".omp/skills/canon"], ".agents alone selects nothing");
    let config = std::fs::read_to_string(repo.join("canon.yaml")).unwrap();
    assert!(config.ends_with("skills:\n  providers: [omp]\n"), "{config}");
    let lock = std::fs::read_to_string(repo.join(".canon/skills/.install-lock.json")).unwrap();
    assert!(lock.contains("\"omp\"") && !lock.contains("\"codex\""), "{lock}");

    let check = run_canon(&["init", "--check-config"], &repo);
    assert!(stdout(&check).contains("[PASS] skills (providers: omp)\n"), "{}", stdout(&check));
    let doctor = Command::new(env!("CARGO_BIN_EXE_canon")).args(["skills", "doctor"]).current_dir(&repo).env("CANON_SKILLS_SOURCE", &source).output().unwrap();
    assert_eq!(stdout(&doctor), format!("providers: omp (from canon.yaml skills.providers)\nstatus: healthy\n{}\n", canon_cli::skills::owned_paths_lines(&[canon_cli::skills::Provider::Omp]).join("\n")));
}

/// authoring.init.91: `--providers codex` answers without a prompt and
/// installs only `.agents/skills/canon`, whatever markers exist.
#[test]
fn authoring_init_91_providers_codex_installs_only_the_agents_projection() {
    let tmp = tempfile::tempdir().unwrap();
    let source = skill_source(tmp.path());
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(repo.join(".claude")).unwrap();

    let out = init_with_source(&["--providers", "codex"], &repo, &source);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(projected(&repo), vec![".agents/skills/canon"]);
    assert!(!repo.join(".codex").exists() && !repo.join(".claude/skills").exists());
    assert!(std::fs::read_to_string(repo.join("canon.yaml")).unwrap().ends_with("skills:\n  providers: [codex]\n"));
    assert!(repo.join("AGENTS.md").is_file() && repo.join(".canon/policy.yaml").is_file(), "the other writes still happen");

    // A rerun keeps canon.yaml and says --providers is not applied there.
    let rerun = init_with_source(&["--providers", "claude"], &repo, &source);
    assert_eq!(rerun.status.code(), Some(0), "{}", stderr(&rerun));
    assert!(stdout(&rerun).contains("--providers is not applied to an existing canon.yaml; run `canon skills install --providers=<list>`"), "{}", stdout(&rerun));
    assert_eq!(projected(&repo), vec![".agents/skills/canon"]);
}

/// authoring.init.92: a bare repo with no harness marker gets no skill
/// and no `skills:` key, and init names the flag that chooses.
#[test]
fn authoring_init_92_a_bare_repo_installs_nothing_and_names_the_flag() {
    let tmp = tempfile::tempdir().unwrap();
    let source = skill_source(tmp.path());
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();

    let out = init_with_source(&[], &repo, &source);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        stdout(&out).ends_with("canon init: no harness detected (no .claude, .codex, .omp or .pi directory), so no canon skill is installed; choose with `canon skills install --providers=<list>` (any of claude, codex, omp, pi)\n"),
        "{}",
        stdout(&out)
    );
    assert!(projected(&repo).is_empty());
    assert!(!repo.join(".canon/skills").exists(), "no install lock");
    let config = std::fs::read_to_string(repo.join("canon.yaml")).unwrap();
    assert!(config.ends_with("# skills:\n#   providers: [claude, codex]\n"), "only a commented example: {config}");
    let check = run_canon(&["init", "--check-config"], &repo);
    assert!(stdout(&check).contains("[not configured] skills"), "{}", stdout(&check));
}

/// authoring.init.93: `--providers none` is an explicit empty choice,
/// recorded as `providers: []`, with nothing installed.
#[test]
fn authoring_init_93_an_explicit_empty_choice_is_recorded() {
    let tmp = tempfile::tempdir().unwrap();
    let source = skill_source(tmp.path());
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(repo.join(".claude")).unwrap();

    let out = init_with_source(&["--providers", "none"], &repo, &source);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(stdout(&out).ends_with("canon init: skills.providers is [] — no canon skill installed; change it with `canon skills install --providers=<list>`\n"), "{}", stdout(&out));
    assert!(projected(&repo).is_empty());
    assert!(std::fs::read_to_string(repo.join("canon.yaml")).unwrap().ends_with("skills:\n  providers: []\n"));
    let check = Command::new(env!("CARGO_BIN_EXE_canon")).args(["skills", "check"]).current_dir(&repo).env("CANON_SKILLS_SOURCE", &source).output().unwrap();
    assert_eq!(check.status.code(), Some(0), "{}", stdout(&check));
    assert_eq!(stdout(&check), "providers: none (from canon.yaml skills.providers)\n");

    let empty = tmp.path().join("empty-flag");
    std::fs::create_dir_all(empty.join(".omp")).unwrap();
    let out = init_with_source(&["--providers="], &empty, &source);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(std::fs::read_to_string(empty.join("canon.yaml")).unwrap().ends_with("skills:\n  providers: []\n"));
    assert!(projected(&empty).is_empty());
}

/// authoring.init.94: an unknown `--providers` name is refused before
/// anything is written.
#[test]
fn authoring_init_94_an_unknown_provider_is_refused_and_nothing_is_written() {
    let tmp = tempfile::tempdir().unwrap();
    let source = skill_source(tmp.path());
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();

    let out = init_with_source(&["--providers", "claude,gemini"], &repo, &source);
    assert_eq!(out.status.code(), Some(2), "{}", stdout(&out));
    assert_eq!(stderr(&out), "canon init: refused — invalid provider `gemini`; expected one of: claude, codex, omp, pi, or none\n");
    assert_eq!(std::fs::read_dir(&repo).unwrap().count(), 0, "nothing is written");
}

/// authoring.init.95: when the skill source is missing, init still
/// records the choice, then exits 2 naming the install to run.
#[test]
fn authoring_init_95_a_missing_skill_source_is_reported_with_the_retry() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(repo.join(".pi")).unwrap();

    let out = init_with_source(&[], &repo, &tmp.path().join("missing"));
    assert_eq!(out.status.code(), Some(2), "{}", stdout(&out));
    let err = stderr(&out);
    assert!(err.contains("wrote `skills: { providers: [pi] }` to ./canon.yaml but did not install the canon skill: canonical skill source is missing SKILL.src.md"), "{err}");
    assert!(err.contains("run `canon skills install` (set CANON_SKILLS_SOURCE or pass --source <dir>; it follows skills.providers)"), "{err}");
    assert!(std::fs::read_to_string(repo.join("canon.yaml")).unwrap().ends_with("skills:\n  providers: [pi]\n"));
    assert!(projected(&repo).is_empty());

    let source = skill_source(tmp.path());
    let retry = Command::new(env!("CARGO_BIN_EXE_canon")).args(["skills", "install", "--source"]).arg(&source).current_dir(&repo).output().unwrap();
    assert_eq!(stdout(&retry), format!("canon v1 — installed (pi; from canon.yaml skills.providers)\n{}\n", canon_cli::skills::owned_paths_lines(&[canon_cli::skills::Provider::Pi]).join("\n")), "{}", stderr(&retry));
    assert_eq!(projected(&repo), vec![".pi/skills/canon"]);
}
