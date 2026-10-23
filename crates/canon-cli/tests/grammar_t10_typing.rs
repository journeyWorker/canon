//! T10 (0.28 D2, D3): output is typed.
//!
//! - no source prints JSON it built by hand: a `println!`, `print!`, `writeln!`
//!   or `write_all` whose arguments hold `json!` or a `serde_json::to_string*`
//!   call is a payload that bypassed `json_out` (and its schema), except the
//!   statements named in [`PRINTS`];
//! - no untyped `serde_json::Value` is left in a report schema: a schema that is
//!   `true` or `{}`, a property with no type, or an object with no properties is
//!   named in [`OPAQUE`] with the reason it keeps its own schema.

mod harness;

use std::path::Path;

use harness::*;
use serde_json::Value;

/// Statements that print serialized JSON outside `json_out`, as (file, needle in the statement, why).
const PRINTS: &[(&str, &str, &str)] = &[
    ("learn.rs", "rollback record serializes", "human mode of `strategy rollback` prints the verbatim ledger record; `--json` goes through `json_out`"),
    ("retention.rs", "options.open(path)", "appends the audit line to `.canon/audit/access.jsonl`: a file, not output"),
];

/// Schemas that are deliberately opaque, as (definition name, why).
const OPAQUE: &[(&str, &str)] = &[
    ("InputDoc", "its `example` and `batchExample` are example `--input` documents: their keys are the leaf's option names, so their shape is that leaf's `<Name>Input` schema, published beside it"),
    ("LedgerRecord", "a ledger record embedded verbatim keeps its own snake_case schema (crates/canon-model/schemas)"),
];

fn sources(dir: &Path, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).expect("read dir") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let text = std::fs::read_to_string(&path).expect("read source");
            let live = text.split("#[cfg(test)]").next().expect("a piece").to_string();
            out.push((path.display().to_string(), live));
        }
    }
}

/// The text of every call of a printing macro or `write_all`, up to its closing parenthesis.
fn print_statements(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for needle in ["println!(", "print!(", "eprintln!(", "eprint!(", "writeln!(", "write!(", ".write_all("] {
        for (at, _) in text.match_indices(needle) {
            let start = at + needle.len();
            let (mut depth, mut end, mut in_str, mut escaped) = (1usize, text.len(), false, false);
            for (i, c) in text[start..].char_indices() {
                match (c, in_str) {
                    ('\\', true) => escaped = !escaped,
                    ('"', _) if !escaped => in_str = !in_str,
                    ('(', false) => depth += 1,
                    (')', false) => {
                        depth -= 1;
                        if depth == 0 {
                            end = start + i;
                            break;
                        }
                    }
                    _ => {}
                }
                if c != '\\' {
                    escaped = false;
                }
            }
            found.push(text[at..end].to_string());
        }
    }
    found
}

fn prints_json(statement: &str) -> bool {
    ["json!(", "serde_json::to_string", "serde_json::to_vec", "serde_json::to_writer", "to_string_pretty("].iter().any(|p| statement.contains(p))
}

