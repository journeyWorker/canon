//! The L1 side of the fake tool's test support: the conforming L1 configuration, its fixture, and the seeded
//! violations of the L1 checks (approaches/checker.md 6, 7.2), one configuration per pass condition.
//!
//! A case is `condition -> (configuration, fixture change)`: `condition` is `<check>/<condition>` of an `L1.<check>`
//! (the same shape as the L0 cases, whose `Case::owner` says `L0.`; the owner of these is `L1.<check>`). The fixture
//! of a case is `Harness::l1_fixture(&case.config)` with `case.tweak` applied. No case needs the fixture to omit
//! something (`unprovided`); the fixture of every case is the standard one, apart from the registry, which is the one
//! the configured tool prints (the fixture is the checker's copy of the registry, and a seeded registry violation is
//! not a drift between the two).
//!
//! Most violations of a printed document are `docPatch` members: JSON-patch operations applied to the document
//! after it was built from the model (the behaviour of the tool still follows the unpatched model); see
//! `fake-tool/l1/patch.rs` for the operations and the `name=<text>` / `code=<text>` / `reason=<text>` selectors of
//! array elements. The other violations are runtime toggles, described where they act in `fake-tool/l1/`.

#![allow(dead_code, reason = "each integration test uses a part of this module")]

use std::path::PathBuf;
use std::process::Command;

use serde_json::{json, Map, Value};

use super::{Case, Harness, FAKE_TOOL};

/// The ids of the tree of a conforming L1 fake, in the order `schema --json` lists them.
pub const TREE: [&str; 12] = [
    "init",
    "items",
    "member",
    "member list",
    "member search",
    "member tag",
    "reject",
    "schema",
    "status",
    "team",
    "team add",
    "touch",
];

/// The position of `id` in the tree of a conforming L1 fake.
pub fn tree_index(id: &str) -> usize {
    TREE.iter()
        .position(|entry| *entry == id)
        .unwrap_or_else(|| panic!("{id} is not in the tree"))
}

/// The configuration of the conforming L1 fake: the L1 registry rows (the L0 conforming tool has the seven L0 rows
/// only, which is what the L0 fixture lists).
pub fn l1_standard_config() -> Value {
    json!({"l1Registry": true})
}

/// The standard configuration with the members of `extra` set.
pub fn l1_config(extra: &Value) -> Value {
    let mut config = l1_standard_config();
    if let (Some(config), Some(extra)) = (config.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            config.insert(key.clone(), value.clone());
        }
    }
    config
}

/// The standard configuration of a fake tool that claims 1.2: the copy of the tool named `claims12` claims 1.2 in the runs that get no configuration too.
pub fn l1_config_1_2(extra: &Value) -> Value {
    let mut config = l1_config(&json!({"claims": "1.2", "binAlias": "atip-fake-tool-claims12"}));
    if let (Some(config), Some(extra)) = (config.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            config.insert(key.clone(), value.clone());
        }
    }
    config
}

/// A pass condition of a 1.2 row: `condition` is `<check>/<condition>` as the checker names it, `owner` is the check.
pub fn l1_case_1_2(condition: &'static str, extra: Value, tweak: Option<fn(&mut Value)>) -> Case {
    Case { condition, config: l1_config_1_2(&extra), tweak }
}

