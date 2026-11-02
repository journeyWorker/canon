//! The checker follows the contract it checks: `atip check` passes on `atip`.
//!
//! The fixture is `atip.fixture.json`: it claims 1.1, its `probe` is `check`, its `registry` and `warnings` are the checker's own
//! ([`atip_check::output::REGISTRY`] and [`atip_check::output::WARNINGS`]), and its `commands` are the refusals it can make, so the file
//! is generated from the code and a test fails when the two differ.
//! `ATIP_CHECK_BLESS_FIXTURE=1 cargo test -p atip-check --test self_check` rewrites it.

mod support;

use std::path::Path;

use serde_json::{json, Value};

use atip_check::output::{REGISTRY, WARNINGS};
use support::{atip_in, Harness, ATIP};

fn fixture_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("atip.fixture.json")
}

fn generated() -> Value {
    let rows: Vec<Value> = REGISTRY
        .iter()
        .map(|row| {
            let mut entry = json!({"code": row.code, "exit": row.exit, "retryable": row.retryable});
            if !row.reasons.is_empty() {
                entry["reasons"] = json!(row.reasons.iter().map(|reason| reason.reason).collect::<Vec<_>>());
            }
            entry
        })
        .collect();
    let warnings: Vec<&str> = WARNINGS.iter().map(|warning| warning.code).collect();
    json!({
        "contractVersion": atip_check::output::DECLARED_MINOR,
        "probe": "check",
        "globalOptions": [],
        "registry": rows,
        "warnings": warnings,
        "commands": {
            "check": {
                "refused": [["check", "--json"]],
                "missingValue": [["check", "--level", "L0", "--fixture", "atip.fixture.json", "--json"]]
            }
        }
    })
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
            .starts_with("L0 met: 18 MUST passed, 0 MUST failed, 0 MUST unprovided, 6 not applicable"),
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
    assert_eq!(results.len(), 29);
    // atip's own answers carry no `meta.next` and no page, offer no `schema`, and name no USAGE without a field; it claims 1.1 and has
    // no group: six checks do not apply and the other 22 pass.
    for (id, result) in &results {
        let expected = if ["L0.next", "L0.page", "L0.registry", "L0.contract", "L0.usage-hint", "L0.global-on-group"].contains(&id.as_str()) {
            "unsupported"
        } else {
            "pass"
        };
        assert_eq!(result, expected, "{id}");
    }
}
