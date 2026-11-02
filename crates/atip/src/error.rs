//! The error value a command speaks.
//!
//! A refusal or an operational failure is a [`CliError`]: a stable
//! SCREAMING_SNAKE `code` (the thing a caller branches on), a `message`, an
//! optional `hint` (a working invocation or the next step), whether a retry can
//! succeed, and the process exit code. The exit code and `retryable` are not
//! chosen at the site that builds the error: they come from the code's row in
//! the tool's table of [`CodeInfo`] rows ([`CliError::from_row`]), so one code
//! has one exit everywhere and nothing overrides it at a call site (spec 7.3.2).
//! A situation that needs another exit needs another code.
//!
//! The crate owns the shape and the tool owns the table: which codes exist,
//! what they mean, which exit each carries, and how a tool turns an error into
//! its failure envelope.
//!
//! - text mode, on stderr: [`CliError::human`], `<tool>: <CODE>: <message>`,
//!   then indented `field:`, `expected:`, `suggestions:` and `hint:` lines
//!   for the members the error has (spec 7.4);
//! - JSON mode: [`CliError::body_with`], the failure envelope's `error` member
//!   ([`ErrorBody`]): `code`, `reason` (one of the reasons registered under the
//!   code, since 1.1), `message`, `retryable`, then `field` (the argument or
//!   JSON pointer refused), `expected` (the valid set when there is one),
//!   `received` (the refused value, secrets redacted; `""` when the caller
//!   sent an empty value), `suggestions` and `hint`. When one input yields
//!   several errors (spec 7.1.1) the first is the error itself and
//!   [`CliError::details`] lists every one as `details`. A member with no value
//!   is absent, never `null` and never `[]` (spec 6.5).
//!
//! Exit codes: `0` ok; `1` the command ran and its answer is negative or
//! incomplete ([`EXIT_FAILED`]); `2` the command refused to run
//! ([`EXIT_REFUSED`]). Nothing else is a registry exit; a signal exit (128+n)
//! is outside the registry.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

/// A finding, a failed check or an operational failure.
pub const EXIT_FAILED: u8 = 1;
/// A refusal or a usage error.
pub const EXIT_REFUSED: u8 = 2;

/// One reason registered under an error code (spec 7.6): a situation under the
/// code that needs another next action. It shares its code's exit and retryability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReasonInfo {
    /// UPPER_SNAKE_CASE, unique within its code.
    pub reason: &'static str,
    pub meaning: &'static str,
    /// What the caller does next in this situation.
    pub fix: &'static str,
}

/// One row of a tool's error-code table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeInfo {
    pub code: &'static str,
    pub meaning: &'static str,
    /// The process exit code the code carries (`1` or `2`).
    pub exit: u8,
    /// True only where running the same command again can succeed.
    pub retryable: bool,
    /// One imperative sentence telling the caller what to do about the code.
    pub fix: &'static str,
    /// The reasons registered under the code, sorted by name (empty: the code has none).
    pub reasons: &'static [ReasonInfo],
}

/// One row of a tool's warning-code table (spec 7.5.2): no exit and no retryability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WarningInfo {
    pub code: &'static str,
    pub meaning: &'static str,
    /// One sentence telling the caller what to do about the warning.
    pub fix: &'static str,
}

/// The row of `code` in `table`, which MUST be sorted by code.
pub fn find_code(table: &'static [CodeInfo], code: &str) -> Option<&'static CodeInfo> {
    table.binary_search_by(|info| info.code.cmp(code)).ok().map(|index| &table[index])
}

/// The row of `code` in the warning `table`, which MUST be sorted by code.
pub fn find_warning(table: &'static [WarningInfo], code: &str) -> Option<&'static WarningInfo> {
    table.binary_search_by(|info| info.code.cmp(code)).ok().map(|index| &table[index])
}

impl CodeInfo {
    /// The reason named `reason` registered under this code.
    pub fn find_reason(&self, reason: &str) -> Option<&'static ReasonInfo> {
        self.reasons.iter().find(|info| info.reason == reason)
    }
}

impl ReasonInfo {
    /// The reason that stands for the code of `row` (spec 8.3.2): the name is the code, the meaning and the fix are the row's.
    /// A `const fn`, so a static table can build a reason from another row of the same crate's tables.
    pub const fn from_code(row: &CodeInfo) -> ReasonInfo {
        ReasonInfo { reason: row.code, meaning: row.meaning, fix: row.fix }
    }
}

const fn str_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut at = 0;
    while at < a.len() {
        if a[at] != b[at] {
            return false;
        }
        at += 1;
    }
    true
}

