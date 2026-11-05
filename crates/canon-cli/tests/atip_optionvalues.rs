//! `canon-0-37-option-values` (scenarios `atip.optionvalues.01-16`): an option of the same name takes the same value in every command that has it, against the built
//! binary and the documents it declares.
//!
//! There are three value rules: an id (`--project`, `--role`, `--domain`) is a kebab slug; a name (`--actor`, `--approver`, `--relayed-via`) and a sentence
//! (`--reason`, `--summary`, `--override-reason`) are one line of text, taken with the white space around it, which the parser trims, and refused when it is
//! blank or has a control character or a line or paragraph separator inside it. `--round` and `--limit` start at 1, `finding close` refuses `--disposition open`, and
//! `--regime` is the one key of `retrieve` and `dispatch begin`. `atip/option_values_0_36.json` holds what canon 0.36.0 accepted, per option and class of value, and
//! the CHANGELOG of the build holds a row for each value of it that is refused now.
//!
//! The checks that seeded faults must trip are small pure functions in this file (`sweep_problems`, `changelog_problems`): each real input passes them, each seeded
//! fault is found, and no sweep is vacuous. Tests are named by scenario id (`atip_optionvalues_05_…` proves `atip.optionvalues.05`).

mod harness;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::LazyLock;

use serde_json::{json, Value};
use tempfile::TempDir;

/// What canon 0.36.0 accepted, per option and class of value.
const GOLDEN: &str = "crates/canon-cli/tests/atip/option_values_0_36.json";

/// The names of the options whose commands do not share a domain or a set of values, and why: each is a decision of another change.
const ALLOWED: &[(&str, &str)] = &[
    ("kind", "the set of values differs by command (the contract's L1.one-meaning-values asks for one meaning, as a SHOULD; a change of its own)"),
    ("providers", "`init --providers` and the skills commands (`canon-0-37-option-forms`)"),
];

// ---------------------------------------------------------------------------
// The story repo and the calls.

struct Story {
    repo: TempDir,
    home: TempDir,
}

static STORY: LazyLock<Story> = LazyLock::new(|| {
    let home = TempDir::new().expect("a home");
    let repo = harness::fixture(home.path());
    Story { repo, home }
});

fn base() -> &'static Path {
    STORY.repo.path()
}

fn home() -> &'static Path {
    STORY.home.path()
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).unwrap_or_else(|e| panic!("`{path}` is readable: {e}"))
}

/// The result of one call on a copy of the story: its exit, its envelope, and the paths it changed.
struct Call {
    exit: i32,
    envelope: Value,
    changed: Vec<PathBuf>,
}

impl Call {
    fn code(&self) -> &str {
        self.envelope["error"]["code"].as_str().unwrap_or_default()
    }

    fn field(&self) -> &str {
        self.envelope["error"]["field"].as_str().unwrap_or_default()
    }
}

fn envelope(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("stdout is one JSON document ({e}): {}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)))
}

/// `canon <args> --json` on a copy of the story (a role is declared with `--as` when the call carries it).
fn call(args: &[&str]) -> Call {
    let mut argv = harness::strs(args);
    argv.push("--json".to_string());
    let (changed, output) = harness::run_on_copy(base(), home(), &argv);
    Call { exit: output.status.code().unwrap_or(-1), envelope: envelope(&output), changed }
}

/// The call, which must be refused with `code` at exit 2 naming `field`, and write nothing.
fn assert_refused(args: &[&str], code: &str, field: &str) {
    let got = call(args);
    assert_eq!((got.exit, got.code(), got.field()), (2, code, field), "canon {args:?}: {}", got.envelope);
    assert!(got.changed.is_empty(), "canon {args:?} was refused and wrote {:?}", got.changed);
}

fn assert_ok(args: &[&str]) -> Call {
    let got = call(args);
    assert_eq!(got.exit, 0, "canon {args:?}: {}", got.envelope);
    got
}

/// A call on a copy of the story that keeps the copy, so what it recorded can be read.
fn run_kept(args: &[&str]) -> (TempDir, PathBuf, Output) {
    let (guard, repo) = harness::copy_of(base());
    let mut argv = harness::strs(args);
    argv.push("--json".to_string());
    let output = harness::canon_in(&repo, home(), &argv, &[]);
    (guard, repo, output)
}

fn files_under(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                found.extend(files_under(&path, extension));
            } else if path.extension().is_some_and(|ext| ext == extension) {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

const FEATURE: &str = ".canon/specs/kind=feature/area=auth/login.feature";

// ---------------------------------------------------------------------------
// The documents.

fn docs() -> Vec<&'static Value> {
    harness::leaves()
}

fn option_of<'a>(doc: &'a Value, name: &str) -> Option<&'a Value> {
    doc["options"].as_array().into_iter().flatten().find(|option| option["name"] == name)
}

fn doc_of(command: &str) -> &'static Value {
    docs().into_iter().find(|doc| doc["command"] == command).unwrap_or_else(|| panic!("`{command}` is a command"))
}

/// Whether the domain of `--option` of `command` takes `value`; a value outside a closed set is refused.
fn admits(command: &str, option: &str, value: &Value) -> bool {
    let argument = option_of(doc_of(command), option).unwrap_or_else(|| panic!("`{command}` has `--{option}`"));
    if let Some(members) = argument["possibleValues"].as_array() {
        return members.contains(value);
    }
    match argument.get("domain") {
        None | Some(Value::Null) => true,
        Some(domain) => atip::domain::compile(domain).expect("the domain compiles").admits(value),
    }
}

/// One option name's declarations: its domains as written, the commands with each, and the closed sets.
type Commands = BTreeMap<String, BTreeSet<String>>;

