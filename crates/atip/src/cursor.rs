//! Paging cursors bound to the call that issued them (spec 6.9, 15.10; approaches/typescript.md 3.8).
//!
//! A cursor is one opaque ASCII string. It carries the *scope* (the command that issued it), the *binding* (a digest of the
//! arguments of the call that issued it) and the tool's own *position* in the result, and it refuses to be resumed by another
//! command or with other arguments: `CURSOR_INVALID`, with the reason `FOREIGN` or `ARGUMENTS_CHANGED`, so changing a filter
//! can never skip or repeat a record without saying so. `EXPIRED` is the tool's to say when it cannot resume the position
//! ([`CursorInvalid::expired`]).
//!
//! Which arguments bind is the contract's, by the minor a tool declares ([`Cursors`]): a tool that declares 1.1 binds to
//! every explicit argument except `--limit`, `--cursor` and `--fields`; one that declares 1.2 binds to every explicit
//! argument except `--json`, `--limit`, `--all`, `--cursor` and `--fields`, counts an argument given with its default value
//! as given, and binds the values an option's `env` variable supplied too, marked as environment. The binding is the
//! SHA-256 of the RFC 8785 text ([`crate::jcs`]) of those arguments, so the same arguments give the same binding in every tool
//! and however they were spelled. [`next_page_argv`] writes the call that continues a page from the explicit arguments.
//!
//! The text of a cursor is the crate's to change between releases; a caller never reads it (spec 6.9), and a tool never
//! parses one it did not get from [`Cursors::encode`].

use std::fmt;

use serde_json::{json, Map};

use crate::error::{CliError, ReasonInfo};
use crate::given::{write_argv, Given, Source};
use crate::jcs;
use crate::minor::{Minor, UnknownMinor};
use crate::project::flag_only_options;

/// The first word of every cursor this crate writes.
const PREFIX: &str = "atip";

/// The arguments left out of the binding by a tool that declares 1.1 (and 1.0): what chooses how much of the answer to
/// print. `input` is left out in every minor: it only carries arguments that are already in the list.
const LEFT_OUT_11: [&str; 4] = ["limit", "cursor", "fields", "input"];
/// The same for a tool that declares 1.2: the reserved options that never change the result set.
const LEFT_OUT_12: [&str; 6] = ["json", "limit", "all", "cursor", "fields", "input"];

/// The cursors of a tool that declares one minor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursors {
    /// The minor whose binding rules apply: 1.1 for a tool that declares 1.0 or 1.1, else 1.2.
    rules: Minor,
}

/// The digest of the arguments a cursor is bound to: 64 lower-case hex digits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding(String);

impl Binding {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Binding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Cursors {
    /// The cursors of a tool that declares `declared`. 1.0 has no rule of its own for cursors and follows 1.1's.
    pub const fn new(declared: Minor) -> Self {
        Cursors { rules: if declared.at_least(Minor::V1_2) { Minor::V1_2 } else { Minor::V1_1 } }
    }

    /// The cursors of a tool that declares the minor written `declared`, or [`UnknownMinor`] for any other text.
    pub fn for_declared(declared: &str) -> Result<Self, UnknownMinor> {
        Minor::parse(declared).map(Cursors::new)
    }

    /// The binding of an invocation whose arguments are `given`: the explicit arguments the rules of the minor name, the
    /// tool's flag-only options apart from them, and, for 1.2, the environment values. A switch that is `false`, a count of
    /// `0` and an empty list are not given (spec 10.7.2), and an argument given twice counts once, the last. An integer
    /// that is not within ±(2^53-1), or a number that is not an integer, is an error (see [`crate::jcs`]).
    pub fn binding(&self, given: &[Given]) -> Result<Binding, jcs::Error> {
        let twelve = self.rules == Minor::V1_2;
        let left_out: &[&str] = if twelve { &LEFT_OUT_12 } else { &LEFT_OUT_11 };
        let (mut arguments, mut flag_only, mut environment) = (Map::new(), Map::new(), Map::new());
        for argument in given {
            if left_out.contains(&argument.name.as_str()) || argument.is_absent() {
                continue;
            }
            match argument.source {
                // A 1.1 tool has no environment source: a value from an `env` variable is not part of its binding.
                Source::Env if !twelve => {}
                Source::Env => {
                    environment.insert(argument.name.clone(), argument.value.clone());
                }
                Source::Explicit if argument.flag_only || flag_only_options(self.rules).contains(&argument.name.as_str()) => {
                    flag_only.insert(argument.name.clone(), argument.value.clone());
                }
                Source::Explicit => {
                    arguments.insert(argument.name.clone(), argument.value.clone());
                }
            }
        }
        let document = json!({ "rules": self.rules.as_str(), "arguments": arguments, "flagOnly": flag_only, "environment": environment });
        Ok(Binding(jcs::sha256_hex(jcs::to_string(&document)?.as_bytes())))
    }

