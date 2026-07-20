//! Acceptance: "a session spanning two workspaces yields distinct
//! rows, not one row with an arbitrary label" — the P2 regression
//! guard for `crates/canon-store/sql/views.sql`'s `mart_session_costs`,
//! which is labeled "session costs by role/repo/session" but used to
//! GROUP BY `session_id`/`client`/`role` only, picking the `repo`
//! proxy (`workspace_label`) via `any_value()` — silently merging two
//! workspaces' costs into one row under whichever workspace happened
//! to be picked.
//!
//! Deliberately builds a standalone two-workspace corpus (a session
//! with two runs, each carrying one `token_usage` event under a
//! DIFFERENT `workspace_label`) rather than reusing `fixtures/
//! corpus.rs` — that shared fixture's `session_costs` module is a
//! fixed single-workspace scenario another test already asserts
//! exactly, and this crate's own precedent (module doc, `crate::marts`)
//! is one fixture per documented scenario, never overloading one
//! fixture with two purposes.

mod support;

use canon_model::envelope::{Actor, Envelope, RecordKind};
use canon_model::ids::{RoleId, RunId, SessionId};
use canon_model::records::{Event, Run, RunStatus, Session};
use canon_report::marts;
use canon_report::roots::Roots;
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;
use chrono::{DateTime, TimeZone, Utc};
use serde_json::json;

fn at(y: i32, m: u32, d: u32, h: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, m, d, h, 0, 0).single().unwrap()
}

#[test]
fn session_costs_yields_distinct_rows_when_one_session_spans_two_workspaces() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let git_root = dir.path().join("ledger");
    let tier = GitTier::new(&git_root);

    let session_id = SessionId::parse("multi-workspace-session").unwrap();
    tier.write(&Session::new(
        Envelope::new(1, RecordKind::Session, at(2026, 2, 1, 9), Actor::new("fixture-session-actor", RoleId::parse("dev").unwrap())),
        session_id.clone(),
        "claude-code",
        at(2026, 2, 1, 9),
        Some(at(2026, 2, 1, 12)),
    ))
    .unwrap();

    // Two runs under the SAME session — one per workspace.
    let run_a = RunId::new();
    tier.write(&Run::new(
        Envelope::new(1, RecordKind::Run, at(2026, 2, 1, 10), Actor::new_unattributed("claude-code")),
        run_a,
        Some(session_id.clone()),
        None,
        RunStatus::Succeeded,
        at(2026, 2, 1, 9),
        Some(at(2026, 2, 1, 10)),
    ))
    .unwrap();
    let run_b = RunId::new();
    tier.write(&Run::new(
        Envelope::new(1, RecordKind::Run, at(2026, 2, 1, 11), Actor::new_unattributed("claude-code")),
        run_b,
        Some(session_id.clone()),
        None,
        RunStatus::Succeeded,
        at(2026, 2, 1, 10),
        Some(at(2026, 2, 1, 11)),
    ))
    .unwrap();

    tier.write(&Event::new(
        Envelope::new(1, RecordKind::Event, at(2026, 2, 1, 10), Actor::new_unattributed("claude-code")),
        run_a,
        1,
        "token_usage",
        json!({
            "provider_id": "anthropic",
            "workspace_key": "acme",
            "workspace_label": "acme",
            "tokens": {"input": 10, "output": 5, "cache_read": 0, "cache_write": 0, "reasoning": 0, "total": 15},
            "cost": 0.01,
            "cost_source": "api",
        }),
    ))
    .unwrap();
    tier.write(&Event::new(
        Envelope::new(1, RecordKind::Event, at(2026, 2, 1, 11), Actor::new_unattributed("claude-code")),
        run_b,
        1,
        "token_usage",
        json!({
            "provider_id": "anthropic",
            "workspace_key": "canon-wt",
            "workspace_label": "canon-wt",
            "tokens": {"input": 20, "output": 10, "cache_read": 0, "cache_write": 0, "reasoning": 0, "total": 30},
            "cost": 0.02,
            "cost_source": "api",
        }),
    ))
    .unwrap();

    let roots = Roots::new(git_root, dir.path().join("r2"), dir.path().join("learn"));
    let result = marts::fetch_session_costs(&roots).unwrap();

    assert_eq!(result.rows.len(), 2, "one session spanning two workspaces must yield two distinct rows, got {:?}", result.rows);

    let labels: std::collections::BTreeSet<&str> = result.rows.iter().map(|r| r["workspace_label"].as_str().unwrap()).collect();
    assert_eq!(labels, std::collections::BTreeSet::from(["acme", "canon-wt"]), "each workspace must keep its own honest label, never merged");

    for row in &result.rows {
        assert_eq!(row["session_id"], "multi-workspace-session");
        assert_eq!(row["client"], "claude-code");
        assert_eq!(row["role"], "dev");
        assert_eq!(row["run_count"], 1, "each workspace's row must count only its own run, not the other workspace's");
    }

    let acme_row = result.rows.iter().find(|r| r["workspace_label"] == "acme").unwrap();
    assert!((acme_row["total_cost"].as_f64().unwrap() - 0.01).abs() < 1e-9);
    assert_eq!(acme_row["total_tokens"], 15);

    let canon_row = result.rows.iter().find(|r| r["workspace_label"] == "canon-wt").unwrap();
    assert!((canon_row["total_cost"].as_f64().unwrap() - 0.02).abs() < 1e-9);
    assert_eq!(canon_row["total_tokens"], 30);
}
