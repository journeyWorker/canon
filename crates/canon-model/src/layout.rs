//! The one rule every store names a record by (0.30 `ledger-naming`,
//! `docs/design/ledger-layout.md` R1–R10).
//!
//! ```text
//! kind=<kind>/<component>_<component>…_<digest8>.json
//! ```
//!
//! This module is the component table of the design: [`components`] gives
//! each kind's name components as TYPED values ([`Component`]), read from
//! the record's own body. Folds, query output, cursors and error messages
//! use the typed components ([`display_key`]); only [`Name`] escapes and
//! joins them, so the git ledger, the archive, `_staging`, a plugin
//! overlay, the sqlite and Postgres `(kind, id, digest)` keys and the R2
//! object key are the same string.
//!
//! # Escaping (R6)
//! A component is written in its canonical lowercase form. Every byte
//! outside `[a-z0-9.-]` (an uppercase letter, `_`, `/`, `#` …) becomes `+`
//! and two lowercase hex digits: reversible ([`unescape`]), safe on
//! case-insensitive file systems, and left alone by the R2 object-store
//! client. Typed ids of the kebab/dot grammars never need it; a free-text
//! id (session id, regime key) and a task id's `#` do. A name whose stem
//! (key and digest) is over [`MAX_STEM_BYTES`] bytes replaces its
//! free-text components with `h` and the first 16 hex digits of the
//! SHA-256 of the value (the value stays in the body).

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::envelope::RecordKind;
use crate::evidence::{EvidenceViolation, FailureClass};
use crate::ids::RunId;

/// A stem (components and digest, no extension) longer than this has its
/// free-text components replaced by `h<16 hex>`.
pub const MAX_STEM_BYTES: usize = 200;

/// The hex digits of a content digest in a name (R7).
pub const DIGEST_LEN: usize = 8;

/// One component of a record's name, before escaping.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Component {
    /// The field an alternative key holds (`scenario`, `change`, `task`, `run`).
    Tag(&'static str),
    /// A typed id, in its canonical lowercase form.
    Id(String),
    /// A free-text id: shortened to a hash when the name is too long.
    Text(String),
    /// An integer, written plain decimal.
    Int(i64),
}

impl Component {
    /// The component's value as the user knows it (never escaped).
    pub fn value(&self) -> String {
        match self {
            Component::Tag(tag) => (*tag).to_string(),
            Component::Id(text) | Component::Text(text) => text.clone(),
            Component::Int(n) => n.to_string(),
        }
    }

    fn escaped(&self) -> String {
        match self {
            Component::Int(n) => n.to_string(),
            Component::Tag(tag) => (*tag).to_string(),
            Component::Id(text) | Component::Text(text) => escape(text),
        }
    }
}

/// Escape one component (R6): every byte outside `[a-z0-9.-]` as `+` and
/// two lowercase hex digits.
pub fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'.' || byte == b'-' {
            out.push(char::from(byte));
        } else {
            out.push('+');
            out.push_str(&format!("{byte:02x}"));
        }
    }
    out
}

/// The inverse of [`escape`]; `None` when `escaped` is not one.
pub fn unescape(escaped: &str) -> Option<String> {
    let bytes = escaped.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'+' {
            let hex = escaped.get(i + 1..i + 3)?;
            if !hex.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
                return None;
            }
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// The components as the user reads them, joined by `_`: the form query's
/// ID column, cursors and error messages use. Not an identity (a free-text
/// value may itself hold `_`); use [`Name::key`] to name a file or row.
pub fn display_key(components: &[Component]) -> String {
    components.iter().map(Component::value).collect::<Vec<_>>().join("_")
}

/// A record's name in every store: the escaped, joined key and the digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name {
    /// The escaped components joined by `_` (the sqlite/Postgres `id`).
    pub key: String,
    /// The content digest (8 hex digits).
    pub digest: String,
}

impl Name {
    /// `<key>_<digest>`.
    pub fn stem(&self) -> String {
        format!("{}_{}", self.key, self.digest)
    }

    /// `kind=<kind>/<stem>.<extension>`, relative to a tier root.
    pub fn relative_path(&self, kind_dir: &str, extension: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(format!("kind={kind_dir}")).join(format!("{}.{extension}", self.stem()))
    }
}

/// Build the name of `components` with `digest` (R1–R7).
pub fn name(components: &[Component], digest: &str) -> Name {
    let join = |shorten: bool| -> String {
        components
            .iter()
            .map(|component| match component {
                Component::Text(text) if shorten => shortened(text),
                other => other.escaped(),
            })
            .collect::<Vec<_>>()
            .join("_")
    };
    let mut key = join(false);
    if key.len() + 1 + digest.len() > MAX_STEM_BYTES && components.iter().any(|c| matches!(c, Component::Text(_))) {
        key = join(true);
    }
    Name { key, digest: digest.to_string() }
}

