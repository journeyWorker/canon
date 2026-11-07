//! `canon-0-38-schema-shape` (scenarios `atip.schemashape.01-12`): an id operand is named for what it is (`subject`, `pack`, `trace`) and the id options for the id they take
//! (`TRACE_ID`, `SKILL_ID`, `EVALUATOR_ID`, `TRAJECTORY_ID`); `x-canon.roles` of a command all four roles use is the string `all`, every other command keeps the array of role
//! names, and the root of the tree says the two forms once; the CHANGELOG has a row for each name that changed. Against the built binary and the documents it declares.
//!
//! The checks that seeded faults must trip are small pure functions in this file (`roles_problems`, `changelog_problems`): each real input passes them, each seeded fault is
//! found. Tests are named by scenario id (`atip_schemashape_05_…` proves `atip.schemashape.05`).

#[path = "support/hermetic.rs"]
mod hermetic;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::LazyLock;

use serde_json::{json, Value};
use tempfile::TempDir;

/// The six commands whose operand was named `id`, with the operand name and value name of 0.38.
const OPERANDS: [(&str, &str, &str); 6] = [
    ("subject new", "subject", "SUBJECT_ID"),
    ("subject show", "subject", "SUBJECT_ID"),
    ("subject move", "subject", "SUBJECT_ID"),
    ("context-pack show", "pack", "PACK_ID"),
    ("context-pack verify", "pack", "PACK_ID"),
    ("trace", "trace", "TRACE_ID"),
];

/// The three options whose value name was `ID`, with the command and the value name of 0.38.
const OPTIONS: [(&str, &str, &str); 3] = [("dispatch begin", "skill", "SKILL_ID"), ("evidence audit", "evaluator", "EVALUATOR_ID"), ("strategy rollback", "contradicting-trajectory", "TRAJECTORY_ID")];

/// The thirteen commands the steps of all four roles use.
const ALL_ROLES: [&str; 13] = ["completion", "context", "divergence list", "finding list", "format", "gate check", "query", "retrieve", "schema", "search", "skills show", "status", "trace"];

/// The commands only a human runs: the empty array of roles.
const HUMAN_ONLY: [&str; 3] = ["brief approve", "divergence accept", "policy approve"];

const ROLES: [&str; 4] = ["planner", "implementer", "reviewer", "orchestrator"];

/// What the root of the tree says about the two forms of `x-canon.roles`.
const ROLES_FORM: &str = "x-canon.roles of a command is the string all (every role) or an array of role names; an empty array is humans only";

// ---------------------------------------------------------------------------
// The calls and the documents.

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).unwrap_or_else(|e| panic!("`{path}` is readable: {e}"))
}

fn canon_in(dir: &Path, args: &[&str]) -> Output {
    hermetic::canon().args(args).current_dir(dir).env("CANON_ACTOR", "schema-shape-test").output().expect("spawn canon")
}

fn document(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("stdout is one JSON document ({e}): {}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)))
}

fn text(output: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr))
}

/// What canon prints for `args` run in a directory that holds no repository.
fn answer(args: &[&str]) -> Value {
    let dir = TempDir::new().expect("a directory");
    let out = canon_in(dir.path(), args);
    assert!(out.status.success(), "{args:?}: {}", text(&out));
    document(&out)
}

/// A repository `canon init` set up, without a policy and without a harness.
fn repo() -> TempDir {
    let dir = TempDir::new().expect("a repository");
    let out = canon_in(dir.path(), &["init", "--repo", ".", "--no-policy", "--no-harness"]);
    assert!(out.status.success(), "{}", text(&out));
    dir
}

/// A repository with a subject `demo`.
fn subject_repo() -> TempDir {
    let dir = repo();
    let out = canon_in(dir.path(), &["subject", "new", "demo", "--domain", "dev", "--title", "Demo", "--json"]);
    assert!(out.status.success(), "{}", text(&out));
    dir
}

/// Every file under `dir` with its content, but the git directory and the derived index (a trace that answers may write that).
fn files(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            let rel = path.strip_prefix(root).expect("under the root").to_path_buf();
            if rel.starts_with(".git") || rel.starts_with(".canon/index") {
                continue;
            }
            if entry.file_type().expect("a file type").is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(rel, std::fs::read(&path).expect("a readable file"));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// The checked-in spec (`docs/cli/canon.cli.json`), which `atip_l0` holds equal to what the generator renders: the tree and every command document, as `canon schema` prints them.
static SPEC: LazyLock<Value> = LazyLock::new(|| serde_json::from_str(&read("docs/cli/canon.cli.json")).expect("the spec is JSON"));

static TREE: LazyLock<&Value> = LazyLock::new(|| &SPEC["tree"]);

static COMMANDS: LazyLock<Vec<&Value>> = LazyLock::new(|| SPEC["commands"].as_array().expect("commands").iter().collect());

fn command_doc(command: &str) -> &'static Value {
    COMMANDS.iter().copied().find(|doc| doc["command"] == command).unwrap_or_else(|| panic!("`{command}` is not a command"))
}