fn declarations(docs: &[&Value]) -> BTreeMap<String, (Commands, Commands)> {
    let mut by_name: BTreeMap<String, (Commands, Commands)> = BTreeMap::new();
    for doc in docs {
        let command = doc["command"].as_str().expect("a command").to_string();
        for option in doc["options"].as_array().into_iter().flatten() {
            let name = option["name"].as_str().expect("an option has a name").to_string();
            let entry = by_name.entry(name).or_default();
            let domain = option.get("domain").map_or_else(|| "none".to_string(), ToString::to_string);
            entry.0.entry(domain).or_default().insert(command.clone());
            if let Some(members) = option["possibleValues"].as_array() {
                entry.1.entry(members.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(",")).or_default().insert(command.clone());
            }
        }
    }
    by_name
}

/// What is wrong with the declared options: a name with more than one domain, or whose closed sets are not nested (one contains each of the others), that is not in `allowed`.
fn sweep_problems(docs: &[&Value], allowed: &[&str]) -> Vec<String> {
    let mut problems = Vec::new();
    for (name, (domains, sets)) in declarations(docs) {
        if allowed.contains(&name.as_str()) {
            continue;
        }
        if domains.len() > 1 {
            let each: Vec<String> = domains.iter().map(|(domain, commands)| format!("{domain} in {commands:?}")).collect();
            problems.push(format!("`--{name}` has {} domains: {}", domains.len(), each.join("; ")));
        }
        let members: Vec<BTreeSet<&str>> = sets.keys().map(|set| set.split(',').collect()).collect();
        let widest = members.iter().max_by_key(|set| set.len());
        if let Some(widest) = widest {
            if members.iter().any(|set| !set.is_subset(widest)) {
                problems.push(format!("`--{name}` has closed sets that are not nested: {:?}", sets.keys().collect::<Vec<_>>()));
            }
        }
    }
    problems
}

// ---------------------------------------------------------------------------
// atip.optionvalues.01 - 02 (F1): an id is a kebab slug.

const KEBAB: &str = "^[a-z0-9]+(-[a-z0-9]+)*$";

#[test]
fn atip_optionvalues_01_project_role_and_domain_take_a_kebab_slug_and_nothing_else() {
    // Every command that has the option declares the one pattern.
    let mut seen = BTreeMap::new();
    for doc in docs() {
        for name in ["project", "role", "domain"] {
            if let Some(option) = option_of(doc, name) {
                assert_eq!(option["domain"]["pattern"], KEBAB, "`{} --{name}` declares the kebab slug", doc["command"]);
                *seen.entry(name).or_insert(0) += 1;
            }
        }
    }
    assert_eq!(seen, BTreeMap::from([("domain", 3), ("project", 11), ("role", 11)]), "every command that has an id option was read");
    // The reviewer-only refusal of `divergence accept --role` is a validator beside the domain, not a second domain.
    assert!(doc_of("divergence accept")["validators"].as_array().expect("validators").iter().any(|validator| validator["code"] == "DIVERGENCE_DENIED" && validator["arguments"] == json!(["role"])));
    // Values inside the grammar are taken where the call can run.
    assert_ok(&["feature", "new", "billing.invoice", "--title", "Invoices", "--project", "root", "--actor", "alice"]);
    assert_ok(&["query", "--kind", "subject", "--domain", "dev"]);
    assert_ok(&["finding", "add", "--change", "add-login", "--round", "1", "--seq", "2", "--severity", "note", "--actor", "alice", "--summary", "ok", "--role", "reviewer"]);
}

#[test]
fn atip_optionvalues_02_a_value_that_is_no_kebab_slug_is_refused_with_exit_2_and_nothing_is_written() {
    // `--project a-` and `--project a--b` were taken in 0.36.0.
    for value in ["a-", "a--b", "-a", "A", "a_b", "a b", ""] {
        let project = format!("--project={value}");
        assert_refused(&["divergence", "add", "auth.login.01", "--actor", "alice", "--role", "reviewer", &project], "USAGE", "--project");
        assert_refused(&["feature", "new", "billing.invoice", "--title", "Invoices", &project], "USAGE", "--project");
        let role = format!("--role={value}");
        assert_refused(&["finding", "add", "--change", "add-login", "--round", "1", "--seq", "2", "--severity", "note", "--actor", "alice", "--summary", "ok", &role], "USAGE", "--role");
    }
    // `--domain` was checked by `subject new` alone.
    for value in ["Dev", "a_b", "a-", "a--b", "a b"] {
        let domain = format!("--domain={value}");
        assert_refused(&["query", "--kind", "subject", &domain], "QUERY_ARGUMENT_INVALID", "--domain");
        assert_refused(&["retrieve", "--for-role", "reviewer", &domain], "USAGE", "--domain");
        assert_refused(&["subject", "new", "s2", "--title", "t", &domain], "SUBJECT_ARGUMENT_INVALID", "--domain");
    }
    // The permission of `divergence accept --role` is answered after the id rule, with its own code: the role must be the reviewer step.
    assert_refused(&["divergence", "accept", "auth.login.01", "--actor", "alice", "--reason", "r", "--tracking", "T-1", "--role", "implementer"], "DIVERGENCE_DENIED", "--role");
    assert_refused(&["divergence", "accept", "auth.login.01", "--actor", "alice", "--reason", "r", "--tracking", "T-1", "--role", "Reviewer"], "USAGE", "--role");
}

// ---------------------------------------------------------------------------
// atip.optionvalues.03 - 05 (F2): a name is one line of text.

/// The text of the provenance comment a scaffold or a record names its actor with.
fn recorded_actor(repo: &Path) -> Vec<String> {
    let mut found = Vec::new();
    for path in files_under(&repo.join(".canon"), "feature").into_iter().chain(files_under(&repo.join(".canon/ledger"), "json")) {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for marker in ["\"agent_id\":\"", "\"agent_id\": \""] {
            for part in text.split(marker).skip(1) {
                found.push(part.split('"').next().unwrap_or_default().to_string());
            }
        }
    }
    found
}

