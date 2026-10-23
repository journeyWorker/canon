//! The one error contract every `canon` command speaks (0.20 D3, 0.28 D3).
//!
//! A refusal or an operational failure is a [`CliError`]: a stable
//! SCREAMING_SNAKE `code` (the thing a caller branches on), a `message`, an
//! optional `hint` (a working invocation or the next step), whether a retry
//! can succeed, and the process exit code. The exit code and `retryable` are
//! not chosen at the site that builds the error: they come from the code's row
//! in the registry ([`all_codes`]), so one code has one exit everywhere. The
//! few sites with a documented per-site exit say so with
//! [`CliError::with_exit`].
//! [`CliError::emit`] is the single rendering path:
//!
//! - human mode, on stderr: `canon <command path>: <CODE> — <message>`, then
//!   an optional `hint: <hint>` line;
//! - JSON mode, on stdout and nothing on stderr: the failure envelope
//!   `{"ok":false,"error":{…},"meta":{…}}` ([`crate::json_out`]). `error` is
//!   [`ErrorBody`]: `code`, `message`, then `field` (the argument or JSON
//!   pointer refused), `expected` (the valid set when there is one),
//!   `received` (the refused value, secrets redacted), `suggestions`, `hint`
//!   (`null` when there is none) and `retryable`.
//!
//! Exit codes (plan D4): `0` ok; `1` the command's finding, a failed check or
//! an operational failure ([`EXIT_FAILED`]); `2` a refusal: usage,
//! `CONFIG_MISSING`, an id or file that does not exist, unreadable input, an
//! invalid argument ([`EXIT_REFUSED`]). Every `*_NOT_FOUND`, `*_UNREADABLE`
//! and `*_INVALID_ARGUMENT` code is `2`.
//!
//! [`all_codes`] is the registry of every code with its meaning, exit and
//! retryability; a test keeps it in step with the codes the sources
//! construct.

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use crate::json_out::Report;

/// A finding, a failed check or an operational failure.
pub const EXIT_FAILED: i32 = 1;
/// A refusal or a usage error.
pub const EXIT_REFUSED: i32 = 2;

/// One refusal or operational failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    pub code: &'static str,
    pub message: String,
    pub hint: Option<String>,
    /// Whether running the same command again, unchanged, can succeed
    /// (the code's registry row).
    pub retryable: bool,
    pub exit: i32,
    /// The argument or record field that was refused (`--to`, `--kind`).
    pub field: Option<String>,
    /// The values `field` accepts, when the refusal is about a closed set.
    pub expected: Option<Vec<String>>,
    /// The refused value as given; redacted when it is rendered ([`redact`]).
    pub received: Option<String>,
    /// The nearest valid words, closest first ([`crate::suggest`]).
    pub suggestions: Vec<String>,
}

/// The `error` member of a failure envelope. Field order is the document
/// order: `code` first.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorBody {
    /// Stable SCREAMING_SNAKE code, listed by `canon schema --errors`.
    pub code: String,
    pub message: String,
    /// The refused argv name (`--scenario-id`) or an RFC 6901 JSON pointer into `--input`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// The valid values of `field`, when it takes a closed set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<Vec<String>>,
    /// The value that was refused, with secrets redacted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub received: Option<String>,
    /// The nearest valid commands, options or values (a destructive command only within edit distance 1).
    pub suggestions: Vec<String>,
    /// A working invocation or the next step; `null` when there is none.
    pub hint: Option<String>,
    /// True only where running the same command again can succeed.
    pub retryable: bool,
}

