//! The derived documentation blocks (0.28 T14): text between
//! `canon:generated <name> begin|end` markers that is rendered from the
//! command spec, never hand-written.
//!
//! Marker convention. A block starts on a line holding
//! `canon:generated <name> begin` and ends on a line holding
//! `canon:generated <name> end`; in `.md` the markers are HTML comments, in
//! `.mdx` JSX comments (`{/* … */}`). Everything between them is replaced;
//! everything outside is hand-written. Names:
//!
//! - `commands-<role>`: the role's leaves with one runnable example each
//!   (`canon/skills/reference/role-<role>.md`);
//! - `cli-<group>` (`cli-top` for the top-level leaves): one table per command
//!   group (`packages/website/src/content/docs/{,ko/}cli.mdx`);
//! - `removed`: the 0.27 → 0.28 table (`README.md`).
//!
//! `tests/docs_generated.rs` rewrites (`CANON_BLESS_DOCS=1`) or drift-checks
//! every file; `tests/docs_lint.rs` lints the result.
#![allow(dead_code)]

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every text file that may carry generated blocks, repo-relative and sorted.
pub fn doc_files() -> Vec<PathBuf> {
    let root = repo_root();
    let mut out = vec![root.join("README.md")];
    walk(&root.join("canon/skills"), "md", &mut out);
    walk(&root.join("packages/website/src/content/docs"), "mdx", &mut out);
    walk(&root.join("examples"), "md", &mut out);
    out.sort();
    out
}

pub fn walk(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "node_modules") {
                continue;
            }
            walk(&path, ext, out);
        } else if path.extension().is_some_and(|e| e == ext) {
            out.push(path);
        }
    }
}

/// The marker text of `name` at `edge` (`begin` or `end`) for a file of kind `mdx`.
pub fn marker(name: &str, edge: &str, mdx: bool) -> String {
    if mdx {
        format!("{{/* canon:generated {name} {edge} */}}")
    } else {
        format!("<!-- canon:generated {name} {edge} -->")
    }
}

/// The group a leaf belongs to: its first path word, or `top` for a top-level leaf.
fn group_of(leaf: &Value) -> String {
    let path = leaf["path"].as_array().expect("path");
    if path.len() == 1 { "top".to_string() } else { path[0].as_str().expect("word").to_string() }
}

fn leaves(spec: &Value) -> impl Iterator<Item = &Value> {
    spec["commands"].as_array().expect("commands").iter().filter(|n| n["nodeKind"] == "leaf")
}

/// Every group name that has a table, sorted (`top` first).
pub fn groups(spec: &Value) -> Vec<String> {
    let mut out: Vec<String> = leaves(spec).map(group_of).collect();
    out.sort_by(|a, b| (a != "top", a).cmp(&(b != "top", b)));
    out.dedup();
    out
}

/// The first clause of an `about`: all of it when short, else up to its first `. ` or `: `.
fn short(about: &str) -> &str {
    if about.len() <= 100 {
        return about.trim_end_matches('.');
    }
    let cut = [". ", ": "].iter().filter_map(|m| about.find(m)).min().unwrap_or(about.len());
    about[..cut].trim_end_matches('.')
}

/// `text` safe in MDX prose: outside backtick spans `<`, `>`, `{` and `}` are entities.
fn mdx_safe(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut code = false;
    for c in text.chars() {
        match c {
            '`' => {
                code = !code;
                out.push(c);
            }
            '<' if !code => out.push_str("&lt;"),
            '>' if !code => out.push_str("&gt;"),
            '{' if !code => out.push_str("&#123;"),
            '}' if !code => out.push_str("&#125;"),
            c => out.push(c),
        }
    }
    out
}

