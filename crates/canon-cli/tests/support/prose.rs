//! What the machine-field comparers compare (0.43 I14): the contract of canon's output without the sentences a person reads.
//!
//! `conformance.rs` and `cli_spec.rs` hold generated output to a checked-in copy (the conformance expectations,
//! `docs/cli/canon.cli.json`). The copy stays the full output; the comparers reduce both sides with this module and compare the
//! reductions, so a reworded `message`, `hint`, help or summary passes and a changed code, field, exit, name, kind, count, path or id
//! fails.
//!
//! - JSON: a document is parsed and the strings at the prose paths below are emptied. A member that is removed is still a difference;
//!   `properties` keys named `message` or `title`, `data.items[*].data.title`, `data.records[*].title`, `data.input.example.*` and an
//!   error `reason` are not on the lists and stay compared.
//! - Text: [`output`] drops what a sentence says and keeps the line's machine tokens (see [`output`]).

use serde_json::Value;

/// Prose paths of the envelope (`/` separated, `*` any array index or key, `**` any run of members).
const ENVELOPE: &[&str] = &[
    "error/message",
    "error/hint",
    "error/details/*/message",
    "error/details/*/hint",
    "data/items/*/error/message",
    "data/items/*/error/hint",
    "meta/next/*/reason",
    "meta/warnings/*/message",
    "meta/warnings/*/hint",
    "data/findings/*/hint",
    "data/effectNote",
    "data/warning/meaning",
    "data/warning/fix",
    // Prose of read answers the brief's list does not name: the sentences of `status`, `context` and `scenario new`.
    "data/warnings/*",
    "data/note",
    "data/languageNote",
    "data/policy/diagnostics/*",
    "data/policy/evidence_binding/invalid",
    "data/commands/*/about",
    "data/joinKeys/*",
];

/// Prose paths of a schema document, relative to its root: `data` of a `canon schema` answer, or the whole of `docs/cli/canon.cli.json`.
const SCHEMA_DOCUMENT: &[&str] = &[
    "_doNotEdit",
    "cli/about",
    "envVars/*/about",
    "summary",
    "commands/*/summary",
    "tree/commands/*/summary",
    "effectNote",
    "commands/*/effectNote",
    "**/options/*/help",
    "**/operands/*/help",
    "**/constraints/*/help",
    "**/options/*/defaultNote",
    "**/operands/*/defaultNote",
    "**/validators/*/description",
    "removed/*/note",
    "error/meaning",
    "error/fix",
    "error/reasons/*/meaning",
    "error/reasons/*/fix",
    "errors/*/meaning",
    "errors/*/fix",
    "errors/*/reasons/*/meaning",
    "errors/*/reasons/*/fix",
    "warning/meaning",
    "warning/fix",
    "warnings/*/meaning",
    "warnings/*/fix",
    "exitTable/*/meaning",
    "**/outputs/*/when",
    "**/x-canon/rolesForm",
    "schemas/**/description",
    "schemas/**/title",
];

/// Whether `pattern` names the position `path`.
fn names(pattern: &[&str], path: &[String]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((&"**", rest)) => (0..=path.len()).any(|skip| names(rest, &path[skip..])),
        Some((segment, rest)) => path.split_first().is_some_and(|(first, tail)| (*segment == "*" || first == segment) && names(rest, tail)),
    }
}