impl CliError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        let (exit, retryable) = code_info(code).map_or((EXIT_REFUSED, false), |info| (info.exit, info.retryable));
        Self { code, message: message.into(), hint: None, retryable, exit, field: None, expected: None, received: None, suggestions: Vec::new() }
    }

    /// The command was understood and declined. The exit comes from the
    /// code's registry row, as for [`CliError::failed`]; the two names say
    /// what the site means.
    pub fn refused(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(code, message)
    }

    /// The command could not do its work. The exit comes from the code's
    /// registry row.
    pub fn failed(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(code, message)
    }

    /// The next step the caller can take.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Names the refused argument or field (`--to`, `--kind`).
    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }

    /// Lists the values the refused field accepts.
    pub fn expected<I, S>(mut self, expected: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.expected = Some(expected.into_iter().map(Into::into).collect());
        self
    }

    /// The value that was refused. It is redacted when rendered.
    pub fn received(mut self, received: impl Into<String>) -> Self {
        self.received = Some(received.into());
        self
    }

    /// Replaces the nearest-word suggestions.
    pub fn suggestions<I, S>(mut self, suggestions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.suggestions = suggestions.into_iter().map(Into::into).collect();
        self
    }

    /// Overrides the registry's exit for one documented site: a batch whose
    /// validation refuses (`2`) and whose write fails later (`1`) is one code
    /// with two exits (`EVIDENCE_BATCH_REFUSED`).
    pub fn with_exit(mut self, exit: i32) -> Self {
        self.exit = exit;
        self
    }

    /// The stderr text: `canon <command>: <CODE> — <message>` and an
    /// optional `hint:` line. `command` is the full prefix, e.g.
    /// `canon evidence add`.
    pub fn human(&self, command: &str) -> String {
        match &self.hint {
            Some(hint) => format!("{command}: {} — {}\nhint: {hint}", self.code, self.message),
            None => format!("{command}: {} — {}", self.code, self.message),
        }
    }

    /// The `error` member of the failure envelope.
    pub fn body(&self) -> ErrorBody {
        let suggestions = match (&self.expected, &self.received) {
            (Some(expected), Some(received)) if self.suggestions.is_empty() => crate::suggest::nearest(received, expected.iter().map(String::as_str)),
            _ => self.suggestions.clone(),
        };
        ErrorBody {
            code: self.code.to_string(),
            message: self.message.clone(),
            field: self.field.clone(),
            expected: self.expected.clone(),
            received: self.received.as_deref().map(|received| redact(self.field.as_deref(), received)),
            suggestions,
            hint: self.hint.clone(),
            retryable: self.retryable,
        }
    }

    /// The failure envelope for `command`, as a value: for a caller that
    /// answers over a socket instead of stdout.
    pub fn envelope(&self, command: &str) -> Value {
        crate::json_out::failure_value::<crate::json_out::Untyped>(command, self, None)
    }

    /// Renders the error (see the module docs) and returns the exit code.
    pub fn emit(&self, command: &str, json: bool) -> i32 {
        debug_assert!(is_registered(self.code), "error code {} is not in cli_error::all_codes()", self.code);
        if json {
            crate::json_out::print_failure::<crate::json_out::Untyped>(command, self, None);
        } else {
            eprintln!("{}", self.human(command));
        }
        self.exit
    }

    /// [`CliError::emit`] for a failure that also has a report worth reading,
    /// such as the violations a failed gate found. The caller has already
    /// printed that report in its human form, so human mode adds nothing: the
    /// exit code is the whole signal there. In JSON mode the one document
    /// carries `error` (the branch point), `data` (the report) and `meta`.
    pub fn emit_with_data<R: Report>(&self, command: &str, json: bool, data: &R) -> i32 {
        debug_assert!(is_registered(self.code), "error code {} is not in cli_error::all_codes()", self.code);
        if json {
            crate::json_out::print_failure(command, self, Some(data));
        }
        self.exit
    }
}

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

/// A clap value parser for an argument that takes one of a closed set of
/// words. clap then knows the set: a bad value is refused with
/// `possible values`, `canon schema` lists them as `possibleValues`, and the
/// `--json` usage error carries them as `expected`.
pub fn closed_set<T: Clone + Send + Sync + 'static>(values: impl Into<clap::builder::PossibleValuesParser>, parse: fn(&str) -> Result<T, String>) -> impl clap::builder::TypedValueParser<Value = T> {
    use clap::builder::TypedValueParser;
    clap::builder::PossibleValuesParser::new(values).try_map(move |word: String| parse(&word))
}

fn is_registered(code: &str) -> bool {
    code_info(code).is_some()
}

/// One row of the error registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeInfo {
    pub code: &'static str,
    pub meaning: &'static str,
    /// The process exit code the code carries (`1` or `2`).
    pub exit: i32,
    /// True only where running the same command again can succeed.
    pub retryable: bool,
}

