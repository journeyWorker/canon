//! The generated command spec (`docs/cli/canon.cli.json`, 0.28 D2) and the
//! things built from it.
//!
//! T4: the spec regenerated in memory equals the checked-in file byte for
//! byte. `CANON_BLESS_CLI_SPEC=1 cargo test -p canon-cli --test cli_spec`
//! rewrites it. The other tests hold the spec to the tree it describes and
//! the `canon completion` leaf to its contract.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use canon_cli::cli;
use canon_cli::spec::{self, INTERNAL_ENV, SPEC_PATH};
use serde_json::Value;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn canon(args: &[&str]) -> Output {
    let dir = tempfile::tempdir().expect("tempdir");
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(dir.path()).output().expect("spawn canon binary")
}

#[test]
fn the_checked_in_spec_is_the_generated_one() {
    let generated = spec::render();
    let checked_in = repo_root().join(SPEC_PATH);
    if std::env::var_os("CANON_BLESS_CLI_SPEC").is_some() {
        fs::create_dir_all(checked_in.parent().expect("the spec has a directory")).expect("create docs/cli");
        fs::write(&checked_in, &generated).expect("write the spec");
        return;
    }
    let on_disk = fs::read_to_string(&checked_in).unwrap_or_else(|e| panic!("read {SPEC_PATH}: {e}; run CANON_BLESS_CLI_SPEC=1 cargo test -p canon-cli --test cli_spec"));
    assert!(on_disk == generated, "{SPEC_PATH} is out of date; regenerate it with CANON_BLESS_CLI_SPEC=1 cargo test -p canon-cli --test cli_spec and commit it");
}

#[test]
fn two_generations_are_byte_identical() {
    assert_eq!(spec::render(), spec::render());
    assert!(spec::render().ends_with("}\n"));
}

#[test]
fn every_node_of_the_clap_tree_is_in_the_spec_and_the_reverse() {
    let doc = spec::generate();
    let mut spec_commands: Vec<&str> = doc["commands"].as_array().expect("commands").iter().map(|n| n["command"].as_str().expect("command")).collect();
    let mut tree_commands = Vec::new();
    fn walk(cmd: &clap::Command, prefix: &mut Vec<String>, out: &mut Vec<String>) {
        for sub in cmd.get_subcommands().filter(|c| c.get_name() != "help") {
            prefix.push(sub.get_name().to_string());
            out.push(prefix.join(" "));
            walk(sub, prefix, out);
            prefix.pop();
        }
    }
    walk(&cli::command(), &mut Vec::new(), &mut tree_commands);
    tree_commands.sort();
    spec_commands.sort_unstable();
    assert_eq!(spec_commands, tree_commands.iter().map(String::as_str).collect::<Vec<_>>());
}

#[test]
fn a_node_is_a_leaf_xor_a_group_and_a_leaf_says_everything_an_agent_needs() {
    let doc = spec::generate();
    let nodes = doc["commands"].as_array().expect("commands");
    let (mut leaves, mut groups) = (0, 0);
    for node in nodes {
        let command = node["command"].as_str().expect("command");
        match node["nodeKind"].as_str().expect("nodeKind") {
            "group" => {
                groups += 1;
                assert!(!node["children"].as_array().expect("a group lists its children").is_empty(), "group `{command}` has no children");
                assert!(node.get("effect").is_none(), "group `{command}` carries leaf fields");
            }
            "leaf" => {
                leaves += 1;
                for key in ["effect", "roles", "operands", "options", "requiredOneOf", "exits", "errorCodes", "outputs", "inputs", "examples"] {
                    assert!(node.get(key).is_some(), "leaf `{command}` has no `{key}`");
                }
                assert!(!node["outputs"].as_array().expect("outputs").is_empty(), "leaf `{command}` declares no output");
                assert!(!node["examples"].as_array().expect("examples").is_empty(), "leaf `{command}` has no example");
                assert!(!node["exits"].as_array().expect("exits").is_empty(), "leaf `{command}` has no exits");
                for operand in node["operands"].as_array().expect("operands") {
                    assert!(operand["description"].as_str().is_some_and(|d| !d.is_empty()), "`{command}`: operand {} has no description", operand["name"]);
                }
                // A writing leaf declares how a repeat behaves; a read only where its table row does (the approval payloads).
                let declaring_reads = ["evidence approval-payload", "strategy approval-payload"];
                if node["effect"] == "read" {
                    assert_eq!(node.get("idempotency").is_some(), declaring_reads.contains(&command), "read leaf `{command}`: unexpected idempotency declaration");
                } else {
                    assert!(node.get("idempotency").is_some(), "writing leaf `{command}` declares no idempotency");
                }
                if let Some(idempotency) = node.get("idempotency") {
                    assert!(["replay-safe", "content-keyed", "appends", "once"].contains(&idempotency.as_str().expect("a word")), "`{command}`: {idempotency}");
                }
                for output in node["outputs"].as_array().expect("outputs") {
                    let kind = output["kind"].as_str().expect("output kind");
                    assert!(["report", "artifact", "stream"].contains(&kind), "`{command}`: output kind {kind}");
                    assert_eq!(output.get("mediaType").is_some(), kind == "artifact", "`{command}`: only an artifact has a media type");
                }
            }
            other => panic!("`{command}` has node kind {other}"),
        }
    }
    assert!(leaves > 60 && groups > 20, "{leaves} leaves, {groups} groups");
}

