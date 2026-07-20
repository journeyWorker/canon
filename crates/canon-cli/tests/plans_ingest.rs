//! End-to-end integration test for `canon ingest plans [--dialect <id>
//! --source <path>] [--repo <dir>] [--json]` (s17 P3
//! `s17-plan-import`), invoking the actually-built `canon` binary
//! (`env!("CARGO_BIN_EXE_canon")`) against offline openspec-change-dir
//! fixtures — zero network, no credentials.

use std::path::Path;
use std::process::{Command, Output};

use canon_model::{Actor, Envelope, EvidenceRecord, EvidenceVerdict, RecordKind, RoleId, TaskId};
use canon_store::git_tier::GitTier;
use canon_store::tier::{Tier, TierQuery};
use serde_json::Value;

fn run_canon(args: &[&str], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).env_remove("CANON_PG_DSN_S17_TEST_UNSET").current_dir(cwd).output().expect("spawn canon binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn write_canon_yaml(root: &Path, body: &str) {
    std::fs::write(root.join("canon.yaml"), body).unwrap();
}

/// One openspec change dir: `<openspec_root>/openspec/changes/<change_id>/`
/// with `proposal.md` (a `## Why` paragraph, task 2.2's mapping) and
/// `tasks.md` (`rows` pasted verbatim as `- [ ]`/`- [x]` lines under one
/// heading).
fn write_change_dir(openspec_root: &Path, change_id: &str, why: &str, rows: &[&str]) {
    let dir = openspec_root.join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("proposal.md"), format!("## Why\n\n{why}\n\n## What Changes\n\n- does the thing\n")).unwrap();
    let mut tasks = String::from("## 1. Work\n\n");
    for row in rows {
        tasks.push_str(row);
        tasks.push('\n');
    }
    std::fs::write(dir.join("tasks.md"), tasks).unwrap();
}

/// One superpowers plan doc (s30 D1: the `writing-plans` skill's
/// grammar) at `<plans_root>/<filename>` -- an H1, a `**Goal:**
/// <goal>` line, then each `(heading, checkbox_rows)` pair rendered
/// as a `### <heading>` section with its rows pasted verbatim. The
/// skill's `**Step N:**` bolding is deliberately omitted from these
/// rows -- D1 pins it as NOT load-bearing, so a plain `- [ ]`/`- [x]`
/// row must still count.
fn write_plan_doc(plans_root: &Path, filename: &str, goal: &str, task_sections: &[(&str, &[&str])]) {
    std::fs::create_dir_all(plans_root).unwrap();
    let mut body = String::from("# Demo Feature Implementation Plan\n\n");
    body.push_str(&format!("**Goal:** {goal}\n\n"));
    for (heading, rows) in task_sections {
        body.push_str(&format!("### {heading}\n\n"));
        for row in *rows {
            body.push_str(row);
            body.push('\n');
        }
        body.push('\n');
    }
    std::fs::write(plans_root.join(filename), body).unwrap();
}

fn ingest_json(dir: &Path, args: &[&str]) -> Value {
    let mut full = vec!["ingest", "plans", "--json"];
    full.extend_from_slice(args);
    let output = run_canon(&full, dir);
    assert!(output.status.success(), "canon ingest plans --json failed: stderr={} stdout={}", stderr(&output), stdout(&output));
    serde_json::from_str(&stdout(&output)).unwrap_or_else(|e| panic!("invalid JSON: {e}\nstdout: {}", stdout(&output)))
}

// ── task 3.2: canon.yaml `plans:` config ──

#[test]
fn absent_plans_section_is_a_clean_noop() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(dir.path(), "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  change: local\n  task: local\n");

    let output = run_canon(&["ingest", "plans"], dir.path());
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("total: 0 change(s) persisted, 0 task(s) persisted, 0 duplicate-change-id skipped"), "{text}");

    let json = ingest_json(dir.path(), &[]);
    assert_eq!(json["sources"].as_array().unwrap().len(), 0, "no `plans:` section -> zero sources, never a hardcoded default root: {json}");
}

#[test]
fn a_typod_plans_key_fails_loud() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(dir.path(), "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  change: local\n  task: local\nplans:\n  source: []\n");

    let output = run_canon(&["ingest", "plans"], dir.path());
    assert!(!output.status.success(), "a typo'd `plans:` key must fail loud, never silently scan zero sources");
    let err = stderr(&output);
    assert!(err.contains("plans"), "error should name the malformed section: {err}");
}

#[test]
fn an_unregistered_dialect_in_config_fails_loud_naming_registered_ids() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  change: local\n  task: local\nplans:\n  sources:\n    - dialect: no-such-dialect\n      root: .\n",
    );

    let output = run_canon(&["ingest", "plans"], dir.path());
    assert!(!output.status.success(), "an unregistered dialect id must fail loud, never a silent zero-source pass");
    let err = stderr(&output);
    assert!(err.contains("no-such-dialect"), "error should name the unknown id: {err}");
    assert!(err.contains("openspec"), "error should name the registered ids: {err}");
}

