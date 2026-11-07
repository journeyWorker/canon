//! The one error contract every `canon` command speaks (0.20 D3, 0.28 D3).
//!
//! A refusal or an operational failure is a [`CliError`]: a stable
//! SCREAMING_SNAKE `code` (the thing a caller branches on), a `message`, an
//! optional `hint` (a working invocation or the next step), whether a retry
//! can succeed, and the process exit code. The error value, its builder, the
//! redaction of `received`, the row type [`CodeInfo`] and the exit constants
//! are the `atip` crate's; this module keeps canon's side of the contract:
//! the table of rows ([`all_codes`]) and the rendering that needs canon's
//! envelope ([`CliErrorExt`]).
//!
//! The exit code and `retryable` are not chosen at the site that builds the
//! error: they come from the code's row in the registry ([`all_codes`]), so
//! one code has one exit everywhere. Nothing overrides it at a call site
//! (spec 7.3.2): a situation that needs another exit needs another code.
//! [`CliErrorExt::emit`] is the single rendering path:
//!
//! - text mode, on stderr: `canon: <CODE>: <message>`, then indented `field:`,
//!   `expected:`, `suggestions:` and `hint:` lines for the members the error has;
//! - JSON mode, on stdout and nothing on stderr: the failure envelope
//!   `{"ok":false,"error":{…},"meta":{…}}` ([`crate::json_out`]). `error` is
//!   [`ErrorBody`]: `code`, `reason`, `message`, `retryable`, then `field` (the argument
//!   or JSON pointer refused), `expected` (the valid set when there is one),
//!   `received` (the refused value, secrets redacted), `suggestions` and
//!   `hint`; a member with no value is absent.
//!
//! Exit codes: `0` ok; `1` the command ran and its answer is negative or
//! incomplete: a finding, a failed check, a failure after the work began, an
//! internal error ([`EXIT_FAILED`]); `2` the command refused to run: usage,
//! `CONFIG_REQUIRED`, an id or file that does not exist, unreadable input, an
//! invalid argument, a target that cannot be written ([`EXIT_REFUSED`]). Every
//! `*_NOT_FOUND`, `*_UNREADABLE` and `*_INVALID_ARGUMENT` code is `2`.
//!
//! [`all_codes`] is the registry of every code with its meaning, exit and
//! retryability, and the reasons registered under it; a test keeps it in step
//! with the codes the sources construct. [`all_warnings`] is the registry of
//! the codes said as warnings (`meta.warnings`); a code that is both is in
//! both, with the same meaning (spec 7.5.3).
//!
//! The methods of [`CliErrorExt`] are called as methods of [`CliError`]; a file
//! that builds or renders an error imports the trait (`CliErrorExt as _` when it
//! does not name it).

use serde_json::Value;

pub use atip::{CliError, CodeInfo, ErrorBody, ReasonInfo, WarningInfo, EXIT_FAILED, EXIT_REFUSED};

use crate::json_out::Report;

/// What canon adds to [`CliError`]: the constructors that read canon's code
/// table, and the renderings that print canon's envelope.
pub trait CliErrorExt {
    /// The command was understood and declined. The exit comes from the
    /// code's registry row, as for [`CliErrorExt::failed`]; the two names say
    /// what the site means.
    fn refused(code: &'static str, message: impl Into<String>) -> CliError;

    /// The command could not do its work. The exit comes from the code's
    /// registry row.
    fn failed(code: &'static str, message: impl Into<String>) -> CliError;

    /// The `error` member of the failure envelope.
    fn body(&self) -> ErrorBody;

    /// The failure envelope for `command`, as a value: for a caller that
    /// answers over a socket instead of stdout.
    fn envelope(&self, command: &str) -> Value;

    /// The text-mode rendering: `canon: <CODE>: <message>` and the member lines (see the module docs).
    fn human_text(&self) -> String;

    /// Renders the error (see the module docs) and returns the exit code.
    fn emit(&self, command: &str, json: bool) -> u8;

    /// [`CliErrorExt::emit`] for a failure that also has a report worth reading,
    /// such as the violations a failed gate found. The caller has already
    /// printed that report in its human form, so human mode adds nothing: the
    /// exit code is the whole signal there. In JSON mode the one document
    /// carries `error` (the branch point), `data` (the report) and `meta`.
    fn emit_with_data<R: Report>(&self, command: &str, json: bool, data: &R) -> u8;

    /// [`CliErrorExt::emit_with_data`] with extra `meta` keys beside `command`
    /// (`coverage`, …).
    fn emit_with_data_and_meta<R: Report>(&self, command: &str, json: bool, data: &R, extra_meta: Value) -> u8;
}

impl CliErrorExt for CliError {
    fn refused(code: &'static str, message: impl Into<String>) -> CliError {
        CliError::from_row(code, message, code_info(code))
    }

    fn failed(code: &'static str, message: impl Into<String>) -> CliError {
        CliError::from_row(code, message, code_info(code))
    }

    fn body(&self) -> ErrorBody {
        let suggestions = match (self.expected_values(), self.received_value()) {
            (Some(expected), Some(received)) if self.suggested().is_empty() => crate::suggest::nearest(received, expected.iter().map(String::as_str)),
            _ => self.suggested().to_vec(),
        };
        let field = self.field_name().map(|field| crate::input::field_for_body(field).into_owned());
        self.body_with(field, suggestions)
    }

    fn envelope(&self, command: &str) -> Value {
        crate::json_out::failure_value::<crate::json_out::NoData>(command, self, None)
    }

    fn human_text(&self) -> String {
        let body = self.body();
        self.human(crate::json_out::TOOL, body.field.as_deref(), &body.suggestions)
    }

    fn emit(&self, command: &str, json: bool) -> u8 {
        debug_assert_answerable(self);
        if json {
            crate::json_out::print_failure::<crate::json_out::NoData>(command, self, None);
        } else {
            eprintln!("{}", with_usage_hint(self, command).human_text());
        }
        self.exit
    }

    fn emit_with_data<R: Report>(&self, command: &str, json: bool, data: &R) -> u8 {
        self.emit_with_data_and_meta(command, json, data, Value::Null)
    }

    fn emit_with_data_and_meta<R: Report>(&self, command: &str, json: bool, data: &R, extra_meta: Value) -> u8 {
        debug_assert_answerable(self);
        // A refusal (exit 2) changed nothing and carries no `data` (spec 8.3, 6.4). Every test that runs the debug binary observes this.
        debug_assert!(self.exit != EXIT_REFUSED, "{} is a refusal (exit 2): it carries no data", self.code);
        if json {
            crate::json_out::print_failure_with_meta(command, self, Some(data), extra_meta);
        }
        self.exit
    }
}

/// A clap value parser for an argument that takes one of a closed set of
/// words. clap then knows the set: a bad value is refused with
/// `possible values`, `canon schema` lists them as `possibleValues`, and the
/// `--json` usage error carries them as `expected`.
pub fn closed_set<T, E>(values: impl Into<clap::builder::PossibleValuesParser>, parse: fn(&str) -> Result<T, E>) -> impl clap::builder::TypedValueParser<Value = T>
where
    T: Clone + Send + Sync + 'static,
    E: Into<Box<dyn std::error::Error + Send + Sync + 'static>> + 'static,
{
    use clap::builder::TypedValueParser;
    clap::builder::PossibleValuesParser::new(values).try_map(move |word: String| parse(&word))
}

/// The reserved codes (spec 7.2.1) a command has because of a feature it offers (spec 10.13 rule 4): `--input`,
/// a batch `--input`, `--commit`, and `--cursor`. The command document lists them in its `errors`.
pub mod feature_codes {
    /// A command that takes `--input`.
    pub const INPUT: [&str; 3] = ["INPUT_INVALID", "INPUT_CONFLICT", "INPUT_TOO_LARGE"];
    /// A command whose `--input` takes an array or NDJSON.
    pub const BATCH: &str = "BATCH_INCOMPLETE";
    /// A command that takes `--commit`.
    pub const COMMIT: &str = "PLAN_STALE";
    /// A command that takes `--cursor`.
    pub const CURSOR: &str = "CURSOR_INVALID";
}

/// A `USAGE` that cannot name the argument at fault has no `field`, and its hint is `canon schema
/// <command> --json` for the command that was run, or `canon schema --json` when no command
/// resolved (spec 7.1.3). Any other error is returned as it is.
pub fn with_usage_hint<'a>(error: &'a CliError, command: &str) -> std::borrow::Cow<'a, CliError> {
    if error.code != "USAGE" || error.field_name().is_some_and(|field| !field.is_empty()) {
        return std::borrow::Cow::Borrowed(error);
    }
    let id = crate::json_out::command_id(command);
    let hint = if id.is_empty() { "canon schema --json".to_string() } else { format!("canon schema {id} --json") };
    std::borrow::Cow::Owned(error.clone().hint(hint))
}

/// A debug build's check of what an error answers, for the error and for every entry of its `error.details` (each is a code of its own, spec
/// 7.2.2): the code is registered, its exit and `retryable` are the registry row's (a reason never changes either, spec 7.6), and a reason is one
/// the code registers.
fn debug_assert_answerable(error: &CliError) {
    if !cfg!(debug_assertions) {
        return;
    }
    for each in std::iter::once(error).chain(error.detail_errors()) {
        debug_assert!(is_registered(each.code), "error code {} is not in cli_error::all_codes()", each.code);
        debug_assert!(
            code_info(each.code).is_some_and(|info| info.exit == each.exit && info.retryable == each.retryable),
            "{} answers exit {} and retryable {}, not its registry row's: a reason never changes either (spec 7.6)",
            each.code,
            each.exit,
            each.retryable
        );
        debug_assert!(
            each.reason_name().is_none_or(|reason| code_info(each.code).is_some_and(|info| info.find_reason(reason).is_some())),
            "reason {:?} is not registered under {} in cli_error::all_codes()",
            each.reason_name(),
            each.code
        );
    }
}

fn is_registered(code: &str) -> bool {
    code_info(code).is_some()
}

/// The warning registry row of `code`, if it has one (spec 7.5.2).
pub fn warning_info(code: &str) -> Option<&'static WarningInfo> {
    atip::find_warning(WARNINGS, code)
}

/// Every warning code, sorted by code. A code that is also an error is in [`all_codes`] too, with the same meaning.
pub fn all_warnings() -> &'static [WarningInfo] {
    WARNINGS
}

/// The registry row of `code`, if it has one.
pub fn code_info(code: &str) -> Option<&'static CodeInfo> {
    atip::find_code(CODES, code)
}

/// Every error code a command can emit, with its meaning, exit and
/// retryability, sorted by code. A code appears once; `tests/cli_error.rs`
/// keeps this in step with the codes the sources construct.
pub fn all_codes() -> &'static [CodeInfo] {
    CODES
}

const APPROVAL_INVALID_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "MALFORMED", meaning: "the approval arguments are not well formed (a blank or multi-line approver, a role that is not human, a timestamp that is not RFC 3339): raised from the arguments alone, before any state is read", fix: "Give the approver as one trimmed line, `--approval-role human` and an RFC 3339 `--approval-at`." },
    ReasonInfo { reason: "SIGNATURE_MISMATCH", meaning: "the signature cannot stand for the approval: the file is missing or unreadable, is not armored SSH signature text, no policy-pinned `allowed_signers` verifier is configured, or the signature does not verify over the payload with a listed key", fix: "Sign the exact bytes `canon evidence approval-payload` prints with a key the policy's `allowed_signers` lists, and pass that signature file." },
];

const INPUT_INVALID_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "BATCH_UNSUPPORTED", meaning: "the document is an array or NDJSON and the command takes one object", fix: "Pass one JSON object and run the command once per item." },
    ReasonInfo { reason: "EMPTY", meaning: "the `--input` value or its source holds no JSON", fix: "Pass inline JSON, a file path, or `-` with a document on stdin." },
    ReasonInfo { reason: "FLAG_ONLY_KEY", meaning: "a document key names a command-line flag that a document cannot set", fix: "Drop the key and pass the flag on the command line." },
    ReasonInfo { reason: "INVALID_VALUE", meaning: "a document value has the wrong type, is outside its closed set or is null", fix: "Give the key a value of the type `canon schema <command> --json` lists under `inputs`, or omit the key." },
    ReasonInfo { reason: "NOT_JSON", meaning: "the source is not JSON, nor NDJSON where a file or stdin allows it", fix: "Fix the JSON, or put one object per line in a file and pass its path." },
    ReasonInfo { reason: "NOT_OBJECT", meaning: "the document, or an item of a batch, is JSON but not an object", fix: "Pass an object, or an array of objects." },
    ReasonInfo { reason: "STDIN_TTY", meaning: "`--input -` reads stdin and stdin is a terminal", fix: "Pipe the document in, or pass it inline or as a file." },
    ReasonInfo { reason: "UNKNOWN_KEY", meaning: "a document key is not an option of the command", fix: "Use only the keys `canon schema <command> --json` lists under `inputs`." },
    ReasonInfo { reason: "UNREADABLE", meaning: "the file named by `--input` cannot be read, or is not UTF-8 text", fix: "Give a readable UTF-8 file, or pass the document inline." },
];

const INPUT_TOO_LARGE_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "TOO_MANY_BYTES", meaning: "the `--input` source is over 8 MiB", fix: "Shrink the document, or split a batch into several runs." },
    ReasonInfo { reason: "TOO_MANY_ITEMS", meaning: "a batch is over 10000 items", fix: "Split the batch into several runs, each under the limit." },
];

/// The reasons registered under each `<SUBJECT>_INCOMPLETE` code that stops after an earlier step made its effects (spec 8.3.2, 7.6): a
/// reason is the code of the refusal a later step met, or, for a stop that has no refusal to name, what stopped the work. Each code keeps its
/// own list, in the sort order of the names, and registers only a reason the sources can meet at that code (`canon-0-36-codes-incomplete`).
const ARTIFACTS_MOVE_INCOMPLETE_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "ARTIFACTS_BLOB_CORRUPT", meaning: "`canon artifacts move --commit` uploaded blobs, then a later blob was rewritten by another process while it was uploaded, or its object already in the store does not verify; the uploads stay, no local blob was removed and canon.yaml is unchanged (`data` says what was uploaded)", fix: "Read what was done (`canon artifacts verify`, the first `meta.next` step), find out what rewrote the blob or damaged the object, then preview and commit a new plan: running the same `--commit --plan-digest` again is refused as `PLAN_STALE`." },
    ReasonInfo { reason: "ARTIFACTS_STORE_DENIED", meaning: "`canon artifacts move --commit` uploaded blobs, then a later local blob could not be read because this user may not read it; the uploads stay, no local blob was removed and canon.yaml is unchanged", fix: "Read what was done (`canon artifacts verify`), make the blob the message names readable, then preview and commit a new plan." },
    ReasonInfo { reason: "ARTIFACTS_STORE_INVALID", meaning: "`canon artifacts move --commit` uploaded blobs, then a later local blob turned out to be no regular file; the uploads stay, no local blob was removed and canon.yaml is unchanged", fix: "Read what was done (`canon artifacts verify`), replace what the message names with a real blob or remove it, then preview and commit a new plan." },
    ReasonInfo { reason: "ARTIFACTS_STORE_REQUIRED", meaning: "`canon artifacts move --commit` uploaded blobs, then the object-store tier could not be attached any more; the uploads stay, no local blob was removed and canon.yaml is unchanged", fix: "Read what was done (`canon artifacts verify`), set the tier's endpoint, credentials and bucket again, then preview and commit a new plan." },
    ReasonInfo { reason: "ARTIFACTS_STORE_UNAVAILABLE", meaning: "`canon artifacts move --commit` uploaded blobs, then the object store stopped answering, or a later local blob could not be read in a way a repeat can pass; the uploads stay, no local blob was removed and canon.yaml is unchanged", fix: "Read what was done (`canon artifacts verify`), wait for the object store to answer, then preview and commit a new plan: running the same `--commit --plan-digest` again is refused as `PLAN_STALE`." },
];

