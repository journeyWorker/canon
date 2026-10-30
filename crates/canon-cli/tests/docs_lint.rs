//! T14: every `canon …` command a reader or an agent is told to type parses
//! against the real clap tree, so the docs cannot drift from the CLI.
//!
//! What is checked, and where it comes from:
//! - the skill sources (`canon/skills/**/*.md`), the website docs
//!   (`packages/website/src/content/docs/**/*.mdx`) and the AGENTS block
//!   `canon init` writes;
//! - every code the error registry gives a `fix` for, and the hint of every
//!   removed spelling (`removed::find`);
//! - every `meta.next` command the per-leaf example sweep prints.
//!
//! Marker convention (kept small on purpose):
//! - A command in a fenced block tagged `bash`, `sh`, `shell`, `console` or
//!   untagged is a RUNNABLE line: it must parse in full (operands, required
//!   options), and may hold no `…`, `...` or `<placeholder>`. Pipes, `&&`,
//!   `;`, trailing `#` comments, `VAR=x` prefixes and `\` continuations are
//!   understood; only the segments that start with `canon ` are checked.
//! - A command in a fenced block tagged `text` or `syntax` is a SYNTAX line:
//!   `<placeholders>`, `[optional]` and `a|b` alternatives are allowed; the
//!   command path and every `--option` must exist.
//! - Inline code that starts with `canon ` is a SYNTAX line, except in a
//!   generated block's Example column (see `tests/doc_gen`), where it is
//!   RUNNABLE. To have a command in prose checked in full, put it in a
//!   `bash` block.
//! - `…` and `...` are never allowed in a command, in any mode.
//! - Other fences (`json`, `yaml`, `rust`, …) are not read.
//!
//! `PENDING` lists the lines that name something wave 4 still renames; an
//! entry that stops failing must be removed (this test says so), and phase B
//! of S8 empties the list.

#[expect(dead_code, reason = "this test uses only the lint half of the generator")]
mod doc_gen;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use clap::error::ErrorKind;
use serde_json::Value;
use tempfile::TempDir;

/// (source suffix, substring of the failing command) pairs that wave 4 owns.
const PENDING: &[(&str, &str)] = &[];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Runnable,
    Syntax,
}

#[derive(Debug)]
struct Cmd {
    source: String,
    text: String,
    mode: Mode,
}

/// Splits one shell segment into words: whitespace separates, quotes group;
/// an unquoted `#` after whitespace ends the segment.
fn words(segment: &str) -> Vec<String> {
    let (mut out, mut cur, mut quote, mut any) = (Vec::new(), String::new(), None::<char>, false);
    let mut prev_space = true;
    let mut chars = segment.chars().peekable();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some('"'), '\\') => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            (Some(_), c) => cur.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                any = true;
            }
            (None, '#') if prev_space => break,
            (None, c) if c.is_whitespace() => {
                if any {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            (None, c) => {
                cur.push(c);
                any = true;
            }
        }
        prev_space = quote.is_none() && c.is_whitespace();
    }
    if any {
        out.push(cur);
    }
    out
}

