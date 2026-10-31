//! Integration test for `canon query --kind <k> [--since <t>] [--json]`
//! (S2 task 4.1), invoking the actually-built `canon` binary against an
//! offline git+r2(local) fixture (`support::Fixture`) — zero network,
//! no credentials.
//!
//! Covers the unified-query spec's own scenarios: a kind split across
//! its routed (git) tier and its aging destination (r2) merges into one
//! output, ordered by `at`, with no duplicate and no gap; and a
//! `--since` filter returns only records at or after the given
//! timestamp.

mod support;

use chrono::{Duration, Utc};
use serde_json::Value;

/// `data` of the one `{ok,data,meta}` document a `--json` success prints.
fn data_of(stdout: &str) -> Value {
    let body: Value = serde_json::from_str(stdout).expect("valid JSON on stdout");
    assert_eq!(body["ok"], true, "{body}");
    body["data"].clone()
}

const ROUTING: &str = "  trajectory: local\n";
const AGING: &str = "  trajectory: { after: 1d, to: cold }\n";

#[test]
fn merges_records_split_across_the_routed_tier_and_its_aging_destination() {
    let fixture = support::Fixture::new(ROUTING, AGING);
    // `older` simulates an already-aged record living in r2 (planted
    // directly, no `tier age` run needed for this test); `newer` is
    // still in git, its routed tier.
    fixture.plant_trajectory_in_r2(Utc::now() - Duration::days(5), 0.1);
    fixture.plant_trajectory_in_git(Utc::now(), 0.2);

    let output = fixture.run_canon(&["query", "--kind", "trajectory", "--json"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));

    let payload = data_of(&support::stdout(&output));
    assert_eq!(payload["kind"], "trajectory");
    assert_eq!(payload["count"], 2);
    let records = payload["records"].as_array().expect("records array");
    assert_eq!(records.len(), 2, "must see records from BOTH the routed tier and the aging destination");
    // Merged and ordered by `at`: the older (r2) record first.
    assert_eq!(records[0]["reward"], 0.1);
    assert_eq!(records[1]["reward"], 0.2);
}

#[test]
fn since_filters_to_records_at_or_after_the_given_timestamp() {
    let fixture = support::Fixture::new(ROUTING, AGING);
    // Whole seconds, so the boundary record's stored `at` and the
    // `--since` argument name the identical instant.
    let cutoff = chrono::DateTime::from_timestamp((Utc::now() - Duration::days(2)).timestamp(), 0).unwrap();
    fixture.plant_trajectory_in_r2(Utc::now() - Duration::days(5), 0.1); // before cutoff
    fixture.plant_trajectory_in_git(cutoff, 0.15); // exactly at cutoff
    fixture.plant_trajectory_in_git(Utc::now(), 0.2); // after cutoff

    let output = fixture.run_canon(&["query", "--kind", "trajectory", "--since", &cutoff.to_rfc3339(), "--json"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));

    let payload = data_of(&support::stdout(&output));
    let records = payload["records"].as_array().expect("records array");
    let rewards: Vec<f64> = records.iter().map(|record| record["reward"].as_f64().unwrap()).collect();
    assert_eq!(rewards, vec![0.15, 0.2], "the at-cutoff and newer records must be returned, the older one filtered out");
}