const ARTIFACTS_PRUNE_INCOMPLETE_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "ARTIFACTS_STORE_FAILED", meaning: "`canon artifacts prune --commit` deleted some blobs, then the object store failed or refused the delete of a later object; what it deleted stays deleted, no blob a record binds was touched (`data` says what was deleted)", fix: "Read what was done (`canon artifacts verify`), check that the object store is reachable and that its credentials may delete objects, then preview and commit a new plan." },
    ReasonInfo { reason: "WRITE_FAILED", meaning: "`canon artifacts prune --commit` deleted some blobs, then a later local blob could not be removed; what it deleted stays deleted, no blob a record binds was touched (`data` says what was deleted)", fix: "Read what was done (`canon artifacts verify`), make `.canon/artifacts/sha256` writable, then preview and commit a new plan." },
];

const ARTIFACTS_STORE_INCOMPLETE_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "ARTIFACTS_STORE_FAILED", meaning: "`canon artifacts store` stored some blobs, then the write of a later blob failed; the stored blobs stay", fix: "Read what was stored (`canon artifacts verify`), check that the artifact store is writable and has space, then run the same command again to store the rest." },
    ReasonInfo { reason: "ARTIFACTS_STORE_UNAVAILABLE", meaning: "`canon artifacts store` stored some blobs, then the check of a later blob in the store failed; the stored blobs stay", fix: "Read what was stored (`canon artifacts verify`), wait for the store to answer, then run the same command again to store the rest." },
];

const CHANGE_ADOPTION_INCOMPLETE_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "WRITE_FAILED", meaning: "the adoption or unlink wrote its first record, then the write of the second failed, so the subject and the change disagree about their link", fix: "Run the repair command named in the hint to finish the link between the change and its subject." },
];

const HOOKS_INSTALL_INCOMPLETE_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "WRITE_FAILED", meaning: "`canon gate install-hooks` wrote a hook file (or the first two), then the write of the next file, or of the pre-commit script, failed; what it wrote stays", fix: "Read what was written (`canon status`), check that the path the message names is writable, then run `canon gate install-hooks` again." },
];

const LEDGER_COMPACT_INCOMPLETE_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "REPO_DENIED", meaning: "`canon ledger compact --commit` moved some versions, then a later kind of the ledger could not be read because this user may not read it; what moved stays in the archive", fix: "Read what was done (`canon status`), make the ledger directory the message names readable, then preview again with `canon ledger compact`." },
    ReasonInfo { reason: "REPO_INVALID", meaning: "`canon ledger compact --commit` moved some versions, then a later kind of the ledger could not be read (a path that is no directory, or a tier that cannot be attached); what moved stays in the archive", fix: "Read what was done (`canon status`), fix what the message names, then preview again with `canon ledger compact`." },
    ReasonInfo { reason: "REPO_UNAVAILABLE", meaning: "`canon ledger compact --commit` moved some versions, then the system failed a read of the ledger in a way a repeat can pass; what moved stays in the archive", fix: "Read what was done (`canon status`), wait a moment, then preview again with `canon ledger compact`." },
    ReasonInfo { reason: "WRITE_FAILED", meaning: "`canon ledger compact --commit` moved some versions, then a later move into the archive failed; what moved stays in the archive", fix: "Read what was done (`canon status`), fix the file-system problem the message names, then preview again with `canon ledger compact`." },
];

const PLUGIN_SYNC_INCOMPLETE_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "WRITE_FAILED", meaning: "`canon plugin sync` wrote some overlay records, then could not write others (`data.roots` names each error); the written ones stay", fix: "Read what was done (`canon status`, `data.roots`), fix each error `data.roots` lists, then run `canon plugin sync` again." },
];

const RETENTION_PURGE_INCOMPLETE_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "WRITE_FAILED", meaning: "`canon purge --commit` deleted some records, then a later deletion failed (`data` names the counts); what was deleted stays deleted", fix: "Read what was done (`canon status`), fix the file-system problem the message names, then preview again with `canon purge` and commit its new plan." },
];

const TIER_INCOMPLETE_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "WRITE_FAILED", meaning: "tier aging moved some records, then a later move failed; what moved stays moved", fix: "Read what was done (`canon status`), fix the cause the message names, then preview again with `canon tier age` and commit its new plan." },
];

const DIVERGENCE_INCOMPLETE_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "PROMOTION_INVALID", meaning: "`canon divergence add` staged the divergence record and the gate then refused to promote it, so the record stays staged and was not committed", fix: "Read what was done (`canon status`), fix the record the message names, then run `canon gate promote`; the refused record is still staged, so do not add it again." },
];

const EVIDENCE_INCOMPLETE_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "PROMOTION_INVALID", meaning: "`canon evidence add` staged the evidence record and the gate then refused to promote it, so the record stays staged and was not committed", fix: "Read what was done (`canon status`), fix the record the message names, then run `canon gate promote`; the refused record is still staged, so do not add it again." },
];

const FINDING_INCOMPLETE_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "PROMOTION_INVALID", meaning: "`canon finding add` or `canon finding close` staged the finding record and the gate then refused to promote it, so the record stays staged and was not committed", fix: "Read what was done (`canon status`), fix the record the message names, then run `canon gate promote`; the refused record is still staged, so do not add it again." },
];

const INIT_INCOMPLETE_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "CONFIG_INVALID", meaning: "`canon init` wrote canon.yaml and the files beside it, then its last step could not install the canon skill because the `skills:` section of canon.yaml is malformed (the reason of the refusal `canon skills install` makes before it starts)", fix: "Read what init wrote (`canon status`), make the `skills:` section of canon.yaml a mapping whose `providers` is a list of claude, codex, omp, pi (`[]` for none), then run `canon skills install`; do not run `canon init` again." },
    ReasonInfo { reason: "SKILLS_SOURCE_NOT_FOUND", meaning: "`canon init` wrote canon.yaml and the files beside it, then its last step could not install the canon skill because the skill source (the directory CANON_SKILLS_SOURCE names) does not exist or holds no SKILL.src.md", fix: "Read what init wrote (`canon status`), point CANON_SKILLS_SOURCE at a directory that holds a SKILL.src.md or unset it, then run `canon skills install`; do not run `canon init` again." },
];

const INVENTORY_INCOMPLETE_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "INVENTORY_FAILED", meaning: "`inventory sync` indexed some spec roots, then the record store failed while it indexed a later one; the records of the earlier roots stay", fix: "Read what was done (`canon status`), fix what the message names, then run `canon inventory sync` again." },
    ReasonInfo { reason: "INVENTORY_INVALID", meaning: "`inventory sync` indexed some spec roots, then left a root unindexed because its corpus has format violations or declares a scenario twice (`data.roots` says per root and names each violation)", fix: "Read what was done (`canon status`, `data.roots`), fix the violations the root names, then run `canon inventory sync` again." },
];

const INVENTORY_INVALID_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "DUPLICATE_SCENARIOS", meaning: "a spec root declares the same scenario twice and the sync indexed nothing (`error.details` names each duplicate by root); nothing was written", fix: "Remove the duplicate scenario the detail names, then run `canon inventory sync` again." },
    ReasonInfo { reason: "VIOLATIONS", meaning: "a spec root's corpus has format violations and the sync indexed nothing (`error.details` names each one by root); nothing was written", fix: "Fix the violations `error.details` names (`canon format` repairs what it can), then run `canon inventory sync` again." },
];

const MIGRATION_INCOMPLETE_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "INDEX", meaning: "`canon migrate` moved the files, then the scenario index could not be rebuilt or its rebuild found problems; what moved stays moved", fix: "Read what was done (`canon status`), run `canon inventory sync` and fix what it reports, then run `canon migrate` again." },
    ReasonInfo { reason: "PINS", meaning: "`canon migrate` moved the files and rebuilt the index, then the reviews could not be pinned to what their evidence says; what moved stays moved", fix: "Read what was done (`canon status`), fix what the message names, then run `canon migrate` again; it finishes the reviews." },
    ReasonInfo { reason: "STOPPED", meaning: "`canon migrate` (or `canon migrate --revert`) stopped part-way, after it had begun changing the repo; what it changed stays changed and the originals are in `.canon/migrate-backup/`", fix: "Read what was done (`canon status`), fix what the message names, then run `canon migrate` again; it finishes the rest." },
];

const STRATEGY_INCOMPLETE_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "STRATEGY_PROMOTION_INVALID", meaning: "`canon strategy promote` demoted an active strategy because a contradicting trajectory turned it off, and then the promotion gate blocked the promotion, so the strategy stays demoted", fix: "Read what was done (`canon status`, `canon strategy promote <id> --dry-run`), then record the evidence the gate asks for (`canon ingest records`)." },
];

const BRIEF_CHECK_FAILED_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "BLOCKERS", meaning: "`brief check` found blocker findings; data.findings names each", fix: "Resolve every blocker finding in data.findings, then run `canon brief check` again." },
];

const BRIEF_INVALID_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "BLOCKERS", meaning: "the brief has blocker findings, so the human cannot approve it yet", fix: "Resolve every blocker finding (`canon brief check` lists them), then ask the human to approve again." },
    ReasonInfo { reason: "UNAPPROVABLE", meaning: "the brief's text cannot take an `Approved:` line (for example an open code fence)", fix: "Close the open code fence or other construct in the brief text so an `Approved:` line can be added, then approve again." },
    ReasonInfo { reason: "UNREADABLE", meaning: "brief.md cannot be read as the brief: a directory stands where the file belongs, or the file is not UTF-8 text", fix: "Put the change's brief.md, a UTF-8 text file, where the message says, then run the command again." },
];

const CONTEXT_INPUT_INVALID_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "SECRET", meaning: "a selected context input contains plaintext secret material", fix: "Remove the plaintext secret from the selected input or deselect it, then build the pack again." },
];

const CONFIG_INVALID_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "ARTIFACT_STORE", meaning: "the `artifact_store:` section of canon.yaml (or the tier it names) is present but unusable", fix: "Fix the `artifact_store:` section of canon.yaml (`canon init --check` names the problem), or set `artifact_store: { backend: local }`, then run the command again." },
    ReasonInfo { reason: "CANON_YAML", meaning: "canon.yaml itself is present but unusable: it does not parse, is not a mapping, or is no file, in a part no other reason of this code names (`specs:`, `skills:`, `artifact_store:`, the tiers and the dashboard have their own)", fix: "Fix canon.yaml so it parses (`canon init --check` names the problem), then run the command again." },
    ReasonInfo { reason: "DASHBOARD", meaning: "the dashboard's canon.yaml section is present but unusable", fix: "Fix the dashboard section of canon.yaml, then start the dashboard again." },
    ReasonInfo { reason: "POLICY", meaning: "`.canon/policy.yaml` is present but unusable: its `adapter_capabilities` section, or the `approval.allowed_signers` setting a `strategy rollback` reads (or the allowed-signers file it names), is malformed", fix: "Fix the policy file the message names, then run the command again." },
    ReasonInfo { reason: "SKILLS", meaning: "the `skills:` section of canon.yaml is present but malformed", fix: "Make the `skills:` section a mapping whose `providers` is a list of claude, codex, omp, pi (`[]` for none), or pass --harness once for each harness or --no-harness." },
    ReasonInfo { reason: "SPECS", meaning: "the `specs:` section of canon.yaml is present but malformed", fix: "Fix the `specs:` section the message names, then run the command again." },
    ReasonInfo { reason: "TIERS", meaning: "the `tiers:`, `routing:` or `aging:` section of canon.yaml is present but unusable, or a record kind has no `routing:` entry", fix: "Fix the `tiers:`, `routing:` and `aging:` sections of canon.yaml (every record kind needs a `routing:` entry), then run the command again." },
];

const SKILLS_CHECK_FAILED_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "DRIFT", meaning: "`skills check` found an installed skill missing, stale, a legacy remnant, an orphan the bundle no longer ships, or written by another canon version", fix: "Run `canon skills install` to refresh the installed skills." },
];

const DISPATCH_CONFLICT_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "ALREADY_ENDED", meaning: "the dispatch run was already ended; ending it again would rewrite the provenance of its end", fix: "Nothing to do: the run is closed; start a new run with `canon dispatch begin` if more work is needed." },
    ReasonInfo { reason: "NOT_RUNNING", meaning: "the run's recorded status is not running, so it cannot be ended", fix: "Close only runs whose recorded status is running; inspect the manifest for the run's status." },
];

const DIVERGENCE_CONFLICT_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "NOT_DIVERGENT", meaning: "the scenario's latest evidence is not divergent (or there is none), so there is no gap to accept", fix: "Accept only a scenario whose latest evidence is divergent; `canon divergence list` shows candidates." },
];

const DIVERGENCE_DENIED_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "NOT_REVIEWER", meaning: "the step is not a reviewer's: only a reviewer step accepts a divergence", fix: "Pass --role reviewer (or declare --as reviewer)." },
    ReasonInfo { reason: "SELF_ATTESTED", meaning: "the accepting actor or session attested the evidence itself", fix: "Have an independent reviewer, in a session that did not attest the evidence, accept the divergence." },
];

const SUBJECT_CONFLICT_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "APPROVED_BRIEF", meaning: "the change carries an approved brief, which is the subject's approved scope, so it stays adopted", fix: "Leave the change adopted, or unadopt only a change nobody approved." },
    ReasonInfo { reason: "FLOW_TAGGED", meaning: "a scenario the subject owns is tagged `@flow:<change>.*`, so unadopting the change would make it fall back to an older approval", fix: "Remove or retag the `@flow:<change>.*` tags on those scenarios first, then unadopt again." },
    ReasonInfo { reason: "TRANSITION", meaning: "the status change is not an allowed transition", fix: "Check the subject's status with `canon subject show` and request an allowed transition." },
];

const SUBJECT_INVALID_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "FALSE_SUCCESS", meaning: "a scenario the transition rests on has a latest audit that is `refuted`: an external evaluator found its declared success false, and a `faithful` verdict does not override it", fix: "Fix the behavior the audit refuted, re-attest it, record the evaluator's new audit (`canon evidence audit <SCENARIO_ID>`), then run `canon subject move` again." },
    ReasonInfo { reason: "MALFORMED_EVIDENCE", meaning: "a record the gate reads to judge the transition does not parse or lacks a required field, so it counts as no evidence and the gate fails closed", fix: "Fix or remove the record `error.details` names, then run `canon subject move` again." },
    ReasonInfo { reason: "OPEN_BLOCKER", meaning: "an open `blocker` finding sits on a change the subject adopts, and the review policy blocks the transition on open findings", fix: "Close the blocker finding (`canon finding close`) once it is fixed, then run `canon subject move` again." },
];

