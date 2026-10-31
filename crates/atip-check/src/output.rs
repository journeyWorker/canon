//! What `atip` prints: the envelope of the contract (spec 6, 7), or the text of a person.
//!
//! The checker follows the contract it checks, at L0. Its error codes and
//! their rows live here, and nothing else decides an exit.

use serde::Serialize;
use serde_json::Value;

use atip::CodeInfo;

/// The tool name every envelope and every text error carries.
pub const TOOL: &str = "atip";
/// The version of the one output `atip check` has.
pub const SCHEMA_VERSION: &str = "1.0";

/// `atip`'s error registry (spec 7.3), sorted by code: the four reserved codes it can produce (`USAGE`, `UNKNOWN_COMMAND`, `COMMAND_REQUIRED`, `INTERNAL`) and four of its own.
pub const REGISTRY: [CodeInfo; 8] = [
    CodeInfo { code: "BINARY_UNUSABLE", meaning: "the binary named by --bin does not exist, is not a file, is not executable, or cannot be started", exit: 2, retryable: false, fix: "Pass the path of a built, executable binary with --bin <BINARY>." },
    CodeInfo { code: "COMMAND_REQUIRED", meaning: "atip was run without a command", exit: 2, retryable: false, fix: "Run atip check --bin <BINARY> --level L0 --fixture <FILE>." },
    CodeInfo { code: "FIXTURE_INVALID", meaning: "the fixture file cannot be read, is not JSON, has a member of the wrong shape, or its setup command failed", exit: 2, retryable: false, fix: "Fix the member the message names, or the setup command, then run atip check again." },
    CodeInfo { code: "INTERNAL", meaning: "a bug in atip, or its scratch space could not be used", exit: 1, retryable: false, fix: "Report it at https://github.com/journeyWorker/atip/issues with the command line; if the message names a scratch directory, point TMPDIR at a writable one." },
    CodeInfo { code: "LEVEL_NOT_MET", meaning: "a MUST check of the level failed or is unprovided", exit: 1, retryable: false, fix: "Read data.checks, fix every check whose result is fail or unprovided, then run atip check again." },
    CodeInfo { code: "LEVEL_UNSUPPORTED", meaning: "the level is known but this release of atip has no checks for it", exit: 2, retryable: false, fix: "Run atip check with --level L0." },
    CodeInfo { code: "UNKNOWN_COMMAND", meaning: "the command word is not a command of atip", exit: 2, retryable: false, fix: "Run atip check; it is the only command." },
    CodeInfo { code: "USAGE", meaning: "the command line does not parse or does not satisfy the command's requirements", exit: 2, retryable: false, fix: "Run atip check --help for the options." },
];

/// The codes `atip` emits, in the order of [`REGISTRY`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    BinaryUnusable,
    CommandRequired,
    FixtureInvalid,
    Internal,
    LevelNotMet,
    LevelUnsupported,
    UnknownCommand,
    Usage,
}

impl Code {
    /// The registry row: the only source of the exit and of `retryable`.
    pub fn info(self) -> &'static CodeInfo {
        &REGISTRY[self as usize]
    }
}

/// A refusal or a failure, before it is printed. Its members are on [`FailureBody`], boxed so that a `Result` carrying one stays small.
#[derive(Debug, Clone)]
pub struct Failure(Box<FailureBody>);

impl std::ops::Deref for Failure {
    type Target = FailureBody;

    fn deref(&self) -> &FailureBody {
        &self.0
    }
}

impl std::ops::DerefMut for Failure {
    fn deref_mut(&mut self) -> &mut FailureBody {
        &mut self.0
    }
}

/// The members of a [`Failure`].
#[derive(Debug, Clone)]
pub struct FailureBody {
    pub code: Code,
    pub message: String,
    /// The command resolved so far (`check`); `None` when not even the first word resolved.
    pub command: Option<&'static str>,
    pub field: Option<String>,
    pub expected: Vec<String>,
    pub received: Option<String>,
    pub suggestions: Vec<String>,
    pub hint: Option<String>,
    /// The partial output of a command that ran (exit 1 only).
    pub data: Option<Value>,
    /// More lines for the text form on stderr, such as the check list; never in the envelope.
    pub detail: Option<String>,
}

impl Failure {
    pub fn new(code: Code, message: impl Into<String>) -> Self {
        Self(Box::new(FailureBody {
            code,
            message: message.into(),
            command: None,
            field: None,
            expected: Vec::new(),
            received: None,
            suggestions: Vec::new(),
            hint: None,
            data: None,
            detail: None,
        }))
    }

    #[must_use]
    pub fn in_command(mut self, command: &'static str) -> Self {
        self.command = Some(command);
        self
    }