/// query.read.04: two otherwise-identical attestations of one scenario at
/// two pinned commits are stored as two records under one key (they differ by digest) and
/// `canon query --kind review` returns BOTH — reviews are never folded
/// by key, so neither attestation is dropped as a stale version.
#[test]
fn two_reviews_of_one_scenario_at_two_pins_are_two_records_and_both_read_back() {
    let fixture = support::Fixture::new("  review: local\n", "");
    let at = Utc::now() - Duration::hours(1);
    let first = fixture.plant_review_in_git("app-a", "world.place-lock.01", "abc123", at);
    let second = fixture.plant_review_in_git("app-a", "world.place-lock.01", "def456", at);
    // The pin is not in the name (0.30 R2): both land under one key prefix, told apart by digest.
    assert!(first.location.starts_with("kind=review/app-a_scenario_world.place-lock.01_"), "{}", first.location);
    assert!(second.location.starts_with("kind=review/app-a_scenario_world.place-lock.01_"), "{}", second.location);
    assert_ne!(first.location, second.location);

    let output = fixture.run_canon(&["query", "--kind", "review", "--json"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));
    let payload = data_of(&support::stdout(&output));
    assert_eq!(payload["count"], 1, "the ledger holds the one in-effect review of this reviewer: {payload}");
    let history = data_of(&support::stdout(&fixture.run_canon(&["query", "--kind", "review", "--json", "--history"])));
    assert_eq!(history["count"], 2, "--history returns both attestations: {history}");
    let payload = history;
    let records = payload["records"].as_array().expect("records array");
    let mut keys: Vec<String> = records
        .iter()
        .map(|record| canon_store::partition::resolve_partition(canon_model::envelope::RecordKind::Review, record).unwrap().natural_key)
        .collect();
    keys.sort();
    assert_eq!(keys, vec!["app-a_scenario_world.place-lock.01".to_string(); 2], "two records under one key");
    let mut pins: Vec<&str> = records.iter().map(|r| r["pin"].as_str().unwrap()).collect();
    pins.sort_unstable();
    assert_eq!(pins, ["abc123", "def456"], "the pin stays in the body");
    for record in records {
        assert_eq!(record["project_id"], "app-a");
        assert_eq!(record["scenario_id"], "world.place-lock.01");
        assert_eq!(record["reviewer"], "reviewer");
    }
}

#[test]
fn human_output_reports_kind_since_and_count() {
    let fixture = support::Fixture::new(ROUTING, AGING);
    fixture.plant_trajectory_in_git(Utc::now(), 0.5);

    let output = fixture.run_canon(&["query", "--kind", "trajectory"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));
    let stdout = support::stdout(&output);
    assert!(stdout.contains("--kind trajectory"), "stdout: {stdout}");
    assert!(stdout.contains("1 record(s)"), "stdout: {stdout}");
    // Not JSON — no top-level braces on the header line.
    assert!(!stdout.trim_start().starts_with('{'), "stdout: {stdout}");
}

#[test]
fn unknown_kind_is_a_clean_cli_error() {
    let fixture = support::Fixture::new(ROUTING, AGING);
    let output = fixture.run_canon(&["query", "--kind", "not-a-real-kind"]);
    assert!(!output.status.success());
    let stderr = support::stderr(&output);
    assert!(stderr.starts_with("canon: USAGE: invalid value 'not-a-real-kind' for '--kind <KIND>'\n"), "stderr: {stderr}");
    assert!(stderr.contains("\n  field: --kind\n") && stderr.contains("\n  expected: change, task, scenario,") && stderr.contains("trajectory"), "the closed set is the `expected` member: {stderr}");
    assert!(!stderr.contains("possible values:"), "stderr: {stderr}");
    assert!(support::stdout(&output).is_empty(), "a text-mode error leaves stdout empty");
}

/// s18 `uniform-repo-resolution` spec: "canon query with no flags resolves
/// from a subdirectory" — matching `canon context`/`canon gate check`'s own
/// D7 subdirectory-invocation behavior (`tests/context.rs`'s
/// `context_from_a_subdirectory_resolves_the_ancestor_repo_root_policy`).
#[test]
fn query_with_no_flags_resolves_from_a_subdirectory_of_the_repo_root() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::write(repo.path().join("canon.yaml"), "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\n  change: local\n").unwrap();
    let subdir = repo.path().join("nested").join("deep");
    std::fs::create_dir_all(&subdir).unwrap();

    let output =
        std::process::Command::new(env!("CARGO_BIN_EXE_canon")).args(["query", "--kind", "change"]).current_dir(&subdir).output().expect("spawn canon binary");
    assert!(
        output.status.success(),
        "canon query from a subdirectory must resolve the ancestor repo root, matching canon context/gate check; stderr: {}",
        support::stderr(&output)
    );
}