const REPORT_CHECK_FAILED_REASONS: &[ReasonInfo] = &[
    ReasonInfo { reason: "STALE", meaning: "`report check` found the report missing or out of date", fix: "Run `canon report build` or `canon report snapshot` to refresh the report." },
];
const CODES: &[CodeInfo] = &[
    CodeInfo { code: "ACTOR_DENIED", meaning: "this leaf records a human's decision (`brief approve`, `policy approve`, `divergence accept`, a `migrate --commit` that re-binds `Approved:` lines), so an agent role (planner, implementer, reviewer, orchestrator declared with --as or CANON_ROLE) is refused unless it relays the human's explicit answer with --relayed-via, and a repo whose canon.yaml says `approval: { relay: deny }` refuses that too", exit: 2, retryable: false, fix: "Show the human what they approve (`canon brief show <change> --body` for a brief), ask with the harness question tool, and only on their explicit approval run the call again with --relayed-via <tool>, --actor, --approver and --note (a brief also takes --digest <change>=<shown digest>). Where canon.yaml says `approval: { relay: deny }`, the human runs the call in their own shell (or on the dashboard for a brief). Never remove your role (`env -u CANON_ROLE`, dropping --as) to run it yourself.", reasons: &[] },
    CodeInfo { code: "ADAPTER_AUTHORIZATION_FAILED", meaning: "`adapter authorize` read the response and its answer is that execution is not authorized: the check's negative answer, its result in data", exit: 1, retryable: false, fix: "Have the adapter return a response that authorizes execution, or do not run the command through this adapter.", reasons: &[] },
    CodeInfo { code: "ADAPTER_FILE_DENIED", meaning: "the adapter response file exists but this user may not read it", exit: 2, retryable: false, fix: "Make the --response file readable by this user, then run the command again.", reasons: &[] },
    CodeInfo { code: "ADAPTER_FILE_INVALID", meaning: "the adapter response file cannot be used as a response: it is not a file or not UTF-8 text, or (for `adapter authorize`) it breaks a rule of the response envelope", exit: 2, retryable: false, fix: "Run `canon adapter validate --response <file>` to see which rule the response breaks, fix the response, then run the command again.", reasons: &[] },
    CodeInfo { code: "ADAPTER_FILE_NOT_FOUND", meaning: "the adapter response file does not exist", exit: 2, retryable: false, fix: "Pass --response naming an existing response file.", reasons: &[] },
    CodeInfo { code: "ADAPTER_FILE_UNAVAILABLE", meaning: "the adapter response file exists but the system failed the read in a way a repeat can pass (an I/O error, an interrupted or timed-out call)", exit: 2, retryable: true, fix: "Wait a moment, then run the command again.", reasons: &[] },
    CodeInfo { code: "ADAPTER_VALIDATION_FAILED", meaning: "`adapter validate` read the response and it breaks a rule of the envelope: the check's negative answer", exit: 1, retryable: false, fix: "Fix the response envelope the message names, then run `canon adapter validate` again.", reasons: &[] },
    CodeInfo { code: "APPROVAL_INVALID", meaning: "the human's answer an approval records is blank or not one line: `brief approve` got an empty or multi-line --approver or --note, or `policy approve` got a blank --approver", exit: 2, retryable: false, fix: "Pass the human's own answer, each one non-empty line: --approver and --note to `canon brief approve`, --approver to `canon policy approve`.", reasons: &[] },
    CodeInfo { code: "ARTIFACTS_BLOB_CORRUPT", meaning: "a local artifact blob does not match its name, or the object store holds a bad copy of it; the stored bytes canon wrote are damaged, and nothing was moved. A blob that cannot be read at all is ARTIFACTS_BLOB_DENIED, ARTIFACTS_STORE_INVALID or ARTIFACTS_STORE_UNAVAILABLE", exit: 2, retryable: false, fix: "Run `canon artifacts verify` to see which blobs differ, restore or delete any local blob whose content does not match its name, replace the bad object-store copy the error names, then run the command again. `canon artifacts verify` answers the same situation with exit 1.", reasons: &[] },
    CodeInfo { code: "ARTIFACTS_BLOB_DENIED", meaning: "a local artifact blob exists but this user may not read it, so it can be neither verified nor moved; nothing was moved", exit: 2, retryable: false, fix: "Make the local blob the error names readable by this user, then run the command again.", reasons: &[] },
    CodeInfo { code: "ARTIFACTS_DIGEST_INVALID", meaning: "a digest an artifact record carries is not 64 lowercase hex characters, so no path was built and nothing was stored", exit: 2, retryable: false, fix: "A recorded digest must be 64 lowercase hex characters; fix or re-create the record, then run the command again.", reasons: &[] },
    CodeInfo { code: "ARTIFACTS_MOVE_INCOMPLETE", meaning: "`artifacts move --commit` uploaded some blobs and then stopped, because a later blob was rewritten or cannot be read, or the object store stopped answering; the uploads stay, no local blob was removed and canon.yaml is unchanged (`data` says what was uploaded, the reason says what stopped it)", exit: 1, retryable: false, fix: "Read what was done first (the first `meta.next` step), deal with what the reason names, then preview and commit a new plan; the same `--commit --plan-digest` is refused as `PLAN_STALE`.", reasons: ARTIFACTS_MOVE_INCOMPLETE_REASONS },
    CodeInfo { code: "ARTIFACTS_PRUNE_INCOMPLETE", meaning: "`artifacts prune --commit` deleted some blobs and then stopped, because the object store failed or refused a later delete, or a later local blob could not be removed; what it deleted stays deleted and no blob a record binds was touched (`data` says what was deleted, the reason says what stopped it)", exit: 1, retryable: false, fix: "Read what was done first (the first `meta.next` step), deal with what the reason names, then preview and commit a new plan; the same `--commit --plan-digest` is refused as `PLAN_STALE`.", reasons: ARTIFACTS_PRUNE_INCOMPLETE_REASONS },
    CodeInfo { code: "ARTIFACTS_STORE_DENIED", meaning: "the local artifact store directory (`.canon/artifacts` or `.canon/artifacts/sha256`) exists but this user may not read or enter it; nothing was written or moved", exit: 2, retryable: false, fix: "Make `.canon/artifacts/sha256` and the directory above it readable and writable by this user, then run the command again.", reasons: &[] },
    CodeInfo { code: "ARTIFACTS_STORE_FAILED", meaning: "writing or uploading a blob to the artifact store failed and nothing it wrote was kept: a record or batch that bound the file was rolled back, or no blob had been stored yet; `artifacts prune --commit` got an object-store delete refused before it had deleted anything", exit: 1, retryable: true, fix: "Check that the artifact store is writable and reachable and has space, then run the command again.", reasons: &[] },
    CodeInfo { code: "ARTIFACTS_STORE_INCOMPLETE", meaning: "`artifacts store` stored some blobs and then a later write or check of the store failed; the stored blobs stay and a repeat of the same call stores the rest; the reason is the code of the step that failed", exit: 1, retryable: true, fix: "Read what was stored first (the first `meta.next` step), check that the artifact store is writable and reachable and has space, then run the same command again to store the rest.", reasons: ARTIFACTS_STORE_INCOMPLETE_REASONS },
    CodeInfo { code: "ARTIFACTS_STORE_INVALID", meaning: "the local artifact store directory (`.canon/artifacts/sha256`) is not a real directory: a symlink, a regular file or a path that is no directory stands there, so writing through it could land outside the store; nothing was written", exit: 2, retryable: false, fix: "Replace `.canon/artifacts/sha256` with a real directory, then run the command again.", reasons: &[] },
    CodeInfo { code: "ARTIFACTS_STORE_REQUIRED", meaning: "the object-store tier `artifact_store.backend` names cannot be attached because its endpoint, credentials or bucket are not set; nothing was written, and waiting does not help", exit: 2, retryable: false, fix: "Set CANON_S3_ENDPOINT, CANON_S3_ACCESS_KEY, CANON_S3_SECRET_KEY and the tier's bucket variable, or set `artifact_store: { backend: local }` in canon.yaml, then run the command again.", reasons: &[] },
    CodeInfo { code: "ARTIFACTS_STORE_UNAVAILABLE", meaning: "the artifact store could not be opened or reached before any blob was written: another canon process holds the store lock, the object store did not answer, or the system failed the open in a way a repeat can pass; nothing was written", exit: 2, retryable: true, fix: "Wait for the other canon process or for the object store to answer, then run the command again.", reasons: &[] },
    CodeInfo { code: "ARTIFACTS_TIER_INVALID", meaning: "`artifacts move` was asked to move to a target that is not an s3 tier of canon.yaml (`local` cannot take bytes back from an object store)", exit: 2, retryable: false, fix: "Pass --tier naming an `s3` tier of canon.yaml `tiers:` (for example `cold`).", reasons: &[] },
    CodeInfo { code: "ARTIFACTS_VERIFY_FAILED", meaning: "a bound file is missing from the artifact store or altered; data.problems names each", exit: 1, retryable: false, fix: "Restore or re-store each file listed in data.problems, then run `canon artifacts verify` again.", reasons: &[] },
    CodeInfo { code: "BATCH_INCOMPLETE", meaning: "a batch stopped during execution: an element failed after the batch passed its preflight; data.items gives each element's status and the failed one's error, and the elements after it were skipped", exit: 1, retryable: false, fix: "Read data.items to see which elements were written, fix the failed element, then run the batch again; elements already written are reported `unchanged`.", reasons: &[] },
    CodeInfo { code: "BRIEF_APPROVAL_REQUIRED", meaning: "an attestation was refused because the subject's brief has no human approval", exit: 2, retryable: false, fix: "Have the human approve the subject's brief with `canon brief approve` before attesting.", reasons: &[] },
    CodeInfo { code: "BRIEF_APPROVAL_STALE", meaning: "the brief was edited after the human approved it, so it reads as not approved until approved again", exit: 2, retryable: false, fix: "Review the edited brief and have the human run `canon brief approve` again.", reasons: &[] },
    CodeInfo { code: "BRIEF_CHECK_FAILED", meaning: "`brief check` ran and its answer is that the brief fails the check", exit: 1, retryable: false, fix: "Resolve every blocker finding in data.findings, then run `canon brief check` again.", reasons: BRIEF_CHECK_FAILED_REASONS },
    CodeInfo { code: "BRIEF_DENIED", meaning: "the brief file exists but this user may not read it", exit: 2, retryable: false, fix: "Make brief.md readable by this user, then run the command again.", reasons: &[] },
    CodeInfo { code: "BRIEF_INVALID", meaning: "the brief cannot be used as it stands: a path that is no file or text that is not UTF-8 stands where brief.md belongs, it has blocker findings, or its text cannot take an `Approved:` line", exit: 2, retryable: false, fix: "Make brief.md a UTF-8 text file, resolve the blocker findings (`canon brief check` lists them) or close the open code fence in the brief text, then run the command again.", reasons: BRIEF_INVALID_REASONS },
    CodeInfo { code: "BRIEF_MIGRATION_REQUIRED", meaning: "a command that writes and judges a change brief was refused because that brief is in the format canon 0.30 and earlier wrote (`canon:` comments or shipped Korean headings); nothing was written", exit: 2, retryable: false, fix: "Run `canon migrate --json` to preview the rewrite; the human runs `canon migrate --commit --plan-digest <digest>` when the plan re-binds an `Approved:` line.", reasons: &[] },
    CodeInfo { code: "BRIEF_NOT_FOUND", meaning: "the named change has no brief", exit: 2, retryable: false, fix: "Pass the id of a change that has a brief; `canon status` lists the changes.", reasons: &[] },
    CodeInfo { code: "BRIEF_PAGE_STALE", meaning: "the dashboard was asked to approve a brief whose file differs from the copy the page showed the human; nothing was recorded", exit: 2, retryable: false, fix: "Reload the dashboard page to see the current brief, then approve it again.", reasons: &[] },
    CodeInfo { code: "BRIEF_REQUIRED", meaning: "no live change has a brief", exit: 2, retryable: false, fix: "Run `canon change new` to create a change with a brief.", reasons: &[] },
    CodeInfo { code: "BRIEF_UNAVAILABLE", meaning: "the brief file exists but the system failed the read in a way a repeat can pass (an I/O error, an interrupted or timed-out call)", exit: 2, retryable: true, fix: "Wait a moment, then run the command again.", reasons: &[] },
    CodeInfo { code: "BRIEF_WRITE_FAILED", meaning: "the brief file could not be written", exit: 1, retryable: false, fix: "Check that the brief file and its directory are writable, then run the command again.", reasons: &[] },
    CodeInfo { code: "CHANGE_ADOPTION_INCOMPLETE", meaning: "an adoption or unlink stopped between its two records, after the first was written, so the subject and the change disagree about their link; the hint names the command that finishes it", exit: 1, retryable: false, fix: "Run the repair command named in the hint to finish the link between the change and its subject.", reasons: CHANGE_ADOPTION_INCOMPLETE_REASONS },
    CodeInfo { code: "CHANGE_ARGUMENT_INVALID", meaning: "`change new` could not build a brief from the given arguments", exit: 2, retryable: false, fix: "Correct the arguments to `canon change new` so a brief can be built from them.", reasons: &[] },
    CodeInfo { code: "CHANGE_EXISTS", meaning: "a change (or its archive) already uses that slug", exit: 2, retryable: false, fix: "Choose a different slug for the new change, or work on the existing one.", reasons: &[] },
    CodeInfo { code: "CHANGE_NOT_FOUND", meaning: "the named change has no record", exit: 2, retryable: false, fix: "Pass the id of a change that exists; `canon ingest plans` imports a change from its plan document.", reasons: &[] },
    CodeInfo { code: "CHANGE_REQUIRED", meaning: "several live changes have a brief; pass the change id as the operand", exit: 2, retryable: false, fix: "Pass the id of the change you mean as the operand.", reasons: &[] },
    CodeInfo { code: "COMMAND_REQUIRED", meaning: "the bare tool or a command group was run without a command; `expected` lists the commands of that level", exit: 2, retryable: false, fix: "Run one of the commands `expected` lists; `canon schema` lists every command.", reasons: &[] },
    CodeInfo { code: "COMMIT_REQUIRED", meaning: "a command needed a commit and the directory is not a git work tree or has no commits", exit: 2, retryable: false, fix: "Run the command inside a git work tree that has at least one commit.", reasons: &[] },
    CodeInfo { code: "CONFIG_CHECK_FAILED", meaning: "`init --check` ran and a present canon.yaml section fails its loader", exit: 1, retryable: false, fix: "Fix each section `data.sections` marks FAIL, then run `canon init --check` again.", reasons: &[] },
    CodeInfo { code: "CONFIG_DENIED", meaning: "canon.yaml or a policy file exists but this user may not read it", exit: 2, retryable: false, fix: "Make the file the error names readable by this user, then run the command again.", reasons: &[] },
    CodeInfo { code: "CONFIG_EXISTS", meaning: "canon.yaml already exists and there is nothing else to refresh; `demo init` refuses any directory that holds one", exit: 2, retryable: false, fix: "Nothing to do: canon.yaml already exists; run `canon init --check` to verify it, or `canon demo init` in a directory without one.", reasons: &[] },
    CodeInfo { code: "CONFIG_INVALID", meaning: "canon.yaml or a policy file a person edits is present but unusable: it does not parse, it is a directory, or a section of it is malformed (the reason, when there is one, names which); a file this user may not read is CONFIG_DENIED", exit: 2, retryable: false, fix: "Fix canon.yaml (`canon init --check` names the problem), or the section the reason names (`ARTIFACT_STORE`: the `artifact_store:` section; `DASHBOARD`: the dashboard section of canon.yaml; `POLICY`: the adapter policy in .canon/policy.yaml; `SPECS`: the `specs:` section; `SKILLS`: the `skills:` section; `TIERS`: the `tiers:`, `routing:` and `aging:` sections), then run the command again.", reasons: CONFIG_INVALID_REASONS },
    CodeInfo { code: "CONFIG_REQUIRED", meaning: "canon.yaml does not exist", exit: 2, retryable: false, fix: "Run `canon init` or pass --repo <DIR> to a directory holding canon.yaml.", reasons: &[] },
    CodeInfo { code: "CONFIG_UNAVAILABLE", meaning: "canon.yaml or a policy file exists but the system failed the read in a way a repeat can pass (an I/O error, an interrupted or timed-out call)", exit: 2, retryable: true, fix: "Wait a moment, then run the command again.", reasons: &[] },
    CodeInfo { code: "CONTEXT_GIT_REQUIRED", meaning: "a context pack needs a git identity that is not available", exit: 2, retryable: false, fix: "Run the command inside a git checkout with a resolvable HEAD and git on the PATH.", reasons: &[] },
    CodeInfo { code: "CONTEXT_INPUT_INVALID", meaning: "a selected context input cannot go into the pack", exit: 2, retryable: false, fix: "Remove the plaintext secret from the selected input or deselect it, then build the pack again.", reasons: CONTEXT_INPUT_INVALID_REASONS },
    CodeInfo { code: "CONTEXT_INPUT_NOT_FOUND", meaning: "a selected context input does not exist", exit: 2, retryable: false, fix: "Create the selected context input or drop it from the selection.", reasons: &[] },
    CodeInfo { code: "CONTEXT_MANIFEST_INVALID", meaning: "the context input manifest has an unsupported version", exit: 2, retryable: false, fix: "Rewrite the context input manifest with a supported version.", reasons: &[] },
    CodeInfo { code: "CONTEXT_OBJECT_CONFLICT", meaning: "an immutable context object already holds different bytes", exit: 2, retryable: false, fix: "Do not overwrite the immutable context object; build a new pack from the current inputs.", reasons: &[] },
    CodeInfo { code: "CONTEXT_PACK_CORRUPT", meaning: "a context pack that a command other than `context-pack verify` reads is stale or was changed after it was created", exit: 2, retryable: false, fix: "Discard the stale or altered pack and build a new one with `canon context-pack add`. `canon context-pack verify` answers the same condition as a check result with CONTEXT_PACK_VERIFY_FAILED (exit 1).", reasons: &[] },
    CodeInfo { code: "CONTEXT_PACK_NOT_FOUND", meaning: "no context pack with that well-formed id (a malformed id is USAGE)", exit: 2, retryable: false, fix: "Pass the id of an existing context pack; `canon context list` shows them.", reasons: &[] },
    CodeInfo { code: "CONTEXT_PACK_VERIFY_FAILED", meaning: "`context-pack verify` read the pack and its answer is that the pack does not verify: it is stale or was changed after it was created", exit: 1, retryable: false, fix: "Discard the stale or altered pack and build a new one with `canon context-pack add`. Every other command that reads the pack refuses the same condition with CONTEXT_PACK_CORRUPT (exit 2).", reasons: &[] },
    CodeInfo { code: "CONTEXT_PATH_INVALID", meaning: "a selected path is absolute, leaves the repository, or contains a symlink", exit: 2, retryable: false, fix: "Select only repository-relative paths that stay inside the repository and contain no symlink.", reasons: &[] },
    CodeInfo { code: "CURSOR_INVALID", meaning: "`--cursor` is not one this listing issued, was issued for other arguments, or points after a record that is gone; `reason` says which", exit: 2, retryable: false, fix: "Restart the listing without --cursor and page with the cursor it returns.", reasons: &atip::CURSOR_INVALID_REASONS },
    CodeInfo { code: "DASHBOARD_FAILED", meaning: "the dashboard could not start for a reason that is not one of the other DASHBOARD_ codes (an I/O failure)", exit: 1, retryable: false, fix: "Read the I/O error in the message, fix its cause, then start the dashboard again.", reasons: &[] },
    CodeInfo { code: "DASHBOARD_PORT_UNAVAILABLE", meaning: "the dashboard's port is already bound by another process", exit: 2, retryable: true, fix: "Stop the process holding the port or choose another port, then start the dashboard again.", reasons: &[] },
    CodeInfo { code: "DASHBOARD_REQUEST_DENIED", meaning: "a dashboard request came with a Host or Origin that is not the bound address, so it was refused before anything was read", exit: 2, retryable: false, fix: "Open the dashboard at the exact address it is bound to.", reasons: &[] },
    CodeInfo { code: "DASHBOARD_REQUEST_INVALID", meaning: "a dashboard request was not the one the briefs page sends: wrong method, content type, length or body", exit: 2, retryable: false, fix: "Send requests only through the briefs page the dashboard serves.", reasons: &[] },
    CodeInfo { code: "DASHBOARD_UNSUPPORTED", meaning: "this canon binary embeds no dashboard app and the repo holds no build of canon's own", exit: 2, retryable: false, fix: "Use a canon binary that embeds the dashboard app, or build the dashboard into the repo.", reasons: &[] },
    CodeInfo { code: "DEMO_REQUIRED", meaning: "`demo review` ran before `demo init`", exit: 2, retryable: false, fix: "Run `canon demo init` before `canon demo review`.", reasons: &[] },
    CodeInfo { code: "DISPATCH_ARGUMENT_INVALID", meaning: "a dispatch flag combination is invalid", exit: 2, retryable: false, fix: "Remove the conflicting dispatch flags; see `canon schema` for the command's options.", reasons: &[] },
    CodeInfo { code: "DISPATCH_CONFLICT", meaning: "the dispatch run is in a state that cannot take this command: it was already ended, or it is not running", exit: 2, retryable: false, fix: "Start a new run with `canon dispatch begin` if more work is needed.", reasons: DISPATCH_CONFLICT_REASONS },
    CodeInfo { code: "DISPATCH_MANIFEST_CORRUPT", meaning: "a dispatch manifest canon wrote is not a readable run record (it is not UTF-8 text, it is a directory, or it does not parse), or its run id differs from its file name; the same manifest is a warning when a command only lists runs", exit: 2, retryable: false, fix: "Restore the dispatch manifest as a readable run record, or rename it to match its run id, then run the command again.", reasons: &[] },
    CodeInfo { code: "DISPATCH_MANIFEST_DENIED", meaning: "a dispatch manifest exists but this user may not read it", exit: 2, retryable: false, fix: "Make the manifest the error names readable by this user, then run the command again.", reasons: &[] },
    CodeInfo { code: "DISPATCH_MANIFEST_UNAVAILABLE", meaning: "a dispatch manifest exists but the system failed the read in a way a repeat can pass (an I/O error, an interrupted or timed-out call)", exit: 2, retryable: true, fix: "Wait a moment, then run the command again.", reasons: &[] },
    CodeInfo { code: "DISPATCH_ROLE_CONFLICT", meaning: "--role does not match the --regime's role", exit: 2, retryable: false, fix: "Pass a --role that matches the --regime's role, or change the --regime.", reasons: &[] },
    CodeInfo { code: "DISPATCH_RUN_NOT_FOUND", meaning: "no dispatch manifest for that run id", exit: 2, retryable: false, fix: "Pass the id of an existing dispatch run.", reasons: &[] },
    CodeInfo { code: "DISPATCH_RUN_UNAVAILABLE", meaning: "another process is closing the run; retry", exit: 2, retryable: true, fix: "Wait a moment and run `canon dispatch end` again.", reasons: &[] },
    CodeInfo { code: "DIVERGENCE_ARGUMENT_INVALID", meaning: "a divergence flag value is invalid", exit: 2, retryable: false, fix: "Correct the divergence flag value; see `canon schema` for the command's options.", reasons: &[] },
    CodeInfo { code: "DIVERGENCE_CONFLICT", meaning: "the evidence is not divergent, so there is no gap to accept", exit: 2, retryable: false, fix: "Accept only divergent evidence; `canon divergence list` shows candidates.", reasons: DIVERGENCE_CONFLICT_REASONS },
    CodeInfo { code: "DIVERGENCE_DENIED", meaning: "this actor may not accept the divergence: the step is not a reviewer's, or the actor or session attested the evidence itself", exit: 2, retryable: false, fix: "Have an independent reviewer, in a session that did not attest the evidence, accept the divergence.", reasons: DIVERGENCE_DENIED_REASONS },
    CodeInfo { code: "DIVERGENCE_INCOMPLETE", meaning: "`divergence add` staged the divergence record and then the gate refused to promote it; the record stays staged and was not committed; `reason` is the gate's refusal", exit: 1, retryable: false, fix: "Read what was done first (the first `meta.next` step), fix the record the message names, then run `canon gate promote`; do not add the divergence again.", reasons: DIVERGENCE_INCOMPLETE_REASONS },
    CodeInfo { code: "DIVERGENCE_INVALID", meaning: "a divergence candidate breaks a rule of the gate; nothing was written", exit: 2, retryable: false, fix: "Resolve the gate rule the error names, then run `canon gate promote --kind divergence` again.", reasons: &[] },
    CodeInfo { code: "EVIDENCE_APPROVAL_INVALID", meaning: "the human approval attached to an attestation is malformed or fails verification", exit: 2, retryable: false, fix: "Attach a well-formed human approval that verifies against the brief.", reasons: APPROVAL_INVALID_REASONS },
    CodeInfo { code: "EVIDENCE_APPROVAL_REQUIRED", meaning: "an attestation that needs a human approval was given none, or only part of the four approval options", exit: 2, retryable: false, fix: "Pass `--approver`, `--approval-role human`, `--signature-file` and `--approval-at` together.", reasons: &[] },
    CodeInfo { code: "EVIDENCE_ARGUMENT_CONFLICT", meaning: "two `evidence add` arguments contradict each other: one forbids the other", exit: 2, retryable: false, fix: "Drop one of the arguments the error names; `canon schema evidence add` lists the combinations.", reasons: &[] },
    CodeInfo { code: "EVIDENCE_ARGUMENT_INVALID", meaning: "an `evidence add` argument value is empty, malformed or outside its form", exit: 2, retryable: false, fix: "Correct the argument the error names; `canon schema evidence add` lists the accepted forms.", reasons: &[] },
    CodeInfo { code: "EVIDENCE_ARGUMENT_REQUIRED", meaning: "an `evidence add` argument that another needs, or one of a set that needs at least one, is missing", exit: 2, retryable: false, fix: "Pass the argument the error names; `canon schema evidence add` lists the arguments.", reasons: &[] },
    CodeInfo { code: "EVIDENCE_BINDING_DENIED", meaning: "a --report or --file to bind lies outside the repository, or this user may not read it", exit: 2, retryable: false, fix: "Copy the file anywhere under the repository (or make it readable) and bind that path.", reasons: &[] },
    CodeInfo { code: "EVIDENCE_BINDING_INVALID", meaning: "a --report or --file to bind cannot be bound: it is not a file, is too large, is not the report format named, or names no case", exit: 2, retryable: false, fix: "Pass a file the binding rules in the error accept; `canon schema evidence add` lists the options and their limits.", reasons: &[] },
    CodeInfo { code: "EVIDENCE_BINDING_NOT_FOUND", meaning: "a --report or --file to bind does not exist", exit: 2, retryable: false, fix: "Pass a --report or --file that exists inside the repository.", reasons: &[] },
    CodeInfo { code: "EVIDENCE_BINDING_STALE", meaning: "a bound file changed after it was bound, so its bytes no longer match the digest taken first", exit: 2, retryable: false, fix: "Bind the file again so the digest is taken from its current bytes.", reasons: &[] },
    CodeInfo { code: "EVIDENCE_BINDING_UNAVAILABLE", meaning: "a --report or --file to bind exists but the system failed the read in a way a repeat can pass (an I/O error, an interrupted or timed-out call); nothing was bound", exit: 2, retryable: true, fix: "Wait a moment, then run the command again.", reasons: &[] },
    CodeInfo { code: "EVIDENCE_CONTRACT_CONFLICT", meaning: "the record's kind and ref differ from the task's typed evidence contract", exit: 2, retryable: false, fix: "Record evidence whose kind and ref match the task's typed evidence contract.", reasons: &[] },
    CodeInfo { code: "EVIDENCE_INCOMPLETE", meaning: "`evidence add` or `evidence audit` staged the evidence record and then the gate refused to promote it; the record stays staged and was not committed; `reason` is the gate's refusal", exit: 1, retryable: false, fix: "Read what was done first (the first `meta.next` step), fix the record the message names, then run `canon gate promote`; do not add the evidence again.", reasons: EVIDENCE_INCOMPLETE_REASONS },
    CodeInfo { code: "EVIDENCE_NOTE_INVALID", meaning: "the note carries a fabrication marker or an unsupported verified claim", exit: 2, retryable: false, fix: "Rewrite the note to state only what was actually run, without fabrication markers or unsupported verified claims.", reasons: &[] },
    CodeInfo { code: "EVIDENCE_REPORT_CONFLICT", meaning: "the bound report contradicts the verdict", exit: 2, retryable: false, fix: "Re-run the check so the report supports the verdict, or change the verdict to match the report.", reasons: &[] },
    CodeInfo { code: "EVIDENCE_REQUIRED", meaning: "an approval or attestation names neither a task nor a scenario, so there is no evidence subject", exit: 2, retryable: false, fix: "Pass --task, --scenario, or both.", reasons: &[] },
    CodeInfo { code: "EVIDENCE_STALE", meaning: "the evidence a review or an approval rests on is out of date: the spec, the code or the report changed since it was recorded; nothing was written", exit: 2, retryable: false, fix: "Record the evidence again (`canon evidence add`), then run the command again.", reasons: &[] },
    CodeInfo { code: "FEATURE_EXISTS", meaning: "the feature file already exists", exit: 2, retryable: false, fix: "Choose a different feature path or edit the existing file.", reasons: &[] },
    CodeInfo { code: "FEATURE_PATH_INVALID", meaning: "the feature path is under no configured specs.roots entry", exit: 2, retryable: false, fix: "Place the feature under a specs.roots entry in canon.yaml, or add a root that covers it.", reasons: &[] },
    CodeInfo { code: "FINDING_ARGUMENT_INVALID", meaning: "a `finding` argument is empty, malformed or out of range", exit: 2, retryable: false, fix: "Correct the `canon finding` argument that is empty, malformed or out of range.", reasons: &[] },
    CodeInfo { code: "FINDING_CONFLICT", meaning: "the finding is still staged and cannot be closed until committed", exit: 2, retryable: false, fix: "Commit the staged finding, then close it.", reasons: &[] },
    CodeInfo { code: "FINDING_EXISTS", meaning: "a finding already holds that change, round and seq", exit: 2, retryable: false, fix: "Choose an unused seq for that change and round.", reasons: &[] },
    CodeInfo { code: "FINDING_INCOMPLETE", meaning: "`finding add` or `finding close` staged the finding record and then the gate refused to promote it; the record stays staged and was not committed; `reason` is the gate's refusal", exit: 1, retryable: false, fix: "Read what was done first (the first `meta.next` step), fix the record the message names, then run `canon gate promote`; do not add the finding again.", reasons: FINDING_INCOMPLETE_REASONS },
    CodeInfo { code: "FINDING_INTRODUCED_REQUIRED", meaning: "a new blocker or should-fix finding names no introducing commit and does not say it is unknown", exit: 2, retryable: false, fix: "Pass --introduced-rev <rev> when the introducing commit was established, or --introduced-unknown when it was not; a note needs neither.", reasons: &[] },
    CodeInfo { code: "FINDING_NOT_FOUND", meaning: "no committed finding at that change, round and seq", exit: 2, retryable: false, fix: "Pass the change, round and seq of a committed finding; `canon finding list` shows them.", reasons: &[] },
    CodeInfo { code: "FINDING_REASON_REQUIRED", meaning: "closing a finding as rejected or deferred needs a one-line --reason", exit: 2, retryable: false, fix: "Pass a one-line --reason when closing a finding as rejected or deferred.", reasons: &[] },
    CodeInfo { code: "FINDING_RESOLUTION_REV_CONFLICT", meaning: "--resolution-rev was given with a disposition other than fixed", exit: 2, retryable: false, fix: "Drop --resolution-rev, or close the finding as fixed.", reasons: &[] },
    CodeInfo { code: "FINDING_REVIEW_NOT_FOUND", meaning: "--review names no review in the ledger or the archive", exit: 2, retryable: false, fix: "Pass the review id that `canon review scenario` or `canon review change` printed; `canon query --kind review` shows the ids.", reasons: &[] },
    CodeInfo { code: "FINDING_ROLE_DENIED", meaning: "`finding close` was asked for `rejected` or `deferred` by a call that declares a role other than `reviewer` (--as or CANON_ROLE); overturning a finding is the reviewer's, nothing was written and the finding stays open", exit: 2, retryable: false, fix: "Leave the finding to a reviewer, who closes it with `--as reviewer`; close it as `fixed` if you fixed it.", reasons: &[] },
    CodeInfo { code: "FORMAT_CHECK_FAILED", meaning: "`format` found spec-corpus violations; data.violations names each", exit: 1, retryable: false, fix: "Fix each violation listed in data.violations, then run `canon format` again.", reasons: &[] },
    CodeInfo { code: "GATE_CHECK_FAILED", meaning: "the evidence gate found violations; data.violations names each", exit: 1, retryable: false, fix: "Fix each violation listed in data.violations, then run the gate again.", reasons: &[] },
    CodeInfo { code: "HOOKS_FILE_CONFLICT", meaning: "a hook file under `.claude` or `.codex`, or the directory it needs, conflicts with what the install must write there; nothing was written", exit: 2, retryable: false, fix: "Move aside whatever occupies the hook path (a directory where the file belongs, a file where its directory belongs), then run the install again.", reasons: &[] },
    CodeInfo { code: "HOOKS_FILE_DENIED", meaning: "a hook file under `.claude` or `.codex` cannot be read or created by this user; nothing was written", exit: 2, retryable: false, fix: "Make the hook files and their directories readable and writable by this user, then run the install again.", reasons: &[] },
    CodeInfo { code: "HOOKS_FILE_UNAVAILABLE", meaning: "a hook file under `.claude` or `.codex` exists but the system failed the read in a way a repeat can pass (an I/O error, an interrupted or timed-out call); nothing was written", exit: 2, retryable: true, fix: "Wait a moment, then run the install again.", reasons: &[] },
    CodeInfo { code: "HOOKS_INSTALL_INCOMPLETE", meaning: "`gate install-hooks` wrote a hook file and then could not write the next file or the pre-commit script; what it wrote stays", exit: 1, retryable: false, fix: "Read what was written first (the first `meta.next` step), check that the path the message names is writable, then run `canon gate install-hooks` again.", reasons: HOOKS_INSTALL_INCOMPLETE_REASONS },
    CodeInfo { code: "HOOKS_REPO_INVALID", meaning: "--repo of `gate install-hooks` is not an existing directory; nothing was written", exit: 2, retryable: false, fix: "Pass --repo naming an existing directory.", reasons: &[] },
    CodeInfo { code: "INGEST_FAILED", meaning: "an ingest command failed", exit: 1, retryable: false, fix: "Read the error message, fix the named source or input, then run the ingest again.", reasons: &[] },
    CodeInfo { code: "INGEST_SOURCE_INVALID", meaning: "an `ingest plans` source was malformed and persisted nothing; check its root", exit: 2, retryable: false, fix: "Fix the malformed plan source under its configured root, then run `canon ingest plans` again.", reasons: &[] },
    CodeInfo { code: "INIT_AGENTS_MD_CONFLICT", meaning: "AGENTS.md cannot take the canon block without overwriting content", exit: 2, retryable: false, fix: "Move or merge the conflicting AGENTS.md content by hand, then run `canon init` again.", reasons: &[] },
    CodeInfo { code: "INIT_ARGUMENT_INVALID", meaning: "an `init` option is invalid (the language)", exit: 2, retryable: false, fix: "Pass a supported language to `canon init`.", reasons: &[] },
    CodeInfo { code: "INIT_INCOMPLETE", meaning: "`init` wrote canon.yaml and the files beside it, then its last step could not install the canon skill because `canon skills install` refuses what it was given; what init wrote stays; `reason` is that refusal", exit: 1, retryable: false, fix: "Read what init wrote first (the first `meta.next` step), act on the fix of the reason, then run `canon skills install`; do not run `canon init` again.", reasons: INIT_INCOMPLETE_REASONS },
    CodeInfo { code: "INPUT_CONFLICT", meaning: "a flag on the command line and the same key in `--input` disagree", exit: 2, retryable: false, fix: "Give the value once: drop the flag or the key, or make the two equal.", reasons: &[] },
    CodeInfo { code: "INPUT_INVALID", meaning: "`--input` cannot be read or framed (empty, not JSON or NDJSON, not an object, a file or stdin that cannot be read, stdin a terminal, an array on a leaf that takes one object) or the document breaks its fragment schema (an unknown key, a flag-only key, a value of the wrong type, null); `field` is `--input` for a fault in the value itself and a JSON pointer for a fault in the document; `reason` says which", exit: 2, retryable: false, fix: "Pass one JSON object, a JSON array, or a file or `-` with one object per line, using only the keys `canon schema <command> --json` lists under `inputs`; `canon schema <command> --json` shows an example.", reasons: INPUT_INVALID_REASONS },
    CodeInfo { code: "INPUT_TOO_LARGE", meaning: "the `--input` document is over 8 MiB or a batch is over 10000 elements", exit: 2, retryable: false, fix: "Split the batch into several runs, each under the limit.", reasons: INPUT_TOO_LARGE_REASONS },
    CodeInfo { code: "INTERNAL", meaning: "a bug in canon: a panic or a state canon did not expect; nothing about the command line is at fault", exit: 1, retryable: false, fix: "Report it at https://github.com/journeyWorker/canon/issues with the command line and this error; it cannot be fixed from the command line.", reasons: &[] },
    CodeInfo { code: "INVENTORY_FAILED", meaning: "the scenario index could not be built: the record store failed while `inventory sync` was reading or writing records", exit: 1, retryable: false, fix: "Fix the store problem the error names, then run `canon inventory sync` again.", reasons: &[] },
    CodeInfo { code: "INVENTORY_INCOMPLETE", meaning: "`inventory sync` indexed some spec roots and then stopped on one (violations, duplicate scenarios, or a store failure); the records of the roots before it stay and `data.roots` names each result; the reason says which stopped it", exit: 1, retryable: false, fix: "Read what was done first (the first `meta.next` step and `data.roots`), fix what the reason names, then run `canon inventory sync` again.", reasons: INVENTORY_INCOMPLETE_REASONS },
    CodeInfo { code: "INVENTORY_INDEX_UNAVAILABLE", meaning: "`inventory sync` could not take the scenario-index lock because another canon process holds it; nothing was written", exit: 2, retryable: true, fix: "Wait for the other canon process, then run `canon inventory sync` again.", reasons: &[] },
    CodeInfo { code: "INVENTORY_INVALID", meaning: "`inventory sync` (or its preview) indexed nothing because a spec root has format violations or declares a scenario twice; nothing was written and `error.details` names each violation and duplicate by root; a sync that indexed an earlier root first is INVENTORY_INCOMPLETE", exit: 2, retryable: false, fix: "Fix each violation `error.details` names (`canon format` repairs what it can), then run `canon inventory sync` again.", reasons: INVENTORY_INVALID_REASONS },
    CodeInfo { code: "LAYOUT_MIGRATION_REQUIRED", meaning: "the spec root uses a pre-0.17 layout, or a command that writes records was refused in a repo whose ledger is laid out by canon 0.29; nothing was written; `canon migrate` moves it", exit: 2, retryable: false, fix: "Run `canon migrate` to preview the move, then apply it as the preview says.", reasons: &[] },
    CodeInfo { code: "LEDGER_ARCHIVE_INVALID", meaning: "`ledger compact` found no archive: the ledger root is not a directory named `ledger`", exit: 2, retryable: false, fix: "Point --ledger-root at a directory named `ledger`, or rename the ledger directory so it has an archive.", reasons: &[] },
    CodeInfo { code: "LEDGER_COMPACT_INCOMPLETE", meaning: "a compaction moved some superseded versions to the archive and then stopped, on a read of the ledger or a move that failed: what moved stays moved; the reason is the code of the step that failed", exit: 1, retryable: false, fix: "Read what was done first (the first `meta.next` step), fix the problem the error names, then preview with `canon ledger compact` and move the rest with `--commit --plan-digest <PLAN_DIGEST>` (the same digest is stale now).", reasons: LEDGER_COMPACT_INCOMPLETE_REASONS },
    CodeInfo { code: "MIGRATION_CONFLICT", meaning: "`migrate` refused because paths it moves have uncommitted changes", exit: 2, retryable: false, fix: "Commit or stash the uncommitted changes under the paths migrate moves, then run `canon migrate` again.", reasons: &[] },
    CodeInfo { code: "MIGRATION_DENIED", meaning: "`migrate` cannot use the scratch directory its preview needs, so it cannot judge the renamed ledger; nothing was written", exit: 2, retryable: false, fix: "Make the system temporary directory writable by this user, then run `canon migrate` again.", reasons: &[] },
    CodeInfo { code: "MIGRATION_INCOMPLETE", meaning: "`migrate` stopped after it had begun changing the repo; what it changed stays changed; the reason says where it stopped (`STOPPED` while moving, `INDEX` while rebuilding the scenario index, `PINS` while pinning the reviews)", exit: 1, retryable: false, fix: "Read what was done first (the first `meta.next` step), act on the fix of the reason, then run `canon migrate` again to finish the rest.", reasons: MIGRATION_INCOMPLETE_REASONS },
    CodeInfo { code: "MIGRATION_INVALID", meaning: "`migrate` cannot move the repository: its layout, a change directory or a record is not one it can move safely; nothing was written", exit: 2, retryable: false, fix: "Fix the layout problem the error names, then run `canon migrate` again.", reasons: &[] },
    CodeInfo { code: "MIGRATION_UNAVAILABLE", meaning: "`migrate` could not read or write the files of its preview because the system failed the call in a way a repeat can pass (an I/O error, an interrupted or timed-out call); nothing was written", exit: 2, retryable: true, fix: "Wait a moment, then run `canon migrate` again.", reasons: &[] },
    CodeInfo { code: "PLAN_CORPUS_DENIED", meaning: "a plan document the plan sources name exists but this user may not read it", exit: 2, retryable: false, fix: "Make the plan document the error names readable by this user, then run the command again.", reasons: &[] },
    CodeInfo { code: "PLAN_CORPUS_INVALID", meaning: "a plan document the plan sources name cannot be used to resolve the task: it is not a file or not UTF-8 text", exit: 2, retryable: false, fix: "Fix the plan sources configured in canon.yaml so every document is a readable text file.", reasons: &[] },
    CodeInfo { code: "PLAN_CORPUS_UNAVAILABLE", meaning: "a plan document the plan sources name exists but the system failed the read in a way a repeat can pass (an I/O error, an interrupted or timed-out call)", exit: 2, retryable: true, fix: "Wait a moment, then run the command again.", reasons: &[] },
    CodeInfo { code: "PLAN_DOCUMENT_INVALID", meaning: "the plan document has no row for the task, so there is nothing to flip; the plan document is a file people edit", exit: 2, retryable: false, fix: "Restore the task's row in its plan document, then run `canon task done` again.", reasons: &[] },
    CodeInfo { code: "PLAN_SOURCES_REQUIRED", meaning: "the repository configures no plan sources", exit: 2, retryable: false, fix: "Configure plan sources in canon.yaml, then run the command again.", reasons: &[] },
    CodeInfo { code: "PLAN_STALE", meaning: "the plan recomputed under `--commit` no longer has the `--plan-digest` the preview printed; nothing was written", exit: 2, retryable: false, fix: "Run the command again without --commit to preview the new plan, then act with `--commit --plan-digest <PLAN_DIGEST>`.", reasons: &[] },
    CodeInfo { code: "PLUGIN_REQUIRED", meaning: "`plugin sync` needs a plugin that is installed under `.canon/plugins/`, has an overlay source registered for it and declares an overlay; one of these is missing and nothing was written (a candidate that cannot be written is PLUGIN_SYNC_INCOMPLETE)", exit: 2, retryable: false, fix: "Install the plugin and check that it declares an overlay, then run `canon plugin sync` again.", reasons: &[] },
    CodeInfo { code: "PLUGIN_SYNC_INCOMPLETE", meaning: "`plugin sync` wrote some candidates and could not write others; the ones it wrote stay and data.roots names each error", exit: 1, retryable: false, fix: "Read what was done first (the first `meta.next` step and `data.roots`), fix each error listed in data.roots, then run `canon plugin sync` again.", reasons: PLUGIN_SYNC_INCOMPLETE_REASONS },
    CodeInfo { code: "POLICY_APPROVAL_CORRUPT", meaning: "`.canon/policy.approval.json` cannot be parsed, so the policy in force cannot be checked against its approval; shipping or reopening a subject is refused, and `--override-reason` does not waive it", exit: 2, retryable: false, fix: "Restore or repair `.canon/policy.approval.json`, or have the human run `canon policy approve` again.", reasons: &[] },
    CodeInfo { code: "POLICY_APPROVAL_DENIED", meaning: "`.canon/policy.approval.json` exists but this user may not read it, so the policy in force cannot be checked against its approval; shipping or reopening a subject is refused, and `--override-reason` does not waive it", exit: 2, retryable: false, fix: "Make `.canon/policy.approval.json` readable by this user, then run the command again.", reasons: &[] },
    CodeInfo { code: "POLICY_APPROVAL_FAILED", meaning: "the policy approval could not be written: the write of `.canon/policy.approval.json` failed", exit: 1, retryable: false, fix: "Check that .canon is writable, then have the human run `canon policy approve` again.", reasons: &[] },
    CodeInfo { code: "POLICY_APPROVAL_STALE", meaning: "`.canon/policy.yaml` differs from the bytes a human approved; shipping or reopening a subject refuses until it is re-approved or restored, and `--override-reason` does not waive it", exit: 2, retryable: false, fix: "Restore `.canon/policy.yaml` to the approved bytes, or have the human run `canon policy approve` again.", reasons: &[] },
    CodeInfo { code: "POLICY_APPROVAL_UNAVAILABLE", meaning: "`.canon/policy.approval.json` exists but the system failed the read in a way a repeat can pass (an I/O error, an interrupted or timed-out call), so the policy in force cannot be checked against its approval; shipping or reopening a subject is refused, and `--override-reason` does not waive it", exit: 2, retryable: true, fix: "Wait a moment, then run the command again.", reasons: &[] },
    CodeInfo { code: "POLICY_DENIED", meaning: "`policy approve` found `.canon/policy.yaml` but this user may not read it; nothing was written", exit: 2, retryable: false, fix: "Make `.canon/policy.yaml` readable by this user, then have the human run `canon policy approve` again.", reasons: &[] },
    CodeInfo { code: "POLICY_INVALID", meaning: "`policy approve` found a policy file that is no policy, or a path that is no file, where `.canon/policy.yaml` belongs; nothing was written. A policy file this user may not read is POLICY_DENIED", exit: 2, retryable: false, fix: "Make `.canon/policy.yaml` a valid policy file, then have the human run `canon policy approve` again.", reasons: &[] },
    CodeInfo { code: "POLICY_REQUIRED", meaning: "`policy approve` found no policy file to approve; nothing was written", exit: 2, retryable: false, fix: "Write a policy with `canon init`, then have the human run `canon policy approve` again.", reasons: &[] },
    CodeInfo { code: "POLICY_UNAVAILABLE", meaning: "`policy approve` found `.canon/policy.yaml` but the system failed the read in a way a repeat can pass (an I/O error, an interrupted or timed-out call); nothing was written", exit: 2, retryable: true, fix: "Wait a moment, then have the human run `canon policy approve` again.", reasons: &[] },
    CodeInfo { code: "PROMOTION_FAILED", meaning: "a staged record was written but promoting it to the ledger failed; nothing reached the ledger and the record stays staged, where `gate promote` takes it", exit: 1, retryable: false, fix: "Fix the ledger problem the error names, then run `canon gate promote` again.", reasons: &[] },
    CodeInfo { code: "PROMOTION_INVALID", meaning: "`gate promote` refused one or more staged records, which stay staged; error.details names each when there are several", exit: 2, retryable: false, fix: "Fix each record error.details names (the error itself when it names one), then run `canon gate promote` again.", reasons: &[] },
    CodeInfo { code: "PROMPT_BUNDLE_INVALID", meaning: "a prompt bundle is invalid", exit: 2, retryable: false, fix: "Fix the prompt bundle so it parses and matches its schema.", reasons: &[] },
    CodeInfo { code: "QUERY_ARGUMENT_INVALID", meaning: "a query flag is invalid for this kind", exit: 2, retryable: false, fix: "Remove the flag that does not apply to this kind; see `canon schema` for its options.", reasons: &[] },
    CodeInfo { code: "QUERY_FAILED", meaning: "the query could not read the ledger", exit: 1, retryable: false, fix: "Check that the ledger is readable, then run the query again.", reasons: &[] },
    CodeInfo { code: "REGIME_KEY_INVALID", meaning: "a regime key part is malformed", exit: 2, retryable: false, fix: "Pass a well-formed regime key; every part must be non-empty and valid.", reasons: &[] },
    CodeInfo { code: "RELAY_DIGEST_STALE", meaning: "a relayed brief approval names a digest that is not the brief's digest now, so the human was shown a different brief than the one on disk; nothing was recorded", exit: 2, retryable: false, fix: "Run `canon brief show <change> --body` again, put the brief and its digest in front of the human, ask again, and relay the digest of what they saw.", reasons: &[] },
    CodeInfo { code: "REPORT_CHECK_FAILED", meaning: "`report check` ran and its answer is that the report is not what it should be; the result is in data", exit: 1, retryable: false, fix: "Read data, then run `canon report build` or `canon report snapshot` to refresh the report.", reasons: REPORT_CHECK_FAILED_REASONS },
    CodeInfo { code: "REPORT_FAILED", meaning: "the report could not be built or checked", exit: 1, retryable: false, fix: "Read the error message, fix its cause, then run `canon report build` or `canon report check` again.", reasons: &[] },
    CodeInfo { code: "REPO_DENIED", meaning: "the repository or its ledger exists but this user may not read it", exit: 2, retryable: false, fix: "Make the repository directory the error names readable by this user, or pass --repo <path> for another root.", reasons: &[] },
    CodeInfo { code: "REPO_INVALID", meaning: "the path given as the repository is not a repository canon can read (it is missing or not a directory), or a tier canon.yaml names cannot be attached (a backend's credentials are unset)", exit: 2, retryable: false, fix: "Check that --repo points at a directory that holds canon.yaml and that its tiers can be attached, then run the command again.", reasons: &[] },
    CodeInfo { code: "REPO_LOCK_UNAVAILABLE", meaning: "another canon process held the repository write lock for the whole wait, so this command, which writes or commits, started nothing", exit: 2, retryable: true, fix: "A canon command that writes or commits is still running in this repository: run the same command again in a moment.", reasons: &[] },
    CodeInfo { code: "REPO_UNAVAILABLE", meaning: "the repository or its ledger exists but the system failed the read in a way a repeat can pass (an I/O error, an interrupted or timed-out call)", exit: 2, retryable: true, fix: "Wait a moment, then run the command again.", reasons: &[] },
    CodeInfo { code: "RETENTION_EXPORT_DENIED", meaning: "the export asked for sensitive records (`--include-sensitive`) and the policy does not allow them; nothing was written", exit: 2, retryable: false, fix: "Ask the human to set `query.allow_sensitive` in the policy, or export without --include-sensitive.", reasons: &[] },
    CodeInfo { code: "RETENTION_EXPORT_FAILED", meaning: "the export failed once it started writing: the output file or the access audit line could not be written", exit: 1, retryable: false, fix: "Check that the export destination is writable and has space, then run the export again.", reasons: &[] },
    CodeInfo { code: "RETENTION_PURGE_INCOMPLETE", meaning: "the purge stopped after it deleted some records: what was deleted stays deleted (`data` names the counts)", exit: 1, retryable: false, fix: "Read what was done first (the first `meta.next` step), fix the file-system problem the error names, then preview with `canon purge` and act with `--commit --plan-digest <PLAN_DIGEST>`.", reasons: RETENTION_PURGE_INCOMPLETE_REASONS },
    CodeInfo { code: "RETENTION_TIERS_CORRUPT", meaning: "a stored object of a tier a purge or export reads does not decode (a Parquet file canon wrote is damaged); nothing was deleted or written", exit: 2, retryable: false, fix: "Restore the damaged object the error names from a copy, or remove it, then run the command again.", reasons: &[] },
    CodeInfo { code: "RETENTION_TIERS_DENIED", meaning: "a file the tiers of a purge or export need (canon.yaml, a ledger directory) exists but this user may not read it; nothing was deleted or written", exit: 2, retryable: false, fix: "Make the file the error names readable by this user, then run the command again.", reasons: &[] },
    CodeInfo { code: "RETENTION_TIERS_INVALID", meaning: "the tiers cannot be used for a purge or export: the policy, a routing entry or the learn retention config is malformed, or a path they name is missing or no directory; nothing was deleted or written", exit: 2, retryable: false, fix: "Fix the tier or policy problem the error names in canon.yaml, then run the command again.", reasons: &[] },
    CodeInfo { code: "RETENTION_TIERS_REQUIRED", meaning: "a tier a purge or export needs is not attached or not configured: its backend's endpoint, DSN or credentials are unset, or canon.yaml has no entry for it; nothing was deleted or written, and waiting does not help", exit: 2, retryable: false, fix: "Set the variables the error names for the tier (or add its `tiers:` entry to canon.yaml), then run the command again.", reasons: &[] },
    CodeInfo { code: "RETENTION_TIERS_UNAVAILABLE", meaning: "a tier's backend a purge or export reads is down or does not answer (a SQL or object-store error, or a Postgres DSN that is set and cannot be reached), or the system failed the read of a file in a way a repeat can pass; nothing was deleted or written", exit: 2, retryable: true, fix: "Check that the tier backends are reachable, then run the command again.", reasons: &[] },
    CodeInfo { code: "RETRIEVE_INVALID", meaning: "`retrieve` could not start: the domain/subject cannot be turned into a regime key (the scope flags that conflict, are missing or disagree with --role are `USAGE`); a store outage is not an error here, it serves empty guidance (exit 0)", exit: 2, retryable: false, fix: "Check --domain and --subject (`canon regime show` prints a valid key) and that the repo directory has a usable name, then run the retrieval again.", reasons: &[] },
    CodeInfo { code: "REVIEW_ARGUMENT_INVALID", meaning: "a `review scenario` or `review change` argument is missing or contradicts another", exit: 2, retryable: false, fix: "Add the missing `canon review` argument or drop the one that contradicts another.", reasons: &[] },
    CodeInfo { code: "REVIEW_ARTIFACT_DENIED", meaning: "a review artifact lies outside the repository, or this user may not read it", exit: 2, retryable: false, fix: "Copy the file anywhere under the repository (or make it readable) and bind that path.", reasons: &[] },
    CodeInfo { code: "REVIEW_ARTIFACT_INVALID", meaning: "a review artifact cannot be bound: it is not a file or is over the size limit", exit: 2, retryable: false, fix: "Bind a file the rules in the error accept; `canon schema review` lists the artifact options and their limits.", reasons: &[] },
    CodeInfo { code: "REVIEW_ARTIFACT_NOT_FOUND", meaning: "a review artifact does not exist", exit: 2, retryable: false, fix: "Bind an artifact that exists inside the repository.", reasons: &[] },
    CodeInfo { code: "REVIEW_ARTIFACT_STALE", meaning: "a review artifact changed after it was bound, so its bytes no longer match the digest taken first", exit: 2, retryable: false, fix: "Bind the file again so the digest is taken from its current bytes.", reasons: &[] },
    CodeInfo { code: "REVIEW_ARTIFACT_UNAVAILABLE", meaning: "a review artifact exists but the system failed the read in a way a repeat can pass (an I/O error, an interrupted or timed-out call); nothing was bound", exit: 2, retryable: true, fix: "Wait a moment, then run the command again.", reasons: &[] },
    CodeInfo { code: "ROLE_INVALID", meaning: "`CANON_ROLE` holds a value that is not a role (planner, implementer, reviewer, orchestrator)", exit: 2, retryable: false, fix: "Set CANON_ROLE to planner, implementer, reviewer or orchestrator, or unset it.", reasons: &[] },
    CodeInfo { code: "SCAFFOLD_ARGUMENT_INVALID", meaning: "a scaffold argument is invalid", exit: 2, retryable: false, fix: "Correct the scaffold argument named in the error.", reasons: &[] },
    CodeInfo { code: "SCENARIO_INDEX_REQUIRED", meaning: "an evidence or review call names a scenario that the scenario index does not hold; no record was written", exit: 2, retryable: false, fix: "Run `canon inventory sync` so the scenario is indexed, or pass the id of a scenario the spec root declares.", reasons: &[] },
    CodeInfo { code: "SCENARIO_NOT_FOUND", meaning: "--scenario names a scenario the index does not know", exit: 2, retryable: false, fix: "Pass a --scenario that the scenario index knows; run `canon inventory sync` if the scenario is new.", reasons: &[] },
    CodeInfo { code: "SCENARIO_TAG_EXISTS", meaning: "the scenario tag already exists", exit: 2, retryable: false, fix: "Choose a tag that is not already used, or edit the existing scenario.", reasons: &[] },
    CodeInfo { code: "SEARCH_INDEX_FAILED", meaning: "`search` or `trace` could not read the records the index is built from, or could not build or read the index", exit: 1, retryable: false, fix: "Fix or restore what the message names (an unreadable store, a record file), then run the command again; the index is derived, so deleting `.canon/index` is always safe.", reasons: &[] },
    CodeInfo { code: "SEARCH_QUERY_REQUIRED", meaning: "`search` was given an empty text or one of only spaces", exit: 2, retryable: false, fix: "Pass the text to find, e.g. `canon search timeout`.", reasons: &[] },
    CodeInfo { code: "SELFTEST_FAILED", meaning: "a selftest suite or fixture did not match its expected result", exit: 1, retryable: false, fix: "Read the failing suite or fixture in the output and fix the behavior it checks.", reasons: &[] },
    CodeInfo { code: "SERIALIZE_FAILED", meaning: "the result could not be serialized", exit: 1, retryable: false, fix: "Report this canon bug with the command you ran; it cannot be fixed from the command line.", reasons: &[] },
    CodeInfo { code: "SKILLS_ARGUMENT_INVALID", meaning: "the install lock names a harness canon does not know", exit: 2, retryable: false, fix: "Run `canon skills install` with --harness once for each of claude, codex, omp, pi (or --no-harness), which writes the lock again.", reasons: &[] },
    CodeInfo { code: "SKILLS_CHECK_FAILED", meaning: "`skills check` ran and its answer is that the installed skills differ from the bundle", exit: 1, retryable: false, fix: "Run `canon skills install` to refresh the installed skills.", reasons: SKILLS_CHECK_FAILED_REASONS },
    CodeInfo { code: "SKILLS_FAILED", meaning: "a skills command failed", exit: 1, retryable: false, fix: "Read the error message, fix its cause, then run the skills command again.", reasons: &[] },
    CodeInfo { code: "SKILLS_SOURCE_NOT_FOUND", meaning: "the `--skills-source` directory does not exist or holds no SKILL.src.md", exit: 2, retryable: false, fix: "Point --skills-source at a directory that holds SKILL.src.md, or drop it to use the text built into canon.", reasons: &[] },
    CodeInfo { code: "SPEC_ROOT_DENIED", meaning: "a spec root directory of canon.yaml exists but this user may not read it, so a brief's flow steps cannot be matched to scenarios", exit: 2, retryable: false, fix: "Make the spec root directory the error names readable by this user, then run the command again.", reasons: &[] },
    CodeInfo { code: "SPEC_ROOT_INVALID", meaning: "a spec root directory of canon.yaml is no directory (a file stands there), so a brief's flow steps cannot be matched to scenarios", exit: 2, retryable: false, fix: "Make the spec root path a directory, or fix `specs.roots` in canon.yaml, then run the command again.", reasons: &[] },
    CodeInfo { code: "SPEC_ROOT_NOT_FOUND", meaning: "`--project` names no spec root configured in canon.yaml, so nothing was written", exit: 2, retryable: false, fix: "Pass a --project that names a spec root configured in canon.yaml.", reasons: &[] },
    CodeInfo { code: "SPEC_ROOT_REQUIRED", meaning: "no single spec root could be chosen; pass --project", exit: 2, retryable: false, fix: "Pass --project with the id of the spec root to use.", reasons: &[] },
    CodeInfo { code: "SPEC_ROOT_UNAVAILABLE", meaning: "a spec root directory of canon.yaml exists but the system failed the read in a way a repeat can pass (an I/O error, an interrupted or timed-out call)", exit: 2, retryable: true, fix: "Wait a moment, then run the command again.", reasons: &[] },
    CodeInfo { code: "STRATEGY_ARGUMENT_INVALID", meaning: "a `strategy` flag value is invalid", exit: 2, retryable: false, fix: "Correct the `canon strategy` flag value; see `canon schema` for the command's options.", reasons: &[] },
    CodeInfo { code: "STRATEGY_DEMOTION_FAILED", meaning: "promotion is blocked and the demotion a contradicting trajectory triggered also failed; the strategy is still active", exit: 1, retryable: false, fix: "Run `canon strategy rollback` on the still-active strategy, then retry the promotion.", reasons: &[] },
    CodeInfo { code: "STRATEGY_EVALUATION_INVALID", meaning: "the promotion evaluation does not fit the candidate strategy", exit: 2, retryable: false, fix: "Supply an evaluation file that fits the candidate strategy, then run `canon strategy promote` again.", reasons: &[] },
    CodeInfo { code: "STRATEGY_FILE_INVALID", meaning: "a `strategy` evaluation or approval file is invalid", exit: 2, retryable: false, fix: "Fix the evaluation or approval file so it parses and matches its schema.", reasons: &[] },
    CodeInfo { code: "STRATEGY_INCOMPLETE", meaning: "`strategy promote` demoted an active strategy that a contradicting trajectory turned off, and then the promotion gate blocked it; the strategy stays demoted; `reason` is the gate's refusal", exit: 1, retryable: false, fix: "Read what was done first (the first `meta.next` step), then record the evidence the gate asks for (`canon ingest records`).", reasons: STRATEGY_INCOMPLETE_REASONS },
    CodeInfo { code: "STRATEGY_NOT_FOUND", meaning: "no strategy with that id; `strategy rollback` and `strategy approval-payload` look the id up first", exit: 2, retryable: false, fix: "Pass the id of an existing strategy.", reasons: &[] },
    CodeInfo { code: "STRATEGY_PROMOTION_FAILED", meaning: "the strategy could not be promoted: its record or file could not be read or written, and nothing of the promotion was kept", exit: 1, retryable: false, fix: "Read the error message, fix its cause, then run `canon strategy promote` again.", reasons: &[] },
    CodeInfo { code: "STRATEGY_PROMOTION_INVALID", meaning: "the promotion gate blocks this strategy; nothing was written", exit: 2, retryable: false, fix: "Resolve what the promotion gate reports for this strategy, then run `canon strategy promote` again.", reasons: &[] },
    CodeInfo { code: "STRATEGY_ROLLBACK_FAILED", meaning: "the strategy exists and its signature verified, but the rollback could not be recorded", exit: 1, retryable: false, fix: "Read the error message, fix its cause, then run `canon strategy rollback` again.", reasons: &[] },
    CodeInfo { code: "STRATEGY_SIGNATURE_DENIED", meaning: "the detached signature file exists but this user may not read it", exit: 2, retryable: false, fix: "Make the --signature file readable by this user, then run the command again.", reasons: &[] },
    CodeInfo { code: "STRATEGY_SIGNATURE_INVALID", meaning: "the detached signature of a strategy rollback is there but does not verify against the actor's key; nothing was recorded", exit: 2, retryable: false, fix: "Sign the rollback again with a key the policy lists, then run `canon strategy rollback` again.", reasons: &[] },
    CodeInfo { code: "STRATEGY_SIGNATURE_NOT_FOUND", meaning: "the detached signature file does not exist", exit: 2, retryable: false, fix: "Pass --signature naming an existing detached signature file.", reasons: &[] },
    CodeInfo { code: "STRATEGY_SIGNATURE_REQUIRED", meaning: "the --signature-file of a `strategy rollback` is empty, so there is no signature to check; nothing was recorded", exit: 2, retryable: false, fix: "Sign the rollback's payload with the actor's key and pass the armored signature, then run `canon strategy rollback` again.", reasons: &[] },
    CodeInfo { code: "STRATEGY_SIGNATURE_UNAVAILABLE", meaning: "the detached signature file exists but the system failed the read in a way a repeat can pass (an I/O error, an interrupted or timed-out call)", exit: 2, retryable: true, fix: "Wait a moment, then run the command again.", reasons: &[] },
    CodeInfo { code: "STRATEGY_SIGNER_DENIED", meaning: "the --actor of a `strategy rollback` is not listed in the policy's allowed_signers file, so it may not sign a rollback; nothing was recorded", exit: 2, retryable: false, fix: "Pass an --actor the allowed_signers file lists, or have the human add the actor's key to it and approve the policy (`canon policy approve`), then run `canon strategy rollback` again.", reasons: &[] },
    CodeInfo { code: "STRATEGY_VERIFIER_REQUIRED", meaning: "`.canon/policy.yaml` names no `approval.allowed_signers` verifier, so a `strategy rollback` cannot be checked; nothing was recorded", exit: 2, retryable: false, fix: "Set `approval: { allowed_signers: <path> }` in .canon/policy.yaml and have the human approve it with `canon policy approve`, then run `canon strategy rollback` again.", reasons: &[] },
    CodeInfo { code: "SUBJECT_APPROVAL_REQUIRED", meaning: "the status transition needs an approval, a review or evidence the subject does not have; error.details names each cause when there are several, and nothing was written", exit: 2, retryable: false, fix: "Get what error.details names (a human's approval of the brief, an independent review, evidence for the scenario), then run `canon subject move` again.", reasons: &[] },
    CodeInfo { code: "SUBJECT_ARGUMENT_INVALID", meaning: "a `subject` argument is malformed", exit: 2, retryable: false, fix: "Correct the malformed `canon subject` argument.", reasons: &[] },
    CodeInfo { code: "SUBJECT_CONFLICT", meaning: "the requested change conflicts with the subject's state: a status transition that is not allowed, an unadopt of an approved or flow-tagged change", exit: 2, retryable: false, fix: "Check the subject with `canon subject show` and request what its state allows.", reasons: SUBJECT_CONFLICT_REASONS },
    CodeInfo { code: "SUBJECT_EXISTS", meaning: "a subject with that id already exists", exit: 2, retryable: false, fix: "Choose a different subject id, or use the existing subject (`canon subject show`).", reasons: &[] },
    CodeInfo { code: "SUBJECT_INVALID", meaning: "the status transition breaks a rule of the gate that is not a missing approval, review or evidence; error.details names each cause when there are several", exit: 2, retryable: false, fix: "Fix each cause error.details names (the error itself when it names one), then run `canon subject move` again.", reasons: SUBJECT_INVALID_REASONS },
    CodeInfo { code: "SUBJECT_NOT_FOUND", meaning: "no subject with that id", exit: 2, retryable: false, fix: "Pass the id of an existing subject; `canon status` lists the subjects.", reasons: &[] },
    CodeInfo { code: "SUBJECT_REASON_REQUIRED", meaning: "reopening a shipped subject needs --reason", exit: 2, retryable: false, fix: "Pass --reason when reopening a shipped subject with `canon subject move`.", reasons: &[] },
    CodeInfo { code: "TASK_EVIDENCE_INVALID", meaning: "the evidence or typed atom for the task is invalid", exit: 2, retryable: false, fix: "Correct the task's evidence or typed atom named in the error, then run `canon task done` again.", reasons: &[] },
    CodeInfo { code: "TASK_EVIDENCE_REQUIRED", meaning: "`task done` refused to flip the task: its evidence does not support it", exit: 2, retryable: false, fix: "Attach the evidence the task requires, then run `canon task done` again.", reasons: &[] },
    CodeInfo { code: "TASK_ID_INVALID", meaning: "the task id is malformed", exit: 2, retryable: false, fix: "Pass a well-formed task id to `canon task done`.", reasons: &[] },
    CodeInfo { code: "TASK_NOT_FOUND", meaning: "the task id names no task in the plan corpus", exit: 2, retryable: false, fix: "Pass a task id that exists in the plan corpus.", reasons: &[] },
    CodeInfo { code: "TASK_UNSUPPORTED", meaning: "the plan dialect cannot flip this task", exit: 2, retryable: false, fix: "Mark the task done by hand in the plan document; this plan dialect cannot be flipped by canon.", reasons: &[] },
    CodeInfo { code: "TIER_FAILED", meaning: "tier aging failed before it moved any record, or a move failed and no record had left its tier", exit: 1, retryable: false, fix: "Read the error message, fix its cause, then run tier aging again.", reasons: &[] },
    CodeInfo { code: "TIER_INCOMPLETE", meaning: "tier aging stopped after it moved some records: what moved stays moved", exit: 1, retryable: false, fix: "Read what was done first (the first `meta.next` step), fix the cause the error names, then preview with `canon tier age` and move the rest with `--commit --plan-digest <PLAN_DIGEST>` (the same digest is stale now).", reasons: TIER_INCOMPLETE_REASONS },
    CodeInfo { code: "TIER_UNAVAILABLE", meaning: "tier aging could not read a tier before it moved any record: a tier backend whose settings are all there did not answer (a Postgres DSN that is set and cannot be reached), or the system failed the read of a file in a way a repeat can pass; nothing was moved", exit: 2, retryable: true, fix: "Wait a moment, check that the tier backends are reachable, then run tier aging again.", reasons: &[] },
    CodeInfo { code: "TRACE_CONFLICT", meaning: "the sha prefix `trace` was given is the start of more than one commit", exit: 2, retryable: false, fix: "Pass more characters of the commit sha; `expected` lists the candidates.", reasons: &[] },
    CodeInfo { code: "TRACE_NOT_FOUND", meaning: "no record says the id `trace` was given", exit: 2, retryable: false, fix: "Pass the key of a finding (`canon query --kind finding`), a review id a review printed, or enough characters of a commit sha to name one commit.", reasons: &[] },
    CodeInfo { code: "TRACE_OPERAND_INVALID", meaning: "`trace` was given something that is none of a review id, a commit sha or a finding key (a placeholder node's id is not one)", exit: 2, retryable: false, fix: "Pass a review id (26 characters), a commit sha (7 to 40 hex digits) or a finding key (`<change>_<round>_<seq>`).", reasons: &[] },
    CodeInfo { code: "TRAJECTORY_NOT_FOUND", meaning: "the contradicting trajectory does not exist", exit: 2, retryable: false, fix: "Pass the id of a trajectory that exists, or omit the contradicting trajectory.", reasons: &[] },
    CodeInfo { code: "TYPED_ATOMS_INVALID", meaning: "the typed-atoms file for the change does not parse", exit: 2, retryable: false, fix: "Fix the change's typed-atoms file so it parses.", reasons: &[] },
    CodeInfo { code: "UNKNOWN_COMMAND", meaning: "a command word is not in canon's command tree, or `canon schema` was given a path canon does not have; `expected` lists the valid words at that level", exit: 2, retryable: false, fix: "Run one of the commands `expected` lists; `canon schema` lists every command path.", reasons: &[] },
    CodeInfo { code: "UNVERIFIABLE", meaning: "a check examined nothing, so it cannot report clean: data.unverifiable is true and meta.coverage says what was looked at", exit: 1, retryable: false, fix: "Widen the check so it examines something; meta.coverage says what was looked at.", reasons: &[] },
    CodeInfo { code: "USAGE", meaning: "the command line did not parse or breaks a constraint: an unknown option, a missing or malformed value, an unexpected operand, a `--fields` name that cannot be selected, `--input` given twice or without a value, `--input -` beside another option that reads stdin, a retrieval scoped by neither or both of `--regime` and `--domain`; `field` names the argument", exit: 2, retryable: false, fix: "Run the command with --help and correct the argument `field` names; `expected` lists the valid values when there is a closed set.", reasons: &[] },
    CodeInfo { code: "WRITE_DENIED", meaning: "a target the command writes is read-only or this user may not write it, known before anything was changed; nothing was written", exit: 2, retryable: false, fix: "Make the target writable (its permissions, a read-only mount), then run the command again.", reasons: &[] },
    CodeInfo { code: "WRITE_FAILED", meaning: "a write began and failed: a file or ledger record could not be written after the command started changing things; what an earlier step wrote stays, and the message says which", exit: 1, retryable: false, fix: "Check that the target path is writable and has space, then run the command again.", reasons: &[] },
    CodeInfo { code: "WRITE_INVALID", meaning: "a target the command writes cannot be a target: its directory is missing or is not a directory, known before anything was changed; nothing was written", exit: 2, retryable: false, fix: "Create the missing directory or name a directory that exists, then run the command again.", reasons: &[] },
];