/// The options of a document: an option object, or the name of a tree option.
fn option_named<'a>(doc: &'a Value, name: &str) -> &'a Value {
    doc["options"].as_array().and_then(|options| options.iter().find(|option| option["name"] == name)).unwrap_or_else(|| panic!("`{}` has no --{name}", doc["command"]))
}

/// The arguments of `canon schema <command> --json`.
fn schema_args(command: &str) -> Vec<&str> {
    let mut args = vec!["schema"];
    args.extend(command.split(' '));
    args.push("--json");
    args
}

/// The tree entries that are commands.
fn commands_of(tree: &Value) -> Vec<&Value> {
    tree["commands"].as_array().expect("commands").iter().filter(|entry| entry["kind"] != "group").collect()
}

// ===========================================================================
// atip.schemashape.01 - 02 (F1): the trace operand.

#[test]
fn atip_schemashape_01_the_trace_operand_is_named_for_what_it_takes() {
    let doc = answer(&["schema", "trace", "--json"]);
    let operands = doc["data"]["operands"].as_array().expect("operands");
    assert_eq!(operands.len(), 1, "{operands:?}");
    assert_eq!((operands[0]["name"].as_str(), operands[0]["valueName"].as_str()), (Some("trace"), Some("TRACE_ID")), "{operands:?}");
    let help = operands[0]["help"].as_str().expect("help");
    assert!(help.contains("review id") && help.contains("commit sha") && help.contains("finding key"), "the operand still says the three things it takes: {help}");
    assert_eq!(atip::schema_doc_problems(&doc), Vec::<String>::new());
    let dir = TempDir::new().expect("a directory");
    let help = canon_in(dir.path(), &["trace", "--help"]);
    let help = String::from_utf8_lossy(&help.stdout).into_owned();
    assert!(help.contains("<TRACE_ID>") && !help.contains("<ID>"), "{help}");
}

/// A repository whose evidence is at two shas that start alike, so `abcdef0` is ambiguous.
fn trace_repo() -> TempDir {
    let dir = TempDir::new().expect("a repository");
    let repo = dir.path();
    let yaml = "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\n  change: local\n  subject: local\n  scenario: local\n  review: local\n  divergence: local\n  evidence: local\n  finding: local\nspecs:\n  roots:\n    - id: root\n      root: .canon/specs\nwrite:\n  promote: auto\n";
    std::fs::write(repo.join("canon.yaml"), yaml).expect("canon.yaml");
    std::fs::write(repo.join(".gitignore"), ".canon/index/\n.canon/hot.db*\n").expect("gitignore");
    for args in [&["init", "-q", "-b", "main"][..], &["add", "-A"], &["commit", "-q", "-m", "base"]] {
        let out = Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "gc.auto=0"]).args(args).current_dir(repo).output().expect("git");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
    use canon_store::tier::{RawWrite, Tier};
    let tier = canon_store::git_tier::GitTier::new(repo.join(".canon/ledger"));
    for (n, sha) in ["abcdef0123456789abcdef0123456789abcdef01", "abcdef0aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"].iter().enumerate() {
        let body = json!({"actor": {"agent_id": "t", "role": "implementer"}, "at": "2026-08-01T00:00:00Z", "evidence": {"kind": "test-run", "ref": "x"}, "evidence_sha": sha, "kind": "evidence", "project_id": "root", "run_seq": n, "scenario_id": format!("loop.run.0{}", n + 1), "schema": 1, "verdict": "faithful"});
        tier.write(&RawWrite(canon_model::RawRecord(body))).expect("write the record");
    }
    dir
}

#[test]
fn atip_schemashape_02_a_trace_operand_that_is_refused_names_trace_id_and_is_refused_as_before() {
    let dir = trace_repo();
    let before = files(dir.path());
    let cases = [
        ("abc", "TRACE_OPERAND_INVALID"),
        ("a_1", "TRACE_OPERAND_INVALID"),
        ("not a shape!", "TRACE_OPERAND_INVALID"),
        ("deadbeefdead", "TRACE_NOT_FOUND"),
        ("01ARZ3NDEKTSV4RRFFQ69G5FAV", "TRACE_NOT_FOUND"),
        ("loop-change_9_9", "TRACE_NOT_FOUND"),
        ("abcdef0", "TRACE_CONFLICT"),
    ];
    for (operand, code) in cases {
        let out = canon_in(dir.path(), &["trace", operand, "--json"]);
        assert_eq!(out.status.code(), Some(2), "{operand}: {}", text(&out));
        let error = document(&out)["error"].clone();
        assert_eq!(error["code"], code, "{operand}: {error}");
        assert_eq!(error["field"], "TRACE_ID", "{operand}: {error}");
        let message = error["message"].as_str().expect("a message");
        assert!(message.contains("review id") && message.contains("commit sha") && message.contains("finding key"), "{operand}: the three shapes are still said: {message}");
        if code == "TRACE_OPERAND_INVALID" {
            assert_eq!(error["received"], operand, "the value as given: {error}");
            assert!(message.starts_with("TRACE_ID is none"), "{operand}: the message names the value name: {message}");
        }
    }
    assert_eq!(files(dir.path()), before, "a refusal writes nothing");
}