/// Whether `a` sorts before `b`, byte by byte: the order `str::cmp` and the tables' sort use.
const fn str_lt(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut at = 0;
    while at < a.len() && at < b.len() {
        if a[at] != b[at] {
            return a[at] < b[at];
        }
        at += 1;
    }
    a.len() < b.len()
}

/// How many rows of `rows` carry `exit`: the `N` of [`reasons_with_exit`].
pub const fn count_with_exit(rows: &[CodeInfo], exit: u8) -> usize {
    let mut count = 0;
    let mut at = 0;
    while at < rows.len() {
        if rows[at].exit == exit {
            count += 1;
        }
        at += 1;
    }
    count
}

/// The reasons that stand for every row of `rows` with `exit`, sorted by name, whatever the order of `rows`: for the
/// `STOPPED_MIDWAY` row, whose reasons are the tool's exit-2 codes (spec 8.3.2). `N` is [`count_with_exit`]`(rows, exit)`;
/// any other length stops the evaluation, so the table does not compile.
///
/// `rows` is the table of the codes the reasons come from, without the row that holds them (a row cannot read the
/// table that contains it). [`with_row`] then puts that row into the table.
///
/// ```
/// use atip::{count_with_exit, reasons_with_exit, CodeInfo, ReasonInfo};
///
/// const REFUSALS: &[CodeInfo] = &[
///     CodeInfo { code: "LEASE_HELD", meaning: "m", exit: 2, retryable: true, fix: "f", reasons: &[] },
///     CodeInfo { code: "WRITE_FAILED", meaning: "m", exit: 1, retryable: false, fix: "f", reasons: &[] },
///     CodeInfo { code: "PLAN_STALE", meaning: "m", exit: 2, retryable: false, fix: "f", reasons: &[] },
/// ];
/// const REASONS: [ReasonInfo; count_with_exit(REFUSALS, 2)] = reasons_with_exit(REFUSALS, 2);
/// assert_eq!(REASONS.map(|reason| reason.reason), ["LEASE_HELD", "PLAN_STALE"]);
/// ```
pub const fn reasons_with_exit<const N: usize>(rows: &[CodeInfo], exit: u8) -> [ReasonInfo; N] {
    assert!(count_with_exit(rows, exit) == N, "the length of the reasons is not the number of rows with that exit");
    let mut out = [ReasonInfo { reason: "", meaning: "", fix: "" }; N];
    let mut filled = 0;
    let mut at = 0;
    while at < rows.len() {
        if rows[at].exit == exit {
            // Insertion sort by name: the result does not depend on the order of `rows`.
            let reason = ReasonInfo::from_code(&rows[at]);
            let mut place = filled;
            while place > 0 && str_lt(reason.reason, out[place - 1].reason) {
                out[place] = out[place - 1];
                place -= 1;
            }
            out[place] = reason;
            filled += 1;
        }
        at += 1;
    }
    out
}

/// The reason that stands for the row of `code` in `rows`. A code that `rows` does not hold stops the evaluation, so a
/// reason of a code the table lacks does not compile:
///
/// ```compile_fail,E0080
/// const MISSING: atip::ReasonInfo = atip::reason_of_code(&[], "NOT_IN_THE_TABLE");
/// ```
pub const fn reason_of_code(rows: &[CodeInfo], code: &str) -> ReasonInfo {
    let mut at = 0;
    while at < rows.len() {
        if str_eq(rows[at].code, code) {
            return ReasonInfo::from_code(&rows[at]);
        }
        at += 1;
    }
    panic!("the table has no row of that code, so no reason stands for it")
}