/// Every code canon says as a warning (`meta.warnings`), sorted by code (spec 7.5.2). A code that is also an error
/// ([`all_codes`]) is in both, with the same meaning (7.5.3).
const WARNINGS: &[WarningInfo] = &[
    WarningInfo { code: "BRIEF_ADOPTION_UNAPPROVED", meaning: "a change was adopted into a building, verifying or shipped subject while its brief is not approved; the subject's existing scenarios keep accepting evidence, but scenarios tagged for the change wait for the approval", fix: "Have the human approve the brief with `canon brief approve`, or take the adoption back with `canon subject unadopt`." },
    WarningInfo { code: "BRIEF_DRAFT_PENDING", meaning: "a subject transition ran while a change of the subject is a pending draft (never approved, not a chore, no scenario tagged for it), so the transition does not judge that change", fix: "Tag a scenario for the draft change or have the human approve its brief if it belongs to this transition; otherwise ignore the note." },
    WarningInfo { code: "BRIEF_OVERRIDE_ON_RECORD", meaning: "an evidence record was attested while its brief was not approved, under `--override-reason`; the waiver stays on the record", fix: "Have the human approve the brief with `canon brief approve`, then attest the evidence again." },
    WarningInfo { code: "BRIEF_UNAPPROVED_NOT_ENFORCED", meaning: "a subject moved past a brief that is not approved but cannot be enforced (the change has no brief canon reads, or the `plans:` section is unreadable)", fix: "Give the change a brief with `canon change new`, or fix the `plans:` section of canon.yaml, then have the human approve the brief." },
    WarningInfo { code: "CODE_SCOPES_UNMATCHED", meaning: "a `code_scopes` key matches no scenario; or a pattern of it matches no file of the code tree, and when none of a key's patterns does the key declares no scope and its scenarios follow the rule without scopes; or the key is malformed (not a map of non-empty lists of non-empty patterns; an empty list is malformed) and no scenario has a scope", fix: "Correct the key or the pattern in `.canon/policy.yaml` (matching is byte-exact), then have the human run `canon policy approve` again." },
    WarningInfo { code: "CONFIG_MISSING", meaning: "canon.yaml does not exist", fix: "Run `canon init` or pass --repo <DIR> to a directory holding canon.yaml." },
    WarningInfo { code: "CONFIG_UNREADABLE", meaning: "canon.yaml, policy or a setting it carries could not be read or is invalid", fix: "Fix the canon.yaml, policy or setting named in the error, then run `canon init --check` to confirm." },
    WarningInfo { code: "DASHBOARD_SNAPSHOT_UNAVAILABLE", meaning: "the dashboard started without a readable panels snapshot, so its panels page shows the reason instead of panels", fix: "Run `canon report snapshot` for the repo, or pass a readable --snapshot directory, then start the dashboard again." },
    WarningInfo { code: "DISPATCH_DIFF_RECORDS_UNAVAILABLE", meaning: "`dispatch diff` could not read the task or run records of a tier, so that side of the diff is empty", fix: "Fix the tier routing or store the message names, then run `canon dispatch diff` again." },
    WarningInfo { code: "DISPATCH_MANIFEST_MISMATCH", meaning: "the manifest's run id differs from its file name", fix: "Rename the manifest to match its run id, or correct the run id inside it." },
    WarningInfo { code: "DISPATCH_MANIFEST_UNREADABLE", meaning: "the dispatch manifest is not a readable run record", fix: "Restore the dispatch manifest as a readable run record." },
    WarningInfo { code: "DISPATCH_RUN_COPIES_DIVERGE", meaning: "a run's tier copy and its dispatch manifest differ; the tier copy is used", fix: "Run `canon dispatch end` to bring the two copies back together." },
    WarningInfo { code: "DISPATCH_TIER_NOT_PERSISTED", meaning: "a dispatch begin or end could not be persisted to the run's routed tier; the manifest file is the only home of the record", fix: "Fix the tier the message names, then run `canon dispatch end` again so the tier holds the run." },
    WarningInfo { code: "LAYOUT_MIGRATION_REQUIRED", meaning: "the spec root uses a pre-0.17 layout, or a command that writes records was refused in a repo whose ledger is laid out by canon 0.29; nothing was written; `canon migrate` moves it", fix: "Run `canon migrate` to move the spec root to the current layout." },
    WarningInfo { code: "LEDGER_KIND_ROUTED_AWAY", meaning: "a record kind is routed away from the `local` rung that status reads, so its records are not counted", fix: "Route the kind to the `local` rung in canon.yaml, then run `canon status` again." },
    WarningInfo { code: "LEDGER_NO_SCENARIOS", meaning: "the ledger holds no scenario records, so every subject reads as owning none", fix: "Run `canon inventory sync` to index the scenarios the .feature files hold." },
    WarningInfo { code: "LEDGER_RECORDS_UNREADABLE", meaning: "ledger record files could not be read and are not counted", fix: "Run `canon gate check` to name the unreadable records, then repair or rename them." },
    WarningInfo { code: "LEDGER_ROLLBACK_NOT_RESTORED", meaning: "a batch write was rolled back but the archived versions its records had replaced were not put back", fix: "Copy the archived versions back from the archive beside the ledger." },
    WarningInfo { code: "LEDGER_UNREADABLE", meaning: "the repo's ledger could not be read, so nothing could be counted", fix: "Run `canon init --check` to see what is wrong in canon.yaml, fix it, then run the command again." },
    WarningInfo { code: "LEGACY_GUIDE_STALE", meaning: "the installed canon skill files are the 0.27 guide and name commands later versions removed", fix: "Do not follow the old skill files; run `canon migrate --json`, then read `canon skills show canon`." },
    WarningInfo { code: "LEGACY_STATE_FOUND", meaning: "the repo holds 0.27-era canon state that later versions do not read", fix: "Run `canon migrate --json` to preview the upgrade, then apply it as the preview says." },
    WarningInfo { code: "OLD_BRIEF_FORMAT_FOUND", meaning: "a read command found briefs in the format canon 0.30 and earlier wrote; the commands that write and judge them are refused until they are rewritten", fix: "Run `canon migrate --json` to preview the rewrite, then apply it as the preview says." },
    WarningInfo { code: "OLD_LAYOUT_FOUND", meaning: "a read command found a ledger laid out by canon 0.29, so the records named by the old rule are not counted and writes are refused", fix: "Run `canon migrate --json` to preview the move, then `canon migrate --commit --plan-digest <digest>`." },
    WarningInfo { code: "OVERRIDE_REASON_UNUSED", meaning: "`--override-reason` was given but nothing was refused, so no override was recorded", fix: "Drop --override-reason; the command needed no override." },
    WarningInfo { code: "OVERRIDE_RECORDED", meaning: "a refusal was waived under `--override-reason` and the waiver is recorded; a gap that appears later is not covered", fix: "Have the human review the recorded waiver and close the gap it names." },
    WarningInfo { code: "OVERRIDE_REFUSAL_WAIVED", meaning: "a refusal was waived under `--override-reason`", fix: "Close the gap the waived refusal names, so the waiver is no longer needed." },
    WarningInfo { code: "POLICY_APPROVAL_UNREADABLE", meaning: "`.canon/policy.approval.json` cannot be read, so the policy in force cannot be checked against its approval; shipping or reopening a subject is refused as a closed gate (exit 2), and `--override-reason` does not waive it", fix: "Restore or repair `.canon/policy.approval.json`, or have the human run `canon policy approve` again." },
    WarningInfo { code: "POLICY_CHANGED_SINCE_APPROVAL", meaning: "`.canon/policy.yaml` differs from the bytes a human approved; shipping or reopening a subject refuses until it is re-approved or restored, and `--override-reason` does not waive it", fix: "Restore `.canon/policy.yaml` to the approved bytes, or have the human run `canon policy approve` again." },
    WarningInfo { code: "POLICY_MISSING", meaning: "`.canon/policy.yaml` does not exist, so the gate requires no evidence, failure cases or review", fix: "Write `.canon/policy.yaml` with a `spec_coverage` section and have the human approve it." },
    WarningInfo { code: "POLICY_NOT_APPROVED", meaning: "`.canon/policy.yaml` has no approval record", fix: "Have a human review the policy and run `canon policy approve --approver <who>`." },
    WarningInfo { code: "POLICY_SPEC_COVERAGE_ABSENT", meaning: "the policy has no `spec_coverage` section, so the gate requires no evidence, failure cases or review", fix: "Add a `spec_coverage` section to `.canon/policy.yaml` and have the human approve it." },
    WarningInfo { code: "POLICY_SPEC_COVERAGE_INVALID", meaning: "the policy's `spec_coverage` section is unusable, so the gate refuses and the review guard cannot read whether review is required", fix: "Fix the `spec_coverage` section of `.canon/policy.yaml`; `canon gate check` reports what is wrong." },
    WarningInfo { code: "POLICY_UNREADABLE", meaning: "`.canon/policy.yaml` cannot be read and falls back to defaults, so the gate requires no evidence, failure cases or review", fix: "Fix `.canon/policy.yaml` so it parses, then have the human approve it." },
    WarningInfo { code: "QUERY_PLUGIN_DIAGNOSTIC", meaning: "a `query --plugin` projection reported a diagnostic; the records are listed without that projection where it failed", fix: "Read the diagnostic in the message and fix the plugin or the record it names." },
    WarningInfo { code: "REFERENCES_UNCHECKED", meaning: "the sources that records refer to could not be read, so references to missing sources were not checked", fix: "Make the sources readable (the message names the cause), then run the command again." },
    WarningInfo { code: "REPORT_TIER_NOT_READ_DIRECTLY", meaning: "a report was built from a ledger where some record kinds are not read directly from their routed tier", fix: "Route the kinds the message names to a rung the report reads, then run the report again." },
    WarningInfo { code: "RETIRED_KIND_KEYS", meaning: "canon.yaml still names the `session` or `event` record kind under `routing` or `aging`; canon 0.39 has no such kinds, so the keys route and age nothing", fix: "Run `canon migrate --json` to preview deleting the lines, then `canon migrate --commit --plan-digest <digest>`." },
    WarningInfo { code: "RETRIEVE_REGIME_FELL_BACK", meaning: "`retrieve` served guidance from a regime derived by fallback instead of the one asked for", fix: "Pass the regime that holds the guidance you want with --regime, or record guidance under the requested regime." },
    WarningInfo { code: "REVIEW_RECORDS_UNMIGRATED", meaning: "scenarios have a review that predates review pinning and does not count until it is rewritten", fix: "Run `canon migrate` to rewrite the old reviews to what the evidence says." },
    WarningInfo { code: "ROLE_UNKNOWN_RECORDED", meaning: "`--as` or `--role` names a role that is not a known role or one the policy names; the command runs and records the value as given", fix: "Pass planner, implementer, reviewer, orchestrator or a role the policy names." },
    WarningInfo { code: "SCENARIO_INDEX_CHECK_FAILED", meaning: "whether the scenario index is stale could not be checked", fix: "Run `canon inventory sync` and read the error it names." },
    WarningInfo { code: "SCENARIO_INDEX_NOT_REFRESHED", meaning: "the scenario index is stale and could not be refreshed, so graded answers may describe specs the files no longer say", fix: "Run `canon inventory sync`; it names each format violation, duplicate id or lock problem to fix." },
    WarningInfo { code: "SCENARIO_INDEX_REFRESHED", meaning: "a read command refreshed a stale scenario index from changed .feature files and reports what it wrote", fix: "Run `canon inventory sync` to list any tag diagnostics the refresh found." },
    WarningInfo { code: "SCENARIO_SUBJECT_UNKNOWN", meaning: "scenarios are tagged `@subject:<id>` but no subject with that id exists", fix: "Create the subject with `canon subject new`, or correct the `@subject` tag." },
    WarningInfo { code: "SEARCH_GITIGNORE_MISSING", meaning: "the repo's .gitignore lacks the line of the search index directory", fix: "Run `canon init` or `canon migrate` to add the line to .gitignore." },
    WarningInfo { code: "SEARCH_INDEX_IN_MEMORY", meaning: "the search index file could not be written or deleted, so the index was built in memory for this run", fix: "Make `.canon/index` writable, then run the command again." },
    WarningInfo { code: "SEARCH_INDEX_REBUILT", meaning: "the search index file was not a usable database, so it was deleted and rebuilt", fix: "Nothing to do; if it recurs, check the disk and permissions of `.canon/index`." },
    WarningInfo { code: "STRATEGY_DEMOTED", meaning: "later contradictory evidence demoted an active strategy while its promotion was blocked", fix: "Review the contradicting trajectory before promoting the strategy again." },
    WarningInfo { code: "SUBJECT_OVERRIDE_ON_RECORD", meaning: "a subject stands in its status under a recorded `--override-reason` waiver of gate refusals", fix: "Close the gaps the waiver names, then move the subject again without an override." },
    WarningInfo { code: "SUBJECT_REOPENED", meaning: "a shipped subject was reopened for a follow-up change; shipping again runs every guard", fix: "Keep the follow-up in its change, then scenario, evidence and review, before shipping again." },
    WarningInfo { code: "TASK_PROOF_IS_CANON_COMMAND", meaning: "a task was flipped on evidence that binds no file and cites a canon command, which reports on canon's own records and proves nothing", fix: "Bind the artifact or report that proves the task with `canon evidence add --artifact <path>`." },
    WarningInfo { code: "TASK_REIMPORT_INCOMPLETE", meaning: "a task was flipped but its change did not re-import cleanly, so the task status record may disagree with the plan", fix: "Run `canon ingest plans` to see why the change did not import." },
    WarningInfo { code: "TASK_STATUS_NOT_REFRESHED", meaning: "a task was flipped but its status record was not refreshed, so `canon query --kind task` may not match the plan", fix: "Run `canon ingest plans` so the task records match the plan." },
    WarningInfo { code: "UNKNOWN_ROLE_IGNORED", meaning: "`CANON_ROLE` holds a value that is not a role and was ignored, because the command (`schema`, `context`, `skills show`) does not depend on the role", fix: "Set CANON_ROLE to planner, implementer, reviewer or orchestrator, or unset it." },
];
#[cfg(test)]
mod fix_tests {
    use super::all_codes;