/// The `canon …` segments of one shell line: split at unquoted `|`, `||`,
/// `&&` and `;`, cut at a heredoc or redirect, drop `VAR=x` prefixes.
fn canon_segments(line: &str, redirects: bool) -> Vec<String> {
    let mut segments = Vec::new();
    let (mut cur, mut quote) = (String::new(), None::<char>);
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let flush = |cur: &mut String, segments: &mut Vec<String>| {
        let seg = std::mem::take(cur);
        segments.push(seg);
    };
    while i < chars.len() {
        let c = chars[i];
        match quote {
            Some(q) => {
                cur.push(c);
                if c == q {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => {
                    quote = Some(c);
                    cur.push(c);
                }
                '#' if cur.chars().last().is_none_or(char::is_whitespace) => break,
                '|' | ';' => flush(&mut cur, &mut segments),
                '&' if chars.get(i + 1) == Some(&'&') => {
                    flush(&mut cur, &mut segments);
                    i += 1;
                }
                '>' if redirects => break,
                '<' if redirects && chars.get(i + 1) == Some(&'<') || cur.chars().last().is_none_or(char::is_whitespace) && chars.get(i + 1).is_some_and(|n| n.is_whitespace()) => break,
                c => cur.push(c),
            },
        }
        i += 1;
    }
    flush(&mut cur, &mut segments);
    segments
        .into_iter()
        .filter_map(|seg| {
            let mut rest = seg.trim();
            while let Some((word, tail)) = rest.split_once(char::is_whitespace) {
                let is_env = word.split_once('=').is_some_and(|(k, _)| !k.is_empty() && k.chars().all(|c| c.is_ascii_uppercase() || c == '_'));
                if !is_env {
                    break;
                }
                rest = tail.trim_start();
            }
            (rest.starts_with("canon ") || rest == "canon").then(|| rest.to_string())
        })
        .collect()
}

/// The `canon …` commands of one inline code span: split at ` && ` and `; ` (a `|` there is an alternative, not a
/// pipe). A span that quotes what canon prints (`canon review scenario: pinned …`, a command word ending in `:`) is
/// output, not a command.
fn inline_commands(code: &str) -> Vec<String> {
    code.split(" && ")
        .flat_map(|part| part.split("; "))
        .map(str::trim)
        .filter(|part| part.starts_with("canon ") && !part.split_whitespace().take_while(|w| !w.starts_with(['-', '<'])).any(|w| w.ends_with(':')))
        .map(str::to_string)
        .collect()
}

/// Every command in `text` (markdown), by mode, with its `source:line`.
fn extract(text: &str, mdx: bool, source: &str) -> Vec<Cmd> {
    let mut out = Vec::new();
    let mut fence: Option<(String, Option<Mode>)> = None;
    let mut generated = false;
    let mut continued = String::new();
    let mut continued_at = 0;
    // The prose paragraph read so far, from its first line's number: an inline code span may wrap its lines.
    let mut paragraph = String::new();
    let mut paragraph_at = 0;
    let flush = |paragraph: &mut String, at: usize, out: &mut Vec<Cmd>| {
        for (offset, code) in code_spans(paragraph) {
            for text in inline_commands(&code) {
                out.push(Cmd { source: format!("{source}:{}", at + offset), text, mode: Mode::Syntax });
            }
        }
        paragraph.clear();
    };
    for (n, raw) in text.lines().enumerate() {
        let n = n + 1;
        let line = raw.trim();
        if let Some((marker, _)) = &fence {
            if line.starts_with(marker.as_str()) && line.trim_start_matches(marker.chars().next().unwrap()).trim().is_empty() {
                fence = None;
                continued.clear();
                continue;
            }
        } else if let Some(rest) = line.strip_prefix("```").or_else(|| line.strip_prefix("~~~")) {
            flush(&mut paragraph, paragraph_at, &mut out);
            let marker = line[..3].to_string();
            let _ = rest;
            let info = line.trim_start_matches(marker.chars().next().unwrap()).trim().split([' ', ',', '{']).next().unwrap_or("").to_ascii_lowercase();
            let mode = match info.as_str() {
                "" | "bash" | "sh" | "shell" | "console" | "zsh" => Some(Mode::Runnable),
                "text" | "syntax" => Some(Mode::Syntax),
                _ => None,
            };
            fence = Some((marker, mode));
            continue;
        }
        if let Some((_, mode)) = &fence {
            let Some(mode) = *mode else { continue };
            let mut line = line.trim_start_matches("$ ").to_string();
            if !continued.is_empty() {
                line = format!("{continued} {line}");
            } else {
                continued_at = n;
            }
            if let Some(stripped) = line.strip_suffix('\\') {
                continued = stripped.trim_end().to_string();
                continue;
            }
            continued.clear();
            for seg in canon_segments(&line, mode == Mode::Runnable) {
                out.push(Cmd { source: format!("{source}:{continued_at}"), text: seg, mode });
            }
            continue;
        }
        if line.contains("canon:generated ") {
            flush(&mut paragraph, paragraph_at, &mut out);
            generated = line.contains(" begin ");
            continue;
        }
        let _ = mdx;
        // A table row is a block of its own; in a generated block its last cell is the runnable example.
        if line.starts_with('|') {
            flush(&mut paragraph, paragraph_at, &mut out);
            let codes = code_spans(&raw.replace("\\|", "|"));
            let last = codes.len().saturating_sub(1);
            for (i, (_, code)) in codes.into_iter().enumerate() {
                for text in inline_commands(&code) {
                    let placeholder = text.contains('<') || text.contains('[') || text.contains('|');
                    let example_cell = generated && i == last;
                    let mode = if example_cell && !placeholder { Mode::Runnable } else { Mode::Syntax };
                    out.push(Cmd { source: format!("{source}:{n}"), text, mode });
                }
            }
            continue;
        }
        if line.is_empty() {
            flush(&mut paragraph, paragraph_at, &mut out);
            continue;
        }
        if paragraph.is_empty() {
            paragraph_at = n;
        } else {
            paragraph.push('\n');
        }
        paragraph.push_str(line);
    }
    flush(&mut paragraph, paragraph_at, &mut out);
    out
}

/// The inline code spans of `text` (one paragraph or table row), as markdown reads them, each with the offset of the
/// line it starts on: a run of N backticks opens a span that the next run of exactly N closes, so a span may wrap
/// lines (a line break reads as a space) and hold a shorter run of backticks; an opening run with no closer is text.
fn code_spans(text: &str) -> Vec<(usize, String)> {
    let bytes = text.as_bytes();
    let run_at = |i: usize| bytes[i..].iter().take_while(|&&b| b == b'`').count();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'`' {
            i += 1;
            continue;
        }
        let open = run_at(i);
        let mut j = i + open;
        let close = loop {
            match bytes[j..].iter().position(|&b| b == b'`') {
                None => break None,
                Some(at) => {
                    let run = run_at(j + at);
                    if run == open {
                        break Some(j + at);
                    }
                    j += at + run;
                }
            }
        };
        match close {
            Some(end) => {
                let line = text[..i].matches('\n').count();
                spans.push((line, text[i + open..end].replace('\n', " ").trim().to_string()));
                i = end + open;
            }
            None => i += open,
        }
    }
    spans
}

