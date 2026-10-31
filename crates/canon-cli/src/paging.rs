//! Bounded collections and field projection (0.20 D4).
//!
//! `query`, `finding list`, `divergence list` and `export` return at most
//! `--limit` records (default [`DEFAULT_LIMIT`]); `--all` lifts the bound and
//! `--cursor` resumes after the last record of the previous page. The JSON
//! `meta.page` carries `returned`, `total` and, while more remain,
//! `nextCursor` (and `meta.next` the same call with it); human mode ends with
//! a line naming the next cursor when more remain.
//!
//! A cursor is opaque to the caller. It names the command and kind it came
//! from (`scope`) and the key of the last record it returned, so it stays
//! valid while the result set is unchanged and is refused (`CURSOR_INVALID`)
//! when handed to a different listing or when that record is gone.

use std::collections::{BTreeMap, BTreeSet};

use clap::Args;
use serde_json::{json, Value};

use crate::cli_error::{CliError, CliErrorExt as _};

/// Records per page when neither `--limit` nor `--all` is given.
pub const DEFAULT_LIMIT: usize = 100;

const SEPARATOR: char = '\u{1f}';

/// The paging flags shared by every bounded listing.
#[derive(Args, Debug, Clone, Default)]
pub struct PageFlags {
    /// Return at most this many records (default 100)
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..), conflicts_with = "all", value_name = "N")]
    pub limit: Option<u64>,
    /// Resume after the last record of the previous page (the `nextCursor` it reported)
    #[arg(long, value_name = "CURSOR")]
    pub cursor: Option<String>,
    /// Return every record from the cursor on, not one page
    #[arg(long)]
    pub all: bool,
}

impl PageFlags {
    pub fn page(&self) -> Page {
        let limit = if self.all { None } else { Some(self.limit.map_or(DEFAULT_LIMIT, |n| usize::try_from(n).unwrap_or(usize::MAX))) };
        Page { limit, cursor: self.cursor.clone() }
    }
}

/// A resolved page request. `limit: None` is `--all`.
#[derive(Debug, Clone)]
pub struct Page {
    pub limit: Option<usize>,
    pub cursor: Option<String>,
}

impl Page {
    pub fn first_page() -> Self {
        Page { limit: Some(DEFAULT_LIMIT), cursor: None }
    }
}

/// What a page says about the rest of the result.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PageMeta {
    pub returned: usize,
    /// Every record the command matched, across all pages.
    pub total: usize,
    /// Present exactly when more records follow.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

impl PageMeta {
    /// The extra `meta` a listing passes to the envelope: `{"page": {returned, total, nextCursor?}}`.
    pub fn to_json(&self) -> Value {
        json!({ "page": self })
    }

    /// `meta.page` added to the extra `meta` members in `meta`.
    pub fn insert_into(&self, meta: &mut serde_json::Map<String, Value>) {
        meta.insert("page".to_string(), json!(self));
    }

    /// The last line of a human listing, when more records remain.
    pub fn human_line(&self) -> Option<String> {
        let cursor = self.next_cursor.as_ref()?;
        Some(format!("{} of {} shown; next cursor: {cursor}  (rerun with --cursor {cursor}, or --all)", self.returned, self.total))
    }
}

fn encode(scope: &str, key: &str) -> String {
    format!("{scope}{SEPARATOR}{key}").bytes().map(|b| format!("{b:02x}")).collect()
}

fn decode(cursor: &str) -> Option<(String, String)> {
    if cursor.is_empty() || !cursor.len().is_multiple_of(2) || !cursor.is_ascii() {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..cursor.len()).step_by(2).map(|i| u8::from_str_radix(&cursor[i..i + 2], 16).ok()).collect();
    let text = String::from_utf8(bytes?).ok()?;
    text.split_once(SEPARATOR).map(|(scope, key)| (scope.to_string(), key.to_string()))
}

/// The record key a cursor points after, when the cursor is one canon issued.
pub fn cursor_key(cursor: &str) -> Option<String> {
    decode(cursor).map(|(_, key)| key)
}

fn invalid_cursor(why: &str) -> CliError {
    CliError::refused("CURSOR_INVALID", why.to_string()).field("--cursor").hint("drop --cursor to start from the first page, or pass the `nextCursor` this same command returned")
}

