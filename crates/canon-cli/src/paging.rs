//! Bounded listings (0.20 D4; the cursor is bound to its call since 0.34 and is `atip::cursor`'s since 0.35, atip/1 1.2 spec 6.9).
//!
//! `query`, `finding list`, `divergence list` and `export` return at most
//! `--limit` records (default [`DEFAULT_LIMIT`]); `--all` lifts the bound and
//! `--cursor` resumes after the last record of the previous page. The JSON
//! `meta.page` carries `returned`, `total` and, while more remain,
//! `nextCursor` (and `meta.next` the same call with it); human mode ends with
//! a line naming the next cursor when more remain. Field selection (`--fields`)
//! is [`crate::fields`].
//!
//! A cursor is opaque to the caller and the `atip` crate's to write and read ([`atip::cursor`]): it names the command
//! (`scope`), a digest of the arguments of the call that issued it (the binding) and the key of the last record it
//! returned. canon hands the crate the arguments the call gave ([`bind_arguments`]): every explicit argument (one given with
//! its default value included), the value `CANON_ROLE` supplied for `--as`, and canon's own flag-only options
//! (`--repo`, `--as`); the crate leaves out `--json`, `--limit`, `--all`, `--cursor` and `--fields`. A call with another
//! cursor is refused (`CURSOR_INVALID`, exit 2, nothing written) with the reason that says why: `FOREIGN` (not a cursor this
//! command issued), `ARGUMENTS_CHANGED` (issued for other arguments or another environment, so a changed filter never skips
//! or repeats a record without saying so) or `EXPIRED` (issued for these arguments, but the record it points after is gone).
//! The next page's `meta.next` entry is written by the crate from the same arguments ([`next_page_argv`]).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use atip::{Binding, CursorInvalid, Cursors, Given, Minor};
use clap::parser::ValueSource;
use clap::{Arg, ArgAction, ArgMatches, Args, Command};
use serde_json::{json, Value};

use crate::cli_error::{CliError, CliErrorExt as _};

/// Records per page when neither `--limit` nor `--all` is given. The `default_value` of `--limit` below says the same,
/// so `--help` and `canon schema` print it; `the_clap_default_of_limit_is_the_default_limit` holds the two together.
pub const DEFAULT_LIMIT: usize = 100;

/// The cursors canon writes and reads: the binding rules of atip/1 1.2, whichever minor `meta.contractVersion` declares.
const CURSORS: Cursors = Cursors::new(Minor::V1_2);

/// The paging flags shared by every bounded listing.
#[derive(Args, Debug, Clone, Default)]
pub struct PageFlags {
    /// Return at most this many records per page
    #[arg(long, value_parser = crate::cli::safe_min1, value_name = "N", default_value = "100")]
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
        Page { limit, cursor: self.cursor.clone(), binding: bound_arguments() }
    }
}

/// The call of this run as the cursor sees it.
struct Call {
    /// The command words of the leaf (`["finding", "list"]`).
    path: Vec<String>,
    /// The explicit arguments of the call, the environment values that filled an option, the operands.
    given: Vec<Given>,
    binding: Result<Binding, CliError>,
}

static CALL: OnceLock<Call> = OnceLock::new();

/// Records the call of this run: the arguments a cursor is bound to and the next page is written from. Called once by
/// `main`, after the parse (`--input` is already expanded into ordinary arguments, so a document and the command line
/// say the same).
pub fn bind_arguments(command_tree: &Command, parsed: &ArgMatches) {
    let mut leaf = parsed;
    let mut leaf_command = command_tree;
    let mut path = Vec::new();
    while let Some((name, sub)) = leaf.subcommand() {
        let Some(found) = leaf_command.find_subcommand(name) else { break };
        path.push(name.to_string());
        leaf_command = found;
        leaf = sub;
    }
    let given = given_of(leaf_command, leaf);
    let binding = binding_of(&given);
    let _ = CALL.set(Call { path, given, binding });
}

fn binding_of(given: &[Given]) -> Result<Binding, CliError> {
    CURSORS.binding(given).map_err(|error| CliError::failed("INTERNAL", format!("canon could not bind the cursor to the call: {error}")).hint("report it with the command line you ran"))
}