#[test]
fn a_nonexistent_configured_source_root_fails_loud_naming_the_source() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  change: local\n  task: local\nplans:\n  sources:\n    - dialect: openspec\n      root: does-not-exist\n",
    );

    let output = run_canon(&["ingest", "plans"], dir.path());
    assert!(!output.status.success(), "a nonexistent source root must fail loud before any scan");
    let err = stderr(&output);
    assert!(err.contains("does-not-exist"), "error should name the missing root: {err}");
}

// ── task 3.3: one-shot `--dialect`/`--source` override ──

#[test]
fn one_shot_override_requires_both_flags_dialect_alone_fails() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(dir.path(), "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  change: local\n  task: local\n");

    let output = run_canon(&["ingest", "plans", "--dialect", "openspec"], dir.path());
    assert!(!output.status.success(), "`--dialect` without `--source` must fail loud");
    assert!(stderr(&output).contains("together"), "stderr: {}", stderr(&output));
}

#[test]
fn one_shot_override_requires_both_flags_source_alone_fails() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(dir.path(), "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  change: local\n  task: local\n");

    let output = run_canon(&["ingest", "plans", "--source", "."], dir.path());
    assert!(!output.status.success(), "`--source` without `--dialect` must fail loud");
    assert!(stderr(&output).contains("together"), "stderr: {}", stderr(&output));
}

#[test]
fn one_shot_override_with_an_unknown_dialect_fails_loud() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(dir.path(), "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  change: local\n  task: local\n");

    let output = run_canon(&["ingest", "plans", "--dialect", "no-such-dialect", "--source", "."], dir.path());
    assert!(!output.status.success());
    let err = stderr(&output);
    assert!(err.contains("no-such-dialect") && err.contains("openspec"), "stderr: {err}");
}

#[test]
fn one_shot_override_bypasses_config_and_imports_the_given_source() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(dir.path(), "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  change: local\n  task: local\n");
    write_change_dir(dir.path(), "add-widget", "Adds a widget.", &["- [x] 1.1 wire it — ✅ done", "- [ ] 1.2 ship it"]);

    let json = ingest_json(dir.path(), &["--dialect", "openspec", "--source", "."]);
    assert_eq!(json["changes_persisted"], 1, "{json}");
    assert_eq!(json["tasks_persisted"], 2, "{json}");
}

// ── task 3.4/3.6: watermark cursor gate ──

#[test]
fn an_unchanged_source_rerun_writes_zero_records() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  change: local\n  task: local\nplans:\n  sources:\n    - dialect: openspec\n      root: openspec-src\n",
    );
    let src = dir.path().join("openspec-src");
    std::fs::create_dir_all(&src).unwrap();
    write_change_dir(&src, "add-widget", "Adds a widget.", &["- [x] 1.1 wire it — ✅ done", "- [ ] 1.2 ship it"]);

    let first = ingest_json(dir.path(), &[]);
    assert_eq!(first["changes_persisted"], 1, "{first}");
    assert_eq!(first["tasks_persisted"], 2, "{first}");
    assert_eq!(first["sources"][0]["cursor_advanced"], true, "{first}");

    let second = ingest_json(dir.path(), &[]);
    assert_eq!(second["changes_persisted"], 0, "an unchanged source must write zero new records: {second}");
    assert_eq!(second["tasks_persisted"], 0, "{second}");
    assert_eq!(second["sources"][0]["skipped_unchanged"], true, "{second}");

    // mtime churn without byte churn (a `touch`/`git checkout`-alike):
    // re-write byte-IDENTICAL content — the digest-only gate must still
    // skip wholesale.
    write_change_dir(&src, "add-widget", "Adds a widget.", &["- [x] 1.1 wire it — ✅ done", "- [ ] 1.2 ship it"]);
    let third = ingest_json(dir.path(), &[]);
    assert_eq!(third["changes_persisted"], 0, "byte-identical mtime churn must still be skipped: {third}");
    assert_eq!(third["tasks_persisted"], 0, "{third}");
    assert_eq!(third["sources"][0]["skipped_unchanged"], true, "{third}");
}