    #[test]
    fn every_code_has_a_clean_fix() {
        for info in all_codes() {
            assert!(!info.fix.is_empty(), "{} has an empty fix", info.code);
            assert_eq!(info.fix, info.fix.trim_end(), "{} fix ends in whitespace", info.code);
            assert!(!info.fix.contains("...") && !info.fix.contains('\u{2026}'), "{} fix has an ellipsis", info.code);
        }
    }

    #[test]
    fn the_registry_is_sorted_by_code_without_repeats_and_every_exit_is_one_or_two() {
        let codes = all_codes();
        assert!(codes.windows(2).all(|pair| pair[0].code < pair[1].code), "the registry must be sorted by code, each once");
        assert!(codes.iter().all(|info| matches!(info.exit, 1 | 2)), "a registry exit is 1 or 2");
    }
}

/// `canon-0-35-crate-1-2` (`atip.crate12.16` and `.17`): canon's own code table can carry a `<SUBJECT>_INCOMPLETE` row (spec 8.3.2) whose
/// reasons are made from its exit-2 rows, with the helpers of the `atip` crate. The probe row has another name, so it
/// stays a probe whatever canon registers.
#[cfg(test)]
mod incomplete_probe {
    use atip::{count_with_exit, find_code, reasons_with_exit, with_row, CodeInfo, ReasonInfo, EXIT_FAILED, EXIT_REFUSED};