/// Every pass condition of the 1.2 halves of the L1 rows and of `L1.check-order`, each with the configuration of a 1.2 tool that breaks it
/// (approaches/checker.md 7.2). A 1.2 case fails its owner and no other check at a claim of 1.2, and fails nothing at a claim of 1.1.
pub fn l1_cases_1_2() -> Vec<Case> {
    vec![
        // grammar: `limit`, `cursor`, `all` and `fields` are flag-only
        l1_case_1_2("grammar/reserved-shape", json!({"limitNotFlagOnly": true}), None),
        // registry: `STOPPED_MIDWAY` is exit 1 and not retryable
        l1_case_1_2("registry/reserved-exit", json!({"stoppedMidway": true, "stoppedMidwayExit2": true}), None),
        l1_case_1_2("registry/reserved-exit", json!({"stoppedMidway": true, "stoppedMidwayRetryable": true}), None),
        // paging: the cursor is bound to what the invocation gave
        l1_case_1_2("paging/cursor-survives-all", json!({"cursorRefusesAll": true}), None),
        l1_case_1_2("paging/cursor-bound-to-given-arguments", json!({"cursorBindsEffective": true}), None),
        l1_case_1_2("paging/cursor-bound-to-environment", json!({"cursorIgnoresEnv": true}), None),
        l1_case_1_2("paging/paged-through-input", json!({"inputRefused": true}), None),
        l1_case_1_2("paging/next-after-input", json!({"nextCarriesDefault": true}), None),
        l1_case_1_2("paging/next-after-input", json!({"nextKeepsInput": true}), None),
        // fields: `--fields` alone is no reason to refuse, and an unknown path is USAGE without `--json` too
        l1_case_1_2("fields/fields-without-json", json!({"fieldsNeedsJson": true}), None),
        l1_case_1_2("fields/unknown-path-refused-in-text", json!({"fieldsUnknownAcceptedInText": true}), None),
        // check-order: the earlier stage answers
        l1_case_1_2("check-order/earlier-stage-answers", json!({"orderConstraintFirst": true}), None),
        l1_case_1_2("check-order/earlier-stage-answers", json!({"orderDomainFirst": true}), None),
        l1_case_1_2("check-order/earlier-stage-answers", json!({"orderLaterDomain": true}), None),
        l1_case_1_2("check-order/earlier-stage-answers", json!({"orderValidatorFirst": true}), None),
        l1_case_1_2("check-order/earlier-stage-answers", json!({"unknownOptionLast": true}), None),
        l1_case_1_2("check-order/earlier-stage-answers", json!({"unknownOptionDeferred": true}), None),
        l1_case_1_2("check-order/earlier-stage-answers", json!({"rulesReversed": true}), None),
    ]
}

/// The 1.2 case fixture: the standard fixture of the tool, changed by the case.
pub fn l1_fixture_of(harness: &Harness, case: &Case) -> Value {
    let mut fixture = harness.l1_fixture(&case.config);
    if let Some(tweak) = case.tweak {
        tweak(&mut fixture);
    }
    fixture
}

/// The executable a case runs: the fake tool itself, or, when the configuration has `binAlias`, a copy of it under
/// that name in the harness's directory. The no-state cases need it: the run of `schema --json` in an empty
/// directory gets no fixture environment, so the only thing that can make the tool misbehave there is how it is
/// called. (The copy is made, not linked, in case the checker resolves a link.)
pub fn l1_bin(harness: &Harness, config: &Value) -> PathBuf {
    let Some(alias) = config["binAlias"].as_str() else {
        return PathBuf::from(FAKE_TOOL);
    };
    let dir = harness.path("bin");
    std::fs::create_dir_all(&dir).expect("bin directory");
    let path = dir.join(alias);
    std::fs::copy(FAKE_TOOL, &path).expect("copy the fake tool");
    path
}

/// What the fixture says about the L1 commands of the fake tool (approaches/checker.md 5).
fn l1_commands(is_1_1: bool, is_1_2: bool) -> Value {
    let mut fields = vec![
        json!(["profile.name"]),
        json!(["owners[].email", "tags"]),
        json!(["name", "nickname"]),
    ];
    if is_1_1 {
        fields.push(json!(["attributes.region"]));
        fields.push(json!(["attributes[\"http.status-code\"]", "profile.email"]));
    }
    json!({
        "items": {"ok": [["items", "--json"]]},
        "status": {"ok": [["status", "--json"]]},
        "touch": {
            "missingValue": [["touch", "--json"]],
            "refused": [["touch", "--input", "nope", "--json"]],
            "ok": [["touch", "--name", "report", "--json"]]
        },
        "team add": {"ok": [["team", "add", "--json"]]},
        "reject": {"refused": [["reject", "--json"]]},
        "schema": {"ok": [["schema", "--errors", "--json"]]},
        "member list": {
            "ok": [["member", "list", "--team", "core", "--limit", "2", "--sort", "name", "--json"]],
            // A 1.2 tool is paged with no filter given, so that its defaults and environment variables are left to be written out.
            "paged": if is_1_2 { json!([["member", "list", "--limit", "1", "--json"]]) } else { json!([["member", "list", "--team", "core", "--limit", "1", "--sort", "name", "--json"]]) },
            "fields": fields
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
            "ok": [["member", "tag", "3", "--tag", "lead", "--tag", "core", "--note", "promoted", "--json"]],
            "refused": [["member", "tag", "99", "--tag", "core", "--json"]],
            "domainRejects": {"tag": ["Core", "-core", "core tag"]}
        }
    })
}