#[test]
fn a_checkbox_flip_appends_refreshed_records() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  change: local\n  task: local\nplans:\n  sources:\n    - dialect: openspec\n      root: openspec-src\n",
    );
    let src = dir.path().join("openspec-src");
    std::fs::create_dir_all(&src).unwrap();
    write_change_dir(&src, "add-widget", "Adds a widget.", &["- [ ] 1.1 wire it", "- [ ] 1.2 ship it"]);

    let first = ingest_json(dir.path(), &[]);
    assert_eq!(first["tasks_persisted"], 2, "{first}");

    // Flip one checkbox -- tasks.md's bytes (and mtime) change.
    write_change_dir(&src, "add-widget", "Adds a widget.", &["- [x] 1.1 wire it — ✅ done", "- [ ] 1.2 ship it"]);
    let second = ingest_json(dir.path(), &[]);
    assert_eq!(second["sources"][0]["skipped_unchanged"], false, "{second}");
    assert!(second["tasks_persisted"].as_u64().unwrap() > 0, "the refreshed tasks must be appended, never silently skipped: {second}");
    assert_eq!(second["sources"][0]["cursor_advanced"], true, "{second}");
}

#[test]
fn one_shot_source_dot_at_repo_root_excludes_its_own_ledger_and_cursor_output_from_the_digest() {
    // `--source .` (root == repo root, F2's "a source root that
    // CONTAINS the importer's own repo-local write surface" case): the
    // git ledger (`tiers.git.root`) and the `canon/ingest` cursor tree
    // both land INSIDE the scanned source tree. Without the exclusion
    // this self-churns forever -- pass 1's own writes shift pass 2's
    // digest before `source_unchanged` ever sees a match.
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(dir.path(), "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  change: local\n  task: local\n");
    write_change_dir(dir.path(), "add-widget", "Adds a widget.", &["- [x] 1.1 wire it — ✅ done", "- [ ] 1.2 ship it"]);

    let first = ingest_json(dir.path(), &["--dialect", "openspec", "--source", "."]);
    assert_eq!(first["changes_persisted"], 1, "{first}");
    assert_eq!(first["tasks_persisted"], 2, "{first}");
    assert_eq!(first["sources"][0]["cursor_advanced"], true, "{first}");

    let second = ingest_json(dir.path(), &["--dialect", "openspec", "--source", "."]);
    assert_eq!(
        second["changes_persisted"], 0,
        "the importer's own ledger+cursor output must be excluded from its own digest, or a `--source .` root self-churns forever: {second}"
    );
    assert_eq!(second["tasks_persisted"], 0, "{second}");
    assert_eq!(second["sources"][0]["skipped_unchanged"], true, "{second}");
}

// ── task 3.5: persistence + the `unwritten` seam ──

#[test]
fn pg_unreachable_task_persists_git_change_but_degrades_task_to_unwritten_and_does_not_advance_cursor() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: canon/ledger }\n  hot: { backend: postgres, dsn_env: CANON_PG_DSN_S17_TEST_UNSET, schema: canon_v1 }\nrouting:\n  change: local\n  task: hot\nplans:\n  sources:\n    - dialect: openspec\n      root: openspec-src\n",
    );
    let src = dir.path().join("openspec-src");
    std::fs::create_dir_all(&src).unwrap();
    write_change_dir(&src, "add-widget", "Adds a widget.", &["- [x] 1.1 wire it — ✅ done", "- [ ] 1.2 ship it"]);

    let output = run_canon(&["ingest", "plans", "--json"], dir.path());
    assert!(output.status.success(), "an unreachable pg tier must be non-fatal: stderr={}", stderr(&output));
    let json: Value = serde_json::from_str(&stdout(&output)).unwrap();

    assert_eq!(json["changes_persisted"], 1, "git-routed Change must still persist: {json}");
    assert_eq!(json["tasks_persisted"], 0, "{json}");
    assert_eq!(json["unwritten_tasks"].as_array().unwrap().len(), 2, "every Task candidate must degrade to the unwritten seam: {json}");
    assert_eq!(json["sources"][0]["tasks_unwritten"], 2, "{json}");
    assert_eq!(json["sources"][0]["cursor_advanced"], false, "the pass was not fully durable -- cursor must NOT advance: {json}");

    // The default (non-`--json`) human run must ALSO print the
    // unwritten bodies -- never the one copy of output silently
    // discarded (mirrors `crate::ingest`'s ReviewS3Full finding-4 fix).
    let human = run_canon(&["ingest", "plans"], dir.path());
    assert!(human.status.success());
    let human_text = stdout(&human);
    assert!(human_text.contains("unwritten"), "{human_text}");
    assert!(human_text.contains("cursor NOT advanced"), "{human_text}");
    let json_start = human_text.find('{').unwrap_or_else(|| panic!("no JSON body in default output: {human_text}"));
    let unwritten_body: Value = serde_json::from_str(&human_text[json_start..]).expect("valid JSON printed by default, no --json flag needed");
    assert_eq!(unwritten_body["tasks"].as_array().unwrap().len(), 2, "{unwritten_body}");
}