fn placeholder_in(text: &str) -> Option<String> {
    if text.contains('…') || text.contains("...") {
        return Some("an ellipsis".into());
    }
    let b = text.as_bytes();
    for (i, c) in text.char_indices() {
        if c == '<' && b.get(i + 1).is_some_and(u8::is_ascii_alphabetic) {
            if let Some(end) = text[i..].find('>') {
                if !text[i + 1..i + end].contains(char::is_whitespace) || text[i + 1..i + end].len() < 40 {
                    return Some(format!("a placeholder {}", &text[i..=i + end]));
                }
            }
        }
    }
    None
}

fn has_option(cmd: &clap::Command, root: &clap::Command, name: &str) -> bool {
    let long = |c: &clap::Command| c.get_arguments().any(|a| a.get_long() == Some(name) || a.get_all_aliases().into_iter().flatten().any(|x| x == name));
    name == "help" || name == "version" || long(cmd) || long(root)
}

/// SYNTAX mode: the command path resolves and every `--option` exists.
fn check_syntax(root: &clap::Command, text: &str) -> Result<(), String> {
    let ws = words(text);
    let mut cur = root;
    let mut i = 1;
    let mut open = false;
    while let Some(w) = ws.get(i) {
        let w = w.trim_matches(['[', ']']);
        if w.is_empty() || w.starts_with('-') || w.starts_with('<') {
            break;
        }
        let alts: Vec<&str> = w.split('|').collect();
        let subs = |name: &str| cur.get_subcommands().find(|s| s.get_name() == name || s.get_all_aliases().any(|a| a == name));
        if alts.iter().all(|a| *a == "help" || subs(a).is_some()) {
            if alts.len() == 1 {
                cur = subs(alts[0]).unwrap_or(cur);
            } else {
                open = true;
                break;
            }
            i += 1;
        } else if cur.has_subcommands() && !w.contains(['.', '/', '=']) && w.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
            return Err(format!("`{w}` is not a subcommand of `{}`", cur.get_name()));
        } else {
            break;
        }
    }
    if open {
        return Ok(());
    }
    for w in &ws[i..] {
        let w = w.trim_matches(['[', ']', ',', '.', ')', '(', '>']);
        if let Some(rest) = w.strip_prefix("--") {
            for alt in rest.split('|') {
                let name = alt.split('=').next().unwrap_or("").trim_start_matches("--").trim_matches(['[', ']', '<', '>']);
                if name.is_empty() || !name.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
                    continue;
                }
                if !has_option(cur, root, name) {
                    return Err(format!("`--{name}` is not an option of `{}`", words_path(root, cur)));
                }
            }
        }
    }
    Ok(())
}