/// `h` and the first 16 hex digits of the value's SHA-256: what replaces a
/// free-text component in a name over [`MAX_STEM_BYTES`].
pub fn shortened(value: &str) -> String {
    format!("h{}", &sha256_hex(value)[..16])
}

fn sha256_hex(value: &str) -> String {
    Sha256::digest(value.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Split a record file stem (`<key>_<digest>`) into its key and digest
/// text. For a reader that has only a file name (archive scoping); a
/// record's own digest is always computed from its body (R10).
pub fn split_stem(stem: &str) -> Option<(&str, &str)> {
    let (key, digest) = stem.rsplit_once('_')?;
    (digest.len() == DIGEST_LEN && digest.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) && !key.is_empty()).then_some((key, digest))
}

fn malformed(subject: &str, detail: impl Into<String>) -> EvidenceViolation {
    EvidenceViolation::new(FailureClass::Malformed, subject, detail)
}

fn text<'a>(json: &'a Value, field: &str) -> Result<&'a str, EvidenceViolation> {
    json.get(field)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| malformed(field, format!("missing or non-string `{field}` field")))
}

fn int(json: &Value, field: &str) -> Result<i64, EvidenceViolation> {
    json.get(field).and_then(Value::as_i64).ok_or_else(|| malformed(field, format!("missing or non-integer `{field}` field")))
}

fn id(json: &Value, field: &str) -> Result<Component, EvidenceViolation> {
    Ok(Component::Id(text(json, field)?.to_string()))
}

/// A `RunId` is a case-insensitive ULID: written in its canonical
/// lowercase form (26 characters).
fn run_id(json: &Value, field: &str) -> Result<Component, EvidenceViolation> {
    let raw = text(json, field)?;
    let canonical = RunId::parse(raw).map_or_else(|_| raw.to_string(), |run| run.to_string());
    Ok(Component::Id(canonical.to_ascii_lowercase()))
}

fn present_string(json: &Value, field: &str) -> Option<String> {
    json.get(field).and_then(Value::as_str).map(str::to_string)
}

/// Whether an evidence body carries at least one join key.
pub fn evidence_has_a_key(json: &Value) -> bool {
    ["task_id", "scenario_id", "run_id"].into_iter().any(|field| json.get(field).and_then(Value::as_str).is_some_and(|s| !s.is_empty()))
}

/// The error of an evidence record with no `task_id`, `scenario_id` or
/// `run_id` (F5): the model refuses it, so no store can hold one.
pub fn keyless_evidence() -> EvidenceViolation {
    malformed("evidence", "an evidence record needs a `task_id`, a `scenario_id` or a `run_id`; none was given, so it names nothing and no store accepts it")
}

/// This kind's name components, read from the record's own body (the
/// component table, design R2/R3). The ONLY input is the body: never the
/// directory a file was found in.
pub fn components(kind: RecordKind, json: &Value) -> Result<Vec<Component>, EvidenceViolation> {
    Ok(match kind {
        RecordKind::Change => vec![id(json, "change_id")?],
        RecordKind::Task => vec![id(json, "task_id")?],
        RecordKind::Scenario => vec![id(json, "project_id")?, id(json, "scenario_id")?],
        RecordKind::Session => vec![Component::Text(text(json, "session_id")?.to_string())],
        RecordKind::Run => vec![run_id(json, "run_id")?],
        RecordKind::Event => vec![run_id(json, "run_id")?, Component::Int(int(json, "seq")?)],
        RecordKind::Handoff => vec![Component::Text(text(json, "id")?.to_string())],
        RecordKind::Review => {
            let project = id(json, "project_id")?;
            // A change-level review (0.23) names its change instead of a scenario.
            if json.get("scenario_id").is_none() {
                vec![project, Component::Tag("change"), id(json, "change_id")?]
            } else {
                vec![project, Component::Tag("scenario"), id(json, "scenario_id")?]
            }
        }
        RecordKind::Divergence => {
            vec![id(json, "project_id")?, id(json, "scenario_id")?, Component::Int(int(json, "run_seq")?), Component::Int(int(json, "round")?)]
        }
        RecordKind::Trajectory => vec![run_id(json, "run_id")?],
        RecordKind::StrategyItem => vec![Component::Text(text(json, "regime_key")?.to_string())],
        RecordKind::Evidence => {
            let project = Component::Id(present_string(json, "project_id").unwrap_or_else(|| "-".to_string()));
            let (tag, field) = if json.get("task_id").and_then(Value::as_str).is_some() {
                ("task", "task_id")
            } else if json.get("scenario_id").and_then(Value::as_str).is_some() {
                ("scenario", "scenario_id")
            } else if json.get("run_id").and_then(Value::as_str).is_some() {
                ("run", "run_id")
            } else {
                return Err(keyless_evidence());
            };
            let key = if tag == "run" { run_id(json, field)? } else { id(json, field)? };
            vec![project, Component::Tag(tag), key]
        }
        RecordKind::Subject => vec![id(json, "subject_id")?],
        RecordKind::Finding => vec![id(json, "change_id")?, Component::Int(int(json, "round")?), Component::Int(int(json, "seq")?)],
    })
}