#[test]
fn a_present_dsn_with_a_malformed_pg_schema_fails_loud_not_a_silent_unwritten_degrade() {
    // Unlike the unset-DSN case above (a documented, non-fatal
    // degrade), a PRESENT `dsn_env` paired with a `tiers.pg.schema`
    // that fails `[a-z0-9_]+` validation must fail the whole command
    // loud -- `PgTier::connect`'s schema check runs BEFORE any socket
    // opens, so this stays a genuinely offline assertion (no live pg
    // needed).
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: canon/ledger }\n  hot: { backend: postgres, dsn_env: CANON_PG_DSN_S17_TEST_MALFORMED_SCHEMA, schema: bad-schema }\nrouting:\n  change: local\n  task: hot\nplans:\n  sources:\n    - dialect: openspec\n      root: openspec-src\n",
    );
    let src = dir.path().join("openspec-src");
    std::fs::create_dir_all(&src).unwrap();
    write_change_dir(&src, "add-widget", "Adds a widget.", &["- [x] 1.1 wire it — ✅ done", "- [ ] 1.2 ship it"]);

    let output = Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(["ingest", "plans", "--json"])
        .env("CANON_PG_DSN_S17_TEST_MALFORMED_SCHEMA", "postgres://unused-because-schema-validation-fails-first/db")
        .current_dir(dir.path())
        .output()
        .expect("spawn canon binary");

    assert!(
        !output.status.success(),
        "a malformed pg schema with a PRESENT dsn_env must fail loud, never exit 0 with the Task silently degraded to unwritten: stdout={} stderr={}",
        stdout(&output),
        stderr(&output)
    );
    let err = stderr(&output);
    assert!(err.contains("bad-schema"), "the fatal error must name the offending schema: {err}");
}

#[test]
fn an_unset_dsn_with_a_malformed_pg_schema_also_fails_loud_never_masked_by_the_degrade() {
    // The F1-residual case operators actually hit: no live pg (`dsn_env`
    // UNSET, the documented degrade path) BUT a `tiers.pg.schema` that
    // fails `[a-z0-9_]+`. A malformed CONFIG must fail loud regardless of
    // DSN presence -- the schema is validated BEFORE the env lookup, so
    // the unset-DSN degrade can never mask a bad schema.
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: canon/ledger }\n  hot: { backend: postgres, dsn_env: CANON_PG_DSN_S17_TEST_UNSET_MALFORMED, schema: bad-schema }\nrouting:\n  change: local\n  task: hot\nplans:\n  sources:\n    - dialect: openspec\n      root: openspec-src\n",
    );
    let src = dir.path().join("openspec-src");
    std::fs::create_dir_all(&src).unwrap();
    write_change_dir(&src, "add-widget", "Adds a widget.", &["- [x] 1.1 wire it — ✅ done", "- [ ] 1.2 ship it"]);

    // Deliberately leave CANON_PG_DSN_S17_TEST_UNSET_MALFORMED unset.
    let output = Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(["ingest", "plans", "--json"])
        .env_remove("CANON_PG_DSN_S17_TEST_UNSET_MALFORMED")
        .current_dir(dir.path())
        .output()
        .expect("spawn canon binary");

    assert!(
        !output.status.success(),
        "a malformed pg schema must fail loud even with an UNSET dsn_env -- never masked by the degrade: stdout={} stderr={}",
        stdout(&output),
        stderr(&output)
    );
    assert!(stderr(&output).contains("bad-schema"), "the fatal error must name the offending schema: {}", stderr(&output));
}

// ── task 3.7 / design D8: cross-source `change_id` collision ──

#[test]
fn cross_source_change_id_collision_first_configured_source_wins() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  change: local\n  task: local\nplans:\n  sources:\n    - dialect: openspec\n      root: source-a\n    - dialect: openspec\n      root: source-b\n",
    );
    let src_a = dir.path().join("source-a");
    let src_b = dir.path().join("source-b");
    std::fs::create_dir_all(&src_a).unwrap();
    std::fs::create_dir_all(&src_b).unwrap();
    write_change_dir(&src_a, "add-widget", "Source A's widget proposal.", &["- [x] 1.1 wire it — ✅ done"]);
    write_change_dir(&src_b, "add-widget", "Source B's widget proposal.", &["- [ ] 1.1 wire it", "- [ ] 1.2 ship it"]);

    let json = ingest_json(dir.path(), &[]);
    assert_eq!(json["changes_persisted"], 1, "only the FIRST-configured source's `add-widget` Change is ever imported: {json}");
    assert_eq!(json["tasks_persisted"], 1, "only source A's single task, never source B's two: {json}");
    assert_eq!(json["duplicate_change_id"], 1, "{json}");
    assert_eq!(json["sources"][0]["duplicate_change_id"], 0, "the FIRST-configured source never loses a collision: {json}");
    assert_eq!(json["sources"][1]["duplicate_change_id"], 1, "the SECOND-configured source's `add-widget` is the one skipped: {json}");

    // Only ONE `Change` record actually landed in the git tier, and it
    // carries source A's content -- never two competing histories from
    // one pass.
    let ledger_root = dir.path().join("canon/ledger");
    let read = GitTier::new(&ledger_root).read(&TierQuery::kind(RecordKind::Change)).expect("read the git tier");
    assert_eq!(read.records.len(), 1, "exactly one Change record, never two competing add-widget histories: {:?}", read.records);
    assert_eq!(read.records[0].0["summary"], "Source A's widget proposal.", "the FIRST-configured source's content is the one that landed: {:?}", read.records[0]);
}