/// The binding recorded by [`bind_arguments`]; that of a call with no arguments when none was (a test that builds a [`Page`] by hand).
fn bound_arguments() -> Result<Binding, CliError> {
    CALL.get().map_or_else(|| binding_of(&[]), |call| call.binding.clone())
}

/// The argv of the call that continues the page after `cursor`: the explicit arguments of this call, normalised and written
/// as the contract says (`--name=value`), then `--cursor=<cursor>`, and `--json` in JSON mode. A default, a value an
/// environment variable supplied and `--input` are not carried.
pub fn next_page_argv(cursor: &str) -> Option<Vec<String>> {
    let call = CALL.get()?;
    let path: Vec<&str> = call.path.iter().map(String::as_str).collect();
    Some(atip::next_page_argv(&path, &call.given, cursor, atip::json_mode()))
}

/// The arguments the call gave, normalised: an integer option as a JSON integer, a switch as `true`, a repeated option as
/// an array, the operands as one list. A value filled in from a default is not among them; one filled in by an environment
/// variable is, marked as such. `--repo` and `--as` are marked as canon's own flag-only options.
fn given_of(command: &Command, matches: &ArgMatches) -> Vec<Given> {
    let mut given = Vec::new();
    let mut operands: Vec<Value> = Vec::new();
    for arg in command.get_arguments() {
        if matches!(arg.get_action(), ArgAction::Help | ArgAction::HelpShort | ArgAction::HelpLong | ArgAction::Version) {
            continue;
        }
        let id = arg.get_id().as_str();
        if !matches.try_contains_id(id).is_ok_and(|present| present) {
            continue;
        }
        let from_env = match matches.value_source(id) {
            Some(ValueSource::CommandLine) => false,
            Some(ValueSource::EnvVariable) => true,
            _ => continue,
        };
        let raw: Vec<String> = matches.get_raw(id).map(|values| values.map(|value| value.to_string_lossy().into_owned()).collect()).unwrap_or_default();
        if arg.is_positional() {
            operands.extend(raw.iter().map(|text| value_of(arg, text)));
            continue;
        }
        let name = atip::constraint::name_of(arg);
        let one = match arg.get_action() {
            ArgAction::SetTrue | ArgAction::SetFalse => Given::switch(&name, true),
            ArgAction::Count => Given::count(&name, u64::from(matches.get_count(id))),
            ArgAction::Append => Given::repeated(&name, raw.iter().map(|text| value_of(arg, text)).collect()),
            _ => match raw.first() {
                Some(first) => Given::option(&name, value_of(arg, first)),
                None => continue,
            },
        };
        let one = if crate::input::OWN_FLAGS_ONLY.contains(&name.as_str()) { one.tool_flag_only() } else { one };
        given.push(if from_env { one.from_env() } else { one });
    }
    // `CANON_ROLE` fills `--as` when argv does not (it is not a clap `env`: `canon schema` never reads it), and binds like any `env` value.
    if !given.iter().any(|argument| argument.name == "as") {
        if let Some(value) = std::env::var(crate::role::ENV).ok().map(|value| value.trim().to_string()).filter(|value| !value.is_empty()) {
            let role = crate::role::Role::parse(&value).map_or(value, |role| role.as_str().to_string());
            given.push(Given::option("as", role).tool_flag_only().from_env());
        }
    }
    if !operands.is_empty() {
        given.push(Given::operands(operands));
    }
    given
}

/// The value of `text` for `arg`: an integer option's as a JSON integer within the contract's range, anything else as text.
fn value_of(arg: &Arg, text: &str) -> Value {
    if atip::project::is_integer_arg(arg) {
        if let Some(number) = text.strip_prefix('+').unwrap_or(text).parse::<i64>().ok().filter(|number| number.unsigned_abs() <= crate::cli::SAFE_INTEGER) {
            return json!(number);
        }
    }
    Value::String(text.to_string())
}