#[test]
fn every_leaf_has_one_facts_row_and_every_row_a_leaf() {
    let problems = canon_cli::spec_facts::check_facts(&cli::command());
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn an_option_says_what_it_conflicts_with_and_requires() {
    let doc = spec::generate();
    let option = |command: &str, name: &str| -> Value {
        let node = doc["commands"].as_array().expect("commands").iter().find(|n| n["command"] == command).unwrap_or_else(|| panic!("no `{command}`"));
        node["options"].as_array().expect("options").iter().find(|o| o["name"] == name).unwrap_or_else(|| panic!("no --{name} on `{command}`")).clone()
    };
    // `scenario new --change` requires `--flow` (a clap `requires`, which clap does not expose).
    assert_eq!(option("scenario new", "change")["requires"], serde_json::json!(["--flow"]));
    // `--history` conflicts `--plugin` on `query`, whichever side declared it.
    assert_eq!(option("query", "history")["conflictsWith"], serde_json::json!(["--plugin"]));
    assert_eq!(option("query", "plugin")["conflictsWith"], serde_json::json!(["--history"]));
    assert_eq!(option("query", "kind")["env"], Value::Null);
    assert_eq!(option("feature new", "actor")["env"], "CANON_ACTOR");
}

/// An embedded schema is a document of its own: it names its base URI and its
/// internal references resolve against it, so a consumer can validate with it.
#[test]
fn every_embedded_schema_is_a_self_contained_json_schema() {
    let doc = spec::generate();
    let mut checked = 0;
    for group in ["schemas"] {
        for (name, schema) in doc[group].as_object().unwrap_or_else(|| panic!("`{group}` is an object")) {
            let id = schema["$id"].as_str().unwrap_or_else(|| panic!("{group}.{name} has no $id"));
            assert!(id.starts_with("urn:canon:"), "{group}.{name}: {id}");
            jsonschema::validator_for(schema).unwrap_or_else(|e| panic!("{group}.{name} is not a usable JSON Schema: {e}"));
            checked += 1;
        }
    }
    assert!(checked >= 2, "{checked} schemas");
}

#[test]
fn an_output_that_names_a_schema_points_at_one() {
    let doc = spec::generate();
    for node in doc["commands"].as_array().expect("commands") {
        for output in node["outputs"].as_array().into_iter().flatten() {
            let report = output["report"].as_str().expect("report name");
            match output["schemaRef"].as_str() {
                Some(reference) => {
                    assert_eq!(reference, format!("#/schemas/{report}"));
                    assert!(doc["schemas"].get(report).is_some(), "`{}`: {reference} names no schema", node["command"]);
                    assert!(output["schemaVersion"].is_string(), "`{}`: a schema without a version", node["command"]);
                }
                None => assert!(doc["schemas"].get(report).is_none(), "`{}`: the schema of {report} exists but the output does not point at it", node["command"]),
            }
        }
    }
}

#[test]
fn the_spec_carries_what_d2_promises() {
    let doc = spec::generate();
    for key in ["_doNotEdit", "specVersion", "cli", "globalOptions", "envVars", "exitTable", "contract", "errors", "commands", "schemas", "records"] {
        assert!(doc.get(key).is_some(), "the spec has no `{key}`");
    }
    assert_eq!(doc["cli"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(doc["contract"], "atip/1", "the envelope is the contract's, not a canon schema");
    assert!(doc.get("envelopes").is_none());
    let env_names: Vec<&str> = doc["envVars"].as_array().expect("envVars").iter().map(|v| v["name"].as_str().expect("name")).collect();
    for name in ["CANON_ROLE", "CANON_ACTOR", "CANON_SKILLS_SOURCE"] {
        assert!(env_names.contains(&name), "{name} is not in the spec");
    }
    assert!(doc["globalOptions"].as_array().expect("globalOptions").iter().any(|o| o["name"] == "as"));
    let codes: Vec<u64> = doc["exitTable"].as_array().expect("exitTable").iter().map(|e| e["code"].as_u64().expect("a code")).collect();
    assert_eq!(codes, [0, 1, 2]);
    let registry: Vec<&str> = canon_cli::cli_error::all_codes().iter().map(|info| info.code).collect();
    let listed: Vec<&str> = doc["errors"].as_array().expect("errors").iter().map(|e| e["code"].as_str().expect("code")).collect();
    assert_eq!(listed, registry);
    for error in doc["errors"].as_array().expect("errors") {
        assert!(error["exit"].is_number() && error["retryable"].is_boolean(), "{error}");
    }
    let kinds: Vec<&str> = doc["records"]["kinds"].as_array().expect("kinds").iter().map(|k| k["kind"].as_str().expect("kind")).collect();
    assert_eq!(kinds, canon_model::envelope::RecordKind::ALL.iter().map(|k| k.as_str()).collect::<Vec<_>>());
    for entry in doc["records"]["kinds"].as_array().expect("kinds").iter().chain(doc["records"]["families"].as_array().expect("families")) {
        let schema = entry["schema"].as_str().expect("schema link");
        assert!(repo_root().join(schema).is_file(), "{schema} does not exist");
    }
}

/// T5 (env half): every `CANON_*` variable the crates' sources name is in the
/// spec's list or in the internal allowlist, and every spec variable is read.
#[test]
fn every_env_var_the_sources_read_is_in_the_spec_or_the_internal_allowlist() {
    let mut read = BTreeSet::new();
    let crates = repo_root().join("crates");
    let mut files = Vec::new();
    fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).expect("read dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                collect(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    for krate in fs::read_dir(&crates).expect("crates dir") {
        let krate = krate.expect("crate").path();
        collect(&krate.join("src"), &mut files);
        if krate.join("build.rs").is_file() {
            files.push(krate.join("build.rs"));
        }
    }
    for file in files {
        let text = fs::read_to_string(&file).expect("read source");
        // Test modules name variables that are never read for real.
        let live = text.split("#[cfg(test)]").next().expect("split yields a piece");
        for (i, _) in live.match_indices("\"CANON_") {
            let rest = &live[i + 1..];
            let name: String = rest.chars().take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_').collect();
            if rest[name.len()..].starts_with('"') {
                read.insert(name);
            }
        }
    }
    let documented: BTreeSet<String> = spec::env_var_names().map(str::to_string).collect();
    let internal: BTreeSet<String> = INTERNAL_ENV.iter().map(std::string::ToString::to_string).collect();
    assert!(documented.is_disjoint(&internal), "a variable is both user-facing and internal: {:?}", documented.intersection(&internal).collect::<Vec<_>>());
    let unaccounted: Vec<&String> = read.iter().filter(|v| !documented.contains(*v) && !internal.contains(*v)).collect();
    assert!(unaccounted.is_empty(), "read under crates/ but neither in the spec nor internal: {unaccounted:?}");
    let unread: Vec<&String> = documented.iter().chain(internal.iter()).filter(|v| !read.contains(*v)).collect();
    assert!(unread.is_empty(), "named in the spec or the allowlist but never read: {unread:?}");
}

#[test]
fn completion_writes_a_script_for_each_shell() {
    for shell in ["bash", "zsh", "fish"] {
        let out = canon(&["completion", shell]);
        assert!(out.status.success(), "{shell}: {}", String::from_utf8_lossy(&out.stderr));
        let script = String::from_utf8(out.stdout).expect("a script is text");
        assert!(script.contains("canon"), "{shell}: the script never names the command");
        assert!(script.contains("completion"), "{shell}: the script does not complete its own command");
        assert!(script.len() > 1000, "{shell}: a {}-byte script", script.len());
    }
}

#[test]
fn completion_json_is_the_script_in_an_envelope() {
    for shell in ["bash", "zsh", "fish"] {
        let plain = canon(&["completion", shell]);
        let json = canon(&["completion", shell, "--json"]);
        assert!(json.status.success());
        assert!(json.stderr.is_empty(), "{shell}: JSON mode writes nothing on stderr: {}", String::from_utf8_lossy(&json.stderr));
        let body: Value = serde_json::from_slice(&json.stdout).expect("stdout is one JSON document");
        assert_eq!(body["meta"]["command"], "completion");
        assert_eq!(body["data"]["shell"], shell);
        assert_eq!(body["data"]["script"].as_str().expect("script"), String::from_utf8(plain.stdout).expect("text"));
    }
}

#[test]
fn completion_refuses_a_shell_it_does_not_know_and_needs_no_repo() {
    assert_eq!(canon(&["completion", "powershell"]).status.code(), Some(2));
    assert_eq!(canon(&["completion"]).status.code(), Some(2));
    // `canon` is run from an empty temp directory above: no canon.yaml anywhere near it.
    assert!(canon(&["completion", "zsh", "--as", "reviewer"]).status.success());
}

#[test]
fn schema_renders_the_node_of_the_spec() {
    let doc = spec::generate();
    for command in ["evidence add", "completion", "purge", "brief check"] {
        let mut args = vec!["schema"];
        args.extend(command.split(' '));
        args.push("--json");
        let out = canon(&args);
        assert!(out.status.success(), "{command}: {}", String::from_utf8_lossy(&out.stderr));
        let shown: Value = serde_json::from_slice(&out.stdout).expect("JSON");
        let node = doc["commands"].as_array().expect("commands").iter().find(|n| n["command"] == command).expect("node");
        assert_eq!(&shown["data"], node, "`canon schema {command}` and the spec disagree");
    }
}