fn blank(value: &mut Value, path: &mut Vec<String>, patterns: &[Vec<&str>]) {
    match value {
        Value::String(text) => {
            if patterns.iter().any(|pattern| names(pattern, path)) {
                text.clear();
            }
        }
        Value::Object(members) => {
            for (key, child) in members.iter_mut() {
                path.push(key.clone());
                blank(child, path, patterns);
                path.pop();
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter_mut().enumerate() {
                path.push(index.to_string());
                blank(child, path, patterns);
                path.pop();
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn blank_paths(value: &mut Value, paths: &[&str]) {
    let patterns: Vec<Vec<&str>> = paths.iter().map(|path| path.split('/').collect()).collect();
    blank(value, &mut Vec::new(), &patterns);
}

/// `docs/cli/canon.cli.json` with its prose emptied.
pub fn spec_document(value: &mut Value) {
    blank_paths(value, SCHEMA_DOCUMENT);
}

/// One envelope with its prose emptied; the `data` of `canon schema` is a schema document.
fn envelope(value: &mut Value) {
    blank_paths(value, ENVELOPE);
    if value.pointer("/meta/command").and_then(Value::as_str) == Some("schema") {
        if let Some(data) = value.get_mut("data") {
            spec_document(data);
        }
    }
}

/// The comparable form of a command's stdout or stderr: a JSON line is parsed and its prose emptied, any other line keeps what a program
/// reads and loses what a person reads.
///
/// Text lines lose: the sentence after `canon: CODE:` (the code stays), a `hint:` line's text, the sentence after `advisory:` or
/// `warning:`, the text after ` — `, the `# …` reasons of a `next:` block. `field:`, `expected:`, `problem:` and `Usage:` lines, the
/// command lines of `next:` and of examples, and lines of at most two words in a row are kept as they are. A line that still holds a
/// sentence (three ordinary words in a row) is reduced to its machine tokens: numbers, paths, ids, `key=value`, backtick pieces,
/// `UPPER_CASE` codes, hyphenated kinds and the like. A run of such lines is one line, so re-wrapping a paragraph is not a change.
pub fn output(text: &str) -> String {
    let mut out = Lines::default();
    let mut in_next = false;
    for line in text.lines() {
        if line.starts_with('{') {
            if let Ok(mut value) = serde_json::from_str::<Value>(line) {
                envelope(&mut value);
                out.keep(&value.to_string());
                continue;
            }
        }
        text_line(line, &mut in_next, &mut out);
    }
    out.finish()
}

#[derive(Default)]
struct Lines {
    lines: Vec<String>,
    run: Vec<String>,
}

impl Lines {
    fn flush(&mut self) {
        if !self.run.is_empty() {
            self.lines.push(self.run.join(" "));
            self.run.clear();
        }
    }

    /// A line kept as it is; blank lines collapse and never open the output.
    fn keep(&mut self, line: &str) {
        self.flush();
        if line.is_empty() && self.lines.last().is_none_or(String::is_empty) {
            return;
        }
        self.lines.push(line.to_string());
    }

    fn finish(mut self) -> String {
        self.flush();
        while self.lines.last().is_some_and(String::is_empty) {
            self.lines.pop();
        }
        self.lines.iter().map(|line| format!("{line}\n")).collect()
    }
}

/// Words that open a line and are kept whole, whatever follows.
const WHOLE_LINES: [&str; 4] = ["field:", "expected:", "problem:", "Usage:"];

/// Abbreviations that read as words.
const ABBREVIATIONS: [&str; 4] = ["e.g", "i.e", "etc", "vs"];

fn text_line(line: &str, in_next: &mut bool, out: &mut Lines) {
    let body = line.trim_start();
    let indent = &line[..line.len() - body.len()];
    let body = body.trim_end();
    if body.is_empty() {
        *in_next = false;
        out.keep("");
        return;
    }
    if indent.is_empty() {
        *in_next = body == "next:";
    }
    if *in_next && body.starts_with('#') {
        out.keep(&format!("{indent}#"));
        return;
    }
    if body.starts_with("hint:") {
        out.keep(&format!("{indent}hint:"));
        return;
    }
    if WHOLE_LINES.iter().any(|label| body.starts_with(label)) {
        out.keep(&format!("{indent}{body}"));
        return;
    }
    let mut body = cut_at_marker(body);
    if let Some((machine, _)) = body.split_once(" — ") {
        body = machine.trim_end();
    }
    if *in_next || body.starts_with("next:") {
        if let Some((command, _)) = body.split_once(" # ") {
            body = command.trim_end();
        }
    }
    let prefix = command_prefix(body);
    if let Some(end) = prefix {
        let rest = body[end..].trim_start();
        if let Some((code, _)) = rest.split_once(':') {
            if code.starts_with(|c: char| c.is_ascii_uppercase()) && code.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
                out.keep(&format!("{indent}{} {code}:", &body[..end]));
                return;
            }
        }
    }
    let after_next = body.strip_prefix("next:").map_or(body, str::trim_start);
    if prefix.is_none() && (after_next == "canon" || after_next.starts_with("canon ")) {
        out.keep(&format!("{indent}{body}"));
        return;
    }
    let (head, rest) = prefix.map_or(("", body), |end| (&body[..end], &body[end..]));
    let mut pieces = chunks(rest);
    let label = pieces.first().copied().filter(|piece| piece.strip_suffix(':').is_some_and(|name| !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')));
    if label.is_some() {
        pieces.remove(0);
    }
    let kinds: Vec<Kind> = pieces.iter().copied().map(classify).collect();
    let (mut run, mut longest) = (0, 0);
    for kind in &kinds {
        match kind {
            Kind::Word => {
                run += 1;
                longest = longest.max(run);
            }
            Kind::Noise => {}
            Kind::Token(_) => run = 0,
        }
    }
    if longest < 3 {
        out.keep(&format!("{indent}{body}"));
        return;
    }
    out.run.extend(head.split_whitespace().map(str::to_string));
    out.run.extend(label.map(str::to_string));
    out.run.extend(kinds.into_iter().filter_map(|kind| match kind {
        Kind::Token(token) => Some(token.to_string()),
        Kind::Word | Kind::Noise => None,
    }));
}

/// `body` up to the end of the first `advisory:` or `warning:` that starts a word: what follows is a sentence.
fn cut_at_marker(body: &str) -> &str {
    ["advisory:", "warning:"]
        .iter()
        .filter_map(|marker| body.match_indices(marker).find(|(at, _)| *at == 0 || body[..*at].ends_with(' ')).map(|(at, _)| at + marker.len()))
        .min()
        .map_or(body, |end| &body[..end])
}

/// The length of a leading `canon [word ...]:` (the command a line speaks for), colon included.
fn command_prefix(body: &str) -> Option<usize> {
    let colon = body.find(':')?;
    let mut words = body[..colon].split(' ');
    let is_command = words.next() == Some("canon") && words.all(|word| word.starts_with(|c: char| c.is_ascii_lowercase()) && word.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'));
    is_command.then_some(colon + 1)
}

/// The pieces of a line between white space, a backtick span being one piece.
fn chunks(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut start, mut in_span) = (None, false);
    for (at, c) in line.char_indices() {
        if c == '`' {
            in_span = !in_span;
        }
        if c.is_whitespace() && !in_span {
            if let Some(from) = start.take() {
                out.push(&line[from..at]);
            }
        } else if start.is_none() {
            start = Some(at);
        }
    }
    if let Some(from) = start {
        out.push(&line[from..]);
    }
    out
}

enum Kind<'a> {
    /// An ordinary word of a sentence.
    Word,
    /// Nothing a person or a program reads: punctuation, arrows.
    Noise,
    /// A machine token: number, path, id, `key=value`, backtick piece, option, code.
    Token(&'a str),
}

/// Punctuation that ends a word of a sentence.
const CLOSING: [char; 11] = [',', '.', ';', ':', '!', '?', '"', '\'', '”', '’', '…'];

fn classify(chunk: &str) -> Kind<'_> {
    let mut token = chunk.trim_start_matches(['(', '"', '\'', '“', '‘']).trim_end_matches(CLOSING);
    // `file(s)` keeps its plural mark; any other closing bracket belongs to a parenthesis that the sentence opened.
    while !token.ends_with("(s)") && !token.ends_with("(ies)") {
        let Some(inner) = token.strip_suffix(')') else { break };
        token = inner.trim_end_matches(CLOSING);
    }
    if token.contains('`') {
        return Kind::Token(token);
    }
    if !token.chars().any(char::is_alphanumeric) {
        return Kind::Noise;
    }
    if is_word(token) { Kind::Word } else { Kind::Token(token) }
}

fn is_word(token: &str) -> bool {
    let token = ["(s)", "(ies)"].iter().find_map(|plural| token.strip_suffix(plural)).unwrap_or(token);
    if ABBREVIATIONS.contains(&token) {
        return true;
    }
    let camel_case = token.chars().skip(1).any(char::is_uppercase) && token.chars().any(char::is_lowercase);
    !token.is_empty() && token.chars().all(|c| c.is_alphabetic() || c == '\'' || c == '’') && !camel_case
}
