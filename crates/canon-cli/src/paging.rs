//! Bounded listings (0.20 D4; the cursor is bound to its call since 0.34, atip/1 spec 6.9).
//!
//! `query`, `finding list`, `divergence list` and `export` return at most
//! `--limit` records (default [`DEFAULT_LIMIT`]); `--all` lifts the bound and
//! `--cursor` resumes after the last record of the previous page. The JSON
//! `meta.page` carries `returned`, `total` and, while more remain,
//! `nextCursor` (and `meta.next` the same call with it); human mode ends with
//! a line naming the next cursor when more remain. Field selection (`--fields`)
//! is [`crate::fields`].
//!
//! A cursor is opaque to the caller. It names the command (`scope`), a digest
//! of every argument of the call that issued it except `--limit`, `--all`,
//! `--cursor`, `--fields` and `--json` (the binding, [`bind_arguments`]) and the key of
//! the last record it returned. A call with another cursor is refused
//! (`CURSOR_INVALID`, exit 2, nothing written) with the reason that says why:
//! `FOREIGN` (not a cursor this command issued), `ARGUMENTS_CHANGED` (issued for
//! other arguments, so a changed filter never skips or repeats a record without
//! saying so) or `EXPIRED` (issued for these arguments, but the record it points
//! after is gone).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use clap::{Args, ArgMatches, Command};
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};

use crate::cli_error::{CliError, CliErrorExt as _};

/// Records per page when neither `--limit` nor `--all` is given. The `default_value` of `--limit` below says the same,
/// so `--help` and `canon schema` print it; `the_clap_default_of_limit_is_the_default_limit` holds the two together.
pub const DEFAULT_LIMIT: usize = 100;

const SEPARATOR: char = '\u{1f}';

/// The options that change how much of a listing one call returns, or how it is shown, never which records it lists: a
/// cursor is not bound to them. An argument equal to its default is not among them: `--repo .` and no `--repo` are two calls.
const PAGING_ARGS: [&str; 5] = ["limit", "all", "cursor", "fields", "json"];

/// The paging flags shared by every bounded listing.
#[derive(Args, Debug, Clone, Default)]
pub struct PageFlags {
    /// Return at most this many records per page
    #[arg(long, value_parser = crate::cli::safe_min1, conflicts_with = "all", value_name = "N", default_value = "100")]
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
        Page { limit, cursor: self.cursor.clone(), binding: bound_arguments().to_string() }
    }
}

static BINDING: OnceLock<String> = OnceLock::new();

/// Records the arguments of this run a cursor is bound to: the explicit arguments of the leaf command, each with its
/// values, except [`PAGING_ARGS`]. Called once by `main`, after the parse. Spelling, order and `--kind=x` against
/// `--kind x` do not matter: the digest is of what clap read.
pub fn bind_arguments(command_tree: &Command, parsed: &ArgMatches) {
    let mut leaf = parsed;
    let mut leaf_command = command_tree;
    while let Some((name, sub)) = leaf.subcommand() {
        let Some(found) = leaf_command.find_subcommand(name) else { break };
        leaf_command = found;
        leaf = sub;
    }
    let _ = BINDING.set(binding_of(leaf_command, leaf));
}

fn binding_of(command: &Command, matches: &ArgMatches) -> String {
    let mut given: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for arg in command.get_arguments() {
        let id = arg.get_id().as_str();
        if PAGING_ARGS.contains(&id) || !matches.try_contains_id(id).is_ok_and(|present| present) || matches.value_source(id) != Some(clap::parser::ValueSource::CommandLine) {
            continue;
        }
        let values = matches.get_raw(id).map(|raw| raw.map(|value| value.to_string_lossy().into_owned()).collect()).unwrap_or_default();
        given.insert(id.to_string(), values);
    }
    let canonical = given.iter().map(|(id, values)| format!("{id}={}", values.join("\u{1e}"))).collect::<Vec<_>>().join("\u{1f}");
    let digest = Sha256::digest(canonical.as_bytes());
    digest.iter().take(8).map(|byte| format!("{byte:02x}")).collect()
}

/// The binding recorded by [`bind_arguments`]; empty when none was (a test that builds a [`Page`] by hand).
fn bound_arguments() -> &'static str {
    BINDING.get().map_or("", String::as_str)
}

/// A resolved page request. `limit: None` is `--all`.
#[derive(Debug, Clone)]
pub struct Page {
    pub limit: Option<usize>,
    pub cursor: Option<String>,
    /// The digest of the call's arguments a cursor is bound to ([`bind_arguments`]).
    pub binding: String,
}

