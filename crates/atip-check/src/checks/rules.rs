//! The rules that read one document: what a check judges of one envelope, as functions of the document alone.
//!
//! The checks of a run call these on every envelope the run saw, and the tests of the contract's examples call them
//! on a file, so one implementation judges both. Each function is the pass condition of one check; a finding that
//! belongs to another check's rule is not reported here (approaches/checker.md 2, "one owner per pass condition").

use serde_json::Value;

use crate::claim::{self, Claim};
use crate::fixture::RegistryRow;
use crate::schema::schemas;

/// `L0.envelope`: what the envelope schema finds wrong, one line each. Findings that belong to `L0.no-null`
/// (a `null` in `meta` or `error`), to `L0.contract-version` (a member of a later minor than the claim, or one
/// marker without the other), to `L0.next` (under `meta.next`) and to `L0.page` (under `meta.page`) are theirs.
pub fn envelope_problems(document: &Value, claim: Claim) -> Vec<String> {
    let prepared = claim::for_envelope_schema(document, claim);
    schemas()
        .envelope_problem_list(&prepared)
        .into_iter()
        .filter(|problem| !problem.is_under("/meta/next") && !problem.is_under("/meta/page"))
        .map(|problem| problem.to_string())
        .collect()
}

/// What the envelope schema finds wrong under `/meta/next` (`L0.next`) or `/meta/page` (`L0.page`) of `document`.
pub fn problems_under(document: &Value, claim: Claim, prefix: &str) -> Vec<String> {
    let prepared = claim::for_envelope_schema(document, claim);
    schemas()
        .envelope_problem_list(&prepared)
        .into_iter()
        .filter(|problem| problem.is_under(prefix))
        .map(|problem| problem.to_string())
        .collect()
}

/// `L0.no-null`: the places in `meta` and `error` that hold `null`.
pub fn null_problems(document: &Value) -> Vec<String> {
    let mut found = Vec::new();
    for member in ["meta", "error"] {
        if let Some(value) = document.get(member) {
            claim::null_paths(value, &format!("/{member}"), &mut found);
        }
    }
    found
        .into_iter()
        .map(|path| format!("{path} is null"))
        .collect()
}