#[test]
fn no_source_prints_json_it_built_by_hand() {
    let mut files = Vec::new();
    for krate in std::fs::read_dir(repo_root().join("crates")).unwrap() {
        sources(&krate.unwrap().path().join("src"), &mut files);
    }
    let (mut statements, mut problems, mut allowed) = (0, Vec::new(), vec![false; PRINTS.len()]);
    for (file, text) in &files {
        if file.ends_with("json_out.rs") {
            continue;
        }
        for statement in print_statements(text) {
            statements += 1;
            if !prints_json(&statement) {
                continue;
            }
            match PRINTS.iter().position(|(f, needle, _)| file.ends_with(f) && statement.contains(needle)) {
                Some(i) => allowed[i] = true,
                None => problems.push(format!("{file}: prints hand-built JSON (use a typed Report through json_out): {}", statement.lines().next().unwrap_or_default())),
            }
        }
    }
    assert!(statements > 300, "the scan found the print statements ({statements})");
    for (i, (file, needle, _)) in PRINTS.iter().enumerate() {
        assert!(allowed[i], "PRINTS names `{needle}` in {file}, which prints no JSON now: drop the entry");
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn the_scan_recognises_a_print_of_hand_built_json() {
    let leak = "fn leak() { println!(\"{}\", serde_json::json!({ \"a\": (1) })); }";
    assert!(print_statements(leak).iter().any(|s| prints_json(s)));
    let fine = "fn fine(json: bool) { println!(\"{} (x)\", name); let v = serde_json::json!({}); }";
    assert!(!print_statements(fine).iter().any(|s| prints_json(s)));
}

fn is_object(schema: &Value) -> bool {
    match &schema["type"] {
        Value::String(t) => t == "object",
        Value::Array(types) => types.iter().any(|t| t == "object"),
        _ => false,
    }
}

fn has_shape(schema: &Value) -> bool {
    ["type", "$ref", "enum", "const", "oneOf", "anyOf", "allOf"].iter().any(|k| schema.get(k).is_some())
}

/// Walks one schema document; `defs` names the definition being walked.
fn untyped(schema: &Value, at: &str, def: Option<&str>, out: &mut Vec<String>) {
    match schema {
        Value::Bool(true) => out.push(format!("{at}: the schema `true` accepts anything")),
        Value::Object(map) if map.is_empty() => out.push(format!("{at}: the empty schema accepts anything")),
        Value::Object(map) => {
            let opaque_ok = def.is_some_and(|d| OPAQUE.iter().any(|(name, _)| *name == d));
            if is_object(schema) && !map.contains_key("properties") && !map.get("additionalProperties").is_some_and(Value::is_object) && !map.contains_key("oneOf") && !map.contains_key("anyOf") && !opaque_ok {
                out.push(format!("{at}: an object with no declared members"));
            }
            // A schema that only constrains (`{"required": [..]}` in a `oneOf`) names no type of its own.
            if !has_shape(schema) && !map.contains_key("properties") && !map.contains_key("required") && !opaque_ok {
                out.push(format!("{at}: no type"));
            }
            for (key, child) in map {
                match key.as_str() {
                    "properties" | "$defs" => {
                        for (name, member) in child.as_object().into_iter().flatten() {
                            let def = if key == "$defs" { Some(name.as_str()) } else { def };
                            untyped(member, &format!("{at}/{name}"), def, out);
                        }
                    }
                    "items" | "additionalProperties" if !child.is_boolean() || *child == Value::Bool(true) => untyped(child, &format!("{at}/{key}"), def, out),
                    "anyOf" | "oneOf" | "allOf" | "prefixItems" => {
                        for (i, member) in child.as_array().into_iter().flatten().enumerate() {
                            untyped(member, &format!("{at}/{key}[{i}]"), def, out);
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

#[test]
fn no_report_schema_holds_an_untyped_value() {
    let mut problems = Vec::new();
    for (name, schema) in spec()["schemas"].as_object().expect("schemas") {
        untyped(schema, name, None, &mut problems);
    }
    for (def, reason) in OPAQUE {
        assert!(!reason.is_empty());
        let used = spec()["schemas"].as_object().unwrap().values().any(|s| s["$defs"].get(*def).is_some()) || spec()["schemas"].get(*def).is_some();
        assert!(used, "OPAQUE names {def}, which no schema defines: drop the entry");
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn the_walk_recognises_an_untyped_member() {
    let bad = serde_json::json!({"type": "object", "properties": {"any": true, "obj": {"type": "object"}, "loose": {"description": "no type"}, "ok": {"type": "string"}}});
    let mut problems = Vec::new();
    untyped(&bad, "S", None, &mut problems);
    assert_eq!(problems.len(), 3, "{problems:?}");
}