#[test]
fn cross_source_change_id_collision_is_visible_in_the_default_human_summary() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  change: local\n  task: local\nplans:\n  sources:\n    - dialect: openspec\n      root: source-a\n    - dialect: openspec\n      root: source-b\n",
    );
    let src_a = dir.path().join("source-a");
    let src_b = dir.path().join("source-b");
    std::fs::create_dir_all(&src_a).unwrap();
    std::fs::create_dir_all(&src_b).unwrap();
    write_change_dir(&src_a, "add-widget", "Source A's widget proposal.", &["- [x] 1.1 wire it — ✅ done"]);
    write_change_dir(&src_b, "add-widget", "Source B's widget proposal.", &["- [ ] 1.1 wire it", "- [ ] 1.2 ship it"]);

    let output = run_canon(&["ingest", "plans"], dir.path());
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("1 duplicate-change-id"), "the NAMED duplicate-change-id diagnostic must be visible in the default summary: {text}");
    assert!(text.contains("total: 1 change(s) persisted, 1 task(s) persisted, 1 duplicate-change-id skipped"), "{text}");
}

// ── design R1: plan import is never an authority ──

fn write_evidence(ledger_root: &Path, task_id: &str) {
    let envelope = Envelope::new(1, RecordKind::EvidenceRecord, chrono::Utc::now(), Actor::new("it-agent", RoleId::parse("implementer").unwrap()));
    let record = EvidenceRecord::new(envelope, Some(TaskId::parse(task_id).unwrap()), None, None, EvidenceVerdict::Faithful);
    GitTier::new(ledger_root).write(&record).expect("write evidence record");
}

#[test]
fn canon_gate_check_verdicts_are_byte_identical_with_and_without_a_prior_plan_import() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  change: local\n  task: local\nplans:\n  sources:\n    - dialect: openspec\n      root: openspec-src\n",
    );
    write_evidence(&dir.path().join("canon/ledger"), "unrelated-change#1");

    let src = dir.path().join("openspec-src");
    std::fs::create_dir_all(&src).unwrap();
    write_change_dir(&src, "add-widget", "Adds a widget.", &["- [x] 1.1 wire it — ✅ done", "- [ ] 1.2 ship it"]);

    let before = run_canon(&["gate", "check", "--repo", "."], dir.path());
    assert!(before.status.success(), "stderr: {}", stderr(&before));
    let before_text = stdout(&before);

    let ingest = run_canon(&["ingest", "plans"], dir.path());
    assert!(ingest.status.success(), "stderr: {}", stderr(&ingest));

    let after = run_canon(&["gate", "check", "--repo", "."], dir.path());
    assert!(after.status.success(), "stderr: {}", stderr(&after));
    let after_text = stdout(&after);

    assert_eq!(before_text, after_text, "canon gate check verdicts must be byte-identical with and without a prior `canon ingest plans` run");
}

// ── smoke ──

#[test]
fn ingest_plans_help_smoke() {
    let dir = tempfile::tempdir().unwrap();
    let output = run_canon(&["ingest", "plans", "--help"], dir.path());
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(text.contains("--dialect"));
    assert!(text.contains("--source"));
    assert!(text.contains("--repo"));
    assert!(text.contains("--json"));
}

// ── s18 `loud-plan-import-diagnostics` (B1) ──

