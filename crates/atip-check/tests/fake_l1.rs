//! The L1 fake tool, run directly (not through the checker): its documents validate, its behaviour is what its
//! documents declare, and every seeded violation of `tests/support/l1.rs` changes the output it claims to.

mod support;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

use support::l1::{l1_bin, l1_cases, l1_config, l1_engine_difference_case, l1_standard_config, TREE};
use support::{Case, Harness, FAKE_TOOL};

/// One run of the fake tool.
struct Answer {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Answer {
    fn document(&self) -> Value {
        assert!(
            self.stdout.ends_with('\n') && self.stdout.matches('\n').count() == 1,
            "stdout is not one line: {:?} (stderr {:?})",
            self.stdout,
            self.stderr
        );
        serde_json::from_str(&self.stdout).unwrap_or_else(|error| panic!("not JSON ({error}): {:?}", self.stdout))
    }

    fn error(&self) -> Value {
        let document = self.document();
        assert_eq!(document["ok"], false, "{document}");
        document["error"].clone()
    }

}

/// A fake tool in a scratch directory, configured by `config`.
struct Fake {
    cwd: tempfile::TempDir,
    config: PathBuf,
    _config_dir: tempfile::TempDir,
    bin: PathBuf,
}

impl Fake {
    fn new(config: &Value) -> Self {
        let config_dir = tempfile::tempdir().expect("tempdir");
        let path = config_dir.path().join("config.json");
        std::fs::write(&path, config.to_string()).expect("write the configuration");
        Self {
            cwd: tempfile::tempdir().expect("tempdir"),
            config: path,
            _config_dir: config_dir,
            bin: PathBuf::from(FAKE_TOOL),
        }
    }

    fn standard() -> Self {
        Self::new(&l1_standard_config())
    }

    fn dir(&self) -> &Path {
        self.cwd.path()
    }