/// One condition of `L0.contract-version` as judged on one document: its id and what is wrong, when something is.
pub type Finding = (&'static str, Option<String>);

/// `L0.contract-version`, for one envelope: the conditions that apply to the claim, each with its problem.
pub fn contract_version_findings(document: &Value, claim: Claim) -> Vec<Finding> {
    let meta = |member: &str| document.pointer(&format!("/meta/{member}"));
    let mut findings: Vec<Finding> = Vec::new();
    match claim {
        Claim::V1_1 | Claim::V1_2 => {
            let missing: Vec<&str> = ["contract", "contractVersion"]
                .into_iter()
                .filter(|member| meta(member).is_none())
                .collect();
            findings.push((
                "markers-present",
                (!missing.is_empty())
                    .then(|| format!("meta has no {}, a {} tool sets both on every envelope", missing.join(" and "), claim.as_str())),
            ));
            findings.push((
                "version-is-claim",
                meta("contractVersion")
                    .filter(|version| version.as_str() != Some(claim.as_str()))
                    .map(|version| format!("meta.contractVersion is {version}, the claim is \"{}\"", claim.as_str())),
            ));
        }
        Claim::V1_0 => {
            findings.push((
                "no-contract-version",
                meta("contractVersion")
                    .map(|version| format!("meta.contractVersion is {version} although the claim is 1.0, which never writes it")),
            ));
            let later: Vec<String> = claim::later_members(document)
                .into_iter()
                .filter(|member| member != "meta.contractVersion")
                .collect();
            findings.push((
                "no-later-members",
                (!later.is_empty()).then(|| format!("the document has {} of a later minor than the claim 1.0", later.join(", "))),
            ));
        }
    }
    if document.get("ok") == Some(&Value::Bool(true)) && meta("command").and_then(Value::as_str) == Some("schema") {
        findings.push((
            "schema-version-is-claim",
            meta("schemaVersion")
                .filter(|version| version.as_str() != Some(claim.as_str()))
                .map(|version| format!("a schema answer has meta.schemaVersion {version}, the claim is \"{}\"", claim.as_str())),
        ));
    }
    findings
}

/// Whether `document` is the first line of a stream (spec 13.5): `{"type":"start", ...}` with no `ok`.
pub fn is_stream_start(document: &Value) -> bool {
    document.get("type").and_then(Value::as_str) == Some("start") && document.get("ok").is_none()
}

/// `L0.contract-version`, for the start line of a stream: its `contract` and `contractVersion` are the markers of an
/// envelope's `meta`, so the same conditions apply to them (a tool emits only envelopes or stream lines).
pub fn stream_start_findings(line: &Value, claim: Claim) -> Vec<Finding> {
    let mut meta = line.clone();
    if let Some(object) = meta.as_object_mut() {
        object.remove("type");
    }
    contract_version_findings(&serde_json::json!({"ok": true, "meta": meta}), claim)
        .into_iter()
        // A start line is no `schema` answer, whatever its command is called.
        .filter(|(id, _)| *id != "schema-version-is-claim")
        .collect()
}

/// `L0.envelope`, for one line of a stream: the stream-line schema's problems.
pub fn stream_line_problems(line: &Value) -> Vec<String> {
    schemas().stream_line_problems(line)
}

/// `L0.hint-form`: the hints of an error (and of its `details`) that start with the tool name and a space, and are
/// not one command line.
pub fn hint_problems(document: &Value, tool: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let mut check = |hint: &str| {
        if hint.starts_with(&format!("{tool} ")) {
            if let Some(why) = claim::command_line_problem(hint) {
                problems.push(format!("the hint {hint:?} starts with the tool name but is not one command line: {why}"));
            }
        }
    };
    if let Some(error) = document.get("error") {
        if let Some(hint) = error.get("hint").and_then(Value::as_str) {
            check(hint);
        }
        for detail in error.get("details").and_then(Value::as_array).into_iter().flatten() {
            if let Some(hint) = detail.get("hint").and_then(Value::as_str) {
                check(hint);
            }
        }
    }
    problems
}

/// Every error code, `(code, reason)` and warning code a document emits: the error and each of its `details`, and
/// `meta.warnings`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Emitted {
    pub codes: Vec<String>,
    pub reasons: Vec<(String, String)>,
    pub warnings: Vec<String>,
}

pub fn emitted(document: &Value) -> Emitted {
    let mut found = Emitted::default();
    let mut take = |error: &Value| {
        let Some(code) = error.get("code").and_then(Value::as_str) else {
            return;
        };
        found.codes.push(code.to_string());
        if let Some(reason) = error.get("reason").and_then(Value::as_str) {
            found.reasons.push((code.to_string(), reason.to_string()));
        }
    };
    if let Some(error) = document.get("error") {
        take(error);
        for detail in error.get("details").and_then(Value::as_array).into_iter().flatten() {
            take(detail);
        }
    }
    for warning in document
        .pointer("/meta/warnings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(code) = warning.get("code").and_then(Value::as_str) {
            found.warnings.push(code.to_string());
        }
    }
    found
}

/// The names of the reasons of one registry entry as a tool prints it (`reasons[].reason`).
pub fn reason_names(entry: &Value) -> Vec<String> {
    entry
        .get("reasons")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|reason| reason.get("reason").and_then(Value::as_str))
        .map(str::to_string)
        .collect()
}

