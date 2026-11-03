//! The command lines of a guide (spec 14.5, approaches/checker.md 7.1).
//!
//! A guide is Markdown. Every line of its fenced code blocks that starts with the tool name is a documented invocation, read by
//! the grammar of 9.12 like an example of a command document; a trailing `\` joins the next line. `<UPPER_CASE>`
//! placeholders stand for a value valid for the argument they are written for (an enum's first value, `1` for an
//! integer, `x` for a string), and are replaced by one before the line is read.

use serde_json::Value;

use crate::l1::invocation::{self, Item};
use crate::l1::model::{camel, Arg, CommandDoc, ToolDocs};
use crate::l1::witness;

/// One documented line of a guide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuideLine {
    /// The 1-based number of the first line of the text in the guide's `text`.
    pub line: usize,
    pub text: String,
}

/// The lines of the fenced code blocks of `text` that start with `tool` and a space, a `\` at the end of a line joining the next.
pub fn lines(text: &str, tool: &str) -> Vec<GuideLine> {
    let mut found = Vec::new();
    let mut fence: Option<String> = None;
    let mut pending: Option<GuideLine> = None;
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let trimmed = line.trim_start();
        let marker = ["```", "~~~"].into_iter().find(|marker| trimmed.starts_with(marker));
        if let Some(open) = &fence {
            if trimmed.starts_with(open.as_str()) {
                fence = None;
                found.extend(pending.take());
                continue;
            }
            match pending.take() {
                Some(mut joined) => {
                    joined.text.push('\n');
                    joined.text.push_str(line);
                    if line.ends_with('\\') {
                        pending = Some(joined);
                    } else {
                        found.push(joined);
                    }
                }
                None => {
                    let starts = line.trim_start().strip_prefix(tool).is_some_and(|rest| rest.starts_with([' ', '\t']));
                    if starts {
                        let joined = GuideLine { line: number, text: line.trim_start().to_string() };
                        if line.ends_with('\\') {
                            pending = Some(joined);
                        } else {
                            found.push(joined);
                        }
                    }
                }
            }
        } else if let Some(marker) = marker {
            fence = Some(marker.to_string());
        }
    }
    found.extend(pending);
    found
}

/// The command a line of words (after the tool name) names: the longest path of `docs` that the words contain after any leading options.
pub fn command_of<'a>(docs: &'a ToolDocs, words: &[String]) -> Option<&'a CommandDoc> {
    (0..words.len().min(8))
        .filter_map(|start| {
            docs.commands
                .values()
                .filter(|doc| !doc.path.is_empty() && words[start..].starts_with(&doc.path))
                .max_by_key(|doc| doc.path.len())
        })
        .next()
}

fn is_placeholder(text: &str) -> bool {
    text.contains('<') && text.contains('>') && placeholders(text).next().is_some()
}

/// The `<UPPER_CASE>` placeholders of `text`, with their spans.
fn placeholders(text: &str) -> impl Iterator<Item = (usize, usize)> + '_ {
    let bytes = text.as_bytes();
    let mut at = 0;
    std::iter::from_fn(move || {
        while at < bytes.len() {
            if bytes[at] == b'<' && bytes.get(at + 1).is_some_and(u8::is_ascii_uppercase) {
                let mut end = at + 1;
                while bytes.get(end).is_some_and(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || *byte == b'_') {
                    end += 1;
                }
                if bytes.get(end) == Some(&b'>') {
                    let span = (at, end + 1);
                    at = end + 1;
                    return Some(span);
                }
            }
            at += 1;
        }
        None
    })
}

/// A value valid for `arg`, as an argv word: what the contract names (an enum's first value, `1` for an integer, `x` for a string), a generated valid one when a domain refuses that.
fn value_for(arg: Option<&Arg>) -> String {
    let Some(arg) = arg else { return "x".to_string() };
    let plain = match arg.value_type.as_str() {
        "enum" => arg.possible_values.first().cloned().unwrap_or_else(|| "x".to_string()),
        "integer" | "count" => "1".to_string(),
        "number" => "1".to_string(),
        "json" => "{}".to_string(),
        _ => "x".to_string(),
    };
    let typed = witness::typed(arg, &plain);
    if invocation::value_problem(arg, &typed).is_none() {
        plain
    } else {
        witness::generate(arg, &[]).unwrap_or(plain)
    }
}

fn replace_placeholders(text: &str, word: &str) -> String {
    let mut out = String::new();
    let mut last = 0;
    for (start, end) in placeholders(text) {
        out.push_str(&text[last..start]);
        out.push_str(word);
        last = end;
    }
    out.push_str(&text[last..]);
    out
}

/// A JSON value of a document with each placeholder in a string replaced by a value valid for the argument the key names.
fn fill_document(doc: &CommandDoc, value: &mut Value) {
    let Some(object) = value.as_object_mut() else { return };
    for (key, member) in object.iter_mut() {
        if key == "operands" {
            if let Some(items) = member.as_array_mut() {
                for (index, item) in items.iter_mut().enumerate() {
                    let arg = doc.operands.get(index).or_else(|| doc.operands.last().filter(|operand| operand.variadic));
                    fill_value(arg, item);
                }
            }
            continue;
        }
        let arg = doc.options.iter().find(|option| camel(&option.name) == *key);
        match member {
            Value::Array(items) => {
                for item in items {
                    fill_value(arg, item);
                }
            }
            other => fill_value(arg, other),
        }
    }
}