fn cell(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

fn kind_word(kind: &str) -> String {
    format!("<{}>", kind.to_ascii_lowercase().replace('_', "-"))
}

fn operand_cell(leaf: &Value) -> String {
    let parts: Vec<String> = leaf["operands"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|o| {
            let word = kind_word(o["kind"].as_str().unwrap_or("value"));
            let mut text = format!("`{word}`");
            if o["required"] != true {
                text.push_str(" (optional)");
            }
            if o["repeatable"] == true {
                text.push_str(" (repeatable)");
            }
            text
        })
        .collect();
    if parts.is_empty() { "none".into() } else { parts.join(", ") }
}

fn option_syntax(o: &Value) -> String {
    let name = o["name"].as_str().expect("option name");
    match (o["kind"].as_str(), o["valueName"].as_str()) {
        (Some("value"), Some(v)) => format!("`--{name} <{v}>`"),
        _ => format!("`--{name}`"),
    }
}

fn required_cell(leaf: &Value) -> String {
    let mut parts: Vec<String> = leaf["options"].as_array().map(|a| a.as_slice()).unwrap_or_default().iter().filter(|o| o["required"] == true).map(option_syntax).collect();
    for group in leaf["requiredOneOf"].as_array().map(|a| a.as_slice()).unwrap_or_default() {
        let names: Vec<String> = group.as_array().expect("group").iter().map(|n| format!("`{}`", n.as_str().expect("name"))).collect();
        parts.push(format!("one of {}", names.join(" or ")));
    }
    if parts.is_empty() { "none".into() } else { parts.join(", ") }
}

fn example(leaf: &Value) -> String {
    leaf["examples"][0].as_str().unwrap_or_else(|| panic!("{} has no example", leaf["command"])).to_string()
}

fn effect_label(leaf: &Value, ko: bool) -> &'static str {
    match (leaf["effect"].as_str(), ko) {
        (Some("read"), false) => "read",
        (Some("write"), false) => "write",
        (Some("destructive"), false) => "destructive (preview, then `--commit --plan-digest`)",
        (Some("read"), true) => "읽기",
        (Some("write"), true) => "쓰기",
        (Some("destructive"), true) => "파괴적 (미리보기 후 `--commit --plan-digest`)",
        (other, _) => panic!("unknown effect {other:?}"),
    }
}

/// One table of the command reference: a group's leaves.
pub fn cli_table(spec: &Value, group: &str, ko: bool) -> String {
    let header = if ko { "| 명령 | 피연산자 | 필수 옵션 | 효과 | 예시 |" } else { "| Command | Operands | Required options | Effect | Example |" };
    let about = spec["commands"].as_array().expect("commands").iter().find(|n| n["nodeKind"] == "group" && n["command"] == group).and_then(|n| n["about"].as_str()).unwrap_or("Commands without a group word");
    let mut out = format!("{}.\n\n{header}\n|---|---|---|---|---|\n", mdx_safe(short(about)));
    for leaf in leaves(spec).filter(|l| group_of(l) == group) {
        let command = leaf["command"].as_str().expect("command");
        let _ = writeln!(out, "| `canon {command}` | {} | {} | {}: {} | `{}` |", operand_cell(leaf), required_cell(leaf), effect_label(leaf, ko), cell(&mdx_safe(short(leaf["about"].as_str().expect("about")))), cell(&example(leaf)));
    }
    out
}

/// The **Commands** block of a role's reference: one runnable example per leaf of the role.
pub fn role_commands(spec: &Value, role: &str) -> String {
    let mut rows: Vec<(String, String)> = leaves(spec)
        .filter(|l| l["roles"].as_array().is_some_and(|r| r.iter().any(|x| x == role)))
        .map(|l| (example(l), short(l["about"].as_str().expect("about")).to_string()))
        .collect();
    rows.sort();
    let width = rows.iter().map(|(e, _)| e.len()).max().unwrap_or(0);
    let mut out = String::from("```bash\n");
    for (example, about) in rows {
        let _ = writeln!(out, "{example:width$}  # {about}");
    }
    out.push_str("```\n");
    out
}

