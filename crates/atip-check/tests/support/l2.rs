//! The L2 side of the fake tool's test support: the conforming L2 configuration and its fixture, and the seeded violations of
//! the L2 checks (approaches/checker.md 6, 7.2), one configuration per pass condition.
//!
//! A case is `condition -> (configuration, fixture change)`, `condition` being `<check>/<condition>` of an `L2.<check>`. The
//! fake of an L2 run is the L1 fake with `"l2": true`: `--input` on every write command, a batch for `member tag`, and the
//! `record` group (see `fake-tool/l1/input.rs`); it claims 1.2 and has the L1 registry.

#![allow(dead_code, reason = "each integration test uses a part of this module")]

use serde_json::{json, Map, Value};

use super::{Case, Harness, Output};

/// The L0 checks of a run and the L1 ones that apply to a 1.2 claim.
pub const L0_CHECKS: usize = 28;

/// The configuration of the conforming L2 fake.
pub fn l2_standard_config() -> Value {
    json!({
        "claims": "1.2",
        "l1Registry": true,
        "l2": true,
        "binAlias": "atip-fake-tool-claims12-l2",
        "extraCodes": [
            {"code": "NOTE_CONFLICT", "exit": 2},
            {"code": "PLAN_STALE", "exit": 2},
            {"code": "OPERATION_REUSED", "exit": 2},
            {"code": "ALREADY_SEALED", "exit": 2},
            {"code": "DEPLOYMENT_NOT_FOUND", "exit": 2},
            {"code": "DEPLOY_FAILED", "exit": 1},
            {"code": "PURGE_FAILED", "exit": 1},
            {"code": "BUILD_FAILED", "exit": 2},
            {"code": "PAIR_REQUIRED", "exit": 2},
            {"code": "WAIT_TIMEOUT", "exit": 1}
        ],
        "retryableCodes": ["WAIT_TIMEOUT"]
    })
}

/// The standard configuration with the members of `extra` set.
pub fn l2_config(extra: &Value) -> Value {
    let mut config = l2_standard_config();
    if let (Some(config), Some(extra)) = (config.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            config.insert(key.clone(), value.clone());
        }
    }
    config
}

/// What the fixture says about the commands of the L2 fake (approaches/checker.md 5).
pub fn l2_commands() -> Value {
    json!({
        "items": {"ok": [["items", "--json"]]},
        "status": {"ok": [["status", "--json"]]},
        "init": {"ok": [["init", "--json"]]},
        "team add": {"ok": [["team", "add", "--json"]]},
        "reject": {"refused": [["reject", "--json"]]},
        "schema": {"ok": [["schema", "--errors", "--json"]]},
        "member list": {
            "ok": [["member", "list", "--team", "core", "--limit", "2", "--sort", "name", "--json"]],
            "paged": [["member", "list", "--team", "core", "--limit", "1", "--sort", "name", "--json"]],
            "fields": [["profile.name"], ["owners[].email", "tags"], ["name", "nickname"], ["attributes.region"], ["attributes[\"http.status-code\"]", "profile.email"]]
        },
        "member search": {
            "ok": [[
                "member", "search", "cy", "--tag", "lead", "--filter", "{\"team\":\"core\"}",
                "--min-score", "10", "--age", "30", "--step", "10", "--since", "2026-02-01",
                "--sort", "name", "--exact", "--verbose", "--json"
            ]],
            "domainRejects": {
                "tag": ["Core", "-core", "core tag"],
                "filter": ["[1]", "{\"team\":1}", "{\"bogus\":true}"],
                "since": ["2026-2-30", "20260230", "2026-02-30x"]
            }
        },
        "member tag": {
            "ok": [["member", "tag", "3", "--tag", "lead", "--tag", "core", "--note", "promoted", "--json"], ["member", "tag", "1", "--tag", "core", "--json"]],
            "refused": [["member", "tag", "99", "--tag", "core", "--json"]],
            "domainRejects": {"tag": ["Core", "-core", "core tag"]},
            "batchFailsAt": [{"input": [{"operands": [3], "tag": ["lead"]}, {"operands": [99], "tag": ["core"]}], "index": 1}],
            "batchFailsFirst": [{"input": [{"operands": [99], "tag": ["core"]}, {"operands": [3], "tag": ["lead"]}], "index": 0}]
        },
        "purge": {"preview": [["purge", "--kind", "session", "--before", "2030-01-01", "--json"]]},
        "deploy start": {
            "ok": [["deploy", "start", "staging", "--json"]],
            "domainRejects": {"environment": ["Staging", "-staging"], "operation": ["-x", "A b"]},
            "async": {"start": ["deploy", "start", "staging", "--json"], "worker": "FAKE_WORKER", "outcomes": ["done", "failed", "timeout"]}
        },
        "build": {"ok": [["build", "--json"], ["build", "--quiet", "--json"]], "refused": [["build", "--target", "broken", "--json"]]},
        "seal": {"ok": [["seal", "--json"]]},
        "stamp": {"ok": [["stamp", "hello", "--json"]]},
        "log": {"ok": [["log", "hello", "--json"]]},
        "peek": {"ok": [["peek", "--json"]], "allowedEffects": [".peek-access.log"]},
        "record put": {
            "ok": [["record", "put", "7", "--round", "1", "--label", "alpha", "--weight", "1", "--meta", "{\"a\":1,\"b\":2}", "--json"]],
            "missingValue": [["record", "put", "7", "--json"]]
        }
    })
}