#[test]
fn atip_optionvalues_03_a_name_is_trimmed_and_the_trimmed_value_is_what_is_recorded_and_signed() {
    // The actor of a scaffold and of a finding.
    let (_guard, repo, output) = run_kept(&["scenario", "new", "auth.login.03", "--title", "Sign in twice", "--feature", FEATURE, "--actor", " alice "]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stdout));
    let feature = std::fs::read_to_string(repo.join(FEATURE)).expect("the feature");
    assert!(feature.contains("\"agent_id\":\"alice\""), "{feature}");
    assert!(!feature.contains("\"agent_id\":\" alice"), "{feature}");
    let (_guard, repo, output) = run_kept(&["finding", "add", "--change", "add-login", "--round", "1", "--seq", "2", "--severity", "note", "--actor", "\talice\n", "--summary", "ok"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stdout));
    let actors = recorded_actor(&repo);
    assert!(actors.iter().any(|actor| actor == "alice"), "{actors:?}");
    assert!(actors.iter().all(|actor| actor == actor.trim()), "every recorded actor is trimmed: {actors:?}");
    // The approver the signature is made of is the trimmed one, for the payload an approval is signed over.
    let payload = |approver: &str| assert_ok(&["evidence", "approval-payload", "--scenario", "auth.login.01", "--project", "root", "--approver", approver, "--approval-at", "2030-01-01T00:00:00Z"]).envelope["data"].clone();
    assert_eq!(payload(" alice "), payload("alice"));
    assert_eq!(payload("\talice\n"), payload("alice"));
    // The parser of every name and sentence option is the one that trims (`cli::trimmed`).
    assert_eq!(canon_cli::cli::trimmed("  a b \n"), Ok("a b".to_string()));
    assert_eq!(canon_cli::cli::trimmed("\u{3000}x\u{a0}"), Ok("x".to_string()));
}

const TEXT_OPTIONS: &[&str] = &["actor", "approver", "relayed-via", "reason", "summary", "override-reason"];

/// Each command that takes a name or a sentence option declares the one text domain for it.
#[test]
fn atip_optionvalues_03_every_name_and_sentence_option_declares_one_domain_in_every_command() {
    let mut read = 0;
    let mut patterns = BTreeSet::new();
    for doc in docs() {
        for name in TEXT_OPTIONS {
            let Some(option) = option_of(doc, name) else { continue };
            let pattern = option["domain"]["pattern"].as_str().unwrap_or_else(|| panic!("`{} --{name}` declares a pattern", doc["command"]));
            patterns.insert(pattern.to_string());
            read += 1;
        }
    }
    assert_eq!(patterns.len(), 1, "one pattern for the six options: {patterns:?}");
    assert!(read >= 40, "every cell was read ({read})");
    let pattern = patterns.into_iter().next().expect("one");
    assert!(pattern.contains("[^\\p{Cc}\\p{Zl}\\p{Zp}]"), "the class of a name and a sentence is written `[^\\p{{Cc}}\\p{{Zl}}\\p{{Zp}}]`: {pattern}");
    assert_eq!(atip::domain::pattern_problem(&pattern, atip::Minor::parse("1.5").expect("1.5")), None);
}

/// The values every name and sentence option takes and refuses, by the domain, over a table of values.
const TAKEN: &[&str] = &["alice", " alice ", "\talice\n", "a b", "a\u{a0}b", "a\u{3000}b", "알리스", "a\u{2028}", "\u{85}a", "--x", "a(b", "a'b"];
const REFUSED: &[&str] = &["", " ", "\t", "\n", "\u{85}", "\u{2028}", "a\nb", "a\rb", "a\tb", "a\u{b}b", "a\u{c}b", "a\u{1b}b", "a\u{7f}b", "a\u{85}b", "a\u{2028}b", "a\u{2029}b", "a\u{9f}b"];

#[test]
fn atip_optionvalues_04_a_blank_name_or_one_with_a_control_character_or_a_line_break_inside_is_refused() {
    let mut cells = 0;
    for doc in docs() {
        for name in TEXT_OPTIONS {
            if option_of(doc, name).is_none() {
                continue;
            }
            let command = doc["command"].as_str().expect("a command");
            for value in TAKEN {
                assert!(admits(command, name, &json!(value)), "`{command} --{name}` takes {value:?}");
            }
            for value in REFUSED {
                assert!(!admits(command, name, &json!(value)), "`{command} --{name}` refuses {value:?}");
            }
            cells += 1;
        }
    }
    assert!(cells >= 40, "{cells} cells");
    // Through the binary, in four commands that took everything or nearly: nothing is written and the command's own code answers.
    for (value, label) in [("a\nb", "a line break"), ("a\tb", "a tab"), ("a\u{1b}b", "an escape"), (" ", "white space only")] {
        let actor = format!("--actor={value}");
        assert_refused(&["divergence", "add", "auth.login.01", "--role", "reviewer", &actor], "DIVERGENCE_ARGUMENT_INVALID", "--actor");
        assert_refused(&["scenario", "new", "auth.login.04", "--title", "t", "--feature", FEATURE, &actor], "SCAFFOLD_ARGUMENT_INVALID", "--actor");
        assert_refused(&["subject", "new", "s2", "--domain", "dev", "--title", "t", &actor], "SUBJECT_ARGUMENT_INVALID", "--actor");
        assert_refused(&["finding", "add", "--change", "add-login", "--round", "1", "--seq", "2", "--severity", "note", "--summary", "ok", &actor], "FINDING_ARGUMENT_INVALID", "--actor");
        assert_refused(&["evidence", "add", "--project", "root", "--scenario", "auth.login.02", "--class", "test-run", "--ref", "npm test", "--role", "implementer", &actor], "EVIDENCE_ARGUMENT_INVALID", "--actor");
        let approver = format!("--approver={value}");
        assert_refused(&["evidence", "approval-payload", "--scenario", "auth.login.01", "--project", "root", "--approval-at", "2030-01-01T00:00:00Z", &approver], "EVIDENCE_ARGUMENT_INVALID", "--approver");
        let got = call(&["policy", "approve", &approver]);
        assert_eq!((got.exit, got.code(), got.field()), (2, "APPROVAL_INVALID", "--approver"), "{label}: {}", got.envelope);
        assert!(got.changed.is_empty(), "{label}: {:?}", got.changed);
    }
}