/// s18 `uniform-repo-resolution` spec: "Running from the repo root itself is
/// unaffected" — the ancestor walk starting at cwd finds `canon.yaml`
/// immediately, exactly as before this change.
#[test]
fn query_with_no_flags_at_the_repo_root_resolves_and_succeeds() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::write(repo.path().join("canon.yaml"), "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\n  change: local\n").unwrap();

    let output =
        std::process::Command::new(env!("CARGO_BIN_EXE_canon")).args(["query", "--kind", "change"]).current_dir(repo.path()).output().expect("spawn canon binary");
    assert!(
        output.status.success(),
        "canon query at the repo root with no flags must resolve exactly as before this change; stderr: {}",
        support::stderr(&output)
    );
}

/// s18 `uniform-repo-resolution` spec: "An explicit non-default --repo is
/// used as-is, no walk" — cwd carries NO `canon.yaml` among its ancestors,
/// so a walk starting from cwd would fail; the command must still succeed
/// because `--repo <dir>` (non-`.`) is used AS-IS.
#[test]
fn query_explicit_non_default_repo_is_used_as_is_no_walk() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::write(repo.path().join("canon.yaml"), "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\n  change: local\n").unwrap();
    let cwd = tempfile::tempdir().unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(["query", "--kind", "change", "--repo"])
        .arg(repo.path())
        .current_dir(cwd.path())
        .output()
        .expect("spawn canon binary");
    assert!(
        output.status.success(),
        "an explicit non-`.` --repo must be used as-is, no ancestor walk from cwd; stderr: {}",
        support::stderr(&output)
    );
}

/// s18 `uniform-repo-resolution` spec: "A missing canon.yaml at every
/// ancestor still fails loud, named" — a genuinely unconfigured cwd never
/// silently succeeds with an empty result.
#[test]
fn query_with_no_canon_yaml_anywhere_fails_loud_naming_the_attempted_path() {
    let cwd = tempfile::tempdir().unwrap();
    let output =
        std::process::Command::new(env!("CARGO_BIN_EXE_canon")).args(["query", "--kind", "change"]).current_dir(cwd.path()).output().expect("spawn canon binary");
    assert!(!output.status.success(), "a cwd with no canon.yaml anywhere among its ancestors must fail, never silently succeed");
    let stderr = support::stderr(&output);
    assert!(stderr.contains("canon.yaml"), "the error must name the canon.yaml path the ancestor walk ultimately attempted to read: {stderr}");
}

/// Every entry under `dir`, recursively, as sorted relative paths.
fn tree(dir: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next).unwrap().filter_map(Result::ok) {
            let path = entry.path();
            out.push(path.strip_prefix(dir).unwrap().display().to_string());
            if path.is_dir() {
                stack.push(path);
            }
        }
    }
    out.sort();
    out
}

/// 0.15 D6 (dogfood eval 2 G14): a read command run in a directory canon
/// was never set up in leaves it exactly as it was. `canon query` used to
/// create `.canon/audit/access.jsonl` there, an untracked write into a
/// repository the operator had only read.
///
/// `canon report --check` is held to the audit half only: it seeds
/// `.canon/learn/` on its own, a separate write this change does not
/// touch.
#[test]
fn read_commands_leave_a_directory_without_canon_yaml_unchanged() {
    let cwd = tempfile::tempdir().unwrap();
    for args in [&["query", "--kind", "change"][..], &["query", "--kind", "subject", "--json"][..], &["status"][..], &["context"][..], &["gate", "check"][..]] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(cwd.path()).output().expect("spawn canon binary");
        assert_eq!(tree(cwd.path()), Vec::<String>::new(), "`canon {}` wrote into a directory without canon.yaml; stderr: {}", args.join(" "), support::stderr(&output));
    }
    let report = tempfile::tempdir().unwrap();
    std::process::Command::new(env!("CARGO_BIN_EXE_canon")).args(["report", "--check"]).current_dir(report.path()).output().expect("spawn canon binary");
    assert!(!report.path().join(".canon/audit").exists(), "`canon report --check` must not write an access audit here");
}