#[test]
fn root_pointed_one_level_above_openspec_changes_exits_non_zero_with_named_diagnostic_and_hint() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  change: local\n  task: local\nplans:\n  sources:\n    - dialect: openspec\n      root: openspec\n",
    );
    // The real change dir lives at `openspec/changes/widget-feature/` --
    // but `root: openspec` (one level too high) makes the adapter scan
    // `openspec`'s own immediate children instead, finding only the
    // `changes` directory itself (no proposal.md at THAT level) -- the
    // exact SYNTHESIS-reproduced near-miss.
    write_change_dir(dir.path(), "widget-feature", "Adds a widget.", &["- [x] 1.1 wire it — ✅ done"]);

    // s23 durable-import-diagnostics: a malformed-only pass must stay
    // loud on EVERY run against the same unfixed config, not just run
    // #1 -- the cursor is never written for a wholly-unproductive
    // pass, so `source_unchanged` has nothing to compare against and
    // the full parse + diagnostic path re-runs from scratch each time.
    for run_number in 1..=3 {
        let output = run_canon(&["ingest", "plans", "--json"], dir.path());
        assert!(!output.status.success(), "run #{run_number}: the near-miss must exit non-zero, never the silent 0 the SYNTHESIS reproduces");

        let err = stderr(&output);
        assert!(err.contains("WARN"), "run #{run_number} stderr: {err}");
        assert!(err.contains("openspec"), "run #{run_number}: WARN must name the source's dialect: {err}");
        assert!(err.contains('1'), "run #{run_number}: WARN must name the malformed count: {err}");

        let json: Value = serde_json::from_str(&stdout(&output)).unwrap_or_else(|e| panic!("run #{run_number}: invalid JSON: {e}\nstdout: {}", stdout(&output)));
        assert_eq!(json["changes_persisted"], 0, "run #{run_number}: {json}");
        assert_eq!(json["tasks_persisted"], 0, "run #{run_number}: {json}");
        assert_eq!(json["sources"][0]["skipped_unchanged"], false, "run #{run_number}: a malformed-only source must never be silently skipped: {json}");
        assert_eq!(json["sources"][0]["cursor_advanced"], false, "run #{run_number}: a malformed-only, zero-persisted pass must never advance its cursor: {json}");
        let malformed = json["sources"][0]["malformed"].as_array().expect("malformed must be a named-entry array, not a bare count");
        assert_eq!(malformed.len(), 1, "run #{run_number}: {json}");
        assert!(malformed[0]["path"].as_str().unwrap().ends_with("changes"), "run #{run_number}: {json}");
        assert_eq!(malformed[0]["reason"], "missing-proposal-md", "run #{run_number}: {json}");
        let hint = malformed[0]["hint"].as_str().expect("the `changes`-basename near-miss must carry the root hint");
        assert!(hint.contains("root:"), "run #{run_number} hint: {hint}");
    }
}

#[test]
fn a_malformed_only_source_becomes_quiet_once_the_config_is_fixed() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  change: local\n  task: local\nplans:\n  sources:\n    - dialect: openspec\n      root: openspec\n",
    );
    write_change_dir(dir.path(), "widget-feature", "Adds a widget.", &["- [x] 1.1 wire it — ✅ done"]);

    // Run #1: `root: openspec` (one level too high) -- malformed-only,
    // zero-persisted, loud, no cursor written (s23).
    let first = run_canon(&["ingest", "plans", "--json"], dir.path());
    assert!(!first.status.success(), "run #1 must fail loud: stdout={}", stdout(&first));
    let first_json: Value = serde_json::from_str(&stdout(&first)).unwrap();
    assert_eq!(first_json["sources"][0]["cursor_advanced"], false, "{first_json}");

    // Fix: give `openspec/changes/` (root's own immediate child, the
    // exact dir the near-miss flagged) a real proposal.md/tasks.md of
    // its own -- `root:` stays UNCHANGED, so this exercises the SAME
    // source/cursor id run #1's withheld cursor never wrote to.
    let changes_dir = dir.path().join("openspec/changes");
    std::fs::write(changes_dir.join("proposal.md"), "## Why\n\nFixed at last.\n\n## What Changes\n\n- does the thing\n").unwrap();
    std::fs::write(changes_dir.join("tasks.md"), "## 1. Work\n\n- [x] 1.1 wire it — ✅ done\n").unwrap();

    // Run #2: a normal fully-durable pass -- real records persist, a
    // cursor is finally earned, no manual reset step needed.
    let second = ingest_json(dir.path(), &[]);
    assert_eq!(second["changes_persisted"], 1, "{second}");
    assert_eq!(second["tasks_persisted"], 1, "{second}");
    assert_eq!(second["sources"][0]["skipped_unchanged"], false, "{second}");
    assert_eq!(second["sources"][0]["cursor_advanced"], true, "a clean pass must finally earn a cursor: {second}");

    // Run #3: the fixed config is now unchanged -- quiet, exit 0.
    let third_output = run_canon(&["ingest", "plans", "--json"], dir.path());
    assert!(third_output.status.success(), "run #3 must be a quiet no-op once fixed: stderr={}", stderr(&third_output));
    assert!(!stderr(&third_output).contains("WARN"), "stderr: {}", stderr(&third_output));
    let third: Value = serde_json::from_str(&stdout(&third_output)).unwrap();
    assert_eq!(third["sources"][0]["skipped_unchanged"], true, "{third}");
    assert_eq!(third["changes_persisted"], 0, "{third}");
}