/// The four commands that take `--relayed-via` check the tool the same way, and declare it the same way.
const RELAYED: &[(&str, &[&str])] = &[
    ("brief approve", &["brief", "approve", "add-login", "--approver", "Ana", "--note", "ok", "--actor", "impl", "--digest", "add-login=sha256:0123456789abcdef"]),
    ("policy approve", &["policy", "approve", "--approver", "Ana", "--note", "ok", "--actor", "impl"]),
    ("divergence accept", &["divergence", "accept", "auth.login.01", "--actor", "alice", "--reason", "r", "--tracking", "T-1", "--role", "reviewer", "--approver", "Ana", "--note", "ok"]),
    ("migrate", &["migrate", "--commit", "--plan-digest", "abc", "--actor", "impl", "--approver", "Ana", "--note", "ok"]),
];

#[test]
fn atip_optionvalues_05_the_tool_of_a_relayed_approval_is_one_word_the_same_on_the_four_commands() {
    let mut descriptions = BTreeSet::new();
    for (command, args) in RELAYED {
        let doc = doc_of(command);
        assert_eq!(option_of(doc, "relayed-via").expect("--relayed-via")["domain"]["pattern"], option_of(doc_of("brief approve"), "relayed-via").expect("--relayed-via")["domain"]["pattern"], "{command}");
        let validator = doc["validators"].as_array().expect("validators").iter().find(|validator| validator["arguments"] == json!(["relayed-via"])).unwrap_or_else(|| panic!("`{command}` declares the one-word validator of --relayed-via"));
        descriptions.insert(validator["description"].as_str().expect("a description").to_string());
        assert_eq!(validator["code"], "USAGE", "{command}");
        for tool in ["ask", "AskUserQuestion", "chat"] {
            let mut argv: Vec<&str> = args.to_vec();
            argv.extend(["--relayed-via", tool, "--as", "implementer"]);
            let got = call(&argv);
            assert_ne!(got.field(), "--relayed-via", "`{command}` takes {tool} as a tool: {}", got.envelope);
        }
        for tool in ["a b", "a(b", "a)b", "a\"b", "a'b", "a\u{a0}b", "a\u{3000}b"] {
            let mut argv: Vec<&str> = args.to_vec();
            argv.extend(["--relayed-via", tool, "--as", "implementer"]);
            let got = call(&argv);
            assert_eq!((got.exit, got.code(), got.field()), (2, "USAGE", "--relayed-via"), "`{command} --relayed-via {tool:?}`: {}", got.envelope);
            assert!(got.changed.is_empty(), "`{command}`: {:?}", got.changed);
        }
        // A tab or a line break inside the tool is the domain's, and the same word is refused, as is an empty one.
        for tool in ["a\tb", "a\nb", "a\u{1b}b", "", " "] {
            let mut argv: Vec<&str> = args.to_vec();
            let value = format!("--relayed-via={tool}");
            argv.extend([value.as_str(), "--as", "implementer"]);
            let got = call(&argv);
            assert_eq!((got.exit, got.code(), got.field()), (2, "USAGE", "--relayed-via"), "`{command} --relayed-via {tool:?}`: {}", got.envelope);
        }
    }
    assert_eq!(descriptions.len(), 1, "the four commands say it in the same words: {descriptions:?}");
}

// ---------------------------------------------------------------------------
// atip.optionvalues.06 - 07 (F3): a sentence is one line of text.