/// The access audit lands in the repository whose `canon.yaml` was read
/// — through `--repo` — never in the directory the command happened to run
/// from.
#[test]
fn query_audits_into_the_repo_it_read_never_into_the_cwd() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::write(repo.path().join("canon.yaml"), "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\n  change: local\n").unwrap();
    let cwd = tempfile::tempdir().unwrap();

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(["query", "--kind", "change", "--repo"])
        .arg(repo.path())
        .current_dir(cwd.path())
        .output()
        .expect("spawn canon binary");
    assert!(output.status.success(), "--repo: {}", support::stderr(&output));
    assert_eq!(tree(cwd.path()), Vec::<String>::new(), "--repo: the cwd must be left unchanged");
    let audit = std::fs::read_to_string(repo.path().join(".canon/audit/access.jsonl")).expect("the target repo carries the audit");
    assert_eq!(audit.lines().count(), 1, "one event per query: {audit}");
}

// ── s19 `query-scope-filters`: --change/--status + rollup + deterministic sort ──

use canon_model::records::{ChangeStatus, TaskStatus};

const CHANGE_TASK_ROUTING: &str = "  change: local\n  task: local\n";
const CHANGE_TASK_AGING: &str = "";

#[test]
fn change_id_flag_with_an_unsupported_kind_fails_loud() {
    let fixture = support::Fixture::new(ROUTING, AGING);
    let output = fixture.run_canon(&["query", "--kind", "trajectory", "--change", "wall-render"]);
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    let stderr = support::stderr(&output);
    assert!(stderr.contains("--change"), "stderr: {stderr}");
    assert!(stderr.contains("--kind change"), "stderr must name the two supported kinds: {stderr}");
}

#[test]
fn status_flag_with_an_unsupported_kind_fails_loud() {
    let fixture = support::Fixture::new(ROUTING, AGING);
    let output = fixture.run_canon(&["query", "--kind", "trajectory", "--status", "done"]);
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    let stderr = support::stderr(&output);
    assert!(stderr.contains("--status"), "stderr: {stderr}");
    assert!(stderr.contains("--kind task"), "stderr must name the two supported kinds: {stderr}");
}

#[test]
fn change_id_scopes_a_task_query_to_one_changes_rows() {
    let fixture = support::Fixture::new(CHANGE_TASK_ROUTING, CHANGE_TASK_AGING);
    let now = Utc::now();
    fixture.plant_task_in_git("add-audio-reactive#1.1", "t1", TaskStatus::Open, now);
    fixture.plant_task_in_git("add-audio-reactive#2.2", "t2", TaskStatus::Done, now + Duration::seconds(1));
    fixture.plant_task_in_git("add-widget#1.1", "t3", TaskStatus::Open, now + Duration::seconds(2));

    let output = fixture.run_canon(&["query", "--kind", "task", "--change", "add-audio-reactive", "--json"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));
    let payload = data_of(&support::stdout(&output));
    let ids: Vec<String> = payload["records"].as_array().unwrap().iter().map(|r| r["task_id"].as_str().unwrap().to_string()).collect();
    assert_eq!(ids, vec!["add-audio-reactive#1.1", "add-audio-reactive#2.2"], "add-widget#1.1 must be excluded: {ids:?}");
}

