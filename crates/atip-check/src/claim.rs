//! The contract minor a tool claims (spec 15.7), and what a document of that minor may carry (10.13 rule 16).
//!
//! A tool that claims `1.0` prints no member, document kind or path syntax the contract marks "since 1.1":
//! `meta.contractVersion`, `error.reason` (also in `details`), registry `reasons`, the `warnings` and `warning`
//! documents of `schema`, and bracketed field paths. The schemas in `schema/` mark each with `x-atip-since`, and a
//! test of this module fails when a schema marks something this module does not know.

use serde_json::Value;

/// The minors this checker knows.
pub const MINORS: [&str; 3] = ["1.0", "1.1", "1.2"];

/// The minor a tool claims.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Claim {
    V1_0,
    V1_1,
    V1_2,
}

impl Claim {
    /// The claim for `<major>.<minor>`, when the checker knows that minor.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "1.0" => Some(Self::V1_0),
            "1.1" => Some(Self::V1_1),
            "1.2" => Some(Self::V1_2),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::V1_0 => "1.0",
            Self::V1_1 => "1.1",
            Self::V1_2 => "1.2",
        }
    }

    /// Whether the claim is `other` or a later minor: a 1.2 tool implements everything 1.1 and 1.0 do (spec 15.1).
    pub fn at_least(self, other: Self) -> bool {
        self.rank() >= other.rank()
    }

    fn rank(self) -> u8 {
        match self {
            Self::V1_0 => 0,
            Self::V1_1 => 1,
            Self::V1_2 => 2,
        }
    }
}

/// Whether a field path uses the bracketed key syntax of 6.10 (since 1.1).
fn is_bracketed(path: &str) -> bool {
    path.contains("[\"")
}

/// The members, kinds and syntaxes of a document that only a tool claiming 1.1 may print, each as a JSON pointer
/// and what it is. `meta.contractVersion` is listed too: a 1.0 tool prints it nowhere.
pub fn later_members(document: &Value) -> Vec<String> {
    let mut found = Vec::new();
    if document.pointer("/meta/contractVersion").is_some() {
        found.push("meta.contractVersion".to_string());
    }
    if let Some(fields) = document.pointer("/meta/fields").and_then(Value::as_array) {
        if fields.iter().filter_map(Value::as_str).any(is_bracketed) {
            found.push("a bracketed key in meta.fields".to_string());
        }
    }
    if document.pointer("/error/reason").is_some() {
        found.push("error.reason".to_string());
    }
    if let Some(details) = document
        .pointer("/error/details")
        .and_then(Value::as_array)
    {
        if details.iter().any(|detail| detail.get("reason").is_some()) {
            found.push("error.details[].reason".to_string());
        }
    }
    if let Some(kind) = document
        .pointer("/data/kind")
        .and_then(Value::as_str)
        .filter(|_| document.pointer("/meta/command").and_then(Value::as_str) == Some("schema"))
    {
        if kind == "warnings" || kind == "warning" {
            found.push(format!("the schema document kind {kind}"));
        }
    }
    let registry_rows = document
        .pointer("/data/errors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .chain(document.pointer("/data/error"))
        .filter(|_| document.pointer("/meta/command").and_then(Value::as_str) == Some("schema"));
    if registry_rows.into_iter().any(|row| row.get("reasons").is_some()) {
        found.push("reasons in a registry entry".to_string());
    }
    found
}

/// `document` without the members only a 1.1 tool may print, so that the envelope schema judges only what
/// `L0.envelope` owns when the claim is 1.0 (the members themselves are `L0.contract-version`'s).
pub fn without_later_members(document: &Value) -> Value {
    let mut copy = document.clone();
    if let Some(meta) = copy.get_mut("meta").and_then(Value::as_object_mut) {
        meta.remove("contractVersion");
        let emptied = meta.get_mut("fields").and_then(Value::as_array_mut).is_some_and(|fields| {
            fields.retain(|path| !path.as_str().is_some_and(is_bracketed));
            fields.is_empty()
        });
        if emptied {
            meta.remove("fields");
        }
    }
    if let Some(error) = copy.get_mut("error").and_then(Value::as_object_mut) {
        error.remove("reason");
        if let Some(details) = error.get_mut("details").and_then(Value::as_array_mut) {
            for detail in details.iter_mut().filter_map(Value::as_object_mut) {
                detail.remove("reason");
            }
        }
    }
    copy
}

/// `document` as the envelope schema should see it for `L0.envelope`: without what other checks own. Every `null`
/// inside `meta` or `error` is `L0.no-null`'s and is dropped; for a 1.0 claim the later members are dropped
/// ([`without_later_members`]); a `contractVersion` without `contract` is `L0.contract-version`'s (the schema makes
/// one depend on the other), so `contract` is supplied; for a later claim a `contractVersion` written as `"1.0"`
/// (the schema's pattern refuses it) is `L0.contract-version`'s too (`version-is-claim`), so the claim stands for it.
pub fn for_envelope_schema(document: &Value, claim: Claim) -> Value {
    let mut copy = match claim {
        Claim::V1_0 => without_later_members(document),
        Claim::V1_1 | Claim::V1_2 => document.clone(),
    };
    for member in ["meta", "error"] {
        if let Some(value) = copy.get_mut(member) {
            drop_nulls(value);
        }
    }
    if let Some(meta) = copy.get_mut("meta").and_then(Value::as_object_mut) {
        if meta.contains_key("contractVersion") && !meta.contains_key("contract") {
            meta.insert("contract".to_string(), Value::String("atip/1".to_string()));
        }
        if claim.at_least(Claim::V1_1) && meta.get("contractVersion").and_then(Value::as_str) == Some("1.0") {
            meta.insert("contractVersion".to_string(), Value::String(claim.as_str().to_string()));
        }
    }
    copy
}

fn drop_nulls(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.retain(|_, member| !member.is_null());
            for member in object.values_mut() {
                drop_nulls(member);
            }
        }
        Value::Array(items) => {
            items.retain(|item| !item.is_null());
            for item in items.iter_mut() {
                drop_nulls(item);
            }
        }
        _ => {}
    }
}