#[test]
fn atip_optionvalues_06_a_sentence_is_trimmed_and_the_trimmed_value_is_what_is_recorded_and_hashed() {
    // `finding close --reason` recorded the trimmed text in 0.36.0; the others recorded what they were given.
    let (_guard, repo, output) = run_kept(&["finding", "close", "add-login", "--round", "1", "--seq", "1", "--disposition", "rejected", "--reason", "  works as designed \t"]);
    assert!(output.status.success(), "{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    let ledger: String = files_under(&repo.join(".canon/ledger/kind=finding"), "json").iter().map(|path| std::fs::read_to_string(path).unwrap_or_default()).collect();
    assert!(ledger.contains("works as designed") && !ledger.contains("  works as designed"), "{ledger}");
    let (_guard, repo, output) = run_kept(&["finding", "add", "--change", "add-login", "--round", "1", "--seq", "2", "--severity", "note", "--actor", "alice", "--summary", "\t Token is logged \n"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stdout));
    let ledger: String = files_under(&repo.join(".canon/ledger/kind=finding"), "json").iter().map(|path| std::fs::read_to_string(path).unwrap_or_default()).collect();
    assert!(ledger.contains("\"summary\": \"Token is logged\"") || ledger.contains("\"summary\":\"Token is logged\""), "{ledger}");
    let (_guard, repo, output) = run_kept(&["divergence", "add", "auth.login.01", "--actor", "alice", "--role", "reviewer", "--state", "deferred", "--reason", " waiting on the vendor ", "--expiry", "2030-01-01T00:00:00Z"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stdout));
    let staged: String = files_under(&repo.join(".canon/ledger"), "json").iter().map(|path| std::fs::read_to_string(path).unwrap_or_default()).collect();
    assert!(staged.contains("waiting on the vendor") && !staged.contains(" waiting on the vendor"), "{staged}");
    // What a signature is made of (the rollback's reason hash, the approver of a payload) is read by the same parser: `strategy rollback --reason` is trimmed.
    let mut command = canon_cli::cli::command();
    command.build();
    let matches = command.try_get_matches_from(["canon", "strategy", "rollback", "01ARZ3NDEKTSV4RRFFQ69G5FAV", "--reason", "  regressed in production \n", "--actor", " alice ", "--signature-file", "s.sig", "--approval-at", "2030-01-01"]).expect("the call parses");
    let (_, rollback) = matches.subcommand().expect("strategy");
    let (_, leaf) = rollback.subcommand().expect("rollback");
    assert_eq!(leaf.get_one::<String>("reason").map(String::as_str), Some("regressed in production"));
    assert_eq!(leaf.get_one::<String>("actor_id").map(String::as_str), Some("alice"));
}

#[test]
fn atip_optionvalues_07_a_blank_sentence_or_one_with_a_control_character_or_a_line_break_inside_is_refused_with_the_commands_own_code() {
    let cases: &[(&str, &[&str], &str, &str)] = &[
        ("divergence add --reason", &["divergence", "add", "auth.login.01", "--actor", "alice", "--role", "reviewer", "--state", "deferred", "--expiry", "2030-01-01T00:00:00Z"], "--reason", "DIVERGENCE_ARGUMENT_INVALID"),
        ("divergence defer --reason", &["divergence", "defer", "auth.login.01", "--actor", "alice", "--role", "reviewer", "--expiry", "2030-01-01T00:00:00Z"], "--reason", "DIVERGENCE_ARGUMENT_INVALID"),
        ("divergence accept --reason", &["divergence", "accept", "auth.login.01", "--actor", "alice", "--tracking", "T-1", "--role", "reviewer"], "--reason", "DIVERGENCE_ARGUMENT_INVALID"),
        ("finding close --reason", &["finding", "close", "add-login", "--round", "1", "--seq", "1", "--disposition", "rejected"], "--reason", "FINDING_REASON_REQUIRED"),
        ("subject move --reason", &["subject", "move", "demo", "--to", "specced"], "--reason", "SUBJECT_ARGUMENT_INVALID"),
        ("subject move --override-reason", &["subject", "move", "demo", "--to", "specced"], "--override-reason", "SUBJECT_ARGUMENT_INVALID"),
        ("subject new --summary", &["subject", "new", "s2", "--domain", "dev", "--title", "t"], "--summary", "SUBJECT_ARGUMENT_INVALID"),
        ("finding add --summary", &["finding", "add", "--change", "add-login", "--round", "1", "--seq", "2", "--severity", "note", "--actor", "alice"], "--summary", "FINDING_ARGUMENT_INVALID"),
        ("evidence add --summary", &["evidence", "add", "--project", "root", "--scenario", "auth.login.02", "--class", "test-run", "--ref", "npm test", "--role", "implementer"], "--summary", "EVIDENCE_ARGUMENT_INVALID"),
        ("evidence add --override-reason", &["evidence", "add", "--project", "root", "--scenario", "auth.login.02", "--class", "test-run", "--ref", "npm test", "--role", "implementer"], "--override-reason", "EVIDENCE_ARGUMENT_INVALID"),
    ];
    for (label, args, option, code) in cases {
        for value in ["a\nb", "a\tb", "a\u{1b}b", "   ", ""] {
            let given = format!("{option}={value}");
            let mut argv: Vec<&str> = args.to_vec();
            argv.push(&given);
            let got = call(&argv);
            assert_eq!((got.exit, got.code(), got.field()), (2, *code, *option), "`{label}` {value:?}: {}", got.envelope);
            assert!(got.changed.is_empty(), "`{label}` {value:?} was refused and wrote {:?}", got.changed);
        }
    }
    // The signed reason: a blank or a multi-line reason never reaches the hash.
    let got = call(&["strategy", "rollback", "01ARZ3NDEKTSV4RRFFQ69G5FAV", "--reason=a\nb", "--actor", "alice", "--signature-file", "s.sig", "--approval-at", "2030-01-01"]);
    assert_eq!((got.exit, got.code(), got.field()), (2, "STRATEGY_ARGUMENT_INVALID", "--reason"), "{}", got.envelope);
}

// ---------------------------------------------------------------------------
// atip.optionvalues.08 - 09 (F4): one key for `--regime`.

#[test]
fn atip_optionvalues_08_retrieve_and_dispatch_begin_declare_the_same_regime_key_and_take_the_same_keys() {
    let retrieve = option_of(doc_of("retrieve"), "regime").expect("retrieve --regime");
    let begin = option_of(doc_of("dispatch begin"), "regime").expect("dispatch begin --regime");
    assert_eq!(retrieve["domain"], begin["domain"], "one domain for the two commands");
    assert!(retrieve["domain"]["pattern"].as_str().is_some_and(|pattern| pattern.contains("{6,64}")), "{retrieve}");
    for key in ["reviewer/canon/join-spine/9c93d024b1a2", "dev/my-repo/api/abcdef", "reviewer/b/c/0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"] {
        for command in ["retrieve", "dispatch begin"] {
            assert!(admits(command, "regime", &json!(key)), "`{command}` takes {key}");
        }
    }
    // A valid key is taken by both at the stateless stage: the next answer is not about it.
    let got = call(&["dispatch", "begin", "--role", "reviewer", "--regime", "reviewer/canon/join-spine/9c93d024b1a2"]);
    assert_ne!(got.field(), "--regime", "{}", got.envelope);
    let got = call(&["retrieve", "--for-role", "reviewer", "--regime", "reviewer/canon/join-spine/9c93d024b1a2"]);
    assert_ne!(got.field(), "--regime", "{}", got.envelope);
}

#[test]
fn atip_optionvalues_09_a_key_of_three_parts_or_with_a_space_is_refused_by_dispatch_begin_as_retrieve_refuses_it() {
    for key in ["reviewer/canon/join-spine", "reviewer/can on/join-spine/9c93d024b1a2", "reviewer/canon/join-spine/9C93D024B1A2", "reviewer/canon/join-spine/9c93d", "reviewer//join-spine/9c93d024b1a2", "reviewer/canon/join-spine/9c93d024b1a2/extra"] {
        let begin = call(&["dispatch", "begin", "--role", "reviewer", &format!("--regime={key}")]);
        let retrieve = call(&["retrieve", "--for-role", "reviewer", &format!("--regime={key}")]);
        assert_eq!((begin.exit, begin.code(), begin.field()), (2, "USAGE", "--regime"), "{key}: {}", begin.envelope);
        assert_eq!((retrieve.exit, retrieve.code(), retrieve.field()), (2, "USAGE", "--regime"), "{key}: {}", retrieve.envelope);
        assert!(begin.changed.is_empty() && retrieve.changed.is_empty());
    }
    // The role of the key is still held to `--role`.
    let got = call(&["dispatch", "begin", "--role", "reviewer", "--regime", "implementer/canon/join-spine/9c93d024b1a2"]);
    assert_eq!((got.exit, got.code()), (2, "DISPATCH_ROLE_CONFLICT"), "{}", got.envelope);
}

// ---------------------------------------------------------------------------
// atip.optionvalues.10 - 11 (F5): the bounds.

#[test]
fn atip_optionvalues_10_round_and_limit_start_at_1_and_a_finding_is_closed_by_fixed_rejected_or_deferred() {
    for doc in docs() {
        for name in ["round", "limit"] {
            if let Some(option) = option_of(doc, name) {
                assert_eq!(option["domain"]["minimum"], 1, "`{} --{name}` starts at 1", doc["command"]);
            }
        }
    }
    for command in ["divergence add", "divergence defer", "divergence resolve"] {
        assert_eq!(option_of(doc_of(command), "round").expect("round")["domain"], json!({ "maximum": 4_294_967_295_u64, "minimum": 1 }), "{command}");
    }
    assert_eq!(option_of(doc_of("retrieve"), "limit").expect("limit")["domain"], json!({ "minimum": 1 }));
    assert_eq!(option_of(doc_of("finding close"), "disposition").expect("disposition")["possibleValues"], json!(["fixed", "rejected", "deferred"]));
    assert_eq!(option_of(doc_of("finding add"), "disposition").expect("disposition")["possibleValues"], json!(["open", "fixed", "rejected", "deferred"]));
    // The values inside are taken.
    assert_ok(&["divergence", "add", "auth.login.01", "--actor", "alice", "--role", "reviewer", "--round", "1"]);
    assert_ok(&["divergence", "resolve", "auth.login.01", "--actor", "alice", "--role", "reviewer", "--round", "1"]);
    assert_ne!(call(&["retrieve", "--for-role", "reviewer", "--domain", "dev", "--limit", "1"]).field(), "--limit");
    assert_ok(&["finding", "close", "add-login", "--round", "1", "--seq", "1", "--disposition", "fixed"]);
    assert_ok(&["finding", "add", "--change", "add-login", "--round", "1", "--seq", "2", "--severity", "note", "--actor", "alice", "--summary", "x", "--disposition", "open"]);
}

#[test]
fn atip_optionvalues_11_round_0_limit_0_and_closing_a_finding_to_open_are_refused_with_exit_2_and_nothing_is_recorded() {
    for command in ["add", "defer", "resolve"] {
        let mut argv = vec!["divergence", command, "auth.login.01", "--actor", "alice", "--role", "reviewer", "--round", "0"];
        if command == "defer" {
            argv.extend(["--reason", "later", "--expiry", "2030-01-01T00:00:00Z"]);
        }
        assert_refused(&argv, "DIVERGENCE_ARGUMENT_INVALID", "--round");
    }
    assert_refused(&["finding", "close", "add-login", "--round", "0", "--seq", "1", "--disposition", "fixed"], "FINDING_ARGUMENT_INVALID", "--round");
    assert_refused(&["retrieve", "--for-role", "reviewer", "--domain", "dev", "--limit", "0"], "USAGE", "--limit");
    // 0.36.0 reopened a finding; a defect that came back is a new finding now.
    let got = call(&["finding", "close", "add-login", "--round", "1", "--seq", "1", "--disposition", "open"]);
    assert_eq!((got.exit, got.code(), got.field()), (2, "USAGE", "--disposition"), "{}", got.envelope);
    assert!(got.changed.is_empty(), "{:?}", got.changed);
    assert_eq!(got.envelope["error"]["expected"], json!(["fixed", "rejected", "deferred"]), "{}", got.envelope);
    // Nor is a reopening reachable by a document of `--input`.
    let (changed, output) = harness::run_on_copy(base(), home(), &harness::strs(&["finding", "close", "--input", "{\"operands\":[\"add-login\"],\"round\":1,\"seq\":1,\"disposition\":\"open\"}", "--json"]));
    assert_eq!(output.status.code(), Some(2), "{}", String::from_utf8_lossy(&output.stdout));
    assert!(changed.is_empty(), "{changed:?}");
}

// ---------------------------------------------------------------------------
// atip.optionvalues.12 - 13 (F6): one definition per name.

#[test]
fn atip_optionvalues_12_every_option_of_the_same_name_has_one_domain_in_every_command() {
    let allowed: Vec<&str> = ALLOWED.iter().map(|(name, _)| *name).collect();
    let problems = sweep_problems(&docs(), &allowed);
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
    // The sweep is not vacuous: the names it reads, and the names several commands share, are many.
    let all = declarations(&docs());
    assert!(all.len() > 80, "{} option names", all.len());
    let shared = all.values().filter(|(domains, _)| domains.values().map(BTreeSet::len).sum::<usize>() > 1).count();
    assert!(shared > 25, "{shared} names are shared by several commands");
    for name in TEXT_OPTIONS.iter().chain(["project", "role", "domain", "round", "limit", "regime", "disposition"].iter()) {
        assert!(all.contains_key(*name), "`--{name}` was read");
        assert_eq!(all[*name].0.len(), 1, "`--{name}` has one domain");
    }
    for (name, why) in ALLOWED {
        assert!(all.contains_key(*name), "the allowed name `--{name}` ({why}) is an option");
    }
}

#[test]
fn atip_optionvalues_13_a_command_that_declares_another_domain_for_the_same_name_fails_the_sweep() {
    let real: Vec<Value> = docs().into_iter().cloned().collect();
    // One command gives `--actor` the pattern of a bare id: the sweep finds it.
    let mut seeded = real.clone();
    let doc = seeded.iter_mut().find(|doc| doc["command"] == "finding add").expect("finding add");
    let option = doc["options"].as_array_mut().expect("options").iter_mut().find(|option| option["name"] == "actor").expect("actor");
    option["domain"] = json!({ "pattern": "^[a-z0-9]+$" });
    let refs: Vec<&Value> = seeded.iter().collect();
    let found = sweep_problems(&refs, &[]);
    assert!(found.iter().any(|problem| problem.contains("`--actor` has 2 domains") && problem.contains("finding add")), "{found:?}");
    // One command takes away the domain of `--reason`.
    let mut seeded = real.clone();
    let doc = seeded.iter_mut().find(|doc| doc["command"] == "subject move").expect("subject move");
    let option = doc["options"].as_array_mut().expect("options").iter_mut().find(|option| option["name"] == "reason").expect("reason");
    option.as_object_mut().expect("an object").remove("domain");
    let refs: Vec<&Value> = seeded.iter().collect();
    assert!(sweep_problems(&refs, &[]).iter().any(|problem| problem.contains("`--reason` has 2 domains")));
    // A closed set that is not a subset of the widest one is found, one that is a subset is not.
    let mut seeded = real.clone();
    let doc = seeded.iter_mut().find(|doc| doc["command"] == "finding close").expect("finding close");
    let option = doc["options"].as_array_mut().expect("options").iter_mut().find(|option| option["name"] == "disposition").expect("disposition");
    option["possibleValues"] = json!(["fixed", "reopened"]);
    let refs: Vec<&Value> = seeded.iter().collect();
    assert!(sweep_problems(&refs, &[]).iter().any(|problem| problem.contains("`--disposition` has closed sets that are not nested")));
    // The allowed names are the allowed ones: the real documents pass with the list and fail without it, by name.
    let refs: Vec<&Value> = real.iter().collect();
    let without = sweep_problems(&refs, &[]);
    assert!(without.iter().any(|problem| problem.contains("`--providers`")), "{without:?}");
    // `--kind` has closed sets that are nested today; a set that is not nested is found, and the allowed name skips it.
    let mut seeded = real.clone();
    let doc = seeded.iter_mut().find(|doc| doc["command"] == "query").expect("query");
    let option = doc["options"].as_array_mut().expect("options").iter_mut().find(|option| option["name"] == "kind").expect("kind");
    option["possibleValues"] = json!(["change", "no-such-kind"]);
    let refs_seeded: Vec<&Value> = seeded.iter().collect();
    assert!(sweep_problems(&refs_seeded, &[]).iter().any(|problem| problem.contains("`--kind` has closed sets that are not nested")));
    assert!(!sweep_problems(&refs_seeded, &["kind"]).iter().any(|problem| problem.contains("`--kind`")));
    assert!(sweep_problems(&refs, &["kind", "providers"]).is_empty());
}

// ---------------------------------------------------------------------------
// atip.optionvalues.14 - 15 (F7): the CHANGELOG says what is refused now.

/// One class of value of the golden file, as an entry says it: the option, the class, a value of it, and the commands that took it in 0.36.0.
struct Entry {
    option: String,
    class: String,
    value: Value,
    commands: Vec<String>,
}

fn golden() -> Vec<Entry> {
    let file: Value = serde_json::from_str(&read(GOLDEN)).expect("the golden file is JSON");
    file["accepted"]
        .as_array()
        .expect("accepted")
        .iter()
        .map(|entry| Entry {
            option: entry["option"].as_str().expect("an option").to_string(),
            class: entry["class"].as_str().expect("a class").to_string(),
            value: match (entry["kind"].as_str(), entry["value"].as_str()) {
                (Some("number"), Some(number)) => json!(number.parse::<i64>().expect("a number")),
                (_, Some(text)) => json!(text),
                _ => panic!("a value is text"),
            },
            commands: entry["commands"].as_array().expect("commands").iter().map(|command| command.as_str().expect("a command").to_string()).collect(),
        })
        .collect()
}

/// The `(option, command, class)` triples of the golden file that this build refuses: a value 0.36.0 took.
fn refused_now(entries: &[Entry]) -> BTreeSet<(String, String, String)> {
    let mut found = BTreeSet::new();
    for entry in entries {
        for command in &entry.commands {
            if !admits(command, &entry.option, &entry.value) {
                found.insert((entry.option.clone(), command.clone(), entry.class.clone()));
            }
        }
    }
    found
}

/// The `### Breaking` table of the `Unreleased` entry: its rows, split into cells.
fn breaking_rows(changelog: &str) -> Vec<Vec<String>> {
    let unreleased = changelog.split("\n## ").find(|part| part.starts_with("Unreleased")).unwrap_or_default();
    let breaking = unreleased.split("\n### ").find(|part| part.starts_with("Breaking")).unwrap_or_default();
    breaking
        .lines()
        .filter(|line| line.starts_with("| `--"))
        .map(|line| line.trim().trim_matches('|').split('|').map(|cell| cell.trim().to_string()).collect())
        .collect()
}

fn backticked(cell: &str) -> BTreeSet<String> {
    cell.split('`').enumerate().filter(|(index, _)| index % 2 == 1).map(|(_, word)| word.to_string()).collect()
}

/// What the CHANGELOG lacks: a row for each value 0.36.0 took and this build refuses (its option in the first cell, its command in the second, its class in the
/// third), and the entry for the values that are recorded trimmed.
fn changelog_problems(changelog: &str, refused: &BTreeSet<(String, String, String)>) -> Vec<String> {
    let rows = breaking_rows(changelog);
    let mut problems = Vec::new();
    for (option, command, class) in refused {
        let found = rows.iter().any(|row| row.len() >= 5 && backticked(&row[0]).contains(&format!("--{option}")) && backticked(&row[1]).contains(command) && backticked(&row[2]).contains(class));
        if !found {
            problems.push(format!("no row of the table of newly refused values names `--{option}` in `{command}` for the class `{class}`"));
        }
    }
    for row in &rows {
        if row.len() < 5 || row[3].is_empty() || row[4].is_empty() {
            problems.push(format!("a row has no 0.37 answer or no way to move: {row:?}"));
        }
    }
    let unreleased = changelog.split("\n## ").find(|part| part.starts_with("Unreleased")).unwrap_or_default();
    if !unreleased.contains("recorded without the white space around") {
        problems.push("no entry says that a value is recorded without the white space around it".to_string());
    }
    problems
}

#[test]
fn atip_optionvalues_14_the_changelog_has_a_row_for_every_value_0_36_took_and_this_build_refuses_and_an_entry_for_the_trimmed_ones() {
    let entries = golden();
    let refused = refused_now(&entries);
    assert!(refused.len() > 80, "the golden file reaches many cells ({})", refused.len());
    for option in ["actor", "approver", "relayed-via", "reason", "summary", "override-reason", "project", "domain", "round", "limit", "disposition"] {
        assert!(refused.iter().any(|(name, _, _)| name == option), "`--{option}` has a value that was taken and is refused");
    }
    // What the golden holds is what the table must say, and a value that is still taken needs no row: an inside value is not refused.
    assert!(!refused.contains(&("actor".to_string(), "finding add".to_string(), "blank".to_string())), "finding add refused a blank actor in 0.36.0 too");
    let changelog = read("CHANGELOG.md");
    let problems = changelog_problems(&changelog, &refused);
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
    // The table has no row that no refusal needs.
    let rows = breaking_rows(&changelog);
    assert!(rows.len() >= 15, "{} rows", rows.len());
    for row in &rows {
        let (options, commands, classes) = (backticked(&row[0]), backticked(&row[1]), backticked(&row[2]));
        let needed = refused.iter().any(|(option, command, class)| options.contains(&format!("--{option}")) && commands.contains(command) && classes.contains(class));
        assert!(needed, "the row {row:?} names a value that is not refused now");
    }
}

#[test]
fn atip_optionvalues_15_the_check_of_the_changelog_fails_for_a_missing_row_a_missing_answer_and_a_missing_entry() {
    let entries = golden();
    let refused = refused_now(&entries);
    let changelog = read("CHANGELOG.md");
    assert!(changelog_problems(&changelog, &refused).is_empty());
    // A row taken out: the first row that names `--actor` and `tab-inside`.
    let lines: Vec<&str> = changelog.lines().collect();
    let at = lines.iter().position(|line| line.starts_with("| `--actor`") && line.contains("`tab-inside`")).expect("a row names --actor and tab-inside");
    let without_row: String = lines.iter().enumerate().filter(|(index, _)| *index != at).map(|(_, line)| *line).collect::<Vec<_>>().join("\n");
    let problems = changelog_problems(&without_row, &refused);
    assert!(problems.iter().any(|problem| problem.contains("`--actor`") && problem.contains("tab-inside")), "{problems:?}");
    // A row with no way to move.
    let blank_move = lines.iter().enumerate().map(|(index, line)| if index == at { "| `--actor` | `evidence add` | `tab-inside` | refused |  |".to_string() } else { (*line).to_string() }).collect::<Vec<_>>().join("\n");
    assert!(changelog_problems(&blank_move, &refused).iter().any(|problem| problem.contains("no way to move")));
    // The entry for the trimmed values taken out.
    let without_entry = changelog.replace("recorded without the white space around", "recorded as given");
    assert!(changelog_problems(&without_entry, &refused).iter().any(|problem| problem.contains("without the white space around it")));
    // A value the build takes is not needed, a refused one is: a seeded class that was taken and is refused needs its row.
    let mut seeded = refused.clone();
    seeded.insert(("actor".to_string(), "dispatch begin".to_string(), "no-such-class".to_string()));
    assert!(changelog_problems(&changelog, &seeded).iter().any(|problem| problem.contains("no-such-class")));
}

// ---------------------------------------------------------------------------
// atip.optionvalues.16: the checker's fixture holds the new domains to the binary.

#[test]
fn atip_optionvalues_16_the_checker_fixture_lists_values_the_new_domains_refuse_and_the_recipes_run_at_1_5() {
    let fixture: Value = serde_json::from_str(&read("crates/canon-cli/tests/atip/canon.fixture.json")).expect("the fixture is JSON");
    assert_eq!(fixture["contractVersion"], "1.5");
    let commands = fixture["commands"].as_object().expect("commands");
    let mut listed = BTreeSet::new();
    for (command, entry) in commands {
        for (argument, values) in entry["domainRejects"].as_object().into_iter().flatten() {
            for value in values.as_array().expect("values") {
                let option = option_of(doc_of(command), argument);
                if let Some(option) = option {
                    let value = if option["valueType"] == "integer" { json!(value.as_str().and_then(|text| text.parse::<i64>().ok()).expect("an integer")) } else { value.clone() };
                    assert!(!admits(command, argument, &value), "`{command} --{argument}` refuses {value}");
                    listed.insert((command.clone(), argument.clone()));
                }
            }
        }
    }
    for cell in [("finding add", "actor"), ("evidence add", "summary"), ("divergence add", "round"), ("query", "domain"), ("retrieve", "limit"), ("dispatch begin", "regime")] {
        assert!(listed.contains(&(cell.0.to_string(), cell.1.to_string())), "the fixture lists values `{} --{}` refuses", cell.0, cell.1);
    }
    let justfile = read("justfile");
    assert!(justfile.contains("--contract 1.5"), "the recipes run the checker at 1.5");
}
