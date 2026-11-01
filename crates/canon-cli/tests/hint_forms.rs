//! atip/1 1.1, 7.1.2: a `hint` is exactly one of a runnable invocation, a command template whose missing
//! values are `<UPPER_CASE>` placeholders, or one sentence of prose. A hint that starts with `canon ` is
//! one command line and nothing else: it splits as a POSIX shell would, names a command of canon's tree,
//! and uses only options and operands that command has. A prose hint never starts with `canon `.
//!
//! The test reads every hint the sources can print (the first string literal of each `.hint(…)` argument and
//! each `*HINT*` constant) and every hint the conformance corpus froze, and holds them to that rule.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use clap::error::ErrorKind;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())).map(|e| e.expect("dir entry").path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The text of the string literal that starts at `at` (a `"`), and the index just past it.
fn literal(text: &str, at: usize) -> (String, usize) {
    let bytes = text.as_bytes();
    let mut out = String::new();
    let mut i = at + 1;
    while i < bytes.len() && bytes[i] != b'"' {
        if bytes[i] == b'\\' {
            match bytes[i + 1] {
                b'n' => out.push('\n'),
                b'"' => out.push('"'),
                b'\\' => out.push('\\'),
                b'\'' => out.push('\''),
                other => out.push(other as char),
            }
            i += 2;
        } else {
            let ch = text[i..].chars().next().expect("a char");
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    (out, i + 1)
}

/// The first string literal in the balanced argument that starts after the `(` at `open`, if there is one before it closes.
fn first_literal(text: &str, open: usize) -> Option<String> {
    let bytes = text.as_bytes();
    let (mut depth, mut i) = (0usize, open);
    while i < bytes.len() {
        match bytes[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    return None;
                }
            }
            b'"' => return Some(literal(text, i).0),
            _ => {}
        }
        i += 1;
    }
    None
}

/// The hint templates the sources hold: `(file, text)`, a `{…}` hole already replaced by a placeholder word.
fn source_hints() -> Vec<(String, String)> {
    let mut files = Vec::new();
    rust_files(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    let mut found = Vec::new();
    for file in files {
        let name = file.strip_prefix(env!("CARGO_MANIFEST_DIR")).expect("under the crate").display().to_string();
        let raw = fs::read_to_string(&file).expect("read source");
        let live = raw.split("#[cfg(test)]").next().expect("a piece");
        let text: String = live.lines().map(|line| if line.trim_start().starts_with("//") { "" } else { line }).collect::<Vec<_>>().join("\n");
        for (at, _) in text.match_indices(".hint(") {
            if let Some(hint) = first_literal(&text, at + ".hint".len()) {
                found.push((name.clone(), hint));
            }
        }
        for (at, _) in text.match_indices("HINT") {
            let line_start = text[..at].rfind('\n').map_or(0, |n| n + 1);
            let line = &text[line_start..];
            if line.starts_with("const ") || line.starts_with("pub const ") || line.starts_with("pub(crate) const ") {
                if let Some(eq) = line.find("= \"") {
                    found.push((name.clone(), literal(line, eq + 2).0));
                }
            }
        }
    }
    found.into_iter().map(|(file, hint)| (file, fill_holes(&hint))).collect()
}

/// A format hole `{…}` becomes a word (`x`), and `{{`/`}}` the brace.
fn fill_holes(hint: &str) -> String {
    let mut out = String::new();
    let mut chars = hint.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push('}');
            }
            '{' => {
                for inner in chars.by_ref() {
                    if inner == '}' {
                        break;
                    }
                }
                out.push('x');
            }
            other => out.push(other),
        }
    }
    out
}

/// Why `hint` breaks the rule of 7.1.2, or `None`.
fn problem(hint: &str, root: &clap::Command) -> Option<String> {
    if !hint.starts_with("canon ") {
        return None;
    }
    if hint.contains('\n') {
        return Some("a hint that starts with `canon ` is one line".to_string());
    }
    if hint.contains('…') || hint.contains("...") {
        return Some("a hint that starts with `canon ` has no ellipsis".to_string());
    }
    // A placeholder is `<UPPER_CASE>`; any other `<` or `>` in a command line is not one.
    let mut stripped = String::new();
    let mut rest = hint;
    while let Some(open) = rest.find('<') {
        stripped.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('>') else { return Some("an unclosed `<`".to_string()) };
        let name = &rest[open + 1..open + close];
        let ok = name.starts_with(|c: char| c.is_ascii_uppercase()) && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
        if !ok {
            return Some(format!("`<{name}>` is not an `<UPPER_CASE>` placeholder"));
        }
        stripped.push('x');
        rest = &rest[open + close + 1..];
    }
    stripped.push_str(rest);
    let quotes = stripped.chars().filter(|c| *c == '"').count();
    if !quotes.is_multiple_of(2) || !stripped.matches('\'').count().is_multiple_of(2) {
        return Some("unbalanced quotes: it does not split as a shell would".to_string());
    }
    let words = atip::split_argv(&stripped);
    if words.first().map(String::as_str) != Some("canon") || words.len() < 2 {
        return Some("does not split into `canon` and a command".to_string());
    }
    let mut command = root.clone();
    command.build();
    // The command words of a `schema` hint are a path, which clap takes as operands: they must name a command.
    if words[1] == "schema" {
        let mut node = &command;
        for word in words[2..].iter().take_while(|word| !word.starts_with('-')) {
            match node.get_subcommands().find(|sub| sub.get_name() == word) {
                Some(sub) => node = sub,
                None => return Some(format!("`canon schema` names `{word}`, which is not a command")),
            }
        }
    }
    match command.try_get_matches_from(words) {
        Ok(_) => None,
        Err(error) => match error.kind() {
            ErrorKind::UnknownArgument | ErrorKind::InvalidSubcommand | ErrorKind::TooManyValues | ErrorKind::WrongNumberOfValues | ErrorKind::NoEquals => Some(format!("not a command line of canon: {}", error.render().to_string().lines().next().unwrap_or_default())),
            _ => None,
        },
    }
}