    /// The cursor of the position `position` that `scope` (the command id) reached in a call bound as `binding`: one ASCII
    /// string without whitespace or quotes, so it can be an argv word as it is.
    pub fn encode(&self, scope: &str, binding: &Binding, position: &str) -> String {
        let body = format!("{PREFIX}.{}.{}.{}", base64(scope), binding.0, base64(position));
        let check = checksum(&body);
        format!("{body}.{check}")
    }

    /// The position `text` points at, when `text` is a cursor `scope` issued for a call bound as `binding`. Otherwise the
    /// refusal: `FOREIGN` for a text that is not a cursor of this crate (any change to a cursor included) or was issued by
    /// another command, `ARGUMENTS_CHANGED` for one issued for other arguments.
    pub fn decode(&self, text: &str, scope: &str, binding: &Binding) -> Result<String, CursorInvalid> {
        let foreign = CursorInvalid::foreign();
        let (body, check) = text.rsplit_once('.').ok_or(foreign)?;
        if checksum(body) != check {
            return Err(foreign);
        }
        let parts: Vec<&str> = body.split('.').collect();
        let [prefix, issued_by, bound_to, position] = parts[..] else { return Err(foreign) };
        if prefix != PREFIX || bound_to.len() != 64 || !bound_to.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
            return Err(foreign);
        }
        if unbase64(issued_by).ok_or(foreign)? != scope {
            return Err(foreign);
        }
        let position = unbase64(position).ok_or(foreign)?;
        if bound_to != binding.0 {
            return Err(CursorInvalid::arguments_changed());
        }
        Ok(position)
    }
}

/// The argv of the call that continues a page (spec 6.9, 6.8): `path` and the explicit arguments of the call that issued
/// `cursor`, normalised, then `--cursor <cursor>` (replacing any `--cursor` among `given`), then `--json` when the call ran
/// in JSON mode and has not got it already, then `--` and the operands. A default, an `env` value and `--input` are not
/// carried (see [`write_argv`]), so the same call in the same environment binds the same.
pub fn next_page_argv(path: &[&str], given: &[Given], cursor: &str, json_mode: bool) -> Vec<String> {
    let mut arguments: Vec<Given> = given.iter().filter(|argument| argument.name != "cursor" && argument.name != "json").cloned().collect();
    arguments.push(Given::option("cursor", cursor));
    if json_mode {
        arguments.push(Given::switch("json", true));
    }
    write_argv(path, &arguments)
}

/// Why a cursor was refused (spec 7.2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// The cursor was not issued by this command, or is not a cursor.
    Foreign,
    /// The cursor was issued for other arguments.
    ArgumentsChanged,
    /// This command issued the cursor for these arguments but the position it points at can no longer be resumed.
    Expired,
}

impl Reason {
    /// The `reason` of the error.
    pub const fn as_str(self) -> &'static str {
        match self {
            Reason::Foreign => "FOREIGN",
            Reason::ArgumentsChanged => "ARGUMENTS_CHANGED",
            Reason::Expired => "EXPIRED",
        }
    }
}

/// A refused cursor: `CURSOR_INVALID` with its reason (exit 2, not retryable, `field` `--cursor`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorInvalid {
    reason: Reason,
}

