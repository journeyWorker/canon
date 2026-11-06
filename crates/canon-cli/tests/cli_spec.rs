//! The generated command spec (`docs/cli/canon.cli.json`, 0.28 D2) and the
//! things built from it.
//!
//! T4: the spec regenerated in memory equals the checked-in file byte for
//! byte. `CANON_BLESS_CLI_SPEC=1 cargo test -p canon-cli --test cli_spec`
//! rewrites it. The other tests hold the spec to the tree it describes and
//! the `canon completion` leaf to its contract.

#[path = "support/hermetic.rs"]
mod hermetic;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use canon_cli::cli;
use canon_cli::spec::{self, INTERNAL_ENV, SPEC_PATH};
use serde_json::Value;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn canon(args: &[&str]) -> Output {
    let dir = tempfile::tempdir().expect("tempdir");
    hermetic::canon().args(args).current_dir(dir.path()).output().expect("spawn canon binary")
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
    let mut spec_commands: Vec<&str> = doc["tree"]["commands"].as_array().expect("the tree lists commands").iter().map(|n| n["command"].as_str().expect("command")).collect();
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
    // `commands` holds one document for each leaf of the tree and no group.
    let leaves: BTreeSet<&str> = doc["tree"]["commands"].as_array().expect("tree").iter().filter(|n| n["kind"] != "group").map(|n| n["command"].as_str().expect("command")).collect();
    let documented: BTreeSet<&str> = doc["commands"].as_array().expect("commands").iter().map(|n| n["command"].as_str().expect("command")).collect();
    assert_eq!(leaves, documented);
}

#[test]
fn a_command_document_says_everything_an_agent_needs() {
    // What the commands declare, in the full model form (the file holds the 1.4 form of the same documents).
    let doc = spec::model();
    let commands = doc["commands"].as_array().expect("commands");
    for node in commands {
        let command = node["command"].as_str().expect("command");
        assert_eq!(node["kind"], "command");
        for key in ["summary", "effect", "operands", "options", "errors", "outputs", "examples", "schemas", "x-canon"] {
            assert!(node.get(key).is_some(), "`{command}` has no `{key}`");
        }
        assert!(!node["outputs"].as_array().expect("outputs").is_empty(), "`{command}` declares no output");
        assert!(!node["examples"].as_array().expect("examples").is_empty(), "`{command}` has no example");
        for operand in node["operands"].as_array().expect("operands") {
            assert!(operand["help"].as_str().is_some_and(|d| !d.is_empty()), "`{command}`: operand {} has no help", operand["name"]);
        }
        // A writing command declares how a repeat behaves; a read has no idempotency.
        assert_eq!(node.get("idempotency").is_some(), node["effect"] != "read", "`{command}`");
        if let Some(idempotency) = node.get("idempotency") {
            assert!(["replay-safe", "content-keyed", "appends", "once"].contains(&idempotency.as_str().expect("a word")), "`{command}`: {idempotency}");
        }
        for output in node["outputs"].as_array().expect("outputs") {
            let kind = output["kind"].as_str().expect("output kind");
            assert!(["report", "artifact", "stream"].contains(&kind), "`{command}`: output kind {kind}");
            assert_eq!(output.get("mediaType").is_some(), kind == "artifact", "`{command}`: only an artifact has a media type");
        }
    }
    let groups = doc["tree"]["commands"].as_array().expect("tree").iter().filter(|n| n["kind"] == "group").count();
    assert!(commands.len() > 60 && groups > 20, "{} commands, {groups} groups", commands.len());
}

#[test]
fn every_leaf_has_one_facts_row_and_every_row_a_leaf() {
    let problems = canon_cli::spec_facts::check_facts(&cli::command());
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The relations the guides' syntax lines are drawn from are the `constraints` of the command documents: clap's
/// conflicts and groups, and the tables' `requires`.
#[test]
fn the_relations_the_guides_draw_are_the_constraints_of_the_documents() {
    let doc = spec::generate();
    let relations = |command: &str| {
        let found = doc["commands"].as_array().expect("commands").iter().find(|leaf| leaf["command"] == command).unwrap_or_else(|| panic!("no `{command}`"));
        atip::project::Relations::from_json(&found["constraints"])
    };
    // `scenario new --change` requires `--flow`: the table says it, clap does not expose it.
    assert_eq!(relations("scenario new").requires["change"], ["flow"]);
    // `--history` conflicts `--plugin` on `query`, whichever side declared it.
    let query = relations("query");
    assert_eq!(query.conflicts_with["history"], ["plugin"]);
    assert_eq!(query.conflicts_with["plugin"], ["history"]);
}

/// An embedded schema does not carry its own base URI: the `#/schemas/..` references of a command document
/// resolve against the document (approaches/rust.md section 5), and none of its members is a stray resource.
#[test]
fn no_embedded_schema_names_its_own_base_uri() {
    let doc = spec::generate();
    let mut checked = 0;
    // The schemas live in the file's `schemas` map, one `--schemas` answer per command; a command document holds none.
    assert!(doc["commands"].as_array().expect("commands").iter().all(|command| command.get("schemas").is_none()));
    for (command, answer) in doc["schemas"].as_object().expect("schemas") {
        for (name, schema) in answer["schemas"].as_object().expect("the schemas of the answer") {
            assert!(schema.get("$id").is_none() && schema.get("$schema").is_none() && schema.get("$defs").is_none(), "`{command}` {name}");
            checked += 1;
        }
    }
    assert!(checked > 200, "{checked} schemas");
}

#[test]
fn the_spec_carries_what_d2_promises() {
    let doc = spec::generate();
    let model = spec::model();
    for key in ["_doNotEdit", "specVersion", "cli", "envVars", "exitTable", "contract", "contractVersion", "errors", "warnings", "removed", "tree", "commands", "records"] {
        assert!(doc.get(key).is_some(), "the spec has no `{key}`");
    }
    assert_eq!(doc["cli"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(doc["contract"], "atip/1", "the envelope is the contract's, not a canon schema");
    assert!(doc.get("envelopes").is_none());
    let env_names: Vec<&str> = doc["envVars"].as_array().expect("envVars").iter().map(|v| v["name"].as_str().expect("name")).collect();
    for name in ["CANON_ROLE", "CANON_ACTOR", "CANON_SKILLS_SOURCE"] {
        assert!(env_names.contains(&name), "{name} is not in the spec");
    }
    assert!(model["commands"].as_array().expect("commands").iter().all(|c| c["options"].as_array().expect("options").iter().any(|o| o["name"] == "as")), "every command declares the inherited --as");
    assert!(doc["tree"]["options"].as_array().expect("the tree's shared options").iter().any(|o| o["name"] == "as") && doc["commands"].as_array().expect("commands").iter().all(|c| c["options"].as_array().is_some_and(|options| options.iter().any(|o| o == "as" || o["name"] == "as"))), "the file names the shared --as on every command (or defines its own)");
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
            // A name that is a reason of a registered code (`CANON_YAML`, the reason of `CONFIG_INVALID`) is a reason, not a variable.
            let is_reason = canon_cli::cli_error::all_codes().iter().any(|info| info.reasons.iter().any(|r| r.reason == name));
            if rest[name.len()..].starts_with('"') && !is_reason {
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