#[test]
fn change_id_scopes_a_change_query_to_at_most_one_record() {
    let fixture = support::Fixture::new(CHANGE_TASK_ROUTING, CHANGE_TASK_AGING);
    let now = Utc::now();
    fixture.plant_change_in_git("add-widget", "Add widget", ChangeStatus::Proposed, now);
    fixture.plant_change_in_git("add-widget", "Add widget, refined", ChangeStatus::InProgress, now + Duration::seconds(1));
    fixture.plant_change_in_git("add-other", "Add other", ChangeStatus::Proposed, now + Duration::seconds(2));

    let output = fixture.run_canon(&["query", "--kind", "change", "--change", "add-widget", "--json"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));
    let payload = data_of(&support::stdout(&output));
    let records = payload["records"].as_array().unwrap();
    assert_eq!(records.len(), 1, "the two add-widget versions fold to the current one (0.18 D4), add-other excluded: {records:?}");
    assert_eq!(records[0]["title"], "Add widget, refined", "the newer version wins: {records:?}");
}

#[test]
fn status_filters_task_records_by_open_done() {
    let fixture = support::Fixture::new(CHANGE_TASK_ROUTING, CHANGE_TASK_AGING);
    let now = Utc::now();
    fixture.plant_task_in_git("add-widget#1.1", "open one", TaskStatus::Open, now);
    fixture.plant_task_in_git("add-widget#1.2", "done one", TaskStatus::Done, now + Duration::seconds(1));

    let output = fixture.run_canon(&["query", "--kind", "task", "--status", "open", "--json"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));
    let payload = data_of(&support::stdout(&output));
    let records = payload["records"].as_array().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["task_id"], "add-widget#1.1");
}

#[test]
fn status_filters_change_records_by_their_four_value_domain() {
    let fixture = support::Fixture::new(CHANGE_TASK_ROUTING, CHANGE_TASK_AGING);
    let now = Utc::now();
    fixture.plant_change_in_git("add-widget", "Add widget", ChangeStatus::InProgress, now);
    fixture.plant_change_in_git("add-other", "Add other", ChangeStatus::Proposed, now + Duration::seconds(1));

    let output = fixture.run_canon(&["query", "--kind", "change", "--status", "in_progress", "--json"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));
    let payload = data_of(&support::stdout(&output));
    let records = payload["records"].as_array().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["change_id"], "add-widget");
}

#[test]
fn a_status_value_outside_the_queried_kinds_domain_fails_loud() {
    let fixture = support::Fixture::new(CHANGE_TASK_ROUTING, CHANGE_TASK_AGING);
    let output = fixture.run_canon(&["query", "--kind", "task", "--status", "archived"]);
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    let stderr = support::stderr(&output);
    assert!(stderr.contains("open"), "stderr must name task's valid status values: {stderr}");
    assert!(stderr.contains("done"), "stderr must name task's valid status values: {stderr}");
}

#[test]
fn rollup_reflects_the_filtered_result_set_not_the_whole_ledger() {
    let fixture = support::Fixture::new(CHANGE_TASK_ROUTING, CHANGE_TASK_AGING);
    let now = Utc::now();
    for (n, status) in [(1, TaskStatus::Done), (2, TaskStatus::Done), (3, TaskStatus::Open), (4, TaskStatus::Open), (5, TaskStatus::Open), (6, TaskStatus::Open)] {
        fixture.plant_task_in_git(&format!("add-audio-reactive#1.{n}"), "t", status, now + Duration::seconds(n));
    }
    fixture.plant_task_in_git("add-widget#1.1", "elsewhere", TaskStatus::Done, now + Duration::seconds(100));

    let output = fixture.run_canon(&["query", "--kind", "task", "--change", "add-audio-reactive", "--json"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));
    let payload = data_of(&support::stdout(&output));
    assert_eq!(payload["rollup"], serde_json::json!({"done": 2, "total": 6}));
}

