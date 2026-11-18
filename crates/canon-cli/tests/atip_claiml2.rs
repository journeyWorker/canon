//! `canon-0-35-claim-l2`: canon claims atip/1 level L2 (scenarios `atip.claiml2.*`; `canon-0-36-atip-dep` moved the checker to the atip repository). The declaration is `canon_cli::json_out::DECLARED_MINOR` and `canon_cli::spec::LEVEL`;
//! the proof is `atip check --level L2 --deep` on the committed fixture, which `just atip-deep` and the job `atip` run and no test of canon runs. The tests here are the behaviours that run does not prove: refusals, batches, previews, secrets and terminals.

#[path = "support/hermetic.rs"]
mod hermetic;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

const CANON: &str = env!("CARGO_BIN_EXE_canon");

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The exit status and the one JSON document of `canon <args>` run in `dir`.
fn canon_out(dir: &Path, args: &[&str]) -> (Option<i32>, Value) {
    let output = hermetic::canon().args(args).current_dir(dir).env_remove("CANON_ROLE").output().expect("spawn canon");
    let document = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| panic!("{args:?}: {error}: {}", String::from_utf8_lossy(&output.stdout)));
    (output.status.code(), document)
}

fn canon_in(dir: &Path, args: &[&str]) -> Value {
    canon_out(dir, args).1
}

#[test]
fn atip_claiml2_06_input_too_large_names_the_field_input() {
    let dir = tempfile::tempdir().expect("tempdir");
    let init = hermetic::canon().args(["init", "--no-harness"]).current_dir(dir.path()).env_remove("CANON_ROLE").output().expect("init");
    assert!(init.status.success(), "{}", String::from_utf8_lossy(&init.stderr));
    let big = dir.path().join("big.json");
    std::fs::write(&big, vec![b' '; 8 * 1024 * 1024 + 1]).expect("write");
    let (bytes_exit, bytes) = canon_out(dir.path(), &["scenario", "new", "--input", big.to_str().expect("utf8"), "--json"]);
    let items = format!("[{}]", vec!["{}"; 10_001].join(","));
    let (many_exit, many) = canon_out(dir.path(), &["scenario", "new", "--input", &items, "--json"]);
    for (exit, document, reason) in [(bytes_exit, &bytes, "TOO_MANY_BYTES"), (many_exit, &many, "TOO_MANY_ITEMS")] {
        assert_eq!(exit, Some(2), "a refusal: {document}");
        assert_eq!((document["error"]["code"].as_str(), document["error"]["reason"].as_str(), document["error"]["field"].as_str()), (Some("INPUT_TOO_LARGE"), Some(reason), Some("--input")), "{document}");
    }
}

/// A repository with one commit, canon initialised, and a finding `add-login` 1/1 that is open.
fn repo_with_a_finding() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let git = |args: &[&str]| assert!(Command::new("git").args(args).current_dir(dir.path()).output().expect("git").status.success(), "git {args:?}");
    git(&["init", "-q"]);
    git(&["-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "commit", "--allow-empty", "-qm", "fixture"]);
    for args in [
        &["init", "--no-harness", "--json"][..],
        &["subject", "new", "alpha", "--domain", "core", "--title", "Alpha", "--json"],
        &["change", "new", "add-login", "--subject", "alpha", "--title", "Add login", "--json"],
        &["finding", "add", "--change", "add-login", "--round", "1", "--seq", "1", "--severity", "note", "--actor", "fixture", "--summary", "first", "--json"],
    ] {
        let document = canon_in(dir.path(), args);
        assert_eq!(document["ok"], true, "{args:?}: {document}");
    }
    dir
}

#[test]
fn atip_claiml2_15_a_batch_of_unchanged_elements_has_only_items_in_data() {
    let dir = repo_with_a_finding();
    let close = json!({"operands": ["add-login"], "round": 1, "seq": 1, "disposition": "fixed"});
    let (first_exit, first) = canon_out(dir.path(), &["finding", "close", "add-login", "--round", "1", "--seq", "1", "--disposition", "fixed", "--json"]);
    assert_eq!((first_exit, first["ok"].clone()), (Some(0), json!(true)), "{first}");
    for count in [1, 2] {
        let batch = Value::Array(vec![close.clone(); count]).to_string();
        let (exit, document) = canon_out(dir.path(), &["finding", "close", "--input", &batch, "--json"]);
        assert_eq!(exit, Some(0), "{document}");
        assert_eq!(document["ok"], true, "{document}");
        let data = document["data"].as_object().expect("data is an object");
        assert_eq!(data.keys().collect::<Vec<_>>(), ["items"], "the batch report is closed: {document}");
        let items = data["items"].as_array().expect("items");
        assert_eq!(items.len(), count);
        for item in items {
            assert_eq!(item["status"], "unchanged", "{document}");
            assert_eq!(item["data"]["unchanged"], true, "an element carries its ordinary report: {document}");
        }
        assert_eq!(atip::envelope_problems(&document), Vec::<String>::new());
    }
}

#[test]
fn atip_claiml2_16_the_preview_of_a_move_to_the_local_store_leads_with_the_commit_call() {
    let dir = repo_with_a_finding();
    let (preview_exit, preview) = canon_out(dir.path(), &["artifacts", "move", "--tier", "local", "--json"]);
    assert_eq!(preview_exit, Some(0), "a preview exits 0: {preview}");
    assert_eq!((preview["ok"].clone(), preview["data"]["committed"].clone()), (json!(true), json!(false)), "{preview}");
    let digest = preview["data"]["planDigest"].as_str().expect("a digest");
    let argv: Vec<&str> = preview["meta"]["next"][0]["argv"].as_array().expect("meta.next[0].argv").iter().map(|word| word.as_str().expect("a word")).collect();
    assert_eq!(argv.first(), Some(&"artifacts"), "{preview}");
    assert!(argv.contains(&"--commit") && argv.contains(&"--json") && argv.contains(&format!("--plan-digest={digest}").as_str()), "{argv:?}");
    let (commit_exit, commit) = canon_out(dir.path(), &argv);
    assert_eq!(commit_exit, Some(0), "{commit}");
    assert_eq!((commit["ok"].clone(), commit["data"]["committed"].clone()), (json!(true), json!(true)), "{commit}");
}

/// The secret every output and file is searched for in `atip_claiml2_17`: the password of a Postgres DSN, the secret key of the s3 tier.
const SECRET: &str = "s3cr3t-Marker-9f2c41";

/// Every file under `dir`, as text where it is UTF-8, bytes searched where it is not.
fn files_holding(dir: &Path, needle: &str, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("a directory") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            files_holding(&path, needle, found);
        } else if std::fs::read(&path).is_ok_and(|bytes| bytes.windows(needle.len()).any(|window| window == needle.as_bytes())) {
            found.push(path);
        }
    }
}