/// `L0.registry`, for one envelope: every error code, reason and warning code it emits is registered. `rows` is the
/// error registry (with the reasons of each code), `warning_codes` the codes of the warning registry; a warning code
/// may also be in the error registry, as 1.0 had it. Reasons are a member of 1.1: under a 1.0 claim they are
/// `L0.contract-version`'s.
pub fn registry_findings(
    document: &Value,
    rows: &[RegistryRow],
    warning_codes: &[String],
    claim: Claim,
) -> Vec<Finding> {
    let found = emitted(document);
    // A code that is not UPPER_SNAKE_CASE is the envelope schema's to refuse, not the registry's.
    let unregistered: Vec<&str> = found
        .codes
        .iter()
        .map(String::as_str)
        .filter(|code| is_upper_snake(code) && !rows.iter().any(|row| row.code == *code))
        .collect();
    let unknown_reasons: Vec<String> = found
        .reasons
        .iter()
        .filter(|_| claim.at_least(Claim::V1_1))
        .filter(|(code, reason)| {
            rows.iter()
                .find(|row| &row.code == code)
                .is_some_and(|row| !row.reasons.as_deref().unwrap_or_default().contains(reason))
        })
        .map(|(code, reason)| format!("{reason} under {code}"))
        .collect();
    let unlisted: Vec<&str> = found
        .warnings
        .iter()
        .map(String::as_str)
        .filter(|code| is_upper_snake(code) && !warning_codes.iter().any(|known| known == code) && !rows.iter().any(|row| row.code == *code))
        .collect();
    vec![
        (
            "emitted-codes-registered",
            (!unregistered.is_empty()).then(|| format!("it failed with {}, which is not in the registry", unregistered.join(", "))),
        ),
        (
            "emitted-reasons-registered",
            (!unknown_reasons.is_empty())
                .then(|| format!("it carries the reason {}, which the registry does not list there", unknown_reasons.join(", "))),
        ),
        (
            "emitted-warnings-registered",
            (!unlisted.is_empty()).then(|| format!("it warned with {}, which neither registry has", unlisted.join(", "))),
        ),
    ]
}

/// The code a document is the answer to `schema --error <CODE>` for, when `CODE` is only in the warning registry: the
/// `received` of a refusal or the code of an entry answered. Used on documents whose call is not known (the examples).
pub fn warning_only_code_of(document: &Value, rows: &[RegistryRow], warning_codes: &[String]) -> Option<String> {
    let warning_only = |code: &str| warning_codes.iter().any(|known| known == code) && !rows.iter().any(|row| row.code == code);
    let code = if document.get("ok") == Some(&Value::Bool(false)) {
        let error = document.get("error");
        let field = error.and_then(|error| error.get("field")).and_then(Value::as_str);
        error
            .and_then(|error| error.get("received"))
            .and_then(Value::as_str)
            .filter(|_| field == Some("--error"))
    } else if document.pointer("/data/kind").and_then(Value::as_str) == Some("error") {
        document.pointer("/data/error/code").and_then(Value::as_str)
    } else {
        None
    };
    code.filter(|code| warning_only(code)).map(str::to_string)
}

/// `L0.registry`, for the answer to `schema --error <CODE>` with a code only the warning registry has (7.3.4): it is a
/// refusal (no entry), its code is `USAGE`, its field `--error`, and its hint `<tool> schema --warning <CODE> --json`.
/// The exit (2) is `L0.ok-exit`'s, which holds every failure to the registered exit of its code.
pub fn warning_only_findings(document: &Value, tool: &str, code: &str) -> Vec<Finding> {
    let refused = document.get("ok") == Some(&Value::Bool(false));
    let error = document.get("error");
    let text = |member: &str| error.and_then(|error| error.get(member)).and_then(Value::as_str);
    let wanted = format!("{tool} schema --warning {code} --json");
    let mut findings: Vec<Finding> = vec![(
        "warning-only-refused",
        (!refused).then(|| format!("`--error {code}` answered an entry for a code only the warning registry has")),
    )];
    if refused {
        findings.push((
            "warning-only-code",
            (text("code") != Some("USAGE")).then(|| format!("the refusal of `--error {code}` has the code {}, expected USAGE", text("code").unwrap_or("missing"))),
        ));
        findings.push((
            "warning-only-field",
            (text("field") != Some("--error")).then(|| format!("the refusal of `--error {code}` has the field {}, expected \"--error\"", text("field").unwrap_or("missing"))),
        ));
        findings.push((
            "warning-only-hint",
            (text("hint") != Some(wanted.as_str())).then(|| format!("the refusal of `--error {code}` has the hint {:?}, expected {wanted:?}", text("hint"))),
        ));
    }
    findings
}