/// Cuts one page out of `items`, which must already be in their final,
/// deterministic order. `key` names one record uniquely (equal keys are told
/// apart by their order of appearance). `scope` identifies the listing — the
/// command and what it lists — so a cursor from another listing is refused.
pub fn paginate<T>(scope: &str, items: Vec<T>, key: impl Fn(&T) -> String, page: &Page) -> Result<(Vec<T>, PageMeta), CliError> {
    let total = items.len();
    // Keys are built in order and only as far as the page needs: up to the
    // cursor's record (to find it), then up to the page's last record.
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut keys: Vec<String> = Vec::new();
    let mut extend_to = |upto: usize, keys: &mut Vec<String>| {
        for item in &items[keys.len()..upto.min(total)] {
            let base = key(item);
            let n = seen.entry(base.clone()).or_insert(0);
            *n += 1;
            keys.push(format!("{base}#{n}"));
        }
    };

    let start = match &page.cursor {
        None => 0,
        Some(cursor) => {
            let (cursor_scope, last) = decode(cursor).ok_or_else(|| invalid_cursor("the cursor is not one canon issued"))?;
            if cursor_scope != scope {
                return Err(invalid_cursor(&format!("the cursor belongs to `{cursor_scope}`, not `{scope}`")));
            }
            let mut found = None;
            while found.is_none() && keys.len() < total {
                let next = keys.len() + 1;
                extend_to(next, &mut keys);
                if keys.last().is_some_and(|k| *k == last) {
                    found = Some(keys.len() - 1);
                }
            }
            found.ok_or_else(|| invalid_cursor("the record the cursor points after is no longer in the result (the store or the filters changed)"))? + 1
        }
    };

    let end = page.limit.map_or(total, |limit| start.saturating_add(limit).min(total));
    let next_cursor = (end < total).then(|| {
        extend_to(end, &mut keys);
        encode(scope, &keys[end - 1])
    });
    let slice: Vec<T> = items.into_iter().skip(start).take(end - start).collect();
    let meta = PageMeta { returned: slice.len(), total, next_cursor };
    Ok((slice, meta))
}

/// A field name compared without its case style: lowercase, `_` and `-` dropped, so
/// `scenarioId`, `scenario_id` and `Scenario-ID` are one field.
pub fn normalize(name: &str) -> String {
    name.chars().filter(|c| *c != '_' && *c != '-').flat_map(char::to_lowercase).collect()
}

/// Refuses (`USAGE`, `field` `--fields`) any requested `--fields` name that `known` does not hold, in either case
/// style (`scenarioId` or `scenario_id`); `suggestions` carries the nearest known fields.
pub fn check_fields(command: &str, requested: &[String], known: &BTreeSet<String>) -> Result<(), CliError> {
    let normal: Vec<(String, &String)> = known.iter().map(|k| (normalize(k), k)).collect();
    let unknown: Vec<&str> = requested.iter().map(String::as_str).filter(|f| !normal.iter().any(|(n, _)| *n == normalize(f))).collect();
    if unknown.is_empty() {
        return Ok(());
    }
    let list = known.iter().map(String::as_str).collect::<Vec<_>>().join(", ");
    let mut suggestions: Vec<String> = Vec::new();
    for field in &unknown {
        let wanted = normalize(field);
        for near in crate::suggest::nearest(&wanted, normal.iter().map(|(n, _)| n.as_str())) {
            if let Some((_, original)) = normal.iter().find(|(n, _)| *n == near) {
                if !suggestions.contains(original) {
                    suggestions.push((*original).clone());
                }
            }
        }
    }
    suggestions.truncate(3);
    Err(CliError::refused("USAGE", format!("`{command}` has no field {}", unknown.iter().map(|f| format!("`{f}`")).collect::<Vec<_>>().join(", ")))
        .hint(format!("known fields: {list} (a comma list; camelCase or snake_case both match)"))
        .field("--fields")
        .expected(known.iter().cloned())
        .received(unknown.join(","))
        .suggestions(suggestions))
}

/// [`check_fields`] against a fixed list of field names.
pub fn check_known(command: &str, requested: &[String], known: &[&str]) -> Result<(), CliError> {
    check_fields(command, requested, &known.iter().map(std::string::ToString::to_string).collect())
}

/// Whether `fields` names `member`, in either case style.
pub fn wants(fields: &[String], member: &str) -> bool {
    let member = normalize(member);
    fields.iter().any(|f| normalize(f) == member)
}

/// Keeps only `fields` of an object record, matched in either case style; the record's own keys are
/// kept as they are. A field the record happens not to carry is omitted.
pub fn project(record: &Value, fields: &[String]) -> Value {
    let Some(object) = record.as_object() else { return record.clone() };
    let mut out = serde_json::Map::new();
    for field in fields {
        let wanted = normalize(field);
        for (key, value) in object.iter().filter(|(key, _)| normalize(key) == wanted) {
            out.insert(key.clone(), value.clone());
        }
    }
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(limit: Option<usize>, cursor: Option<String>) -> Page {
        Page { limit, cursor }
    }

    #[test]
    fn pages_cover_the_list_once_in_order_and_duplicate_keys_do_not_stall() {
        let items: Vec<&str> = vec!["a", "b", "b", "c", "d"];
        let mut cursor = None;
        let mut got = Vec::new();
        loop {
            let (rows, meta) = paginate("t", items.clone(), std::string::ToString::to_string, &page(Some(2), cursor.clone())).unwrap();
            got.extend(rows);
            match meta.next_cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        assert_eq!(got, items);
    }

    #[test]
    fn a_cursor_from_another_scope_or_garbage_is_refused() {
        let items = vec!["a", "b", "c"];
        let (_, meta) = paginate("query:handoff", items.clone(), std::string::ToString::to_string, &page(Some(1), None)).unwrap();
        let cursor = meta.next_cursor.unwrap();
        for bad in [cursor.clone(), "zz".to_string(), String::new()] {
            let err = paginate("query:run", items.clone(), std::string::ToString::to_string, &page(Some(1), Some(bad))).unwrap_err();
            assert_eq!(err.code, "CURSOR_INVALID");
        }
    }
}
