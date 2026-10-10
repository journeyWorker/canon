//! Integration tests for `canon init [--repo <dir>]` + `canon init
//! --check-config [--repo <dir>]` (s19 `canon-init-scaffold` spec) —
//! invokes the actually-built `canon` binary (`env!("CARGO_BIN_EXE_canon")`),
//! zero network, no credentials.
//!
//! Covers every scenario spec.md names: a fresh repo's `init` writes a
//! working skeleton plus the 0.14 starter policy, AGENTS.md block and
//! plans home; a rerun keeps `canon.yaml`/`policy.yaml` and refreshes only
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
    assert!(text.contains("root: specs"), "{text}");
    assert!(text.contains("plans:"), "{text}");
    assert!(text.contains("- dialect: openspec\n      root: .\n"), "init must configure the default openspec plans source: {text}");
    assert!(dir.path().join("openspec/changes/.gitkeep").is_file(), "the plans source's changes dir must exist and survive a clone");

    let policy = std::fs::read_to_string(dir.path().join(".canon/policy.yaml")).unwrap();
    for line in ["spec_coverage:", "require_evidence: true", "require_cases: [failure]", "require_review: {}"] {
        assert!(policy.contains(line), "starter policy is missing `{line}`: {policy}");
    }
    assert!(policy.contains("A human"), "the starter policy must say a human approves it: {policy}");
    assert!(stdout(&out).contains("a human should review and approve it"), "{}", stdout(&out));

    let agents = std::fs::read_to_string(dir.path().join("AGENTS.md")).unwrap();
    assert!(agents.starts_with("<!-- canon:begin -->\n") && agents.ends_with("<!-- canon:end -->\n"), "{agents}");
    for needle in ["managed with canon", "canon status", "canon skill", "failure", "Independent review"] {
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
    let before = std::fs::read(dir.path().join("canon.yaml")).unwrap();

    let out = run_canon(&["init", "--repo", ".", "--no-agents-md"], dir.path());
    assert_eq!(out.status.code(), Some(2), "with nothing to refresh, a rerun is the overwrite refusal");
    assert!(stderr(&out).contains("canon.yaml"), "{}", stderr(&out));
    assert_eq!(std::fs::read(dir.path().join("canon.yaml")).unwrap(), before, "the existing file's bytes must be byte-identical before and after the refused init");
    assert!(!dir.path().join("AGENTS.md").exists());
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

// ── the scaffolded config resolves cleanly through every existing loader ──

#[test]
fn init_then_inventory_sync_succeeds_with_zero_further_edits() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run_canon(&["init", "--repo", "."], dir.path()).status.success());

    let prov = "  # canon: {\"schema\":1,\"at\":\"2026-07-10T00:00:00Z\",\"actor\":{\"agent_id\":\"a-human\"}}";
    let feature_text = format!("Feature: world hotdeal\n{prov}\n\n  @world.hotdeal.01\n  Scenario: Apply a hotdeal coupon\n{prov}\n    Given a step\n");
    write(dir.path(), "specs/features/kind=feature/area=world/hotdeal.feature", &feature_text);

    let out = run_canon(&["inventory", "sync", "--repo", "."], dir.path());
    assert!(out.status.success(), "canon inventory sync must succeed against a freshly-init'd repo with zero further config edits: {}", stderr(&out));
}

#[test]
fn init_then_ingest_plans_exits_0_as_a_clean_no_op() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run_canon(&["init", "--repo", "."], dir.path()).status.success());
    // Top-level dirs the openspec source must NOT mistake for changes.
    write(dir.path(), "specs/README.md", "specs\n");
    write(dir.path(), "src/main.rs", "fn main() {}\n");

    let out = run_canon(&["ingest", "plans", "--repo", ".", "--json"], dir.path());
    assert!(out.status.success(), "canon ingest plans against the scaffolded openspec source must be a clean no-op: {}", stderr(&out));
    let payload: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    assert_eq!(payload["sources"][0]["dialect"], "openspec", "{payload}");
    assert_eq!(payload["sources"][0]["changes_parsed"], 0, "{payload}");
    assert_eq!(payload["sources"][0]["malformed"].as_array().map(Vec::len), Some(0), "{payload}");
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
