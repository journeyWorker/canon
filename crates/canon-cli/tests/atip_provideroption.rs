//! `canon-0-38-provider-option` (scenarios `atip.provideroption.01-15`): the harnesses are chosen by the repeatable `--harness` and the switch `--no-harness` of `init`,
//! `skills install`, `skills check` and `skills doctor`; `--input` takes `harness: [...]` and `noHarness: true`; `--providers` is refused with `USAGE` and a hint that
//! shows the corrected line; the four commands declare one enumeration. Against the built binary and the documents
//! it declares.
//!
//! The check that seeded faults must trip is a small pure function in this file (`harness_problems`): the real input passes it, each seeded fault is
//! found. Tests are named by scenario id (`atip_provideroption_05_…` proves `atip.provideroption.05`).

#[path = "support/hermetic.rs"]
mod hermetic;
mod harness;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::LazyLock;

use serde_json::{json, Value};
use tempfile::TempDir;

/// The four commands that take the harness options.
const COMMANDS: [&str; 4] = ["init", "skills check", "skills doctor", "skills install"];

/// The three commands of the `skills` group among them.
const SKILLS_COMMANDS: [&str; 3] = ["install", "check", "doctor"];

/// The names `--harness` enumerates, once each.
const NAMES: [&str; 4] = ["claude", "codex", "omp", "pi"];

/// What a name has to be to be taken: none of these is.
const NOT_A_NAME: [&str; 3] = ["Claude", "claude ", "gemini"];

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

/// A new git directory, which `init` sets up.
fn fresh() -> TempDir {
    let dir = TempDir::new().expect("a directory");
    harness::git(dir.path(), &["init", "-q", "."]);
    dir
}

fn document(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("stdout is one JSON document ({e}): {}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)))
}

fn run(dir: &Path, args: &[&str]) -> Output {
    harness::canon_in(dir, home(), &harness::strs(args), &[])
}

fn run_strings(dir: &Path, args: &[String]) -> Output {
    harness::canon_in(dir, home(), args, &[])
}

/// What a call said and did: its exit, its document and the paths it changed in the directory it ran in.
struct Call {
    exit: i32,
    document: Value,
    changed: Vec<PathBuf>,
}

impl Call {
    fn code(&self) -> &str {
        self.document["error"]["code"].as_str().unwrap_or_default()
    }

    fn field(&self) -> &str {
        self.document["error"]["field"].as_str().unwrap_or_default()
    }
}

/// `canon <args> --json` on a copy of the story.
fn on_story(args: &[&str]) -> Call {
    let mut argv = harness::strs(args);
    argv.push("--json".to_string());
    let (changed, output) = harness::run_on_copy(base(), home(), &argv);
    Call { exit: output.status.code().unwrap_or(-1), document: document(&output), changed }
}

/// `canon init <args> --json` in a new git directory.
fn init_fresh(args: &[&str]) -> (TempDir, Call) {
    let dir = fresh();
    let before = harness::tree(dir.path());
    let mut argv = vec!["init"];
    argv.extend_from_slice(args);
    argv.push("--json");
    let output = run(dir.path(), &argv);
    let changed = harness::changed(&before, &harness::tree(dir.path()));
    let call = Call { exit: output.status.code().unwrap_or(-1), document: document(&output), changed };
    (dir, call)
}

/// The call, which must be refused with `code` at exit 2 naming `field`, and write nothing.
fn assert_refused(args: &[&str], code: &str, field: &str) -> Call {
    let got = on_story(args);
    assert_eq!((got.exit, got.code(), got.field()), (2, code, field), "canon {args:?}: {}", got.document);
    assert!(got.changed.is_empty(), "canon {args:?} was refused and wrote {:?}", got.changed);
    got
}

/// The arguments that choose `names` (`--harness` for each of them).
fn choose(names: &[&str]) -> Vec<String> {
    names.iter().flat_map(|name| ["--harness".to_string(), (*name).to_string()]).collect()
}

fn yaml_of(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("canon.yaml")).expect("canon.yaml")
}

