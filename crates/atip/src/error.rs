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
//!   ([`ErrorBody`]): `code`, `message`, `retryable`, then `field` (the argument
//!   or JSON pointer refused), `expected` (the valid set when there is one),
//!   `received` (the refused value, secrets redacted), `suggestions` and
//!   `hint`. A member with no value is absent, never `null` and never `[]`
//!   (spec 6.5).
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
}

/// The row of `code` in `table`, which MUST be sorted by code.
pub fn find_code(table: &'static [CodeInfo], code: &str) -> Option<&'static CodeInfo> {
    table.binary_search_by(|info| info.code.cmp(code)).ok().map(|index| &table[index])
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
    hint: Option<String>,
    /// The argument or record field that was refused (`--to`, `--kind`).
    field: Option<String>,
    /// The values `field` accepts, when the refusal is about a closed set.
    expected: Option<Vec<String>>,
    /// The refused value as given; redacted when it is rendered ([`redact`]).
    received: Option<String>,
    /// The nearest valid words, closest first.
    suggestions: Vec<String>,
    /// Tool extension members of the error object (`x-<tool>`, spec 7.1): name and value.
    extensions: BTreeMap<String, Value>,
}

/// The `error` member of a failure envelope. Field order is the document
/// order: `code` first. An optional member with no value is left out of the
/// document, so the body validates against the contract's error schema.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorBody {
    /// Stable SCREAMING_SNAKE code a caller branches on, registered in the tool's error registry.
    pub code: String,
    pub message: String,
    /// True only where running the same command again can succeed; equal to the registry row's.
    pub retryable: bool,
    /// The refused argv name (`--scenario`) or an RFC 6901 JSON pointer into `--input`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// The valid values of `field`, when it takes a closed set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<Vec<String>>,
    /// The value that was refused, with secrets redacted.
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
        text
    }

    /// The `error` member of the failure envelope, with the `field` as the
    /// tool names it in the document (`field`; an input pointer for a tool that
    /// has `--input`) and the `suggestions` the tool computed. `received` is
    /// redacted by the error's own `field`.
    pub fn body_with(&self, field: Option<String>, suggestions: Vec<String>) -> ErrorBody {
        ErrorBody {
            code: self.code.to_string(),
            message: self.message.clone(),
            retryable: self.retryable,
            field: field.filter(|field| !field.is_empty()),
            expected: self.expected_values().filter(|expected| !expected.is_empty()).map(<[String]>::to_vec),
            received: self.received_value().filter(|received| !received.is_empty()).map(|received| redact(self.field_name(), received)),
            suggestions,
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
    use super::{find_code, redact, CliError, CodeInfo, EXIT_FAILED, EXIT_REFUSED};

    const TABLE: &[CodeInfo] = &[
        CodeInfo { code: "GATE_VIOLATIONS", meaning: "m", exit: EXIT_FAILED, retryable: false, fix: "f" },
        CodeInfo { code: "LOCKED", meaning: "m", exit: EXIT_REFUSED, retryable: true, fix: "f" },
        CodeInfo { code: "USAGE", meaning: "m", exit: EXIT_REFUSED, retryable: false, fix: "f" },
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
}