impl CursorInvalid {
    /// The cursor is not one this command issued, or is not a cursor.
    pub const fn foreign() -> Self {
        CursorInvalid { reason: Reason::Foreign }
    }

    /// The cursor was issued for other arguments.
    pub const fn arguments_changed() -> Self {
        CursorInvalid { reason: Reason::ArgumentsChanged }
    }

    /// The tool cannot resume the position (the record it points after is gone): the crate cannot know, so the tool says it.
    pub const fn expired() -> Self {
        CursorInvalid { reason: Reason::Expired }
    }

    pub const fn reason(self) -> Reason {
        self.reason
    }
}

impl fmt::Display for CursorInvalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.reason {
            Reason::Foreign => "the cursor is not one this command issued",
            Reason::ArgumentsChanged => "the cursor was issued for other arguments",
            Reason::Expired => "the position the cursor points at can no longer be resumed",
        })
    }
}

impl std::error::Error for CursorInvalid {}

impl From<CursorInvalid> for CliError {
    fn from(refused: CursorInvalid) -> Self {
        let fix = CURSOR_INVALID_REASONS.iter().find(|row| row.reason == refused.reason.as_str()).map_or("", |row| row.fix);
        CliError::from_row("CURSOR_INVALID", refused.to_string(), None).field("--cursor").reason(refused.reason.as_str()).hint(fix)
    }
}

/// The reasons of `CURSOR_INVALID` for a tool's code row, sorted by name (spec 7.2.3, 7.6).
pub const CURSOR_INVALID_REASONS: [ReasonInfo; 3] = [
    ReasonInfo {
        reason: "ARGUMENTS_CHANGED",
        meaning: "the cursor was issued for other arguments: a cursor is bound to every argument of the call that issued it except the options that never change the result set",
        fix: "Repeat the call with the arguments that issued the cursor, or restart without --cursor.",
    },
    ReasonInfo { reason: "EXPIRED", meaning: "this command issued the cursor for these arguments but the position it points at can no longer be resumed", fix: "Restart the listing without --cursor." },
    ReasonInfo { reason: "FOREIGN", meaning: "the cursor is not one this command issued, or is not a cursor", fix: "Pass the nextCursor this same command returned, or restart without --cursor." },
];

/// The first 16 hex digits of the SHA-256 of `body`: any change to a cursor is found before it is read.
fn checksum(body: &str) -> String {
    jcs::sha256_hex(body.as_bytes()).chars().take(16).collect()
}

const ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// `text` in base64url without padding.
fn base64(text: &str) -> String {
    let alphabet = ALPHABET.as_bytes();
    let sextet = |n: u32, shift: u32| char::from(alphabet.get(usize::try_from((n >> shift) & 63).unwrap_or(0)).copied().unwrap_or(b'A'));
    let mut out = String::new();
    for chunk in text.as_bytes().chunks(3) {
        let byte = |at: usize| chunk.get(at).copied().map_or(0, u32::from);
        let n = (byte(0) << 16) | (byte(1) << 8) | byte(2);
        out.push(sextet(n, 18));
        out.push(sextet(n, 12));
        if chunk.len() > 1 {
            out.push(sextet(n, 6));
        }
        if chunk.len() > 2 {
            out.push(sextet(n, 0));
        }
    }
    out
}

