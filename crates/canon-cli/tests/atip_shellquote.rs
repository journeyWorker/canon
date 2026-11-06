//! `canon-0-35-crate-shell`, seen from canon (scenarios `atip.shellquote.*`): the `command` of every next step canon prints is its `argv` written
//! as one shell line (`atip::quote` on each word), and reading that line back (`atip::split_argv`) gives the tool and the `argv` again, so a
//! value with an apostrophe, a space or a character the shell reads is not broken on the way.
//!
//! The values come from `--repo`: a repository directory may have any name, and the first page of a listing carries `--repo=<name>` into its
//! next call. Every test runs the built binary. Tests are named by scenario id (`atip_shellquote_01_…` proves `atip.shellquote.01`).

#[path = "support/hermetic.rs"]
mod hermetic;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

/// A directory name and the word `--repo=<name>` as a POSIX shell reads it back, written out by hand: a word of letters, digits and
/// `-_./:=@,+` stays bare, any other is in single quotes and an apostrophe in it is `'\''`.
const NAMES: [(&str, &str); 12] = [
    ("plain", "--repo=plain"),
    ("it's", r"'--repo=it'\''s'"),
    ("a''b", r"'--repo=a'\'''\''b'"),
    ("a b", "'--repo=a b'"),
    ("say \"hi\"", "'--repo=say \"hi\"'"),
    ("back\\slash", "'--repo=back\\slash'"),
    ("a$HOME", "'--repo=a$HOME'"),
    ("a`id`b", "'--repo=a`id`b'"),
    ("tab\there", "'--repo=tab\there'"),
    ("two\nlines", "'--repo=two\nlines'"),
    ("한글", "'--repo=한글'"),
    ("x;y&z|w", "'--repo=x;y&z|w'"),
];