#[test]
fn atip_contract11_10_every_hint_in_the_sources_is_a_command_line_a_template_or_prose() {
    let root = canon_cli::cli::command();
    let hints = source_hints();
    assert!(hints.len() > 200, "the scan found {} hints; it cannot be looking at the sources", hints.len());
    let commands = hints.iter().filter(|(_, hint)| hint.starts_with("canon ")).count();
    assert!(commands >= 3, "the scan saw {commands} command-line hints");
    let problems: BTreeSet<String> = hints.iter().filter_map(|(file, hint)| problem(hint, &root).map(|why| format!("{file}: {hint:?}: {why}"))).collect();
    assert!(problems.is_empty(), "hints that break 7.1.2:\n{}", problems.into_iter().collect::<Vec<_>>().join("\n"));
}

#[test]
fn atip_contract11_10_every_hint_the_conformance_corpus_froze_follows_the_same_rule() {
    let root = canon_cli::cli::command();
    let mut files = Vec::new();
    let mut stack = vec![repo_root().join("conformance/cases")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("read dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "out") && path.parent().and_then(Path::file_name).is_some_and(|n| n == "expected") {
                files.push(path);
            }
        }
    }
    let (mut seen, mut problems) = (0, Vec::new());
    for file in files {
        for line in fs::read_to_string(&file).expect("read").lines().filter(|line| line.starts_with("{\"ok\":false")) {
            let document: serde_json::Value = serde_json::from_str(line).expect("a frozen envelope is JSON");
            if let Some(hint) = document["error"]["hint"].as_str() {
                seen += 1;
                if let Some(why) = problem(hint, &root) {
                    problems.push(format!("{}: {hint:?}: {why}", file.display()));
                }
            }
        }
    }
    assert!(seen >= 10, "the corpus froze {seen} hints");
    assert!(problems.is_empty(), "frozen hints that break 7.1.2:\n{}", problems.join("\n"));
}

#[test]
fn atip_contract11_10_the_rule_catches_prose_after_a_command_and_a_lowercase_placeholder() {
    let root = canon_cli::cli::command();
    assert!(problem("canon retrieve --for-role <ROLE> --domain <DOMAIN>", &root).is_none());
    assert!(problem("canon schema retrieve --json", &root).is_none());
    assert!(problem("run `canon schema --json` to list the commands", &root).is_none(), "prose may mention a command");
    assert!(problem("canon retrieve --for-role <ROLE> --domain <DOMAIN>, or --regime <KEY> (`canon regime show` prints a valid key)", &root).is_some(), "the 0.33 NoScope hint");
    assert!(problem("canon subject move demo --to verifying --reason \"<why it reopens>\" --as <role>", &root).is_some(), "a lowercase placeholder");
    assert!(problem("canon status and then read it", &root).is_some(), "prose after a command");
    assert!(problem("canon nosuch --json", &root).is_some(), "a command canon does not have");
    assert!(problem("canon schema nosuch --json", &root).is_some(), "a schema path that names no command");
}

/// Every command word path of canon's tree: `(path, is_group)`, the root first (`[]`).
fn nodes(command: &clap::Command, path: &mut Vec<String>, out: &mut Vec<(Vec<String>, bool)>) {
    let children: Vec<&clap::Command> = command.get_subcommands().filter(|sub| sub.get_name() != "help").collect();
    out.push((path.clone(), !children.is_empty()));
    for child in children {
        path.push(child.get_name().to_string());
        nodes(child, path, out);
        path.pop();
    }
}

/// The hints the binary builds at run time (the removed-spelling hints of `canon brief` and `canon report` run bare,
/// the usage hints made from the clap tree): every group run bare, every leaf with an option it does not have.
#[test]
fn atip_contract11_10_every_hint_the_binary_prints_for_a_bare_group_or_a_bad_option_follows_the_rule() {
    let root = canon_cli::cli::command();
    let mut all = Vec::new();
    nodes(&root, &mut Vec::new(), &mut all);
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut seen, mut groups, mut problems) = (0, 0, Vec::new());
    for (path, is_group) in &all {
        let mut args: Vec<&str> = path.iter().map(String::as_str).collect();
        if !is_group {
            args.push("--no-such-option-for-the-test");
        } else {
            groups += 1;
        }
        args.push("--json");
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_canon")).args(&args).current_dir(dir.path()).env_remove("CANON_ROLE").output().expect("spawn canon");
        let document: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{args:?}: stdout is not JSON ({e})"));
        if let Some(hint) = document["error"]["hint"].as_str() {
            seen += 1;
            if let Some(why) = problem(hint, &root) {
                problems.push(format!("canon {}: {hint:?}: {why}", args.join(" ")));
            }
        }
    }
    assert!(groups >= 10 && seen >= 100, "the run covered {groups} groups and {seen} hints");
    assert!(problems.is_empty(), "hints the binary prints that break 7.1.2:\n{}", problems.join("\n"));
    // The two removed groups of 0.28 answer with a sentence, not a command line.
    for group in ["brief", "report"] {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_canon")).args([group, "--json"]).current_dir(dir.path()).output().expect("spawn canon");
        let document: serde_json::Value = serde_json::from_slice(&out.stdout).expect("JSON");
        let hint = document["error"]["hint"].as_str().expect("a hint");
        assert!(!hint.starts_with("canon "), "{group}: {hint}");
    }
}