fn words_path(_root: &clap::Command, cur: &clap::Command) -> String {
    cur.get_name().to_string()
}

/// RUNNABLE mode: the line parses in full.
fn check_runnable(root: &clap::Command, text: &str) -> Result<(), String> {
    let ws = words(text);
    match root.clone().try_get_matches_from(ws) {
        Ok(_) => Ok(()),
        Err(e) if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand) => Ok(()),
        Err(e) => Err(e.to_string().lines().filter(|l| !l.trim().is_empty()).take(2).collect::<Vec<_>>().join(" ")),
    }
}

fn check(root: &clap::Command, cmd: &Cmd) -> Option<String> {
    let result = match placeholder_in(&cmd.text) {
        Some(what) if what == "an ellipsis" || cmd.mode == Mode::Runnable => Err(format!("holds {what}")),
        _ => match cmd.mode {
            Mode::Syntax => check_syntax(root, &cmd.text),
            Mode::Runnable => check_runnable(root, &cmd.text),
        },
    };
    result.err().map(|e| format!("{}: {e}\n    {}", cmd.source, cmd.text))
}

fn pending(cmd_source: &str, text: &str) -> bool {
    PENDING.iter().any(|(src, sub)| cmd_source.contains(src) && text.contains(sub))
}

fn markdown_commands(spec_root: &Path) -> Vec<Cmd> {
    let mut out = Vec::new();
    for path in doc_gen::doc_files() {
        let rel = doc_gen::rel_of(&path);
        if rel == "CHANGELOG.md" {
            continue;
        }
        let text = fs::read_to_string(&path).expect("read doc");
        out.extend(extract(&text, rel.ends_with(".mdx"), &rel));
    }
    out.extend(extract(canon_cli::init::AGENTS_BLOCK, false, "AGENTS block (crates/canon-cli/src/init.rs)"));
    let _ = spec_root;
    out
}

fn registry_commands(spec: &Value, root: &clap::Command) -> Vec<Cmd> {
    let mut out = Vec::new();
    for error in spec["errors"].as_array().expect("errors") {
        let code = error["code"].as_str().expect("code");
        out.extend(extract(error["fix"].as_str().unwrap_or_default(), false, &format!("error registry {code} fix")));
    }
    for row in spec["removed"].as_array().expect("removed") {
        let old = row["old"].as_str().expect("old");
        let scope = row["scope"].as_array().and_then(|s| s.first()).and_then(Value::as_str);
        let args: Vec<String> = match row["kind"].as_str() {
            Some("option") => scope.unwrap_or("").split(' ').filter(|w| !w.is_empty()).map(str::to_string).chain(std::iter::once(old.to_string())).collect(),
            _ => old.split(' ').map(str::to_string).collect(),
        };
        if let Some(removed) = canon_cli::removed::find(root, &args) {
            out.extend(extract(&removed.hint(), false, &format!("removed hint of `{old}`")));
        }
        let new = row["new"].as_str().expect("new");
        if !new.starts_with('-') {
            out.push(Cmd { source: format!("removed table: replacement of `{old}`"), text: format!("canon {new}"), mode: Mode::Syntax });
        }
    }
    out
}

fn run(dir: &Path, home: &Path, args: &[String]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(dir).env("CANON_ACTOR", "docs-lint").env("HOME", home).env("CANON_SKILLS_SOURCE", concat!(env!("CARGO_MANIFEST_DIR"), "/../../canon/skills")).output().expect("spawn canon")
}