/// The text `base64` wrote, or `None` for anything else, including bytes that are not UTF-8.
fn unbase64(text: &str) -> Option<String> {
    let mut values: Vec<u32> = Vec::new();
    for c in text.chars() {
        values.push(u32::try_from(ALPHABET.find(c)?).ok()?);
    }
    if values.len() % 4 == 1 {
        return None;
    }
    let mut bytes: Vec<u8> = Vec::new();
    for chunk in values.chunks(4) {
        let value = |at: usize| chunk.get(at).copied().unwrap_or(0);
        let n = (value(0) << 18) | (value(1) << 12) | (value(2) << 6) | value(3);
        bytes.push(u8::try_from((n >> 16) & 0xff).ok()?);
        if chunk.len() > 2 {
            bytes.push(u8::try_from((n >> 8) & 0xff).ok()?);
        }
        if chunk.len() > 3 {
            bytes.push(u8::try_from(n & 0xff).ok()?);
        }
    }
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;
    use crate::given::Kind;

    fn cursors(minor: &str) -> Cursors {
        Cursors::for_declared(minor).expect("a known minor")
    }

    fn bind(minor: &str, given: &[Given]) -> Binding {
        cursors(minor).binding(given).expect("a binding")
    }

    /// `acme member list --status active`.
    fn list() -> Vec<Given> {
        vec![Given::option("status", "active")]
    }

    #[test]
    fn atip_cratecursor_01_the_same_arguments_bind_the_same_whatever_their_order_or_spelling() {
        for minor in ["1.1", "1.2"] {
            let ordered = vec![Given::option("status", "active"), Given::option("round", 1), Given::repeated("label", vec![json!("a"), json!("b")])];
            let shuffled = vec![Given::repeated("label", vec![json!("a"), json!("b")]), Given::option("round", 1), Given::option("status", "active")];
            assert_eq!(bind(minor, &ordered), bind(minor, &shuffled), "{minor}: the order of the arguments does not matter");
            // `--round=1` and `--round 1` are the integer 1, and `1` and `1.0` are one number.
            assert_eq!(bind(minor, &[Given::option("round", 1)]), bind(minor, &[Given::option("round", 1.0)]), "{minor}");
            assert_eq!(bind(minor, &[Given::option("round", 1)]).as_str().len(), 64);
            // The order of a repeated option's values is its meaning.
            let reversed = vec![Given::option("status", "active"), Given::option("round", 1), Given::repeated("label", vec![json!("b"), json!("a")])];
            assert_ne!(bind(minor, &ordered), bind(minor, &reversed), "{minor}");
            // Another value, another option and another source are other arguments.
            assert_ne!(bind(minor, &list()), bind(minor, &[Given::option("status", "archived")]), "{minor}");
            assert_ne!(bind(minor, &list()), bind(minor, &[Given::option("state", "active")]), "{minor}");
            assert_eq!(bind(minor, &list()), bind(minor, &list()), "{minor}");
        }
        // The binding is the SHA-256 of the RFC 8785 text, so another tool computes the same one: pin one value.
        let document = r#"{"arguments":{"status":"active"},"environment":{},"flagOnly":{},"rules":"1.2"}"#;
        let expected = jcs::sha256_hex(document.as_bytes());
        assert_eq!(bind("1.2", &list()).as_str(), expected);
    }

    #[test]
    fn atip_cratecursor_02_a_1_2_binding_leaves_out_only_the_options_that_never_change_the_result() {
        let base = list();
        let with = |extra: Given| {
            let mut given = base.clone();
            given.push(extra);
            bind("1.2", &given)
        };
        let plain = bind("1.2", &base);
        for no_effect in [Given::switch("json", true).tool_flag_only(), Given::switch("json", true), Given::option("limit", 5), Given::switch("all", true), Given::option("cursor", "atip.x"), Given::option("fields", "a,b"), Given::option("input", "-")] {
            assert_eq!(with(no_effect.clone()), plain, "{} does not bind in 1.2", no_effect.name);
        }
        // An argument given with its default value is given, and so is a flag-only option of the tool's own.
        assert_ne!(with(Given::option("role", "member")), plain, "a default given explicitly binds");
        assert_ne!(with(Given::option("repo", ".").tool_flag_only()), plain, "the tool's own flag-only option binds");
        assert_ne!(with(Given::option("repo", ".").tool_flag_only()), with(Given::option("repo", "other").tool_flag_only()));
        // A switch given `false` and a count given `0` are not given.
        assert_eq!(with(Given::switch("verbose", false)), plain);
        assert_eq!(with(Given::count("level", 0)), plain);
        assert_ne!(with(Given::count("level", 1)), plain);
        assert_ne!(with(Given::count("level", 1)), with(Given::count("level", 2)));
        // The environment: setting, changing and giving the option explicitly instead each change the arguments.
        let from_env = with(Given::option("org", "eng").from_env());
        assert_ne!(from_env, plain, "setting the variable binds");
        assert_ne!(from_env, with(Given::option("org", "ops").from_env()), "changing it binds");
        assert_ne!(from_env, with(Given::option("org", "eng")), "giving the option explicitly instead binds");
        assert_ne!(with(Given::option("org", "eng")), plain);
    }

    #[test]
    fn atip_cratecursor_03_a_1_1_binding_binds_all_and_json_and_ignores_the_environment() {
        let base = list();
        let with = |extra: Given| {
            let mut given = base.clone();
            given.push(extra);
            bind("1.1", &given)
        };
        let plain = bind("1.1", &base);
        for no_effect in [Given::option("limit", 5), Given::option("cursor", "atip.x"), Given::option("fields", "a"), Given::option("input", "-"), Given::switch("verbose", false)] {
            assert_eq!(with(no_effect.clone()), plain, "{} does not bind in 1.1", no_effect.name);
        }
        assert_ne!(with(Given::switch("all", true)), plain, "`--all` binds in 1.1");
        assert_ne!(with(Given::switch("json", true)), plain, "`--json` binds in 1.1");
        assert_ne!(with(Given::option("role", "member")), plain);
        assert_eq!(with(Given::option("org", "eng").from_env()), plain, "1.1 has no environment source");
        // 1.0 follows 1.1.
        assert_eq!(Cursors::new(Minor::V1_0), Cursors::new(Minor::V1_1));
        assert_eq!(bind("1.0", &list()), bind("1.1", &list()));
        // The two rules never give one binding to one invocation: a cursor of one is not valid for the other.
        assert_ne!(bind("1.1", &list()), bind("1.2", &list()));
    }

    #[test]
    fn atip_cratecursor_04_an_unknown_minor_and_an_integer_a_digest_cannot_hold_are_refused() {
        for text in ["1.3", "2.0", "1", "", "1.2.0"] {
            assert_eq!(Cursors::for_declared(text), Err(UnknownMinor(text.to_string())), "{text:?}");
        }
        for number in [json!(9_007_199_254_740_993_u64), json!(-9_007_199_254_740_993_i64), json!(18_446_744_073_709_551_615_u64)] {
            let given = vec![Given::option("n", number.clone())];
            for minor in ["1.1", "1.2"] {
                assert!(matches!(cursors(minor).binding(&given), Err(jcs::Error::NumberNotSafe(_))), "{minor}: {number}");
            }
        }
        // The largest safe integer is fine, and a number written with a fraction is a double (RFC 8785), so it binds.
        cursors("1.2").binding(&[Given::option("n", 1.5)]).expect("a fraction binds");
        cursors("1.2").binding(&[Given::option("n", 9_007_199_254_740_991_i64)]).expect("the largest safe integer binds");
    }

    #[test]
    fn atip_cratecursor_05_a_cursor_decodes_to_its_scope_binding_and_position_and_is_plain_ascii() {
        let binding = bind("1.2", &list());
        for (scope, position) in [("member list", "key-1"), ("", ""), ("a.b", "a.b.c"), ("x y", "p q\"r'\n\t"), ("é", "日本語😀"), ("finding list", &"z".repeat(500))] {
            let text = cursors("1.2").encode(scope, &binding, position);
            assert!(text.is_ascii() && !text.chars().any(|c| c.is_whitespace() || c == '"' || c == '\'' || c == '\\'), "{text:?}");
            assert_eq!(cursors("1.2").decode(&text, scope, &binding), Ok(position.to_string()), "{scope:?} {position:?}");
            // Without a quote, so it is an argv word as it is.
            assert_eq!(crate::quote(&text), text, "a cursor needs no quoting");
        }
    }

    #[test]
    fn atip_cratecursor_06_changing_one_character_or_giving_any_text_is_foreign() {
        let binding = bind("1.2", &list());
        let text = cursors("1.2").encode("member list", &binding, "key-1");
        let codec = cursors("1.2");
        for at in 0..text.len() {
            for replacement in ['a', 'Z', '0', '-', '_', '.', 'x'] {
                let mut changed: Vec<char> = text.chars().collect();
                if changed[at] == replacement {
                    continue;
                }
                changed[at] = replacement;
                let changed: String = changed.into_iter().collect();
                assert_eq!(codec.decode(&changed, "member list", &binding), Err(CursorInvalid::foreign()), "{changed}");
            }
            // A character removed is as foreign.
            let mut shorter = text.clone();
            shorter.remove(at);
            assert_eq!(codec.decode(&shorter, "member list", &binding), Err(CursorInvalid::foreign()), "{shorter}");
        }
        for text in ["", ".", "atip", "atip....", "x", "abc.def", "01HZX5V0R2S4K9J3M8N6P7Q1WE", "{\"cursor\":1}", "atip.a.b.c.d.e", "\u{0}", "🙂"] {
            assert_eq!(codec.decode(text, "member list", &binding), Err(CursorInvalid::foreign()), "{text:?}");
        }
    }

    #[test]
    fn atip_cratecursor_07_another_command_is_foreign_other_arguments_are_changed_and_the_error_is_curs_or_invalid() {
        let codec = cursors("1.2");
        let binding = bind("1.2", &list());
        let text = codec.encode("member list", &binding, "key-1");
        assert_eq!(codec.decode(&text, "member tag", &binding), Err(CursorInvalid::foreign()));
        let other = bind("1.2", &[Given::option("status", "archived")]);
        assert_eq!(codec.decode(&text, "member list", &other), Err(CursorInvalid::arguments_changed()));
        assert_eq!(codec.decode(&text, "member list", &binding), Ok("key-1".to_string()));
        // A foreign cursor is foreign before its arguments are compared.
        assert_eq!(codec.decode(&text, "member tag", &other), Err(CursorInvalid::foreign()));
        for (refused, reason) in [(CursorInvalid::foreign(), "FOREIGN"), (CursorInvalid::arguments_changed(), "ARGUMENTS_CHANGED"), (CursorInvalid::expired(), "EXPIRED")] {
            let error = CliError::from(refused);
            assert_eq!((error.code, error.exit, error.retryable), ("CURSOR_INVALID", 2, false));
            assert_eq!((error.field_name(), error.reason_name()), (Some("--cursor"), Some(reason)));
        }
    }

    #[test]
    fn atip_cratecursor_08_a_cursor_of_another_minor_is_refused_and_expired_is_the_tools_to_say() {
        let one_one = cursors("1.1");
        let one_two = cursors("1.2");
        let issued = one_one.encode("member list", &bind("1.1", &list()), "key-1");
        assert_eq!(one_two.decode(&issued, "member list", &bind("1.2", &list())), Err(CursorInvalid::arguments_changed()), "a tool that raised its minor refuses the cursors of the old one");
        let issued = one_two.encode("member list", &bind("1.2", &list()), "key-1");
        assert_eq!(one_one.decode(&issued, "member list", &bind("1.1", &list())), Err(CursorInvalid::arguments_changed()));
        assert_eq!(CursorInvalid::expired().reason(), Reason::Expired);
        assert_eq!(CliError::from(CursorInvalid::expired()).reason_name(), Some("EXPIRED"));
        // The rows a tool registers under `CURSOR_INVALID`: sorted, the three reasons of spec 7.2.3.
        let names: Vec<&str> = CURSOR_INVALID_REASONS.iter().map(|reason| reason.reason).collect();
        assert_eq!(names, ["ARGUMENTS_CHANGED", "EXPIRED", "FOREIGN"]);
        for reason in [Reason::Foreign, Reason::ArgumentsChanged, Reason::Expired] {
            assert!(CURSOR_INVALID_REASONS.iter().any(|row| row.reason == reason.as_str()));
        }
    }

    /// The invocation `acme member list ...` read back from argv, as a tool would normalise it, for the options of this test.
    fn parse(words: &[String], kinds: &BTreeMap<&str, Kind>, path_words: usize) -> Vec<Given> {
        let mut given = Vec::new();
        let mut operands = Vec::new();
        let mut at = path_words;
        let mut after_double_dash = false;
        while at < words.len() {
            let word = &words[at];
            at += 1;
            if after_double_dash {
                operands.push(json!(word));
            } else if word == "--" {
                after_double_dash = true;
            } else if let Some(option) = word.strip_prefix("--") {
                let (name, joined) = option.split_once('=').map_or((option, None), |(name, value)| (name, Some(value)));
                match kinds.get(name).copied().unwrap_or(Kind::Value) {
                    Kind::Switch => given.push(Given::switch(name, true)),
                    Kind::Count => match given.iter_mut().find(|argument| argument.name == name) {
                        Some(counted) => counted.value = json!(counted.value.as_u64().unwrap_or(0) + 1),
                        None => given.push(Given::count(name, 1)),
                    },
                    _ => given.push(Given::option(name, joined.expect("every valued option is written joined"))),
                }
            }
        }
        if !operands.is_empty() {
            given.push(Given::operands(operands));
        }
        given
    }

    #[test]
    fn atip_cratecursor_09_the_next_page_argv_carries_the_explicit_arguments_and_the_cursor_and_nothing_else() {
        let kinds: BTreeMap<&str, Kind> = [("verbose", Kind::Switch), ("level", Kind::Count), ("json", Kind::Switch)].into_iter().collect();
        // What `acme member list --input -` read: a document with these keys, and the environment supplied one more.
        let given = vec![
            Given::option("status", "active"),
            Given::option("limit", 5),
            Given::switch("verbose", true),
            Given::count("level", 2),
            Given::option("repo", ".").tool_flag_only(),
            Given::option("org", "eng").from_env(),
            Given::option("role", "member"),
            Given::option("cursor", "atip.stale"),
            // The call read its arguments from `--input -`: they are in the list, and `--input` itself is not written.
            Given::option("input", "-"),
            Given::operands(vec![json!("eng")]),
        ];
        let binding = bind("1.2", &given);
        let text = cursors("1.2").encode("member list", &binding, "key-1");
        let argv = next_page_argv(&["member", "list"], &given, &text, true);
        assert_eq!(argv, ["member", "list", "--status=active", "--limit=5", "--verbose", "--level", "--level", "--repo=.", "--role=member", &format!("--cursor={text}"), "--json", "--", "eng"]);
        assert!(!argv.iter().any(|word| word == "--input" || word == "-" || word.contains("atip.stale") || word.starts_with("--org")), "{argv:?}");
        assert_eq!(argv.iter().filter(|word| word.starts_with("--cursor")).count(), 1, "the earlier --cursor is replaced");
        // Run in the same environment, the call binds the same, and its own cursor is accepted.
        let mut again = parse(&argv, &kinds, 2);
        again.push(Given::option("org", "eng").from_env());
        for argument in &mut again {
            if argument.name == "limit" {
                argument.value = json!(5);
            }
            if argument.name == "repo" {
                argument.flag_only = true;
            }
        }
        let rebound = bind("1.2", &again);
        assert_eq!(rebound, binding);
        assert_eq!(cursors("1.2").decode(&text, "member list", &rebound), Ok("key-1".to_string()));
        // Without JSON mode, no --json; and `--limit` could be changed by the caller without changing the binding.
        assert!(!next_page_argv(&["member", "list"], &given, &text, false).contains(&"--json".to_string()));
        let mut smaller = again.clone();
        smaller.retain(|argument| argument.name != "limit");
        smaller.push(Given::option("limit", 1));
        assert_eq!(bind("1.2", &smaller), binding);
    }

    #[test]
    fn atip_cratecursor_10_a_value_starting_with_a_dash_is_joined_and_an_earlier_cursor_is_replaced() {
        let given = vec![Given::option("cursor", "atip.old"), Given::option("name", "-x"), Given::option("empty", "")];
        let argv = next_page_argv(&["list"], &given, "atip.new", false);
        assert_eq!(argv, ["list", "--name=-x", "--empty=", "--cursor=atip.new"]);
        assert_eq!(argv.iter().filter(|word| word.starts_with("--cursor")).count(), 1);
        let given = vec![Given::switch("json", true), Given::option("status", "a")];
        assert_eq!(next_page_argv(&["list"], &given, "atip.new", true), ["list", "--status=a", "--cursor=atip.new", "--json"], "--json once");
    }

    #[test]
    fn atip_cratecursor_11_the_contract_example_differs_between_1_1_and_1_2_as_the_contract_says() {
        // `acme member list --status active --limit 5`, then variants of it.
        let call = |extra: &[Given]| {
            let mut given = vec![Given::option("status", "active"), Given::option("limit", 5)];
            given.extend_from_slice(extra);
            given
        };
        let (eleven, twelve) = (cursors("1.1"), cursors("1.2"));
        let bound = |codec: &Cursors, extra: &[Given]| codec.binding(&call(extra)).expect("a binding");
        // `--all` replaces `--limit`: a 1.1 tool binds it, a 1.2 tool does not.
        let all = [Given::switch("all", true)];
        assert_ne!(bound(&eleven, &[]), bound(&eleven, &all));
        assert_eq!(bound(&twelve, &[]), bound(&twelve, &all));
        // `--json` added or removed: the same split.
        assert_ne!(bound(&eleven, &[]), bound(&eleven, &[Given::switch("json", true)]));
        assert_eq!(bound(&twelve, &[]), bound(&twelve, &[Given::switch("json", true)]));
        // An option added at its default value: another call in both (a 1.1 tool binds every explicit argument).
        let role = [Given::option("role", "member")];
        assert_ne!(bound(&eleven, &[]), bound(&eleven, &role));
        assert_ne!(bound(&twelve, &[]), bound(&twelve, &role));
        // The `env` variable of an option: bound by a 1.2 tool only.
        let org = [Given::option("org", "eng").from_env()];
        assert_eq!(bound(&eleven, &[]), bound(&eleven, &org));
        assert_ne!(bound(&twelve, &[]), bound(&twelve, &org));
        // `--limit` and `--fields` bind in neither.
        let fields = [Given::option("fields", "name")];
        assert_eq!(bound(&eleven, &[]), bound(&eleven, &fields));
        assert_eq!(bound(&twelve, &[]), bound(&twelve, &fields));
    }

    #[test]
    fn atip_cratecursor_13_what_only_one_minor_binds_is_refused_between_pages_by_that_minor_only() {
        let first = vec![Given::option("status", "active"), Given::option("limit", 5)];
        let then = |extra: Given| {
            let mut given = first.clone();
            given.push(extra);
            given
        };
        // (what the second page adds, the minor that binds it, the minor that does not)
        let cases = [
            ("--all", Given::switch("all", true), "1.1", "1.2"),
            ("--json", Given::switch("json", true), "1.1", "1.2"),
            ("an environment value", Given::option("org", "eng").from_env(), "1.2", "1.1"),
        ];
        for (what, extra, binds, ignores) in cases {
            let second = then(extra);
            // The minor that binds it: the cursor of the first page is refused on the second.
            let codec = cursors(binds);
            let issued = codec.encode("member list", &bind(binds, &first), "key-1");
            assert_eq!(codec.decode(&issued, "member list", &bind(binds, &second)), Err(CursorInvalid::arguments_changed()), "{binds} refuses {what} added between pages");
            assert_eq!(codec.decode(&issued, "member list", &bind(binds, &first)), Ok("key-1".to_string()), "{binds} takes the same call");
            // The minor that leaves it out: the same cursor goes on.
            let codec = cursors(ignores);
            let issued = codec.encode("member list", &bind(ignores, &first), "key-1");
            assert_eq!(codec.decode(&issued, "member list", &bind(ignores, &second)), Ok("key-1".to_string()), "{ignores} does not bind {what}");
        }
        // An argument both bind, changed between pages, is refused by both.
        for minor in ["1.1", "1.2"] {
            let codec = cursors(minor);
            let issued = codec.encode("member list", &bind(minor, &first), "key-1");
            assert_eq!(codec.decode(&issued, "member list", &bind(minor, &then(Given::option("role", "member")))), Err(CursorInvalid::arguments_changed()), "{minor}");
        }
    }
}