/// A resolved page request. `limit: None` is `--all`.
#[derive(Debug, Clone)]
pub struct Page {
    pub limit: Option<usize>,
    pub cursor: Option<String>,
    /// The digest of the call's arguments a cursor is bound to ([`bind_arguments`]).
    pub binding: Result<Binding, CliError>,
}

impl Page {
    pub fn first_page() -> Self {
        Page { limit: Some(DEFAULT_LIMIT), cursor: None, binding: binding_of(&[]) }
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

/// The refusal of a cursor: `CURSOR_INVALID` with the reason and the fix the crate gives ([`atip::CURSOR_INVALID_REASONS`]), exit 2,
/// `field` `--cursor`, built on canon's row of the code so the exit and `retryable` are the registry's.
fn invalid_cursor(refused: CursorInvalid) -> CliError {
    let reason = refused.reason().as_str();
    let fix = atip::CURSOR_INVALID_REASONS.iter().find(|row| row.reason == reason).map_or("", |row| row.fix);
    CliError::refused("CURSOR_INVALID", refused.to_string()).reason(reason).field("--cursor").hint(fix)
}

/// Cuts one page out of `items`, which must already be in their final,
/// deterministic order. `key` names one record uniquely (equal keys are told
/// apart by their order of appearance). `scope` is the command, so a cursor of
/// another command is refused; `page.binding` is the call's arguments, so a
/// cursor of this command issued for other arguments is refused.
pub fn paginate<T>(scope: &str, items: Vec<T>, key: impl Fn(&T) -> String, page: &Page) -> Result<(Vec<T>, PageMeta), CliError> {
    let binding = page.binding.as_ref().map_err(Clone::clone)?;
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
            let last = CURSORS.decode(cursor, scope, binding).map_err(invalid_cursor)?;
            let mut found = None;
            while found.is_none() && keys.len() < total {
                let next = keys.len() + 1;
                extend_to(next, &mut keys);
                if keys.last().is_some_and(|k| *k == last) {
                    found = Some(keys.len() - 1);
                }
            }
            found.ok_or_else(|| invalid_cursor(CursorInvalid::expired()))? + 1
        }
    };

    let end = page.limit.map_or(total, |limit| start.saturating_add(limit).min(total));
    let next_cursor = (end < total).then(|| {
        extend_to(end, &mut keys);
        CURSORS.encode(scope, binding, &keys[end - 1])
    });
    let slice: Vec<T> = items.into_iter().skip(start).take(end - start).collect();
    let meta = PageMeta { returned: slice.len(), total, next_cursor };
    Ok((slice, meta))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bound(arguments: &[Given]) -> Result<Binding, CliError> {
        binding_of(arguments)
    }

