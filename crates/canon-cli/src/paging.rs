//! Bounded collections and field projection (0.20 D4).
//!
//! `query`, `finding list`, `divergence status` and `export` return at most
//! `--limit` records (default [`DEFAULT_LIMIT`]); `--all` lifts the bound and
//! `--cursor` resumes after the last record of the previous page. The JSON
//! `meta` carries `returned`, `total`, `hasMore` and `nextCursor`; human mode
//! ends with a line naming the next cursor when more remain.
//!
//! A cursor is opaque to the caller. It names the command and kind it came
//! from (`scope`) and the key of the last record it returned, so it stays
//! valid while the result set is unchanged and is refused (`INVALID_CURSOR`)
//! when handed to a different listing or when that record is gone.

use std::collections::{BTreeMap, BTreeSet};

use clap::Args;
use serde_json::{json, Value};

use crate::cli_error::CliError;

/// Records per page when neither `--limit` nor `--all` is given.
pub const DEFAULT_LIMIT: usize = 100;

const SEPARATOR: char = '\u{1f}';

/// The paging flags shared by every bounded listing.
#[derive(Args, Debug, Clone, Default)]
pub struct PageFlags {
    /// Return at most this many records (default 100)
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..), conflicts_with = "all")]
    pub limit: Option<u64>,
    /// Resume after the last record of the previous page (the `nextCursor` it reported)
    #[arg(long)]
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
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

impl PageMeta {
    /// The fields `meta` carries in JSON mode.
    pub fn insert_into(&self, meta: &mut serde_json::Map<String, Value>) {
        meta.insert("returned".to_string(), json!(self.returned));
        meta.insert("total".to_string(), json!(self.total));
        meta.insert("hasMore".to_string(), json!(self.has_more));
        meta.insert("nextCursor".to_string(), json!(self.next_cursor));
    }

    pub fn to_json(&self) -> Value {
        let mut meta = serde_json::Map::new();
        self.insert_into(&mut meta);
        Value::Object(meta)
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
    if cursor.is_empty() || cursor.len() % 2 != 0 || !cursor.is_ascii() {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..cursor.len()).step_by(2).map(|i| u8::from_str_radix(&cursor[i..i + 2], 16).ok()).collect();
    let text = String::from_utf8(bytes?).ok()?;
    text.split_once(SEPARATOR).map(|(scope, key)| (scope.to_string(), key.to_string()))
}

fn invalid_cursor(why: &str) -> CliError {
    CliError::refused("INVALID_CURSOR", why.to_string()).hint("drop --cursor to start from the first page, or pass the `nextCursor` this same command returned")
}

/// Cuts one page out of `items`, which must already be in their final,
/// deterministic order. `key` names one record uniquely (equal keys are told
/// apart by their order of appearance). `scope` identifies the listing — the
/// command and what it lists — so a cursor from another listing is refused.
pub fn paginate<T>(scope: &str, items: Vec<T>, key: impl Fn(&T) -> String, page: &Page) -> Result<(Vec<T>, PageMeta), CliError> {
    let total = items.len();
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let keys: Vec<String> = items
        .iter()
        .map(|item| {
            let base = key(item);
            let n = seen.entry(base.clone()).or_insert(0);
            *n += 1;
            format!("{base}#{n}")
        })
        .collect();

    let start = match &page.cursor {
        None => 0,
        Some(cursor) => {
            let (cursor_scope, last) = decode(cursor).ok_or_else(|| invalid_cursor("the cursor is not one canon issued"))?;
            if cursor_scope != scope {
                return Err(invalid_cursor(&format!("the cursor belongs to `{cursor_scope}`, not `{scope}`")));
            }
            let at = keys.iter().position(|k| *k == last).ok_or_else(|| invalid_cursor("the record the cursor points after is no longer in the result (the store or the filters changed)"))?;
            at + 1
        }
    };

    let end = page.limit.map_or(total, |limit| start.saturating_add(limit).min(total));
    let has_more = end < total;
    let next_cursor = has_more.then(|| encode(scope, &keys[end - 1]));
    let slice: Vec<T> = items.into_iter().skip(start).take(end - start).collect();
    let meta = PageMeta { returned: slice.len(), total, has_more, next_cursor };
    Ok((slice, meta))
}

/// Refuses (`UNKNOWN_FIELD`) any requested `--fields` name that `known` does not hold.
pub fn check_fields(command: &str, requested: &[String], known: &BTreeSet<String>) -> Result<(), CliError> {
    let unknown: Vec<&str> = requested.iter().map(String::as_str).filter(|f| !known.contains(*f)).collect();
    if unknown.is_empty() {
        return Ok(());
    }
    let list = known.iter().map(String::as_str).collect::<Vec<_>>().join(", ");
    Err(CliError::refused("UNKNOWN_FIELD", format!("`{command}` has no field {}", unknown.iter().map(|f| format!("`{f}`")).collect::<Vec<_>>().join(", "))).hint(format!("known fields: {list}")).field("--fields").expected(known.iter().cloned()))
}

/// [`check_fields`] against a fixed list of field names.
pub fn check_known(command: &str, requested: &[String], known: &[&str]) -> Result<(), CliError> {
    check_fields(command, requested, &known.iter().map(|f| f.to_string()).collect())
}

/// Keeps only `fields` of an object record; a field the record happens
/// not to carry is omitted.
pub fn project(record: &Value, fields: &[String]) -> Value {
    let Some(object) = record.as_object() else { return record.clone() };
    let mut out = serde_json::Map::new();
    for field in fields {
        if let Some(value) = object.get(field) {
            out.insert(field.clone(), value.clone());
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
            let (rows, meta) = paginate("t", items.clone(), |s| s.to_string(), &page(Some(2), cursor.clone())).unwrap();
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
        let (_, meta) = paginate("query:handoff", items.clone(), |s| s.to_string(), &page(Some(1), None)).unwrap();
        let cursor = meta.next_cursor.unwrap();
        for bad in [cursor.clone(), "zz".to_string(), String::new()] {
            let err = paginate("query:run", items.clone(), |s| s.to_string(), &page(Some(1), Some(bad))).unwrap_err();
            assert_eq!(err.code, "INVALID_CURSOR");
        }
    }
}