impl Harness {
    /// The fixture of the L2 fake configured by `config`: the L1 fixture with the commands of the L2 fake.
    pub fn l2_fixture(&self, config: &Value) -> Value {
        let mut fixture = self.l1_fixture(config);
        fixture["commands"] = l2_commands();
        fixture["search"] = json!([{"query": "tag a member", "expect": "member tag"}, {"query": "remove old logs", "expect": "purge"}]);
        fixture["contractVersion"] = json!("1.2");
        fixture
    }

    /// `atip check --bin <the fake tool of config> --level L2 --fixture <fixture> --json`, with the given arguments after.
    pub fn check_l2_with(&self, config: &Value, fixture: &Value, extra: &[&str]) -> Output {
        let fixture_path = self.write("fixture.json", fixture);
        let bin = super::l1::l1_bin(self, config);
        let work = self.path("cwd");
        std::fs::create_dir_all(&work).expect("cwd");
        let tmp = self.path("tmp");
        std::fs::create_dir_all(&tmp).expect("tmp");
        let mut args = vec!["check", "--bin", bin.to_str().expect("utf8"), "--level", "L2", "--fixture", fixture_path.to_str().expect("utf8"), "--json"];
        args.extend(extra);
        super::atip_in(&work, &tmp, &args)
    }

    pub fn check_l2(&self, config: &Value, fixture: &Value) -> Output {
        self.check_l2_with(config, fixture, &[])
    }

    /// The same at `level` (`L0`, `L1` or `L2`).
    pub fn check_l2_at(&self, config: &Value, fixture: &Value, level: &str) -> Output {
        let fixture_path = self.write("fixture.json", fixture);
        let bin = super::l1::l1_bin(self, config);
        let work = self.path("cwd");
        std::fs::create_dir_all(&work).expect("cwd");
        let tmp = self.path("tmp");
        std::fs::create_dir_all(&tmp).expect("tmp");
        let args = ["check", "--bin", bin.to_str().expect("utf8"), "--level", level, "--fixture", fixture_path.to_str().expect("utf8"), "--json"];
        super::atip_in(&work, &tmp, &args)
    }
}

// ---- the operations of a `docPatch` ----

pub fn set(path: &str, value: Value) -> Value {
    json!({"op": "set", "path": path, "value": value})
}

pub fn add(path: &str, value: Value) -> Value {
    json!({"op": "add", "path": path, "value": value})
}

pub fn remove(path: &str) -> Value {
    json!({"op": "remove", "path": path})
}

/// `{"docPatch": {key: ops}}`.
pub fn patch(entries: &[(&str, Vec<Value>)]) -> Value {
    let mut patches = Map::new();
    for (key, ops) in entries {
        patches.insert((*key).to_string(), Value::Array(ops.clone()));
    }
    json!({"docPatch": patches})
}

/// A case: the condition `<check>/<condition>` and the configuration (the standard one with `extra`) that breaks it.
pub fn case(condition: &'static str, extra: Value) -> Case {
    Case { condition, config: l2_config(&extra), tweak: None }
}

