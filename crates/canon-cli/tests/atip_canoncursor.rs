//! The cursor of atip/1 1.2 in canon (`canon-0-35-canon-cursor-1-2`, scenarios `atip.canoncursor.*`), against the built
//! binary: the binding is `atip::cursor`'s (explicit arguments, an explicit default, the value `CANON_ROLE` supplied for
//! `--as`, canon's own flag-only options), the next page is written from the call's own arguments, `--all` continues a
//! `--limit` listing, and `limit`, `cursor`, `all` and `fields` are flag-only: a document cannot carry them.
//!
//! Tests are named by scenario id (`atip_canoncursor_01_…` proves `atip.canoncursor.01`).

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::Value;
use tempfile::TempDir;

fn canon_in(dir: &Path, args: &[&str], env: &[(&str, &str)], stdin: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_canon"));
    command.args(args).current_dir(dir).env_remove("CANON_ROLE").env("CANON_ACTOR", "atip-canoncursor").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    for (name, value) in env {
        command.env(name, value);
    }
    let mut child = command.spawn().expect("spawn canon");
    let mut pipe = child.stdin.take().expect("stdin");
    if let Some(text) = stdin {
        pipe.write_all(text.as_bytes()).expect("write stdin");
    }
    drop(pipe);
    child.wait_with_output().expect("canon runs")
}