#[test]
fn an_unfiltered_task_query_rolls_up_the_whole_result_set() {
    let fixture = support::Fixture::new(CHANGE_TASK_ROUTING, CHANGE_TASK_AGING);
    let now = Utc::now();
    for n in 1..=10 {
        let status = if n <= 4 { TaskStatus::Done } else { TaskStatus::Open };
        fixture.plant_task_in_git(&format!("add-widget#1.{n}"), "t", status, now + Duration::seconds(n));
    }

    let output = fixture.run_canon(&["query", "--kind", "task", "--json"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));
    let payload = data_of(&support::stdout(&output));
    assert_eq!(payload["rollup"], serde_json::json!({"done": 4, "total": 10}));
}

#[test]
fn task_output_sorts_by_change_id_then_task_number_not_merge_order() {
    let fixture = support::Fixture::new(CHANGE_TASK_ROUTING, CHANGE_TASK_AGING);
    let now = Utc::now();
    // Planted in raw `at`-merge order (earliest first): add-widget#2.1,
    // add-audio-reactive#1.2, add-widget#1.1, add-audio-reactive#1.1 —
    // the exact interleaved shape spec.md's own scenario names.
    fixture.plant_task_in_git("add-widget#2.1", "t", TaskStatus::Open, now);
    fixture.plant_task_in_git("add-audio-reactive#1.2", "t", TaskStatus::Open, now + Duration::seconds(1));
    fixture.plant_task_in_git("add-widget#1.1", "t", TaskStatus::Open, now + Duration::seconds(2));
    fixture.plant_task_in_git("add-audio-reactive#1.1", "t", TaskStatus::Open, now + Duration::seconds(3));

    let output = fixture.run_canon(&["query", "--kind", "task", "--json"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));
    let payload = data_of(&support::stdout(&output));
    let ids: Vec<String> = payload["records"].as_array().unwrap().iter().map(|r| r["task_id"].as_str().unwrap().to_string()).collect();
    assert_eq!(ids, vec!["add-audio-reactive#1.1", "add-audio-reactive#1.2", "add-widget#1.1", "add-widget#2.1"]);
}

// ── s21 `cross-tier-supersession` P4: PgTier-routed-kind reader fold ──
// The fold gates on KIND identity (design.md D5), not on which tier
// actually served the read, so a GIT-routed `task`/`handoff` fixture
// exercises the identical code path production's PG-routed `task`/
// `handoff` traffic hits — no live Postgres needed (assignment note).

const TASK_HANDOFF_ROUTING: &str = "  task: local\n  handoff: local\n";
const TASK_HANDOFF_AGING: &str = "";

/// design.md R3's row-count-parity mitigation: an unsuperseded `task`
/// corpus (one write per `task_id`) folds to the SAME row set — the
/// s21 P4 fold is a no-op when there is nothing to supersede.
#[test]
fn fold_is_a_noop_for_an_unsuperseded_task_corpus() {
    let fixture = support::Fixture::new(TASK_HANDOFF_ROUTING, TASK_HANDOFF_AGING);
    let now = Utc::now();
    fixture.plant_task_in_git("add-widget#1.1", "t1", TaskStatus::Open, now);
    fixture.plant_task_in_git("add-widget#1.2", "t2", TaskStatus::Done, now + Duration::seconds(1));
    fixture.plant_task_in_git("add-other#1.1", "t3", TaskStatus::Open, now + Duration::seconds(2));

    let output = fixture.run_canon(&["query", "--kind", "task", "--json"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));
    let payload = data_of(&support::stdout(&output));
    let ids: Vec<String> = payload["records"].as_array().unwrap().iter().map(|r| r["task_id"].as_str().unwrap().to_string()).collect();
    assert_eq!(ids.len(), 3, "one write per key must fold to exactly 3 rows, not fewer: {ids:?}");
    assert_eq!(ids, vec!["add-other#1.1", "add-widget#1.1", "add-widget#1.2"]);
}