    fn run(&self, args: &[&str]) -> Answer {
        let output = Command::new(&self.bin)
            .args(args)
            .current_dir(self.dir())
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", self.dir())
            .env("LANG", "C.UTF-8")
            .env("FAKE_ATIP_CONFIG", &self.config)
            .output()
            .expect("run the fake tool");
        Answer {
            code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    /// `args` and `--json`, exit 0: the document.
    fn ok(&self, args: &[&str]) -> Value {
        let mut all = args.to_vec();
        all.push("--json");
        let answer = self.run(&all);
        let document = answer.document();
        assert_eq!(answer.code, 0, "{args:?}: {document}");
        assert_eq!(document["ok"], true, "{args:?}: {document}");
        assert!(atip::envelope_problems(&document).is_empty(), "{args:?}: {:?}", atip::envelope_problems(&document));
        document
    }

    /// `args` and `--json`, exit 2: the error.
    fn refused(&self, args: &[&str]) -> Value {
        let mut all = args.to_vec();
        all.push("--json");
        let answer = self.run(&all);
        assert_eq!(answer.code, 2, "{args:?}: {}", answer.stdout);
        let document = answer.document();
        assert!(atip::envelope_problems(&document).is_empty(), "{args:?}: {:?}", atip::envelope_problems(&document));
        document["error"].clone()
    }

    /// The answer of `schema` with `args` (the document of `data`), checked against the contract's schemas.
    fn schema(&self, args: &[&str]) -> Value {
        let mut all = vec!["schema"];
        all.extend(args);
        let document = self.ok(&all);
        let problems = atip::schema_doc_problems(&document);
        assert!(problems.is_empty(), "{args:?}: {problems:?}");
        document["data"].clone()
    }

    fn command_doc(&self, id: &str) -> Value {
        let words: Vec<&str> = id.split(' ').collect();
        self.schema(&words)
    }
}

fn validator(doc: &Value, reference: &str) -> jsonschema::Validator {
    let name = reference.strip_prefix("#/schemas/").expect("a #/schemas/ reference");
    let mut root = json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "$ref": format!("#/schemas/{name}")});
    root["schemas"] = doc["schemas"].clone();
    atip_check::schema::schemas()
        .validator(&root)
        .unwrap_or_else(|error| panic!("{reference} does not compile: {error}"))
}

/// Whether `data` of `document` validates against the output schema `meta.output` names (or the only output).
fn assert_data_valid(doc: &Value, document: &Value) {
    let outputs = doc["outputs"].as_array().expect("outputs");
    let output = match document["meta"]["output"].as_str() {
        Some(name) => outputs
            .iter()
            .find(|output| output["schemaRef"] == format!("#/schemas/{name}"))
            .unwrap_or_else(|| panic!("meta.output {name} names no output")),
        None => {
            assert_eq!(outputs.len(), 1, "meta.output is required with several outputs: {document}");
            &outputs[0]
        }
    };
    assert_eq!(document["meta"]["schemaVersion"], output["schemaVersion"], "{document}");
    // A projected answer is judged by the schema with `required` reduced (6.10); the structure of the records is
    // asserted exactly where the projections are tested.
    let judged = if document["meta"].get("fields").is_some() { loosened(doc) } else { doc.clone() };
    let schema = validator(&judged, output["schemaRef"].as_str().expect("schemaRef"));
    let errors: Vec<String> = schema.iter_errors(&document["data"]).map(|error| error.to_string()).collect();
    assert!(errors.is_empty(), "{errors:?} in {}", document["data"]);
}

fn ids(entries: &Value) -> Vec<String> {
    entries
        .as_array()
        .expect("an array")
        .iter()
        .map(|entry| entry["command"].as_str().expect("command").to_string())
        .collect()
}

// ---- documents ----

#[test]
fn atip_checkl1_fake_every_document_validates_and_the_tree_is_the_commands() {
    for config in [l1_standard_config(), l1_config(&json!({"claims": "1.0"})), l1_config(&json!({"deepTree": true}))] {
        let fake = Fake::new(&config);
        let tree = fake.schema(&[]);
        assert_eq!(tree["kind"], "tree");
        assert_eq!(tree["level"], "L1");
        let listed = ids(&tree["commands"]);
        if config["deepTree"] != true {
            assert_eq!(listed, TREE.to_vec(), "the tree is the one the support lists");
        } else {
            assert!(listed.contains(&"team roster list".to_string()));
        }
        let mut sorted = listed.clone();
        sorted.sort();
        assert_eq!(sorted, listed, "a group before its commands, in a stable order");
        for entry in tree["commands"].as_array().expect("commands") {
            let id = entry["command"].as_str().expect("command");
            assert_eq!(entry["path"], json!(id.split(' ').collect::<Vec<_>>()));
            let words: Vec<&str> = id.split(' ').collect();
            let doc = fake.schema(&words);
            assert_eq!(doc["kind"], entry["kind"], "{id}");
            assert_eq!(doc["command"], id);
            assert_eq!(doc["summary"], entry["summary"]);
            if entry["kind"] == "group" {
                let children: Vec<Value> = tree["commands"]
                    .as_array()
                    .expect("commands")
                    .iter()
                    .filter(|child| {
                        let path = child["path"].as_array().expect("path");
                        path.len() == words.len() + 1 && path[..words.len()] == *entry["path"].as_array().expect("path")
                    })
                    .cloned()
                    .collect();
                assert_eq!(doc["commands"], json!(children), "the children of {id}");
            } else {
                assert_eq!(doc["effect"], entry["effect"]);
                // The one-string form is the same answer.
                let quoted = fake.schema(&[id]);
                assert_eq!(quoted, doc, "schema \"{id}\" differs from the split form");
            }
        }
        let errors = fake.schema(&["--errors"]);
        for row in errors["errors"].as_array().expect("errors") {
            let code = row["code"].as_str().expect("code");
            assert_eq!(fake.schema(&["--error", code])["error"], *row);
        }
        if config["claims"] != "1.0" {
            let warnings = fake.schema(&["--warnings"]);
            for row in warnings["warnings"].as_array().expect("warnings") {
                let code = row["code"].as_str().expect("code");
                assert_eq!(fake.schema(&["--warning", code])["warning"], *row);
            }
        }
    }
}

#[test]
fn atip_checkl1_fake_the_commands_register_every_code_they_list_and_the_reserved_ones_of_their_features() {
    let fake = Fake::standard();
    let registry = fake.schema(&["--errors"]);
    let codes: BTreeSet<String> = registry["errors"]
        .as_array()
        .expect("errors")
        .iter()
        .map(|row| row["code"].as_str().expect("code").to_string())
        .collect();
    for required in ["USAGE", "UNKNOWN_COMMAND", "COMMAND_REQUIRED", "INTERNAL"] {
        assert!(codes.contains(required), "{required}");
    }
    for id in TREE {
        let doc = fake.command_doc(id);
        if doc["kind"] != "command" {
            continue;
        }
        let errors: Vec<&str> = doc["errors"].as_array().expect("errors").iter().filter_map(Value::as_str).collect();
        for code in &errors {
            assert!(codes.contains(*code), "{id} lists {code}, which the registry lacks");
        }
        let has = |name: &str| doc["options"].as_array().expect("options").iter().any(|option| option["name"] == name);
        if has("cursor") {
            assert!(errors.contains(&"CURSOR_INVALID"), "{id}");
        }
        if doc.get("input").is_some() {
            for code in ["INPUT_INVALID", "INPUT_CONFLICT", "INPUT_TOO_LARGE"] {
                assert!(errors.contains(&code), "{id} lacks {code}");
            }
        }
        for constraint in doc["constraints"].as_array().into_iter().flatten() {
            if let Some(code) = constraint["code"].as_str() {
                assert!(errors.contains(&code), "{id}: constraint code {code}");
            }
        }
        for validator in doc["validators"].as_array().into_iter().flatten() {
            assert!(errors.contains(&validator["code"].as_str().expect("code")), "{id}: validator code");
        }
    }
}

#[test]
fn atip_checkl1_fake_help_points_at_schema_and_shows_the_defaults_of_the_documents() {
    let fake = Fake::standard();
    let root = fake.run(&["--help"]);
    assert_eq!(root.code, 0);
    assert!(root.stdout.lines().next().unwrap_or_default().contains("schema"), "{}", root.stdout);
    for id in TREE {
        let words: Vec<&str> = id.split(' ').chain(["--help"]).collect();
        let help = fake.run(&words);
        assert_eq!(help.code, 0, "{id}");
        assert!(help.stdout.lines().next().unwrap_or_default().contains("schema"), "{id}: {}", help.stdout);
        if let Some(command) = Some(fake.command_doc(id)).filter(|doc| doc["kind"] == "command") {
            for option in command["options"].as_array().expect("options") {
                let shows = format!("[default: {}]", option["default"].as_str().map_or_else(|| option["default"].to_string(), ToString::to_string));
                let wanted = !option["default"].is_null() && !matches!(option["valueType"].as_str(), Some("switch" | "count"));
                assert_eq!(help.stdout.contains(&shows), wanted, "{id} --{}: {}", option["name"], help.stdout);
            }
        }
    }
    // A seeded help does not mention schema.
    let bare = Fake::new(&l1_config(&json!({"helpNoSchemaLine": true}))).run(&["--help"]);
    assert!(!bare.stdout.lines().next().unwrap_or_default().contains("schema"));
}

#[test]
fn atip_checkl1_fake_schema_refuses_what_the_contract_says() {
    let fake = Fake::standard();
    let unknown = fake.refused(&["schema", "nosuch-xyz"]);
    assert_eq!((unknown["code"].as_str(), unknown["field"].as_str()), (Some("UNKNOWN_COMMAND"), Some("command")));
    assert!(unknown["expected"].as_array().is_some_and(|expected| expected.contains(&json!("member"))));
    let nested = fake.refused(&["schema", "member", "nosuch"]);
    assert_eq!(nested["expected"], json!(["list", "search", "tag"]));
    for args in [
        vec!["schema", "--errors", "--error", "USAGE"],
        vec!["schema", "team", "--errors"],
        vec!["schema", "--warnings", "--errors"],
    ] {
        let error = fake.refused(&args);
        assert_eq!(error["code"], "USAGE", "{args:?}");
        assert!(error["field"].is_string(), "{args:?}");
    }
    let error = fake.refused(&["schema", "--error", "NOSUCH_XYZ"]);
    assert_eq!((error["code"].as_str(), error["field"].as_str()), (Some("USAGE"), Some("--error")));
    let warning = fake.refused(&["schema", "--warning", "NOSUCH_XYZ"]);
    assert_eq!((warning["code"].as_str(), warning["field"].as_str()), (Some("USAGE"), Some("--warning")));
    let group = fake.refused(&["team"]);
    assert_eq!((group["code"].as_str(), group["expected"].clone()), (Some("COMMAND_REQUIRED"), json!(["add"])));
    let group = fake.refused(&["member", "--repo", ".", "--json"][..3]);
    assert_eq!((group["code"].as_str(), group["expected"].clone()), (Some("COMMAND_REQUIRED"), json!(["list", "search", "tag"])));
}

// ---- the documents and the behaviour agree ----

#[test]
fn atip_checkl1_fake_every_example_of_a_read_command_runs_and_its_data_validates() {
    let fake = Fake::standard();
    for id in TREE {
        let doc = fake.command_doc(id);
        if doc["kind"] != "command" || doc["effect"] != "read" {
            continue;
        }
        for example in doc["examples"].as_array().expect("examples") {
            let words = atip::split_argv(example.as_str().expect("example"));
            assert_eq!(words[0], "fake", "{example}");
            let args: Vec<&str> = words[1..].iter().map(String::as_str).collect();
            let answer = fake.run(&args);
            let document = answer.document();
            assert_eq!(answer.code, 0, "{example}: {document}");
            assert_data_valid(&doc, &document);
        }
    }
}

#[test]
fn atip_checkl1_fake_every_validator_rejects_line_is_refused_with_its_code_and_names_an_argument() {
    let fake = Fake::standard();
    let doc = fake.command_doc("member search");
    let validators = doc["validators"].as_array().expect("validators");
    assert!(!validators.is_empty());
    for validator in validators {
        for line in validator["rejects"].as_array().expect("rejects") {
            assert!(!doc["examples"].as_array().expect("examples").contains(line));
            let words = atip::split_argv(line.as_str().expect("line"));
            let args: Vec<&str> = words[1..].iter().map(String::as_str).collect();
            let error = Fake::standard().run(&args).error();
            assert_eq!(error["code"], validator["code"], "{line}");
            let field = error["field"].as_str().expect("field");
            assert!(
                validator["arguments"].as_array().expect("arguments").iter().any(|name| field == format!("--{}", name.as_str().expect("name"))),
                "{field}"
            );
        }
    }
    // The date the pattern admits and the calendar refuses, and one it accepts (leap day).
    assert_eq!(fake.refused(&["member", "search", "cy", "--exact", "--since", "2026-02-30"])["code"], "DATE_INVALID");
    fake.ok(&["member", "search", "cy", "--exact", "--since", "2028-02-29"]);
    assert_eq!(fake.refused(&["member", "search", "cy", "--exact", "--since", "2027-02-29"])["code"], "DATE_INVALID");
}

/// The invocation of `member search` with the criteria every rule is satisfied by, and `extra` words.
fn search(extra: &[&str]) -> Vec<String> {
    let mut args: Vec<String> = ["member", "search", "cy", "--exact"].iter().map(ToString::to_string).collect();
    args.extend(extra.iter().map(ToString::to_string));
    args
}

fn run_args(fake: &Fake, args: &[String]) -> Answer {
    let mut all: Vec<&str> = args.iter().map(String::as_str).collect();
    all.push("--json");
    fake.run(&all)
}

#[test]
fn atip_checkl1_fake_domains_are_enforced_at_their_bounds_and_name_the_argument() {
    let fake = Fake::standard();
    let accepted = |extra: &[&str]| {
        let answer = run_args(&fake, &search(extra));
        assert_eq!(answer.code, 0, "{extra:?}: {}", answer.stdout);
    };
    let refused = |extra: &[&str], field: &str| {
        let answer = run_args(&fake, &search(extra));
        assert_eq!(answer.code, 2, "{extra:?}: {}", answer.stdout);
        let error = answer.error();
        assert_eq!((error["code"].as_str(), error["field"].as_str()), (Some("USAGE"), Some(field)), "{extra:?}");
    };
    // minimum / maximum
    accepted(&["--min-score", "0"]);
    accepted(&["--min-score", "100"]);
    refused(&["--min-score", "-1"], "--min-score");
    refused(&["--min-score", "101"], "--min-score");
    // exclusiveMinimum / exclusiveMaximum
    accepted(&["--age", "1"]);
    accepted(&["--age", "149"]);
    refused(&["--age", "0"], "--age");
    refused(&["--age", "150"], "--age");
    // multipleOf
    accepted(&["--step", "10"]);
    refused(&["--step", "7"], "--step");
    // a count's number of occurrences
    accepted(&["-v", "-v", "-v"]);
    refused(&["-v", "-v", "-v", "-v"], "--verbose");
    // pattern
    accepted(&["--tag", "core-1"]);
    for bad in ["Core", "1core", "core tag", ""] {
        refused(&[&format!("--tag={bad}")], "--tag");
    }
    refused(&["--tag=-core"], "--tag");
    // the structural domain of a json option
    accepted(&["--filter", "{\"team\":\"core\",\"min\":3}"]);
    for bad in ["[1]", "{\"team\":1}", "{\"bogus\":true}", "{\"min\":-1}", "not json", "null"] {
        refused(&[&format!("--filter={bad}")], "--filter");
    }
    // minLength / maxLength of the operand
    for good in ["ab", &"x".repeat(40)] {
        let answer = fake.run(&["member", "search", good, "--exact", "--json"]);
        assert_eq!(answer.code, 0, "{good}");
    }
    for bad in ["a", &"x".repeat(41)] {
        let error = fake.run(&["member", "search", bad, "--exact", "--json"]).error();
        assert_eq!((error["code"].as_str(), error["field"].as_str()), (Some("USAGE"), Some("QUERY")), "{bad}");
    }
    // an enum, a number that is no integer
    refused(&["--sort", "bogus"], "--sort");
    refused(&["--age", "3.5"], "--age");
    refused(&["--age", "1e2"], "--age");
    // limit: the minimum, with no maximum
    let list = |value: &str| fake.run(&["member", "list", &format!("--limit={value}"), "--json"]);
    assert_eq!(list("1").code, 0);
    assert_eq!(list("0").code, 2);
    assert_eq!(list("-1").error()["field"], "--limit");
}

#[test]
fn atip_checkl1_fake_integers_beyond_the_safe_range_are_refused_and_never_rounded() {
    let fake = Fake::standard();
    for (value, accepted) in [
        ("9007199254740991", true),
        ("9007199254740992", false),
        ("9007199254740993", false),
        ("-9007199254740991", false),
        ("-9007199254740992", false),
        ("99999999999999999999999", false),
    ] {
        let answer = fake.run(&["member", "list", &format!("--limit={value}"), "--json"]);
        // A negative value is refused by the domain of `limit`; the positive ones show the range.
        let expected = if accepted { 0 } else { 2 };
        assert_eq!(answer.code, expected, "{value}: {}", answer.stdout);
    }
    // An integer without a domain of its own range: the operand of `member tag` has `minimum: 1` only.
    let tag = fake.run(&["member", "tag", "9007199254740993", "--tag", "core", "--json"]);
    assert_eq!((tag.code, tag.error()["code"].clone()), (2, json!("USAGE")));
    let tag = fake.run(&["member", "tag", "9007199254740991", "--tag", "core", "--json"]);
    assert_eq!((tag.code, tag.error()["code"].clone()), (2, json!("MEMBER_NOT_FOUND")));
    // The seeded violation rounds it.
    let rounding = Fake::new(&l1_config(&json!({"roundBigIntegers": true})));
    let answer = rounding.run(&["member", "list", "--limit=9007199254740993", "--json"]);
    assert_eq!(answer.code, 0, "{}", answer.stdout);
}

#[test]
fn atip_checkl1_fake_constraints_of_every_rule_are_enforced_as_the_document_declares() {
    let fake = Fake::standard();
    let doc = fake.command_doc("member search");
    let rules: Vec<&str> = doc["constraints"].as_array().expect("constraints").iter().map(|rule| rule["rule"].as_str().expect("rule")).collect();
    assert_eq!(rules, ["atLeastOne", "exactlyOne", "atMostOne", "requires", "forbids"]);
    let refused = |args: Vec<String>, code: &str, field: &str| {
        let error = run_args(&fake, &args).error();
        assert_eq!((error["code"].as_str(), error["field"].as_str()), (Some(code), Some(field)), "{args:?}");
    };
    let strings = |words: &[&str]| words.iter().map(ToString::to_string).collect::<Vec<_>>();
    // atLeastOne [query, tag, filter]
    refused(strings(&["member", "search", "--exact"]), "USAGE", "QUERY");
    for criterion in [vec!["--tag", "lead"], vec!["--filter", "{}"], vec!["cy"]] {
        let mut args = strings(&["member", "search", "--exact"]);
        args.extend(criterion.iter().map(ToString::to_string));
        assert_eq!(run_args(&fake, &args).code, 0, "{args:?}");
    }
    // exactlyOne [exact, prefix]
    refused(strings(&["member", "search", "cy"]), "USAGE", "--exact");
    refused(strings(&["member", "search", "cy", "--exact", "--prefix"]), "USAGE", "--exact");
    assert_eq!(run_args(&fake, &strings(&["member", "search", "cy", "--prefix"])).code, 0);
    // atMostOne [quiet, verbose]
    refused(search(&["--quiet", "-v"]), "USAGE", "--quiet");
    assert_eq!(run_args(&fake, &search(&["--quiet"])).code, 0);
    assert_eq!(run_args(&fake, &search(&["-v"])).code, 0);
    // requires: sort in [score] requires min-score
    refused(search(&["--sort", "score"]), "USAGE", "--min-score");
    assert_eq!(run_args(&fake, &search(&["--sort", "score", "--min-score", "5"])).code, 0);
    assert_eq!(run_args(&fake, &search(&["--sort", "name"])).code, 0);
    // forbids with a code: sort in [age] forbids step
    refused(search(&["--sort", "age", "--step", "5"]), "SORT_CONFLICT", "--step");
    assert_eq!(run_args(&fake, &search(&["--sort", "age"])).code, 0);
    assert_eq!(fake.run(&["member", "search", "cy", "--exact", "--sort", "age", "--step", "5", "--json"]).code, 2);
    // the seeded ones
    let skipping = Fake::new(&l1_config(&json!({"ignoreConstraint": {"member search": [0]}})));
    assert_eq!(run_args(&skipping, &strings(&["member", "search", "--exact"])).code, 0);
    let as_usage = Fake::new(&l1_config(&json!({"constraintCodeUsage": true})));
    refused_in(&as_usage, search(&["--sort", "age", "--step", "5"]), "USAGE", "--step");
}

fn refused_in(fake: &Fake, args: Vec<String>, code: &str, field: &str) {
    let error = run_args(fake, &args).error();
    assert_eq!((error["code"].as_str(), error["field"].as_str()), (Some(code), Some(field)), "{args:?}");
}

#[test]
fn atip_checkl1_fake_the_write_command_has_an_entailed_constraint_and_two_equivalent_ones_and_writes_a_file() {
    let fake = Fake::standard();
    let doc = fake.command_doc("member tag");
    assert_eq!(doc["effect"], "write");
    assert_eq!(doc["idempotency"], "replay-safe");
    let rules = doc["constraints"].as_array().expect("constraints");
    // `tag` requires `member`, which is required: entailed.
    assert_eq!(rules[0], json!({"rule": "requires", "if": {"name": "tag"}, "then": ["member"]}));
    assert!(doc["operands"][0]["required"] == true);
    // `atMostOne [note, clear-note]` and `note` forbids `clear-note`: equivalent.
    assert_eq!(rules[1], json!({"rule": "atMostOne", "names": ["note", "clear-note"]}));
    assert_eq!(rules[2], json!({"rule": "forbids", "if": {"name": "note"}, "then": ["clear-note"]}));
    refused_in(&fake, strings(&["member", "tag", "3", "--tag", "core", "--note", "a", "--clear-note"]), "USAGE", "--note");
    refused_in(&fake, strings(&["member", "tag", "--tag", "core"]), "USAGE", "MEMBER");
    // the domains of its operand and its option
    refused_in(&fake, strings(&["member", "tag", "0", "--tag", "core"]), "USAGE", "MEMBER");
    refused_in(&fake, strings(&["member", "tag", "3", "--tag", "Core"]), "USAGE", "--tag");
    refused_in(&fake, strings(&["member", "tag", "3", "--tag", "core", &format!("--note={}", "x".repeat(81))]), "USAGE", "--note");
    // a member that does not exist refuses before anything is written
    let missing = fake.refused(&["member", "tag", "99", "--tag", "core"]);
    assert_eq!((missing["code"].as_str(), missing["field"].as_str()), (Some("MEMBER_NOT_FOUND"), Some("MEMBER")));
    assert!(!fake.dir().join("tags").exists());
    // it writes tags/<MEMBER>.json, and the same call again changes nothing
    let first = fake.ok(&["member", "tag", "3", "--tag", "lead", "--tag", "core", "--note", "promoted"]);
    assert_data_valid(&doc, &first);
    assert_eq!(first["data"], json!({"member": 3, "tags": ["core", "lead"], "note": "promoted", "status": "written"}));
    let file = fake.dir().join("tags").join("3.json");
    let written = std::fs::read_to_string(&file).expect("the file of the member");
    let again = fake.ok(&["member", "tag", "3", "--tag", "lead", "--tag", "core", "--note", "promoted"]);
    assert_eq!(again["data"]["status"], "unchanged");
    assert_eq!(std::fs::read_to_string(&file).expect("the file"), written);
    // --clear-note removes the note
    let cleared = fake.ok(&["member", "tag", "3", "--tag", "core", "--clear-note"]);
    assert_eq!((cleared["data"]["status"].as_str(), cleared["data"].get("note")), (Some("written"), None));
}

fn strings(words: &[&str]) -> Vec<String> {
    words.iter().map(ToString::to_string).collect()
}

#[test]
fn atip_checkl1_fake_touch_takes_its_arguments_inline_and_refuses_what_the_registry_names() {
    let fake = Fake::standard();
    let doc = fake.command_doc("touch");
    let input = &doc["input"];
    assert_eq!((input["option"].as_str(), input["batch"].clone(), input["maxItems"].clone()), (Some("--input"), json!(true), json!(8)));
    assert_eq!(doc["outputs"].as_array().map(Vec::len), Some(2));
    assert_eq!(doc["outputs"][1]["batch"], true);
    let fragment = validator(&doc, input["schemaRef"].as_str().expect("schemaRef"));
    assert!(fragment.is_valid(&input["example"]));
    let arguments = validator(&doc, input["argumentsRef"].as_str().expect("argumentsRef"));
    assert!(arguments.is_valid(&json!({"name": "report"})));
    assert_eq!(fake.ok(&["touch", "--name", "report"])["data"], json!({"touched": true}));
    assert!(fake.dir().join("touched.txt").exists());
    let example = serde_json::to_string(&input["example"]).expect("json");
    assert_eq!(fake.ok(&["touch", "--input", &example])["data"], json!({"touched": true}));
    let not_json = fake.refused(&["touch", "--input", "nope"]);
    assert_eq!(
        (not_json["code"].as_str(), not_json["reason"].as_str(), not_json["field"].as_str()),
        (Some("INPUT_INVALID"), Some("NOT_JSON"), Some("--input"))
    );
    let unknown = fake.refused(&["touch", "--input", "{\"bogus\":1}"]);
    assert_eq!(
        (unknown["code"].as_str(), unknown["reason"].as_str(), unknown["field"].as_str()),
        (Some("INPUT_INVALID"), Some("UNKNOWN_KEY"), Some("/bogus"))
    );
    let wrong = fake.refused(&["touch", "--input", "{\"name\":5}"]);
    assert_eq!((wrong["code"].as_str(), wrong["field"].as_str()), (Some("INPUT_INVALID"), Some("/name")));
    let conflict = fake.refused(&["touch", "--name", "a", "--input", "{\"name\":\"b\"}"]);
    assert_eq!((conflict["code"].as_str(), conflict["field"].as_str()), (Some("INPUT_CONFLICT"), Some("/name")));
    assert_eq!(fake.ok(&["touch", "--name", "a", "--input", "{\"name\":\"a\"}"])["ok"], true);
    let big = format!("{{\"name\":\"{}\"}}", "x".repeat(5000));
    assert_eq!(fake.refused(&["touch", "--input", &big])["code"], "INPUT_TOO_LARGE");
    let missing = fake.refused(&["touch"]);
    assert_eq!((missing["code"].as_str(), missing["field"].as_str()), (Some("USAGE"), Some("--name")));
    // A batch: one report with an item per document, validated against the batch output; refused whole when a document is wrong.
    let batch = fake.ok(&["touch", "--input", "[{\"name\":\"a\"},{\"name\":\"b\"}]"]);
    assert_eq!(batch["meta"]["output"], "TouchBatchReport");
    assert_data_valid(&doc, &batch);
    assert_eq!(batch["data"]["items"].as_array().map(Vec::len), Some(2));
    let single = fake.ok(&["touch", "--name", "a"]);
    assert_eq!(single["meta"]["output"], "TouchReport");
    assert_data_valid(&doc, &single);
    let wrong = fake.refused(&["touch", "--input", "[{\"name\":\"a\"},{\"bogus\":1}]"]);
    assert_eq!((wrong["code"].as_str(), wrong["field"].as_str()), (Some("INPUT_INVALID"), Some("/1/bogus")));
    let many = format!("[{}]", ["{\"name\":\"a\"}"; 9].join(","));
    assert_eq!(fake.refused(&["touch", "--input", &many])["code"], "INPUT_TOO_LARGE");
    assert_eq!(fake.refused(&["touch", "--input", "[]"])["code"], "INPUT_INVALID");
    let nameless = fake.refused(&["touch", "--input", "[{\"name\":\"a\"},{}]"]);
    assert_eq!((nameless["code"].as_str(), nameless["field"].as_str()), (Some("USAGE"), Some("/1/name")));
}

// ---- paging and projection ----

/// The `meta.next` entry that carries `--cursor <cursor>`.
fn next_with(document: &Value, cursor: &str) -> Vec<String> {
    document["meta"]["next"]
        .as_array()
        .expect("meta.next")
        .iter()
        .find(|entry| entry["argv"].as_array().is_some_and(|argv| argv.windows(2).any(|pair| pair[0] == "--cursor" && pair[1] == cursor)))
        .unwrap_or_else(|| panic!("no next entry carries {cursor}: {document}"))["argv"]
        .as_array()
        .expect("argv")
        .iter()
        .map(|word| word.as_str().expect("word").to_string())
        .collect()
}

fn page_ids(document: &Value) -> Vec<i64> {
    document["data"]["members"]
        .as_array()
        .expect("members")
        .iter()
        .map(|member| member["id"].as_i64().expect("id"))
        .collect()
}

#[test]
fn atip_checkl1_fake_paging_follows_the_next_entry_and_binds_the_cursor_to_the_other_arguments() {
    let fake = Fake::standard();
    let doc = fake.command_doc("member list");
    let mut argv = strings(&["member", "list", "--team", "core", "--limit", "1", "--json"]);
    let mut seen = Vec::new();
    let mut first_cursor = None;
    loop {
        let answer = fake.run(&argv.iter().map(String::as_str).collect::<Vec<_>>());
        let document = answer.document();
        assert_eq!(answer.code, 0, "{document}");
        assert!(atip::envelope_problems(&document).is_empty());
        assert_data_valid(&doc, &document);
        assert_eq!(document["meta"]["page"]["returned"], 1);
        assert_eq!(document["meta"]["page"]["total"], 4);
        seen.extend(page_ids(&document));
        match document["meta"]["page"]["nextCursor"].as_str() {
            Some(cursor) => {
                first_cursor.get_or_insert_with(|| cursor.to_string());
                argv = next_with(&document, cursor);
                assert!(argv.contains(&"--json".to_string()) && argv.contains(&"--limit".to_string()));
            }
            None => {
                assert!(document["meta"].get("next").is_none(), "no next page, no next entry");
                break;
            }
        }
    }
    assert_eq!(seen, [2, 4, 1, 3], "every core member once, by name");
    let cursor = first_cursor.expect("a first page has a next cursor");
    // A cursor reused with other arguments, or not made by this command, is CURSOR_INVALID on --cursor.
    for extra in [vec!["--team", "ops"], vec!["--sort", "id"], vec!["--team", "core", "--sort", "id"]] {
        let mut args = vec!["member", "list", "--cursor", &cursor, "--limit", "1"];
        if !extra.contains(&"--team") {
            args.extend(["--team", "core"]);
        }
        args.extend(&extra);
        let error = fake.refused(&args);
        assert_eq!((error["code"].as_str(), error["field"].as_str()), (Some("CURSOR_INVALID"), Some("--cursor")), "{args:?}");
    }
    for bad in ["nope", "m1.99.00000000", "m1.x.y"] {
        let error = fake.refused(&["member", "list", "--team", "core", "--cursor", bad]);
        assert_eq!((error["code"].as_str(), error["field"].as_str()), (Some("CURSOR_INVALID"), Some("--cursor")), "{bad}");
    }
    // --limit, --fields and --all do not bind it.
    let more = fake.ok(&["member", "list", "--team", "core", "--cursor", &cursor, "--limit", "2", "--fields", "name"]);
    assert_eq!(page_ids(&more), [4, 1]);
    let rest = fake.ok(&["member", "list", "--team", "core", "--cursor", &cursor, "--all"]);
    assert_eq!(page_ids(&rest), [4, 1, 3]);
    assert!(rest["meta"]["page"].get("nextCursor").is_none());
    // No limit pages by the default of 50: one page, no next entry.
    let all = fake.ok(&["member", "list"]);
    assert_eq!(page_ids(&all).len(), 5);
    assert!(all["meta"].get("next").is_none());
    // The seeded violations.
    let first_page = |config: Value| -> (Value, Fake) {
        let seeded = Fake::new(&l1_config(&config));
        (seeded.ok(&["member", "list", "--team", "core", "--limit", "1"]), seeded)
    };
    let (page, _seeded) = first_page(json!({"limitIgnored": true}));
    assert_eq!(page["meta"]["page"]["returned"], 2);
    assert_eq!(page_ids(&page).len(), 2);
    let (page, seeded) = first_page(json!({"cursorRepeats": true}));
    let cursor = page["meta"]["page"]["nextCursor"].as_str().expect("cursor").to_string();
    let second = seeded.ok(&["member", "list", "--team", "core", "--limit", "1", "--cursor", &cursor]);
    assert_eq!(page_ids(&second), page_ids(&page), "the next page repeats the first");
    let (page, seeded) = first_page(json!({"cursorIgnoresFilter": true}));
    let cursor = page["meta"]["page"]["nextCursor"].as_str().expect("cursor").to_string();
    assert_eq!(seeded.run(&["member", "list", "--team", "ops", "--cursor", &cursor, "--json"]).code, 0);
    let (page, seeded) = first_page(json!({"cursorFilterRefusedAsUsage": true}));
    let cursor = page["meta"]["page"]["nextCursor"].as_str().expect("cursor").to_string();
    let error = seeded.refused(&["member", "list", "--team", "ops", "--cursor", &cursor]);
    assert_eq!((error["code"].as_str(), error["field"].as_str()), (Some("USAGE"), Some("--cursor")));
}

fn members_of(document: &Value) -> Vec<Value> {
    document["data"]["members"].as_array().expect("members").clone()
}

/// The schema of the records with `required` dropped: a projection removes members, and `required` is reduced to
/// the kept ones in every cut node at or below the record (6.10), which the structure checks below state exactly.
fn loosened(doc: &Value) -> Value {
    let mut doc = doc.clone();
    for name in ["MemberRow", "Profile", "Owner"] {
        if let Some(schema) = doc["schemas"][name].as_object_mut() {
            schema.remove("required");
        }
    }
    doc
}

#[test]
fn atip_checkl1_fake_fields_keep_the_selection_and_the_keep_paths_and_nothing_else() {
    let fake = Fake::standard();
    let doc = fake.command_doc("member list");
    let fields = &doc["outputs"][0]["fields"];
    assert_eq!((fields["records"].as_str(), fields["keep"].clone()), (Some("/members"), json!(["id"])));
    let paths: Vec<&str> = fields["paths"].as_array().expect("paths").iter().filter_map(Value::as_str).collect();
    assert!(paths.contains(&"profile.name") && paths.contains(&"owners[].email") && paths.contains(&"attributes"));
    let base = fake.ok(&["member", "list", "--team", "core"]);
    let select = |fields: &str| -> Value {
        let document = fake.ok(&["member", "list", "--team", "core", "--fields", fields]);
        assert_data_valid(&loosened(&doc), &document);
        document
    };
    // a nested path: the profile keeps its name only, a null profile stays null
    let document = select("profile.name");
    assert_eq!(document["meta"]["fields"], json!(["profile.name", "id"]));
    for (projected, original) in members_of(&document).iter().zip(members_of(&base)) {
        let expected = match original["profile"].as_object() {
            Some(profile) => json!({"id": original["id"], "profile": {"name": profile["name"]}}),
            None => json!({"id": original["id"], "profile": null}),
        };
        assert_eq!(*projected, expected);
    }
    // an array path: every element keeps its email; a selected whole array and a missing optional member
    let document = select("owners[].email,tags,nickname");
    assert_eq!(document["meta"]["fields"], json!(["owners[].email", "tags", "nickname", "id"]));
    for (projected, original) in members_of(&document).iter().zip(members_of(&base)) {
        let emails: Vec<Value> = original["owners"].as_array().expect("owners").iter().map(|owner| json!({"email": owner["email"]})).collect();
        let mut expected = json!({"id": original["id"], "owners": emails, "tags": original["tags"]});
        if let Some(nickname) = original.get("nickname") {
            expected["nickname"] = nickname.clone();
        }
        assert_eq!(*projected, expected);
    }
    // a covered member is kept whole, even when a descendant is selected as well
    let document = select("profile.name,profile");
    for (projected, original) in members_of(&document).iter().zip(members_of(&base)) {
        assert_eq!(projected["profile"], original["profile"]);
    }
    // the keys of a declared map
    let document = select("attributes.region");
    for (projected, original) in members_of(&document).iter().zip(members_of(&base)) {
        let expected: serde_json::Map<String, Value> = original["attributes"]
            .as_object()
            .expect("attributes")
            .iter()
            .filter(|(key, _)| key.as_str() == "region")
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        assert_eq!(projected["attributes"], Value::Object(expected));
    }
    let document = select("attributes[\"http.status-code\"],name");
    assert_eq!(document["meta"]["fields"], json!(["attributes[\"http.status-code\"]", "name", "id"]));
    assert!(members_of(&document).iter().any(|member| member["attributes"]["http.status-code"] == "200"));
    // an unknown path, a path through an array without [], a keep path, a bracketed identifier
    for bad in ["nosuch", "profile.nosuch", "owners.email", "owners[]", "id", "attributes[\"region\"]", "attributes.region.x", "name,,tags", ""] {
        let error = fake.refused(&["member", "list", &format!("--fields={bad}")]);
        assert_eq!((error["code"].as_str(), error["field"].as_str()), (Some("USAGE"), Some("--fields")), "{bad}");
        assert!(error["expected"].as_array().is_some_and(|expected| expected.contains(&json!("profile.name"))), "{bad}");
    }
    // a 1.0 tool keeps a map whole: its keys are not selectable
    let old = Fake::new(&l1_config(&json!({"claims": "1.0"})));
    assert_eq!(old.refused(&["member", "list", "--fields", "attributes.region"])["field"], "--fields");
    old.ok(&["member", "list", "--fields", "attributes"]);
    // the seeded violations
    let seeded = |config: Value, fields: &str| -> Value { Fake::new(&l1_config(&config)).ok(&["member", "list", "--team", "core", "--fields", fields]) };
    let document = seeded(json!({"fieldsKeepsUnselected": true}), "profile.name");
    assert!(members_of(&document).iter().any(|member| member["profile"].get("email").is_some()), "{document}");
    let document = seeded(json!({"fieldsDropsSelected": true}), "name,nickname");
    assert!(members_of(&document).iter().all(|member| member.get("nickname").is_none()));
    assert!(members_of(&base).iter().any(|member| member.get("nickname").is_some()));
    let document = seeded(json!({"fieldsMetaOmitsKeep": true}), "name");
    assert_eq!(document["meta"]["fields"], json!(["name"]));
    assert!(members_of(&document)[0].get("id").is_some());
    let document = seeded(json!({"fieldsBreaksSchema": true}), "tags");
    let errors = validator(&loosened(&doc), "#/schemas/MemberListReport").iter_errors(&document["data"]).count();
    assert!(errors > 0);
    let document = seeded(json!({"fieldsAcceptsUnknown": true}), "nosuch,name");
    assert_eq!(document["meta"]["fields"], json!(["name", "id"]));
}

// ---- conformance at L0, and the seeded violations ----

#[test]
fn atip_checkl1_fake_the_l1_tool_passes_every_l0_check() {
    for config in [l1_standard_config(), l1_config(&json!({"claims": "1.0"}))] {
        let harness = Harness::new();
        let fixture = harness.l1_fixture(&config);
        let output = harness.check(&fixture, true);
        assert_eq!(output.code, Some(0), "{}", output.stdout);
    }
}

/// Every condition of the L1 checks the lead's checker judges (checker.md 6), as `<check>/<condition>`.
fn conditions() -> BTreeSet<String> {
    let table: [(&str, &[&str]); 31] = [
        ("tree", &["answered", "valid", "level", "groups-first", "unique-ids", "entry-path"]),
        ("no-state", &["answers-in-an-empty-directory", "writes-nothing"]),
        (
            "walk",
            &[
                "answered", "valid", "kind", "group-path", "group-children", "rule-1", "rule-2-refs", "rule-2-compile",
                "rule-2-outputs", "rule-3-defaults", "rule-3-domain-profile",
                "rule-3-domain-compile", "rule-4-names", "rule-4-codes", "rule-4-relation-once", "rule-5", "rule-6",
                "rule-7", "rule-9", "rule-10", "rule-11", "rule-13",
            ],
        ),
        ("quoted-path", &["same-answer"]),
        ("schema-errors", &["unknown-path", "unknown-path-shape", "overlap", "group-with-errors", "unknown-error-code", "unknown-error-field"]),
        ("registry", &["codes-unique", "reserved-exit", "required-codes", "commands-registered"]),
        ("reasons", &["reason-form", "reasons-unique", "no-exit-on-reason", "entry-lists-same-reasons"]),
        ("reasons-reserved", &["reserved-names"]),
        ("warnings", &["codes-unique", "unknown-code-refused", "meaning-agrees"]),
        ("reserved-applicable", &["input-codes", "batch-code", "commit-code", "operation-code", "cursor-code"]),
        ("codes-known", &["code-in-errors"]),
        ("groups-not-runnable", &["command-required", "expected-is-the-groups-verbs"]),
        ("grammar", &["kebab-words", "no-id-suffix", "reserved-shape"]),
        ("depth", &["at-most-two-words"]),
        ("one-meaning", &["value-name", "value-type"]),
        ("one-meaning-values", &["overlapping"]),
        ("help-line", &["mentions-schema"]),
        ("defaults", &["help-agrees"]),
        ("deterministic", &["tree-bytes", "doc-bytes"]),
        (
            "examples",
            &["tool-name", "path", "grammar", "no-placeholder", "values", "requirements", "constraints", "arguments-ref", "input-inline"],
        ),
        ("example-runs", &["not-usage", "exit-or-registered-code"]),
        ("constraints", &["refused", "declared-code", "equivalent-codes-agree"]),
        ("domains", &["inside-accepted", "outside-refused", "refusal-names-argument"]),
        ("patterns", &["form", "rust-compiles", "engines-agree"]),
        ("numbers", &["beyond-safe-range-refused"]),
        ("validators", &["refused", "declared-code", "field-is-an-argument"]),
        ("output-data", &["data-validates", "schema-version"]),
        ("next", &["names-a-command", "read-next-not-usage"]),
        ("paging", &["returned-at-most-limit", "next-page-follows", "changed-filter-refused"]),
        ("fields", &["meta-fields", "no-unselected", "selected-present", "projected-schema", "unknown-path-refused"]),
        ("hint-command", &["parses-as-a-command"]),
    ];
    table
        .iter()
        .flat_map(|(check, names)| names.iter().map(move |name| format!("{check}/{name}")))
        .collect()
}

#[test]
fn atip_checkl1_fake_every_condition_of_the_l1_checks_has_a_seeded_case() {
    let mut have: BTreeSet<String> = l1_cases().iter().map(|case| case.condition.to_string()).collect();
    assert_eq!(have.len(), l1_cases().len(), "a condition has two cases");
    have.insert(l1_engine_difference_case().condition.to_string());
    let wanted = conditions();
    assert_eq!(have.difference(&wanted).collect::<Vec<_>>(), Vec::<&String>::new(), "cases for no condition");
    assert_eq!(wanted.difference(&have).collect::<Vec<_>>(), Vec::<&String>::new(), "conditions without a case");
}

/// The invocations that show a case: the answers of the conforming fake and of the seeded one differ on them.
fn probes(case: &Case) -> Vec<Vec<String>> {
    let mut probes: Vec<Vec<String>> = Vec::new();
    if let Some(patches) = case.config["docPatch"].as_object() {
        for key in patches.keys() {
            let args = match key.as_str() {
                "<tree>" => vec!["schema"],
                "--errors" => vec!["schema", "--errors"],
                "--warnings" => vec!["schema", "--warnings"],
                other => match other.split_once(' ') {
                    Some(("--error", code)) => vec!["schema", "--error", code],
                    Some(("--warning", code)) => vec!["schema", "--warning", code],
                    _ => [&["schema"][..], &other.split(' ').collect::<Vec<_>>()].concat(),
                },
            };
            probes.push(strings(&args));
        }
    }
    let by_name: &[&[&str]] = match case.condition {
        "tree/answered" | "tree/groups-first" | "deterministic/tree-bytes" | "no-state/writes-nothing" => &[&["schema"]],
        "no-state/answers-in-an-empty-directory" => &[&["schema"]],
        "walk/answered" => &[&["schema", "touch"]],
        "depth/at-most-two-words" => &[&["schema"], &["schema", "team", "roster", "list"], &["team", "roster", "list"]],
        "quoted-path/same-answer" => &[&["schema", "member list"]],
        "schema-errors/unknown-path" | "schema-errors/unknown-path-shape" => &[&["schema", "nosuch-xyz"]],
        "schema-errors/overlap" => &[&["schema", "--errors", "--error", "USAGE"]],
        "schema-errors/group-with-errors" => &[&["schema", "team", "--errors"]],
        "schema-errors/unknown-error-code" | "schema-errors/unknown-error-field" => &[&["schema", "--error", "NOSUCH_XYZ"]],
        "registry/reserved-exit" => &[&["schema", "--errors"]],
        "warnings/unknown-code-refused" => &[&["schema", "--warning", "NOSUCH_XYZ"]],
        "warnings/meaning-agrees" => &[&["schema", "--warnings"]],
        "groups-not-runnable/command-required" => &[&["team"]],
        "groups-not-runnable/expected-is-the-groups-verbs" => &[&["member"]],
        "help-line/mentions-schema" => &[&["--help"]],
        "defaults/help-agrees" => &[&["member", "list", "--help"]],
        "deterministic/doc-bytes" => &[&["schema", "touch"]],
        "example-runs/not-usage" => &[&["member", "list", "--team", "ghost"]],
        "example-runs/exit-or-registered-code" => &[&["member", "search", "--filter", "{\"team\":\"ghost\"}", "--exact"]],
        "constraints/refused" => &[&["member", "search", "--exact"]],
        "constraints/declared-code" => &[&["member", "search", "cy", "--exact", "--sort", "age", "--step", "5"]],
        "domains/inside-accepted" => &[&["member", "search", "cy", "--exact", "--min-score", "0"]],
        "domains/outside-refused" | "domains/refusal-names-argument" => &[&["member", "list", "--limit", "0"]],
        "numbers/beyond-safe-range-refused" => &[&["member", "list", "--limit=9007199254740993"]],
        "validators/refused" | "validators/declared-code" | "validators/field-is-an-argument" => {
            &[&["member", "search", "cy", "--exact", "--since=2026-02-30"]]
        }
        "output-data/data-validates" | "output-data/schema-version" => &[&["member", "search", "cy", "--exact"]],
        "next/names-a-command" | "next/read-next-not-usage" => &[&["status"]],
        "hint-command/parses-as-a-command" => &[&["member", "tag", "99", "--tag", "core"]],
        "paging/returned-at-most-limit" | "paging/next-page-follows" | "paging/changed-filter-refused" => {
            &[&["member", "list", "--team", "core", "--limit", "1"]]
        }
        "fields/meta-fields" | "fields/no-unselected" => &[&["member", "list", "--team", "core", "--fields", "profile.name"]],
        "fields/selected-present" => &[&["member", "list", "--team", "core", "--fields", "name,nickname"]],
        "fields/projected-schema" => &[&["member", "list", "--team", "core", "--fields", "tags"]],
        "fields/unknown-path-refused" => &[&["member", "list", "--fields", "nosuch"]],
        _ => &[],
    };
    probes.extend(by_name.iter().map(|args| strings(args)));
    probes
}

fn answer_of(fake: &Fake, args: &[String]) -> (i32, String) {
    let mut all: Vec<&str> = args.iter().map(String::as_str).collect();
    all.push("--json");
    let answer = fake.run(&all);
    (answer.code, answer.stdout)
}

#[test]
fn atip_checkl1_fake_every_seeded_case_changes_the_output_it_claims_to() {
    let harness = Harness::new();
    let conforming = Fake::standard();
    let mut cases = l1_cases();
    cases.push(l1_engine_difference_case());
    for case in &cases {
        // `tree/level` is told from the conforming tool by the claim (L2 over an L1 tree), not by what the tool prints.
        if case.condition == "tree/level" {
            continue;
        }
        let mut seeded = Fake::new(&case.config);
        seeded.bin = l1_bin(&harness, &case.config);
        let probes = probes(case);
        assert!(!probes.is_empty(), "{}: no probe shows the case", case.condition);
        for probe in &probes {
            let (standard, other) = match case.condition {
                // These two are told from the conforming tool by how it is called, in an empty directory without
                // the configuration; the conforming one answers, the seeded one refuses or writes.
                "no-state/writes-nothing" => {
                    answer_of(&conforming, probe);
                    answer_of(&seeded, probe);
                    assert!(seeded.dir().join("schema-cache.txt").exists(), "the seeded tool writes");
                    assert!(!conforming.dir().join("schema-cache.txt").exists(), "the conforming tool writes nothing");
                    continue;
                }
                "no-state/answers-in-an-empty-directory" => {
                    let bare = |fake: &Fake| {
                        let output = Command::new(&fake.bin)
                            .args(["schema", "--json"])
                            .current_dir(fake.dir())
                            .env_clear()
                            .output()
                            .expect("run");
                        (output.status.code().unwrap_or(-1), String::from_utf8_lossy(&output.stdout).into_owned())
                    };
                    (bare(&conforming), bare(&seeded))
                }
                "deterministic/tree-bytes" | "deterministic/doc-bytes" => {
                    assert_eq!(answer_of(&conforming, probe), answer_of(&conforming, probe), "{}: the conforming tool is stable", case.condition);
                    (answer_of(&seeded, probe), answer_of(&seeded, probe))
                }
                "paging/changed-filter-refused" => {
                    let first = conforming.ok(&["member", "list", "--team", "core", "--limit", "1"]);
                    let cursor = first["meta"]["page"]["nextCursor"].as_str().expect("a cursor").to_string();
                    let reuse = strings(&["member", "list", "--team", "ops", "--limit", "1", "--cursor", &cursor]);
                    (answer_of(&conforming, &reuse), answer_of(&seeded, &reuse))
                }
                _ => (answer_of(&conforming, probe), answer_of(&seeded, probe)),
            };
            // Two runs of the seeded tool differ (`deterministic`), or the seeded one differs from the conforming one.
            assert!(standard != other, "{} on {probe:?}: the seeded tool answers as the conforming one", case.condition);
            // The answer of a seeded document is still an envelope (the help is text; the crash prints nothing).
            if case.condition == "example-runs/exit-or-registered-code" && probe.first().map(String::as_str) != Some("schema") {
                assert_eq!((other.0, other.1.as_str()), (1, ""), "the seeded run ends with exit 1 and no output");
            } else if !probe.iter().any(|word| word == "--help") {
                let document: Value = serde_json::from_str(&other.1).unwrap_or_else(|error| panic!("{}: {error}: {}", case.condition, other.1));
                assert!(atip::envelope_problems(&document).is_empty(), "{}: {:?}", case.condition, atip::envelope_problems(&document));
            }
        }
    }
    // The one that writes, writes.
    let writing = Fake::new(&l1_config(&json!({"schemaWrites": true})));
    writing.ok(&["schema"]);
    assert!(writing.dir().join("schema-cache.txt").exists());
    assert!(!conforming.dir().join("schema-cache.txt").exists());
}

#[test]
fn atip_checkl1_fake_a_fixture_registry_read_from_the_schema_passes_l0_registry() {
    let harness = Harness::new();
    let config = l1_standard_config();
    let mut fixture = harness.l1_fixture(&config);
    fixture["registry"] = json!("from-schema");
    fixture.as_object_mut().expect("object").remove("warnings");
    let fixture_path = harness.write("fixture.json", &fixture);
    let bin = l1_bin(&harness, &config);
    let work = harness.path("cwd");
    std::fs::create_dir_all(&work).expect("cwd");
    let tmp = harness.path("tmp");
    std::fs::create_dir_all(&tmp).expect("tmp");
    let output = support::atip_in(
        &work,
        &tmp,
        &[
            "check",
            "--bin",
            bin.to_str().expect("utf8"),
            "--level",
            "L1",
            "--fixture",
            fixture_path.to_str().expect("utf8"),
            "--json",
        ],
    );
    assert_eq!(output.check("L0.registry")["result"], "pass", "{}", output.stdout);
}

#[test]
fn atip_checkl1_fake_the_seeded_broken_schema_does_not_compile_and_the_conforming_ones_do() {
    let case = l1_cases().into_iter().find(|case| case.condition == "walk/rule-2-compile").expect("the case");
    let doc = Fake::new(&case.config).command_doc("member list");
    let mut root = json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "$ref": "#/schemas/Broken"});
    root["schemas"] = doc["schemas"].clone();
    assert!(atip_check::schema::schemas().validator(&root).is_err(), "{}", doc["schemas"]["Broken"]);
    let conforming = Fake::standard();
    for id in TREE {
        let doc = conforming.command_doc(id);
        for name in doc["schemas"].as_object().into_iter().flat_map(|schemas| schemas.keys()) {
            let _compiled = validator(&doc, &format!("#/schemas/{name}"));
        }
    }
}
