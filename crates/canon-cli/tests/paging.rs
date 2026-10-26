//! Bounded listings, field projection and plan-then-confirm purge (0.20 D4,
//! D5), against the actually-built `canon` binary and an offline fixture.

mod support;

use chrono::{Duration, Utc};
use serde_json::Value;
use support::{stderr, stdout, Fixture};

const ROUTING: &str = "  trajectory: local\n  handoff: local\n";
const AGING: &str = "  trajectory: { after: 30d, to: cold }\n";

fn json(output: &std::process::Output) -> Value {
    serde_json::from_str(&stdout(output)).unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {}\nstderr: {}", stdout(output), stderr(output)))
}

fn error_code(output: &std::process::Output) -> String {
    json(output)["error"]["code"].as_str().unwrap_or_else(|| panic!("no error.code in {}", stdout(output))).to_string()
}

/// Five trajectories, oldest first, rewards 0.1 .. 0.5.
fn five_trajectories() -> Fixture {
    let fixture = Fixture::new(ROUTING, AGING);
    for i in 1..=5 {
        fixture.plant_trajectory_in_git(Utc::now() - Duration::days(10 - i), i as f64 / 10.0);
    }
    fixture
}

fn rewards(page: &Value) -> Vec<f64> {
    page["data"]["records"].as_array().unwrap().iter().map(|r| r["reward"].as_f64().unwrap()).collect()
}

#[test]
fn paging_with_next_cursor_yields_exactly_the_all_set_in_order() {
    let fixture = five_trajectories();
    let all = json(&fixture.run_canon(&["query", "--kind", "trajectory", "--json", "--all"]));
    assert_eq!(rewards(&all), vec![0.1, 0.2, 0.3, 0.4, 0.5]);
    assert_eq!(all["meta"]["hasMore"], false);
    assert!(all["meta"]["nextCursor"].is_null());

    let mut paged = Vec::new();
    let mut cursor: Option<String> = None;
    let mut pages = 0;
    loop {
        let mut args = vec!["query", "--kind", "trajectory", "--json", "--limit", "2"];
        if let Some(c) = &cursor {
            args.extend(["--cursor", c]);
        }
        let page = json(&fixture.run_canon(&args));
        pages += 1;
        assert_eq!(page["meta"]["returned"], page["data"]["records"].as_array().unwrap().len());
        assert_eq!(page["meta"]["total"], 5);
        paged.extend(rewards(&page));
        match page["meta"]["nextCursor"].as_str() {
            Some(next) => {
                assert_eq!(page["meta"]["hasMore"], true);
                cursor = Some(next.to_string());
            }
            None => {
                assert_eq!(page["meta"]["hasMore"], false);
                break;
            }
        }
    }
    assert_eq!(pages, 3);
    assert_eq!(paged, rewards(&all));
}