/// design.md R3's row-count-parity mitigation, `handoff` kind.
#[test]
fn fold_is_a_noop_for_an_unsuperseded_handoff_corpus() {
    let fixture = support::Fixture::new(TASK_HANDOFF_ROUTING, TASK_HANDOFF_AGING);
    let now = Utc::now();
    fixture.plant_handoff_in_git("20260713-0900-first-topic-a1a1", "v1", now);
    fixture.plant_handoff_in_git("20260713-0901-second-topic-b2b2", "v1", now + Duration::seconds(1));

    let output = fixture.run_canon(&["query", "--kind", "handoff", "--json"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));
    let payload = data_of(&support::stdout(&output));
    let ids: Vec<String> = payload["records"].as_array().unwrap().iter().map(|r| r["id"].as_str().unwrap().to_string()).collect();
    assert_eq!(ids.len(), 2, "one write per key must fold to exactly 2 rows, not fewer: {ids:?}");
}

/// design.md R3's second mitigation: a superseded `Task` (two writes,
/// same `task_id`, one carrying a newer `at`) folds to exactly the
/// newer version — regardless of which write physically landed last.
#[test]
fn fold_resolves_to_the_newer_version_for_a_superseded_task() {
    let fixture = support::Fixture::new(TASK_HANDOFF_ROUTING, TASK_HANDOFF_AGING);
    let now = Utc::now();
    let newer_at = now + Duration::seconds(10);
    let older_at = now;
    // Out-of-order arrival: the chronologically NEWER version is
    // written FIRST, the OLDER one SECOND — proving the fold resolves
    // by content (`at`), never by physical write/arrival order.
    fixture.plant_task_in_git("add-widget#1.1", "t", TaskStatus::Done, newer_at);
    fixture.plant_task_in_git("add-widget#1.1", "t", TaskStatus::Open, older_at);

    let output = fixture.run_canon(&["query", "--kind", "task", "--json"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));
    let payload = data_of(&support::stdout(&output));
    let records = payload["records"].as_array().unwrap();
    assert_eq!(records.len(), 1, "two versions of the same task_id must fold to exactly one row: {records:?}");
    assert_eq!(records[0]["status"], "done", "the chronologically newer (done) version must win, not the physically-last-arrived (open) one");
}

/// design.md R3's second mitigation, `handoff` kind.
#[test]
fn fold_resolves_to_the_newer_version_for_a_superseded_handoff() {
    let fixture = support::Fixture::new(TASK_HANDOFF_ROUTING, TASK_HANDOFF_AGING);
    let now = Utc::now();
    let newer_at = now + Duration::seconds(10);
    let older_at = now;
    fixture.plant_handoff_in_git("20260713-0900-supersede-test-a1b2", "v2", newer_at);
    fixture.plant_handoff_in_git("20260713-0900-supersede-test-a1b2", "v1", older_at);

    let output = fixture.run_canon(&["query", "--kind", "handoff", "--json"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));
    let payload = data_of(&support::stdout(&output));
    let records = payload["records"].as_array().unwrap();
    assert_eq!(records.len(), 1, "two versions of the same handoff id must fold to exactly one row: {records:?}");
    assert_eq!(records[0]["body"]["fields"]["tag"], "v2", "the chronologically newer version must win, not the physically-last-arrived one");
}