/// `UPPER_SNAKE_CASE`, the pattern of codes (error.schema.json `$defs/code`).
fn is_upper_snake(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase())
        && name.split('_').all(|part| {
            !part.is_empty() && part.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        })
}

/// `L0.next`, for one envelope that has `meta.next`: the entries are valid, and each has no placeholder, `--json`
/// when the call was in JSON mode, and a `command` that splits into the tool name and the `argv` (spec 6.8).
pub fn next_findings(document: &Value, claim: Claim, json_mode: bool, tool_fallback: &str) -> Vec<Finding> {
    let Some(next) = document.pointer("/meta/next") else {
        return Vec::new();
    };
    let found = problems_under(document, claim, "/meta/next");
    let mut findings: Vec<Finding> = vec![(
        "entries-valid",
        (!found.is_empty()).then(|| format!("meta.next is not valid: {}", found.iter().take(2).cloned().collect::<Vec<_>>().join(", "))),
    )];
    let tool = document.pointer("/meta/tool").and_then(Value::as_str).unwrap_or(tool_fallback);
    for entry in next.as_array().into_iter().flatten() {
        let (Some(command), Some(argv)) = (
            entry.get("command").and_then(Value::as_str),
            entry.get("argv").and_then(Value::as_array),
        ) else {
            continue;
        };
        let words: Vec<&str> = argv.iter().filter_map(Value::as_str).collect();
        let placeholder = words
            .iter()
            .find(|word| atip::has_placeholder(word) || **word == "…" || **word == "...");
        findings.push((
            "no-placeholder",
            placeholder.map(|word| format!("next argv holds the placeholder {word:?}")),
        ));
        if json_mode && !words.is_empty() {
            let has_json = words.iter().take_while(|word| **word != "--").any(|word| *word == "--json");
            findings.push((
                "json-flag",
                (!has_json).then(|| format!("a next step of a JSON-mode call has no --json: {command}")),
            ));
        }
        let mut expected = vec![tool.to_string()];
        expected.extend(words.iter().map(ToString::to_string));
        let split = atip::split_argv(command);
        findings.push((
            "command-matches-argv",
            (split != expected).then(|| format!("next command {command:?} splits to {split:?}, not {expected:?}")),
        ));
    }
    findings
}

/// `L0.page`, for one envelope that has `meta.page`: the page is valid, and `nextCursor` has its entry in `meta.next` (spec 6.9).
pub fn page_findings(document: &Value, claim: Claim) -> Vec<Finding> {
    let Some(page) = document.pointer("/meta/page") else {
        return Vec::new();
    };
    let found = problems_under(document, claim, "/meta/page");
    let mut findings: Vec<Finding> = vec![(
        "page-valid",
        (!found.is_empty()).then(|| format!("meta.page is not valid: {}", found.iter().take(2).cloned().collect::<Vec<_>>().join(", "))),
    )];
    if let Some(cursor) = page.get("nextCursor").and_then(Value::as_str) {
        let has_entry = document
            .pointer("/meta/next")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.get("argv").and_then(Value::as_array))
            .any(|argv| {
                let words: Vec<&str> = argv.iter().filter_map(Value::as_str).collect();
                words.windows(2).any(|pair| pair[0] == "--cursor" && pair[1] == cursor)
                    || words.contains(&format!("--cursor={cursor}").as_str())
            });
        findings.push((
            "next-cursor-entry",
            (!has_entry).then(|| format!("meta.page.nextCursor is {cursor:?} but meta.next has no entry with --cursor {cursor}")),
        ));
    }
    findings
}