    use super::CODES;

    const REASONS: [ReasonInfo; count_with_exit(CODES, EXIT_REFUSED)] = reasons_with_exit(CODES, EXIT_REFUSED);
    const PROBE: CodeInfo = CodeInfo { code: "RUN_INCOMPLETE_PROBE", meaning: "probe", exit: EXIT_FAILED, retryable: false, fix: "probe", reasons: &REASONS };
    static WITH_PROBE: [CodeInfo; CODES.len() + 1] = with_row(CODES, PROBE);

    #[test]
    fn atip_crate12_16_canons_code_table_gives_a_probe_row_every_exit_2_row_as_a_reason() {
        let refusals: Vec<&str> = CODES.iter().filter(|row| row.exit == EXIT_REFUSED).map(|row| row.code).collect();
        assert!(refusals.len() > 10, "canon has exit-2 rows");
        let reasons: Vec<&str> = REASONS.iter().map(|reason| reason.reason).collect();
        assert_eq!(reasons, refusals, "the reasons are the exit-2 codes, in the table's sort order");
        for row in CODES.iter().filter(|row| row.exit == EXIT_REFUSED) {
            let reason = REASONS.iter().find(|reason| reason.reason == row.code).expect("a reason");
            assert_eq!((reason.meaning, reason.fix), (row.meaning, row.fix), "{}: the meaning and the fix are the row's", row.code);
        }
        // The order of the table does not matter: the same rows reversed make the same reasons.
        let mut reversed = CODES.to_vec();
        reversed.reverse();
        let from_reversed: [ReasonInfo; count_with_exit(CODES, EXIT_REFUSED)] = reasons_with_exit(&reversed, EXIT_REFUSED);
        assert_eq!(from_reversed, REASONS);
        // The probe row sits in the table in its place, so the table stays sorted by code.
        assert!(WITH_PROBE.windows(2).all(|pair| pair[0].code < pair[1].code));
        let probe = find_code(&WITH_PROBE, "RUN_INCOMPLETE_PROBE").expect("the probe row");
        assert_eq!(probe.reasons.len(), refusals.len());
        assert_eq!(WITH_PROBE.len(), CODES.len() + 1);
    }