// ===========================================================================
// atip.schemashape.03 - 04 (F2): the operand names.

#[test]
fn atip_schemashape_03_the_six_operands_are_named_subject_pack_and_trace_and_no_operand_is_named_id() {
    for (command, name, value_name) in OPERANDS {
        let built = command_doc(command);
        let operands = built["operands"].as_array().expect("operands");
        assert_eq!(operands.len(), 1, "`{command}`: {operands:?}");
        assert_eq!((operands[0]["name"].as_str(), operands[0]["valueName"].as_str()), (Some(name), Some(value_name)), "`{command}`");
        assert_eq!(answer(&schema_args(command))["data"], *built, "`{command}`: the binary answers the document of the checked-in spec");
    }
    for doc in COMMANDS.iter() {
        for operand in doc["operands"].as_array().into_iter().flatten() {
            assert_ne!(operand["name"], "id", "`{}` has an operand named id", doc["command"]);
        }
        let named: Vec<&Value> = doc["operands"].as_array().into_iter().flatten().chain(doc["options"].as_array().into_iter().flatten()).collect();
        assert!(named.iter().all(|argument| argument["valueName"] != "ID"), "`{}` has the value name ID", doc["command"]);
    }
    let tree = answer(&["schema", "--json"]);
    assert!(tree["data"]["options"].as_array().expect("options").iter().all(|option| option["valueName"] != "ID" && option["name"] != "id"));
    assert!(!serde_json::to_string(&tree["data"]).expect("serializes").contains("\"name\":\"id\""), "no member named id is left in the tree");
}