/// The name template of `kind`, for `canon context` and error messages.
pub fn template(kind: RecordKind) -> String {
    let parts = match kind {
        RecordKind::Change => "{change_id}",
        RecordKind::Task => "{task_id}",
        RecordKind::Scenario => "{project_id}_{scenario_id}",
        RecordKind::Session => "{session_id}",
        RecordKind::Run | RecordKind::Trajectory => "{run_id}",
        RecordKind::Event => "{run_id}_{seq}",
        RecordKind::Handoff => "{id}",
        RecordKind::Review => "{project_id}_(scenario_{scenario_id}|change_{change_id})",
        RecordKind::Divergence => "{project_id}_{scenario_id}_{run_seq}_{round}",
        RecordKind::StrategyItem => "{regime_key}",
        RecordKind::Evidence => "{project_id|-}_(task_{task_id}|scenario_{scenario_id}|run_{run_id})",
        RecordKind::Subject => "{subject_id}",
        RecordKind::Finding => "{change_id}_{round}_{seq}",
    };
    format!("kind={}/{parts}_{{digest8}}.json", kind.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn escape_keeps_the_safe_set_and_reverses() {
        assert_eq!(escape("abc-1.2"), "abc-1.2");
        assert_eq!(escape("Ab_c/d#1"), "+41b+5fc+2fd+231");
        for value in ["Ab_c/d#1", "plain", "한글 id", "a+b"] {
            assert_eq!(unescape(&escape(value)).as_deref(), Some(value));
        }
        assert_eq!(unescape("+zz"), None);
    }

    #[test]
    fn evidence_names_tag_the_key_and_lead_with_the_project() {
        let body = json!({"project_id": "canon", "scenario_id": "vocab.typed.01", "run_id": "01HZ0000000000000000000000"});
        let c = components(RecordKind::Evidence, &body).unwrap();
        assert_eq!(name(&c, "abcd1234").stem(), "canon_scenario_vocab.typed.01_abcd1234");
        let keyed_by_task = json!({"task_id": "x#1.2", "scenario_id": "vocab.typed.01"});
        assert_eq!(name(&components(RecordKind::Evidence, &keyed_by_task).unwrap(), "abcd1234").stem(), "-_task_x+231.2_abcd1234");
    }

    #[test]
    fn keyless_evidence_has_no_name() {
        let err = components(RecordKind::Evidence, &json!({"project_id": "canon"})).unwrap_err();
        assert_eq!(err.subject, "evidence");
    }

    #[test]
    fn run_ids_are_written_lowercase() {
        let body = json!({"run_id": "01HZ0000000000000000000000", "seq": 7});
        let c = components(RecordKind::Event, &body).unwrap();
        assert_eq!(name(&c, "00000000").stem(), "01hz0000000000000000000000_7_00000000");
    }

    #[test]
    fn a_long_free_text_component_becomes_a_hash() {
        let long = "S".repeat(300);
        let c = components(RecordKind::Session, &json!({"session_id": long})).unwrap();
        let n = name(&c, "00000000");
        assert_eq!(n.key.len(), 17);
        assert!(n.key.starts_with('h'));
        assert!(name(&components(RecordKind::Session, &json!({"session_id": "Short/one"})).unwrap(), "00000000").key == "+53hort+2fone");
    }

    #[test]
    fn review_name_has_no_pin() {
        let body = json!({"project_id": "canon", "scenario_id": "gate.check.104", "pin": "a".repeat(40)});
        assert_eq!(name(&components(RecordKind::Review, &body).unwrap(), "e7f52dd1").stem(), "canon_scenario_gate.check.104_e7f52dd1");
        let change = json!({"project_id": "canon", "change_id": "canon-0-28", "pin": "a"});
        assert_eq!(name(&components(RecordKind::Review, &change).unwrap(), "e7f52dd1").stem(), "canon_change_canon-0-28_e7f52dd1");
    }

    #[test]
    fn integers_are_unpadded_and_order_numerically() {
        let body = json!({"change_id": "canon-0-28", "round": 2, "seq": 2});
        assert_eq!(name(&components(RecordKind::Finding, &body).unwrap(), "b10258db").stem(), "canon-0-28_2_2_b10258db");
        assert!(Component::Int(9) < Component::Int(10));
    }

    #[test]
    fn split_stem_takes_the_last_eight_hex_digits() {
        assert_eq!(split_stem("a_b_0123abcd"), Some(("a_b", "0123abcd")));
        assert_eq!(split_stem("a_b_0123abcg"), None);
    }
}
