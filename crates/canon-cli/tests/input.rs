//! T16 — `--input <JSON|FILE|->` (0.28 D7): one JSON document, or a batch of
//! them, runs a leaf exactly as its flags would. Every test drives the built
//! `canon` binary; none reads source text or help text.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::{json, Value};
use tempfile::TempDir;

// ---------------------------------------------------------------- harness

fn canon(dir: &Path, home: &Path, args: &[&str], stdin: Option<&str>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(args)
        .current_dir(dir)
        .env("CANON_ACTOR", "input-test")
        .env_remove("CANON_ROLE")
        .env("HOME", home)
        .env("CANON_SKILLS_SOURCE", concat!(env!("CARGO_MANIFEST_DIR"), "/../../canon/skills"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn canon");
    let mut pipe = child.stdin.take().expect("stdin");
    if let Some(text) = stdin {
        pipe.write_all(text.as_bytes()).expect("write stdin");
    }
    drop(pipe);
    child.wait_with_output().expect("canon runs")
}

fn json_of(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("stdout is one JSON document ({e}): {}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}

fn ok(dir: &Path, home: &Path, args: &[&str]) {
    let out = canon(dir, home, args, None);
    assert!(out.status.success(), "{args:?}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}

/// A repo with the story the examples tell: subject `demo`, change `add-login`, scenario
/// `auth.login.01` (and `auth.login.02` when `second`), finding 1.
fn fixture(home: &Path, second: bool) -> TempDir {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    ok(repo, home, &["init", "--providers", "none"]);
    ok(repo, home, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(repo, home, &["change", "new", "add-login", "--subject-id", "demo", "--title", "Add login"]);
    ok(repo, home, &["feature", "new", "auth.login", "--title", "Login"]);
    let feature = ".canon/specs/kind=feature/area=auth/login.feature";
    ok(repo, home, &["scenario", "new", "auth.login.01", "--title", "Sign in", "--feature", feature]);
    if second {
        ok(repo, home, &["scenario", "new", "auth.login.02", "--title", "Sign out", "--feature", feature]);
    }
    ok(repo, home, &["inventory", "sync"]);
    ok(repo, home, &["finding", "add", "--change-id", "add-login", "--round", "1", "--seq", "1", "--severity", "note", "--actor-id", "bob", "--summary", "Naming nit"]);
    dir
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// `text` with every RFC 3339 timestamp, ULID and long hex digest replaced, so two runs that
/// minted different times and ids compare equal when they wrote the same thing.
fn normalized(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let is_ulid = |c: char| c.is_ascii_digit() || c.is_ascii_uppercase();
    let mut i = 0;
    while i < chars.len() {
        let rest = &chars[i..];
        // 2030-01-01T00:00:00(.fff)Z, also with a `+hh:mm` offset
        let stamp = rest.len() >= 19 && rest[..4].iter().all(char::is_ascii_digit) && rest[4] == '-' && rest[10] == 'T' && rest[13] == ':';
        if stamp {
            let mut j = 19;
            while j < rest.len() && (rest[j].is_ascii_digit() || matches!(rest[j], '.' | 'Z' | '+' | ':' | '-')) {
                j += 1;
            }
            out.push_str("<time>");
            i += j;
            continue;
        }
        let word = rest.iter().take_while(|c| c.is_ascii_alphanumeric()).count();
        let boundary = i == 0 || !chars[i - 1].is_ascii_alphanumeric();
        if boundary && word == 26 && rest[..26].iter().all(|c| is_ulid(*c)) && rest[0] == '0' {
            out.push_str("<ulid>");
            i += 26;
            continue;
        }
        if boundary && (word >= 40 || word == 12) && rest[..word].iter().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()) && rest[..word].iter().any(char::is_ascii_digit) {
            out.push_str("<hex>");
            i += word;
            continue;
        }
        if boundary && rest.iter().take_while(|c| c.is_ascii_digit()).count() >= 10 {
            out.push_str("<n>");
            i += rest.iter().take_while(|c| c.is_ascii_digit()).count();
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Every file of the repo (not `.git`) by relative path, the repo's own path in the text replaced.
fn tree(root: &Path) -> BTreeMap<String, String> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            // the git directory and the hot tier's SQLite files (page-level state, never records)
            if rel == ".git" || rel.starts_with(".canon/hot.db") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let body = std::fs::read(&path).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
                out.insert(rel, body.replace(root.to_string_lossy().as_ref(), "<repo>"));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// [`tree`] with the timestamps, ids and digests of two runs made alike.
fn normalized_tree(root: &Path) -> BTreeMap<String, String> {
    tree(root).into_iter().map(|(path, body)| (normalized(&path), normalized(&body))).collect()
}

fn scratch_copy(base: &Path) -> (TempDir, PathBuf) {
    let scratch = TempDir::new().unwrap();
    let repo = scratch.path().join("repo");
    copy_dir(base, &repo);
    (scratch, repo)
}

fn spec_schemas() -> Value {
    canon_cli::spec::generate()["schemas"].clone()
}

fn validates(schemas: &Value, name: &str, instance: &Value) {
    let schema = schemas.get(name).unwrap_or_else(|| panic!("the spec has no schema `{name}`"));
    let validator = jsonschema::validator_for(schema).unwrap_or_else(|e| panic!("`{name}` does not compile: {e}"));
    let problems: Vec<String> = validator.iter_errors(instance).map(|e| format!("{e} at {}", e.instance_path())).collect();
    assert!(problems.is_empty(), "{name}: {problems:?}\n{instance}");
}

/// The spec node of a leaf, from the binary.
fn node(dir: &Path, home: &Path, leaf: &str) -> Value {
    let mut args: Vec<&str> = vec!["schema"];
    args.extend(leaf.split(' '));
    args.push("--json");
    json_of(&canon(dir, home, &args, None))["data"].clone()
}

fn write_leaves(dir: &Path, home: &Path) -> Vec<String> {
    let listing = json_of(&canon(dir, home, &["schema", "--json"], None));
    listing["data"]["commands"].as_array().unwrap().iter().filter(|c| c["effect"] != "read").map(|c| c["command"].as_str().unwrap().to_string()).collect()
}

// ---------------------------------------------------------------- the spec

#[test]
fn every_writing_leaf_takes_input_and_no_read_leaf_does() {
    let home = TempDir::new().unwrap();
    let dir = TempDir::new().unwrap();
    let listing = json_of(&canon(dir.path(), home.path(), &["schema", "--json"], None));
    let mut writers = 0;
    let schemas = spec_schemas();
    for command in listing["data"]["commands"].as_array().unwrap() {
        let leaf = command["command"].as_str().unwrap();
        let detail = node(dir.path(), home.path(), leaf);
        let has_input = detail["options"].as_array().unwrap().iter().any(|o| o["name"] == "input");
        let writes = detail["effect"] != "read";
        assert_eq!(has_input, writes, "`{leaf}`: --input belongs to exactly the writing leaves");
        let inputs = detail["inputs"].as_array().unwrap();
        assert_eq!(inputs.len(), usize::from(writes), "`{leaf}`: one input per writing leaf");
        if !writes {
            continue;
        }
        writers += 1;
        let input = &inputs[0];
        assert_eq!(input["maxBytes"], 8 * 1024 * 1024, "{leaf}");
        let name = input["schemaRef"].as_str().unwrap().trim_start_matches("#/schemas/");
        validates(&schemas, name, &input["example"]);
        // The batch variant is declared exactly where a batch is accepted.
        let outputs = detail["outputs"].as_array().unwrap();
        let batch: Vec<&Value> = outputs.iter().filter(|o| o["batch"] == true).collect();
        let destructive = detail["effect"] == "destructive";
        let stream = outputs.iter().any(|o| o["kind"] == "stream");
        assert_eq!(input["batch"], !(destructive || stream), "{leaf}");
        assert_eq!(batch.len(), usize::from(!(destructive || stream)), "{leaf}: {outputs:?}");
        for variant in batch {
            let report = variant["report"].as_str().unwrap();
            assert!(report.starts_with("Batch") && report.ends_with("Report"), "{report}");
            assert_eq!(variant["schemaVersion"], "1.0");
            assert!(schemas.get(report).is_some(), "{report} is published");
        }
    }
    assert_eq!(writers, 47, "every leaf whose effect is write or destructive");
    assert_eq!(write_leaves(dir.path(), home.path()).len(), writers);
}

#[test]
fn the_published_batch_examples_are_valid_documents() {
    let home = TempDir::new().unwrap();
    let dir = TempDir::new().unwrap();
    let schemas = spec_schemas();
    let mut seen = Vec::new();
    for leaf in write_leaves(dir.path(), home.path()) {
        let input = node(dir.path(), home.path(), &leaf)["inputs"][0].clone();
        let name = input["schemaRef"].as_str().unwrap().trim_start_matches("#/schemas/").to_string();
        validates(&schemas, &name, &input["example"]);
        if let Some(batch) = input.get("batchExample") {
            seen.push(leaf.clone());
            for element in batch.as_array().expect("a batch example is an array") {
                validates(&schemas, &name, element);
            }
        }
    }
    seen.sort();
    assert_eq!(seen, ["divergence add", "evidence add", "finding add", "scenario new", "subject new"]);
}

// ---------------------------------------------------------------- the example documents equal the flags

/// Leaves with no run to compare: a server that never returns.
const NOT_RUN: &[&str] = &["dashboard"];

#[test]
fn every_spec_example_document_writes_what_its_flags_write() {
    let home = TempDir::new().unwrap();
    let base = fixture(home.path(), false);
    // Every run happens at the same path, so a record that embeds its repo path matches across runs.
    let work = TempDir::new().unwrap();
    let mut problems = Vec::new();
    for leaf in write_leaves(base.path(), home.path()) {
        if NOT_RUN.contains(&leaf.as_str()) {
            continue;
        }
        let detail = node(base.path(), home.path(), &leaf);
        let example = detail["examples"][0].as_str().unwrap().to_string();
        let document = detail["inputs"][0]["example"].to_string();
        let mut flags: Vec<String> = split_words(&example).into_iter().skip(1).collect();
        flags.push("--json".into());
        let mut words: Vec<String> = leaf.split(' ').map(str::to_string).collect();
        // `--repo` and `--as` are flags only: they stay on the command line beside `--input`
        for pair in flags.windows(2) {
            if pair[0] == "--repo" || pair[0] == "--as" {
                words.extend([pair[0].clone(), pair[1].clone()]);
            }
        }

        // The example run on its own copy of the repo; `how` says how its arguments arrive.
        let run = |how: How| -> (Option<i32>, Value, BTreeMap<String, String>) {
            let repo = work.path().join("repo");
            let _ = std::fs::remove_dir_all(&repo);
            if leaf == "init" {
                std::fs::create_dir_all(&repo).unwrap();
            } else {
                copy_dir(base.path(), &repo);
            }
            let own_home = TempDir::new().unwrap();
            let doc_file = work.path().join("doc.json");
            std::fs::write(&doc_file, &document).unwrap();
            let (argv, stdin): (Vec<String>, Option<&str>) = match how {
                How::Flags => (flags.clone(), None),
                How::Inline => ([words.clone(), vec!["--input".into(), document.clone(), "--json".into()]].concat(), None),
                How::File => ([words.clone(), vec!["--input".into(), doc_file.to_string_lossy().into_owned(), "--json".into()]].concat(), None),
                How::Stdin => ([words.clone(), vec!["--input".into(), "-".into(), "--json".into()]].concat(), Some(document.as_str())),
            };
            let out = canon(&repo, own_home.path(), &argv.iter().map(String::as_str).collect::<Vec<_>>(), stdin);
            (out.status.code(), serde_json::from_slice(&out.stdout).unwrap_or(Value::Null), normalized_tree(&repo))
        };

        let by_flags = run(How::Flags);
        let inline = run(How::Inline);
        let by_file = run(How::File);
        let by_stdin = run(How::Stdin);
        for (how, got) in [("inline", &inline), ("file", &by_file), ("stdin", &by_stdin)] {
            let code = |r: &(Option<i32>, Value, BTreeMap<String, String>)| r.1["error"]["code"].as_str().unwrap_or("").to_string();
            if got.0 != by_flags.0 || code(got) != code(&by_flags) || got.1["ok"] != by_flags.1["ok"] {
                problems.push(format!("{leaf}: --input {how} exited {:?} ({}), the flags {:?} ({})", got.0, code(got), by_flags.0, code(&by_flags)));
            } else if got.2 != by_flags.2 {
                let differing: Vec<&String> = got.2.keys().chain(by_flags.2.keys()).filter(|k| got.2.get(*k) != by_flags.2.get(*k)).collect();
                problems.push(format!("{leaf}: --input {how} wrote different records than the flags: {differing:?}"));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[derive(Clone, Copy)]
enum How {
    Flags,
    Inline,
    File,
    Stdin,
}

fn split_words(line: &str) -> Vec<String> {
    let (mut out, mut current, mut quoted, mut any) = (Vec::new(), String::new(), false, false);
    for c in line.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any {
                    out.push(std::mem::take(&mut current));
                    any = false;
                }
            }
            c => {
                current.push(c);
                any = true;
            }
        }
    }
    if any {
        out.push(current);
    }
    out
}

// ---------------------------------------------------------------- framing

fn init_repo(home: &Path) -> TempDir {
    let dir = TempDir::new().unwrap();
    ok(dir.path(), home, &["init", "--providers", "none"]);
    dir
}

fn subjects(repo: &Path) -> Vec<String> {
    let mut found: Vec<String> = tree(repo).into_iter().filter(|(k, _)| k.contains("kind=subject")).map(|(k, _)| k).collect();
    found.sort();
    found
}

#[test]
fn an_object_is_one_invocation_and_an_array_or_ndjson_is_a_batch() {
    let home = TempDir::new().unwrap();
    let dir = init_repo(home.path());
    let repo = dir.path();
    let schemas = spec_schemas();

    // an object: the ordinary report
    let out = canon(repo, home.path(), &["subject", "new", "--input", r#"{"operands":["alpha"],"domain":"dev","title":"Alpha"}"#, "--json"], None);
    assert_eq!(out.status.code(), Some(0));
    let body = json_of(&out);
    assert_eq!(body["data"]["subject_id"], "alpha", "{body}");
    assert!(body["data"].get("items").is_none());
    assert_eq!(body["meta"]["command"], "subject new");

    // an array, even with one element: the batch report
    let out = canon(repo, home.path(), &["subject", "new", "--input", r#"[{"operands":["beta"],"domain":"dev","title":"Beta"}]"#, "--json"], None);
    assert_eq!(out.status.code(), Some(0));
    let body = json_of(&out);
    assert_eq!(body["data"]["items"].as_array().unwrap().len(), 1, "{body}");
    assert_eq!((body["data"]["items"][0]["index"].as_u64(), body["data"]["items"][0]["status"].as_str()), (Some(0), Some("written")));
    assert_eq!(body["meta"]["command"], "subject new", "the batch keeps the leaf id");
    validates(&schemas, "BatchSubjectNewReport", &body["data"]);

    // NDJSON in a file: comments and blank lines skipped
    std::fs::write(repo.join("batch.ndjson"), "# subjects\n\n{\"operands\":[\"gamma\"],\"domain\":\"dev\",\"title\":\"Gamma\"}\n\n# more\n{\"operands\":[\"delta\"],\"domain\":\"dev\",\"title\":\"Delta\"}\n").unwrap();
    let out = canon(repo, home.path(), &["subject", "new", "--input", "batch.ndjson", "--json"], None);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stdout));
    let body = json_of(&out);
    assert_eq!(body["data"]["items"].as_array().unwrap().iter().map(|i| i["status"].as_str().unwrap()).collect::<Vec<_>>(), ["written", "written"]);
    validates(&schemas, "BatchSubjectNewReport", &body["data"]);

    // NDJSON on stdin
    let out = canon(repo, home.path(), &["subject", "new", "--input", "-", "--json"], Some("{\"operands\":[\"epsilon\"],\"domain\":\"dev\",\"title\":\"E\"}\n{\"operands\":[\"zeta\"],\"domain\":\"dev\",\"title\":\"Z\"}\n"));
    assert_eq!(json_of(&out)["data"]["items"].as_array().unwrap().len(), 2);

    // a file holding one object is one JSON value: the ordinary report
    std::fs::write(repo.join("one.json"), "{\"operands\":[\"eta\"],\"domain\":\"dev\",\"title\":\"Eta\"}\n").unwrap();
    let body = json_of(&canon(repo, home.path(), &["subject", "new", "--input", "one.json", "--json"], None));
    assert_eq!(body["data"]["subject_id"], "eta", "{body}");
    assert_eq!(subjects(repo).len(), 7);
}

#[test]
fn text_that_is_neither_one_json_value_nor_ndjson_is_input_invalid() {
    let home = TempDir::new().unwrap();
    let dir = init_repo(home.path());
    let repo = dir.path();
    std::fs::write(repo.join("bad.txt"), "this is not json\n").unwrap();
    std::fs::write(repo.join("half.ndjson"), "{\"operands\":[\"a\"],\"domain\":\"dev\",\"title\":\"A\"}\nnot json\n").unwrap();
    std::fs::write(repo.join("empty.ndjson"), "# only a comment\n\n").unwrap();
    let before = tree(repo);
    for (what, args, stdin, expected) in [
        ("inline text that is not JSON", vec!["--input", "{not json"], None, "INPUT_INVALID"),
        ("a file that is not JSON", vec!["--input", "bad.txt"], None, "INPUT_INVALID"),
        ("an NDJSON file with a bad line", vec!["--input", "half.ndjson"], None, "INPUT_INVALID"),
        ("an NDJSON file with no line", vec!["--input", "empty.ndjson"], None, "INPUT_INVALID"),
        ("stdin that is not JSON", vec!["--input", "-"], Some("nope"), "INPUT_INVALID"),
        ("an array whose element is not an object", vec!["--input", "[1]"], None, "INPUT_INVALID_TYPE"),
        ("a path that is no file", vec!["--input", "7"], None, "INPUT_UNREADABLE"),
    ] {
        let mut argv = vec!["subject", "new"];
        argv.extend(args.iter().copied());
        argv.push("--json");
        let out = canon(repo, home.path(), &argv, stdin);
        let body = json_of(&out);
        assert_eq!((out.status.code(), body["error"]["code"].as_str()), (Some(2), Some(expected)), "{what}: {body}");
    }
    let after = tree(repo);
    let differing: Vec<&String> = after.keys().chain(before.keys()).filter(|k| after.get(*k) != before.get(*k)).collect();
    assert!(differing.is_empty(), "a refused document writes nothing: {differing:?}");
}

// ---------------------------------------------------------------- refusals

#[test]
fn unknown_keys_null_conflicts_and_the_flags_only_options_are_refused() {
    let home = TempDir::new().unwrap();
    let dir = init_repo(home.path());
    let repo = dir.path();
    let before = tree(repo);
    let run = |args: &[&str], stdin: Option<&str>| {
        let mut argv = vec!["subject", "new"];
        argv.extend_from_slice(args);
        argv.push("--json");
        let out = canon(repo, home.path(), &argv, stdin);
        (out.status.code(), json_of(&out))
    };

    let (code, body) = run(&["--input", r#"{"operands":["a"],"domain":"dev","title":"A","titl":"x"}"#], None);
    assert_eq!(code, Some(2));
    assert_eq!((body["error"]["code"].as_str(), body["error"]["field"].as_str()), (Some("INPUT_UNKNOWN_FIELD"), Some("/titl")), "{body}");
    let expected: Vec<&str> = body["error"]["expected"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    assert!(expected.contains(&"title") && expected.contains(&"operands") && !expected.contains(&"repo"), "{expected:?}");
    assert!(body["error"]["suggestions"].as_array().unwrap().iter().any(|s| s == "title"), "{body}");

    for key in ["repo", "as", "json", "commit", "planDigest"] {
        let doc = format!(r#"{{"operands":["a"],"domain":"dev","title":"A","{key}":"x"}}"#);
        let (code, body) = run(&["--input", &doc], None);
        assert_eq!((code, body["error"]["code"].as_str(), body["error"]["field"].as_str()), (Some(2), Some("INPUT_UNKNOWN_FIELD"), Some(format!("/{key}").as_str())), "{key}: {body}");
    }

    let (code, body) = run(&["--input", r#"{"operands":["a"],"domain":"dev","title":null}"#], None);
    assert_eq!((code, body["error"]["code"].as_str(), body["error"]["field"].as_str()), (Some(2), Some("INPUT_INVALID_TYPE"), Some("/title")), "{body}");
    let (code, body) = run(&["--input", r#"{"operands":["a"],"domain":"dev","title":["x"]}"#], None);
    assert_eq!((code, body["error"]["code"].as_str()), (Some(2), Some("INPUT_INVALID_TYPE")), "{body}");
    let (code, body) = run(&["--input", r#"{"operands":"a","domain":"dev","title":"A"}"#], None);
    assert_eq!((code, body["error"]["code"].as_str(), body["error"]["field"].as_str()), (Some(2), Some("INPUT_INVALID_TYPE"), Some("/operands")), "{body}");

    // a flag and the same key: different is refused, equal is fine
    let (code, body) = run(&["--title", "Other", "--input", r#"{"operands":["a"],"domain":"dev","title":"A"}"#], None);
    assert_eq!((code, body["error"]["code"].as_str(), body["error"]["field"].as_str()), (Some(2), Some("INPUT_CONFLICT"), Some("/title")), "{body}");
    let (code, body) = run(&["a", "--input", r#"{"operands":["b"],"domain":"dev","title":"A"}"#], None);
    assert_eq!((code, body["error"]["code"].as_str(), body["error"]["field"].as_str()), (Some(2), Some("INPUT_CONFLICT"), Some("/operands")), "{body}");
    assert_eq!(tree(repo), before, "every refusal leaves the tree unchanged");
    let (code, body) = run(&["--title", "A", "a", "--input", r#"{"operands":["a"],"domain":"dev","title":"A"}"#], None);
    assert_eq!(code, Some(0), "{body}");
    assert_eq!(body["data"]["subject_id"], "a");

    // clap's own rules apply: a missing required option, pointed at its document key
    let (code, body) = run(&["--input", r#"{"operands":["c"],"title":"C"}"#], None);
    assert_eq!((code, body["error"]["code"].as_str(), body["error"]["field"].as_str()), (Some(2), Some("USAGE"), Some("/domain")), "{body}");
    // and so does a value set
    let out = canon(repo, home.path(), &["finding", "add", "--input", r#"{"changeId":"add-login","round":1,"seq":1,"severity":"urgent","actorId":"a","summary":"s"}"#, "--json"], None);
    let body = json_of(&out);
    assert_eq!((out.status.code(), body["error"]["code"].as_str(), body["error"]["field"].as_str()), (Some(2), Some("USAGE"), Some("/severity")), "{body}");
    assert!(body["error"]["expected"].as_array().is_some_and(|e| e.iter().any(|v| v == "blocker")), "{body}");
}

#[test]
fn stdin_conflicts_and_oversize_documents_are_refused() {
    let home = TempDir::new().unwrap();
    let base = fixture(home.path(), false);
    let repo = base.path();
    let before = tree(repo);

    let out = canon(repo, home.path(), &["evidence", "add", "--input", "-", "--report", "junit:-", "--json"], Some(r#"{"scenarioId":"auth.login.01","class":"test-run","ref":"x","role":"implementer"}"#));
    let body = json_of(&out);
    assert_eq!((out.status.code(), body["error"]["code"].as_str()), (Some(2), Some("INPUT_STDIN_CONFLICT")), "{body}");

    // over 8 MiB: refused from the file's size, before it is read
    let big = repo.join("big.ndjson");
    let line = format!("{{\"operands\":[\"x\"],\"domain\":\"dev\",\"title\":\"{}\"}}\n", "t".repeat(1000));
    std::fs::write(&big, line.repeat(8 * 1024 * 1024 / line.len() + 2)).unwrap();
    let out = canon(repo, home.path(), &["subject", "new", "--input", "big.ndjson", "--json"], None);
    let body = json_of(&out);
    assert_eq!((out.status.code(), body["error"]["code"].as_str()), (Some(2), Some("INPUT_TOO_LARGE")), "{body}");
    std::fs::remove_file(&big).unwrap();
    // and the same from stdin
    let out = canon(repo, home.path(), &["subject", "new", "--input", "-", "--json"], Some(&line.repeat(8 * 1024 * 1024 / line.len() + 2)));
    assert_eq!(json_of(&out)["error"]["code"], "INPUT_TOO_LARGE");

    // more than 10 000 elements
    let many = format!("[{}]", vec!["{}"; 10_001].join(","));
    let out = canon(repo, home.path(), &["subject", "new", "--input", &many, "--json"], None);
    assert_eq!((out.status.code(), json_of(&out)["error"]["code"].as_str().map(str::to_string)), (Some(2), Some("INPUT_TOO_LARGE".to_string())));
    let out = canon(repo, home.path(), &["subject", "new", "--input", "-", "--json"], Some(&"{}\n".repeat(10_001)));
    assert_eq!(json_of(&out)["error"]["code"], "INPUT_TOO_LARGE");
    assert_eq!(tree(repo), before);
}

#[test]
fn a_value_that_starts_with_a_dash_survives_and_is_never_parsed_as_a_flag() {
    let home = TempDir::new().unwrap();
    let dir = init_repo(home.path());
    let out = canon(dir.path(), home.path(), &["subject", "new", "--input", r#"{"operands":["dashed"],"domain":"dev","title":"--no-such-flag -x"}"#, "--json"], None);
    let body = json_of(&out);
    assert_eq!(out.status.code(), Some(0), "{body}");
    assert_eq!(body["data"]["title"], "--no-such-flag -x");
    // an operand that starts with a dash reaches the leaf's own grammar, not clap's flag parser
    let out = canon(dir.path(), home.path(), &["subject", "new", "--input", r#"{"operands":["-dashed"],"domain":"dev","title":"T"}"#, "--json"], None);
    let body = json_of(&out);
    assert_eq!(body["error"]["field"], "/operands", "{body}");
    assert!(body["error"]["message"].as_str().unwrap().contains("-dashed"), "{body}");
}

// ---------------------------------------------------------------- batches

#[test]
fn one_invalid_element_writes_nothing_and_points_at_it() {
    let home = TempDir::new().unwrap();
    let base = fixture(home.path(), false);
    let repo = base.path();
    let before = tree(repo);
    let doc = json!([
        {"operands": ["auth.login.02"], "title": "Sign out"},
        {"operands": ["auth.login.03"], "title": "Locked out"},
        {"operands": ["auth.login.04"], "lane": "behavior"},
    ])
    .to_string();
    let out = canon(repo, home.path(), &["scenario", "new", "--input", &doc, "--json"], None);
    let body = json_of(&out);
    assert_eq!(out.status.code(), Some(2), "{body}");
    assert_eq!((body["error"]["code"].as_str(), body["error"]["field"].as_str()), (Some("USAGE"), Some("/items/2/title")), "{body}");
    assert_eq!(tree(repo), before, "nothing was written, not even the valid elements");

    // an unknown key and a bad value of a leaf with a value set
    let doc = json!([{"changeId":"add-login","round":1,"seq":2,"severity":"note","actorId":"a","summary":"s"},{"changeId":"add-login","round":1,"seq":3,"severity":"urgent","actorId":"a","summary":"s"}]).to_string();
    let out = canon(repo, home.path(), &["finding", "add", "--input", &doc, "--json"], None);
    let body = json_of(&out);
    assert_eq!((out.status.code(), body["error"]["field"].as_str()), (Some(2), Some("/items/1/severity")), "{body}");
    assert_eq!(tree(repo), before);

    // the destructive leaves' own refusals ride the leaf's --dry-run: a leaf with one validates the whole batch first
    let doc = json!([{"operands": ["demo"], "changeId": "add-login"}, {"operands": ["demo"], "changeId": "no-such-change"}]).to_string();
    let out = canon(repo, home.path(), &["subject", "unadopt", "--input", &doc, "--json"], None);
    let body = json_of(&out);
    assert_eq!(out.status.code(), Some(2), "{body}");
    assert_eq!(body["error"]["field"], "/items/1", "{body}");
    assert_eq!(tree(repo), before, "the dry run of element 1 refused before element 0 wrote");
}

#[test]
fn a_batch_stops_at_the_first_write_failure_and_a_rerun_reports_unchanged() {
    let home = TempDir::new().unwrap();
    let base = fixture(home.path(), true);
    let repo = base.path();
    let schemas = spec_schemas();
    let element = |scenario: &str| json!({"scenarioId": scenario, "class": "test-run", "ref": "npm test", "role": "implementer"});
    // element 2 names a task no plan has: it passes clap and is refused when it runs
    let unknown_task = json!({"taskId": "no-such-change#1", "class": "test-run", "ref": "npm test", "role": "implementer"});
    let doc = json!([element("auth.login.01"), element("auth.login.02"), unknown_task, element("auth.login.01")]).to_string();

    let out = canon(repo, home.path(), &["evidence", "add", "--input", &doc, "--json"], None);
    let body = json_of(&out);
    assert_eq!(out.status.code(), Some(1), "{body}");
    assert_eq!(body["ok"], false);
    let statuses: Vec<&str> = body["data"]["items"].as_array().unwrap().iter().map(|i| i["status"].as_str().unwrap()).collect();
    assert_eq!(statuses, ["written", "written", "failed", "skipped"], "{body}");
    assert!(body["data"]["items"][2]["error"]["code"].is_string(), "{body}");
    assert_eq!(body["error"]["code"], body["data"]["items"][2]["error"]["code"]);
    assert!(body["data"]["items"][0]["data"]["records"].is_array(), "{body}");
    assert_eq!(body["meta"]["command"], "evidence add");
    validates(&schemas, "BatchEvidenceAddReport", &body["data"]);
    let written = tree(repo);

    // the same batch again: the two that landed are unchanged, the third fails again
    let out = canon(repo, home.path(), &["evidence", "add", "--input", &doc, "--json"], None);
    let body = json_of(&out);
    assert_eq!(out.status.code(), Some(1), "{body}");
    let statuses: Vec<&str> = body["data"]["items"].as_array().unwrap().iter().map(|i| i["status"].as_str().unwrap()).collect();
    assert_eq!(statuses, ["unchanged", "unchanged", "failed", "skipped"], "{body}");
    assert_eq!(tree(repo), written, "a repeat writes nothing");
    validates(&schemas, "BatchEvidenceAddReport", &body["data"]);

    // without the failing element every element is written or unchanged: exit 0
    let ok_doc = json!([element("auth.login.01"), element("auth.login.02")]).to_string();
    let out = canon(repo, home.path(), &["evidence", "add", "--input", &ok_doc, "--json"], None);
    let body = json_of(&out);
    assert_eq!(out.status.code(), Some(0), "{body}");
    assert!(body["data"]["items"].as_array().unwrap().iter().all(|i| i["status"] == "unchanged"));
    validates(&schemas, "BatchEvidenceAddReport", &body["data"]);
}

#[test]
fn a_batch_writes_what_the_same_calls_one_by_one_write() {
    let home = TempDir::new().unwrap();
    let base = fixture(home.path(), false);
    let (_batch_scratch, batch) = scratch_copy(base.path());
    let (_single_scratch, single) = scratch_copy(base.path());
    let schemas = spec_schemas();

    let ndjson = "# the login scenarios\n\n{\"operands\":[\"auth.login.02\"],\"title\":\"Wrong password\",\"subjectId\":\"demo\",\"lane\":\"behavior\",\"case\":\"failure\"}\n{\"operands\":[\"auth.session.01\"],\"title\":\"Session expires\"}\n";
    let out = canon(&batch, home.path(), &["scenario", "new", "--input", "-", "--json"], Some(ndjson));
    let body = json_of(&out);
    assert_eq!(out.status.code(), Some(0), "{body}");
    assert_eq!(body["data"]["items"].as_array().unwrap().len(), 2);
    validates(&schemas, "BatchScenarioNewReport", &body["data"]);

    ok(&single, home.path(), &["scenario", "new", "auth.login.02", "--title", "Wrong password", "--subject-id", "demo", "--lane", "behavior", "--case", "failure"]);
    ok(&single, home.path(), &["scenario", "new", "auth.session.01", "--title", "Session expires"]);
    assert_eq!(normalized_tree(&batch), normalized_tree(&single));

    // a scenario id that exists is a write failure, not a validation failure: earlier elements stay
    let doc = json!([{"operands": ["auth.login.03"], "title": "Third"}, {"operands": ["auth.login.01"], "title": "Again"}, {"operands": ["auth.login.04"], "title": "Fourth"}]).to_string();
    let out = canon(&batch, home.path(), &["scenario", "new", "--input", &doc, "--json"], None);
    let body = json_of(&out);
    assert_eq!(out.status.code(), Some(1), "{body}");
    let statuses: Vec<&str> = body["data"]["items"].as_array().unwrap().iter().map(|i| i["status"].as_str().unwrap()).collect();
    assert_eq!(statuses, ["written", "failed", "skipped"], "{body}");
    assert_eq!(body["error"]["code"], "SCENARIO_TAG_EXISTS", "{body}");
    assert_eq!(body["error"]["field"], "/items/1/operands", "the runtime refusal points into the batch: {body}");
    let feature = std::fs::read_to_string(batch.join(".canon/specs/kind=feature/area=auth/login.feature")).unwrap();
    assert!(feature.contains("auth.login.03") && !feature.contains("auth.login.04"), "{feature}");
}

#[test]
fn flags_on_the_command_line_apply_to_every_element() {
    let home = TempDir::new().unwrap();
    let dir = init_repo(home.path());
    let repo = dir.path();
    // `--dry-run` on the command line makes the whole batch a preview
    let before = tree(repo);
    let out = canon(repo, home.path(), &["inventory", "sync", "--dry-run", "--input", r#"[{}]"#, "--json"], None);
    let body = json_of(&out);
    assert_eq!(out.status.code(), Some(0), "{body}");
    assert_eq!(body["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(tree(repo), before);
}

// ---------------------------------------------------------------- destructive leaves

#[test]
fn the_destructive_leaves_take_one_object_and_never_a_commit() {
    let home = TempDir::new().unwrap();
    let base = fixture(home.path(), false);
    let repo = base.path();
    let before = tree(repo);
    for leaf in ["purge", "artifacts move", "migrate", "ledger compact", "tier age"] {
        let words: Vec<&str> = leaf.split(' ').collect();
        let doc_args = |doc: &str| -> Vec<String> {
            let mut argv: Vec<String> = words.iter().map(|w| w.to_string()).collect();
            argv.extend(["--input".to_string(), doc.to_string(), "--json".to_string()]);
            argv
        };
        for doc in ["[{}]", "{}\n{}"] {
            let argv = doc_args(doc);
            let out = canon(repo, home.path(), &argv.iter().map(String::as_str).collect::<Vec<_>>(), None);
            let body = json_of(&out);
            // `{}\n{}` is not one JSON value inline: it is invalid there, a batch only from a file
            let want = if doc == "[{}]" { "INPUT_BATCH_UNSUPPORTED" } else { "INPUT_INVALID" };
            assert_eq!((out.status.code(), body["error"]["code"].as_str()), (Some(2), Some(want)), "{leaf} {doc}: {body}");
        }
        std::fs::write(repo.join("two.ndjson"), "{}\n{}\n").unwrap();
        let out = canon(repo, home.path(), &[words.as_slice(), &["--input", "two.ndjson", "--json"]].concat(), None);
        assert_eq!((out.status.code(), json_of(&out)["error"]["code"].as_str().map(str::to_string)), (Some(2), Some("INPUT_BATCH_UNSUPPORTED".to_string())), "{leaf}");
        std::fs::remove_file(repo.join("two.ndjson")).unwrap();

        for key in ["commit", "planDigest"] {
            let doc = format!(r#"{{"{key}":"x"}}"#);
            let argv = doc_args(&doc);
            let out = canon(repo, home.path(), &argv.iter().map(String::as_str).collect::<Vec<_>>(), None);
            let body = json_of(&out);
            assert_eq!((out.status.code(), body["error"]["code"].as_str()), (Some(2), Some("INPUT_UNKNOWN_FIELD")), "{leaf} {key}: {body}");
        }
    }
    assert_eq!(tree(repo), before);

    // one object is a run like the flags: the bare destructive run previews
    let doc = json!({"kind": "session", "before": "2020-01-01T00:00:00Z"}).to_string();
    let out = canon(repo, home.path(), &["purge", "--input", &doc, "--json"], None);
    let body = json_of(&out);
    assert_eq!(out.status.code(), Some(0), "{body}");
    assert_eq!(body["data"]["committed"], false);
    assert!(body["data"]["planDigest"].is_string());
    assert!(body["data"].get("items").is_none());
}

#[test]
fn removed_scenario_new_from_names_input() {
    let home = TempDir::new().unwrap();
    let dir = init_repo(home.path());
    let out = canon(dir.path(), home.path(), &["scenario", "new", "--from", "x.ndjson", "--json"], None);
    let body = json_of(&out);
    assert_eq!((out.status.code(), body["error"]["code"].as_str()), (Some(2), Some("USAGE")), "{body}");
    assert!(body["error"]["expected"].as_array().unwrap().iter().any(|v| v.as_str().unwrap().contains("--input")), "{body}");
}