/// The registry row of `code`, if it has one.
pub fn code_info(code: &str) -> Option<&'static CodeInfo> {
    CODES.binary_search_by(|info| info.code.cmp(code)).ok().map(|index| &CODES[index])
}

/// Every error code a command can emit, with its meaning, exit and
/// retryability, sorted by code. A code appears once; `tests/cli_error.rs`
/// keeps this in step with the codes the sources construct.
pub fn all_codes() -> &'static [CodeInfo] {
    CODES
}

const CODES: &[CodeInfo] = &[
    CodeInfo { code: "ADAPTER_EXECUTION_NOT_AUTHORIZED", meaning: "the adapter response did not authorize execution", exit: 1, retryable: false },
    CodeInfo { code: "ADAPTER_REFUSED", meaning: "an adapter response was malformed or refused", exit: 1, retryable: false },
    CodeInfo { code: "APPROVAL_INCOMPLETE", meaning: "`brief approve` got an empty or multi-line --by or --note", exit: 2, retryable: false },
    CodeInfo { code: "ARTIFACTS_MIGRATE_REFUSED", meaning: "`artifacts migrate` cannot move the blobs: the target is not an s3 tier, a local blob does not match its name, or the object store holds a bad copy", exit: 2, retryable: false },
    CodeInfo { code: "ARTIFACTS_STORE_UNREACHABLE", meaning: "the artifact store tier cannot be attached or reached, so no blob can be verified or uploaded", exit: 2, retryable: true },
    CodeInfo { code: "ARTIFACTS_VERIFY_FAILED", meaning: "a bound file is missing from the artifact store or altered; data.problems names each", exit: 1, retryable: false },
    CodeInfo { code: "BRIEF_CHANGED_SINCE_APPROVAL", meaning: "the brief was edited after the human approved it, so it reads as not approved until approved again", exit: 1, retryable: false },
    CodeInfo { code: "BRIEF_CHANGED_SINCE_PAGE_LOAD", meaning: "the dashboard was asked to approve a brief whose file differs from the copy the page showed the human; nothing was recorded", exit: 2, retryable: false },
    CodeInfo { code: "BRIEF_HAS_BLOCKERS", meaning: "the brief has blocker findings, so it cannot be approved yet", exit: 1, retryable: false },
    CodeInfo { code: "BRIEF_NOT_FOUND", meaning: "the named change has no brief", exit: 2, retryable: false },
    CodeInfo { code: "BRIEF_UNAPPROVABLE", meaning: "the brief's text cannot take an `Approved:` line (for example an open code fence)", exit: 2, retryable: false },
    CodeInfo { code: "BRIEF_UNAPPROVED", meaning: "an attestation was refused because the subject's brief has no human approval", exit: 1, retryable: false },
    CodeInfo { code: "BRIEF_UNREADABLE", meaning: "the brief file could not be read", exit: 2, retryable: false },
    CodeInfo { code: "BRIEF_WRITE_FAILED", meaning: "the brief file could not be written", exit: 1, retryable: false },
    CodeInfo { code: "CHANGE_ADOPTION_INCOMPLETE", meaning: "the change was written but its subject link was not; the hint names the repair", exit: 2, retryable: false },
    CodeInfo { code: "CHANGE_AMBIGUOUS", meaning: "several live changes have a brief; pass --change-id <slug>", exit: 2, retryable: false },
    CodeInfo { code: "CHANGE_EXISTS", meaning: "a change (or its archive) already uses that slug", exit: 2, retryable: false },
    CodeInfo { code: "CHANGE_INVALID_ARGUMENT", meaning: "`change new` could not build a brief from the given arguments", exit: 2, retryable: false },
    CodeInfo { code: "CONFIG_MISSING", meaning: "canon.yaml does not exist", exit: 2, retryable: false },
    CodeInfo { code: "CONFIG_UNREADABLE", meaning: "canon.yaml, policy or a setting it carries could not be read or is invalid", exit: 2, retryable: false },
    CodeInfo { code: "CONTEXT_GIT_UNAVAILABLE", meaning: "a context pack needs a git identity that is not available", exit: 1, retryable: false },
    CodeInfo { code: "CONTEXT_INPUT_MISSING", meaning: "a selected context input does not exist", exit: 2, retryable: false },
    CodeInfo { code: "CONTEXT_MANIFEST_INVALID", meaning: "the context input manifest has an unsupported version", exit: 2, retryable: false },
    CodeInfo { code: "CONTEXT_OBJECT_CONFLICT", meaning: "an immutable context object already holds different bytes", exit: 1, retryable: false },
    CodeInfo { code: "CONTEXT_PACK_NOT_FOUND", meaning: "no context pack with that id", exit: 2, retryable: false },
    CodeInfo { code: "CONTEXT_PACK_TAMPERED", meaning: "a context pack is stale or was changed after it was created", exit: 1, retryable: false },
    CodeInfo { code: "CONTEXT_SECRET_DETECTED", meaning: "a selected context input contains plaintext secret material", exit: 2, retryable: false },
    CodeInfo { code: "CONTEXT_UNSAFE_PATH", meaning: "a selected path is absolute, leaves the repository, or contains a symlink", exit: 2, retryable: false },
    CodeInfo { code: "DASHBOARD_BAD_REQUEST", meaning: "a dashboard request was not the one the briefs page sends: wrong method, content type, length or body", exit: 2, retryable: false },
    CodeInfo { code: "DASHBOARD_CONFIG_INVALID", meaning: "the dashboard's canon.yaml section is present but unusable", exit: 2, retryable: false },
    CodeInfo { code: "DASHBOARD_FAILED", meaning: "the dashboard could not start for a reason that is not one of the other DASHBOARD_ codes (an I/O failure)", exit: 1, retryable: false },
    CodeInfo { code: "DASHBOARD_NOT_BUILT", meaning: "this canon binary embeds no dashboard app and the repo holds no build of canon's own", exit: 1, retryable: false },
    CodeInfo { code: "DASHBOARD_ORIGIN_REFUSED", meaning: "a dashboard request came with a Host or Origin that is not the bound address, so it was refused before anything was read", exit: 2, retryable: false },
    CodeInfo { code: "DASHBOARD_PORT_IN_USE", meaning: "the dashboard's port is already bound by another process", exit: 1, retryable: true },
    CodeInfo { code: "DEMO_NOT_INITIALIZED", meaning: "`demo add` ran before `demo init`", exit: 2, retryable: false },
    CodeInfo { code: "DISPATCH_ALREADY_ENDED", meaning: "the dispatch run was already ended", exit: 2, retryable: false },
    CodeInfo { code: "DISPATCH_END_IN_PROGRESS", meaning: "another process is closing the run; retry", exit: 1, retryable: true },
    CodeInfo { code: "DISPATCH_INVALID_ARGUMENT", meaning: "a dispatch flag combination is invalid", exit: 2, retryable: false },
    CodeInfo { code: "DISPATCH_MANIFEST_MISMATCH", meaning: "the manifest's run id differs from its file name", exit: 2, retryable: false },
    CodeInfo { code: "DISPATCH_MANIFEST_UNREADABLE", meaning: "the dispatch manifest is not a readable run record", exit: 2, retryable: false },
    CodeInfo { code: "DISPATCH_NOT_RUNNING", meaning: "the run's recorded status cannot be closed", exit: 2, retryable: false },
    CodeInfo { code: "DISPATCH_ROLE_REGIME_MISMATCH", meaning: "--role does not match the --regime's role", exit: 2, retryable: false },
    CodeInfo { code: "DISPATCH_RUN_NOT_FOUND", meaning: "no dispatch manifest for that run id", exit: 2, retryable: false },
    CodeInfo { code: "DIVERGENCE_ACCEPT_REFUSED", meaning: "a divergence acceptance was refused: not a reviewer, the evidence is not divergent, or it was attested by the accepting actor or session", exit: 2, retryable: false },
    CodeInfo { code: "DIVERGENCE_INVALID_ARGUMENT", meaning: "a divergence flag value is invalid", exit: 2, retryable: false },
    CodeInfo { code: "DIVERGENCE_REFUSED", meaning: "a divergence candidate was refused by a gate rule", exit: 1, retryable: false },
    CodeInfo { code: "EVIDENCE_APPROVAL_INVALID", meaning: "the human approval attached to an attestation is missing, malformed or fails verification", exit: 2, retryable: false },
    CodeInfo { code: "EVIDENCE_BATCH_REFUSED", meaning: "a batch attestation was refused and none of it was written", exit: 2, retryable: false },
    CodeInfo { code: "EVIDENCE_BIND_REFUSED", meaning: "a --report or --file binding was refused", exit: 2, retryable: false },
    CodeInfo { code: "EVIDENCE_CONTRACT_MISMATCH", meaning: "the record's kind and ref differ from the task's typed evidence contract", exit: 1, retryable: false },
    CodeInfo { code: "EVIDENCE_FABRICATED", meaning: "the note carries a fabrication marker or an unsupported verified claim", exit: 1, retryable: false },
    CodeInfo { code: "EVIDENCE_INVALID_ARGUMENT", meaning: "an `evidence add` argument is empty, malformed or contradicts another", exit: 2, retryable: false },
    CodeInfo { code: "EVIDENCE_REFUSED", meaning: "an attestation was refused by an evidence rule", exit: 2, retryable: false },
    CodeInfo { code: "EVIDENCE_REPORT_CONTRADICTION", meaning: "the bound report contradicts the verdict", exit: 1, retryable: false },
    CodeInfo { code: "EVIDENCE_STORE_FAILED", meaning: "a bound file could not be stored in the artifact store", exit: 2, retryable: true },
    CodeInfo { code: "EVIDENCE_VAULT_FAILED", meaning: "the evidence vault could not write a blob", exit: 2, retryable: false },
    CodeInfo { code: "FEATURE_EXISTS", meaning: "the feature file already exists", exit: 2, retryable: false },
    CodeInfo { code: "FEATURE_OUTSIDE_SPEC_ROOTS", meaning: "the feature path is under no configured specs.roots entry", exit: 2, retryable: false },
    CodeInfo { code: "FINDING_INVALID_ARGUMENT", meaning: "a `finding` argument is empty, malformed or out of range", exit: 2, retryable: false },
    CodeInfo { code: "FINDING_NOT_FOUND", meaning: "no committed finding at that change, round and seq", exit: 2, retryable: false },
    CodeInfo { code: "FINDING_REASON_REQUIRED", meaning: "closing a finding as rejected or deferred needs a one-line --reason", exit: 2, retryable: false },
    CodeInfo { code: "FINDING_RESOLUTION_SHA_REQUIRED", meaning: "a fixed finding needs --resolution-sha", exit: 2, retryable: false },
    CodeInfo { code: "FINDING_RESOLUTION_SHA_UNEXPECTED", meaning: "--resolution-sha was given with a disposition other than fixed", exit: 2, retryable: false },
    CodeInfo { code: "FINDING_SEQ_OCCUPIED", meaning: "a finding already holds that change, round and seq", exit: 2, retryable: false },
    CodeInfo { code: "FINDING_STAGED", meaning: "the finding is still staged and cannot be closed until committed", exit: 2, retryable: false },
    CodeInfo { code: "FINDING_UNKNOWN_SCENARIO", meaning: "--scenario-id names a scenario the index does not know", exit: 2, retryable: false },
    CodeInfo { code: "FINDING_UNRESOLVABLE_COMMIT", meaning: "a commit reference names no commit in this repository", exit: 2, retryable: false },
    CodeInfo { code: "FORMAT_VIOLATIONS", meaning: "`format` found spec-corpus violations; data.violations names each", exit: 1, retryable: false },
    CodeInfo { code: "GATE_TASK_BLOCKED", meaning: "`gate task` refused to flip the task: its evidence does not support it", exit: 1, retryable: false },
    CodeInfo { code: "GATE_TASK_EVIDENCE_INVALID", meaning: "the evidence or typed atom for the task is invalid", exit: 1, retryable: false },
    CodeInfo { code: "GATE_TASK_INVALID_ARGUMENT", meaning: "the task id is malformed", exit: 2, retryable: false },
    CodeInfo { code: "GATE_TASK_UNSUPPORTED", meaning: "the plan dialect cannot flip this task", exit: 2, retryable: false },
    CodeInfo { code: "GATE_VIOLATIONS", meaning: "the evidence gate found violations; data.violations names each", exit: 1, retryable: false },
    CodeInfo { code: "HOOKS_INSTALL_FAILED", meaning: "a hook entry could not be installed", exit: 2, retryable: false },
    CodeInfo { code: "INGEST_FAILED", meaning: "an ingest command failed", exit: 1, retryable: false },
    CodeInfo { code: "INGEST_SOURCE_UNUSABLE", meaning: "an `ingest plans` source was malformed and persisted nothing; check its root", exit: 1, retryable: false },
    CodeInfo { code: "INIT_AGENTS_MD_CONFLICT", meaning: "AGENTS.md cannot take the canon block without overwriting content", exit: 2, retryable: false },
    CodeInfo { code: "INIT_CONFIG_EXISTS", meaning: "canon.yaml already exists and there is nothing else to refresh", exit: 2, retryable: false },
    CodeInfo { code: "INIT_INVALID_ARGUMENT", meaning: "an `init` option is invalid (language or skill providers)", exit: 2, retryable: false },
    CodeInfo { code: "INIT_SKILL_INSTALL_FAILED", meaning: "canon.yaml was written but the canon skill was not installed", exit: 2, retryable: false },
    CodeInfo { code: "INVALID_CURSOR", meaning: "`--cursor` is not one this listing issued, or the record it points after is gone", exit: 2, retryable: false },
    CodeInfo { code: "INVENTORY_ABORTED", meaning: "`inventory sync` aborted a spec root on violations or duplicate scenarios; data.roots names each", exit: 1, retryable: false },
    CodeInfo { code: "INVENTORY_FAILED", meaning: "the scenario index could not be built", exit: 2, retryable: false },
    CodeInfo { code: "LAYOUT_MIGRATION_REQUIRED", meaning: "the spec root uses a pre-0.17 layout; run `canon migrate`", exit: 2, retryable: false },
    CodeInfo { code: "LEARN_DEMOTION_FAILED", meaning: "promotion is blocked and the demotion a contradicting trajectory triggered also failed; the strategy is still active", exit: 1, retryable: false },
    CodeInfo { code: "LEARN_EVALUATION_REJECTED", meaning: "the promotion evaluation does not fit the candidate strategy", exit: 1, retryable: false },
    CodeInfo { code: "LEARN_INVALID_ARGUMENT", meaning: "a `learn` flag value is invalid", exit: 2, retryable: false },
    CodeInfo { code: "LEARN_INVALID_INPUT", meaning: "a `learn` evaluation or approval file is invalid", exit: 2, retryable: false },
    CodeInfo { code: "LEARN_PROMOTION_BLOCKED", meaning: "the promotion gate blocks this strategy", exit: 1, retryable: false },
    CodeInfo { code: "LEARN_PROMOTION_FAILED", meaning: "the strategy could not be promoted", exit: 1, retryable: false },
    CodeInfo { code: "LEARN_ROLLBACK_FAILED", meaning: "the strategy could not be rolled back", exit: 1, retryable: false },
    CodeInfo { code: "LEARN_SIGNATURE_UNREADABLE", meaning: "the detached signature file could not be read", exit: 2, retryable: false },
    CodeInfo { code: "LEARN_STRATEGY_NOT_FOUND", meaning: "no strategy with that id", exit: 2, retryable: false },
    CodeInfo { code: "LEARN_TRAJECTORY_NOT_FOUND", meaning: "the contradicting trajectory does not exist", exit: 2, retryable: false },
    CodeInfo { code: "LEDGER_ARCHIVE_UNAVAILABLE", meaning: "`ledger compact` found no archive: the ledger root is not a directory named `ledger`", exit: 2, retryable: false },
    CodeInfo { code: "LEDGER_COMPACT_FAILED", meaning: "a superseded version could not be moved to the archive; what moved stays moved", exit: 1, retryable: false },
    CodeInfo { code: "MIGRATE_INDEX_FAILED", meaning: "files moved but the scenario index was not rebuilt; run `canon inventory sync`", exit: 1, retryable: false },
    CodeInfo { code: "MIGRATE_PINS_FAILED", meaning: "files moved but the reviews were not pinned to what their evidence says; run `canon migrate` again", exit: 2, retryable: false },
    CodeInfo { code: "MIGRATE_REFUSED", meaning: "`migrate` refused: the repository's layout or config cannot be moved safely", exit: 2, retryable: false },
    CodeInfo { code: "MIGRATE_STOPPED", meaning: "`migrate` stopped partway; running it again finishes the rest", exit: 2, retryable: false },
    CodeInfo { code: "MIGRATE_UNCOMMITTED", meaning: "`migrate` refused because paths it moves have uncommitted changes", exit: 2, retryable: false },
    CodeInfo { code: "NO_BRIEF", meaning: "no live change has a brief", exit: 2, retryable: false },
    CodeInfo { code: "PLAN_CHANGED", meaning: "the data changed since `purge --plan`, so the confirmed digest no longer matches; nothing was deleted", exit: 2, retryable: false },
    CodeInfo { code: "PLAN_CORPUS_UNREADABLE", meaning: "the plan corpus could not be read to resolve a task", exit: 2, retryable: false },
    CodeInfo { code: "PLAN_SOURCES_MISSING", meaning: "the repository configures no plan sources", exit: 2, retryable: false },
    CodeInfo { code: "PLUGIN_SYNC_ERRORS", meaning: "`plugin sync` could not write some candidates; data.roots names each error", exit: 1, retryable: false },
    CodeInfo { code: "PLUGIN_SYNC_FAILED", meaning: "plugin sync failed", exit: 2, retryable: false },
    CodeInfo { code: "POLICY_APPROVAL_FAILED", meaning: "the policy approval could not be recorded", exit: 2, retryable: false },
    CodeInfo { code: "POLICY_APPROVAL_UNREADABLE", meaning: "`.canon/policy.approval.json` cannot be read, so the policy in force cannot be checked against its approval; shipping or reopening a subject refuses, and `--override-reason` does not waive it", exit: 2, retryable: false },
    CodeInfo { code: "POLICY_CHANGED_SINCE_APPROVAL", meaning: "`.canon/policy.yaml` differs from the bytes a human approved; shipping or reopening a subject refuses until it is re-approved or restored, and `--override-reason` does not waive it", exit: 1, retryable: false },
    CodeInfo { code: "PROMOTE_FAILED", meaning: "a staged record was written but promoting it to the ledger failed", exit: 1, retryable: false },
    CodeInfo { code: "PROMOTE_REFUSED", meaning: "`gate promote` refused one or more staged records; data.refused names each", exit: 1, retryable: false },
    CodeInfo { code: "PROMPT_BUNDLE_INVALID", meaning: "a prompt bundle is invalid", exit: 2, retryable: false },
    CodeInfo { code: "PURGE_PLAN_REQUIRED", meaning: "`purge` deletes only a reviewed plan: run `purge --plan`, then `purge --confirm <planDigest>`", exit: 2, retryable: false },
    CodeInfo { code: "QUERY_FAILED", meaning: "the query could not read the ledger", exit: 1, retryable: false },
    CodeInfo { code: "QUERY_INVALID_ARGUMENT", meaning: "a query flag is invalid for this kind", exit: 2, retryable: false },
    CodeInfo { code: "REGIME_KEY_INVALID", meaning: "a regime key part is malformed", exit: 2, retryable: false },
    CodeInfo { code: "REPORT_FAILED", meaning: "the report could not be built or checked", exit: 1, retryable: false },
    CodeInfo { code: "REPORT_STALE", meaning: "`report --check` found the report missing or out of date", exit: 1, retryable: false },
    CodeInfo { code: "REPO_UNREADABLE", meaning: "the repository, its ledger or an input file could not be read", exit: 2, retryable: false },
    CodeInfo { code: "RETENTION_EXPORT_FAILED", meaning: "the export failed", exit: 2, retryable: false },
    CodeInfo { code: "RETENTION_PURGE_FAILED", meaning: "the purge failed or stopped partway", exit: 2, retryable: false },
    CodeInfo { code: "RETRIEVE_FAILED", meaning: "retrieval failed", exit: 2, retryable: false },
    CodeInfo { code: "REVIEW_ARTIFACT_REFUSED", meaning: "a review's artifact binding was refused", exit: 2, retryable: false },
    CodeInfo { code: "REVIEW_INVALID_ARGUMENT", meaning: "a `review add` argument is missing or contradicts another", exit: 2, retryable: false },
    CodeInfo { code: "REVIEW_SCENARIO_NOT_INDEXED", meaning: "a reviewed scenario has no indexed record; run `canon inventory sync`", exit: 2, retryable: false },
    CodeInfo { code: "SCAFFOLD_INVALID_ARGUMENT", meaning: "a scaffold argument is invalid", exit: 2, retryable: false },
    CodeInfo { code: "SCENARIO_TAG_EXISTS", meaning: "the scenario tag already exists", exit: 2, retryable: false },
    CodeInfo { code: "SELFTEST_FAILED", meaning: "a selftest suite or fixture did not match its expected result", exit: 1, retryable: false },
    CodeInfo { code: "SERIALIZE_FAILED", meaning: "the result could not be serialized", exit: 1, retryable: false },
    CodeInfo { code: "SKILLS_DRIFT", meaning: "`skills check` found an installed skill missing, stale, a legacy remnant, an orphan the bundle no longer ships, or written by another canon version", exit: 1, retryable: false },
    CodeInfo { code: "SKILLS_FAILED", meaning: "a skills command failed", exit: 1, retryable: false },
    CodeInfo { code: "SPEC_ROOTS_UNREADABLE", meaning: "the scenario files a brief's flow steps point at could not be read", exit: 2, retryable: false },
    CodeInfo { code: "SPEC_ROOT_UNRESOLVED", meaning: "no single spec root could be chosen; pass --project-id", exit: 2, retryable: false },
    CodeInfo { code: "SUBJECT_ADOPT_REFUSED", meaning: "the change could not be adopted by the subject", exit: 2, retryable: false },
    CodeInfo { code: "SUBJECT_EXISTS", meaning: "a subject with that id already exists", exit: 2, retryable: false },
    CodeInfo { code: "SUBJECT_GATE_VIOLATION", meaning: "the status transition is blocked by gate violations", exit: 1, retryable: false },
    CodeInfo { code: "SUBJECT_INVALID_ARGUMENT", meaning: "a `subject` argument is malformed", exit: 2, retryable: false },
    CodeInfo { code: "SUBJECT_INVALID_TRANSITION", meaning: "the status change is not an allowed transition", exit: 2, retryable: false },
    CodeInfo { code: "SUBJECT_NOT_FOUND", meaning: "no subject with that id", exit: 2, retryable: false },
    CodeInfo { code: "SUBJECT_REASON_REQUIRED", meaning: "reopening a shipped subject needs --reason", exit: 2, retryable: false },
    CodeInfo { code: "SUBJECT_UNADOPT_REFUSED", meaning: "the change carries an approved brief, or a scenario the subject owns is tagged `@flow:<change>.*`, so it stays adopted", exit: 2, retryable: false },
    CodeInfo { code: "TASK_NOT_FOUND", meaning: "the task id names no task in the plan corpus", exit: 2, retryable: false },
    CodeInfo { code: "TASK_ROW_NOT_FOUND", meaning: "the task's row is missing from its plan document", exit: 2, retryable: false },
    CodeInfo { code: "TIER_FAILED", meaning: "tier aging failed", exit: 1, retryable: false },
    CodeInfo { code: "TYPED_ATOMS_INVALID", meaning: "the typed-atoms file for the change does not parse", exit: 2, retryable: false },
    CodeInfo { code: "UNKNOWN_COMMAND", meaning: "`canon schema` was given a command path canon does not have", exit: 2, retryable: false },
    CodeInfo { code: "UNKNOWN_FIELD", meaning: "`--fields` names a field the records do not have", exit: 2, retryable: false },
    CodeInfo { code: "UNKNOWN_ROLE", meaning: "`CANON_ROLE` holds a value that is not a role (planner, implementer, reviewer, orchestrator)", exit: 2, retryable: false },
    CodeInfo { code: "UNKNOWN_SPEC_ROOT", meaning: "`--project-id` names no spec root configured in canon.yaml, so nothing was written", exit: 2, retryable: false },
    CodeInfo { code: "USAGE", meaning: "the command line did not parse: an unknown flag or subcommand, or a missing or malformed argument", exit: 2, retryable: false },
    CodeInfo { code: "WRITE_FAILED", meaning: "a file or ledger record could not be written", exit: 2, retryable: false },
];