    fn page(limit: Option<usize>, cursor: Option<String>) -> Page {
        Page { limit, cursor, binding: bound(&[Given::option("kind", "a")]) }
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
        // The exclusion of `--all` and an explicit `--limit` is a rule of the command's tables (`arg_rules::rules_of`), refused by the stateless stage
        // after the parse: clap alone parses the pair (spec 11.7 step 6 answers a failure to parse, then `required`, then domains, then constraints).
        Probe::command().try_get_matches_from(["probe", "--all", "--limit", "5"]).expect("clap leaves the exclusion to the tables");
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
        let mut changed = cursor.clone().into_bytes();
        let at = changed.len() / 2;
        changed[at] = if changed[at] == b'A' { b'B' } else { b'A' };
        for bad in [cursor.clone(), "zz".to_string(), String::new(), "0102".to_string(), String::from_utf8(changed).unwrap()] {
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
        let changed = Page { limit: Some(1), cursor: Some(cursor.clone()), binding: bound(&[Given::option("kind", "b")]) };
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
    fn a_0_34_cursor_is_foreign() {
        // `<scope> 0x1f <binding> 0x1f <key>` in hex: the format of 0.34 (the binding then was 8 bytes of a hash).
        let old: String = "query\u{1f}0123456789abcdef\u{1f}a#1".bytes().map(|b| format!("{b:02x}")).collect();
        let err = paginate("query", vec!["a", "b"], std::string::ToString::to_string, &page(Some(1), Some(old))).unwrap_err();
        assert_eq!(reason_of(&err), Some("FOREIGN"));
    }

    #[test]
    fn the_cursor_does_not_show_the_key_of_the_record_it_follows() {
        let (_, meta) = paginate("query", vec!["record-key-0001", "record-key-0002"], std::string::ToString::to_string, &page(Some(1), None)).unwrap();
        let cursor = meta.next_cursor.unwrap();
        assert!(!cursor.contains("record-key-0001"), "{cursor}");
        let hex: String = "record-key-0001".bytes().map(|b| format!("{b:02x}")).collect();
        assert!(!cursor.contains(&hex), "{cursor}");
    }

    /// The `Given`s of `canon list <args>` on a tree like a listing's, as `bind_arguments` reads them.
    fn given(args: &[&str]) -> Vec<Given> {
        let command = Command::new("t").subcommand(
            Command::new("list")
                .arg(clap::Arg::new("kind").long("kind").value_name("K"))
                .arg(clap::Arg::new("since").long("since").value_name("T"))
                .arg(clap::Arg::new("round").long("round").value_parser(clap::value_parser!(u32)))
                .arg(clap::Arg::new("json").long("json").action(ArgAction::SetTrue))
                .arg(clap::Arg::new("history").long("history").action(ArgAction::SetTrue))
                .arg(clap::Arg::new("repo").long("repo").value_name("D").default_value("."))
                .arg(clap::Arg::new("limit").long("limit").value_name("N").default_value("100"))
                .arg(clap::Arg::new("all").long("all").action(ArgAction::SetTrue))
                .arg(clap::Arg::new("cursor").long("cursor").value_name("C"))
                .arg(clap::Arg::new("fields").long("fields").value_name("F")),
        );
        let matches = command.clone().try_get_matches_from(["t", "list"].into_iter().chain(args.iter().copied())).unwrap();
        let (_, leaf) = matches.subcommand().unwrap();
        given_of(command.find_subcommand("list").unwrap(), leaf)
    }

    fn binds(args: &[&str]) -> Binding {
        CURSORS.binding(&given(args)).unwrap()
    }

    #[test]
    fn the_binding_ignores_spelling_and_order_and_the_paging_options_and_nothing_else() {
        let base = binds(&["--kind", "subject", "--since", "2030-01-01"]);
        assert_eq!(base, binds(&["--since", "2030-01-01", "--kind=subject"]), "order and spelling");
        assert_eq!(base, binds(&["--kind", "subject", "--since", "2030-01-01", "--limit", "3", "--cursor", "x", "--fields", "a"]), "paging options, an explicit default `--limit` included");
        assert_eq!(base, binds(&["--kind", "subject", "--since", "2030-01-01", "--all"]), "--all is the unbounded --limit");
        assert_ne!(base, binds(&["--kind", "handoff", "--since", "2030-01-01"]), "--kind");
        assert_ne!(base, binds(&["--kind", "subject"]), "a dropped filter");
        assert_eq!(base, binds(&["--kind", "subject", "--since", "2030-01-01", "--json"]), "the output mode does not change the result set");
        assert_ne!(base, binds(&["--kind", "subject", "--since", "2030-01-01", "--repo", "."]), "an explicit argument equal to its default is a change");
        assert_ne!(base, binds(&["--kind", "subject", "--since", "2030-01-01", "--history"]), "a flag");
        assert_eq!(binds(&["--round", "1"]), binds(&["--round=01"]), "an integer is the number, however it was spelled");
    }

    #[test]
    fn the_next_page_carries_only_the_explicit_arguments_normalised_and_never_a_default() {
        let argv = atip::next_page_argv(&["list"], &given(&["--kind", "subject", "--limit", "1", "--json"]), "atip.cursor", true);
        assert_eq!(argv, ["list", "--kind=subject", "--limit=1", "--cursor=atip.cursor", "--json"]);
        let argv = atip::next_page_argv(&["list"], &given(&["--kind", "subject"]), "atip.cursor", false);
        assert_eq!(argv, ["list", "--kind=subject", "--cursor=atip.cursor"], "`--repo` and `--limit` are defaults here: not carried");
    }
}