/// `rows` with `row` put in its place by code, so a table sorted by code stays sorted ([`find_code`]). `N` is
/// `rows.len() + 1`; a row whose code `rows` already holds, or any other length, stops the evaluation.
pub const fn with_row<const N: usize>(rows: &[CodeInfo], row: CodeInfo) -> [CodeInfo; N] {
    assert!(N == rows.len() + 1, "the length of the table is not the number of rows plus one");
    let mut out = [row; N];
    let mut place = 0;
    while place < rows.len() && str_lt(rows[place].code, row.code) {
        out[place] = rows[place];
        place += 1;
    }
    assert!(place == rows.len() || !str_eq(rows[place].code, row.code), "the table already has a row of that code");
    out[place] = row;
    while place < rows.len() {
        out[place + 1] = rows[place];
        place += 1;
    }
    out
}

/// One refusal or operational failure.
///
/// The members every error has stay inline; the optional ones that most errors
/// leave empty live in one boxed [`CliErrorDetail`], so the `Err` side of a
/// `Result<_, CliError>` stays small.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    pub code: &'static str,
    pub message: String,
    /// Whether running the same command again, unchanged, can succeed
    /// (the code's table row).
    pub retryable: bool,
    /// The process exit code: the code's table row, `1` or `2`.
    pub exit: u8,
    detail: Option<Box<CliErrorDetail>>,
}

/// The optional members of a [`CliError`], read through its accessors and set
/// through its builder methods.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CliErrorDetail {
    /// Which of the situations registered under the code this is (spec 7.6).
    reason: Option<&'static str>,
    hint: Option<String>,
    /// The argument or record field that was refused (`--to`, `--kind`).
    field: Option<String>,
    /// The values `field` accepts, when the refusal is about a closed set.
    expected: Option<Vec<String>>,
    /// The refused value as given, `""` included (a value the caller did not send is `None`); redacted when it is rendered ([`redact`]).
    received: Option<String>,
    /// The nearest valid words, closest first.
    suggestions: Vec<String>,
    /// Tool extension members of the error object (`x-<tool>`, spec 7.1): name and value.
    extensions: BTreeMap<String, Value>,
    /// Every error of an input that yields several, the first included (spec 7.1.1); empty for an error alone.
    details: Vec<CliError>,
}

/// The `error` member of a failure envelope. Field order is the document
/// order: `code` first. An optional member with no value is left out of the
/// document, so the body validates against the contract's error schema.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorBody {
    /// Stable SCREAMING_SNAKE code a caller branches on, registered in the tool's error registry.
    pub code: String,
    /// Which of the situations registered under `code` this is (since 1.1); registered in the code's row.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub message: String,
    /// True only where running the same command again can succeed; equal to the registry row's.
    pub retryable: bool,
    /// The refused argv name (`--scenario`) or an RFC 6901 JSON pointer into `--input`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// The valid values of `field`, when it takes a closed set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<Vec<String>>,
    /// The value that was refused, with secrets redacted; `""` for an empty value the caller sent (1.1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub received: Option<String>,
    /// The nearest valid commands, options or values.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<String>,
    /// A working invocation or the next step.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// The tool's own members (`x-canon`); the contract's error schema allows any `x-<tool>` member.
    #[serde(flatten, skip_serializing_if = "BTreeMap::is_empty")]
    #[schemars(skip)]
    pub extensions: BTreeMap<String, Value>,
    /// Every error of an input that yields several, in the order the tool found them (spec 7.1.1); absent for one error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<ErrorDetailBody>>,
}

/// One item of `details` (spec 7.1.1): the members of [`ErrorBody`] except `retryable` and `details`.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorDetailBody {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub received: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(flatten, skip_serializing_if = "BTreeMap::is_empty")]
    #[schemars(skip)]
    pub extensions: BTreeMap<String, Value>,
}