/// The JSON pointers of every `null` below `value` (which sits at `at`).
pub fn null_paths(value: &Value, at: &str, found: &mut Vec<String>) {
    match value {
        Value::Null => found.push(at.to_string()),
        Value::Object(object) => {
            for (key, member) in object {
                null_paths(member, &format!("{at}/{}", key.replace('~', "~0").replace('/', "~1")), found);
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                null_paths(item, &format!("{at}/{index}"), found);
            }
        }
        _ => {}
    }
}

/// Why `hint` is not one command line, when it starts with the tool name and a space and so must be one (7.1.2):
/// it must split as a POSIX shell would, and hold no unquoted control operator, redirection or substitution.
/// `<UPPER_CASE>` is a placeholder, not a redirection.
pub fn command_line_problem(hint: &str) -> Option<String> {
    let chars: Vec<char> = hint.chars().collect();
    let mut quote: Option<char> = None;
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        let next = chars.get(at + 1).copied();
        match quote {
            Some('\'') => {
                if c == '\'' {
                    quote = None;
                }
            }
            Some(_) => match c {
                '\\' => at += 1,
                '"' => quote = None,
                '`' => return Some("a command substitution inside double quotes".to_string()),
                '$' if next == Some('(') => {
                    return Some("a command substitution inside double quotes".to_string())
                }
                _ => {}
            },
            None => match c {
                '\'' | '"' => quote = Some(c),
                '\\' => {
                    if next.is_none() {
                        return Some("it ends with a backslash".to_string());
                    }
                    at += 1;
                }
                ';' | '|' | '&' | '`' => {
                    return Some(format!("an unquoted `{c}` is not part of one command line"))
                }
                '$' if next == Some('(') => {
                    return Some("an unquoted command substitution".to_string())
                }
                '<' => {
                    let close = chars[at + 1..].iter().position(|mark| *mark == '>');
                    let placeholder = close.is_some_and(|len| {
                        len > 0 && chars[at + 1..at + 1 + len].iter().all(|name| name.is_ascii_uppercase() || name.is_ascii_digit() || *name == '_')
                    });
                    match close {
                        Some(len) if placeholder => at += len + 1,
                        _ => return Some("an unquoted `<` is a redirection, not a placeholder".to_string()),
                    }
                }
                '>' => return Some("an unquoted `>` is a redirection".to_string()),
                _ => {}
            },
        }
        at += 1;
    }
    quote.map(|mark| format!("a {mark} quote is never closed"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn atip_check_58_the_schemas_mark_exactly_what_this_module_knows_as_later_than_1_0() {
        let marked = |text: &str| text.matches("\"x-atip-since\"").count();
        // envelope: meta.contractVersion and the bracketed field path; error: reason; registry entry: reasons;
        // warning entry: the whole document.
        assert_eq!(marked(atip::schema::ENVELOPE_SCHEMA), 2);
        assert_eq!(marked(atip::schema::ERROR_SCHEMA), 1);
        assert_eq!(marked(atip::schema::REGISTRY_ENTRY_SCHEMA), 1);
        assert_eq!(marked(atip::schema::WARNING_ENTRY_SCHEMA), 1);
    }

    #[test]
    fn atip_check_58_a_one_zero_claim_finds_each_member_of_a_later_minor() {
        let document = json!({
            "ok": false,
            "error": {"code": "X", "reason": "R", "message": "m", "retryable": false, "details": [{"code": "X", "reason": "R", "message": "m"}]},
            "meta": {"contractVersion": "1.1", "fields": ["a.b", "a[\"b.c\"]"]}
        });
        assert_eq!(
            later_members(&document),
            ["meta.contractVersion", "a bracketed key in meta.fields", "error.reason", "error.details[].reason"]
        );
        let stripped = without_later_members(&document);
        assert!(later_members(&stripped).is_empty(), "{stripped}");
        assert_eq!(stripped["meta"]["fields"], json!(["a.b"]));
        let schema = json!({"ok": true, "data": {"kind": "warnings", "warnings": []}, "meta": {"command": "schema"}});
        assert_eq!(later_members(&schema), ["the schema document kind warnings"]);
        let errors = json!({"ok": true, "data": {"kind": "errors", "errors": [{"code": "X", "reasons": []}]}, "meta": {"command": "schema"}});
        assert_eq!(later_members(&errors), ["reasons in a registry entry"]);
    }

    #[test]
    fn atip_check_58_nulls_inside_meta_and_error_are_found_and_dropped_for_the_schema() {
        let document = json!({"ok": false, "error": {"code": "X", "hint": null}, "meta": {"context": {"repo": null}}, "data": {"keep": null}});
        let mut found = Vec::new();
        for member in ["meta", "error"] {
            null_paths(&document[member], &format!("/{member}"), &mut found);
        }
        assert_eq!(found, ["/meta/context/repo", "/error/hint"]);
        let copy = for_envelope_schema(&document, Claim::V1_1);
        assert_eq!(copy["error"], json!({"code": "X"}));
        assert_eq!(copy["data"], json!({"keep": null}), "data is the report's, not the envelope's");
        let lone = json!({"ok": true, "data": {}, "meta": {"contractVersion": "1.1"}});
        assert_eq!(for_envelope_schema(&lone, Claim::V1_1)["meta"]["contract"], "atip/1");
    }

    #[test]
    fn atip_check_58_a_hint_that_starts_with_the_tool_is_one_command_line_or_it_is_not() {
        for good in [
            "acme member tag --input '{\"operands\":[7]}' --json",
            "acme schema member tag --json",
            "acme finding close <CHANGE_ID> --round <N> --json",
            "acme schema \"member tag\" --json",
        ] {
            assert_eq!(command_line_problem(good), None, "{good}");
        }
        for bad in [
            "acme member tag --input '{\"operands\":[7]} --json",
            "acme can't read this document; quote it in single quotes.",
            "acme a | b",
            "acme run > out",
            "acme run $(date)",
            "acme run \\",
        ] {
            assert!(command_line_problem(bad).is_some(), "{bad}");
        }
    }
}