#[test]
fn the_default_page_is_bounded_and_human_mode_names_the_next_cursor() {
    let fixture = five_trajectories();
    let out = fixture.run_canon(&["query", "--kind", "trajectory", "--limit", "2"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    let last = text.lines().last().unwrap();
    assert!(last.contains("next cursor: "), "last line must name the cursor: {text}");
    let cursor = last.split("next cursor: ").nth(1).unwrap().split_whitespace().next().unwrap();

    let rest = fixture.run_canon(&["query", "--kind", "trajectory", "--all", "--cursor", cursor]);
    assert!(rest.status.success(), "{}", stderr(&rest));
    assert!(stdout(&rest).contains("3 record(s)"), "{}", stdout(&rest));
    assert!(!stdout(&rest).contains("next cursor"));
}

#[test]
fn a_cursor_from_another_listing_or_garbage_is_refused() {
    let fixture = five_trajectories();
    fixture.plant_handoff_in_git("20260713-0900-first-topic-a1a1", "a", Utc::now());
    fixture.plant_handoff_in_git("20260713-0901-second-topic-b2b2", "b", Utc::now() + Duration::seconds(1));
    let page = json(&fixture.run_canon(&["query", "--kind", "trajectory", "--json", "--limit", "2"]));
    let cursor = page["meta"]["nextCursor"].as_str().unwrap().to_string();

    let other_kind = fixture.run_canon(&["query", "--kind", "handoff", "--json", "--limit", "1", "--cursor", &cursor]);
    assert_eq!(other_kind.status.code(), Some(2));
    assert_eq!(error_code(&other_kind), "INVALID_CURSOR");

    let garbage = fixture.run_canon(&["query", "--kind", "trajectory", "--json", "--cursor", "not-a-cursor"]);
    assert_eq!(garbage.status.code(), Some(2));
    assert_eq!(error_code(&garbage), "INVALID_CURSOR");
}

#[test]
fn fields_projects_records_and_an_unknown_field_is_refused() {
    let fixture = five_trajectories();
    let out = fixture.run_canon(&["query", "--kind", "trajectory", "--json", "--all", "--fields", "reward,at"]);
    assert!(out.status.success(), "{}", stderr(&out));
    for record in json(&out)["data"]["records"].as_array().unwrap() {
        let keys: Vec<&str> = record.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys, vec!["at", "reward"], "only the requested fields");
    }

    let bad = fixture.run_canon(&["query", "--kind", "trajectory", "--json", "--fields", "reward,nope"]);
    assert_eq!(bad.status.code(), Some(2));
    assert_eq!(error_code(&bad), "UNKNOWN_FIELD");
    assert!(json(&bad)["error"]["message"].as_str().unwrap().contains("nope"));
}

#[test]
fn fields_match_in_either_case_style_and_an_unknown_one_gets_suggestions() {
    let fixture = five_trajectories();
    let plain = fixture.run_canon(&["query", "--kind", "trajectory", "--json", "--all", "--fields", "reward,at"]);
    let mixed = fixture.run_canon(&["query", "--kind", "trajectory", "--json", "--all", "--fields", "Reward,AT"]);
    assert!(mixed.status.success(), "{}", stderr(&mixed));
    assert_eq!(json(&mixed)["data"]["records"], json(&plain)["data"]["records"], "the case style of a field name changes nothing");

    // A snake_case record field is also reachable by its camelCase spelling, and the output key stays as stored.
    let snake = fixture.run_canon(&["query", "--kind", "trajectory", "--json", "--limit", "1"]);
    let record = json(&snake)["data"]["records"][0].clone();
    let key = record.as_object().unwrap().keys().find(|k| k.contains('_')).expect("a trajectory record has a snake_case field").clone();
    let camel: String = key.split('_').enumerate().map(|(i, w)| if i == 0 { w.to_string() } else { w[..1].to_uppercase() + &w[1..] }).collect();
    let by_camel = fixture.run_canon(&["query", "--kind", "trajectory", "--json", "--limit", "1", "--fields", &camel]);
    assert!(by_camel.status.success(), "{}", stderr(&by_camel));
    let projected = json(&by_camel)["data"]["records"][0].clone();
    assert_eq!(projected.as_object().unwrap().keys().collect::<Vec<_>>(), vec![&key], "{projected}");

    let bad = fixture.run_canon(&["query", "--kind", "trajectory", "--json", "--fields", "rewrd"]);
    assert_eq!(bad.status.code(), Some(2));
    let body = json(&bad);
    assert_eq!(body["error"]["code"], "UNKNOWN_FIELD");
    assert!(body["error"]["suggestions"].as_array().unwrap().iter().any(|s| s == "reward"), "{body}");
}

fn purge_fixture() -> Fixture {
    let fixture = Fixture::new(ROUTING, AGING);
    std::fs::create_dir_all(fixture.root.path().join(".canon/learn")).unwrap();
    fixture.plant_trajectory_in_git(Utc::now() - Duration::days(5), 0.1);
    fixture.plant_trajectory_in_git(Utc::now() - Duration::days(4), 0.2);
    fixture
}

fn purge_args<'a>(before: &'a str, extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec!["purge", "--kind", "trajectory", "--before", before, "--json"];
    args.extend(extra);
    args
}