/// The registry rows and warning codes the tool configured by the file at `config_path` prints: the fixture's copy
/// of the registry is the one the tool publishes, so a seeded violation of the published registry is no drift
/// between the two (which only `L0.registry` judges).
fn published(config_path: &std::path::Path) -> Option<(Vec<Value>, Vec<String>)> {
    let dir = tempfile::tempdir().ok()?;
    let run = |args: &[&str]| -> Option<Value> {
        let output = Command::new(FAKE_TOOL)
            .args(args)
            .current_dir(dir.path())
            .env("FAKE_ATIP_CONFIG", config_path)
            .output()
            .ok()?;
        serde_json::from_slice(&output.stdout).ok()
    };
    let errors = run(&["schema", "--errors", "--json"])?;
    let mut rows: Vec<Value> = Vec::new();
    for row in errors["data"]["errors"].as_array()? {
        let code = row["code"].clone();
        if rows.iter().any(|seen| seen["code"] == code) {
            continue;
        }
        let mut entry = json!({"code": code, "exit": row["exit"], "retryable": row["retryable"]});
        let reasons: Vec<Value> = row["reasons"]
            .as_array()
            .map(|reasons| reasons.iter().map(|reason| reason["reason"].clone()).collect())
            .unwrap_or_default();
        if !reasons.is_empty() {
            entry["reasons"] = json!(reasons);
        }
        rows.push(entry);
    }
    let mut warnings: Vec<String> = Vec::new();
    if let Some(answer) = run(&["schema", "--warnings", "--json"]) {
        for row in answer["data"]["warnings"].as_array().into_iter().flatten() {
            let code = row["code"].as_str().unwrap_or_default().to_string();
            if !warnings.contains(&code) {
                warnings.push(code);
            }
        }
    }
    Some((rows, warnings))
}

impl Harness {
    /// The fixture of a fake tool configured by `config` at level L1: the L0 fixture, the registry the tool prints,
    /// and the entries of the L1 commands. A fake that claims 1.0 has no map-key selection and no warning registry.
    pub fn l1_fixture(&self, config: &Value) -> Value {
        let mut fixture = self.fixture(config);
        let is_1_1 = config["claims"] != "1.0";
        if let Some((rows, warnings)) = published(&self.path("fake-config.json")) {
            fixture["registry"] = Value::Array(rows);
            if is_1_1 {
                fixture["warnings"] = json!(warnings);
            }
        }
        fixture["commands"] = l1_commands(is_1_1, config["claims"] == "1.2");
        fixture
    }

    /// The fixture of the conforming L1 fake.
    pub fn l1_standard_fixture(&self) -> Value {
        self.l1_fixture(&l1_standard_config())
    }
}

impl Harness {
    /// `atip check --bin <the fake tool of config> --level L1 --fixture <fixture> --json`, with the given arguments after.
    pub fn check_l1_with(&self, config: &Value, fixture: &Value, extra: &[&str]) -> super::Output {
        let fixture_path = self.write("fixture.json", fixture);
        let bin = l1_bin(self, config);
        let work = self.path("cwd");
        std::fs::create_dir_all(&work).expect("cwd");
        let tmp = self.path("tmp");
        std::fs::create_dir_all(&tmp).expect("tmp");
        let mut args = vec!["check", "--bin", bin.to_str().expect("utf8"), "--level", "L1", "--fixture", fixture_path.to_str().expect("utf8"), "--json"];
        args.extend(extra);
        super::atip_in(&work, &tmp, &args)
    }

    /// The same without more arguments.
    pub fn check_l1(&self, config: &Value, fixture: &Value) -> super::Output {
        self.check_l1_with(config, fixture, &[])
    }
}

/// The fixture of the conforming L1 fake (the function form of [`Harness::l1_standard_fixture`]).
pub fn l1_standard_fixture(harness: &Harness) -> Value {
    harness.l1_standard_fixture()
}