    #[must_use]
    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }

    #[must_use]
    pub fn expected<I, S>(mut self, expected: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.expected = expected.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn received(mut self, received: impl Into<String>) -> Self {
        self.received = Some(received.into());
        self
    }

    #[must_use]
    pub fn suggestions<I, S>(mut self, suggestions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.suggestions = suggestions.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    #[must_use]
    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }

    #[must_use]
    pub fn with_detail(mut self, detail: String) -> Self {
        self.detail = Some(detail);
        self
    }

    /// The process exit: the code's registry row, never chosen here.
    pub fn exit(&self) -> u8 {
        self.code.info().exit
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Meta {
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<&'static str>,
    contract: &'static str,
    schema_version: &'static str,
    tool: &'static str,
    tool_version: &'static str,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'static str,
    message: &'a str,
    retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    hint: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<&'a str>,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    expected: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    received: Option<&'a str>,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    suggestions: &'a [String],
}

#[derive(Serialize)]
struct Envelope<'a> {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<&'a Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorBody<'a>>,
    meta: Meta,
}

fn meta(command: Option<&'static str>) -> Meta {
    Meta {
        command,
        contract: "atip/1",
        schema_version: SCHEMA_VERSION,
        tool: TOOL,
        tool_version: env!("CARGO_PKG_VERSION"),
    }
}

/// The success envelope of `atip check`: one compact line.
pub fn success_line(data: &Value) -> String {
    atip::line(&Envelope {
        ok: true,
        data: Some(data),
        error: None,
        meta: meta(Some("check")),
    })
}

/// The failure envelope: one compact line.
pub fn failure_line(failure: &Failure) -> String {
    let error = ErrorBody {
        code: failure.code.info().code,
        message: &failure.message,
        retryable: failure.code.info().retryable,
        hint: failure.hint.as_deref(),
        field: failure.field.as_deref(),
        expected: &failure.expected,
        received: failure.received.as_deref(),
        suggestions: &failure.suggestions,
    };
    atip::line(&Envelope {
        ok: false,
        data: failure.data.as_ref(),
        error: Some(error),
        meta: meta(failure.command),
    })
}

/// The failure as text for stderr (spec 7.4), then its `detail` lines.
pub fn failure_text(failure: &Failure) -> String {
    let mut lines = vec![format!(
        "{TOOL}: {}: {}",
        failure.code.info().code,
        failure.message
    )];
    if let Some(field) = &failure.field {
        lines.push(format!("  field: {field}"));
    }
    if !failure.expected.is_empty() {
        lines.push(format!("  expected: {}", failure.expected.join(", ")));
    }
    if let Some(received) = &failure.received {
        lines.push(format!("  received: {received}"));
    }
    if !failure.suggestions.is_empty() {
        lines.push(format!("  suggestions: {}", failure.suggestions.join(", ")));
    }
    if let Some(hint) = &failure.hint {
        lines.push(format!("  hint: {hint}"));
    }
    if let Some(detail) = &failure.detail {
        lines.extend(detail.lines().map(|line| format!("  {line}")));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::{failure_line, failure_text, success_line, Code, Failure, REGISTRY};

    #[test]
    fn atip_check_06_the_registry_is_sorted_and_each_code_reads_its_own_row() {
        assert!(REGISTRY.windows(2).all(|pair| pair[0].code < pair[1].code));
        for (code, name, exit) in [
            (Code::BinaryUnusable, "BINARY_UNUSABLE", 2),
            (Code::CommandRequired, "COMMAND_REQUIRED", 2),
            (Code::FixtureInvalid, "FIXTURE_INVALID", 2),
            (Code::Internal, "INTERNAL", 1),
            (Code::LevelNotMet, "LEVEL_NOT_MET", 1),
            (Code::LevelUnsupported, "LEVEL_UNSUPPORTED", 2),
            (Code::UnknownCommand, "UNKNOWN_COMMAND", 2),
            (Code::Usage, "USAGE", 2),
        ] {
            assert_eq!((code.info().code, code.info().exit), (name, exit));
            assert_eq!(Failure::new(code, "m").exit(), exit);
        }
        assert!(REGISTRY.iter().all(|row| !row.retryable));
    }

    #[test]
    fn atip_check_03_a_failure_omits_what_it_does_not_have() {
        let line = failure_line(&Failure::new(Code::UnknownCommand, "no such command"));
        assert_eq!(line, r#"{"ok":false,"error":{"code":"UNKNOWN_COMMAND","message":"no such command","retryable":false},"meta":{"contract":"atip/1","schemaVersion":"1.0","tool":"atip","toolVersion":""#.to_string() + env!("CARGO_PKG_VERSION") + r#""}}"#);
        let full = failure_line(
            &Failure::new(Code::Usage, "m")
                .in_command("check")
                .field("--bin")
                .expected(["a"])
                .received("b")
                .suggestions(["a"])
                .hint("h"),
        );
        assert!(
            full.contains(r#""command":"check""#)
                && full.contains(r#""field":"--bin""#)
                && full.contains(r#""expected":["a"]"#)
                && full.contains(r#""hint":"h""#),
            "{full}"
        );
        assert!(!full.contains("null") && !full.contains("[]"));
    }

    #[test]
    fn atip_check_05_the_text_form_is_code_message_then_the_members_that_exist() {
        let text = failure_text(
            &Failure::new(Code::Usage, "bad")
                .field("--bin")
                .hint("pass it")
                .with_detail("line one\nline two".to_string()),
        );
        assert_eq!(
            text,
            "atip: USAGE: bad\n  field: --bin\n  hint: pass it\n  line one\n  line two"
        );
        assert!(success_line(&serde_json::json!({"level": "L0"})).contains(r#""command":"check""#));
    }
}
