//! Field selection (spec 6.10): the grammar of a field path, the list of paths in `--fields`, and the projection of
//! a record by the rules 1-4 of 6.10.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

/// One step of a field path: a member name, whether `[]` follows, and whether it is written in brackets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub name: String,
    pub array: bool,
    pub bracket: bool,
}

fn is_ident(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A path as the grammar of 6.10 reads it; `None` when it is malformed. A bracketed key that is an identifier is
/// malformed too: a member has exactly one spelling.
pub fn parse_path(path: &str) -> Option<Vec<Step>> {
    let chars: Vec<char> = path.chars().collect();
    let mut steps: Vec<Step> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let mut name = String::new();
        let mut bracket = false;
        if chars[i] == '[' && chars.get(i + 1) == Some(&'"') {
            let mut j = i + 2;
            while j < chars.len() && chars[j] != '"' {
                if chars[j] == '\0' {
                    return None;
                }
                if chars[j] == '\\' {
                    if !matches!(chars.get(j + 1), Some('"' | '\\')) {
                        return None;
                    }
                    j += 1;
                }
                name.push(chars[j]);
                j += 1;
            }
            if !(chars.get(j) == Some(&'"') && chars.get(j + 1) == Some(&']')) {
                return None;
            }
            if is_ident(&name) {
                return None;
            }
            bracket = true;
            i = j + 2;
        } else {
            if !steps.is_empty() {
                if chars[i] != '.' {
                    return None;
                }
                i += 1;
            }
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                name.push(chars[i]);
                i += 1;
            }
            if !is_ident(&name) {
                return None;
            }
        }
        let array = chars.get(i) == Some(&'[') && chars.get(i + 1) == Some(&']');
        if array {
            i += 2;
        }
        steps.push(Step { name, array, bracket });
    }
    (!steps.is_empty()).then_some(steps)
}

/// The one spelling of a parsed path.
pub fn render(steps: &[Step]) -> String {
    let mut out = String::new();
    for (index, step) in steps.iter().enumerate() {
        if step.bracket {
            out.push_str("[\"");
            out.push_str(&step.name.replace('\\', "\\\\").replace('"', "\\\""));
            out.push_str("\"]");
        } else {
            if index > 0 {
                out.push('.');
            }
            out.push_str(&step.name);
        }
        if step.array {
            out.push_str("[]");
        }
    }
    out
}

/// The comma-separated paths of a `--fields` value; a comma inside a bracketed key does not separate.
pub fn split_list(value: &str) -> Vec<String> {
    let chars: Vec<char> = value.chars().collect();
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if quoted {
            current.push(c);
            if c == '\\' && i + 1 < chars.len() {
                i += 1;
                current.push(chars[i]);
            } else if c == '"' && chars.get(i + 1) == Some(&']') {
                quoted = false;
            }
        } else if c == '[' && chars.get(i + 1) == Some(&'"') {
            quoted = true;
            current.push(c);
            i += 1;
            current.push(chars[i]);
        } else if c == ',' {
            parts.push(std::mem::take(&mut current));
        } else {
            current.push(c);
        }
        i += 1;
    }
    parts.push(current);
    parts
}

/// The kept paths as a trie: a member maps to `Whole` (covered, kept entirely) or to the trie of its kept descendants.
#[derive(Debug, Clone)]
pub enum Node {
    Whole,
    Part(BTreeMap<String, Node>),
}

/// Builds the trie of rule 2: a covered member is kept whole even when another kept path names a descendant.
pub fn trie(paths: &[Vec<Step>]) -> BTreeMap<String, Node> {
    let mut root: BTreeMap<String, Node> = BTreeMap::new();
    for steps in paths {
        let mut node = &mut root;
        for (index, step) in steps.iter().enumerate() {
            if matches!(node.get(&step.name), Some(Node::Whole)) {
                break;
            }
            if index + 1 == steps.len() {
                node.insert(step.name.clone(), Node::Whole);
                break;
            }
            let entry = node
                .entry(step.name.clone())
                .or_insert_with(|| Node::Part(BTreeMap::new()));
            let Node::Part(child) = entry else { break };
            node = child;
        }
    }
    root
}

/// Projects one value by the trie: an object keeps the members the trie covers or passes through; an array keeps
/// every element, each projected the same way; anything else (a `null` ancestor included) stays as it is.
pub fn project(value: &Value, node: &BTreeMap<String, Node>) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(|item| project(item, node)).collect()),
        Value::Object(members) => {
            let mut out = Map::new();
            for (key, member) in members {
                match node.get(key) {
                    Some(Node::Whole) => {
                        out.insert(key.clone(), member.clone());
                    }
                    Some(Node::Part(child)) => {
                        out.insert(key.clone(), project(member, child));
                    }
                    None => {}
                }
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}