    #[test]
    fn atip_crate12_17_a_reason_made_from_a_code_the_table_does_not_hold_or_in_a_table_of_the_wrong_length_stops_the_evaluation() {
        use atip::{reason_of_code, CodeInfo};
        use std::panic::catch_unwind;

        // A code the table holds makes its reason: the code's name, and the row's own meaning and fix.
        let row = find_code(CODES, "PLAN_STALE").expect("canon registers PLAN_STALE");
        let reason = reason_of_code(CODES, "PLAN_STALE");
        assert_eq!((reason.reason, reason.meaning, reason.fix), (row.code, row.meaning, row.fix));
        // A code it does not hold makes none: the evaluation stops, as a constant of the table would not compile.
        assert!(catch_unwind(|| reason_of_code(CODES, "NOT_A_CODE_OF_CANON")).is_err(), "no reason stands for a code the table lacks");
        assert!(catch_unwind(|| reason_of_code(&[], "PLAN_STALE")).is_err(), "nor in a table that holds nothing");
        // A length other than the number of exit-2 rows stops it, so a table that gained a row without its reasons does not build.
        assert!(catch_unwind(|| reasons_with_exit::<1>(CODES, EXIT_REFUSED)).is_err(), "a reasons list of the wrong length");
        // A probe row whose code the table already holds is not put in twice.
        const ONE_MORE: usize = CODES.len() + 1;
        let twice = CodeInfo { code: "PLAN_STALE", ..PROBE };
        assert!(catch_unwind(|| with_row::<ONE_MORE>(CODES, twice)).is_err(), "a code the table already holds");
    }
}