impl Page {
    pub fn first_page() -> Self {
        Page { limit: Some(DEFAULT_LIMIT), cursor: None, binding: String::new() }
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

fn encode(scope: &str, binding: &str, key: &str) -> String {
    format!("{scope}{SEPARATOR}{binding}{SEPARATOR}{key}").bytes().map(|b| format!("{b:02x}")).collect()
}

/// `(scope, binding, key)` of a cursor canon issued; `None` for anything else (a 0.33 cursor included: it has no binding).
fn decode(cursor: &str) -> Option<(String, String, String)> {
    if cursor.is_empty() || !cursor.len().is_multiple_of(2) || !cursor.is_ascii() {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..cursor.len()).step_by(2).map(|i| u8::from_str_radix(&cursor[i..i + 2], 16).ok()).collect();
    let text = String::from_utf8(bytes?).ok()?;
    let mut parts = text.splitn(3, SEPARATOR);
    Some((parts.next()?.to_string(), parts.next()?.to_string(), parts.next()?.to_string()))
}

fn invalid_cursor(reason: &'static str, why: &str, hint: &str) -> CliError {
    CliError::refused("CURSOR_INVALID", why.to_string()).reason(reason).field("--cursor").hint(hint.to_string())
}

/// Cuts one page out of `items`, which must already be in their final,
/// deterministic order. `key` names one record uniquely (equal keys are told
/// apart by their order of appearance). `scope` is the command, so a cursor of
/// another command is refused; `page.binding` is the call's arguments, so a
/// cursor of this command issued for other arguments is refused.
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
            const START_OVER: &str = "drop --cursor to start from the first page, or pass the `nextCursor` this same command returned";
            let (cursor_scope, binding, last) = decode(cursor).ok_or_else(|| invalid_cursor("FOREIGN", "the cursor is not one this command issued", START_OVER))?;
            if cursor_scope != scope {
                return Err(invalid_cursor("FOREIGN", &format!("the cursor was issued by `canon {cursor_scope}`, not `canon {scope}`"), START_OVER));
            }
            if binding != page.binding {
                return Err(invalid_cursor(
                    "ARGUMENTS_CHANGED",
                    "the cursor was issued for a call with other arguments: a cursor is bound to every argument except --limit, --all, --cursor, --fields and --json",
                    "repeat the call that issued the cursor and change only --limit, --all, --fields or --json, or drop --cursor to start from the first page",
                ));
            }
            let mut found = None;
            while found.is_none() && keys.len() < total {
                let next = keys.len() + 1;
                extend_to(next, &mut keys);
                if keys.last().is_some_and(|k| *k == last) {
                    found = Some(keys.len() - 1);
                }
            }
            found.ok_or_else(|| invalid_cursor("EXPIRED", "the record the cursor points after is no longer in the result (the store changed since it was issued)", "drop --cursor to start from the first page"))? + 1
        }
    };

    let end = page.limit.map_or(total, |limit| start.saturating_add(limit).min(total));
    let next_cursor = (end < total).then(|| {
        extend_to(end, &mut keys);
        encode(scope, &page.binding, &keys[end - 1])
    });
    let slice: Vec<T> = items.into_iter().skip(start).take(end - start).collect();
    let meta = PageMeta { returned: slice.len(), total, next_cursor };
    Ok((slice, meta))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(limit: Option<usize>, cursor: Option<String>) -> Page {
        Page { limit, cursor, binding: "b1".to_string() }
    }

    fn reason_of(error: &CliError) -> Option<&'static str> {
        error.reason_name()
    }

    #[test]
    fn the_clap_default_of_limit_is_the_default_limit() {
        use clap::{CommandFactory as _, FromArgMatches as _};
        #[derive(clap::Parser, Debug)]
        struct Probe {
            #[command(flatten)]
            page: PageFlags,
        }
        let matches = Probe::command().try_get_matches_from(["probe"]).expect("no argument parses");
        let probe = Probe::from_arg_matches(&matches).expect("the flags read");
        assert_eq!(probe.page.page().limit, Some(DEFAULT_LIMIT));
        assert_eq!(probe.page.limit, Some(DEFAULT_LIMIT as u64), "clap itself holds the default, so help and the schema print it");
        let all = Probe::command().try_get_matches_from(["probe", "--all"]).expect("--all alone parses: a default does not conflict with it");
        assert_eq!(Probe::from_arg_matches(&all).expect("the flags read").page.page().limit, None);
        assert!(Probe::command().try_get_matches_from(["probe", "--all", "--limit", "5"]).is_err(), "--all with an explicit --limit still conflicts");
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
    fn a_cursor_of_another_command_or_garbage_is_foreign() {
        let items = vec!["a", "b", "c"];
        let (_, meta) = paginate("query", items.clone(), std::string::ToString::to_string, &page(Some(1), None)).unwrap();
        let cursor = meta.next_cursor.unwrap();
        for bad in [cursor.clone(), "zz".to_string(), String::new(), "0102".to_string()] {
            let err = paginate("export", items.clone(), std::string::ToString::to_string, &page(Some(1), Some(bad))).unwrap_err();
            assert_eq!((err.code, reason_of(&err)), ("CURSOR_INVALID", Some("FOREIGN")));
            assert_eq!(err.field_name(), Some("--cursor"));
        }
    }

    #[test]
    fn a_cursor_of_this_command_with_other_arguments_is_arguments_changed_and_the_page_size_is_free() {
        let items = vec!["a", "b", "c", "d"];
        let (_, meta) = paginate("query", items.clone(), std::string::ToString::to_string, &page(Some(1), None)).unwrap();
        let cursor = meta.next_cursor.unwrap();
        let changed = Page { limit: Some(1), cursor: Some(cursor.clone()), binding: "b2".to_string() };
        let err = paginate("query", items.clone(), std::string::ToString::to_string, &changed).unwrap_err();
        assert_eq!((err.code, reason_of(&err)), ("CURSOR_INVALID", Some("ARGUMENTS_CHANGED")));
        let (rows, _) = paginate("query", items.clone(), std::string::ToString::to_string, &page(Some(2), Some(cursor.clone()))).unwrap();
        assert_eq!(rows, ["b", "c"], "--limit does not bind the cursor");
        let (rest, meta) = paginate("query", items, std::string::ToString::to_string, &page(None, Some(cursor))).unwrap();
        assert_eq!(rest, ["b", "c", "d"], "--all does not bind it either");
        assert!(meta.next_cursor.is_none());
    }

    #[test]
    fn a_cursor_whose_record_is_gone_is_expired() {
        let (_, meta) = paginate("query", vec!["a", "b", "c"], std::string::ToString::to_string, &page(Some(2), None)).unwrap();
        let cursor = meta.next_cursor.unwrap();
        let err = paginate("query", vec!["a", "c"], std::string::ToString::to_string, &page(Some(2), Some(cursor))).unwrap_err();
        assert_eq!((err.code, reason_of(&err)), ("CURSOR_INVALID", Some("EXPIRED")));
    }

    #[test]
    fn a_0_33_cursor_has_no_binding_and_is_foreign() {
        // `<scope> 0x1f <key>` in hex: the format before the binding.
        let old: String = format!("query:subject{SEPARATOR}a#1").bytes().map(|b| format!("{b:02x}")).collect();
        let err = paginate("query", vec!["a", "b"], std::string::ToString::to_string, &page(Some(1), Some(old))).unwrap_err();
        assert_eq!(reason_of(&err), Some("FOREIGN"));
    }

    #[test]
    fn the_binding_ignores_spelling_and_order_and_the_paging_options_and_nothing_else() {
        fn tree() -> Command {
            Command::new("t").subcommand(
                Command::new("list")
                    .arg(clap::Arg::new("kind").long("kind").value_name("K"))
                    .arg(clap::Arg::new("since").long("since").value_name("T"))
                    .arg(clap::Arg::new("json").long("json").action(clap::ArgAction::SetTrue))
                    .arg(clap::Arg::new("history").long("history").action(clap::ArgAction::SetTrue))
                    .arg(clap::Arg::new("repo").long("repo").value_name("D").default_value("."))
                    .arg(clap::Arg::new("limit").long("limit").value_name("N"))
                    .arg(clap::Arg::new("all").long("all").action(clap::ArgAction::SetTrue))
                    .arg(clap::Arg::new("cursor").long("cursor").value_name("C"))
                    .arg(clap::Arg::new("fields").long("fields").value_name("F")),
            )
        }
        let bind = |args: &[&str]| {
            let command = tree();
            let matches = command.clone().try_get_matches_from(std::iter::once("t").chain(std::iter::once("list")).chain(args.iter().copied())).unwrap();
            let (_, leaf) = matches.subcommand().unwrap();
            binding_of(command.find_subcommand("list").unwrap(), leaf)
        };
        let base = bind(&["--kind", "subject", "--since", "2030-01-01"]);
        assert_eq!(base, bind(&["--since", "2030-01-01", "--kind=subject"]), "order and spelling");
        assert_eq!(base, bind(&["--kind", "subject", "--since", "2030-01-01", "--limit", "3", "--cursor", "x", "--fields", "a"]), "paging options");
        assert_eq!(base, bind(&["--kind", "subject", "--since", "2030-01-01", "--all"]), "--all is the unbounded --limit");
        assert_ne!(base, bind(&["--kind", "handoff", "--since", "2030-01-01"]), "--kind");
        assert_ne!(base, bind(&["--kind", "subject"]), "a dropped filter");
        assert_eq!(base, bind(&["--kind", "subject", "--since", "2030-01-01", "--json"]), "the output mode does not change the result set");
        assert_ne!(base, bind(&["--kind", "subject", "--since", "2030-01-01", "--repo", "."]), "an explicit argument equal to its default is a change");
        assert_ne!(base, bind(&["--kind", "subject", "--since", "2030-01-01", "--history"]), "a flag");
    }
}