fn fill_value(arg: Option<&Arg>, value: &mut Value) {
    if let Value::String(text) = value {
        if is_placeholder(text) {
            let word = value_for(arg);
            let replaced = replace_placeholders(text, &word);
            let exact = placeholders(text).count() == 1 && placeholders(text).next().is_some_and(|(start, end)| start == 0 && end == text.len());
            let numeric = arg.is_some_and(|arg| matches!(arg.value_type.as_str(), "integer" | "number" | "count"));
            *value = match (exact && numeric, replaced.parse::<serde_json::Number>()) {
                (true, Ok(number)) => Value::Number(number),
                _ => Value::String(replaced),
            };
        }
    }
}

/// The words of `words` (after the tool name) with every placeholder replaced by a value valid for the argument it is written for.
pub fn substitute(docs: &ToolDocs, words: &[String]) -> Vec<String> {
    let Some(doc) = command_of(docs, words) else {
        return words.iter().map(|word| replace_placeholders(word, "x")).collect();
    };
    let Ok(parsed) = invocation::parse(doc, words) else {
        return words.iter().map(|word| replace_placeholders(word, "x")).collect();
    };
    let mut operand = 0;
    let mut items = parsed.items.clone();
    for item in &mut items {
        match item {
            Item::Operand(word) => {
                let arg = doc.operands.get(operand).or_else(|| doc.operands.last().filter(|arg| arg.variadic));
                operand += 1;
                if is_placeholder(word) {
                    *word = replace_placeholders(word, &value_for(arg));
                }
            }
            Item::Option { name, value: Some(word) } if is_placeholder(word) => {
                if name == "input" {
                    if let Ok(mut parsed) = serde_json::from_str::<Value>(&replace_placeholders_in_json(word)) {
                        match &mut parsed {
                            Value::Array(documents) => {
                                for item in documents {
                                    fill_document(doc, item);
                                }
                            }
                            other => fill_document(doc, other),
                        }
                        *word = serde_json::to_string(&parsed).unwrap_or_else(|_| word.clone());
                    } else {
                        *word = replace_placeholders(word, "x");
                    }
                } else {
                    *word = replace_placeholders(word, &value_for(doc.option(name)));
                }
            }
            Item::Option { .. } => {}
        }
    }
    let mut rebuilt = parsed;
    rebuilt.before = rebuilt.before.iter().map(|word| replace_placeholders(word, "x")).collect();
    rebuilt.items = items;
    invocation::render(&rebuilt)
}

/// A document text whose placeholders stand where a JSON value is: a bare `<X>` becomes the string `"<X>"`, which `fill_value` replaces.
fn replace_placeholders_in_json(text: &str) -> String {
    let mut out = String::new();
    let mut last = 0;
    let mut in_string = false;
    let mut escaped = false;
    let mut spans = placeholders(text).peekable();
    for (index, character) in text.char_indices() {
        if let Some((start, end)) = spans.peek().copied() {
            if index == start && !in_string {
                out.push_str(&text[last..start]);
                out.push('"');
                out.push_str(&text[start..end]);
                out.push('"');
                last = end;
                spans.next();
                continue;
            }
            if index >= end {
                spans.next();
            }
        }
        if escaped {
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            in_string = !in_string;
        }
    }
    out.push_str(&text[last..]);
    out
}

/// A line quoted so that `invocation::split_line` reads it back as `words` (one word each, single-quoted).
pub fn quoted(words: &[String]) -> String {
    words
        .iter()
        .map(|word| format!("'{}'", word.replace('\'', "'\\''")))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fenced_line_that_starts_with_the_tool_name_is_a_documented_invocation() {
        let text = "Intro\n\n```bash\n# a comment\nacme build --json\nsome other\nacme deploy start \\\n  staging --json\n```\nacme not fenced\n";
        let found = lines(text, "acme");
        assert_eq!(found.len(), 2);
        assert_eq!((found[0].line, found[0].text.as_str()), (5, "acme build --json"));
        assert_eq!(found[1].line, 7);
        assert!(found[1].text.contains("staging"));
    }

    #[test]
    fn placeholders_are_found_with_their_spans() {
        assert_eq!(placeholders("--tag=<TAG> <X1>").collect::<Vec<_>>(), vec![(6, 11), (12, 16)]);
        assert!(is_placeholder("<ID>"));
        assert!(!is_placeholder("a < b > c"));
    }

    #[test]
    fn a_placeholder_in_a_document_becomes_a_json_string_to_fill() {
        assert_eq!(replace_placeholders_in_json("{\"a\":<X>}"), "{\"a\":\"<X>\"}");
        assert_eq!(replace_placeholders_in_json("{\"a\":\"<X>\"}"), "{\"a\":\"<X>\"}");
    }
}