#[test]
fn purge_preview_then_commit_deletes_and_a_stale_digest_deletes_nothing() {
    let fixture = purge_fixture();
    let before = (Utc::now() - Duration::days(1)).to_rfc3339();

    let preview = fixture.run_canon(&purge_args(&before, &[]));
    assert!(preview.status.success(), "{}", stderr(&preview));
    let preview_body = json(&preview);
    let digest = preview_body["data"]["planDigest"].as_str().expect("a preview carries a planDigest").to_string();
    assert_eq!(preview_body["data"]["committed"], false, "{preview_body}");
    assert_eq!(preview_body["data"]["effects"].as_array().unwrap().len(), 2, "one effect per record the plan removes: {preview_body}");
    // The one step after a preview is the exact commit, digest filled in.
    assert_eq!(preview_body["meta"]["command"], "purge", "{preview_body}");
    let repo = fixture.root.path().display().to_string();
    let expected = ["canon", "purge", "--kind", "trajectory", "--before", before.as_str(), "--repo", repo.as_str(), "--commit", "--plan-digest", digest.as_str(), "--json"];
    assert_eq!(preview_body["meta"]["next"][0]["argv"], serde_json::json!(expected), "{preview_body}");
    assert_eq!(fixture.git_file_count(), 2, "a preview deletes nothing");
    assert!(!fixture.root.path().join(canon_model::paths::ACCESS_AUDIT_FILE).exists(), "a preview writes no audit line");

    // A record the plan did not list lands between the preview and the commit.
    fixture.plant_trajectory_in_git(Utc::now() - Duration::days(3), 0.3);
    let stale = fixture.run_canon(&purge_args(&before, &["--commit", "--plan-digest", &digest]));
    assert_eq!(stale.status.code(), Some(2));
    assert_eq!(error_code(&stale), "PLAN_CHANGED");
    assert_eq!(fixture.git_file_count(), 3, "a stale digest deletes nothing");

    let replan = fixture.run_canon(&purge_args(&before, &[]));
    let fresh = json(&replan)["data"]["planDigest"].as_str().unwrap().to_string();
    assert_ne!(fresh, digest);
    let committed = fixture.run_canon(&purge_args(&before, &["--commit", "--plan-digest", &fresh]));
    assert!(committed.status.success(), "{}", stderr(&committed));
    assert_eq!(fixture.git_file_count(), 0, "the committed plan removed what it listed");
    let committed_body = json(&committed);
    assert_eq!(committed_body["data"]["planDigest"], fresh.as_str(), "{committed_body}");
    assert_eq!(committed_body["data"]["committed"], true, "{committed_body}");
    assert!(committed_body["meta"]["next"].is_array(), "a committed purge is a write: {committed_body}");
}

#[test]
fn the_old_purge_spellings_are_usage_errors_that_name_the_new_form() {
    let fixture = purge_fixture();
    let before = (Utc::now() - Duration::days(1)).to_rfc3339();
    for old in [&["--plan"][..], &["--confirm", "abc"], &["--dry-run"]] {
        let out = fixture.run_canon(&purge_args(&before, old));
        assert_eq!(out.status.code(), Some(2), "{old:?}");
        assert_eq!(error_code(&out), "USAGE", "{old:?}");
        assert!(json(&out)["error"]["hint"].as_str().unwrap().contains("--commit --plan-digest"), "{old:?}: {}", json(&out));
    }
    // --commit and --plan-digest come together.
    for half in [&["--commit"][..], &["--plan-digest", "abc"]] {
        assert_eq!(fixture.run_canon(&purge_args(&before, half)).status.code(), Some(2), "{half:?}");
    }
    assert_eq!(fixture.git_file_count(), 2);
}

fn first_file_under(dir: &std::path::Path) -> std::path::PathBuf {
    if let Some(entry) = std::fs::read_dir(dir).unwrap().next() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            return first_file_under(&path);
        }
        return path;
    }
    panic!("no file under {}", dir.display());
}

#[test]
fn replacing_a_matching_record_with_another_at_equal_count_is_plan_changed() {
    let fixture = purge_fixture();
    let before = (Utc::now() - Duration::days(1)).to_rfc3339();
    let plan = fixture.run_canon(&purge_args(&before, &[]));
    assert!(plan.status.success(), "{}", stderr(&plan));
    let digest = json(&plan)["data"]["planDigest"].as_str().unwrap().to_string();
    assert_eq!(fixture.git_file_count(), 2);

    // Same count, different removal set: one planned record goes, another matching one arrives.
    std::fs::remove_file(first_file_under(&fixture.git_root())).unwrap();
    fixture.plant_trajectory_in_git(Utc::now() - Duration::days(3), 0.9);
    assert_eq!(fixture.git_file_count(), 2);

    let out = fixture.run_canon(&purge_args(&before, &["--commit", "--plan-digest", &digest]));
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(error_code(&out), "PLAN_CHANGED");
    assert_eq!(fixture.git_file_count(), 2, "nothing was deleted");
}