/// The problems of the entries a `schema` answer of the kind `errors`, `error`, `warnings` or `warning` lists, against
/// the registry-entry and warning-entry schemas. A 1.0 claim has no `reasons` (`L0.contract-version`'s).
pub fn entry_problems(document: &Value, claim: Claim) -> Vec<String> {
    if document.get("ok") != Some(&Value::Bool(true))
        || document.pointer("/meta/command").and_then(Value::as_str) != Some("schema")
    {
        return Vec::new();
    }
    let data = document.get("data");
    let kind = data.and_then(|data| data.get("kind")).and_then(Value::as_str);
    let (entries, is_warning): (Vec<&Value>, bool) = match kind {
        Some("errors") => (data.and_then(|data| data.get("errors")).and_then(Value::as_array).into_iter().flatten().collect(), false),
        Some("error") => (data.and_then(|data| data.get("error")).into_iter().collect(), false),
        Some("warnings") => (data.and_then(|data| data.get("warnings")).and_then(Value::as_array).into_iter().flatten().collect(), true),
        Some("warning") => (data.and_then(|data| data.get("warning")).into_iter().collect(), true),
        _ => return Vec::new(),
    };
    entries
        .into_iter()
        .flat_map(|entry| {
            let code = entry.get("code").map_or("?".to_string(), Value::to_string);
            let problems = if is_warning {
                schemas().warning_entry_problems(entry)
            } else {
                let mut prepared = entry.clone();
                if claim == Claim::V1_0 {
                    if let Some(object) = prepared.as_object_mut() {
                        object.remove("reasons");
                    }
                }
                schemas().registry_entry_problems(&prepared)
            };
            problems.into_iter().map(move |problem| format!("the entry {code} is invalid: {problem}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn failure(meta_extra: Value, error_extra: Value) -> Value {
        let mut document = json!({
            "ok": false,
            "error": {"code": "USAGE", "message": "m", "retryable": false},
            "meta": {"schemaVersion": "1.0", "tool": "t", "toolVersion": "1", "contract": "atip/1"}
        });
        for (key, value) in meta_extra.as_object().expect("object") {
            document["meta"][key] = value.clone();
        }
        for (key, value) in error_extra.as_object().expect("object") {
            document["error"][key] = value.clone();
        }
        document
    }

    #[test]
    fn atip_check_59_a_finding_belongs_to_one_rule_and_the_envelope_schema_leaves_the_others_theirs() {
        let fine = failure(json!({"contractVersion": "1.1"}), json!({"reason": "NOT_JSON"}));
        assert!(envelope_problems(&fine, Claim::V1_1).is_empty());
        // A null is no-null's: the envelope check does not report it.
        let null = failure(json!({}), json!({"hint": null}));
        assert!(envelope_problems(&null, Claim::V1_1).is_empty());
        assert_eq!(null_problems(&null), ["/error/hint is null"]);
        // A later member under a 1.0 claim is contract-version's.
        assert!(envelope_problems(&fine, Claim::V1_0).is_empty());
        let findings = contract_version_findings(&fine, Claim::V1_0);
        assert!(findings.iter().any(|(id, problem)| *id == "no-contract-version" && problem.is_some()));
        assert!(findings.iter().any(|(id, problem)| *id == "no-later-members" && problem.is_some()));
        // A contractVersion without contract is contract-version's, not the schema's dependentRequired.
        let mut lone = failure(json!({"contractVersion": "1.1"}), json!({}));
        lone["meta"].as_object_mut().expect("meta").remove("contract");
        assert!(envelope_problems(&lone, Claim::V1_1).is_empty());
        assert!(contract_version_findings(&lone, Claim::V1_1)
            .iter()
            .any(|(id, problem)| *id == "markers-present" && problem.is_some()));
        // What the schema owns: a wrong type, an unknown meta member.
        let bad = failure(json!({"unchanged": true}), json!({}));
        assert!(!envelope_problems(&bad, Claim::V1_1).is_empty());
        // The next and page problems are theirs.
        let mut paged = failure(json!({"page": {"returned": -1}}), json!({}));
        assert!(envelope_problems(&paged, Claim::V1_1).is_empty());
        assert!(!problems_under(&paged, Claim::V1_1, "/meta/page").is_empty());
        paged["meta"]["page"] = json!({"returned": 1});
        assert!(problems_under(&paged, Claim::V1_1, "/meta/page").is_empty());
    }

    #[test]
    fn atip_check_59_the_markers_and_the_version_are_the_claims() {
        let ok = |document: &Value, claim| contract_version_findings(document, claim).iter().all(|(_, problem)| problem.is_none());
        assert!(ok(&failure(json!({"contractVersion": "1.1"}), json!({})), Claim::V1_1));
        assert!(!ok(&failure(json!({}), json!({})), Claim::V1_1), "no contractVersion");
        assert!(!ok(&failure(json!({"contractVersion": "1.0"}), json!({})), Claim::V1_1), "says 1.0");
        assert!(!ok(&failure(json!({"contractVersion": "1.2"}), json!({})), Claim::V1_1), "another minor");
        assert!(ok(&failure(json!({}), json!({})), Claim::V1_0));
        assert!(!ok(&failure(json!({"contractVersion": "1.1"}), json!({})), Claim::V1_0));
        let schema = |version: &str| json!({"ok": true, "data": {"kind": "errors", "errors": []}, "meta": {"command": "schema", "contract": "atip/1", "contractVersion": "1.1", "schemaVersion": version, "tool": "t", "toolVersion": "1"}});
        assert!(ok(&schema("1.1"), Claim::V1_1));
        assert!(!ok(&schema("1.0"), Claim::V1_1), "a schema answer's schemaVersion is the claim");
    }

    #[test]
    fn atip_check_59_a_hint_that_starts_with_the_tool_name_is_judged_as_one_command_line() {
        let with = |hint: &str| failure(json!({}), json!({"hint": hint}));
        assert!(hint_problems(&with("t schema member --json"), "t").is_empty());
        assert!(hint_problems(&with("Run the fixer"), "t").is_empty());
        assert!(hint_problems(&with("tool can't do that"), "t").is_empty(), "the tool name is `t`, followed by no space");
        assert_eq!(hint_problems(&with("t can't do that."), "t").len(), 1);
        let mut details = with("t ok --json");
        details["error"]["details"] = json!([{"code": "X", "message": "m", "hint": "t a | b"}]);
        assert_eq!(hint_problems(&details, "t").len(), 1);
    }

    #[test]
    fn atip_check_59_every_code_reason_and_warning_a_document_emits_is_listed() {
        let mut document = failure(json!({"warnings": [{"code": "W", "message": "m"}]}), json!({"code": "INPUT_INVALID", "reason": "NOT_JSON"}));
        document["error"]["details"] = json!([{"code": "INPUT_INVALID", "reason": "UNKNOWN_KEY", "message": "m"}, {"code": "USAGE", "message": "m"}]);
        let found = emitted(&document);
        assert_eq!(found.codes, ["INPUT_INVALID", "INPUT_INVALID", "USAGE"]);
        assert_eq!(found.reasons, [("INPUT_INVALID".to_string(), "NOT_JSON".to_string()), ("INPUT_INVALID".to_string(), "UNKNOWN_KEY".to_string())]);
        assert_eq!(found.warnings, ["W"]);
        assert_eq!(reason_names(&json!({"code": "X", "reasons": [{"reason": "A", "meaning": "m", "fix": "f"}]})), ["A"]);
    }
}