#[test]
fn atip_claiml2_17_a_known_secret_of_the_environment_reaches_no_output_and_no_file() {
    // The tiers canon.yaml can route to, named by the environment variables that carry their credentials: Postgres (a DSN with a password) and s3.
    const CANON_YAML: &str = "tiers:\n  local: { backend: git, root: .canon/ledger }\n  hot: { backend: postgres, dsn_env: CANON_PG_DSN, schema: canon_v1 }\n  cold: { backend: s3, bucket_env: CANON_R2_BUCKET, prefix: \"canon/\" }\nrouting:\n  change: local\n  task: hot\n  run: hot\n  trajectory: cold\n";
    // The search sees a marker where one is: a file that holds it is found.
    let planted = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir(planted.path().join("sub")).expect("mkdir");
    std::fs::write(planted.path().join("sub/leak"), format!("x{SECRET}y")).expect("write");
    let mut seen = Vec::new();
    files_holding(planted.path(), SECRET, &mut seen);
    assert_eq!(seen, [planted.path().join("sub/leak")]);
    let dsn = format!("postgres://canon:{SECRET}@127.0.0.1:1/canon_v1");
    let environment = [
        ("CANON_PG_DSN", dsn.as_str()),
        ("DATABASE_URL", dsn.as_str()),
        ("CANON_R2_BUCKET", "canon-bucket"),
        ("CANON_S3_ENDPOINT", "http://127.0.0.1:1"),
        ("CANON_S3_ACCESS_KEY", "AKIA-access-s3cr3t-Marker-9f2c41"),
        ("CANON_S3_SECRET_KEY", SECRET),
    ];
    let calls: [&[&str]; 14] = [
        &["query", "--kind", "task"],
        &["query", "--kind", "trajectory"],
        &["query", "--kind", "change"],
        &["status"],
        &["gate", "check"],
        &["search", "x"],
        &["export", "--kind", "task"],
        &["purge", "--kind", "session", "--before", "2030-01-01"],
        &["purge", "--kind", "session", "--before", "2030-01-01", "--commit", "--plan-digest", "sha256:0000000000000000000000000000000000000000000000000000000000000000"],
        &["tier", "age"],
        &["artifacts", "move", "--tier", "cold"],
        &["artifacts", "verify"],
        &["ledger", "compact"],
        &["migrate"],
    ];
    // A call that reaches Postgres waits out the connection (30 seconds), so the calls run side by side, each in a repository of its own.
    let results: Vec<(String, bool, Vec<PathBuf>)> = std::thread::scope(|scope| {
        let mut running = Vec::new();
        for call in calls {
            for json_mode in [false, true] {
                let environment = &environment;
                running.push(scope.spawn(move || {
                    let dir = tempfile::tempdir().expect("tempdir");
                    let git = |args: &[&str]| assert!(Command::new("git").args(args).current_dir(dir.path()).output().expect("git").status.success(), "git {args:?}");
                    git(&["init", "-q"]);
                    git(&["-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "commit", "--allow-empty", "-qm", "fixture"]);
                    std::fs::write(dir.path().join("canon.yaml"), CANON_YAML).expect("write canon.yaml");
                    let mut command = hermetic::canon();
                    command.args(call).current_dir(dir.path()).env_remove("CANON_ROLE").env_remove("CANON_R2_LOCAL_ROOT");
                    if json_mode {
                        command.arg("--json");
                    }
                    for (name, value) in environment {
                        command.env(name, value);
                    }
                    let output = command.output().expect("spawn canon");
                    let (out, err) = (String::from_utf8_lossy(&output.stdout).into_owned(), String::from_utf8_lossy(&output.stderr).into_owned());
                    let leaked = out.contains(SECRET) || err.contains(SECRET);
                    let named_a_tier = ["postgres", "s3"].iter().any(|tier| out.contains(tier) || err.contains(tier));
                    let mut found = Vec::new();
                    files_holding(dir.path(), SECRET, &mut found);
                    (format!("{call:?} json={json_mode}{}", if leaked { " printed the secret" } else { "" }), named_a_tier, found)
                }));
            }
        }
        running.into_iter().map(|handle| handle.join().expect("a probe")).collect()
    });
    let leaks: Vec<&str> = results.iter().filter(|(label, _, _)| label.ends_with("printed the secret")).map(|(label, _, _)| label.as_str()).collect();
    assert_eq!(leaks, Vec::<&str>::new(), "answers that print the secret");
    // The calls reached the tiers (probes that never connect would prove nothing), and nothing they wrote holds the secret either.
    let named = results.iter().filter(|(_, named_a_tier, _)| *named_a_tier).count();
    assert!(named >= 6, "{named} of {} answers name a remote tier: the secret was never in play", results.len());
    let holders: Vec<&PathBuf> = results.iter().flat_map(|(_, _, found)| found).collect();
    assert_eq!(holders, Vec::<&PathBuf>::new(), "files that hold the secret");
}

#[test]
fn atip_claiml2_18_no_command_of_canon_reads_a_terminal_for_input_and_init_asks_nothing_on_one() {
    fn sources(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
        for entry in std::fs::read_dir(dir).expect("a directory") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                sources(&path, out);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                out.push((path.clone(), read(&path)));
            }
        }
    }
    let mut found = Vec::new();
    for krate in std::fs::read_dir(repo_root().join("crates")).expect("crates") {
        let src = krate.expect("a crate").path().join("src");
        if src.is_dir() {
            sources(&src, &mut found);
        }
    }
    assert!(!found.is_empty(), "the scan found canon's sources");
    let mut offenders: Vec<String> = Vec::new();
    for (path, text) in &found {
        for needle in ["/dev/tty", "rpassword", "dialoguer", "inquire::", "read_password", "termios"] {
            // The scan is for the tool that is checked: every crate of this workspace.
            if text.lines().any(|line| line.contains(needle) && !line.trim_start().starts_with("//")) {
                offenders.push(format!("{}: {needle}", path.display()));
            }
        }
    }
    assert_eq!(offenders, Vec::<String>::new(), "a source opens the controlling terminal or uses a prompt library");
    // Every read of the process's stdin, a path to it and every inherited stdio of a child: `--input -` alone, which refuses a terminal (`STDIN_TTY`)
    // instead of waiting. A new reader is a new prompt until the contract's 5.5 is met by it and it is named here.
    let mut readers: Vec<String> = found
        .iter()
        .filter(|(_, text)| text.lines().any(|line| !line.trim_start().starts_with("//") && (line.contains("io::stdin()") || line.contains("/dev/stdin") || line.contains("Stdio::inherit"))))
        .map(|(path, _)| path.strip_prefix(repo_root().join("crates")).unwrap_or(path).display().to_string().replace("/../", "/"))
        .collect();
    readers.sort();
    assert_eq!(readers, ["canon-cli/src/input.rs"], "a read of stdin that is not `--input -`");
    let input = read(&repo_root().join("crates/canon-cli/src/input.rs"));
    assert!(input.contains("STDIN_TTY") && input.contains("is_terminal()"), "`--input -` on a terminal is a refusal");
    // And running it: `canon init`, the one command that once asked, run with a terminal on stdin and stderr (a pty from `script`) prints no question and
    // installs for the detected harness. `init.rs` holds no prompt and nothing else reads a line.
    let init = read(&repo_root().join("crates/canon-cli/src/init.rs"));
    assert!(!init.contains("read_line") && !init.contains("prompt_providers"), "init asks nothing");
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir(dir.path().join(".claude")).expect("mkdir");
    let mut command = Command::new("script");
    hermetic::scrub(&mut command);
    if cfg!(target_os = "macos") {
        command.args(["-q", "/dev/null", CANON, "init"]);
    } else {
        command.args(["-qec", &format!("{} init", atip::quote(CANON)), "/dev/null"]);
    }
    // The pipe stays open while canon runs: a prompt would wait on it, and this test would not end.
    let mut child = command.current_dir(dir.path()).env_remove("CANON_ROLE").stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().expect("`script` gives canon a terminal; install it to run this case");
    let _held_open = child.stdin.take();
    let output = child.wait_with_output().expect("canon init on a terminal");
    let shown = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(output.status.success(), "{shown}");
    assert!(read(&dir.path().join("canon.yaml")).ends_with("skills:\n  providers: [claude]\n"), "init took the detected harness");
}