/// The owner of a condition `<check>/<condition>`: `L2.<check>`, but for the numbers of a document, which `L1.numbers` owns.
pub fn owner_of(condition: &str) -> String {
    let check = condition.split('/').next().unwrap_or_default();
    if check == "numbers" {
        "L1.numbers".to_string()
    } else {
        format!("L2.{check}")
    }
}

impl Case {
    /// The same case with a change to the fixture.
    pub fn with_tweak(mut self, tweak: fn(&mut Value)) -> Self {
        self.tweak = Some(tweak);
        self
    }
}

/// Every pass condition of the `--input` and batch rows, each with the configuration that breaks it and no other.
pub fn input_cases() -> Vec<Case> {
    let record = |ops: Vec<Value>| patch(&[("record put", ops)]);
    vec![
        // input-present and input-example judge the documents alone.
        case("input-present/every-write-has-input", patch(&[("init", vec![remove("/input"), remove("/options/0")])])),
        case("input-example/example-fragment", patch(&[("member tag", vec![set("/input/example", json!({"operands": [3], "tag": "lead"}))])])),
        // A value outside its domain that the fragment and the arguments schema both admit.
        case(
            "input-example/example-values",
            record(vec![set("/input/example", json!({"operands": [7], "round": 100})), set("/schemas/RecordPutArguments/properties/round", json!({"type": "integer"}))]),
        ),
        case(
            "input-example/example-required",
            record(vec![set("/input/example", json!({"operands": [7]})), remove("/schemas/RecordPutArguments/required")]),
        ),
        case("input-example/example-constraints", record(vec![set("/input/example", json!({"operands": [7], "round": 1, "note": "a", "clearNote": true}))])),
        case(
            "input-example/example-arguments-ref",
            record(vec![set("/input/example", json!({"operands": [7], "round": 1, "score": 0})), set("/schemas/RecordPutArguments/properties/score", json!({"type": "integer", "minimum": 1}))]),
        ),
        // The framing.
        case("input-refusals/refused", json!({"inputAccepts": ["unknown-key"]})),
        case("input-refusals/field", json!({"inputField": "ARGUMENT"})),
        case("input-refusals/details", json!({"inputNoDetails": true})),
        case("input-tty/refused", json!({"inputTtyCode": "USAGE"})),
        case("input-tty/field", json!({"inputTtyField": "stdin"})),
        case("input-usage/usage", json!({"inputUsageAccepted": true})),
        case("input-usage/usage", json!({"inputTwiceInvalid": true, "inputLastWordInvalid": true})),
        case("input-usage/field", json!({"inputUsageField": "--inputs"})),
        case("input-reasons/reserved-reason", json!({"inputNoReason": true})),
        // The merge.
        case("input-fragment/same-as-argv", json!({"inputRequiresArgv": true})),
        case("input-normalise/numeric-string-same-as-number", json!({"inputNoNumericStrings": true})),
        case("input-conflict/different-value-conflicts", json!({"inputNoConflict": true})),
        case("input-conflict/conflict-pointer", json!({"inputConflictField": "--round"})),
        case("input-conflict/same-value-succeeds", json!({"conflictJsonObjects": true})),
        case("input-env/env-applies-after-the-merge", json!({"envBeforeMerge": true})),
        case("input-pointer/constraint-code", json!({"inputConstraintCode": "USAGE"})),
        case("input-pointer/document-pointer", json!({"inputFieldsAsArgv": true})),
        case("input-pointer/required-code", json!({"inputRequiredInvalid": true})),
        case("input-pointer/required-pointer", json!({"inputRequiredFieldAsArgv": true})),
        case("input-dash-values/value-is-a-value", json!({"dashValuesAsOptions": true})),
        case("numbers/beyond-safe-range-refused-in-a-document", json!({"inputRoundsBigIntegers": true})),
        // The batches.
        case("batch-preflight/item-code", json!({"batchLatePreflight": true})),
        case("batch-preflight/item-pointer", json!({"batchNoPrefix": true})),
        case("batch-incomplete/stopped", json!({"batchFirstFailPlain": true})),
        case("batch-incomplete/failed-index", json!({"batchWrongField": true})),
        case("batch-incomplete/report-items", json!({"batchNoData": true})),
        case("batch-incomplete/item-count", json!({"batchItemsShort": true})),
        case("batch-incomplete/item-statuses", json!({"batchStatusWrong": true})),
        case("batch-incomplete/failed-error-registered", json!({"batchRetryableWrong": true})),
        case("batch-incomplete/batch-report-shape", json!({"batchReportBadShape": true})),
        case("batch-incomplete/batch-output", json!({"batchNoOutputOnFailure": true})),
        case("batch-ok/succeeded", json!({"batchFailsOnSuccess": true})),
        case("batch-ok/batch-report-shape", json!({"batchOkReportBadShape": true})),
        case("batch-ok/item-count", json!({"batchOkItemsShort": true})),
        case("batch-ok/every-status-succeeded", json!({"batchOkStatusOdd": true})),
        case("batch-ok/batch-output", json!({"batchNoOutputOnSuccess": true})),
    ]
}