fn canon_in(dir: &Path, args: &[&str]) -> Output {
    hermetic::canon().args(args).current_dir(dir).env_remove("CANON_ROLE").env("CANON_ACTOR", "atip-shellquote").output().expect("spawn canon binary")
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("stdout is one JSON document ({e}): {}\nstderr: {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)))
}

fn ok(dir: &Path, args: &[&str]) {
    let out = canon_in(dir, args);
    assert!(out.status.success(), "{args:?}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}

/// A directory with a repository in a directory called `name`, holding two subjects (so a listing of one has a next page). Returns the parent,
/// which is where the calls run, and `name` is given to them as `--repo`.
fn parent_of_repo(name: &str) -> TempDir {
    let parent = TempDir::new().expect("a temp dir");
    let repo: PathBuf = parent.path().join(name);
    std::fs::create_dir(&repo).unwrap_or_else(|e| panic!("a directory called {name:?}: {e}"));
    ok(&repo, &["init", "--providers", "none"]);
    for subject in ["one", "two"] {
        ok(&repo, &["subject", "new", subject, "--domain", "dev", "--title", subject]);
    }
    parent
}

/// The first page of the subjects of the repository `name`, and its `meta.next[0]`: the command, the argv, the cursor and the whole page.
struct Page {
    command: String,
    argv: Vec<String>,
    page: Value,
}

fn first_page(parent: &Path, name: &str) -> Page {
    let page = json(&canon_in(parent, &["query", "--kind", "subject", "--limit", "1", "--repo", name, "--json"]));
    assert_eq!(page["ok"], true, "{name:?}: {page}");
    let step = &page["meta"]["next"][0];
    let command = step["command"].as_str().unwrap_or_else(|| panic!("{name:?}: a next step with a command: {page}")).to_string();
    let argv = step["argv"].as_array().expect("an argv").iter().map(|word| word.as_str().expect("a word").to_string()).collect();
    Page { command, argv, page }
}

/// `word` as a POSIX shell word, written for this test apart from the crate: bare when it is made of letters, digits and `-_./:=@,+`, else in
/// single quotes with each apostrophe closed, escaped and reopened.
fn reference_quote(word: &str) -> String {
    if !word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric() || "-_./:=@,+".contains(c)) {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
}

/// atip.shellquote.01: the word that holds the repository's name is quoted as a shell reads it; every other word is bare.
#[test]
fn atip_shellquote_01_a_word_with_an_apostrophe_is_closed_escaped_and_reopened_and_a_safe_word_stays_bare() {
    for (name, quoted) in NAMES {
        let parent = parent_of_repo(name);
        let page = first_page(parent.path(), name);
        assert!(page.argv.iter().any(|word| *word == format!("--repo={name}")), "{name:?}: the argv carries the repository as given: {:?}", page.argv);
        // The word is where the argv has it, quoted by hand.
        let words: Vec<&str> = page.command.strip_prefix("canon ").unwrap_or_else(|| panic!("{name:?}: the command starts with the tool: {}", page.command)).split(' ').collect();
        if name.contains(['\n', ' ', '\t']) {
            // A quoted word with a space, tab or newline in it is more than one piece when cut at a space: look for the whole quoted word instead.
            assert!(page.command.contains(&format!(" {quoted} ")), "{name:?}: {}", page.command);
        } else {
            assert!(words.contains(&quoted), "{name:?}: the word is {quoted}: {}", page.command);
        }
        // Every other word of the command is bare: it has no quote, so a quote is only where a character needs it.
        let rest = page.command.replacen(quoted, "", 1);
        assert!(!rest.contains('\'') && !rest.contains('"') && !rest.contains('\\'), "{name:?}: no other word is quoted: {rest}");
        // The command is canon and the words, each quoted by the reference.
        let mut line = vec!["canon".to_string()];
        line.extend(page.argv.iter().map(|word| reference_quote(word)));
        assert_eq!(page.command, line.join(" "), "{name:?}");
        // A safe name has no quote; any other has the quoted word and its apostrophes as `'\''`.
        assert_eq!(quoted.starts_with('\''), name != "plain", "{name:?}");
        assert_eq!(quoted.matches(r"'\''").count(), name.matches('\'').count(), "{name:?}: each apostrophe is closed, escaped and reopened once");
    }
}

/// The directory that holds the `canon` binary, put first on `PATH` so that `sh` finds the tool the `command` names.
fn path_with_canon() -> String {
    let binary = Path::new(env!("CARGO_BIN_EXE_canon"));
    assert_eq!(binary.file_name().and_then(|name| name.to_str()), Some("canon"));
    format!("{}:{}", binary.parent().expect("a directory").display(), std::env::var("PATH").unwrap_or_default())
}

/// atip.shellquote.02: the command is the call again.
#[test]
fn atip_shellquote_02_the_command_splits_into_the_tool_and_the_argv_and_a_shell_reads_the_same_page() {
    let path = path_with_canon();
    for (name, _) in NAMES {
        let parent = parent_of_repo(name);
        let page = first_page(parent.path(), name);
        let mut want = vec!["canon".to_string()];
        want.extend(page.argv.iter().cloned());
        assert_eq!(atip::split_argv(&page.command), want, "{name:?}: {}", page.command);
        // The argv run as it is, and the command run by a shell, read the same page.
        let words: Vec<&str> = page.argv.iter().map(String::as_str).collect();
        let direct = json(&canon_in(parent.path(), &words));
        let by_shell = Command::new("sh").arg("-c").arg(&page.command).current_dir(parent.path()).env("PATH", &path).env_remove("CANON_ROLE").env("CANON_ACTOR", "atip-shellquote").output().expect("spawn sh");
        let shell = json(&by_shell);
        assert_eq!(shell["ok"], true, "{name:?}: the shell runs the command: {shell}");
        assert_eq!(shell["data"], direct["data"], "{name:?}: the same records");
        assert_eq!(shell["meta"]["page"], direct["meta"]["page"], "{name:?}: the same page");
        assert_ne!(shell["data"], page.page["data"], "{name:?}: and it is the next page, not the first");
        assert_eq!(shell["meta"]["page"]["returned"], 1, "{name:?}");
    }
    // The steps of `status` are split into the tool and the argv too: a check that compares the two finds nothing to report.
    let parent = parent_of_repo("plain");
    let repo = parent.path().join("plain");
    ok(&repo, &["change", "new", "add-login", "--subject", "one", "--title", "Add login"]);
    ok(&repo, &["feature", "new", "auth.login", "--title", "Login"]);
    ok(&repo, &["scenario", "new", "auth.login.01", "--title", "Sign in", "--feature", ".canon/specs/kind=feature/area=auth/login.feature"]);
    ok(&repo, &["inventory", "sync"]);
    let mut seen = 0;
    for role in ["implementer", "planner", "reviewer", "orchestrator"] {
        let out = hermetic::canon().args(["status", "--json"]).current_dir(&repo).env("CANON_ROLE", role).env("CANON_ACTOR", "atip-shellquote").output().expect("spawn canon binary");
        for step in json(&out)["meta"]["next"].as_array().into_iter().flatten() {
            seen += 1;
            let mut want = vec!["canon".to_string()];
            want.extend(step["argv"].as_array().unwrap().iter().map(|word| word.as_str().unwrap().to_string()));
            assert_eq!(atip::split_argv(step["command"].as_str().unwrap()), want, "{role}: {step}");
        }
    }
    assert!(seen > 0, "status has steps to compare");
    // The word a single quote used to break: it is `it's` and not `it\s`.
    assert_eq!(atip::split_argv(r"canon x 'it'\''s'"), ["canon", "x", "it's"]);
}

/// atip.shellquote.03: a command that is not canon's own is told apart.
#[test]
fn atip_shellquote_03_a_command_with_a_bare_apostrophe_a_bare_word_or_a_dropped_quote_is_told_apart_from_canons_own() {
    for (name, quoted) in NAMES {
        let parent = parent_of_repo(name);
        let page = first_page(parent.path(), name);
        let mut want = vec!["canon".to_string()];
        want.extend(page.argv.iter().cloned());
        assert_eq!(atip::split_argv(&page.command), want, "{name:?}: canon's own command splits back");
        let raw = format!("--repo={name}");
        let mut damaged: Vec<(&str, String)> = Vec::new();
        // The word with its closing quote dropped: the quote reaches to the end of the line and takes the rest of the words with it.
        if let Some(open) = quoted.strip_suffix('\'') {
            damaged.push(("the closing quote dropped", page.command.replacen(quoted, open, 1)));
        }
        // The word written bare, when a shell reads something in it: a separator, a quote or a backslash.
        if name.contains([' ', '\t', '\n', '\'', '"', '\\']) {
            damaged.push(("the word bare", page.command.replacen(quoted, &raw, 1)));
        }
        // An apostrophe written as it is inside single quotes, without `'\''`.
        if name.contains('\'') {
            damaged.push(("the apostrophe without its escape", page.command.replacen(quoted, &format!("'{raw}'"), 1)));
        }
        for (what, command) in damaged {
            assert_ne!(command, page.command, "{name:?}: {what} changes the command");
            assert_ne!(atip::split_argv(&command), want, "{name:?}: {what} is not the argv: {command}");
        }
    }
    // A quote that never closes is read to the end of the line, with no panic, and a lone backslash stays one.
    for command in ["canon x 'never closed", "canon x \"never closed", "canon x \\", "canon x 'a'\\''b", "'", "\""] {
        let words = atip::split_argv(command);
        assert_eq!(words.first().map(String::as_str), if command.starts_with("canon") { Some("canon") } else { Some("") }, "{command:?}: {words:?}");
    }
}