#[test]
fn a_legitimately_empty_source_stays_a_clean_silent_no_op() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  change: local\n  task: local\nplans:\n  sources:\n    - dialect: openspec\n      root: .\n",
    );
    // A well-formed, genuinely empty source: `openspec/changes/` exists
    // but has zero change dirs yet -- `malformed == 0`.
    std::fs::create_dir_all(dir.path().join("openspec/changes")).unwrap();

    let first_output = run_canon(&["ingest", "plans", "--json"], dir.path());
    assert!(first_output.status.success(), "a legitimately empty source must stay a clean 0-exit no-op: stderr={}", stderr(&first_output));
    assert!(!stderr(&first_output).contains("WARN"), "no WARN for a genuinely empty/fresh plan tree: {}", stderr(&first_output));
    let first: Value = serde_json::from_str(&stdout(&first_output)).unwrap();
    assert_eq!(
        first["sources"][0]["cursor_advanced"], true,
        "a genuinely empty, well-formed source must still earn a cursor (s23 targets `malformed > 0`, never `changes_parsed == 0` alone): {first}"
    );

    // Run #2: unchanged and still empty -- must be a silent skip, not
    // a re-scan on every run (s23's exclusion is scoped to `malformed
    // > 0`, never a bare empty source).
    let second_output = run_canon(&["ingest", "plans", "--json"], dir.path());
    assert!(second_output.status.success(), "run #2 must stay a clean 0-exit no-op: stderr={}", stderr(&second_output));
    assert!(!stderr(&second_output).contains("WARN"), "{}", stderr(&second_output));
    let second: Value = serde_json::from_str(&stdout(&second_output)).unwrap();
    assert_eq!(second["sources"][0]["skipped_unchanged"], true, "{second}");
}

#[test]
fn a_source_with_some_malformed_dirs_but_at_least_one_persisted_record_stays_clean() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  change: local\n  task: local\nplans:\n  sources:\n    - dialect: openspec\n      root: .\n",
    );
    write_change_dir(dir.path(), "add-widget", "Adds a widget.", &["- [x] 1.1 wire it — ✅ done"]);
    // A sibling malformed dir (basename fails ChangeId's grammar) sits
    // alongside the well-formed one -- a partial success, not the
    // targeted near-miss.
    std::fs::create_dir_all(dir.path().join("openspec/changes/Bad_Slug!")).unwrap();
    std::fs::write(dir.path().join("openspec/changes/Bad_Slug!/proposal.md"), "## Why\n\nirrelevant\n").unwrap();

    let first_output = run_canon(&["ingest", "plans", "--json"], dir.path());
    assert!(first_output.status.success(), "a source with SOME malformed dirs but at least one persisted record must stay clean: stderr={}", stderr(&first_output));
    assert!(!stderr(&first_output).contains("WARN"), "stderr: {}", stderr(&first_output));
    let first: Value = serde_json::from_str(&stdout(&first_output)).unwrap();
    assert_eq!(
        first["sources"][0]["cursor_advanced"], true,
        "partial success must still advance its cursor -- s23 does not widen the exclusion to ANY malformed entry: {first}"
    );

    // Run #2: unchanged partial-success source -- s23's exclusion must
    // NOT widen to "any malformed entry blocks the cursor" -- the
    // cursor written on run #1 gates the whole re-scan away.
    let second_output = run_canon(&["ingest", "plans", "--json"], dir.path());
    assert!(second_output.status.success(), "run #2 must stay a clean, silent no-op: stderr={}", stderr(&second_output));
    assert!(!stderr(&second_output).contains("WARN"), "stderr: {}", stderr(&second_output));
    let second: Value = serde_json::from_str(&stdout(&second_output)).unwrap();
    assert_eq!(second["sources"][0]["skipped_unchanged"], true, "{second}");
    assert_eq!(second["changes_persisted"], 0, "{second}");
}

#[test]
fn one_flagged_source_among_several_makes_the_whole_pass_non_zero() {
    let dir = tempfile::tempdir().unwrap();
    // Two independently-configured roots: `good` (a well-formed
    // source) and `bad/openspec` (the near-miss, one level too high).
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  change: local\n  task: local\nplans:\n  sources:\n    - dialect: openspec\n      root: good\n    - dialect: openspec\n      root: bad/openspec\n",
    );
    write_change_dir(&dir.path().join("good"), "add-widget", "Adds a widget.", &["- [x] 1.1 wire it — ✅ done"]);
    write_change_dir(&dir.path().join("bad"), "widget-feature", "Adds a widget.", &["- [x] 1.1 wire it — ✅ done"]);

    let output = run_canon(&["ingest", "plans", "--json"], dir.path());
    assert!(!output.status.success(), "one flagged source among several must make the whole pass non-zero");

    let json: Value = serde_json::from_str(&stdout(&output)).unwrap_or_else(|e| panic!("invalid JSON: {e}\nstdout: {}", stdout(&output)));
    assert_eq!(json["changes_persisted"], 1, "the well-formed source's records must still persist normally, visible in the summary: {json}");
    assert_eq!(json["tasks_persisted"], 1, "{json}");
}