impl CliError {
    /// An error for `code` with the exit and retryability of its table `row`.
    /// A code with no row is a refusal that cannot be retried.
    pub fn from_row(code: &'static str, message: impl Into<String>, row: Option<&CodeInfo>) -> Self {
        let (exit, retryable) = row.map_or((EXIT_REFUSED, false), |info| (info.exit, info.retryable));
        Self { code, message: message.into(), retryable, exit, detail: None }
    }

    fn detail_mut(&mut self) -> &mut CliErrorDetail {
        self.detail.get_or_insert_with(Box::default)
    }

    /// The reason, if the error has one.
    pub fn reason_name(&self) -> Option<&'static str> {
        self.detail.as_deref()?.reason
    }

    /// The hint, if the error has one.
    pub fn hint_text(&self) -> Option<&str> {
        self.detail.as_deref()?.hint.as_deref()
    }

    /// The refused argument or field, if the error names one.
    pub fn field_name(&self) -> Option<&str> {
        self.detail.as_deref()?.field.as_deref()
    }

    /// The values the refused field accepts, if the error lists them.
    pub fn expected_values(&self) -> Option<&[String]> {
        self.detail.as_deref()?.expected.as_deref()
    }

    /// The refused value as given, before redaction.
    pub fn received_value(&self) -> Option<&str> {
        self.detail.as_deref()?.received.as_deref()
    }

    /// The nearest-word suggestions set on the error, closest first.
    pub fn suggested(&self) -> &[String] {
        self.detail.as_deref().map_or(&[], |detail| detail.suggestions.as_slice())
    }

    /// Which of the situations registered under the error's code this is (spec 7.6). The reason
    /// MUST be registered in the code's row: a tool checks that where it renders the error.
    pub fn reason(mut self, reason: &'static str) -> Self {
        self.detail_mut().reason = Some(reason);
        self
    }

    /// The next step the caller can take.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.detail_mut().hint = Some(hint.into());
        self
    }

    /// Names the refused argument or field (`--to`, `--kind`).
    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.detail_mut().field = Some(field.into());
        self
    }

    /// Lists the values the refused field accepts.
    pub fn expected<I, S>(mut self, expected: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.detail_mut().expected = Some(expected.into_iter().map(Into::into).collect());
        self
    }

    /// The value that was refused. It is redacted when rendered.
    pub fn received(mut self, received: impl Into<String>) -> Self {
        self.detail_mut().received = Some(received.into());
        self
    }

    /// Sets the tool extension member `name` (`x-canon`) of the error object to `value`.
    pub fn extension(mut self, name: impl Into<String>, value: Value) -> Self {
        self.detail_mut().extensions.insert(name.into(), value);
        self
    }

    /// The tool extension members the error carries, by name.
    pub fn extensions(&self) -> Option<&BTreeMap<String, Value>> {
        self.detail.as_deref().map(|detail| &detail.extensions).filter(|extensions| !extensions.is_empty())
    }

    /// Replaces the nearest-word suggestions.
    pub fn suggestions<I, S>(mut self, suggestions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.detail_mut().suggestions = suggestions.into_iter().map(Into::into).collect();
        self
    }

    /// Lists every error of an input that yields several (spec 7.1.1), the error itself first. The body carries
    /// them as `details` when there are two or more.
    pub fn details(mut self, errors: Vec<CliError>) -> Self {
        self.detail_mut().details = errors;
        self
    }

    /// The errors listed with [`CliError::details`].
    pub fn detail_errors(&self) -> &[CliError] {
        self.detail.as_deref().map_or(&[], |detail| detail.details.as_slice())
    }

    /// The stderr text: `<tool>: <CODE>: <message>`, then one indented line for
    /// each of `field`, `expected`, `suggestions` and `hint` the error has
    /// (spec 7.4.1). `tool` is the executable name (`canon`). `field` is the
    /// name as the tool states it in the document (see [`CliError::body_with`]).
    pub fn human(&self, tool: &str, field: Option<&str>, suggestions: &[String]) -> String {
        let mut text = format!("{tool}: {}: {}", self.code, self.message);
        if let Some(field) = field {
            text.push_str(&format!("\n  field: {field}"));
        }
        if let Some(expected) = self.expected_values().filter(|expected| !expected.is_empty()) {
            text.push_str(&format!("\n  expected: {}", expected.join(", ")));
        }
        if !suggestions.is_empty() {
            text.push_str(&format!("\n  suggestions: {}", suggestions.join(", ")));
        }
        if let Some(hint) = self.hint_text() {
            text.push_str(&format!("\n  hint: {hint}"));
        }
        for item in self.detail_errors() {
            text.push_str(&format!("\n  problem: {}: {}", item.field_name().unwrap_or_default(), item.message));
        }
        text
    }

    /// The `error` member of the failure envelope, with the `field` as the
    /// tool names it in the document (`field`; an input pointer for a tool that
    /// has `--input`) and the `suggestions` the tool computed. `received` is
    /// redacted by the error's own `field`.
    pub fn body_with(&self, field: Option<String>, suggestions: Vec<String>) -> ErrorBody {
        ErrorBody {
            code: self.code.to_string(),
            reason: self.reason_name().map(str::to_string),
            message: self.message.clone(),
            retryable: self.retryable,
            field: field.filter(|field| !field.is_empty()),
            expected: self.expected_values().filter(|expected| !expected.is_empty()).map(<[String]>::to_vec),
            received: self.received_value().map(|received| redact(self.field_name(), received)),
            suggestions,
            hint: self.hint_text().filter(|hint| !hint.is_empty()).map(str::to_string),
            extensions: self.extensions().cloned().unwrap_or_default(),
            details: (self.detail_errors().len() > 1).then(|| self.detail_errors().iter().map(CliError::detail_body).collect()),
        }
    }

    /// This error as one item of `details`: the item's own `field` and suggestions, `received` redacted by that field.
    fn detail_body(&self) -> ErrorDetailBody {
        ErrorDetailBody {
            code: self.code.to_string(),
            reason: self.reason_name().map(str::to_string),
            message: self.message.clone(),
            field: self.field_name().filter(|field| !field.is_empty()).map(str::to_string),
            expected: self.expected_values().filter(|expected| !expected.is_empty()).map(<[String]>::to_vec),
            received: self.received_value().map(|received| redact(self.field_name(), received)),
            suggestions: self.suggested().to_vec(),
            hint: self.hint_text().filter(|hint| !hint.is_empty()).map(str::to_string),
            extensions: self.extensions().cloned().unwrap_or_default(),
        }
    }
}