// ---- the shared runners of the L2 test files ----

use std::collections::BTreeSet;

use atip_check::app::check_binary_mutated;
use atip_check::cli::{CheckArgs, Level};

/// `atip check --level <level> --json` of the fake tool of `case`, with its fixture (the commands of the L2 fake).
pub fn run_case(case: &Case, level: &str) -> Output {
    let harness = Harness::new();
    let mut fixture = harness.l2_fixture(&case.config);
    if let Some(tweak) = case.tweak {
        tweak(&mut fixture);
    }
    harness.check_l2_at(&case.config, &fixture, level)
}

pub fn run_config(config: &Value) -> Output {
    let harness = Harness::new();
    let fixture = harness.l2_fixture(config);
    harness.check_l2(config, &fixture)
}

pub fn check_of<'a>(data: &'a Value, id: &str) -> &'a Value {
    data["checks"]
        .as_array()
        .and_then(|checks| checks.iter().find(|check| check["id"] == id))
        .unwrap_or_else(|| panic!("{id} is not in data.checks"))
}

/// The checks that are neither `pass` nor `unsupported`, each with its reason.
pub fn not_passing(data: &Value) -> String {
    data["checks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|check| !matches!(check["result"].as_str(), Some("pass" | "unsupported")))
        .map(|check| format!("  {} {}: {}", check["id"], check["result"], check["reason"].as_str().unwrap_or_default().chars().take(400).collect::<String>()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The owner is `fail` with a reason and rule numbers, and every other check passes or does not apply.
pub fn verdict(data: &Value, owner: &str) -> Result<(), String> {
    let checks = data["checks"].as_array().ok_or("data.checks is missing")?;
    for check in checks {
        let id = check["id"].as_str().unwrap_or_default();
        let result = check["result"].as_str().unwrap_or_default();
        if id == owner {
            if result != "fail" {
                return Err(format!("{id} is {result}, expected fail"));
            }
            if check["reason"].as_str().is_none_or(str::is_empty) || check["rules"].as_array().is_none_or(Vec::is_empty) {
                return Err(format!("{id} fails without a reason and rule numbers"));
            }
        } else if !matches!(result, "pass" | "unsupported") {
            return Err(format!("{id} is {result}, but only {owner} was seeded"));
        }
    }
    Ok(())
}

/// Runs `jobs` on at most six threads and returns their results in order.
pub fn in_parallel<T: Send>(jobs: Vec<Box<dyn FnOnce() -> T + Send>>) -> Vec<T> {
    let queue = std::sync::Mutex::new(jobs.into_iter().enumerate().collect::<Vec<_>>());
    let results = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..6 {
            scope.spawn(|| loop {
                let next = queue.lock().expect("queue").pop();
                let Some((index, job)) = next else { break };
                let value = job();
                results.lock().expect("results").push((index, value));
            });
        }
    });
    let mut results = results.into_inner().expect("results");
    results.sort_by_key(|(index, _)| *index);
    results.into_iter().map(|(_, value)| value).collect()
}

/// Every case of `cases` whose check is one of `names`: `atip check --level L2` fails that check and no other, with the exit its strength says.
pub fn proves(cases: Vec<Case>, names: &[&str]) {
    let only = std::env::var("ONLY").unwrap_or_default();
    let cases: Vec<Case> = cases
        .into_iter()
        .filter(|case| names.contains(&case.condition.split('/').next().unwrap_or_default()) && case.condition.contains(only.as_str()))
        .collect();
    for name in names.iter().filter(|_| only.is_empty()) {
        assert!(cases.iter().any(|case| case.condition.split('/').next() == Some(*name)), "{name} has a seeded violation for each of its conditions");
    }
    let jobs: Vec<Box<dyn FnOnce() -> Result<(), String> + Send>> = cases
        .into_iter()
        .map(|case| -> Box<dyn FnOnce() -> Result<(), String> + Send> {
            Box::new(move || {
                let owner = owner_of(case.condition);
                let output = run_case(&case, "L2");
                let document = output.envelope();
                verdict(&document["data"], &owner).map_err(|why| format!("{}: {why}\n{}", case.condition, not_passing(&document["data"])))?;
                super::kind_verdict(&document["data"], &owner, case.condition.split('/').nth(1).unwrap_or_default()).map_err(|why| format!("{}: {why}", case.condition))?;
                let must = check_of(&document["data"], &owner)["strength"] == "MUST";
                let expected = if must { (Some(1), Some(false)) } else { (Some(0), Some(true)) };
                let got = (output.code, document["ok"].as_bool());
                if got != expected {
                    return Err(format!("{}: exit {got:?}, expected {expected:?}", case.condition));
                }
                if must && document["error"]["code"] != "LEVEL_NOT_MET" {
                    return Err(format!("{}: a MUST failure is LEVEL_NOT_MET", case.condition));
                }
                if must && !document["error"]["message"].as_str().unwrap_or_default().contains(&owner) {
                    return Err(format!("{}: the error names the failing check", case.condition));
                }
                Ok(())
            })
        })
        .collect();
    let failures: Vec<String> = in_parallel(jobs).into_iter().filter_map(Result::err).collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A conforming L2 run judges every condition of `cases`' checks (but the exempt ones), and each case alone: with its condition switched off its owner stops
/// failing, and for the first case of each check the owner forced to pass stops failing too.
pub fn covers(cases: Vec<Case>, checks: &[&str], exempt: &[&str]) {
    let harness = Harness::new();
    let config = l2_standard_config();
    let fixture = harness.l2_fixture(&config);
    let args = CheckArgs {
        bin: super::l1::l1_bin(&harness, &config).into(),
        level: Level::L2,
        fixture: harness.write("fixture.json", &fixture).into(),
        contract: None,
    };
    let (report, judged) = check_binary_mutated(&args, &[], &[]).expect("a report");
    assert!(report.passed, "{}", report.text());
    let l1_args = CheckArgs { level: Level::L1, ..args.clone() };
    let (_, judged_l1) = check_binary_mutated(&l1_args, &[], &[]).expect("a report");
    // The conditions of `L2.effects` are judged at every level: they are this level's all the same.
    let judged_l1: BTreeSet<String> = judged_l1.into_iter().filter(|condition| !condition.starts_with("effects/")).collect();
    let judged: BTreeSet<String> = judged.difference(&judged_l1).filter(|condition| checks.contains(&condition.split('/').next().unwrap_or_default())).cloned().collect();
    let named: BTreeSet<String> = cases.iter().filter(|case| checks.contains(&case.condition.split('/').next().unwrap_or_default())).map(|case| case.condition.to_string()).collect();
    let unplanted: Vec<&String> = judged.difference(&named).collect();
    let unjudged: Vec<&String> = named.difference(&judged).collect();
    assert!(unplanted.is_empty(), "judged by a conforming run and not planted by any case: {unplanted:?}");
    assert!(unjudged.iter().all(|condition| exempt.contains(&condition.as_str())), "planted, never judged by a conforming run, and not named as exempt: {unjudged:?}");
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let firsts: Vec<(Case, bool)> = cases
        .into_iter()
        .filter(|case| checks.contains(&case.condition.split('/').next().unwrap_or_default()))
        .map(|case| {
            let first = seen.insert(owner_of(case.condition));
            (case, first)
        })
        .collect();
    let jobs: Vec<Box<dyn FnOnce() -> Result<(), String> + Send>> = firsts
        .into_iter()
        .map(|(case, first)| -> Box<dyn FnOnce() -> Result<(), String> + Send> {
            Box::new(move || {
                let harness = Harness::new();
                let mut fixture = harness.l2_fixture(&case.config);
                if let Some(tweak) = case.tweak {
                    tweak(&mut fixture);
                }
                let args = CheckArgs {
                    bin: super::l1::l1_bin(&harness, &case.config).into(),
                    level: Level::L2,
                    fixture: harness.write("fixture.json", &fixture).into(),
                    contract: None,
                };
                let owner = owner_of(case.condition);
                if first {
                    let (forced, _) = check_binary_mutated(&args, &[owner.as_str()], &[]).map_err(|failure| format!("{}: {}", case.condition, failure.message))?;
                    if forced.find(&owner).map(|check| check.result) == Some(atip_check::report::CheckResult::Fail) {
                        return Err(format!("{}: forced to pass, {owner} still fails", case.condition));
                    }
                }
                let (skipped, _) = check_binary_mutated(&args, &[], &[case.condition]).map_err(|failure| format!("{}: {}", case.condition, failure.message))?;
                if skipped.find(&owner).map(|check| check.result) == Some(atip_check::report::CheckResult::Fail) {
                    return Err(format!("{}: with the condition switched off {owner} still fails", case.condition));
                }
                Ok(())
            })
        })
        .collect();
    let failures: Vec<String> = in_parallel(jobs).into_iter().filter_map(Result::err).collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Every pass condition of the safety rows and of `L2.effects`, each with the configuration that breaks it and no other.
pub fn safety_cases() -> Vec<Case> {
    let purge = |ops: Vec<Value>| patch(&[("purge", ops)]);
    vec![
        // The repeat of a write.
        case("idempotency/second-run-result", json!({"replayStatusWritten": true})),
        case("idempotency/second-run-no-new-effect", json!({"stampDuplicates": true})),
        case("idempotency/once-second-refused", json!({"onceTwice": true})),
        // The rules of a destructive command, from its document alone.
        case("destructive-safety/option-present", purge(vec![remove("/options/3"), remove("/constraints")])),
        case(
            "destructive-safety/option-flag-only",
            purge(vec![
                set("/options/2/flagOnly", json!(false)),
                set("/schemas/CommandInput/properties/commit", json!({"type": "boolean"})),
                set("/schemas/CommandArguments/properties/commit", json!({"type": "boolean"})),
            ]),
        ),
        case(
            "destructive-safety/no-batch",
            purge(vec![
                set("/input/batch", json!(true)),
                set("/input/maxItems", json!(4)),
                set("/schemas/PurgeBatchReport", json!({"type": "object", "required": ["items"], "properties": {"items": {"type": "array"}}})),
                add("/outputs/1", json!({"kind": "report", "schemaRef": "#/schemas/PurgeBatchReport", "schemaVersion": "1.0", "when": "--input is a list of documents", "batch": true})),
                add("/errors/0", json!("BATCH_INCOMPLETE")),
            ]),
        ),
        // The preview.
        case("destructive-preview/preview-succeeds", json!({"previewFails": true})),
        case("destructive-preview/preview-changes-nothing", json!({"previewWrites": true})),
        case("destructive-preview/plan-digest", json!({"previewNoDigest": true})),
        case("destructive-preview/effects-listed", json!({"previewNoEffects": true})),
        case("destructive-preview/committed-false", json!({"previewCommitted": true})),
        case("destructive-preview/next-commits-the-plan", json!({"previewNoNext": true})),
        case("destructive-preview-stdin/stdin-preview-succeeds", json!({"stdinPreviewRefused": true})),
        case("destructive-preview-stdin/next-without-stdin", json!({"previewNextInput": true})),
        case("destructive-preview-stdin/next-replays-the-plan", json!({"commitDigestChanges": true})),
        // The commit.
        case("destructive-stale/stale-refused", json!({"staleAccepted": true})),
        case("destructive-stale/stale-next-is-the-preview", json!({"staleNoNext": true})),
        case("destructive-half/half-refused", json!({"halfAccepted": true})),
        case("destructive-commit/commit-committed", json!({"commitNotMarked": true})),
        case("destructive-commit/commit-effects-happen", json!({"commitNoEffect": true})),
        // Work that is accepted and not done.
        case("async/fixture-names-an-async-command", json!({})).with_tweak(|fixture| {
            fixture["commands"]["seal"]["async"] = json!({"start": ["seal", "--json"], "worker": "FAKE_WORKER", "outcomes": ["done"]});
        }),
        case("async/start-accepted", json!({"startFails": true})),
        case("async/start-reports-identifier", json!({"startIdentifierMismatch": true})),
        case("async/start-is-not-complete", json!({"startClaimsDone": true})),
        case("async/start-next-exists", json!({"startNoWait": true})),
        case("async/start-next-reads", json!({"startNextWrites": true})),
        case("async/done-exits-0", json!({"waitDoneExits1": true})),
        case("async/failed-exits-1", json!({"failedExits0": true})),
        case("async/timeout-exits-1", json!({"timeoutExits0": true})),
        case("async/timeout-retryable", json!({"retryableCodes": []})),
        case("async/timeout-next-is-the-same-wait", json!({"timeoutNoNext": true})),
        // Operation keys.
        case("operation-key/repeat-succeeds", json!({"operationRepeatRefused": true})),
        case("operation-key/repeat-same-report", json!({"operationNewId": true})),
        case("operation-key/repeat-starts-nothing", json!({"operationRestarts": true})),
        case("operation-key/concurrent-one-start", json!({"operationRace": true})),
        case("operation-key/other-payload-reused", json!({"operationReusedAccepted": true})),
        case("operation-key/reused-field", json!({"reusedFieldWrong": true})),
        // What must change nothing.
        case("effects/version-changes-nothing", json!({"versionWrites": true})),
        case("effects/help-changes-nothing", json!({"helpWrites": true})),
        case("effects/dry-run-changes-nothing", json!({"dryRunWrites": true})),
        case("effects/read-changes-only-allowed", json!({"peekWritesOther": true})),
    ]
}

/// Every pass condition of the rows that are the entry of an agent into the tool, each with the configuration that breaks it and no other.
pub fn guide_cases() -> Vec<Case> {
    vec![
        // The stream.
        case("stream/starts-with-start", json!({"streamPrintsEnvelope": true})),
        case("stream/start-names-the-output", json!({"streamNoOutputOnStart": true})),
        case("stream/ends-with-end", json!({"streamNoEnd": true})),
        case("stream/events-match-the-event-schema", json!({"streamBadEvent": true})),
        case("stream/start-and-end-agree", json!({"streamStartVersionMismatch": true})),
        case("stream/end-names-the-output", json!({"streamEndOutputMismatch": true})),
        case("stream/end-data-matches-the-output", json!({"streamEndBadData": true})),
        case("stream/end-failure-is-registered", json!({"streamEndRetryableWrong": true})),
        case("stream/exit-is-the-end-result", json!({"streamBrokenExit0": true})),
        case("stream/refusal-is-one-envelope", json!({"streamRefusalHasStart": true})),
        // The guides.
        case("skills/writes-nothing", json!({"binAlias": "atip-fake-tool-claims12-l2-skillsputsfile"})),
        case("skills/works-in-an-empty-directory", json!({"binAlias": "atip-fake-tool-claims12-l2-skillsneedsstate"})),
        case("skills/default-guide-shape", json!({"binAlias": "atip-fake-tool-claims12-l2-skillsbadschema"})),
        case("skills/default-lists-itself", json!({"binAlias": "atip-fake-tool-claims12-l2-skillsdefaultnotlisted"})),
        case("skills/guide-shape", json!({"binAlias": "atip-fake-tool-claims12-l2-skillsguidebadschema"})),
        case("skills/guide-name", json!({"binAlias": "atip-fake-tool-claims12-l2-skillswrongname"})),
        case("skills/guides-agree", json!({"binAlias": "atip-fake-tool-claims12-l2-skillsguidesdiffer"})),
        case("skills/unknown-refused", json!({"binAlias": "atip-fake-tool-claims12-l2-skillsunknownok"})),
        case("skills/unknown-lists-the-guides", json!({"binAlias": "atip-fake-tool-claims12-l2-skillsunknownnoexpected"})),
        case("skills/unknown-expected-agrees", json!({"binAlias": "atip-fake-tool-claims12-l2-skillsmissingname"})),
        case("guide-commands/line-parses", json!({"binAlias": "atip-fake-tool-claims12-l2-guidebadoption"})),
        // The search and the first line of `--help`.
        case("search/answers-a-tree", json!({"searchNotTree": true})),
        case("search/echoes-the-query", json!({"searchNoQuery": true})),
        case("search/expected-in-the-first-five", json!({"searchRanksLast": true})),
        case("help-guide/mentions-skills-show", json!({"helpNoSkillsLine": true})),
    ]
}