// ---- the operations of a `docPatch` ----

fn set(path: &str, value: Value) -> Value {
    json!({"op": "set", "path": path, "value": value})
}

fn add(path: &str, value: Value) -> Value {
    json!({"op": "add", "path": path, "value": value})
}

fn remove(path: &str) -> Value {
    json!({"op": "remove", "path": path})
}

fn copy(from: &str, path: &str) -> Value {
    json!({"op": "copy", "from": from, "path": path})
}

fn remove_value(path: &str, value: Value) -> Value {
    json!({"op": "remove-value", "path": path, "value": value})
}

/// `{"docPatch": {key: ops}}`; `more` adds the members of another document key.
fn patch(entries: &[(&str, Vec<Value>)]) -> Value {
    let mut patches = Map::new();
    for (key, ops) in entries {
        patches.insert((*key).to_string(), Value::Array(ops.clone()));
    }
    json!({"docPatch": patches})
}

/// An option of a command document, the way the tool prints one (a switch).
fn switch_option(name: &str) -> Value {
    json!({"name": name, "valueType": "switch", "required": false, "repeatable": false, "default": false, "help": "A seeded option"})
}

fn string_option(name: &str, value_name: &str) -> Value {
    json!({"name": name, "valueType": "string", "valueName": value_name, "required": false, "repeatable": false, "default": null, "help": "A seeded option"})
}

fn registry_patch(ops: &[Value], entry_ops: &[(&str, Vec<Value>)]) -> Value {
    let mut entries: Vec<(&str, Vec<Value>)> = vec![("--errors", ops.to_vec())];
    entries.extend(entry_ops.iter().cloned());
    patch(&entries)
}

/// The fixture with `"registry": "from-schema"` (and so no `warnings`): nothing is compared with the tool's registry.
fn from_schema(fixture: &mut Value) {
    fixture["registry"] = json!("from-schema");
    if let Some(object) = fixture.as_object_mut() {
        object.remove("warnings");
    }
}

fn with_fixture(mut case: Case, tweak: fn(&mut Value)) -> Case {
    case.tweak = Some(tweak);
    case
}

const INPUT_INVALID: &str = "/errors/code=INPUT_INVALID";

/// The `NOT_JSON`/`UNKNOWN_KEY` reasons live under `INPUT_INVALID` in both answers: `--errors` (`/errors/code=…`)
/// and `--error INPUT_INVALID` (`/error/…`).
fn reason_patch(make: impl Fn(&str) -> Vec<Value>) -> Value {
    patch(&[
        ("--errors", make(&format!("{INPUT_INVALID}/reasons"))),
        ("--error INPUT_INVALID", make("/error/reasons")),
    ])
}