fn canon(dir: &Path, args: &[&str]) -> Output {
    canon_in(dir, args, &[], None)
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("stdout is one JSON document ({e}): {}\nstderr: {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)))
}

fn ok(dir: &Path, args: &[&str]) {
    let out = canon(dir, args);
    assert!(out.status.success(), "{args:?}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}

/// A repo with three subjects, a change with three findings, a feature with three scenarios, two of them divergent. Committed.
fn fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    ok(repo, &["init", "--providers", "none"]);
    for subject in ["demo", "demo-two", "demo-three"] {
        ok(repo, &["subject", "new", subject, "--domain", "dev", "--title", &format!("Title {subject}")]);
    }
    ok(repo, &["change", "new", "add-login", "--subject", "demo", "--title", "Add login"]);
    ok(repo, &["change", "new", "add-logout", "--subject", "demo-two", "--title", "Add logout"]);
    ok(repo, &["feature", "new", "auth.login", "--title", "Login"]);
    for n in 1..=3 {
        ok(repo, &["scenario", "new", &format!("auth.login.0{n}"), "--title", &format!("Case {n}"), "--feature", ".canon/specs/kind=feature/area=auth/login.feature"]);
    }
    ok(repo, &["inventory", "sync"]);
    for args in [&["init", "-q"][..], &["add", "-A"], &["-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "-q", "-m", "fixture"]] {
        assert!(Command::new("git").args(args).current_dir(repo).status().unwrap().success());
    }
    for n in 1..=3 {
        ok(repo, &["finding", "add", "--change", "add-login", "--round", "1", "--seq", &n.to_string(), "--severity", "should-fix", "--introduced-unknown", "--actor", "unit-1", "--summary", &format!("Finding {n}")]);
    }
    for n in 1..=2 {
        ok(repo, &["divergence", "add", &format!("auth.login.0{n}"), "--project", "root", "--actor", "alice", "--role", "reviewer"]);
    }
    dir
}

/// Every file under `dir` (but `.git`) with its bytes: what a refused call must leave as it found it.
fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, at: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(at).unwrap().flatten() {
            let path = entry.path();
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(path.strip_prefix(root).unwrap().display().to_string(), std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// The four paged commands, each with the arguments that select its records (and `--json`).
fn listings() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("query", vec!["query", "--kind", "subject", "--json"]),
        ("finding list", vec!["finding", "list", "--change", "add-login", "--json"]),
        ("divergence list", vec!["divergence", "list", "--json"]),
        ("export", vec!["export", "--kind", "subject", "--json"]),
    ]
}

/// `args` followed by `extra`.
fn with(args: &[&str], extra: &[&str]) -> Vec<String> {
    args.iter().chain(extra.iter()).map(ToString::to_string).collect()
}

fn run(dir: &Path, args: &[String], env: &[(&str, &str)]) -> Output {
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    canon_in(dir, &words, env, None)
}

/// The first page of `args` with `extra` (`--limit 1`...) and the call `meta.next[0]` gives for the next one.
fn first_page(dir: &Path, args: &[&str], extra: &[&str], env: &[(&str, &str)]) -> (Value, Vec<String>) {
    let document = json(&run(dir, &with(args, extra), env));
    assert_eq!(document["ok"], true, "{args:?} {extra:?}: {document}");
    let next: Vec<String> = document["meta"]["next"][0]["argv"].as_array().unwrap_or_else(|| panic!("{args:?} has a next page: {document}")).iter().map(|word| word.as_str().unwrap().to_string()).collect();
    (document, next)
}

fn cursor_of(next: &[String]) -> String {
    next.iter().find_map(|word| word.strip_prefix("--cursor=")).unwrap_or_else(|| panic!("a --cursor= word in {next:?}")).to_string()
}

/// `(exit, code, reason, field)` of a refusal.
fn refusal(output: &Output) -> (Option<i32>, String, Option<String>, Option<String>) {
    let error = json(output)["error"].clone();
    (output.status.code(), error["code"].as_str().unwrap_or_default().to_string(), error["reason"].as_str().map(str::to_string), error["field"].as_str().map(str::to_string))
}

fn refused_cursor(output: &Output, reason: &str, what: &str) {
    assert_eq!(refusal(output), (Some(2), "CURSOR_INVALID".to_string(), Some(reason.to_string()), Some("--cursor".to_string())), "{what}: {}", String::from_utf8_lossy(&output.stdout));
}

/// `next` with its `--cursor=` word replaced by the one of `cursor`, and `extra` before it.
fn resumed(next: &[String], cursor: &str, extra: &[&str]) -> Vec<String> {
    let mut call: Vec<String> = next.iter().filter(|word| !word.starts_with("--cursor=")).cloned().collect();
    call.extend(extra.iter().map(ToString::to_string));
    call.push(format!("--cursor={cursor}"));
    call
}

/// `args` as the contract writes a call: the command words, then each option `--name=value` (a switch as `--name`).
fn normalised(args: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let mut words = args.iter().peekable();
    while let Some(word) = words.next() {
        match words.peek() {
            Some(value) if word.starts_with("--") && !value.starts_with("--") => out.push(format!("{word}={}", words.next().unwrap())),
            _ => out.push(word.to_string()),
        }
    }
    out
}

#[test]
fn atip_canoncursor_01_the_next_call_holds_the_explicit_arguments_and_the_cursor_and_nothing_else() {
    let repo = fixture();
    for (name, args) in listings() {
        // The explicit arguments, in the contract's form `--name=value`, and the cursor: no default, no `--repo`, no `--as`.
        let (document, next) = first_page(repo.path(), &args, &["--limit", "1"], &[]);
        let cursor = document["meta"]["page"]["nextCursor"].as_str().unwrap();
        let mut want: Vec<String> = normalised(&args);
        want.push("--limit=1".to_string());
        want.sort();
        let mut got: Vec<String> = next.iter().filter(|word| !word.starts_with("--cursor=")).cloned().collect();
        got.sort();
        assert_eq!(got, want, "{name}: {next:?}");
        assert_eq!(cursor_of(&next), cursor, "{name}");
        // `--repo` and `--as` are given on the line, so they are carried; the value CANON_ROLE supplied is not.
        let (_, carried) = first_page(repo.path(), &args, &["--limit", "1", "--repo", ".", "--as", "reviewer"], &[]);
        assert!(carried.iter().any(|word| word == "--repo=.") && carried.iter().any(|word| word == "--as=reviewer"), "{name}: {carried:?}");
        let (_, from_env) = first_page(repo.path(), &args, &["--limit", "1"], &[("CANON_ROLE", "reviewer")]);
        assert!(!from_env.iter().any(|word| word.starts_with("--as") || word.starts_with("--repo")), "{name}: {from_env:?}");
        // A spelling is the same call: `--limit 01` is the number 1.
        let (_, spelled) = first_page(repo.path(), &args, &["--limit", "01"], &[]);
        assert!(spelled.iter().any(|word| word == "--limit=1"), "{name}: {spelled:?}");
        // The call is run as it is, page after page, and the pages cover every record once.
        let mut call = next;
        let mut seen = 1;
        loop {
            let page = json(&run(repo.path(), &call, &[]));
            assert_eq!(page["ok"], true, "{name}: {page}");
            seen += page["meta"]["page"]["returned"].as_u64().unwrap();
            match page["meta"]["next"][0]["argv"].as_array() {
                Some(argv) if page["meta"]["page"].get("nextCursor").is_some() => call = argv.iter().map(|word| word.as_str().unwrap().to_string()).collect(),
                _ => {
                    assert_eq!(seen, page["meta"]["page"]["total"].as_u64().unwrap(), "{name}: every record once");
                    break;
                }
            }
        }
    }
}

#[test]
fn atip_canoncursor_02_the_environment_and_an_explicit_default_bind_the_cursor_and_the_page_size_does_not() {
    let repo = fixture();
    for (name, args) in listings() {
        let (_, plain) = first_page(repo.path(), &args, &["--limit", "1"], &[]);
        let cursor = cursor_of(&plain);
        let before = snapshot(repo.path());
        // CANON_ROLE set, changed or explicit: another call.
        for (what, extra, env) in [
            ("CANON_ROLE set", vec![], vec![("CANON_ROLE", "reviewer")]),
            ("CANON_ROLE set to the other role", vec![], vec![("CANON_ROLE", "planner")]),
            ("--as given", vec!["--as", "reviewer"], vec![]),
            ("--as given and CANON_ROLE set to the same", vec!["--as", "reviewer"], vec![("CANON_ROLE", "reviewer")]),
            ("--repo given with its default", vec!["--repo", "."], vec![]),
        ] {
            let call = resumed(&with(&args, &["--limit=1"]), &cursor, &extra);
            refused_cursor(&run(repo.path(), &call, &env), "ARGUMENTS_CHANGED", &format!("{name}: {what}"));
        }
        assert_eq!(snapshot(repo.path()), before, "{name}: a refusal writes nothing");
        // A cursor issued under CANON_ROLE continues under the same one and only under it.
        let (_, issued) = first_page(repo.path(), &args, &["--limit", "1"], &[("CANON_ROLE", "reviewer")]);
        let under = cursor_of(&issued);
        let same = run(repo.path(), &resumed(&with(&args, &["--limit=1"]), &under, &[]), &[("CANON_ROLE", "reviewer")]);
        assert!(same.status.success(), "{name}: {}", String::from_utf8_lossy(&same.stdout));
        let dropped = run(repo.path(), &resumed(&with(&args, &["--limit=1"]), &under, &[]), &[]);
        refused_cursor(&dropped, "ARGUMENTS_CHANGED", &format!("{name}: CANON_ROLE dropped"));
        // An explicit default dropped.
        let (_, with_repo) = first_page(repo.path(), &args, &["--limit", "1", "--repo", "."], &[]);
        let dropped = run(repo.path(), &resumed(&with(&args, &["--limit=1"]), &cursor_of(&with_repo), &[]), &[]);
        refused_cursor(&dropped, "ARGUMENTS_CHANGED", &format!("{name}: --repo dropped"));
        // `--limit`, `--fields` and `--json` do not change the place.
        let wider = run(repo.path(), &resumed(&with(&args, &["--limit=2"]), &cursor, &[]), &[]);
        assert!(wider.status.success(), "{name}: {}", String::from_utf8_lossy(&wider.stdout));
        let fields = match name {
            "finding list" => "summary",
            "divergence list" => "state",
            "export" => "kind",
            _ => "subject_id",
        };
        let projected = run(repo.path(), &resumed(&with(&args, &["--limit=1"]), &cursor, &["--fields", fields]), &[]);
        assert!(projected.status.success(), "{name}: {}", String::from_utf8_lossy(&projected.stdout));
        let text: Vec<String> = resumed(&with(&args, &["--limit=1"]), &cursor, &[]).into_iter().filter(|word| word != "--json").collect();
        let in_text = run(repo.path(), &text, &[]);
        assert!(in_text.status.success(), "{name}: {}", String::from_utf8_lossy(&in_text.stderr));
    }
}

#[test]
fn atip_canoncursor_03_the_cursor_holds_no_record_key_as_written_and_a_changed_character_is_foreign() {
    let repo = fixture();
    // The key of an export record is its time, id and digest, in the manifest.
    let (page, next) = first_page(repo.path(), &["export", "--kind", "subject", "--json"], &["--limit", "1"], &[]);
    let record = &page["data"]["records"][0];
    let key = format!("{}|{}|{}", record["at"].as_str().unwrap(), record["id"].as_str().unwrap_or_default(), record["digest"].as_str().unwrap());
    let cursor = cursor_of(&next);
    assert!(!cursor.contains(&key) && !cursor.contains(record["digest"].as_str().unwrap()), "{cursor}");
    let hex: String = key.bytes().map(|byte| format!("{byte:02x}")).collect();
    assert!(!cursor.contains(&hex), "{cursor}");
    assert!(cursor.bytes().all(|byte| byte.is_ascii_alphanumeric() || b".-_".contains(&byte)), "one word of ASCII without quotes or whitespace: {cursor}");
    // Each character of a cursor, changed, is no cursor of this command.
    for (name, args) in listings() {
        let (_, next) = first_page(repo.path(), &args, &["--limit", "1"], &[]);
        let cursor = cursor_of(&next);
        for at in (0..cursor.len()).step_by(5).chain([cursor.len() - 1]) {
            let mut bytes = cursor.clone().into_bytes();
            bytes[at] = if bytes[at] == b'A' { b'B' } else { b'A' };
            let changed = String::from_utf8(bytes).unwrap();
            let call = resumed(&next, &changed, &[]);
            refused_cursor(&run(repo.path(), &call, &[]), "FOREIGN", &format!("{name}: character {at} changed"));
        }
    }
}

#[test]
fn atip_canoncursor_04_all_in_place_of_limit_reads_the_rest_without_refusal() {
    let repo = fixture();
    for (name, args) in listings() {
        let (first, next) = first_page(repo.path(), &args, &["--limit", "1"], &[]);
        let total = first["meta"]["page"]["total"].as_u64().unwrap();
        let call: Vec<String> = next.iter().map(|word| if word == "--limit=1" { "--all".to_string() } else { word.clone() }).collect();
        assert!(call.iter().any(|word| word == "--all"), "{name}: {call:?}");
        let rest = json(&run(repo.path(), &call, &[]));
        assert_eq!(rest["ok"], true, "{name}: {rest}");
        assert_eq!(rest["meta"]["page"]["returned"].as_u64().unwrap(), total - 1, "{name}: the rest");
        assert!(rest["meta"]["page"].get("nextCursor").is_none(), "{name}: {rest}");
    }
}

#[test]
fn atip_canoncursor_05_limit_with_all_is_usage_naming_the_first_name_of_the_rule() {
    let repo = fixture();
    let before = snapshot(repo.path());
    for (name, args) in listings() {
        // `field` is the first given name of the rule's `names` (`limit`, `all`), whichever the line wrote first (spec 10.8.1).
        for (order, field) in [(["--limit", "1", "--all"], "--limit"), (["--all", "--limit", "1"], "--limit")] {
            let output = canon(repo.path(), &args.iter().copied().chain(order).collect::<Vec<_>>());
            assert_eq!(refusal(&output), (Some(2), "USAGE".to_string(), None, Some(field.to_string())), "{name} {order:?}: {}", String::from_utf8_lossy(&output.stdout));
        }
    }
    assert_eq!(snapshot(repo.path()), before);
}

#[test]
fn atip_canoncursor_06_other_arguments_are_arguments_changed_another_command_is_foreign_and_a_gone_record_is_expired() {
    let repo = fixture();
    let other = fixture();
    let changed: Vec<(&str, Vec<&str>, Vec<&str>)> = vec![
        ("query --kind", vec!["query", "--kind", "subject", "--json"], vec!["query", "--kind", "change", "--json"]),
        ("query filter", vec!["query", "--kind", "subject", "--json"], vec!["query", "--kind", "subject", "--domain", "dev", "--json"]),
        ("finding list --change", vec!["finding", "list", "--change", "add-login", "--json"], vec!["finding", "list", "--change", "add-logout", "--json"]),
        ("finding list --open", vec!["finding", "list", "--change", "add-login", "--json"], vec!["finding", "list", "--change", "add-login", "--open", "--json"]),
        ("divergence list --as-of", vec!["divergence", "list", "--json"], vec!["divergence", "list", "--as-of", "2030-01-01", "--json"]),
        ("export --kind", vec!["export", "--kind", "subject", "--json"], vec!["export", "--kind", "change", "--json"]),
    ];
    for (what, issued, called) in changed {
        let (_, next) = first_page(repo.path(), &issued, &["--limit", "1"], &[]);
        let call = resumed(&with(&called, &["--limit=1"]), &cursor_of(&next), &[]);
        refused_cursor(&run(repo.path(), &call, &[]), "ARGUMENTS_CHANGED", what);
    }
    // Another `--repo`: the cursor was issued with the first one.
    let (first, second) = (repo.path().to_str().unwrap(), other.path().to_str().unwrap());
    let (_, next) = first_page(repo.path(), &["query", "--kind", "subject", "--json"], &["--limit", "1", "--repo", first], &[]);
    let call = resumed(&["query", "--kind=subject", "--json", "--limit=1"].map(String::from), &cursor_of(&next), &["--repo", second]);
    refused_cursor(&run(other.path(), &call, &[]), "ARGUMENTS_CHANGED", "another --repo");
    // Another command's cursor.
    let (_, finding_next) = first_page(repo.path(), &["finding", "list", "--change", "add-login", "--json"], &["--limit", "1"], &[]);
    let foreign = run(repo.path(), &["query", "--kind", "subject", "--json", "--cursor", &cursor_of(&finding_next)].map(String::from), &[]);
    refused_cursor(&foreign, "FOREIGN", "another command's cursor");
    // The record the cursor points after is gone.
    let ledger = repo.path().join(".canon/ledger/kind=finding");
    let first_finding = std::fs::read_dir(&ledger).unwrap().flatten().map(|entry| entry.path()).find(|path| path.file_name().unwrap().to_string_lossy().starts_with("add-login_1_1_")).expect("the first finding's file");
    std::fs::remove_file(first_finding).unwrap();
    let expired = run(repo.path(), &resumed(&finding_next, &cursor_of(&finding_next), &[]), &[]);
    refused_cursor(&expired, "EXPIRED", "the record is gone");
}

#[test]
fn atip_canoncursor_07_a_refused_cursor_writes_nothing_and_an_export_refusal_writes_no_audit_line() {
    let repo = fixture();
    let out = repo.path().join("out.json");
    let out = out.to_str().unwrap();
    let (_, next) = first_page(repo.path(), &["export", "--kind", "subject", "--json"], &["--limit", "1", "--output", out], &[]);
    let cursor = cursor_of(&next);
    // The first page wrote its file and its audit line; what a refusal must leave alone is what is there now.
    std::fs::remove_file(out).unwrap();
    let before = snapshot(repo.path());
    let refused = [
        resumed(&["export", "--kind=subject", "--json", "--limit=1", &format!("--output={out}")].map(String::from), &cursor, &["--dry-run"]),
        resumed(&["export", "--kind=change", "--json", "--limit=1", &format!("--output={out}")].map(String::from), &cursor, &[]),
        resumed(&["export", "--kind=subject", "--json", "--limit=1", &format!("--output={out}")].map(String::from), "not-a-cursor", &[]),
    ];
    for call in &refused {
        let output = run(repo.path(), call, &[]);
        assert_eq!(output.status.code(), Some(2), "{call:?}: {}", String::from_utf8_lossy(&output.stdout));
        assert_eq!(json(&output)["error"]["code"], "CURSOR_INVALID");
    }
    assert!(!Path::new(out).exists(), "no file");
    assert_eq!(snapshot(repo.path()), before, "no file, no audit line, nothing");
    // The same export with its own cursor does write: the audit line is what the refusals did not add.
    let good = run(repo.path(), &next, &[]);
    assert!(good.status.success(), "{}", String::from_utf8_lossy(&good.stdout));
    assert_ne!(snapshot(repo.path()), before, "a real run writes the file and its audit line");
    // The three listings that write nothing are left as they are too.
    for (name, args) in listings().into_iter().filter(|(name, _)| *name != "export") {
        let (_, next) = first_page(repo.path(), &args, &["--limit", "1"], &[]);
        let before = snapshot(repo.path());
        for call in [resumed(&next, "not-a-cursor", &[]), resumed(&next, &cursor, &[])] {
            assert_eq!(run(repo.path(), &call, &[]).status.code(), Some(2), "{name}");
        }
        assert_eq!(snapshot(repo.path()), before, "{name}");
    }
}

#[test]
fn atip_canoncursor_08_fields_is_checked_the_same_in_text_mode_and_in_json_mode() {
    let repo = fixture();
    let before = snapshot(repo.path());
    for (name, args) in listings() {
        let json_call: Vec<&str> = args.iter().copied().chain(["--fields", "nosuch"]).collect();
        let text_call: Vec<&str> = args.iter().copied().filter(|word| *word != "--json").chain(["--fields", "nosuch"]).collect();
        let in_json = canon(repo.path(), &json_call);
        let error = json(&in_json)["error"].clone();
        assert_eq!((in_json.status.code(), error["code"].as_str(), error["field"].as_str()), (Some(2), Some("USAGE"), Some("--fields")), "{name}: {error}");
        let expected = error["expected"].as_array().expect("the declared paths").iter().map(|path| path.as_str().unwrap().to_string()).collect::<Vec<_>>();
        let in_text = canon(repo.path(), &text_call);
        assert_eq!(in_text.status.code(), Some(2), "{name}");
        assert!(in_text.stdout.is_empty(), "{name}: a refusal prints no report");
        let stderr = String::from_utf8_lossy(&in_text.stderr);
        assert!(stderr.starts_with("canon: USAGE:") && stderr.contains("field: --fields"), "{name}: {stderr}");
        assert!(stderr.contains(&format!("expected: {}", expected[0])), "{name}: the same declared paths: {stderr}");
    }
    assert_eq!(snapshot(repo.path()), before);
}

fn command_doc(dir: &Path, command: &str) -> Value {
    let mut args: Vec<&str> = vec!["schema"];
    args.extend(command.split(' '));
    args.push("--json");
    json(&canon(dir, &args))["data"].clone()
}

#[test]
fn atip_canoncursor_09_limit_cursor_all_and_fields_are_flag_only_in_every_command_document() {
    let repo = fixture();
    for command in ["query", "finding list", "divergence list", "export", "retrieve", "search"] {
        let doc = command_doc(repo.path(), command);
        let options = doc["options"].as_array().unwrap();
        let mut found = 0;
        for name in ["limit", "cursor", "all", "fields"] {
            if let Some(option) = options.iter().find(|option| option["name"] == name) {
                found += 1;
                assert_eq!(option["flagOnly"], true, "{command}: --{name}");
            }
        }
        assert!(found >= 1, "{command} has some of them");
        if ["query", "finding list", "divergence list", "export"].contains(&command) {
            assert_eq!(found, 4, "{command} has all four");
        }
        for name in atip::flag_only_options(atip::Minor::V1_2) {
            if let Some(option) = options.iter().find(|option| option["name"] == *name) {
                assert_eq!(option["flagOnly"], true, "{command}: --{name} is in the contract's set");
            }
        }
        for schema in doc["schemas"].as_object().unwrap().values() {
            if let Some(properties) = schema["properties"].as_object() {
                for key in ["limit", "cursor", "all", "fields"] {
                    assert!(!properties.contains_key(key), "{command}: a schema has `{key}`");
                }
            }
        }
    }
}

#[test]
fn atip_canoncursor_10_a_flag_only_key_of_a_document_is_flag_only_key_at_its_pointer() {
    let repo = fixture();
    let before = snapshot(repo.path());
    let out = repo.path().join("out.json");
    for (document, field) in [
        (r#"{"kind":"subject","limit":5}"#, "/limit"),
        (r#"{"kind":"subject","cursor":"x"}"#, "/cursor"),
        (r#"{"kind":"subject","all":true}"#, "/all"),
        (r#"{"kind":"subject","fields":"kind"}"#, "/fields"),
        (r#"{"limit":5}"#, "/limit"),
    ] {
        let output = canon(repo.path(), &["export", "--input", document, "--json"]);
        assert_eq!(refusal(&output), (Some(2), "INPUT_INVALID".to_string(), Some("FLAG_ONLY_KEY".to_string()), Some(field.to_string())), "{document}: {}", String::from_utf8_lossy(&output.stdout));
    }
    // An element of a batch is pointed at by its index.
    let batch = format!(r#"[{{"kind":"subject","output":"{}"}},{{"kind":"subject","limit":5}}]"#, out.display());
    let output = canon(repo.path(), &["export", "--input", &batch, "--json"]);
    assert_eq!(refusal(&output), (Some(2), "INPUT_INVALID".to_string(), Some("FLAG_ONLY_KEY".to_string()), Some("/1/limit".to_string())), "{}", String::from_utf8_lossy(&output.stdout));
    assert!(!out.exists(), "nothing of the batch ran");
    assert_eq!(snapshot(repo.path()), before, "nothing was written, no audit line included");
    // The same documents with the option on the command line are accepted.
    assert!(canon(repo.path(), &["export", "--input", r#"{"kind":"subject"}"#, "--limit", "1", "--json"]).status.success());
}

#[test]
fn atip_canoncursor_11_the_flag_only_set_is_the_crates_and_repo_and_as_and_no_source_lists_the_contracts_options() {
    let mut want: Vec<&str> = atip::flag_only_options(atip::Minor::V1_2).to_vec();
    want.extend(["repo", "as"]);
    want.sort_unstable();
    let mut got: Vec<&str> = canon_cli::input::flags_only().to_vec();
    got.sort_unstable();
    assert_eq!(got, want);
    assert_eq!(want.len(), 10);
    assert_eq!(canon_cli::input::OWN_FLAGS_ONLY, ["repo", "as"]);
    // Every document marks each of them where the command has the option.
    let repo = fixture();
    for command in ["query", "finding list", "divergence list", "export", "finding add", "evidence add", "gate promote"] {
        let doc = command_doc(repo.path(), command);
        for option in doc["options"].as_array().unwrap() {
            let name = option["name"].as_str().unwrap();
            assert_eq!(option["flagOnly"] == true, want.contains(&name), "{command}: --{name}");
        }
    }
    // No source file spells the contract's options as a list of its own.
    let mut stack = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let source = std::fs::read_to_string(&path).unwrap();
                for list in [r#""json", "input""#, r#""commit", "plan-digest", "fields""#, r#""limit", "cursor", "all""#, r#""plan-digest", "limit""#] {
                    assert!(!source.lines().filter(|line| !line.trim_start().starts_with("//") && !line.contains("assert")).any(|line| line.contains(list)), "{} lists the flag-only options by name: {list}", path.display());
                }
            }
        }
    }
}

/// A cursor the 0.34 binary issued for `canon query --kind subject --limit 1 --json` (hex of `query`, a unit separator, an 8-byte binding in hex, a unit separator and the key).
const CURSOR_0_34: &str = "71756572791f333761646335626464396164313835301f323032362d31312d30325431333a33343a34392e3235333639332b30303a30307c617c34396130343064362331";

#[test]
fn atip_canoncursor_12_a_cursor_of_0_34_is_foreign() {
    let repo = fixture();
    let before = snapshot(repo.path());
    for args in [
        vec!["query", "--kind", "subject", "--limit", "1", "--json"],
        vec!["query", "--kind", "subject", "--json"],
        vec!["finding", "list", "--change", "add-login", "--json"],
        vec!["divergence", "list", "--json"],
        vec!["export", "--kind", "subject", "--json"],
    ] {
        let call: Vec<&str> = args.iter().copied().chain(["--cursor", CURSOR_0_34]).collect();
        refused_cursor(&canon(repo.path(), &call), "FOREIGN", &format!("{args:?}"));
    }
    assert_eq!(snapshot(repo.path()), before);
}

#[test]
fn atip_canoncursor_13_the_changelog_states_the_upgrade() {
    let changelog = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CHANGELOG.md")).expect("the CHANGELOG");
    let unreleased = changelog.split("\n## ").nth(1).expect("the Unreleased section");
    assert!(unreleased.starts_with("Unreleased"), "{}", &unreleased[..40]);
    let entry = unreleased.split("\n**").find(|entry| entry.contains("canon-0-35-canon-cursor-1-2")).expect("the entry of this change");
    for phrase in ["0.34", "FOREIGN", "from the first page", "FLAG_ONLY_KEY", "--cursor=<value>", "CANON_ROLE", "ARGUMENTS_CHANGED"] {
        assert!(entry.contains(phrase), "the entry says `{phrase}`: {entry}");
    }
}

#[test]
fn atip_canoncursor_14_a_call_given_by_input_is_carried_as_argv_options() {
    let repo = fixture();
    for (what, args, stdin) in [
        ("inline", vec!["export", "--input", r#"{"kind":"subject"}"#, "--limit", "1", "--json"], None),
        ("stdin", vec!["export", "--input", "-", "--limit", "1", "--json"], Some(r#"{"kind":"subject"}"#)),
    ] {
        let first = canon_in(repo.path(), &args, &[], stdin);
        let document = json(&first);
        assert_eq!(document["ok"], true, "{what}: {document}");
        let next: Vec<String> = document["meta"]["next"][0]["argv"].as_array().unwrap().iter().map(|word| word.as_str().unwrap().to_string()).collect();
        assert!(!next.iter().any(|word| word == "--input" || word.starts_with("--input=") || word == "-"), "{what}: {next:?}");
        assert!(next.iter().any(|word| word == "--kind=subject") && next.iter().any(|word| word == "--limit=1"), "{what}: {next:?}");
        // Run as it is, with no stdin: the next page is read.
        let second = json(&run(repo.path(), &next, &[]));
        assert_eq!(second["ok"], true, "{what}: {second}");
        assert_eq!(second["meta"]["page"]["returned"], 1, "{what}");
        // The same call as arguments is the same call: the cursor continues it.
        let (_, spelled) = first_page(repo.path(), &["export", "--json"], &["--kind", "subject", "--limit", "1"], &[]);
        assert_eq!(cursor_of(&spelled), cursor_of(&next), "{what}: a document and the line say the same");
    }
}