fn ok(dir: &Path, home: &Path, args: &[&str]) {
    let out = run(dir, home, &args.iter().map(std::string::ToString::to_string).collect::<Vec<_>>());
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stdout));
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Every agent-typed `meta.next` command the per-leaf example sweep prints.
fn next_commands(spec: &Value) -> Vec<Cmd> {
    let home = TempDir::new().unwrap();
    let base = TempDir::new().unwrap();
    let repo = base.path();
    ok(repo, home.path(), &["init", "--providers", "none"]);
    ok(repo, home.path(), &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(repo, home.path(), &["change", "new", "add-login", "--subject", "demo", "--title", "Add login"]);
    ok(repo, home.path(), &["feature", "new", "auth.login", "--title", "Login"]);
    ok(repo, home.path(), &["scenario", "new", "auth.login.01", "--title", "Sign in", "--feature", ".canon/specs/kind=feature/area=auth/login.feature"]);
    ok(repo, home.path(), &["inventory", "sync"]);
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for leaf in spec["commands"].as_array().expect("commands").iter().filter(|n| n["nodeKind"] == "leaf") {
        let command = leaf["command"].as_str().expect("command");
        if matches!(command, "dashboard" | "init") {
            continue;
        }
        let scratch = TempDir::new().unwrap();
        let cwd = scratch.path().join("repo");
        copy_dir(base.path(), &cwd);
        let mut argv: Vec<String> = canon_cli::next::split_argv(leaf["examples"][0].as_str().expect("example"));
        argv.remove(0);
        argv.push("--json".into());
        let output = run(&cwd, home.path(), &argv);
        let Ok(doc) = serde_json::from_slice::<Value>(&output.stdout) else { continue };
        for step in doc["meta"]["next"].as_array().map(std::vec::Vec::as_slice).unwrap_or_default() {
            if step["actor"] == "agent" && step["command"].as_str().is_some_and(|c| c.starts_with("canon ")) {
                let text = step["command"].as_str().unwrap().to_string();
                if seen.insert(text.clone()) {
                    let mode = if placeholder_in(&text).is_some() { Mode::Syntax } else { Mode::Runnable };
                    out.push(Cmd { source: format!("meta.next of `{command}`"), text, mode });
                }
            }
        }
    }
    out
}

/// context.capability.57: every command line of the skills, the website and the hints parses against the clap tree.
#[test]
fn context_capability_57_every_command_in_the_docs_and_hints_parses() {
    let spec = canon_cli::spec::generate();
    let root = canon_cli::cli::command();
    let mut commands = markdown_commands(&doc_gen::repo_root());
    commands.extend(registry_commands(&spec, &root));
    assert!(commands.len() > 200, "found only {} commands", commands.len());
    assert_all_parse(&root, &commands, true);
}

/// context.capability.56: the `meta.next` commands the example sweep prints: an agent runs these, so they parse too.
/// (A step that has a `<placeholder>` for what only the agent knows is checked as a syntax line.)
#[test]
fn context_capability_56_every_meta_next_command_parses() {
    let spec = canon_cli::spec::generate();
    let root = canon_cli::cli::command();
    let commands = next_commands(&spec);
    assert!(commands.len() > 5, "found only {} commands", commands.len());
    assert_all_parse(&root, &commands, false);
}

fn assert_all_parse(root: &clap::Command, commands: &[Cmd], stale_pending: bool) {

    let mut problems = Vec::new();
    let mut hit = BTreeSet::new();
    for cmd in commands {
        if let Some(problem) = check(root, cmd) {
            match PENDING.iter().position(|(src, sub)| pending(&cmd.source, &cmd.text) && cmd.source.contains(src) && cmd.text.contains(sub)) {
                Some(i) => {
                    hit.insert(i);
                }
                None => problems.push(problem),
            }
        }
    }
    for (i, (src, sub)) in PENDING.iter().enumerate() {
        if stale_pending && !hit.contains(&i) {
            problems.push(format!("PENDING entry ({src}, {sub}) no longer fails; remove it"));
        }
    }
    assert!(problems.is_empty(), "{} problem(s):\n{}", problems.len(), problems.join("\n"));
}

/// context.capability.57: an inline span that wraps a line, or is delimited by two backticks, is read whole and checked.
#[test]
fn context_capability_57_a_code_span_that_wraps_a_line_or_uses_double_backticks_is_read_whole() {
    let md = "Run `canon review\nadd auth.login.01` to sign.\nOr ``canon query --kind `evidence` --bogus`` here, and `canon\ngate check`.\n\n`canon status` stands alone, and an unclosed ` stays text.\n| a | `canon\n";
    let cmds = extract(md, false, "t.md");
    let shown: Vec<(&str, &str)> = cmds.iter().map(|c| (c.text.as_str(), c.source.as_str())).collect();
    assert_eq!(shown, [("canon review add auth.login.01", "t.md:1"), ("canon query --kind `evidence` --bogus", "t.md:3"), ("canon gate check", "t.md:3"), ("canon status", "t.md:6")]);
    let root = canon_cli::cli::command();
    assert!(check(&root, &cmds[0]).is_some_and(|p| p.contains("t.md:1")), "the obsolete `review add` is caught");
    assert!(check(&root, &cmds[1]).is_some(), "an option the leaf lacks inside a double-backtick span is caught");
    assert!(check(&root, &cmds[2]).is_none() && check(&root, &cmds[3]).is_none());
}

#[test]
fn the_extractor_reads_what_the_convention_says() {
    let md = "```bash\nCANON_ROLE=reviewer canon status --json | jq .\ncanon gate check # trailing\nnpm test\n```\n```text\ncanon evidence add <flags>\n```\nProse `canon query --kind <kind>` and `npm i`.\n";
    let cmds = extract(md, false, "t.md");
    let shown: Vec<(&str, Mode)> = cmds.iter().map(|c| (c.text.as_str(), c.mode)).collect();
    assert_eq!(shown, [("canon status --json", Mode::Runnable), ("canon gate check", Mode::Runnable), ("canon evidence add <flags>", Mode::Syntax), ("canon query --kind <kind>", Mode::Syntax)]);
    let root = canon_cli::cli::command();
    assert!(check(&root, &cmds[2]).is_none());
    let bad = Cmd { source: "x".into(), text: "canon gate check --nope".into(), mode: Mode::Runnable };
    assert!(check(&root, &bad).is_some());
    let ellipsis = Cmd { source: "x".into(), text: "canon gate check …".into(), mode: Mode::Syntax };
    assert!(check(&root, &ellipsis).is_some());
    let placeholder = Cmd { source: "x".into(), text: "canon schema <path>".into(), mode: Mode::Runnable };
    assert!(check(&root, &placeholder).is_some());
    let agents = extract(canon_cli::init::AGENTS_BLOCK, false, "AGENTS block");
    assert!(agents.len() >= 8, "the AGENTS block yields {} commands", agents.len());
}

/// context.capability.58: a skill that keeps an id-suffixed option fails the parse check, whatever the mode of its line.
#[test]
fn context_capability_58_a_skill_command_that_keeps_an_old_id_spelling_fails_the_parse_check() {
    let root = canon_cli::cli::command();
    let old = "```bash\ncanon evidence add --actor-id impl-1 --scenario auth.login.01 --class test-run --ref r\n```\n```text\ncanon finding list --change-id <slug>\n```\nProse `canon scenario new <id> --project-id <root>`.\n";
    let cmds = extract(old, false, "reference/role-implementer.md");
    assert_eq!(cmds.len(), 3, "{cmds:?}");
    for cmd in &cmds {
        let problem = check(&root, cmd).unwrap_or_else(|| panic!("`{}` passes the parse check", cmd.text));
        assert!(problem.contains("role-implementer.md") && problem.contains("-id"), "{problem}");
    }
    let new = old.replace("--actor-id", "--actor").replace("--change-id", "--change").replace("--project-id", "--project");
    for cmd in extract(&new, false, "reference/role-implementer.md") {
        assert!(check(&root, &cmd).is_none(), "`{}` fails: {:?}", cmd.text, check(&root, &cmd));
    }
}

/// A guide is read through `canon skills show`, and the consumer's repo holds no `reference/` tree, so no guide
/// points at a `reference/<file>.md` path; it names `canon skills show <name>` instead. Every name it shows exists.
#[test]
fn the_guides_point_at_skills_show_and_every_name_exists() {
    let names: BTreeSet<String> = canon_cli::guide::names().iter().map(std::string::ToString::to_string).collect();
    let mut problems = Vec::new();
    for (path, text) in canon_cli::guide::files() {
        if !path.ends_with(".md") {
            continue;
        }
        if text.contains("reference/") {
            problems.push(format!("{path}: names a reference/ path"));
        }
        for (at, _) in text.match_indices("canon skills show ") {
            let name: String = text[at + 18..].chars().take_while(|c| c.is_ascii_lowercase() || *c == '-').collect();
            if !name.is_empty() && !name.ends_with('-') && !names.contains(&name) {
                problems.push(format!("{path}: `canon skills show {name}` is not a guide"));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
