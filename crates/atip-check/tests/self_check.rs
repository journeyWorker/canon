//! The checker follows the contract it checks: `atip check` passes on `atip`.
//!
//! The fixture is `atip.fixture.json`: its `probe` is `check` and its `registry` is the checker's own
//! ([`atip_check::output::REGISTRY`]), so the file is generated from the code and a test fails when the two differ.
//! `ATIP_CHECK_BLESS_FIXTURE=1 cargo test -p atip-check --test self_check` rewrites it.

mod support;

use std::path::Path;

use serde_json::{json, Value};

use atip_check::output::REGISTRY;
use support::{atip_in, Harness, ATIP};

fn fixture_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("atip.fixture.json")
}

fn generated() -> Value {
    let rows: Vec<Value> = REGISTRY.iter().map(|row| json!({"code": row.code, "exit": row.exit, "retryable": row.retryable, "meaning": row.meaning, "fix": row.fix})).collect();
    json!({"probe": "check", "registry": rows})
}

#[test]
fn atip_check_37_the_committed_fixture_of_atip_is_generated_from_its_registry() {
    let text = serde_json::to_string_pretty(&generated()).expect("json") + "\n";
    if std::env::var_os("ATIP_CHECK_BLESS_FIXTURE").is_some() {
        std::fs::write(fixture_path(), &text).expect("write the fixture");
    }
    let committed = std::fs::read_to_string(fixture_path()).expect("atip.fixture.json");
    assert_eq!(committed, text, "atip.fixture.json is out of date; run ATIP_CHECK_BLESS_FIXTURE=1 cargo test -p atip-check --test self_check and commit");
}

#[test]
fn atip_check_37_atip_check_passes_on_atip_itself() {
    let harness = Harness::new();
    let (cwd, tmp) = (harness.path("cwd"), harness.path("tmp"));
    std::fs::create_dir_all(&cwd).expect("cwd");
    std::fs::create_dir_all(&tmp).expect("tmp");
    let fixture = fixture_path();
    let args = [
        "check",
        "--bin",
        ATIP,
        "--level",
        "L0",
        "--fixture",
        fixture.to_str().expect("utf8"),
    ];
    let text = atip_in(&cwd, &tmp, &args);
    assert_eq!(text.code, Some(0), "{}{}", text.stdout, text.stderr);
    assert!(
        text.stdout
            .lines()
            .last()
            .expect("summary")
            .starts_with("L0 met: 13 MUST passed, 0 MUST failed, 0 MUST unprovided, 2 not applicable"),
        "{}",
        text.stdout
    );
    let mut json_args = args.to_vec();
    json_args.push("--json");
    let output = atip_in(&cwd, &tmp, &json_args);
    assert_eq!(output.code, Some(0));
    let document = output.envelope();
    assert_eq!(document["data"]["passed"], true);
    let results = output.results();
    assert_eq!(results.len(), 19);
    // atip's own answers carry no `meta.next` and no `meta.page`, so those two checks do not apply; the other 17 pass.
    for (id, result) in &results {
        let expected = if id == "L0.next" || id == "L0.page" {
            "unsupported"
        } else {
            "pass"
        };
        assert_eq!(result, expected, "{id}");
    }
}