/// `<CODE>: <message>`: the form for logs and `?` chains, without the command
/// prefix or the hint. Every user-visible rendering goes through
/// [`CliError::human`] or the envelope ([`CliError::body_with`]) instead, never
/// through this.
impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CliError {}

/// What a `received` value may keep: argument names that carry secrets lose the
/// whole value, `NAME=value` environment assignments keep only the name, and a
/// token-shaped word (a well-known prefix, or 24 or more mixed-case letters and
/// digits) is replaced. A digest is lowercase hex and passes.
pub fn redact(field: Option<&str>, received: &str) -> String {
    const SECRET_NAMES: [&str; 8] = ["token", "secret", "password", "passwd", "key", "signature", "credential", "auth"];
    const TOKEN_PREFIXES: [&str; 9] = ["sk-", "ghp_", "gho_", "ghs_", "github_pat_", "xoxb-", "xoxp-", "AKIA", "eyJ"];
    const REDACTED: &str = "<redacted>";
    if let Some(field) = field {
        let field = field.to_ascii_lowercase();
        // A flag or key *named* for a secret; `--signature-file` names a path, which is not one.
        if SECRET_NAMES.iter().any(|name| field.contains(name)) && !field.ends_with("file") {
            return REDACTED.to_string();
        }
    }
    if let Some((name, value)) = received.split_once('=') {
        let env_name = name.starts_with(|c: char| c.is_ascii_uppercase()) && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
        if env_name && !value.is_empty() {
            return format!("{name}={REDACTED}");
        }
    }
    let word = received.trim();
    let bearer = word.get(..7).is_some_and(|prefix| prefix.eq_ignore_ascii_case("bearer "));
    let token_alphabet = word.len() >= 24 && word.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '+' | '=' | '/'));
    let mixed = word.chars().any(|c| c.is_ascii_digit()) && word.chars().any(|c| c.is_ascii_uppercase()) && word.chars().any(|c| c.is_ascii_lowercase());
    if bearer || TOKEN_PREFIXES.iter().any(|prefix| word.starts_with(prefix)) || (token_alphabet && mixed) {
        return REDACTED.to_string();
    }
    match received.char_indices().nth(200) {
        Some((end, _)) => format!("{}…", &received[..end]),
        None => received.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{count_with_exit, find_code, reason_of_code, reasons_with_exit, redact, with_row, CliError, CodeInfo, ReasonInfo, EXIT_FAILED, EXIT_REFUSED};

    const TABLE: &[CodeInfo] = &[
        CodeInfo { code: "GATE_VIOLATIONS", meaning: "m", exit: EXIT_FAILED, retryable: false, fix: "f", reasons: &[] },
        CodeInfo { code: "LOCKED", meaning: "m", exit: EXIT_REFUSED, retryable: true, fix: "f", reasons: &[] },
        CodeInfo { code: "USAGE", meaning: "m", exit: EXIT_REFUSED, retryable: false, fix: "f", reasons: &[ReasonInfo { reason: "SHAPE", meaning: "m", fix: "f" }] },
    ];

    #[test]
    fn display_is_code_and_message_only_and_the_human_form_has_its_own_lines() {
        let error = CliError::from_row("USAGE", "bad flag", find_code(TABLE, "USAGE")).hint("pass --x").expected(["--x", "--y"]);
        assert_eq!(error.to_string(), "USAGE: bad flag");
        assert_eq!(error.human("canon", Some("--z"), &["--x".to_string()]), "canon: USAGE: bad flag\n  field: --z\n  expected: --x, --y\n  suggestions: --x\n  hint: pass --x");
        assert_eq!(CliError::from_row("USAGE", "bad", None).human("canon", None, &[]), "canon: USAGE: bad");
        let as_error: &dyn std::error::Error = &error;
        assert!(as_error.source().is_none());
    }

    #[test]
    fn an_error_takes_its_exit_and_retryability_from_its_row() {
        assert_eq!(CliError::from_row("GATE_VIOLATIONS", "x", find_code(TABLE, "GATE_VIOLATIONS")).exit, EXIT_FAILED);
        assert_eq!(CliError::from_row("USAGE", "x", find_code(TABLE, "USAGE")).exit, EXIT_REFUSED);
        assert!(CliError::from_row("LOCKED", "x", find_code(TABLE, "LOCKED")).retryable);
        let unlisted = CliError::from_row("NOPE", "x", find_code(TABLE, "NOPE"));
        assert_eq!((unlisted.exit, unlisted.retryable), (EXIT_REFUSED, false), "a code with no row is a refusal that cannot be retried");
        assert!(!CliError::from_row("USAGE", "x", None).retryable);
    }

    #[test]
    fn the_body_carries_the_builder_members_and_redacts_by_the_raw_field() {
        let error = CliError::from_row("USAGE", "bad", None).field("--api-token").expected(["a", "b"]).received("hunter2").suggestions(["a"]).hint("try a");
        assert_eq!(error.field_name(), Some("--api-token"));
        assert_eq!(error.expected_values(), Some(["a".to_string(), "b".to_string()].as_slice()));
        assert_eq!(error.received_value(), Some("hunter2"));
        assert_eq!(error.suggested(), ["a"]);
        let body = error.body_with(Some("/apiToken".to_string()), vec!["b".to_string()]);
        assert_eq!(serde_json::to_string(&body).expect("a body serializes"), r#"{"code":"USAGE","message":"bad","retryable":false,"field":"/apiToken","expected":["a","b"],"received":"<redacted>","suggestions":["b"],"hint":"try a"}"#);
        let bare = CliError::from_row("USAGE", "bad", None).body_with(None, Vec::new());
        assert_eq!(serde_json::to_string(&bare).expect("a body serializes"), r#"{"code":"USAGE","message":"bad","retryable":false}"#, "a member with no value is absent, never null or empty");
        assert!(CliError::from_row("USAGE", "bad", None).suggested().is_empty());
    }

    #[test]
    fn a_reason_follows_the_code_and_an_empty_received_is_kept() {
        let row = find_code(TABLE, "USAGE").expect("a row");
        assert_eq!(row.find_reason("SHAPE").map(|reason| reason.meaning), Some("m"));
        assert!(row.find_reason("OTHER").is_none());
        let error = CliError::from_row("USAGE", "bad", Some(row)).reason("SHAPE").field("--x").received("");
        assert_eq!(error.reason_name(), Some("SHAPE"));
        let body = error.body_with(None, Vec::new());
        assert_eq!(serde_json::to_string(&body).expect("a body serializes"), r#"{"code":"USAGE","reason":"SHAPE","message":"bad","retryable":false,"received":""}"#, "`received` keeps an empty value the caller sent; a value never sent is absent");
    }

    #[test]
    fn a_received_value_is_redacted_when_it_looks_like_a_secret() {
        assert_eq!(redact(Some("--api-token"), "abc"), "<redacted>");
        assert_eq!(redact(None, "CANON_SECRET=hunter2"), "CANON_SECRET=<redacted>");
        assert_eq!(redact(None, "ghp_0123456789abcdef"), "<redacted>");
        assert_eq!(redact(None, "Ab3dEf6hIj9kLm2nOp5qRs8tUv1"), "<redacted>");
        assert_eq!(redact(Some("--kind"), "evidence"), "evidence");
        let digest = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert_eq!(redact(Some("--plan-digest"), digest), digest, "a digest is not a secret");
        assert_eq!(redact(Some("--signature-file"), "approval.sig"), "approval.sig");
    }

    const ROWS: &[CodeInfo] = &[
        CodeInfo { code: "A_REFUSED", meaning: "a refusal", exit: EXIT_REFUSED, retryable: false, fix: "fix a", reasons: &[] },
        CodeInfo { code: "LEASE_HELD", meaning: "another run holds the lease", exit: EXIT_REFUSED, retryable: true, fix: "wait for the lease", reasons: &[] },
        CodeInfo { code: "WRITE_FAILED", meaning: "a write failed", exit: EXIT_FAILED, retryable: false, fix: "fix write", reasons: &[] },
        CodeInfo { code: "ZED_REFUSED", meaning: "z refusal", exit: EXIT_REFUSED, retryable: false, fix: "fix z", reasons: &[] },
    ];
    /// The same rows in another order.
    const SHUFFLED: &[CodeInfo] = &[ROWS[3], ROWS[2], ROWS[0], ROWS[1]];
    const REASONS: [ReasonInfo; count_with_exit(ROWS, EXIT_REFUSED)] = reasons_with_exit(ROWS, EXIT_REFUSED);
    const REASONS_SHUFFLED: [ReasonInfo; count_with_exit(SHUFFLED, EXIT_REFUSED)] = reasons_with_exit(SHUFFLED, EXIT_REFUSED);
    const STOPPED_MIDWAY: CodeInfo = CodeInfo { code: "STOPPED_MIDWAY", meaning: "m", exit: EXIT_FAILED, retryable: false, fix: "f", reasons: &REASONS };
    static WITH_STOPPED: [CodeInfo; ROWS.len() + 1] = with_row(ROWS, STOPPED_MIDWAY);
    const ONLY_FAILED: [ReasonInfo; count_with_exit(ROWS, EXIT_FAILED)] = reasons_with_exit(ROWS, EXIT_FAILED);

    #[test]
    fn atip_crate12_08_a_row_in_a_static_table_gets_its_reasons_from_the_other_rows_whatever_their_order() {
        let names: Vec<&str> = REASONS.iter().map(|reason| reason.reason).collect();
        assert_eq!(names, ["A_REFUSED", "LEASE_HELD", "ZED_REFUSED"], "every exit-2 row, sorted by name, and no exit-1 row");
        assert_eq!(REASONS[1], ReasonInfo { reason: "LEASE_HELD", meaning: "another run holds the lease", fix: "wait for the lease" }, "the meaning and the fix are the row's");
        assert_eq!(REASONS, REASONS_SHUFFLED, "the result does not depend on the order of the table");
        assert_eq!(ONLY_FAILED.map(|reason| reason.reason), ["WRITE_FAILED"]);
        // The row that holds them sits in the table in its place, so the table stays sorted for `find_code`.
        let codes: Vec<&str> = WITH_STOPPED.iter().map(|row| row.code).collect();
        assert_eq!(codes, ["A_REFUSED", "LEASE_HELD", "STOPPED_MIDWAY", "WRITE_FAILED", "ZED_REFUSED"]);
        let found = find_code(&WITH_STOPPED, "STOPPED_MIDWAY").expect("a row");
        assert_eq!((found.exit, found.retryable, found.reasons.len()), (EXIT_FAILED, false, 3));
        assert_eq!(found.find_reason("LEASE_HELD"), Some(&REASONS[1]));
        assert!(find_code(&WITH_STOPPED, "ZED_REFUSED").is_some() && find_code(&WITH_STOPPED, "A_REFUSED").is_some(), "the others keep their rows");
        // A reason of one named row, made on its own.
        assert_eq!(reason_of_code(ROWS, "ZED_REFUSED"), REASONS[2]);
        assert_eq!(ReasonInfo::from_code(&ROWS[0]), REASONS[0]);
    }

    #[test]
    fn atip_crate12_09_a_reason_of_a_code_the_table_does_not_hold_stops_the_evaluation() {
        let missing = std::panic::catch_unwind(|| reason_of_code(ROWS, "NOT_IN_THE_TABLE"));
        assert!(missing.is_err(), "a code the table lacks has no reason");
        let empty = std::panic::catch_unwind(|| reason_of_code(&[], "A_REFUSED"));
        assert!(empty.is_err(), "an empty table holds no code");
        // A row of a code the table holds, and a length other than the count, stop the evaluation too.
        let twice = std::panic::catch_unwind(|| with_row::<5>(ROWS, ROWS[1]));
        assert!(twice.is_err(), "a table does not hold one code twice");
        let wrong_length = std::panic::catch_unwind(|| reasons_with_exit::<2>(ROWS, EXIT_REFUSED));
        assert!(wrong_length.is_err(), "the length of the reasons is the count of the rows");
        let wrong_table_length = std::panic::catch_unwind(|| with_row::<9>(ROWS, STOPPED_MIDWAY));
        assert!(wrong_table_length.is_err(), "the length of the table is the number of rows plus one");
    }

    #[test]
    fn several_errors_of_one_input_are_details_and_one_error_has_none() {
        let first = CliError::from_row("USAGE", "a is wrong", None).field("/a").reason("SHAPE");
        let second = CliError::from_row("USAGE", "b is wrong", None).field("/api-token").received("hunter2").suggestions(["c"]);
        let several = first.clone().details(vec![first.clone(), second.clone()]);
        assert_eq!(several.detail_errors().len(), 2);
        let body = several.body_with(Some("/a".to_string()), Vec::new());
        assert_eq!(
            serde_json::to_string(&body).expect("a body serializes"),
            r#"{"code":"USAGE","reason":"SHAPE","message":"a is wrong","retryable":false,"field":"/a","details":[{"code":"USAGE","reason":"SHAPE","message":"a is wrong","field":"/a"},{"code":"USAGE","message":"b is wrong","field":"/api-token","received":"<redacted>","suggestions":["c"]}]}"#
        );
        assert_eq!(several.human("canon", Some("/a"), &[]), "canon: USAGE: a is wrong\n  field: /a\n  problem: /a: a is wrong\n  problem: /api-token: b is wrong");
        // one item is not "several": the member is absent
        let one = first.clone().details(vec![first]);
        assert!(one.body_with(None, Vec::new()).details.is_none());
    }
}