/// The removed-spellings table of the README's Upgrading list.
pub fn removed_table(spec: &Value) -> String {
    let mut out = String::from("| 0.27 spelling | 0.28 spelling | What changed |\n|---|---|---|\n");
    for row in spec["removed"].as_array().expect("removed") {
        if row["since"] != "0.28.0" {
            continue;
        }
        let old = row["old"].as_str().expect("old");
        let new = row["new"].as_str().expect("new");
        let scope: Vec<&str> = row["scope"].as_array().map(|a| a.as_slice()).unwrap_or_default().iter().filter_map(Value::as_str).collect();
        let (old, new) = match row["kind"].as_str() {
            Some("option") => {
                let on = scope.iter().map(|s| format!("`canon {s}`")).collect::<Vec<_>>().join(", ");
                let new_shown = if new.starts_with('-') { format!("`{new}`") } else { format!("`canon {new}`") };
                (format!("`{old}` on {on}"), new_shown)
            }
            Some("bare") => (format!("`canon {old}` (bare)"), format!("`canon {new}`")),
            _ => (format!("`canon {old}`"), format!("`canon {new}`")),
        };
        let note = row["note"].as_str().unwrap_or_default();
        let _ = writeln!(out, "| {} | {} | {} |", cell(&old), cell(&new), cell(note));
    }
    out
}

/// The text of block `name`, or `None` when the name is not one the spec defines.
pub fn render(spec: &Value, name: &str, ko: bool) -> Option<String> {
    if let Some(role) = name.strip_prefix("commands-") {
        return Some(role_commands(spec, role));
    }
    if let Some(group) = name.strip_prefix("cli-") {
        return groups(spec).iter().any(|g| g == group).then(|| cli_table(spec, group, ko));
    }
    (name == "removed").then(|| removed_table(spec))
}

/// The block name a line opens, if it is a `begin` marker.
fn begin_name(line: &str, mdx: bool) -> Option<&str> {
    let (open, close) = if mdx { ("{/* canon:generated ", " begin */}") } else { ("<!-- canon:generated ", " begin -->") };
    line.trim().strip_prefix(open)?.strip_suffix(close)
}

/// `text` with every generated block re-rendered; `Err` names a marker problem.
pub fn splice(text: &str, mdx: bool, ko: bool, spec: &Value) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut lines = text.split_inclusive('\n');
    while let Some(line) = lines.next() {
        out.push_str(line);
        let Some(name) = begin_name(line, mdx) else { continue };
        let end = marker(name, "end", mdx);
        let body = render(spec, name, ko).ok_or_else(|| format!("unknown generated block `{name}`"))?;
        let mut closed = false;
        for old in lines.by_ref() {
            if old.trim() == end {
                out.push_str(&body);
                out.push_str(old);
                closed = true;
                break;
            }
        }
        if !closed {
            return Err(format!("block `{name}` has no `{end}` line"));
        }
    }
    Ok(out)
}

/// The generated blocks a file must carry, by repo-relative path.
pub fn required_blocks(spec: &Value, rel: &str) -> Vec<String> {
    if let Some(role) = rel.strip_prefix("canon/skills/reference/role-").and_then(|r| r.strip_suffix(".md")) {
        return vec![format!("commands-{role}")];
    }
    if rel.ends_with("docs/cli.mdx") {
        return groups(spec).iter().map(|g| format!("cli-{g}")).collect();
    }
    if rel == "README.md" {
        return vec!["removed".into()];
    }
    Vec::new()
}

/// The file's contents with its blocks rendered; `Err` when a required block is missing.
pub fn regenerate(spec: &Value, path: &Path) -> Result<String, String> {
    let rel = rel_of(path);
    let text = fs::read_to_string(path).map_err(|e| format!("{rel}: {e}"))?;
    let mdx = path.extension().is_some_and(|e| e == "mdx");
    for block in required_blocks(spec, &rel) {
        if !text.lines().any(|l| begin_name(l, mdx) == Some(block.as_str())) {
            return Err(format!("{rel}: missing the `{}` line", marker(&block, "begin", mdx)));
        }
    }
    splice(&text, mdx, rel.contains("/ko/"), spec).map_err(|e| format!("{rel}: {e}"))
}

/// The path relative to the repository root, with `/` separators.
pub fn rel_of(path: &Path) -> String {
    let root = fs::canonicalize(repo_root()).expect("repo root");
    let full = fs::canonicalize(path).expect("doc file");
    full.strip_prefix(&root).expect("inside the repo").to_string_lossy().replace('\\', "/")
}