// ── s30 plan-dialect-superpowers task 2.1: the `superpowers` dialect ──

#[test]
fn superpowers_dialect_end_to_end_import_via_one_shot_override() {
    // s30 spec.md "End-to-end CLI import of a fixture plan corpus": a
    // `--dialect superpowers --source <fixture-root>` run against a
    // local-routed canon.yaml and a fixture plan doc with one done and
    // one open task exits 0, and `canon query --kind change`/`--kind
    // task` return the imported records with the derived statuses.
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(dir.path(), "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  change: local\n  task: local\n");
    write_plan_doc(
        &dir.path().join("plans"),
        "2026-07-14-demo-feature.md",
        "Demo goal.",
        &[("Task 1: Adapter", &["- [x] step one", "- [x] step two"]), ("Task 2: Docs", &["- [x] step one", "- [ ] step two"])],
    );

    let json = ingest_json(dir.path(), &["--dialect", "superpowers", "--source", "plans"]);
    assert_eq!(json["changes_persisted"], 1, "{json}");
    assert_eq!(json["tasks_persisted"], 2, "{json}");

    let change_output = run_canon(&["query", "--kind", "change", "--json"], dir.path());
    assert!(change_output.status.success(), "stderr: {}", stderr(&change_output));
    let change_payload: Value = serde_json::from_str(&stdout(&change_output)).unwrap();
    let change_records = change_payload["records"].as_array().unwrap();
    assert_eq!(change_records.len(), 1, "{change_records:?}");
    assert_eq!(change_records[0]["change_id"], "2026-07-14-demo-feature");
    assert_eq!(change_records[0]["summary"], "Demo goal.");
    assert_eq!(change_records[0]["status"], "in_progress", "one done + one open task -> in_progress: {change_records:?}");

    let task_output = run_canon(&["query", "--kind", "task", "--json"], dir.path());
    assert!(task_output.status.success(), "stderr: {}", stderr(&task_output));
    let task_payload: Value = serde_json::from_str(&stdout(&task_output)).unwrap();
    let mut task_records: Vec<Value> = task_payload["records"].as_array().unwrap().clone();
    task_records.sort_by(|a, b| a["task_id"].as_str().cmp(&b["task_id"].as_str()));
    assert_eq!(task_records.len(), 2, "{task_records:?}");
    assert_eq!(task_records[0]["task_id"], "2026-07-14-demo-feature#1");
    assert_eq!(task_records[0]["title"], "Adapter");
    assert_eq!(task_records[0]["status"], "done");
    assert_eq!(task_records[1]["task_id"], "2026-07-14-demo-feature#2");
    assert_eq!(task_records[1]["title"], "Docs");
    assert_eq!(task_records[1]["status"], "open");
}

#[test]
fn superpowers_dialect_resolves_through_the_canon_yaml_plans_config_path() {
    // s30 spec.md "The dialect registers through the one-entry seam and
    // the CLI resolves it": `plans.sources[].dialect: superpowers`
    // resolves through the SAME `plan_registry::find` lookup the
    // one-shot override uses above -- bare `canon ingest plans`, no
    // `--dialect`/`--source` flags.
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  change: local\n  task: local\nplans:\n  sources:\n    - dialect: superpowers\n      root: plans\n",
    );
    write_plan_doc(&dir.path().join("plans"), "2026-07-14-config-path.md", "Config-path goal.", &[("Task 1: Only", &["- [x] step one"])]);

    let output = run_canon(&["ingest", "plans"], dir.path());
    assert!(output.status.success(), "stderr: {}", stderr(&output));

    let change_output = run_canon(&["query", "--kind", "change", "--json"], dir.path());
    assert!(change_output.status.success(), "stderr: {}", stderr(&change_output));
    let change_payload: Value = serde_json::from_str(&stdout(&change_output)).unwrap();
    let change_records = change_payload["records"].as_array().unwrap();
    assert_eq!(change_records.len(), 1, "{change_records:?}");
    assert_eq!(change_records[0]["change_id"], "2026-07-14-config-path");
    assert_eq!(change_records[0]["status"], "completed", "the lone task is done -> completed: {change_records:?}");

    let task_output = run_canon(&["query", "--kind", "task", "--json"], dir.path());
    assert!(task_output.status.success(), "stderr: {}", stderr(&task_output));
    let task_payload: Value = serde_json::from_str(&stdout(&task_output)).unwrap();
    let task_records = task_payload["records"].as_array().unwrap();
    assert_eq!(task_records.len(), 1, "{task_records:?}");
    assert_eq!(task_records[0]["task_id"], "2026-07-14-config-path#1");
    assert_eq!(task_records[0]["status"], "done");
}