#[test]
fn atip_schemashape_04_an_input_document_takes_the_operands_array_as_before_and_never_the_operand_name() {
    // `subject new` and `subject move` are the two of the six commands that take `--input`.
    for command in ["subject new", "subject move"] {
        assert!(command_doc(command).get("input").is_some(), "`{command}` takes --input");
    }
    for command in ["subject show", "context-pack show", "context-pack verify", "trace"] {
        assert!(command_doc(command).get("input").is_none(), "`{command}` takes no --input");
        let dir = repo();
        let mut args: Vec<&str> = command.split(' ').collect();
        args.extend(["--input", r#"{"operands":["demo"]}"#, "--json"]);
        let out = canon_in(dir.path(), &args);
        assert_eq!(out.status.code(), Some(2), "{command}: {}", text(&out));
        let error = document(&out)["error"].clone();
        assert_eq!((error["code"].as_str(), error["field"].as_str()), (Some("USAGE"), Some("--input")), "{command}: {error}");
    }
    // The operands array runs as the flag line does.
    let by_flags = repo();
    let by_input = repo();
    let flags = canon_in(by_flags.path(), &["subject", "new", "alpha", "--domain", "dev", "--title", "Alpha", "--json"]);
    let input = canon_in(by_input.path(), &["subject", "new", "--input", r#"{"operands":["alpha"],"domain":"dev","title":"Alpha"}"#, "--json"]);
    assert!(flags.status.success() && input.status.success(), "{}{}", text(&flags), text(&input));
    let subject = |out: &Output| {
        let data = document(out)["data"].clone();
        (data["subject_id"].clone(), data["domain"].clone(), data["title"].clone(), data["status"].clone())
    };
    assert_eq!(subject(&flags), subject(&input), "the same subject is recorded");
    assert_eq!(subject(&input).0, "alpha");
    let dir = subject_repo();
    let moved = canon_in(dir.path(), &["subject", "move", "--input", r#"{"operands":["demo"],"to":"specced"}"#, "--json"]);
    assert!(moved.status.success(), "{}", text(&moved));
    let shown = document(&canon_in(dir.path(), &["subject", "show", "demo", "--json"]));
    assert_eq!(shown["data"]["status"], "specced", "{shown}");
    // A key named after the operand is an unknown key, as `id` was; nothing is written.
    let before = files(dir.path());
    for (command, key) in [("new", "subject"), ("new", "id"), ("move", "subject"), ("move", "id")] {
        let document_text = if command == "new" { format!(r#"{{"{key}":"other","domain":"dev","title":"Other"}}"#) } else { format!(r#"{{"{key}":"demo","to":"building"}}"#) };
        let out = canon_in(dir.path(), &["subject", command, "--input", &document_text, "--json"]);
        assert_eq!(out.status.code(), Some(2), "{command} {key}: {}", text(&out));
        let error = document(&out)["error"].clone();
        assert_eq!((error["code"].as_str(), error["reason"].as_str(), error["field"].as_str()), (Some("INPUT_INVALID"), Some("UNKNOWN_KEY"), Some(format!("/{key}").as_str())), "{command} {key}: {error}");
        assert!(error["expected"].as_array().expect("expected").iter().any(|name| name == "operands"), "{error}");
    }
    assert_eq!(files(dir.path()), before, "an unknown key writes nothing");
    // The input documents of the two commands have `operands` and no property named after the operand.
    for command in ["subject new", "subject move"] {
        let doc = command_doc(command);
        let arguments = doc["input"]["argumentsRef"].as_str().and_then(|reference| reference.strip_prefix("#/schemas/")).expect("a schema ref");
        let properties = SPEC["schemas"][command]["schemas"][arguments]["properties"].as_object().unwrap_or_else(|| panic!("`{command}` has an arguments schema with properties"));
        assert!(properties.contains_key("operands") && !["subject", "pack", "trace", "id"].iter().any(|name| properties.contains_key(*name)), "`{command}`: {:?}", properties.keys().collect::<Vec<_>>());
    }
}

// ===========================================================================
// atip.schemashape.05 - 06 (F3): the value names of three options.

#[test]
fn atip_schemashape_05_the_three_options_that_took_a_bare_id_name_what_they_take() {
    for (command, option, value_name) in OPTIONS {
        let built = option_named(command_doc(command), option);
        assert_eq!(built["valueName"], value_name, "`{command} --{option}`");
        assert!(built.get("valueType").is_none(), "the value is a string as before: {built}");
        assert_eq!(option_named(&answer(&schema_args(command))["data"], option), built, "`{command}`: the binary answers the document of the checked-in spec");
        let mut help: Vec<&str> = command.split(' ').collect();
        help.push("--help");
        let dir = TempDir::new().expect("a directory");
        let shown = String::from_utf8_lossy(&canon_in(dir.path(), &help).stdout).into_owned();
        assert!(shown.contains(&format!("--{option} <{value_name}>")) && !shown.contains(&format!("--{option} <ID>")), "{command}: {shown}");
    }
    // Option names, whether the option is required and the domain are 0.37's.
    assert_eq!(option_named(command_doc("evidence audit"), "evaluator").get("required"), Some(&json!(true)));
    assert_eq!(option_named(command_doc("evidence audit"), "evaluator")["domain"], json!({ "pattern": "^\\P{White_Space}([\\P{White_Space}\\p{Zs}\\t]*\\P{White_Space})?$" }));
    for (command, option) in [("dispatch begin", "skill"), ("strategy rollback", "contradicting-trajectory")] {
        let built = option_named(command_doc(command), option);
        assert!(built.get("required").is_none() && built.get("domain").is_none(), "{built}");
    }
    let tree = answer(&["schema", "--json"]);
    assert!(!serde_json::to_string(&tree).expect("serializes").contains("\"valueName\":\"ID\""), "no value name ID is left in the tree");
}

#[test]
fn atip_schemashape_06_a_value_the_three_options_refuse_is_refused_with_the_code_and_the_domain_of_0_37() {
    let dir = repo();
    let before = files(dir.path());
    let digest = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let audit = canon_in(dir.path(), &["evidence", "audit", "auth.login.01", "--ref", "r", "--evaluator", " a", "--evaluator-version", "1", "--evaluator-digest", digest, "--audit-result", "confirmed", "--role", "implementer", "--json"]);
    assert_eq!(audit.status.code(), Some(2), "{}", text(&audit));
    let error = document(&audit)["error"].clone();
    assert_eq!((error["code"].as_str(), error["field"].as_str(), error["received"].as_str()), (Some("EVIDENCE_ARGUMENT_INVALID"), Some("--evaluator"), Some(" a")), "{error}");
    let begin = canon_in(dir.path(), &["dispatch", "begin", "--regime", "implementer/canon/core/abc123", "--skill-digest", digest, "--json"]);
    assert_eq!(begin.status.code(), Some(2), "{}", text(&begin));
    let error = document(&begin)["error"].clone();
    assert_eq!((error["code"].as_str(), error["field"].as_str()), (Some("DISPATCH_ARGUMENT_INVALID"), Some("--skill")), "{error}");
    let signature = dir.path().join("approval.sig");
    std::fs::write(&signature, "signature").expect("a signature file");
    let rollback = canon_in(dir.path(), &["strategy", "rollback", "01ARZ3NDEKTSV4RRFFQ69G5FAV", "--reason", "r", "--actor", "a", "--signature-file", "approval.sig", "--approval-at", "2030-01-01T10:00:00Z", "--contradicting-trajectory", "nosuch", "--json"]);
    assert_eq!(rollback.status.code(), Some(2), "{}", text(&rollback));
    assert_eq!(document(&rollback)["error"]["code"], "STRATEGY_NOT_FOUND");
    std::fs::remove_file(&signature).expect("the signature file goes");
    assert_eq!(files(dir.path()), before, "nothing is written");
}

// ===========================================================================
// atip.schemashape.07 - 10 (F4): the roles.

/// The commands of the 0.37.0 tree that later releases removed (0.39: `canon ingest sessions`).
const REMOVED_SINCE_0_37: [&str; 1] = ["ingest sessions"];

/// The tree of 0.37.0 as that binary printed `x-canon.roles`: `(command, roles)` for each of its 78 commands.
static ROLES_0_37: LazyLock<BTreeMap<String, Vec<String>>> = LazyLock::new(|| {
    let fixture: Value = serde_json::from_str(&read("crates/canon-cli/tests/atip/roles_0_37.json")).expect("the fixture is JSON");
    fixture["commands"].as_object().expect("commands").iter().map(|(command, roles)| (command.clone(), roles.as_array().expect("an array").iter().map(|role| role.as_str().expect("a name").to_string()).collect())).collect()
});

/// What is wrong with the roles of the command entries of a tree: a missing member (it means nothing), a value that is neither `all` nor an array of known role names,
/// an array that says all four roles, a name twice, or an empty array on a command that is not one of `human_only`.
fn roles_problems(commands: &[&Value], human_only: &[&str]) -> Vec<String> {
    let mut problems = Vec::new();
    for entry in commands {
        let command = entry["command"].as_str().unwrap_or_default();
        let Some(roles) = entry.get("x-canon").and_then(|declared| declared.get("roles")) else {
            problems.push(format!("`{command}` has no roles member"));
            continue;
        };
        match roles {
            Value::String(word) if word == "all" => {}
            Value::String(word) => problems.push(format!("`{command}` roles is the string `{word}`, not `all`")),
            Value::Array(names) => {
                let names: Vec<&str> = names.iter().map(|name| name.as_str().unwrap_or("<not a string>")).collect();
                if let Some(unknown) = names.iter().find(|name| !ROLES.contains(name)) {
                    problems.push(format!("`{command}` roles names `{unknown}`, which is no role"));
                }
                if names.iter().collect::<BTreeSet<_>>().len() != names.len() {
                    problems.push(format!("`{command}` roles names a role twice"));
                }
                if ROLES.iter().all(|role| names.contains(role)) {
                    problems.push(format!("`{command}` roles names all four roles: say `all`"));
                }
                if names.is_empty() && !human_only.contains(&command) {
                    problems.push(format!("`{command}` roles is empty and is not a command only a human runs"));
                }
            }
            other => problems.push(format!("`{command}` roles is {other}, neither `all` nor an array")),
        }
    }
    problems
}

#[test]
fn atip_schemashape_07_a_command_all_four_roles_use_says_all_and_every_other_command_keeps_the_array() {
    let tree = answer(&["schema", "--json"])["data"].clone();
    assert_eq!(&tree, *TREE, "the binary answers the tree of the checked-in spec");
    let commands = commands_of(&tree);
    let problems = roles_problems(&commands, &HUMAN_ONLY);
    assert!(problems.is_empty(), "{problems:#?}");
    let says_all: Vec<&str> = commands.iter().filter(|entry| entry["x-canon"]["roles"] == "all").map(|entry| entry["command"].as_str().expect("command")).collect();
    assert_eq!(says_all, ALL_ROLES, "the thirteen commands, and only they");
    let empty: Vec<&str> = commands.iter().filter(|entry| entry["x-canon"]["roles"] == json!([])).map(|entry| entry["command"].as_str().expect("command")).collect();
    assert_eq!(empty, HUMAN_ONLY, "the human-only commands keep the empty array");
    // Each command document says what the tree says.
    for entry in &commands {
        let doc = command_doc(entry["command"].as_str().expect("command"));
        assert_eq!(doc["x-canon"], entry["x-canon"], "`{}`", entry["command"]);
    }
    for command in ALL_ROLES {
        assert_eq!(answer(&schema_args(command))["data"]["x-canon"], json!({ "roles": "all" }), "`{command}`");
    }
    // Every command of 0.37 has the roles it had: the four names became `all`, the other arrays are as they were.
    assert_eq!(ROLES_0_37.len(), 78);
    for (command, before) in ROLES_0_37.iter() {
        if REMOVED_SINCE_0_37.contains(&command.as_str()) {
            assert!(!commands.iter().any(|entry| entry["command"] == command.as_str()), "`{command}` was removed and is not listed");
            continue;
        }
        let entry = commands.iter().find(|entry| entry["command"] == command.as_str()).unwrap_or_else(|| panic!("`{command}` is still listed"));
        let now = &entry["x-canon"]["roles"];
        let is_all = ROLES.iter().all(|role| before.iter().any(|name| name == role));
        if is_all {
            assert_eq!(now, "all", "`{command}` was the four names");
        } else {
            assert_eq!(*now, json!(before), "`{command}` keeps its array");
        }
    }
    assert_eq!(ROLES_0_37.values().filter(|roles| roles.len() == 4).count(), 13);
    // The bytes: 13 times 46 fewer for `all`, and what the declaration at the root adds.
    let (saved, declared, net) = measured_bytes(&answer(&["schema", "--json"]));
    assert_eq!((saved, declared), (13 * 46, 141), "598 bytes fewer, 141 bytes more");
    assert_eq!(net, saved - declared);
}

/// The bytes the tree answer saves against the same tree with every `all` written as the four names and no root declaration: `(saved by all, added by the declaration, net)`.
fn measured_bytes(answer: &Value) -> (usize, usize, usize) {
    let size = |value: &Value| serde_json::to_string(value).expect("serializes").len();
    let declared = size(&answer["data"]["x-canon"]) + ",\"x-canon\":".len();
    let mut without_declaration = answer.clone();
    without_declaration["data"].as_object_mut().expect("data").remove("x-canon");
    let mut legacy = without_declaration.clone();
    for entry in legacy["data"]["commands"].as_array_mut().expect("commands") {
        if entry["x-canon"]["roles"] == "all" {
            entry["x-canon"]["roles"] = json!(ROLES);
        }
    }
    (size(&legacy) - size(&without_declaration), declared, size(&legacy) - size(answer))
}

#[test]
fn atip_schemashape_08_the_listings_of_a_role_are_the_commands_it_had_and_the_guides_hold_them() {
    let dir = repo();
    let guide_of = |role: &str| read(&format!("canon/skills/reference/role-{role}.md"));
    for role in ROLES {
        let listed = |args: &[&str]| -> BTreeSet<String> {
            let out = canon_in(dir.path(), args);
            assert!(out.status.success(), "{args:?}: {}", text(&out));
            let data = &document(&out)["data"];
            data["commands"].as_array().expect("commands").iter().filter(|entry| entry["kind"] != "group").map(|entry| entry["command"].as_str().expect("command").to_string()).collect()
        };
        let by_schema = listed(&["schema", "--as", role, "--json"]);
        // What the metadata table says: the commands whose rows name the role.
        let by_table: BTreeSet<String> = canon_cli::schema::META.iter().filter(|meta| meta.roles.iter().any(|name| name.as_str() == role)).map(|meta| meta.path.to_string()).collect();
        assert_eq!(by_schema, by_table, "schema --as {role}");
        // What 0.37 listed for the role: every command of that tree whose roles array held the role.
        let before: BTreeSet<String> = ROLES_0_37.iter().filter(|(command, roles)| !REMOVED_SINCE_0_37.contains(&command.as_str()) && roles.iter().any(|name| name == role)).map(|(command, _)| command.clone()).collect();
        assert!(before.is_subset(&by_schema), "{role}: a command 0.37 listed is gone: {:?}", before.difference(&by_schema).collect::<Vec<_>>());
        assert!(by_schema.difference(&before).all(|command| !ROLES_0_37.contains_key(command)), "{role}: a command 0.37 did not list for the role is listed: {:?}", by_schema.difference(&before).collect::<Vec<_>>());
        assert!(ALL_ROLES.iter().all(|command| by_schema.contains(*command)), "{role} lists the thirteen commands of all four roles");
        assert_eq!(listed(&["context", "--as", role, "--json"]), by_schema, "context --as {role} lists what schema --as {role} lists");
        let status = canon_in(dir.path(), &["status", "--as", role, "--json"]);
        assert!(status.status.success(), "{}", text(&status));
        // The command block of the role's guide holds each command of the role.
        let guide = guide_of(role);
        for command in &by_schema {
            assert!(guide.lines().any(|line| line.strip_prefix(&format!("usage: canon {command}")).is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))), "the {role} guide has no usage line of `{command}`");
        }
    }
}

#[test]
fn atip_schemashape_09_a_roles_value_that_is_not_all_or_an_array_of_role_names_or_a_missing_member_fails_the_check() {
    let tree = answer(&["schema", "--json"])["data"].clone();
    let commands = commands_of(&tree);
    assert!(roles_problems(&commands, &HUMAN_ONLY).is_empty());
    let mutate = |command: &str, roles: Option<Value>| -> Vec<String> {
        let mut copy = tree.clone();
        let entry = copy["commands"].as_array_mut().expect("commands").iter_mut().find(|entry| entry["command"] == command).expect("the command");
        match roles {
            Some(roles) => entry["x-canon"]["roles"] = roles,
            None => {
                entry["x-canon"].as_object_mut().expect("x-canon").remove("roles");
            }
        }
        roles_problems(&commands_of(&copy), &HUMAN_ONLY)
    };
    let seeded: [(&str, Option<Value>, &str); 8] = [
        ("status", Some(json!("any")), "not `all`"),
        ("status", Some(json!("ALL")), "not `all`"),
        ("status", Some(json!({})), "neither `all` nor an array"),
        ("status", Some(json!(4)), "neither `all` nor an array"),
        ("evidence add", Some(json!(["implementer", "janitor"])), "which is no role"),
        ("evidence add", Some(json!(["planner", "implementer", "reviewer", "orchestrator"])), "say `all`"),
        ("evidence add", Some(json!([])), "not a command only a human runs"),
        ("evidence add", None, "has no roles member"),
    ];
    for (command, roles, expected) in seeded {
        let problems = mutate(command, roles.clone());
        assert!(problems.iter().any(|problem| problem.contains(command) && problem.contains(expected)), "{command} {roles:?}: {problems:?}");
    }
    let twice = mutate("evidence add", Some(json!(["implementer", "implementer"])));
    assert!(twice.iter().any(|problem| problem.contains("a role twice")), "{twice:?}");
    // A member that is missing is not read as `all` and not as humans only: the check names it, whichever the command.
    for command in ["status", "brief approve"] {
        assert!(mutate(command, None).iter().any(|problem| problem.contains("has no roles member")), "{command}");
    }
}

#[test]
fn atip_schemashape_10_the_root_of_the_tree_says_the_two_forms_once_and_the_documents_say_them() {
    let tree = answer(&["schema", "--json"])["data"].clone();
    assert_eq!(tree["x-canon"], json!({ "rolesForm": ROLES_FORM }), "the root says the two forms and nothing else");
    assert!(ROLES_FORM.contains("all") && ROLES_FORM.contains("array of role names") && ROLES_FORM.contains("humans only"));
    let filtered = answer(&["schema", "--as", "reviewer", "--json"])["data"].clone();
    assert_eq!(filtered["x-canon"], json!({ "as": "reviewer", "rolesForm": ROLES_FORM }), "a filtered tree says them beside the role");
    // No group and no command document says it again.
    let group = answer(&["schema", "subject", "--json"])["data"].clone();
    assert!(group.get("x-canon").is_none(), "a group document does not say it: {group}");
    assert!(!serde_json::to_string(&group).expect("serializes").contains("rolesForm"));
    for doc in COMMANDS.iter() {
        assert!(!serde_json::to_string(doc).expect("serializes").contains("rolesForm"), "`{}` says the two forms again", doc["command"]);
    }
    // The guide and the website say the same two forms.
    for (path, needles) in [
        ("canon/skills/SKILL.src.md", ["`all`", "rolesForm", "array"]),
        ("packages/website/src/content/docs/cli.mdx", ["`\"all\"`", "rolesForm", "array of role names"]),
        ("packages/website/src/content/docs/ko/cli.mdx", ["`\"all\"`", "rolesForm", "역할 이름 배열"]),
    ] {
        let page = read(path);
        for needle in needles {
            assert!(page.contains(needle), "{path} does not say {needle}");
        }
    }
    // The checked-in spec has the same tree: the declaration, and `all` where the four names were.
    assert_eq!(TREE["x-canon"], json!({ "rolesForm": ROLES_FORM }));
    let problems = roles_problems(&commands_of(&TREE), &HUMAN_ONLY);
    assert!(problems.is_empty(), "{problems:#?}");
    for doc in COMMANDS.iter() {
        let entry = TREE["commands"].as_array().expect("commands").iter().find(|entry| entry["command"] == doc["command"]).expect("the tree lists it");
        assert_eq!(doc["x-canon"], entry["x-canon"], "`{}`", doc["command"]);
    }
}

// ===========================================================================
// atip.schemashape.11 - 12 (F5): the CHANGELOG.

/// The rows of the table of the `Unreleased` Breaking entry of this change, split into cells: `(first cell, 0.37 cell, 0.38 cell)` for the rows that name a command.
fn breaking_rows(changelog: &str) -> Vec<Vec<String>> {
    let unreleased = changelog.split("\n## ").find(|part| part.starts_with("0.38.0")).unwrap_or_default();
    let breaking = unreleased.split("\n### ").find(|part| part.starts_with("Breaking")).unwrap_or_default();
    let entry = breaking.split("\n**").find(|part| part.contains("`canon-0-38-schema-shape`")).unwrap_or_default();
    entry.lines().filter(|line| line.starts_with("| `")).map(|line| line.trim().trim_matches('|').split('|').map(|cell| cell.trim().to_string()).collect::<Vec<String>>()).collect()
}

/// The first cell, and what the cells of 0.37 and 0.38 must hold, of each row the table needs.
fn needed_rows() -> Vec<(String, String, String)> {
    let mut rows = Vec::new();
    for (command, _, value_name) in OPERANDS {
        let name = OPERANDS.iter().find(|(other, ..)| *other == command).map(|(_, name, _)| *name).expect("a name");
        rows.push((format!("`{command}` operand"), "named `id`".to_string(), format!("named `{name}`, value name `{value_name}`")));
    }
    for (command, option, value_name) in OPTIONS {
        rows.push((format!("`{command} --{option}`"), "value name `ID`".to_string(), format!("value name `{value_name}`")));
    }
    for command in ALL_ROLES {
        rows.push((format!("`{command}` roles"), "\"planner\"".to_string(), "`\"all\"`".to_string()));
    }
    rows
}

/// What the CHANGELOG lacks: a row for each name that changed with what 0.37 and 0.38 say, the header, how a tool reads `all` and the bytes saved and added.
fn changelog_problems(changelog: &str, saved: usize, declared: usize) -> Vec<String> {
    let rows = breaking_rows(changelog);
    let mut problems = Vec::new();
    for (first, before, after) in needed_rows() {
        match rows.iter().find(|row| row.first().is_some_and(|cell| *cell == first)) {
            None => problems.push(format!("no row of the table names {first}")),
            Some(row) if row.len() != 3 || row.iter().any(String::is_empty) => problems.push(format!("a row lacks its 0.37 answer or its 0.38 answer: {row:?}")),
            Some(row) => {
                if !row[1].contains(&before) {
                    problems.push(format!("the 0.37 cell of {first} does not say {before}: {}", row[1]));
                }
                if !row[2].contains(&after) {
                    problems.push(format!("the 0.38 cell of {first} does not say {after}: {}", row[2]));
                }
            }
        }
    }
    let entry = changelog.split("\n## ").find(|part| part.starts_with("0.38.0")).unwrap_or_default().split("\n**").find(|part| part.contains("`canon-0-38-schema-shape`")).unwrap_or_default();
    if !entry.contains("| name | 0.37 | 0.38 |") {
        problems.push("the table has no header of its three columns".to_string());
    }
    if !entry.contains("reads `\"all\"` as the four roles") || !entry.contains("breaks") {
        problems.push("the entry does not say how a tool reads all and who breaks".to_string());
    }
    for needed in [format!("{saved} bytes fewer"), format!("{declared} bytes more")] {
        if !entry.contains(&needed) {
            problems.push(format!("the entry does not say `{needed}`"));
        }
    }
    problems
}

#[test]
fn atip_schemashape_11_the_changelog_has_a_row_for_each_name_that_changed_and_the_bytes() {
    let changelog = read("CHANGELOG.md");
    let (saved, declared, _) = measured_bytes(&answer(&["schema", "--json"]));
    let problems = changelog_problems(&changelog, saved, declared);
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
    assert_eq!(breaking_rows(&changelog).len(), 6 + 3 + 13, "one row per name: six operands, three options, thirteen commands");
    let entry = changelog.split("\n**").find(|part| part.contains("`canon-0-38-schema-shape`")).expect("the entry");
    assert!(entry.contains("25,709") && entry.contains("25,252"), "the entry says the tree of 0.37.0 and the same tree with the change");
}

#[test]
fn atip_schemashape_12_the_check_of_the_changelog_fails_for_a_missing_row_an_empty_cell_a_missing_header_and_missing_bytes() {
    let changelog = read("CHANGELOG.md");
    let (saved, declared, _) = measured_bytes(&answer(&["schema", "--json"]));
    assert!(changelog_problems(&changelog, saved, declared).is_empty());
    let lines: Vec<&str> = changelog.lines().collect();
    // One row removed for each kind of row.
    for first in ["| `subject new` operand |", "| `trace` operand |", "| `evidence audit --evaluator` |", "| `completion` roles |", "| `trace` roles |"] {
        let at = lines.iter().position(|line| line.starts_with(first)).unwrap_or_else(|| panic!("a row begins {first}"));
        let without: String = lines.iter().enumerate().filter(|(index, _)| *index != at).map(|(_, line)| *line).collect::<Vec<_>>().join("\n");
        let problems = changelog_problems(&without, saved, declared);
        assert!(problems.iter().any(|problem| problem.contains("no row of the table names")), "{first}: {problems:?}");
    }
    let at = lines.iter().position(|line| line.starts_with("| `status` roles |")).expect("the row of status");
    let blank: String = lines.iter().enumerate().map(|(index, line)| if index == at { "| `status` roles | `[\"planner\"]` |  |".to_string() } else { (*line).to_string() }).collect::<Vec<_>>().join("\n");
    assert!(changelog_problems(&blank, saved, declared).iter().any(|problem| problem.contains("lacks its 0.37 answer")), "{:?}", changelog_problems(&blank, saved, declared));
    let wrong: String = changelog.replace("| `status` roles | `x-canon.roles` is `[\"planner\",\"implementer\",\"reviewer\",\"orchestrator\"]` | `x-canon.roles` is `\"all\"` |", "| `status` roles | `x-canon.roles` is `[\"planner\"]` | `x-canon.roles` is `[\"planner\"]` |");
    assert!(changelog_problems(&wrong, saved, declared).iter().any(|problem| problem.contains("the 0.38 cell of `status` roles")), "{:?}", changelog_problems(&wrong, saved, declared));
    let without_header = changelog.replace("| name | 0.37 | 0.38 |", "| a | b | c |");
    assert!(changelog_problems(&without_header, saved, declared).iter().any(|problem| problem.contains("no header")));
    let without_bytes = changelog.replace(&format!("{saved} bytes fewer"), "bytes fewer");
    assert!(changelog_problems(&without_bytes, saved, declared).iter().any(|problem| problem.contains("bytes fewer")));
    let without_reading = changelog.replace("reads `\"all\"` as the four roles", "reads it");
    assert!(changelog_problems(&without_reading, saved, declared).iter().any(|problem| problem.contains("how a tool reads all")));
}