fn lock_providers(dir: &Path) -> Vec<String> {
    let lock: Value = serde_json::from_str(&std::fs::read_to_string(dir.join(".canon/skills/.install-lock.json")).expect("the install lock")).expect("the lock is JSON");
    lock["providers"].as_array().expect("providers").iter().map(|name| name.as_str().expect("a name").to_string()).collect()
}

// ---------------------------------------------------------------------------
// The documents.

fn docs() -> Vec<&'static Value> {
    harness::leaves()
}

fn doc_of(command: &str) -> &'static Value {
    docs().into_iter().find(|doc| doc["command"] == command).unwrap_or_else(|| panic!("`{command}` is a command"))
}

fn option_of<'a>(doc: &'a Value, name: &str) -> Option<&'a Value> {
    doc["options"].as_array().into_iter().flatten().find(|option| option["name"] == name)
}

fn at_most_one(doc: &Value) -> Vec<BTreeSet<String>> {
    doc["constraints"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|rule| rule["rule"] == "atMostOne")
        .map(|rule| rule["names"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect())
        .collect()
}

fn harness_option_mut(doc: &mut Value) -> &mut Value {
    doc["options"].as_array_mut().expect("options").iter_mut().find(|option| option["name"] == "harness").expect("--harness")
}

/// What is wrong with the declarations of the harness options in `docs`: a command that lacks one, lists other names or a name twice, declares a domain, does not repeat the
/// option, keeps the old `--providers`, or lacks the exclusion of the two options (and of `--check` for `init`).
fn harness_problems(docs: &[&Value]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut declared: BTreeSet<String> = BTreeSet::new();
    for command in COMMANDS {
        let Some(doc) = docs.iter().find(|doc| doc["command"] == command) else {
            problems.push(format!("`{command}` is not a command"));
            continue;
        };
        if option_of(doc, "providers").is_some() {
            problems.push(format!("`{command}` still declares `--providers`"));
        }
        match option_of(doc, "harness") {
            None => problems.push(format!("`{command}` has no `--harness`")),
            Some(harness) => {
                let values: Vec<&str> = harness["possibleValues"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                if values != NAMES {
                    problems.push(format!("`{command} --harness` lists {values:?}, not {NAMES:?}"));
                }
                if harness["repeatable"] != true {
                    problems.push(format!("`{command} --harness` is not repeatable"));
                }
                if harness["valueType"] != "enum" {
                    problems.push(format!("`{command} --harness` is a {}, not an enum", harness["valueType"]));
                }
                if harness.get("domain").is_some_and(|domain| !domain.is_null()) {
                    problems.push(format!("`{command} --harness` declares a domain: {}", harness["domain"]));
                }
                declared.insert(format!("{}|{}|{}|{}", harness["valueName"], harness["valueType"], harness["possibleValues"], harness["repeatable"]));
            }
        }
        match option_of(doc, "no-harness") {
            None => problems.push(format!("`{command}` has no `--no-harness`")),
            Some(switch) if switch["valueType"] != "switch" => problems.push(format!("`{command} --no-harness` is a {}, not a switch", switch["valueType"])),
            Some(_) => {}
        }
        let names: &[&str] = if command == "init" { &["check", "harness", "no-harness"] } else { &["harness", "no-harness"] };
        let wanted: BTreeSet<String> = names.iter().map(|name| (*name).to_string()).collect();
        if !at_most_one(doc).iter().any(|names| wanted.is_subset(names)) {
            problems.push(format!("`{command}` does not declare atMostOne of {wanted:?}: {}", doc["constraints"]));
        }
    }
    if declared.len() > 1 {
        problems.push(format!("the four commands declare `--harness` {} ways: {declared:?}", declared.len()));
    }
    problems
}

// ===========================================================================
// atip.provideroption.01 - 02 (F1): the repeatable option.

#[test]
fn atip_provideroption_01_harness_names_given_once_each_in_any_order_are_one_selection() {
    // `init`: two orders and a name twice are one config and one install.
    let orders: [&[&str]; 3] = [&["claude", "omp"], &["omp", "claude"], &["omp", "claude", "omp"]];
    let mut trees = Vec::new();
    for names in orders {
        let argv = choose(names);
        let refs: Vec<&str> = argv.iter().map(String::as_str).collect();
        let (dir, call) = init_fresh(&refs);
        assert_eq!(call.exit, 0, "init {names:?}: {}", call.document);
        assert!(yaml_of(dir.path()).ends_with("skills:\n  providers: [claude, omp]\n"), "init {names:?}: {}", yaml_of(dir.path()));
        assert!(dir.path().join(".claude/skills/canon/SKILL.md").is_file() && dir.path().join(".omp/skills/canon/SKILL.md").is_file());
        assert!(!dir.path().join(".agents/skills/canon").exists() && !dir.path().join(".pi/skills/canon").exists(), "only the chosen are installed");
        assert_eq!(lock_providers(dir.path()), ["claude", "omp"]);
        trees.push(harness::tree(dir.path()));
    }
    assert!(trees.windows(2).all(|pair| pair[0] == pair[1]), "the three runs wrote the same tree");
    // `skills install` on copies of the story repository: the same.
    let mut trees = Vec::new();
    for names in orders {
        let (_guard, repo) = harness::copy_of(base());
        let mut argv = vec!["skills".to_string(), "install".to_string(), "--json".to_string()];
        argv.extend(choose(names));
        let output = run_strings(&repo, &argv);
        assert!(output.status.success(), "skills install {names:?}: {}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        assert!(yaml_of(&repo).contains("providers: [claude, omp]"), "{}", yaml_of(&repo));
        assert_eq!(lock_providers(&repo), ["claude", "omp"]);
        trees.push(harness::tree(&repo));
    }
    assert!(trees.windows(2).all(|pair| pair[0] == pair[1]), "the three installs wrote the same tree");
    // Each of the four names is taken.
    for name in NAMES {
        let (dir, call) = init_fresh(&["--harness", name]);
        assert_eq!(call.exit, 0, "init {name}: {}", call.document);
        assert!(yaml_of(dir.path()).ends_with(&format!("skills:\n  providers: [{name}]\n")));
    }
}

#[test]
fn atip_provideroption_02_a_name_that_is_not_one_of_the_four_is_refused_with_its_choices_and_nothing_is_written() {
    for value in NOT_A_NAME {
        let given = format!("--harness={value}");
        let (dir, call) = init_fresh(&[&given]);
        assert_eq!((call.exit, call.code(), call.field()), (2, "USAGE", "--harness"), "init `{value}`: {}", call.document);
        assert_eq!(call.document["error"]["expected"], json!(NAMES), "init `{value}`: {}", call.document);
        assert_eq!(call.document["error"]["received"], value, "init `{value}`: {}", call.document);
        assert!(call.changed.is_empty(), "init `{value}` wrote {:?}", call.changed);
        assert!(!dir.path().join("canon.yaml").exists());
        for command in SKILLS_COMMANDS {
            let got = assert_refused(&["skills", command, &given], "USAGE", "--harness");
            assert_eq!(got.document["error"]["expected"], json!(NAMES), "skills {command} `{value}`: {}", got.document);
        }
    }
    // A name that is near one is offered back; one that is not offers none.
    let (_dir, call) = init_fresh(&["--harness", "Claude"]);
    assert_eq!(call.document["error"]["suggestions"], json!(["claude"]), "{}", call.document);
    let (_dir, call) = init_fresh(&["--harness", "gemini"]);
    assert!(call.document["error"].get("suggestions").is_none(), "{}", call.document);
    // The refusal leaves what a repository holds as it was.
    let (_guard, repo) = harness::copy_of(base());
    let before = harness::tree(&repo);
    let output = run(&repo, &["skills", "install", "--harness", "claude", "--harness", "gemini", "--json"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(harness::changed(&before, &harness::tree(&repo)), Vec::<PathBuf>::new());
}

// ===========================================================================
// atip.provideroption.03 - 04 (F2): no harness.

#[test]
fn atip_provideroption_03_no_harness_records_an_empty_list_and_installs_nothing() {
    // After an install for claude, `--no-harness` empties the list and the lock's providers.
    let (_guard, repo) = harness::copy_of(base());
    let output = run(&repo, &["skills", "install", "--harness", "claude", "--json"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(lock_providers(&repo), ["claude"]);
    let output = run(&repo, &["skills", "install", "--no-harness", "--json"]);
    assert!(output.status.success(), "{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    let data = &document(&output)["data"];
    assert_eq!((data["providers"].clone(), data["selection"].clone()), (json!([]), json!("from --harness or --no-harness")), "{data}");
    assert!(yaml_of(&repo).ends_with("skills:\n  providers: []\n"), "{}", yaml_of(&repo));
    assert_eq!(lock_providers(&repo), Vec::<String>::new(), "the lock names no harness");
    // `init` records the empty list too, whatever it detects.
    let dir = fresh();
    std::fs::create_dir_all(dir.path().join(".claude")).expect("a marker");
    let output = run(dir.path(), &["init", "--no-harness", "--json"]);
    assert!(output.status.success(), "{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    assert!(yaml_of(dir.path()).ends_with("skills:\n  providers: []\n"), "{}", yaml_of(dir.path()));
    assert!(!dir.path().join(".claude/skills").exists() && !dir.path().join(".canon/skills").exists(), "nothing is installed");
    // `init --dry-run` says the same and writes nothing.
    let dir = fresh();
    std::fs::create_dir_all(dir.path().join(".omp")).expect("a marker");
    let before = harness::tree(dir.path());
    let output = run(dir.path(), &["init", "--no-harness", "--dry-run", "--json"]);
    assert!(output.status.success());
    assert_eq!(harness::changed(&before, &harness::tree(dir.path())), Vec::<PathBuf>::new());
}

#[test]
fn atip_provideroption_04_no_harness_beside_a_harness_or_beside_check_is_refused_and_nothing_is_written() {
    for command in SKILLS_COMMANDS {
        for args in [["--no-harness", "--harness", "claude"], ["--harness", "claude", "--no-harness"]] {
            let mut argv = vec!["skills", command];
            argv.extend(args);
            assert_refused(&argv, "USAGE", "--harness");
        }
    }
    for args in [vec!["--no-harness", "--harness", "claude"], vec!["--check", "--harness", "claude"], vec!["--check", "--no-harness"]] {
        let (dir, call) = init_fresh(&args);
        assert_eq!((call.exit, call.code()), (2, "USAGE"), "init {args:?}: {}", call.document);
        assert!(call.changed.is_empty() && !dir.path().join("canon.yaml").exists(), "init {args:?} wrote {:?}", call.changed);
    }
}

// ===========================================================================
// atip.provideroption.05 - 06 (F3): the read commands.

/// The story repository with the canon skill installed for claude and omp: a canon.yaml that lists them, a lock and the two projections.
static INSTALLED: LazyLock<TempDir> = LazyLock::new(|| {
    let dir = TempDir::new().expect("a directory");
    harness::copy_dir(base(), &dir.path().join("repo"));
    let output = run(&dir.path().join("repo"), &["skills", "install", "--harness", "claude", "--harness", "omp", "--json"]);
    assert!(output.status.success(), "{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    dir
});

fn installed() -> PathBuf {
    INSTALLED.path().join("repo")
}

#[test]
fn atip_provideroption_05_check_and_doctor_serve_the_harnesses_they_are_given_and_only_those() {
    let repo = installed();
    // With no option they follow canon.yaml: claude and omp.
    let check = document(&run(&repo, &["skills", "check", "--json"]));
    assert_eq!(check["data"]["providers"], json!(["claude", "omp"]), "{check}");
    // With a name they serve that harness alone, and say where the choice came from.
    let check = document(&run(&repo, &["skills", "check", "--harness", "omp", "--json"]));
    assert_eq!(check["data"]["providers"], json!(["omp"]), "{check}");
    let statuses = check["data"]["statuses"].as_array().expect("statuses");
    assert!(!statuses.is_empty() && statuses.iter().all(|status| status["provider"] == "omp" && status["state"] == "ok"), "only omp's projection is read, and it is whole: {check}");
    let doctor = document(&run(&repo, &["skills", "doctor", "--harness", "omp", "--json"]));
    let lines: Vec<&str> = doctor["data"]["lines"].as_array().expect("lines").iter().filter_map(Value::as_str).collect();
    assert_eq!(lines[0], "providers: omp", "{doctor}");
    assert!(lines.iter().any(|line| line.starts_with("stray: ") && line.contains("claude")), "claude is not the selection, so its projection is a stray: {lines:?}");
    // A harness that was never installed is found missing.
    let check = run(&repo, &["skills", "check", "--harness", "pi", "--json"]);
    assert_eq!(check.status.code(), Some(1), "{}", String::from_utf8_lossy(&check.stdout));
    assert_eq!(document(&check)["data"]["providers"], json!(["pi"]));
    // `--no-harness` is an empty selection, in check and in doctor.
    let check = document(&run(&repo, &["skills", "check", "--no-harness", "--json"]));
    assert_eq!((check["data"]["providers"].clone(), check["data"]["statuses"].clone()), (json!([]), json!([])), "{check}");
    let doctor = document(&run(&repo, &["skills", "doctor", "--no-harness", "--json"]));
    assert_eq!(doctor["data"]["lines"][0], "providers: none", "{doctor}");
    // Neither writes anything.
    let before = harness::tree(&repo);
    run(&repo, &["skills", "check", "--harness", "codex"]);
    run(&repo, &["skills", "doctor", "--harness", "codex"]);
    assert_eq!(harness::changed(&before, &harness::tree(&repo)), Vec::<PathBuf>::new());
}

#[test]
fn atip_provideroption_06_check_and_doctor_refuse_what_install_refuses() {
    for command in ["check", "doctor"] {
        for value in NOT_A_NAME {
            let given = format!("--harness={value}");
            assert_refused(&["skills", command, &given], "USAGE", "--harness");
        }
        assert_refused(&["skills", command, "--harness", "claude", "--no-harness"], "USAGE", "--harness");
        // A list in the old spelling is a removed spelling, not a harness.
        let got = assert_refused(&["skills", command, "--providers", "claude"], "USAGE", "--providers");
        assert!(got.document["error"]["hint"].as_str().unwrap_or_default().contains("--harness claude"), "{}", got.document);
    }
}

// ===========================================================================
// atip.provideroption.07 - 08 (F4): the input document.

#[test]
fn atip_provideroption_07_an_input_document_chooses_as_the_flags_do() {
    for (document_text, flags) in [(r#"{"harness":["claude","omp"]}"#, vec!["--harness", "claude", "--harness", "omp"]), (r#"{"noHarness":true}"#, vec!["--no-harness"]), (r#"{"harness":["pi"]}"#, vec!["--harness", "pi"])] {
        // init
        let by_flags = init_fresh(&flags);
        let dir = fresh();
        let output = run(dir.path(), &["init", "--input", document_text, "--json"]);
        assert!(output.status.success(), "init --input {document_text}: {}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        assert_eq!(by_flags.1.exit, 0);
        assert_eq!(harness::tree(dir.path()), harness::tree(by_flags.0.path()), "init --input {document_text} wrote what {flags:?} write");
        // skills install
        let (_a, flagged) = harness::copy_of(base());
        let (_b, inputted) = harness::copy_of(base());
        let mut argv = vec!["skills".to_string(), "install".to_string(), "--json".to_string()];
        argv.extend(flags.iter().map(|flag| (*flag).to_string()));
        assert!(run_strings(&flagged, &argv).status.success());
        let output = run(&inputted, &["skills", "install", "--input", document_text, "--json"]);
        assert!(output.status.success(), "skills install --input {document_text}: {}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        assert_eq!(harness::tree(&inputted), harness::tree(&flagged), "skills install --input {document_text} wrote what {flags:?} write");
    }
    // A batch of documents chooses for each element.
    let dir = fresh();
    let output = run(dir.path(), &["init", "--input", r#"[{"harness":["omp"]}]"#, "--json"]);
    assert!(output.status.success(), "{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    assert!(yaml_of(dir.path()).ends_with("skills:\n  providers: [omp]\n"));
}

#[test]
fn atip_provideroption_08_an_input_document_that_gives_none_a_name_that_is_none_or_the_old_key_is_refused_and_nothing_is_recorded() {
    for (text, code, field) in [
        (r#"{"harness":[]}"#, "INPUT_INVALID", "/harness"),
        (r#"{"harness":["none"]}"#, "INPUT_INVALID", "/harness/0"),
        (r#"{"harness":["Claude"]}"#, "INPUT_INVALID", "/harness/0"),
        (r#"{"harness":"claude"}"#, "INPUT_INVALID", "/harness"),
        (r#"{"providers":"claude"}"#, "INPUT_INVALID", "/providers"),
        (r#"{"noHarness":"yes"}"#, "INPUT_INVALID", "/noHarness"),
        (r#"{"harness":["claude"],"noHarness":true}"#, "USAGE", "/harness"),
    ] {
        let (dir, call) = {
            let dir = fresh();
            let before = harness::tree(dir.path());
            let output = run(dir.path(), &["init", "--input", text, "--json"]);
            let changed = harness::changed(&before, &harness::tree(dir.path()));
            (dir, Call { exit: output.status.code().unwrap_or(-1), document: document(&output), changed })
        };
        assert_eq!((call.exit, call.code(), call.field()), (2, code, field), "init --input {text}: {}", call.document);
        assert!(call.changed.is_empty() && !dir.path().join("canon.yaml").exists(), "init --input {text} wrote {:?}", call.changed);
        let got = assert_refused(&["skills", "install", "--input", text], code, field);
        assert!(got.changed.is_empty());
    }
    // The old key is a key no command has; no key of the command is near it, and `expected` names the keys it takes.
    let got = on_story(&["skills", "install", "--input", r#"{"providers":"claude"}"#]);
    let expected: Vec<&str> = got.document["error"]["expected"].as_array().expect("expected keys").iter().filter_map(Value::as_str).collect();
    assert!(expected.contains(&"harness") && expected.contains(&"noHarness") && !expected.contains(&"providers"), "{}", got.document);
    // A flag and a key that disagree are `INPUT_CONFLICT`.
    let got = assert_refused(&["skills", "install", "--harness", "claude", "--input", r#"{"harness":["omp"]}"#], "INPUT_CONFLICT", "/harness");
    assert!(got.changed.is_empty());
}

// ===========================================================================
// atip.provideroption.09 - 11 (F5): the old spelling.

/// The command line a hint offers: the text between the first pair of backticks, without its `canon`.
fn hinted_line(hint: &str) -> Vec<String> {
    let line = hint.split('`').nth(1).unwrap_or_else(|| panic!("a command line in the hint: {hint}"));
    line.strip_prefix("canon ").unwrap_or_else(|| panic!("a canon command line: {line}")).split(' ').map(str::to_string).collect()
}

#[test]
fn atip_provideroption_09_the_old_providers_is_refused_with_a_hint_whose_line_runs() {
    // init: a comma list, the `=` form, and none.
    for (typed, expected_line) in [
        (vec!["init", "--providers", "claude,omp", "--json"], "init --harness claude --harness omp --json"),
        (vec!["init", "--providers=omp,claude", "--json"], "init --harness omp --harness claude --json"),
        (vec!["init", "--providers", "none", "--json"], "init --no-harness --json"),
        (vec!["init", "--providers", "pi,pi", "--json"], "init --harness pi --harness pi --json"),
    ] {
        let dir = fresh();
        let before = harness::tree(dir.path());
        let output = run(dir.path(), &typed);
        let doc = document(&output);
        assert_eq!((output.status.code(), doc["error"]["code"].as_str(), doc["error"]["field"].as_str()), (Some(2), Some("USAGE"), Some("--providers")), "{typed:?}: {doc}");
        assert_eq!(harness::changed(&before, &harness::tree(dir.path())), Vec::<PathBuf>::new(), "{typed:?} wrote");
        let hint = doc["error"]["hint"].as_str().unwrap_or_default();
        let line = hinted_line(hint);
        assert_eq!(line.join(" "), expected_line, "{typed:?}: {hint}");
        // The corrected line, as the hint gives it, runs.
        let output = run_strings(dir.path(), &line);
        assert!(output.status.success(), "`canon {}`: {}{}", line.join(" "), String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        assert!(dir.path().join("canon.yaml").is_file());
    }
    // The three skills commands, with the other options kept in place.
    for command in SKILLS_COMMANDS {
        let mut typed = vec!["skills", command, "--providers", "codex,pi"];
        if command == "install" {
            typed.push("--dry-run");
        }
        typed.push("--json");
        let (_guard, repo) = harness::copy_of(base());
        let output = run(&repo, &typed);
        let doc = document(&output);
        assert_eq!((output.status.code(), doc["error"]["code"].as_str(), doc["error"]["field"].as_str()), (Some(2), Some("USAGE"), Some("--providers")), "{typed:?}: {doc}");
        let line = hinted_line(doc["error"]["hint"].as_str().unwrap_or_default());
        assert!(line.join(" ").starts_with(&format!("skills {command} --harness codex --harness pi")), "{line:?}");
        let output = run_strings(&repo, &line);
        assert!(output.status.code().is_some_and(|code| code != 2), "`canon {}` is no longer a usage error: {}", line.join(" "), String::from_utf8_lossy(&output.stdout));
    }
}

#[test]
fn atip_provideroption_10_a_providers_value_the_old_spelling_did_not_take_is_refused_and_the_hint_names_the_options_only() {
    for value in ["Claude", "claude,", "claude, pi", "gemini", "none,claude", "NONE", ""] {
        let given = format!("--providers={value}");
        for command in [vec!["init"], vec!["skills", "install"], vec!["skills", "check"], vec!["skills", "doctor"]] {
            let mut argv = command.clone();
            argv.push(&given);
            argv.push("--json");
            let (_guard, repo) = harness::copy_of(base());
            let before = harness::tree(&repo);
            let output = run(&repo, &argv);
            let doc = document(&output);
            assert_eq!((output.status.code(), doc["error"]["code"].as_str(), doc["error"]["field"].as_str()), (Some(2), Some("USAGE"), Some("--providers")), "{argv:?}: {doc}");
            assert_eq!(harness::changed(&before, &harness::tree(&repo)), Vec::<PathBuf>::new());
            let hint = doc["error"]["hint"].as_str().unwrap_or_default();
            assert!(hint.contains("`--harness`") && hint.contains("`--no-harness`"), "{argv:?}: {hint}");
            assert!(!hint.contains("canon "), "no command line is offered for a value that was no list: {hint}");
            assert_eq!(doc["error"]["expected"], json!(["--harness"]), "{doc}");
        }
    }
}

// ===========================================================================
// atip.provideroption.12 - 13 (F6): the declarations.

#[test]
fn atip_provideroption_12_the_four_commands_declare_one_repeatable_enumeration_and_the_values_agree_with_the_parser() {
    let docs = docs();
    let problems = harness_problems(&docs);
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
    // The names are in `possibleValues` once, and no command declares a pattern for them.
    for command in COMMANDS {
        let harness = option_of(doc_of(command), "harness").expect("--harness");
        assert_eq!(harness["possibleValues"], json!(NAMES), "{command}");
        assert!(harness.get("domain").is_none(), "{command}: an enumeration has no pattern: {harness}");
        // The `--input` fragment of the two commands that write says the same: `harness` is an array of the names, `noHarness` a boolean, and no other key is taken.
        let mut argv: Vec<&str> = vec!["schema"];
        argv.extend(command.split(' '));
        argv.extend(["--schemas", "--json"]);
        let schemas = document(&run(base(), &argv));
        let input = &schemas["data"]["schemas"]["Input"];
        if command.starts_with("skills c") || command == "skills doctor" {
            assert!(input.is_null(), "{command} is a read and takes no document: {input}");
            continue;
        }
        assert_eq!(input["properties"]["harness"], json!({ "type": "array", "items": { "enum": NAMES } }), "{command}: {input}");
        assert_eq!(input["properties"]["noHarness"], json!({ "type": "boolean" }), "{command}: {input}");
        assert!(input["properties"].get("providers").is_none() && input["additionalProperties"] == false, "{command}: {input}");
    }
    // Every value of a table is judged the way the declaration says: a name of `possibleValues` is taken by the parser, any other is refused for `--harness`.
    let values: [&str; 10] = ["claude", "codex", "omp", "pi", "Claude", "none", "", "claude,omp", " pi", "gemini"];
    for command in COMMANDS {
        let declared: Vec<String> = option_of(doc_of(command), "harness").expect("--harness")["possibleValues"].as_array().expect("possibleValues").iter().map(|name| name.as_str().expect("a name").to_string()).collect();
        for value in values {
            let given = format!("--harness={value}");
            let mut argv: Vec<&str> = command.split(' ').collect();
            argv.push(&given);
            if command == "skills install" || command == "init" {
                argv.push("--dry-run");
            }
            argv.push("--json");
            let (_guard, repo) = harness::copy_of(base());
            let output = run(&repo, &argv);
            let doc = document(&output);
            let refused_for_option = output.status.code() == Some(2) && doc["error"]["field"] == "--harness";
            assert_eq!(!refused_for_option, declared.iter().any(|name| name == value), "`canon {}` and {value:?}: {doc}", argv.join(" "));
        }
    }
}

#[test]
fn atip_provideroption_13_a_command_that_declares_other_names_a_pattern_or_the_old_option_fails_the_check() {
    let real: Vec<Value> = docs().into_iter().cloned().collect();
    let seed = |command: &str, change: &dyn Fn(&mut Value)| -> Vec<String> {
        let mut seeded = real.clone();
        let doc = seeded.iter_mut().find(|doc| doc["command"] == command).expect("a command");
        change(doc);
        let refs: Vec<&Value> = seeded.iter().collect();
        harness_problems(&refs)
    };
    let found = seed("skills check", &|doc| harness_option_mut(doc)["possibleValues"] = json!(["claude", "codex", "omp", "pi", "gemini"]));
    assert!(!found.is_empty(), "{found:?}");
    let found = seed("init", &|doc| harness_option_mut(doc)["possibleValues"] = json!(["claude", "claude", "codex", "omp", "pi"]));
    assert!(!found.is_empty(), "{found:?}");
    let found = seed("skills doctor", &|doc| harness_option_mut(doc)["domain"] = json!({ "pattern": "^(claude|codex|omp|pi)(,(claude|codex|omp|pi))*$" }));
    assert!(!found.is_empty(), "{found:?}");
    let found = seed("skills install", &|doc| harness_option_mut(doc)["repeatable"] = json!(false));
    assert!(!found.is_empty(), "{found:?}");
    let found = seed("skills install", &|doc| doc["options"].as_array_mut().expect("options").push(json!({ "name": "providers", "valueType": "string" })));
    assert!(!found.is_empty(), "{found:?}");
    let found = seed("skills check", &|doc| doc["options"].as_array_mut().expect("options").retain(|option| option["name"] != "no-harness"));
    assert!(!found.is_empty(), "{found:?}");
    let found = seed("init", &|doc| doc["constraints"] = json!([]));
    assert!(!found.is_empty(), "{found:?}");
    let refs: Vec<&Value> = real.iter().collect();
    assert!(harness_problems(&refs).is_empty());
}