/// s21 ReviewS21 important-finding guard: `fold_pg_routed_kind` (query.rs)
/// is KIND-gated (not routing-derived) so the R3 fold tests above can run
/// on git-routed fixtures with no live Postgres — the trade-off is that the
/// allowlist MUST stay in sync with the repo's real hot-rung/postgres
/// routing. If `canon.yaml` routes a new kind to a postgres-backed rung
/// (whose `PgTier::read` no longer pre-folds) without adding it to the
/// fold list, `canon query` for that kind silently regresses to
/// N-independent-versions. This fails loud first.
#[test]
fn fold_list_matches_pg_routing() {
    use canon_model::RecordKind;
    use canon_store::policy::{Backend, TierPolicy};
    use std::collections::BTreeSet;
    let repo_canon_yaml = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../canon.yaml");
    let yaml = std::fs::read_to_string(&repo_canon_yaml)
        .unwrap_or_else(|e| panic!("read {}: {e}", repo_canon_yaml.display()));
    let policy = TierPolicy::from_yaml(&yaml).expect("parse repo canon.yaml routing");
    let pg_routed: BTreeSet<&str> = RecordKind::ALL
        .iter()
        .filter(|k| {
            policy.tier_for(**k).ok().and_then(|rung| policy.tiers.get(&rung)).map(canon_store::policy::BackendConfig::backend) == Some(Backend::Postgres)
        })
        .map(|k| k.as_str())
        .collect();
    let fold_list: BTreeSet<&str> = ["task", "handoff", "session", "run", "event"].into_iter().collect();
    assert_eq!(
        pg_routed, fold_list,
        "fold_pg_routed_kind's KIND allowlist (query.rs) must equal canon.yaml's postgres-routed set; \
         a kind routed to a postgres-backed rung but missing from the fold list regresses `canon query` for it"
    );
}

/// s47: `canon inventory sync` re-materializes a `Scenario` index
/// record per scan, so a `.feature` edit followed by a re-sync leaves
/// BOTH generations committed. `canon_gate::spec_coverage` has folded
/// them since s44 — it must, or it judges one scenario twice and may
/// judge the STALE copy — while `canon query --kind scenario` did not,
/// and reported this repo's 24 scenarios as 38.
///
/// The assertion is that the two AGREE, not that a count is 1: a reader
/// deciding what to attest and the gate deciding what is unattested must
/// see the same corpus, or a green gate and a red-looking query describe
/// the same repo.
#[test]
fn a_resynced_scenario_reads_as_one_current_record() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("canon.yaml"), "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\n  scenario: local\n").unwrap();
    let ledger = dir.path().join(".canon/ledger");
    let tier = canon_store::git_tier::GitTier::new(&ledger);

    let scenario = |at: chrono::DateTime<chrono::Utc>, title: &str| {
        canon_model::Scenario::new(
            canon_model::Envelope::new(1, canon_model::RecordKind::Scenario, at, canon_model::Actor::new("canon", canon_model::RoleId::parse("implementer").unwrap())),
            canon_model::ProjectId::parse("canon").unwrap(),
            canon_model::ScenarioId::parse("finding.close.01").unwrap(),
            title,
            "",
            canon_model::SpecDigest::of(title.as_bytes()),
        )
    };
    let first = chrono::DateTime::parse_from_rfc3339("2026-08-05T10:00:00Z").unwrap().with_timezone(&chrono::Utc);
    canon_store::tier::Tier::write(&tier, &scenario(first, "the title before the edit")).unwrap();
    canon_store::tier::Tier::write(&tier, &scenario(first + chrono::Duration::hours(1), "the title after the edit")).unwrap();

    let raw = tier.read_history(canon_model::RecordKind::Scenario).unwrap();
    assert_eq!(raw.records.len(), 2, "a re-sync keeps both generations: the current one in the ledger, the prior one in the archive");
    assert_eq!(canon_store::tier::Tier::read(&tier, &canon_store::tier::TierQuery::kind(canon_model::RecordKind::Scenario)).unwrap().records.len(), 1);

    let outcome = canon_cli::query::run(dir.path(), canon_model::RecordKind::Scenario, None, None, None, None, false).expect("query the scenario corpus");
    assert_eq!(outcome.records.len(), 1, "`canon query --kind scenario` must fold to the CURRENT record, the way the gate already does");
    assert_eq!(
        outcome.records[0].0.get("title").and_then(|t| t.as_str()),
        Some("the title after the edit"),
        "and the survivor is the LATEST, never an arbitrary one"
    );
}