/// Every pass condition of the L1 checks, each with the configuration that breaks it and (in this list) no other
/// condition: the seeded violations of approaches/checker.md 7.2, one per condition. A violation that is also a JSON
/// Schema failure of another rule is seeded anyway when the check that owns it is the specific one (`grammar`,
/// `reasons`, `walk` rule 5): `tree/valid`-style schema findings that belong to a specific check are the checker's
/// to filter, and the list in the report of this slice names them.
///
/// `patterns/engines-agree` has no case here: no pattern of the restricted form separates the two engines but for a
/// Unicode-version difference, which passes (see [`l1_engine_difference_case`]).
pub fn l1_cases() -> Vec<Case> {
    let case = |condition: &'static str, extra: Value| Case {
        condition,
        config: l1_config(&extra),
        tweak: None,
    };
    vec![
        // tree
        case("tree/answered", json!({"treeAnswer": "usage"})),
        case("tree/valid", patch(&[("<tree>", vec![remove("/commands/0/summary")])])),
        // The tree says L1 and the claim is L2 (`atip_check_l1` runs this case at L2): a tree below the claimed level. Levels are cumulative, so a tree above it passes.
        case("tree/level", json!({})),
        case("tree/groups-first", json!({"treeGroupsLast": true})),
        case("tree/unique-ids", patch(&[("<tree>", vec![copy("/commands/0", "/commands/-")])])),
        case("tree/entry-path", patch(&[("<tree>", vec![set("/commands/0/command", json!("initx"))])])),
        // no-state
        // The run in an empty directory has no fixture environment, so a configuration cannot make the tool refuse
        // there: the seeded tool is a copy of the fake under the name `binAlias` (see `l1_bin`), which the tool
        // reads from its own name.
        case(
            "no-state/answers-in-an-empty-directory",
            json!({"binAlias": "atip-fake-tool-needs-config"}),
        ),
        case(
            "no-state/writes-nothing",
            json!({"schemaWrites": true, "binAlias": "atip-fake-tool-writes"}),
        ),
        // walk
        case("walk/answered", json!({"schemaUnknown": ["touch"]})),
        case("walk/valid", patch(&[("member tag", vec![remove("/summary")])])),
        // The document of `init` says it is a group; the tree is untouched.
        // The tree lists `team` as a group and its document is a complete, valid command document: nothing but `data.kind` is wrong.
        case(
            "walk/kind",
            patch(&[(
                "team",
                vec![
                    set("/kind", json!("command")),
                    remove("/commands"),
                    set("/effect", json!("read")),
                    set("/operands", json!([])),
                    set("/options", json!([{"name": "json", "valueType": "switch", "required": false, "repeatable": false, "default": false, "flagOnly": true, "help": "Print the answer as one JSON document"}])),
                    set("/errors", json!(["USAGE", "COMMAND_REQUIRED"])),
                    set("/outputs", json!([{"kind": "report", "schemaRef": "#/schemas/TeamReport", "schemaVersion": "1.0"}])),
                    set("/examples", json!(["fake team --json"])),
                    set("/schemas", json!({"TeamReport": {"type": "object"}})),
                ],
            )]),
        ),
        case(
            "walk/group-path",
            patch(&[("team", vec![set("/command", json!("teams"))])]),
        ),
        case("walk/group-children", patch(&[("member", vec![remove("/commands/2")])])),
        case("walk/rule-1", patch(&[("init", vec![set("/command", json!("ini"))])])),
        case(
            "walk/rule-2-refs",
            patch(&[("init", vec![set("/outputs/0/schemaRef", json!("#/schemas/Missing"))])]),
        ),
        case(
            "walk/rule-2-compile",
            patch(&[("member list", vec![add("/schemas/Broken", json!({"pattern": "("}))])]),
        ),
        // A third output, distinguished by its `when`, with the schema of the first.
        case(
            "walk/rule-2-outputs",
            patch(&[(
                "member search",
                vec![add(
                    "/outputs/-",
                    json!({"kind": "report", "schemaRef": "#/schemas/MemberSearchReport", "schemaVersion": "1.0", "when": "--quiet"}),
                )],
            )]),
        ),
        case(
            "walk/rule-3-defaults",
            patch(&[("member search", vec![set("/options/name=sort/default", json!("bogus"))])]),
        ),
        case(
            "walk/rule-3-domain-profile",
            // A domain on a switch: a switch has two values and no domain (spec 10.7.4).
            patch(&[("member search", vec![add("/options/name=quiet/domain", json!({"minimum": 1}))])]),
        ),
        case(
            "walk/rule-3-domain-compile",
            patch(&[("member search", vec![set("/options/name=filter/domain", json!({"type": "object", "required": ["team", "team"]}))])]),
        ),
        case(
            "walk/rule-4-names",
            patch(&[("member tag", vec![set("/constraints/0/if/name", json!("nosuch"))])]),
        ),
        case(
            "walk/rule-4-codes",
            patch(&[("member tag", vec![add("/constraints/0/code", json!("DATE_INVALID"))])]),
        ),
        case(
            "walk/rule-4-relation-once",
            patch(&[("member tag", vec![copy("/constraints/0", "/constraints/-")])]),
        ),
        // `touch` is a batch command: its document says the input is no batch, and the batch output stays.
        case(
            "walk/rule-5",
            patch(&[("touch", vec![remove("/outputs/1/batch")])]),
        ),
        case("walk/rule-6", patch(&[("member search", vec![remove("/outputs/1/when")])])),
        case(
            "walk/rule-7",
            patch(&[("touch", vec![set("/input/example", json!({"name": 5}))])]),
        ),
        case("walk/rule-9", patch(&[("member list", vec![copy("/options/name=json", "/options/-")])])),
        case(
            "walk/rule-10",
            patch(&[("touch", vec![add("/schemas/TouchInput/required", json!(["name"]))])]),
        ),
        case(
            "walk/rule-11",
            patch(&[("member list", vec![add("/schemas/MemberRow/minProperties", json!(1))])]),
        ),
        case(
            "walk/rule-13",
            patch(&[(
                "member search",
                vec![add("/examples/-", json!("fake member search ada --exact --since=2026-02-30 --json"))],
            )]),
        ),
        // quoted-path
        case("quoted-path/same-answer", json!({"quotedPathDiffers": true})),
        // schema-errors
        case("schema-errors/unknown-path", json!({"schemaUnknownCode": "USAGE"})),
        case("schema-errors/unknown-path-shape", json!({"schemaUnknownField": true})),
        case("schema-errors/overlap", json!({"schemaOverlapAnswers": true})),
        case("schema-errors/group-with-errors", json!({"schemaGroupErrorsAnswers": true})),
        case("schema-errors/unknown-error-code", json!({"schemaErrorUnknownCode": "UNKNOWN_COMMAND"})),
        case("schema-errors/unknown-error-field", json!({"schemaErrorUnknownField": "--code"})),
        // registry
        case(
            "registry/codes-unique",
            registry_patch(&[copy("/errors/code=USAGE", "/errors/-")], &[]),
        ),
        case("registry/reserved-exit", json!({"reservedExit": {"INPUT_TOO_LARGE": 1}})),
        case(
            "registry/required-codes",
            registry_patch(&[remove("/errors/code=INTERNAL")], &[]),
        ),
        case(
            "registry/commands-registered",
            patch(&[("member list", vec![add("/errors/-", json!("NOT_REGISTERED"))])]),
        ),
        // reasons
        // The fixture cannot hold a reason that is not UPPER_SNAKE_CASE or one listed twice (`FIXTURE_INVALID`): these two take the registry from the tool.
        with_fixture(
            case(
                "reasons/reason-form",
                reason_patch(|base| vec![set(&format!("{base}/reason=UNKNOWN_KEY/reason"), json!("unknown-key"))]),
            ),
            from_schema,
        ),
        with_fixture(
            case(
                "reasons/reasons-unique",
                reason_patch(|base| vec![copy(&format!("{base}/0"), &format!("{base}/-"))]),
            ),
            from_schema,
        ),
        case(
            "reasons/no-exit-on-reason",
            reason_patch(|base| vec![add(&format!("{base}/0/exit"), json!(2))]),
        ),
        case(
            "reasons/entry-lists-same-reasons",
            registry_patch(&[remove(&format!("{INPUT_INVALID}/reasons/1"))], &[]),
        ),
        // reasons-reserved (SHOULD)
        case(
            "reasons-reserved/reserved-names",
            reason_patch(|base| vec![set(&format!("{base}/reason=UNKNOWN_KEY/reason"), json!("UNKNOWN_FIELD"))]),
        ),
        // warnings
        case(
            "warnings/codes-unique",
            patch(&[("--warnings", vec![copy("/warnings/0", "/warnings/-")])]),
        ),
        case("warnings/unknown-code-refused", json!({"schemaWarningUnknownField": "--code"})),
        case("warnings/meaning-agrees", json!({"dualWarning": true})),
        // reserved-applicable
        case(
            "reserved-applicable/input-codes",
            patch(&[("touch", vec![remove_value("/errors", json!("INPUT_CONFLICT"))])]),
        ),
        case(
            "reserved-applicable/batch-code",
            patch(&[("touch", vec![remove_value("/errors", json!("BATCH_INCOMPLETE"))])]),
        ),
        case(
            "reserved-applicable/commit-code",
            patch(&[("member tag", vec![add("/options/-", {
                let mut option = switch_option("commit");
                option["flagOnly"] = json!(true);
                option
            })])]),
        ),
        case(
            "reserved-applicable/operation-code",
            patch(&[("member tag", vec![add("/options/-", string_option("operation", "KEY"))])]),
        ),
        // `reject` is never run with --cursor: its document offers a cursor and does not list CURSOR_INVALID.
        case(
            "reserved-applicable/cursor-code",
            patch(&[("reject", vec![add("/options/-", string_option("cursor", "CURSOR"))])]),
        ),
        // codes-known
        case(
            "codes-known/code-in-errors",
            patch(&[("member tag", vec![remove_value("/errors", json!("MEMBER_NOT_FOUND"))])]),
        ),
        // groups-not-runnable
        case("groups-not-runnable/command-required", json!({"groupRunnable": true})),
        case("groups-not-runnable/expected-is-the-groups-verbs", json!({"groupExpectedWrong": true})),
        // grammar
        case(
            "grammar/kebab-words",
            patch(&[("member list", vec![add("/options/-", switch_option("Dry_Run"))])]),
        ),
        case(
            "grammar/no-id-suffix",
            patch(&[("member list", vec![add("/options/-", switch_option("member-id"))])]),
        ),
        case(
            "grammar/reserved-shape",
            patch(&[("status", vec![remove("/options/name=json/flagOnly")])]),
        ),
        // depth (SHOULD)
        case("depth/at-most-two-words", json!({"deepTree": true})),
        // one-meaning
        case(
            "one-meaning/value-name",
            patch(&[("member tag", vec![set("/options/name=tag/valueName", json!("LABEL"))])]),
        ),
        case(
            "one-meaning/value-type",
            patch(&[("touch", vec![set("/options/name=repo/valueType", json!("string"))])]),
        ),
        // one-meaning-values (SHOULD)
        case(
            "one-meaning-values/overlapping",
            patch(&[(
                "member list",
                vec![
                    set("/options/name=sort/possibleValues", json!(["id", "joined"])),
                    set("/options/name=sort/default", json!("id")),
                ],
            )]),
        ),
        // help-line
        case("help-line/mentions-schema", json!({"helpNoSchemaLine": true})),
        // defaults (SHOULD)
        case("defaults/help-agrees", json!({"helpDefaultDiffers": true})),
        // deterministic (SHOULD)
        case("deterministic/tree-bytes", json!({"treeNondeterministic": true})),
        case("deterministic/doc-bytes", json!({"docNondeterministic": ["touch"]})),
        // examples
        case(
            "examples/tool-name",
            patch(&[(
                "member list",
                vec![set(
                    "/examples/0",
                    json!("other member list --team core --limit 2 --fields profile.name,owners[].email --json"),
                )],
            )]),
        ),
        case(
            "examples/path",
            patch(&[("member list", vec![set("/examples/1", json!("fake member lst --team=core --sort id --json"))])]),
        ),
        case(
            "examples/grammar",
            patch(&[(
                "member list",
                vec![set("/examples/1", json!("fake member list --team=core --team=ops --json"))],
            )]),
        ),
        case(
            "examples/no-placeholder",
            patch(&[("member list", vec![set("/examples/1", json!("fake member list --team <TEAM> --json"))])]),
        ),
        case(
            "examples/values",
            patch(&[("member tag", vec![add("/examples/-", json!("fake member tag 0 --tag core --json"))])]),
        ),
        case(
            "examples/requirements",
            patch(&[("member tag", vec![add("/examples/-", json!("fake member tag 3 --json"))])]),
        ),
        case(
            "examples/constraints",
            patch(&[(
                "member tag",
                vec![add("/examples/-", json!("fake member tag 3 --tag core --note a --clear-note --json"))],
            )]),
        ),
        case(
            "examples/arguments-ref",
            patch(&[("touch", vec![add("/schemas/TouchArguments/properties/name/maxLength", json!(5))])]),
        ),
        case(
            "examples/input-inline",
            patch(&[("touch", vec![add("/examples/-", json!("fake touch --input - --json"))])]),
        ),
        // example-runs
        case("example-runs/not-usage", {
            let mut config = patch(&[(
                "member list",
                vec![add("/examples/-", json!("fake member list --team ghost --json"))],
            )]);
            config["ghostTeamUsage"] = json!(true);
            config
        }),
        case("example-runs/exit-or-registered-code", {
            let mut config = patch(&[(
                "member search",
                vec![add(
                    "/examples/-",
                    json!("fake member search --filter '{\"team\":\"ghost\"}' --exact --json"),
                )],
            )]);
            config["ghostFilterCrash"] = json!(true);
            config
        }),
        // constraints
        case("constraints/refused", json!({"ignoreConstraint": {"member search": [0]}})),
        case("constraints/declared-code", json!({"constraintCodeUsage": true})),
        case(
            "constraints/equivalent-codes-agree",
            patch(&[(
                "member tag",
                vec![
                    add("/constraints/1/code", json!("SORT_CONFLICT")),
                    add("/constraints/2/code", json!("DATE_INVALID")),
                    add("/errors/-", json!("SORT_CONFLICT")),
                    add("/errors/-", json!("DATE_INVALID")),
                ],
            )]),
        ),
        // domains
        case("domains/inside-accepted", json!({"strictBounds": {"member search": ["min-score"]}})),
        case("domains/outside-refused", json!({"acceptDomain": {"member list": ["limit"]}})),
        case("domains/refusal-names-argument", json!({"domainOmitsField": true})),
        // patterns
        case(
            "patterns/form",
            patch(&[("member search", vec![set("/options/name=tag/domain/pattern", json!("^[a-z0-9&&a-z]+$"))])]),
        ),
        case(
            "patterns/rust-compiles",
            patch(&[("member search", vec![set("/options/name=tag/domain/pattern", json!("^\\p{L}{1,500}$"))])]),
        ),
        // numbers
        case("numbers/beyond-safe-range-refused", json!({"roundBigIntegers": true})),
        // validators
        case("validators/refused", json!({"validatorRejectsNothing": true})),
        case("validators/declared-code", json!({"validatorCodeUsage": true})),
        case("validators/field-is-an-argument", json!({"validatorField": "--sort"})),
        // output-data
        case(
            "output-data/data-validates",
            json!({"dataPatch": {"member search": [{"op": "remove", "path": "/count"}]}}),
        ),
        case(
            "output-data/schema-version",
            json!({"metaPatch": {"member search": [{"op": "set", "path": "/schemaVersion", "value": "1.5"}]}}),
        ),
        // next
        case("next/names-a-command", json!({"nextBogusCommand": true})),
        case("next/read-next-not-usage", json!({"nextUsage": true})),
        // hint-command (SHOULD): the refusal of `member tag 99` carries `fake member lst --json`, one command line that
        // names no command (L0.hint-form still holds).
        case("hint-command/parses-as-a-command", json!({"hintNoCommand": true})),
        // paging
        case("paging/returned-at-most-limit", json!({"limitIgnored": true})),
        case("paging/next-page-follows", json!({"cursorRepeats": true})),
        case("paging/changed-filter-refused", json!({"cursorFilterRefusedAsUsage": true})),
        // fields
        case("fields/meta-fields", json!({"fieldsMetaOmitsKeep": true})),
        case("fields/no-unselected", json!({"fieldsKeepsUnselected": true})),
        case("fields/selected-present", json!({"fieldsDropsSelected": true})),
        case("fields/projected-schema", json!({"fieldsBreaksSchema": true})),
        case("fields/unknown-path-refused", json!({"fieldsAcceptsUnknown": true})),
    ]
}

/// The `patterns/engines-agree` case that can be made, and it PASSES: the published pattern `^\p{L}+$` of `tag`
/// with the fixture's `domainRejects` value U+11DB0 (a letter in Unicode 17, unassigned in Unicode 16) is the one
/// disagreement the restricted I-Regexp form allows, a Unicode-version difference between the two engines' tables.
/// The checker's report must name the code point and both Unicode versions and pass; no engine difference in the
/// fake tool can be made to fail, because the fake enforces the pattern it really uses (`^[a-z][a-z0-9-]*$`), which
/// refuses U+11DB0, whatever it publishes.
pub fn l1_engine_difference_case() -> Case {
    fn reject_the_code_point(fixture: &mut Value) {
        fixture["commands"]["member search"]["domainRejects"]["tag"] = json!(["\u{11DB0}"]);
    }
    Case {
        condition: "patterns/engines-agree",
        config: l1_config(&patch(&[(
            "member search",
            vec![set("/options/name=tag/domain/pattern", json!("^\\p{L}+$"))],
        )])),
        tweak: Some(reject_the_code_point),
    }
}
