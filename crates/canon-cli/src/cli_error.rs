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
//!   [`ErrorBody`]: `code`, `message`, `retryable`, then `field` (the argument
//!   or JSON pointer refused), `expected` (the valid set when there is one),
//!   `received` (the refused value, secrets redacted), `suggestions` and
//!   `hint`; a member with no value is absent.
//!
//! Exit codes: `0` ok; `1` the command ran and its answer is negative or
//! incomplete: a finding, a failed check, a failure after the work began, an
//! internal error ([`EXIT_FAILED`]); `2` the command refused to run: usage,
//! `CONFIG_MISSING`, an id or file that does not exist, unreadable input, an
//! invalid argument, a target that cannot be written ([`EXIT_REFUSED`]). Every
//! `*_NOT_FOUND`, `*_UNREADABLE` and `*_INVALID_ARGUMENT` code is `2`.
//!
//! [`all_codes`] is the registry of every code with its meaning, exit and
//! retryability; a test keeps it in step with the codes the sources
//! construct.
//!
//! The methods of [`CliErrorExt`] are called as methods of [`CliError`]; a file
//! that builds or renders an error imports the trait (`CliErrorExt as _` when it
//! does not name it).

use serde_json::Value;

pub use atip::{CliError, CodeInfo, ErrorBody, EXIT_FAILED, EXIT_REFUSED};

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
        debug_assert!(is_registered(self.code), "error code {} is not in cli_error::all_codes()", self.code);
        if json {
            crate::json_out::print_failure::<crate::json_out::NoData>(command, self, None);
        } else {
            eprintln!("{}", self.human_text());
        }
        self.exit
    }

    fn emit_with_data<R: Report>(&self, command: &str, json: bool, data: &R) -> u8 {
        self.emit_with_data_and_meta(command, json, data, Value::Null)
    }

    fn emit_with_data_and_meta<R: Report>(&self, command: &str, json: bool, data: &R, extra_meta: Value) -> u8 {
        debug_assert!(is_registered(self.code), "error code {} is not in cli_error::all_codes()", self.code);
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

fn is_registered(code: &str) -> bool {
    code_info(code).is_some()
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

const CODES: &[CodeInfo] = &[
    CodeInfo { code: "ADAPTER_EXECUTION_NOT_AUTHORIZED", meaning: "the adapter response did not authorize execution", exit: 1, retryable: false, fix: "Have the adapter return a response that authorizes execution, or do not run the command through this adapter." },
    CodeInfo { code: "ADAPTER_REFUSED", meaning: "`adapter validate` could not read the response file, or `adapter authorize` refused: its response is unreadable, breaks a rule, or the policy file is broken", exit: 2, retryable: false, fix: "Fix the response file, or the policy file `.canon/policy.yaml` when the message names it, then run the command again; `adapter validate` re-checks a response." },
    CodeInfo { code: "ADAPTER_RESPONSE_INVALID", meaning: "`adapter validate` read the response and it breaks a rule of the envelope: the check's negative answer", exit: 1, retryable: false, fix: "Fix the response envelope the message names, then run `canon adapter validate` again." },
    CodeInfo { code: "APPROVAL_INCOMPLETE", meaning: "`brief approve` got an empty or multi-line --by or --note", exit: 2, retryable: false, fix: "Pass a single-line, non-empty --approver and --note to `canon brief approve`." },
    CodeInfo { code: "ARTIFACTS_MOVE_REFUSED", meaning: "`artifacts move` cannot move the blobs: the target is not an s3 tier, a local blob does not match its name, or the object store holds a bad copy", exit: 2, retryable: false, fix: "Target an s3 tier, restore any local blob whose content does not match its name, and replace the bad object-store copy named in the error." },
    CodeInfo { code: "ARTIFACTS_STORE_FAILED", meaning: "writing or uploading a blob to the artifact store failed after the command started writing; what was already stored stays, and a second run resumes", exit: 1, retryable: true, fix: "Check that the artifact store is writable and reachable and has space, then run the command again." },
    CodeInfo { code: "ARTIFACTS_STORE_UNREACHABLE", meaning: "the artifact store cannot be opened, attached or reached before any blob is written, so no blob can be verified or uploaded", exit: 2, retryable: true, fix: "Check the artifact store tier's endpoint and credentials in canon.yaml, then run the command again." },
    CodeInfo { code: "ARTIFACTS_VERIFY_FAILED", meaning: "a bound file is missing from the artifact store or altered; data.problems names each", exit: 1, retryable: false, fix: "Restore or re-store each file listed in data.problems, then run `canon artifacts verify` again." },
    CodeInfo { code: "BATCH_INCOMPLETE", meaning: "a batch stopped during execution: an element failed after the batch passed its preflight; data.items gives each element's status and the failed one's error, and the elements after it were skipped", exit: 1, retryable: false, fix: "Read data.items to see which elements were written, fix the failed element, then run the batch again; elements already written are reported `unchanged`." },
    CodeInfo { code: "BRIEF_ADOPTION_UNAPPROVED", meaning: "a change was adopted into a building, verifying or shipped subject while its brief is not approved; the subject's existing scenarios keep accepting evidence, but scenarios tagged for the change wait for the approval", exit: 1, retryable: false, fix: "Have the human approve the brief with `canon brief approve`, or take the adoption back with `canon subject unadopt`." },
    CodeInfo { code: "BRIEF_APPROVE_REFUSED", meaning: "`brief approve` was refused because a brief has blocker findings, or, for several ids, because at least one id was refused (a blocker, no brief, a brief that cannot carry the line); data.refused names each with its own code; nothing was written", exit: 2, retryable: false, fix: "Resolve what each refused id names (`canon brief check` lists blocker findings), then ask the human to approve again." },
    CodeInfo { code: "BRIEF_CHANGED_SINCE_APPROVAL", meaning: "the brief was edited after the human approved it, so it reads as not approved until approved again", exit: 1, retryable: false, fix: "Review the edited brief and have the human run `canon brief approve` again." },
    CodeInfo { code: "BRIEF_CHANGED_SINCE_PAGE_LOAD", meaning: "the dashboard was asked to approve a brief whose file differs from the copy the page showed the human; nothing was recorded", exit: 2, retryable: false, fix: "Reload the dashboard page to see the current brief, then approve it again." },
    CodeInfo { code: "BRIEF_DRAFT_PENDING", meaning: "a subject transition ran while a change of the subject is a pending draft (never approved, not a chore, no scenario tagged for it), so the transition does not judge that change", exit: 1, retryable: false, fix: "Tag a scenario for the draft change or have the human approve its brief if it belongs to this transition; otherwise ignore the note." },
    CodeInfo { code: "BRIEF_HAS_BLOCKERS", meaning: "`brief check` found blocker findings; data.findings names each", exit: 1, retryable: false, fix: "Resolve every blocker finding in data.findings, then run `canon brief check` again." },
    CodeInfo { code: "BRIEF_NOT_FOUND", meaning: "the named change has no brief", exit: 2, retryable: false, fix: "Pass the id of a change that has a brief; `canon status` lists the changes." },
    CodeInfo { code: "BRIEF_OVERRIDE_ON_RECORD", meaning: "an evidence record was attested while its brief was not approved, under `--override-reason`; the waiver stays on the record", exit: 1, retryable: false, fix: "Have the human approve the brief with `canon brief approve`, then attest the evidence again." },
    CodeInfo { code: "BRIEF_UNAPPROVABLE", meaning: "the brief's text cannot take an `Approved:` line (for example an open code fence)", exit: 2, retryable: false, fix: "Close the open code fence or other construct in the brief text so an `Approved:` line can be added, then approve again." },
    CodeInfo { code: "BRIEF_UNAPPROVED", meaning: "an attestation was refused because the subject's brief has no human approval", exit: 1, retryable: false, fix: "Have the human approve the subject's brief with `canon brief approve` before attesting." },
    CodeInfo { code: "BRIEF_UNAPPROVED_NOT_ENFORCED", meaning: "a subject moved past a brief that is not approved but cannot be enforced (the change has no brief canon reads, or the `plans:` section is unreadable)", exit: 1, retryable: false, fix: "Give the change a brief with `canon change new`, or fix the `plans:` section of canon.yaml, then have the human approve the brief." },
    CodeInfo { code: "BRIEF_UNREADABLE", meaning: "the brief file could not be read", exit: 2, retryable: false, fix: "Check that the brief file exists and is readable, then run the command again." },
    CodeInfo { code: "BRIEF_WRITE_FAILED", meaning: "the brief file could not be written", exit: 1, retryable: false, fix: "Check that the brief file and its directory are writable, then run the command again." },
    CodeInfo { code: "CHANGE_ADOPTION_INCOMPLETE", meaning: "an adoption or unlink stopped between its two records, after the first was written, so the subject and the change disagree about their link; the hint names the command that finishes it", exit: 1, retryable: false, fix: "Run the repair command named in the hint to finish the link between the change and its subject." },
    CodeInfo { code: "CHANGE_AMBIGUOUS", meaning: "several live changes have a brief; pass the change id as the operand", exit: 2, retryable: false, fix: "Pass the id of the change you mean as the operand." },
    CodeInfo { code: "CHANGE_EXISTS", meaning: "a change (or its archive) already uses that slug", exit: 2, retryable: false, fix: "Choose a different slug for the new change, or work on the existing one." },
    CodeInfo { code: "CHANGE_INVALID_ARGUMENT", meaning: "`change new` could not build a brief from the given arguments", exit: 2, retryable: false, fix: "Correct the arguments to `canon change new` so a brief can be built from them." },
    CodeInfo { code: "COMMAND_REQUIRED", meaning: "the bare tool or a command group was run without a command; `expected` lists the commands of that level", exit: 2, retryable: false, fix: "Run one of the commands `expected` lists; `canon schema` lists every command." },
    CodeInfo { code: "CONFIG_CHECK_FAILED", meaning: "`init --check` ran and a present canon.yaml section fails its loader", exit: 1, retryable: false, fix: "Fix each section `data.sections` marks FAIL, then run `canon init --check` again." },
    CodeInfo { code: "CONFIG_MISSING", meaning: "canon.yaml does not exist", exit: 2, retryable: false, fix: "Run `canon init` or pass --repo <DIR> to a directory holding canon.yaml." },
    CodeInfo { code: "CONFIG_UNREADABLE", meaning: "canon.yaml, policy or a setting it carries could not be read or is invalid", exit: 2, retryable: false, fix: "Fix the canon.yaml, policy or setting named in the error, then run `canon init --check` to confirm." },
    CodeInfo { code: "CONTEXT_GIT_UNAVAILABLE", meaning: "a context pack needs a git identity that is not available", exit: 1, retryable: false, fix: "Run the command inside a git checkout with a resolvable HEAD and git on the PATH." },
    CodeInfo { code: "CONTEXT_INPUT_MISSING", meaning: "a selected context input does not exist", exit: 2, retryable: false, fix: "Create the selected context input or drop it from the selection." },
    CodeInfo { code: "CONTEXT_MANIFEST_INVALID", meaning: "the context input manifest has an unsupported version", exit: 2, retryable: false, fix: "Rewrite the context input manifest with a supported version." },
    CodeInfo { code: "CONTEXT_OBJECT_CONFLICT", meaning: "an immutable context object already holds different bytes", exit: 1, retryable: false, fix: "Do not overwrite the immutable context object; build a new pack from the current inputs." },
    CodeInfo { code: "CONTEXT_PACK_NOT_FOUND", meaning: "no context pack with that well-formed id (a malformed id is USAGE)", exit: 2, retryable: false, fix: "Pass the id of an existing context pack; `canon context list` shows them." },
    CodeInfo { code: "CONTEXT_PACK_TAMPERED", meaning: "a context pack is stale or was changed after it was created", exit: 1, retryable: false, fix: "Discard the stale or altered pack and build a new one." },
    CodeInfo { code: "CONTEXT_SECRET_DETECTED", meaning: "a selected context input contains plaintext secret material", exit: 2, retryable: false, fix: "Remove the plaintext secret from the selected input or deselect it, then build the pack again." },
    CodeInfo { code: "CONTEXT_UNSAFE_PATH", meaning: "a selected path is absolute, leaves the repository, or contains a symlink", exit: 2, retryable: false, fix: "Select only repository-relative paths that stay inside the repository and contain no symlink." },
    CodeInfo { code: "CURSOR_INVALID", meaning: "`--cursor` is not one this listing issued, or the record it points after is gone", exit: 2, retryable: false, fix: "Restart the listing without --cursor and page with the cursor it returns." },
    CodeInfo { code: "DASHBOARD_BAD_REQUEST", meaning: "a dashboard request was not the one the briefs page sends: wrong method, content type, length or body", exit: 2, retryable: false, fix: "Send requests only through the briefs page the dashboard serves." },
    CodeInfo { code: "DASHBOARD_CONFIG_INVALID", meaning: "the dashboard's canon.yaml section is present but unusable", exit: 2, retryable: false, fix: "Fix the dashboard section of canon.yaml, then start the dashboard again." },
    CodeInfo { code: "DASHBOARD_FAILED", meaning: "the dashboard could not start for a reason that is not one of the other DASHBOARD_ codes (an I/O failure)", exit: 1, retryable: false, fix: "Read the I/O error in the message, fix its cause, then start the dashboard again." },
    CodeInfo { code: "DASHBOARD_NOT_BUILT", meaning: "this canon binary embeds no dashboard app and the repo holds no build of canon's own", exit: 1, retryable: false, fix: "Use a canon binary that embeds the dashboard app, or build the dashboard into the repo." },
    CodeInfo { code: "DASHBOARD_ORIGIN_REFUSED", meaning: "a dashboard request came with a Host or Origin that is not the bound address, so it was refused before anything was read", exit: 2, retryable: false, fix: "Open the dashboard at the exact address it is bound to." },
    CodeInfo { code: "DASHBOARD_PORT_IN_USE", meaning: "the dashboard's port is already bound by another process", exit: 1, retryable: true, fix: "Stop the process holding the port or choose another port, then start the dashboard again." },
    CodeInfo { code: "DASHBOARD_SNAPSHOT_UNAVAILABLE", meaning: "the dashboard started without a readable panels snapshot, so its panels page shows the reason instead of panels", exit: 1, retryable: false, fix: "Run `canon report snapshot` for the repo, or pass a readable --snapshot directory, then start the dashboard again." },
    CodeInfo { code: "DEMO_NOT_INITIALIZED", meaning: "`demo review` ran before `demo init`", exit: 2, retryable: false, fix: "Run `canon demo init` before `canon demo review`." },
    CodeInfo { code: "DISPATCH_ALREADY_ENDED", meaning: "the dispatch run was already ended", exit: 2, retryable: false, fix: "Nothing to do: the run is already ended; start a new dispatch run if more work is needed." },
    CodeInfo { code: "DISPATCH_DIFF_RECORDS_UNAVAILABLE", meaning: "`dispatch diff` could not read the task or run records of a tier, so that side of the diff is empty", exit: 1, retryable: false, fix: "Fix the tier routing or store the message names, then run `canon dispatch diff` again." },
    CodeInfo { code: "DISPATCH_END_IN_PROGRESS", meaning: "another process is closing the run; retry", exit: 1, retryable: true, fix: "Wait a moment and run `canon dispatch end` again." },
    CodeInfo { code: "DISPATCH_INVALID_ARGUMENT", meaning: "a dispatch flag combination is invalid", exit: 2, retryable: false, fix: "Remove the conflicting dispatch flags; see `canon schema` for the command's options." },
    CodeInfo { code: "DISPATCH_MANIFEST_MISMATCH", meaning: "the manifest's run id differs from its file name", exit: 2, retryable: false, fix: "Rename the manifest to match its run id, or correct the run id inside it." },
    CodeInfo { code: "DISPATCH_MANIFEST_UNREADABLE", meaning: "the dispatch manifest is not a readable run record", exit: 2, retryable: false, fix: "Restore the dispatch manifest as a readable run record." },
    CodeInfo { code: "DISPATCH_NOT_RUNNING", meaning: "the run's recorded status cannot be closed", exit: 2, retryable: false, fix: "Close only runs whose recorded status is running; inspect the manifest for this run's status." },
    CodeInfo { code: "DISPATCH_ROLE_REGIME_MISMATCH", meaning: "--role does not match the --regime's role", exit: 2, retryable: false, fix: "Pass a --role that matches the --regime's role, or change the --regime." },
    CodeInfo { code: "DISPATCH_RUN_COPIES_DIVERGE", meaning: "a run's tier copy and its dispatch manifest differ; the tier copy is used", exit: 1, retryable: false, fix: "Run `canon dispatch end` or `canon ingest sessions` to bring the two copies back together." },
    CodeInfo { code: "DISPATCH_RUN_NOT_FOUND", meaning: "no dispatch manifest for that run id", exit: 2, retryable: false, fix: "Pass the id of an existing dispatch run." },
    CodeInfo { code: "DISPATCH_TIER_NOT_PERSISTED", meaning: "a dispatch begin or end could not be persisted to the run's routed tier; the manifest file is the only home of the record", exit: 1, retryable: false, fix: "Fix the tier the message names, then run `canon ingest sessions` so the tier holds the run." },
    CodeInfo { code: "DIVERGENCE_ACCEPT_REFUSED", meaning: "a divergence acceptance was refused: not a reviewer, the evidence is not divergent, or it was attested by the accepting actor or session", exit: 2, retryable: false, fix: "Accept only divergent evidence as a reviewer whose actor and session did not attest it; `canon divergence list` shows candidates." },
    CodeInfo { code: "DIVERGENCE_INVALID_ARGUMENT", meaning: "a divergence flag value is invalid", exit: 2, retryable: false, fix: "Correct the divergence flag value; see `canon schema` for the command's options." },
    CodeInfo { code: "DIVERGENCE_REFUSED", meaning: "a divergence candidate was refused by a gate rule", exit: 1, retryable: false, fix: "Resolve the gate rule the error names, then run `canon gate promote --kind divergence` again." },
    CodeInfo { code: "EVIDENCE_APPROVAL_INVALID", meaning: "the human approval attached to an attestation is missing, malformed or fails verification", exit: 2, retryable: false, fix: "Attach a well-formed human approval that verifies against the brief." },
    CodeInfo { code: "EVIDENCE_BATCH_REFUSED", meaning: "a batch attestation was refused by the gate while its records were being promoted; every record and blob the run wrote was removed", exit: 1, retryable: false, fix: "Fix the entry the error names and submit the whole batch again; nothing was kept." },
    CodeInfo { code: "EVIDENCE_BIND_REFUSED", meaning: "a --report or --file binding was refused", exit: 2, retryable: false, fix: "Pass a --report or --file that exists inside the repository and passes the binding rules named in the error." },
    CodeInfo { code: "EVIDENCE_CONTRACT_MISMATCH", meaning: "the record's kind and ref differ from the task's typed evidence contract", exit: 1, retryable: false, fix: "Record evidence whose kind and ref match the task's typed evidence contract." },
    CodeInfo { code: "EVIDENCE_FABRICATED", meaning: "the note carries a fabrication marker or an unsupported verified claim", exit: 1, retryable: false, fix: "Rewrite the note to state only what was actually run, without fabrication markers or unsupported verified claims." },
    CodeInfo { code: "EVIDENCE_INVALID_ARGUMENT", meaning: "an `evidence add` argument is empty, malformed or contradicts another", exit: 2, retryable: false, fix: "Correct the `canon evidence add` arguments so none is empty, malformed or contradictory." },
    CodeInfo { code: "EVIDENCE_REFUSED", meaning: "an attestation was refused by an evidence rule", exit: 2, retryable: false, fix: "Satisfy the evidence rule named in the error, then attest again." },
    CodeInfo { code: "EVIDENCE_REPORT_CONTRADICTION", meaning: "the bound report contradicts the verdict", exit: 1, retryable: false, fix: "Re-run the check so the report supports the verdict, or change the verdict to match the report." },
    CodeInfo { code: "EVIDENCE_SCENARIO_NOT_INDEXED", meaning: "`evidence approval-payload` names a scenario the scenario index does not hold, so there are no bytes to sign", exit: 2, retryable: false, fix: "Run `canon inventory sync` so the scenario is indexed, or pass the id of a scenario the spec root declares." },
    CodeInfo { code: "EVIDENCE_STORE_FAILED", meaning: "a bound file could not be written to the artifact store after the store was opened; the batch or record was rolled back", exit: 1, retryable: true, fix: "Check the artifact store is reachable and writable, then run the command again." },
    CodeInfo { code: "EVIDENCE_STORE_UNAVAILABLE", meaning: "the artifact store could not be used for a bound file before anything was written: the tier is not attached, the `artifact_store:` block is unusable, or another process holds the store lock", exit: 2, retryable: true, fix: "Check the artifact store tier's endpoint and credentials in canon.yaml, or wait for the other canon process, then run the command again." },
    CodeInfo { code: "FEATURE_EXISTS", meaning: "the feature file already exists", exit: 2, retryable: false, fix: "Choose a different feature path or edit the existing file." },
    CodeInfo { code: "FEATURE_OUTSIDE_SPEC_ROOTS", meaning: "the feature path is under no configured specs.roots entry", exit: 2, retryable: false, fix: "Place the feature under a specs.roots entry in canon.yaml, or add a root that covers it." },
    CodeInfo { code: "FINDING_INTRODUCED_REQUIRED", meaning: "a new blocker or should-fix finding names no introducing commit and does not say it is unknown", exit: 2, retryable: false, fix: "Pass --introduced-rev <rev> when the introducing commit was established, or --introduced-unknown when it was not; a note needs neither." },
    CodeInfo { code: "FINDING_INVALID_ARGUMENT", meaning: "a `finding` argument is empty, malformed or out of range", exit: 2, retryable: false, fix: "Correct the `canon finding` argument that is empty, malformed or out of range." },
    CodeInfo { code: "FINDING_NOT_FOUND", meaning: "no committed finding at that change, round and seq", exit: 2, retryable: false, fix: "Pass the change, round and seq of a committed finding; `canon finding list` shows them." },
    CodeInfo { code: "FINDING_REASON_REQUIRED", meaning: "closing a finding as rejected or deferred needs a one-line --reason", exit: 2, retryable: false, fix: "Pass a one-line --reason when closing a finding as rejected or deferred." },
    CodeInfo { code: "FINDING_RESOLUTION_REV_UNEXPECTED", meaning: "--resolution-rev was given with a disposition other than fixed", exit: 2, retryable: false, fix: "Drop --resolution-rev, or close the finding as fixed." },
    CodeInfo { code: "FINDING_REVIEW_NOT_FOUND", meaning: "--review names no review in the ledger or the archive", exit: 2, retryable: false, fix: "Pass the review id that `canon review scenario` or `canon review change` printed; `canon query --kind review` shows the ids." },
    CodeInfo { code: "FINDING_SEQ_OCCUPIED", meaning: "a finding already holds that change, round and seq", exit: 2, retryable: false, fix: "Choose an unused seq for that change and round." },
    CodeInfo { code: "FINDING_STAGED", meaning: "the finding is still staged and cannot be closed until committed", exit: 2, retryable: false, fix: "Commit the staged finding, then close it." },
    CodeInfo { code: "FINDING_UNKNOWN_SCENARIO", meaning: "--scenario names a scenario the index does not know", exit: 2, retryable: false, fix: "Pass a --scenario that the scenario index knows; run `canon inventory sync` if the scenario is new." },
    CodeInfo { code: "FORMAT_VIOLATIONS", meaning: "`format` found spec-corpus violations; data.violations names each", exit: 1, retryable: false, fix: "Fix each violation listed in data.violations, then run `canon format` again." },
    CodeInfo { code: "GATE_TASK_BLOCKED", meaning: "`task done` refused to flip the task: its evidence does not support it", exit: 1, retryable: false, fix: "Attach the evidence the task requires, then run `canon task done` again." },
    CodeInfo { code: "GATE_TASK_EVIDENCE_INVALID", meaning: "the evidence or typed atom for the task is invalid", exit: 1, retryable: false, fix: "Correct the task's evidence or typed atom named in the error, then run `canon task done` again." },
    CodeInfo { code: "GATE_TASK_INVALID_ARGUMENT", meaning: "the task id is malformed", exit: 2, retryable: false, fix: "Pass a well-formed task id to `canon task done`." },
    CodeInfo { code: "GATE_TASK_UNSUPPORTED", meaning: "the plan dialect cannot flip this task", exit: 2, retryable: false, fix: "Mark the task done by hand in the plan document; this plan dialect cannot be flipped by canon." },
    CodeInfo { code: "GATE_VIOLATIONS", meaning: "the evidence gate found violations; data.violations names each", exit: 1, retryable: false, fix: "Fix each violation listed in data.violations, then run the gate again." },
    CodeInfo { code: "HOOKS_INSTALL_FAILED", meaning: "a hook file could not be written after the install began", exit: 1, retryable: false, fix: "Check that the hook file and its directory are writable, then run `canon gate install-hooks` again." },
    CodeInfo { code: "HOOKS_INSTALL_REFUSED", meaning: "a hook install could not start: --repo is not an existing directory, or a hook file is unreadable or conflicting; nothing was written", exit: 2, retryable: false, fix: "Pass --repo naming an existing directory and check that the hook files under .claude and .codex are valid JSON, then run the install again." },
    CodeInfo { code: "HUMAN_ONLY", meaning: "this leaf records a human's decision (`brief approve`, `policy approve`, `divergence accept`, a `migrate --commit` that re-binds `Approved:` lines), so an agent role (planner, implementer, reviewer, orchestrator declared with --as or CANON_ROLE) is refused unless it relays the human's explicit answer with --relayed-via, and a repo whose canon.yaml says `approval: { relay: deny }` refuses that too", exit: 2, retryable: false, fix: "Show the human what they approve (`canon brief show <change> --body` for a brief), ask with the harness question tool, and only on their explicit approval run the call again with --relayed-via <tool>, --actor, --approver and --note (a brief also takes --digest <change>=<shown digest>). Where canon.yaml says `approval: { relay: deny }`, the human runs the call in their own shell (or on the dashboard for a brief). Never remove your role (`env -u CANON_ROLE`, dropping --as) to run it yourself." },
    CodeInfo { code: "INGEST_FAILED", meaning: "an ingest command failed", exit: 1, retryable: false, fix: "Read the error message, fix the named source or input, then run the ingest again." },
    CodeInfo { code: "INGEST_SOURCE_UNUSABLE", meaning: "an `ingest plans` source was malformed and persisted nothing; check its root", exit: 1, retryable: false, fix: "Fix the malformed plan source under its configured root, then run `canon ingest plans` again." },
    CodeInfo { code: "INIT_AGENTS_MD_CONFLICT", meaning: "AGENTS.md cannot take the canon block without overwriting content", exit: 2, retryable: false, fix: "Move or merge the conflicting AGENTS.md content by hand, then run `canon init` again." },
    CodeInfo { code: "INIT_CONFIG_EXISTS", meaning: "canon.yaml already exists and there is nothing else to refresh; `demo init` refuses any directory that holds one", exit: 2, retryable: false, fix: "Nothing to do: canon.yaml already exists; run `canon init --check` to verify it, or `canon demo init` in a directory without one." },
    CodeInfo { code: "INIT_INVALID_ARGUMENT", meaning: "an `init` option is invalid (language or skill providers)", exit: 2, retryable: false, fix: "Pass a supported language and skill providers to `canon init`." },
    CodeInfo { code: "INIT_SKILL_INSTALL_FAILED", meaning: "canon.yaml was written but the canon skill was not installed", exit: 1, retryable: false, fix: "Fix the skill install target the error names, then run `canon skills install`." },
    CodeInfo { code: "INPUT_CONFLICT", meaning: "a flag on the command line and the same key in `--input` disagree", exit: 2, retryable: false, fix: "Give the value once: drop the flag or the key, or make the two equal." },
    CodeInfo { code: "INPUT_INVALID", meaning: "`--input` cannot be read or framed (empty, not JSON or NDJSON, a file or stdin that cannot be read, stdin a terminal, an array on a leaf that takes one object) or the document breaks its fragment schema (an unknown key, a value of the wrong type, null); `field` is `--input` for a fault in the value itself and carries `x-canon.reason` (`unreadable`, `tty`, `empty`, `syntax` or `batch-unsupported`), and is a JSON pointer with no reason for a fault in the document", exit: 2, retryable: false, fix: "Pass one JSON object, a JSON array, or a file or `-` with one object per line, using only the keys `canon schema <command> --json` lists under `inputs`; `canon schema <command> --json` shows an example." },
    CodeInfo { code: "INPUT_TOO_LARGE", meaning: "the `--input` document is over 8 MiB or a batch is over 10000 elements", exit: 2, retryable: false, fix: "Split the batch into several runs, each under the limit." },
    CodeInfo { code: "INTERNAL", meaning: "a bug in canon: a panic or a state canon did not expect; nothing about the command line is at fault", exit: 1, retryable: false, fix: "Report it at https://github.com/journeyWorker/canon/issues with the command line and this error; it cannot be fixed from the command line." },
    CodeInfo { code: "INVENTORY_ABORTED", meaning: "`inventory sync` aborted a spec root on violations or duplicate scenarios; data.roots names each", exit: 1, retryable: false, fix: "Fix the violations or duplicate scenarios listed in data.roots, then run `canon inventory sync` again." },
    CodeInfo { code: "INVENTORY_FAILED", meaning: "the scenario index could not be built: the record store failed while `inventory sync` was reading or writing records", exit: 1, retryable: false, fix: "Fix the store problem the error names, then run `canon inventory sync` again." },
    CodeInfo { code: "INVENTORY_REFUSED", meaning: "`inventory sync` could not start: the `specs:` section or canon.yaml is unusable, or another process holds the index lock; nothing was written", exit: 2, retryable: false, fix: "Fix the `specs:` section or canon.yaml the error names, or wait for the other canon process, then run `canon inventory sync` again." },
    CodeInfo { code: "LAYOUT_MIGRATION_REQUIRED", meaning: "the spec root uses a pre-0.17 layout; run `canon migrate`", exit: 2, retryable: false, fix: "Run `canon migrate` to move the spec root to the current layout." },
    CodeInfo { code: "LEARN_DEMOTION_FAILED", meaning: "promotion is blocked and the demotion a contradicting trajectory triggered also failed; the strategy is still active", exit: 1, retryable: false, fix: "Run `canon strategy rollback` on the still-active strategy, then retry the promotion." },
    CodeInfo { code: "LEARN_EVALUATION_REJECTED", meaning: "the promotion evaluation does not fit the candidate strategy", exit: 1, retryable: false, fix: "Supply an evaluation file that fits the candidate strategy, then run `canon strategy promote` again." },
    CodeInfo { code: "LEARN_INVALID_ARGUMENT", meaning: "a `strategy` flag value is invalid", exit: 2, retryable: false, fix: "Correct the `canon strategy` flag value; see `canon schema` for the command's options." },
    CodeInfo { code: "LEARN_INVALID_INPUT", meaning: "a `strategy` evaluation or approval file is invalid", exit: 2, retryable: false, fix: "Fix the evaluation or approval file so it parses and matches its schema." },
    CodeInfo { code: "LEARN_PROMOTION_BLOCKED", meaning: "the promotion gate blocks this strategy", exit: 1, retryable: false, fix: "Resolve what the promotion gate reports for this strategy, then run `canon strategy promote` again." },
    CodeInfo { code: "LEARN_PROMOTION_FAILED", meaning: "the strategy could not be promoted", exit: 1, retryable: false, fix: "Read the error message, fix its cause, then run `canon strategy promote` again." },
    CodeInfo { code: "LEARN_ROLLBACK_FAILED", meaning: "the strategy exists but could not be rolled back: its signature does not verify, or the rollback could not be recorded", exit: 1, retryable: false, fix: "Read the error message, fix its cause (sign the rollback again if the signature failed), then run `canon strategy rollback` again." },
    CodeInfo { code: "LEARN_SIGNATURE_UNREADABLE", meaning: "the detached signature file could not be read", exit: 2, retryable: false, fix: "Pass the path of a readable detached signature file." },
    CodeInfo { code: "LEARN_STRATEGY_NOT_FOUND", meaning: "no strategy with that id; `strategy rollback` and `strategy approval-payload` look the id up first", exit: 2, retryable: false, fix: "Pass the id of an existing strategy." },
    CodeInfo { code: "LEARN_TRAJECTORY_NOT_FOUND", meaning: "the contradicting trajectory does not exist", exit: 2, retryable: false, fix: "Pass the id of a trajectory that exists, or omit the contradicting trajectory." },
    CodeInfo { code: "LEDGER_ARCHIVE_UNAVAILABLE", meaning: "`ledger compact` found no archive: the ledger root is not a directory named `ledger`", exit: 2, retryable: false, fix: "Point --ledger-root at a directory named `ledger`, or rename the ledger directory so it has an archive." },
    CodeInfo { code: "LEDGER_COMPACT_FAILED", meaning: "a superseded version could not be moved to the archive; what moved stays moved", exit: 1, retryable: false, fix: "Fix the file-system problem the error names, then run `canon ledger compact` again to move the rest." },
    CodeInfo { code: "LEDGER_KIND_ROUTED_AWAY", meaning: "a record kind is routed away from the `local` rung that status reads, so its records are not counted", exit: 1, retryable: false, fix: "Route the kind to the `local` rung in canon.yaml, then run `canon status` again." },
    CodeInfo { code: "LEDGER_NO_SCENARIOS", meaning: "the ledger holds no scenario records, so every subject reads as owning none", exit: 1, retryable: false, fix: "Run `canon inventory sync` to index the scenarios the .feature files hold." },
    CodeInfo { code: "LEDGER_RECORDS_UNREADABLE", meaning: "ledger record files could not be read and are not counted", exit: 2, retryable: false, fix: "Run `canon gate check` to name the unreadable records, then repair or rename them." },
    CodeInfo { code: "LEDGER_ROLLBACK_NOT_RESTORED", meaning: "a batch write was rolled back but the archived versions its records had replaced were not put back", exit: 1, retryable: false, fix: "Copy the archived versions back from the archive beside the ledger." },
    CodeInfo { code: "LEDGER_UNREADABLE", meaning: "the repo's ledger could not be read, so nothing could be counted", exit: 2, retryable: false, fix: "Run `canon init --check` to see what is wrong in canon.yaml, fix it, then run the command again." },
    CodeInfo { code: "LEGACY_GUIDE_STALE", meaning: "the installed canon skill files are the 0.27 guide and name commands later versions removed", exit: 1, retryable: false, fix: "Do not follow the old skill files; run `canon migrate --json`, then read `canon skills show canon`." },
    CodeInfo { code: "LEGACY_STATE_FOUND", meaning: "the repo holds 0.27-era canon state that later versions do not read", exit: 2, retryable: false, fix: "Run `canon migrate --json` to preview the upgrade, then apply it as the preview says." },
    CodeInfo { code: "MIGRATE_INDEX_FAILED", meaning: "files moved but the scenario index was not rebuilt; run `canon inventory sync`", exit: 1, retryable: false, fix: "Run `canon inventory sync` to rebuild the scenario index." },
    CodeInfo { code: "MIGRATE_PINS_FAILED", meaning: "files moved but the reviews were not pinned to what their evidence says; run `canon migrate` again", exit: 1, retryable: false, fix: "Run `canon migrate` again to pin the reviews to their evidence." },
    CodeInfo { code: "MIGRATE_REFUSED", meaning: "`migrate` refused: the repository's layout or config cannot be moved safely", exit: 2, retryable: false, fix: "Fix the layout or config problem the error names, then run `canon migrate` again." },
    CodeInfo { code: "MIGRATE_STOPPED", meaning: "`migrate` stopped partway, after it had begun changing the repo; running it again finishes the rest", exit: 1, retryable: false, fix: "Run `canon migrate` again to finish the rest." },
    CodeInfo { code: "MIGRATE_UNCOMMITTED", meaning: "`migrate` refused because paths it moves have uncommitted changes", exit: 2, retryable: false, fix: "Commit or stash the uncommitted changes under the paths migrate moves, then run `canon migrate` again." },
    CodeInfo { code: "NO_BRIEF", meaning: "no live change has a brief", exit: 2, retryable: false, fix: "Run `canon change new` to create a change with a brief." },
    CodeInfo { code: "NO_COMMIT", meaning: "a command needed a commit and the directory is not a git work tree or has no commits", exit: 2, retryable: false, fix: "Run the command inside a git work tree that has at least one commit." },
    CodeInfo { code: "OLD_BRIEF_FORMAT", meaning: "a command that writes and judges a change brief was refused because that brief is in the format canon 0.30 and earlier wrote (`canon:` comments or shipped Korean headings); nothing was written", exit: 2, retryable: false, fix: "Run `canon migrate --json` to preview the rewrite; the human runs `canon migrate --commit --plan-digest <digest>` when the plan re-binds an `Approved:` line." },
    CodeInfo { code: "OLD_BRIEF_FORMAT_FOUND", meaning: "a read command found briefs in the format canon 0.30 and earlier wrote; the commands that write and judge them are refused until they are rewritten", exit: 2, retryable: false, fix: "Run `canon migrate --json` to preview the rewrite, then apply it as the preview says." },
    CodeInfo { code: "OLD_LAYOUT", meaning: "a command that writes records was refused in a repo whose ledger is laid out by canon 0.29; nothing was written", exit: 2, retryable: false, fix: "Run `canon migrate --json` to preview the move, then `canon migrate --commit --plan-digest <digest>`." },
    CodeInfo { code: "OLD_LAYOUT_FOUND", meaning: "a read command found a ledger laid out by canon 0.29, so the records named by the old rule are not counted and writes are refused", exit: 2, retryable: false, fix: "Run `canon migrate --json` to preview the move, then `canon migrate --commit --plan-digest <digest>`." },
    CodeInfo { code: "OVERRIDE_REASON_UNUSED", meaning: "`--override-reason` was given but nothing was refused, so no override was recorded", exit: 1, retryable: false, fix: "Drop --override-reason; the command needed no override." },
    CodeInfo { code: "OVERRIDE_RECORDED", meaning: "a refusal was waived under `--override-reason` and the waiver is recorded; a gap that appears later is not covered", exit: 1, retryable: false, fix: "Have the human review the recorded waiver and close the gap it names." },
    CodeInfo { code: "OVERRIDE_REFUSAL_WAIVED", meaning: "a refusal was waived under `--override-reason`", exit: 1, retryable: false, fix: "Close the gap the waived refusal names, so the waiver is no longer needed." },
    CodeInfo { code: "PLAN_CORPUS_UNREADABLE", meaning: "the plan corpus could not be read to resolve a task", exit: 2, retryable: false, fix: "Fix the plan sources configured in canon.yaml so every document can be read." },
    CodeInfo { code: "PLAN_SOURCES_MISSING", meaning: "the repository configures no plan sources", exit: 2, retryable: false, fix: "Configure plan sources in canon.yaml, then run the command again." },
    CodeInfo { code: "PLAN_STALE", meaning: "the plan recomputed under `--commit` no longer has the `--plan-digest` the preview printed; nothing was written", exit: 2, retryable: false, fix: "Run the command again without --commit to preview the new plan, then act with `--commit --plan-digest <DIGEST>`." },
    CodeInfo { code: "PLUGIN_SYNC_ERRORS", meaning: "`plugin sync` could not write some candidates; data.roots names each error", exit: 1, retryable: false, fix: "Fix each error listed in data.roots, then run `canon plugin sync` again." },
    CodeInfo { code: "PLUGIN_SYNC_FAILED", meaning: "`plugin sync` could not start: canon.yaml or its `specs:` section is unusable, the plugin is not installed under `.canon/plugins/`, no overlay source is registered for it, or it declares no overlay; nothing was written (a candidate that cannot be written is PLUGIN_SYNC_ERRORS)", exit: 2, retryable: false, fix: "Check that the plugin is installed and declares an overlay, and that canon.yaml parses, then run `canon plugin sync` again." },
    CodeInfo { code: "POLICY_APPROVAL_FAILED", meaning: "the policy approval could not be written: the write of `.canon/policy.approval.json` failed", exit: 1, retryable: false, fix: "Check that .canon is writable, then have the human run `canon policy approve` again." },
    CodeInfo { code: "POLICY_APPROVAL_REFUSED", meaning: "`policy approve` could not start: there is no policy file, the policy cannot be read, or --approver is blank; nothing was written", exit: 2, retryable: false, fix: "Write a policy with `canon init`, make `.canon/policy.yaml` readable, and pass --approver <name>, then have the human run `canon policy approve` again." },
    CodeInfo { code: "POLICY_APPROVAL_UNREADABLE", meaning: "`.canon/policy.approval.json` cannot be read, so the policy in force cannot be checked against its approval; shipping or reopening a subject is refused as a closed gate (exit 1), and `--override-reason` does not waive it", exit: 1, retryable: false, fix: "Restore or repair `.canon/policy.approval.json`, or have the human run `canon policy approve` again." },
    CodeInfo { code: "POLICY_CHANGED_SINCE_APPROVAL", meaning: "`.canon/policy.yaml` differs from the bytes a human approved; shipping or reopening a subject refuses until it is re-approved or restored, and `--override-reason` does not waive it", exit: 1, retryable: false, fix: "Restore `.canon/policy.yaml` to the approved bytes, or have the human run `canon policy approve` again." },
    CodeInfo { code: "POLICY_MISSING", meaning: "`.canon/policy.yaml` does not exist, so the gate requires no evidence, failure cases or review", exit: 1, retryable: false, fix: "Write `.canon/policy.yaml` with a `spec_coverage` section and have the human approve it." },
    CodeInfo { code: "POLICY_NOT_APPROVED", meaning: "`.canon/policy.yaml` has no approval record", exit: 1, retryable: false, fix: "Have a human review the policy and run `canon policy approve --approver <who>`." },
    CodeInfo { code: "POLICY_SPEC_COVERAGE_ABSENT", meaning: "the policy has no `spec_coverage` section, so the gate requires no evidence, failure cases or review", exit: 1, retryable: false, fix: "Add a `spec_coverage` section to `.canon/policy.yaml` and have the human approve it." },
    CodeInfo { code: "POLICY_SPEC_COVERAGE_INVALID", meaning: "the policy's `spec_coverage` section is unusable, so the gate refuses and the review guard cannot read whether review is required", exit: 2, retryable: false, fix: "Fix the `spec_coverage` section of `.canon/policy.yaml`; `canon gate check` reports what is wrong." },
    CodeInfo { code: "POLICY_UNREADABLE", meaning: "`.canon/policy.yaml` cannot be read and falls back to defaults, so the gate requires no evidence, failure cases or review", exit: 2, retryable: false, fix: "Fix `.canon/policy.yaml` so it parses, then have the human approve it." },
    CodeInfo { code: "PROMOTE_FAILED", meaning: "a staged record was written but promoting it to the ledger failed", exit: 1, retryable: false, fix: "Fix the ledger problem the error names, then run `canon gate promote` again." },
    CodeInfo { code: "PROMOTE_REFUSED", meaning: "`gate promote` refused one or more staged records; data.refused names each", exit: 1, retryable: false, fix: "Fix each record listed in data.refused, then run `canon gate promote` again." },
    CodeInfo { code: "PROMPT_BUNDLE_INVALID", meaning: "a prompt bundle is invalid", exit: 2, retryable: false, fix: "Fix the prompt bundle so it parses and matches its schema." },
    CodeInfo { code: "QUERY_FAILED", meaning: "the query could not read the ledger", exit: 1, retryable: false, fix: "Check that the ledger is readable, then run the query again." },
    CodeInfo { code: "QUERY_INVALID_ARGUMENT", meaning: "a query flag is invalid for this kind", exit: 2, retryable: false, fix: "Remove the flag that does not apply to this kind; see `canon schema` for its options." },
    CodeInfo { code: "QUERY_PLUGIN_DIAGNOSTIC", meaning: "a `query --plugin` projection reported a diagnostic; the records are listed without that projection where it failed", exit: 1, retryable: false, fix: "Read the diagnostic in the message and fix the plugin or the record it names." },
    CodeInfo { code: "REFERENCES_UNCHECKED", meaning: "the sources that records refer to could not be read, so references to missing sources were not checked", exit: 1, retryable: false, fix: "Make the sources readable (the message names the cause), then run the command again." },
    CodeInfo { code: "REGIME_KEY_INVALID", meaning: "a regime key part is malformed", exit: 2, retryable: false, fix: "Pass a well-formed regime key; every part must be non-empty and valid." },
    CodeInfo { code: "RELAY_DIGEST_MISMATCH", meaning: "a relayed brief approval names a digest that is not the brief's digest now, so the human was shown a different brief than the one on disk; nothing was recorded", exit: 2, retryable: false, fix: "Run `canon brief show <change> --body` again, put the brief and its digest in front of the human, ask again, and relay the digest of what they saw." },
    CodeInfo { code: "REPORT_FAILED", meaning: "the report could not be built or checked", exit: 1, retryable: false, fix: "Read the error message, fix its cause, then run `canon report build` or `canon report check` again." },
    CodeInfo { code: "REPORT_STALE", meaning: "`report check` found the report missing or out of date", exit: 1, retryable: false, fix: "Run `canon report build` or `canon report snapshot` to refresh the report." },
    CodeInfo { code: "REPORT_TIER_NOT_READ_DIRECTLY", meaning: "a report was built from a ledger where some record kinds are not read directly from their routed tier", exit: 1, retryable: false, fix: "Route the kinds the message names to a rung the report reads, then run the report again." },
    CodeInfo { code: "REPO_UNREADABLE", meaning: "the repository, its ledger or an input file could not be read", exit: 2, retryable: false, fix: "Check that --repo points at a readable repository and that the named input file exists." },
    CodeInfo { code: "RETENTION_EXPORT_FAILED", meaning: "the export failed once it started writing: the output file or the access audit line could not be written", exit: 1, retryable: false, fix: "Check that the export destination is writable and has space, then run the export again." },
    CodeInfo { code: "RETENTION_EXPORT_REFUSED", meaning: "the export could not start: the ledger or its tiers could not be read, `--include-sensitive` was not allowed; nothing was written", exit: 2, retryable: false, fix: "Check that the ledger is readable and the tiers resolve, then run the export again." },
    CodeInfo { code: "RETENTION_PURGE_FAILED", meaning: "the purge stopped after it began deleting: what was deleted stays deleted (`data` names the counts)", exit: 1, retryable: false, fix: "Fix the file-system problem the error names, then preview with `canon purge` and act with `--commit --plan-digest <DIGEST>`." },
    CodeInfo { code: "RETENTION_PURGE_REFUSED", meaning: "the purge could not start, or it was only a preview: the tiers could not be opened or read, nothing was deleted", exit: 2, retryable: false, fix: "Fix the tier or ledger problem the error names, then preview with `canon purge` and act with `--commit --plan-digest <DIGEST>`." },
    CodeInfo { code: "RETRIEVE_FAILED", meaning: "`retrieve` could not start: the domain/subject cannot be turned into a regime key (the scope flags that conflict, are missing or disagree with --role are `USAGE`); a store outage is not an error here, it serves empty guidance (exit 0)", exit: 2, retryable: false, fix: "Check --domain and --subject (`canon regime show` prints a valid key) and that the repo directory has a usable name, then run the retrieval again." },
    CodeInfo { code: "RETRIEVE_REGIME_FELL_BACK", meaning: "`retrieve` served guidance from a regime derived by fallback instead of the one asked for", exit: 1, retryable: false, fix: "Pass the regime that holds the guidance you want with --regime, or record guidance under the requested regime." },
    CodeInfo { code: "REVIEW_ARTIFACT_REFUSED", meaning: "a review's artifact binding was refused", exit: 2, retryable: false, fix: "Bind an artifact that exists inside the repository and passes the rules named in the error." },
    CodeInfo { code: "REVIEW_INVALID_ARGUMENT", meaning: "a `review scenario` or `review change` argument is missing or contradicts another", exit: 2, retryable: false, fix: "Add the missing `canon review` argument or drop the one that contradicts another." },
    CodeInfo { code: "REVIEW_RECORDS_UNMIGRATED", meaning: "scenarios have a review that predates review pinning and does not count until it is rewritten", exit: 1, retryable: false, fix: "Run `canon migrate` to rewrite the old reviews to what the evidence says." },
    CodeInfo { code: "REVIEW_SCENARIO_NOT_INDEXED", meaning: "a reviewed scenario has no indexed record; run `canon inventory sync`", exit: 2, retryable: false, fix: "Run `canon inventory sync`, then review the scenario again." },
    CodeInfo { code: "ROLE_UNKNOWN_RECORDED", meaning: "`--as` or `--role` names a role that is not a known role or one the policy names; the command runs and records the value as given", exit: 1, retryable: false, fix: "Pass planner, implementer, reviewer, orchestrator or a role the policy names." },
    CodeInfo { code: "SCAFFOLD_INVALID_ARGUMENT", meaning: "a scaffold argument is invalid", exit: 2, retryable: false, fix: "Correct the scaffold argument named in the error." },
    CodeInfo { code: "SCENARIO_INDEX_CHECK_FAILED", meaning: "whether the scenario index is stale could not be checked", exit: 1, retryable: false, fix: "Run `canon inventory sync` and read the error it names." },
    CodeInfo { code: "SCENARIO_INDEX_NOT_REFRESHED", meaning: "the scenario index is stale and could not be refreshed, so graded answers may describe specs the files no longer say", exit: 1, retryable: false, fix: "Run `canon inventory sync`; it names each format violation, duplicate id or lock problem to fix." },
    CodeInfo { code: "SCENARIO_INDEX_REFRESHED", meaning: "a read command refreshed a stale scenario index from changed .feature files and reports what it wrote", exit: 1, retryable: false, fix: "Run `canon inventory sync` to list any tag diagnostics the refresh found." },
    CodeInfo { code: "SCENARIO_SUBJECT_UNKNOWN", meaning: "scenarios are tagged `@subject:<id>` but no subject with that id exists", exit: 1, retryable: false, fix: "Create the subject with `canon subject new`, or correct the `@subject` tag." },
    CodeInfo { code: "SCENARIO_TAG_EXISTS", meaning: "the scenario tag already exists", exit: 2, retryable: false, fix: "Choose a tag that is not already used, or edit the existing scenario." },
    CodeInfo { code: "SEARCH_GITIGNORE_MISSING", meaning: "the repo's .gitignore lacks the line of the search index directory", exit: 1, retryable: false, fix: "Run `canon init` or `canon migrate` to add the line to .gitignore." },
    CodeInfo { code: "SEARCH_INDEX_FAILED", meaning: "`search` or `trace` could not read the records the index is built from, or could not build or read the index", exit: 1, retryable: false, fix: "Fix or restore what the message names (an unreadable store, a record file), then run the command again; the index is derived, so deleting `.canon/index` is always safe." },
    CodeInfo { code: "SEARCH_INDEX_IN_MEMORY", meaning: "the search index file could not be written or deleted, so the index was built in memory for this run", exit: 1, retryable: false, fix: "Make `.canon/index` writable, then run the command again." },
    CodeInfo { code: "SEARCH_INDEX_REBUILT", meaning: "the search index file was not a usable database, so it was deleted and rebuilt", exit: 1, retryable: false, fix: "Nothing to do; if it recurs, check the disk and permissions of `.canon/index`." },
    CodeInfo { code: "SEARCH_QUERY_EMPTY", meaning: "`search` was given an empty text or one of only spaces", exit: 2, retryable: false, fix: "Pass the text to find, e.g. `canon search timeout`." },
    CodeInfo { code: "SELFTEST_FAILED", meaning: "a selftest suite or fixture did not match its expected result", exit: 1, retryable: false, fix: "Read the failing suite or fixture in the output and fix the behavior it checks." },
    CodeInfo { code: "SERIALIZE_FAILED", meaning: "the result could not be serialized", exit: 1, retryable: false, fix: "Report this canon bug with the command you ran; it cannot be fixed from the command line." },
    CodeInfo { code: "SKILLS_DRIFT", meaning: "`skills check` found an installed skill missing, stale, a legacy remnant, an orphan the bundle no longer ships, or written by another canon version", exit: 1, retryable: false, fix: "Run `canon skills install` to refresh the installed skills." },
    CodeInfo { code: "SKILLS_FAILED", meaning: "a skills command failed", exit: 1, retryable: false, fix: "Read the error message, fix its cause, then run the skills command again." },
    CodeInfo { code: "SKILLS_INVALID_ARGUMENT", meaning: "a skill provider is unknown, the `skills:` section of canon.yaml is malformed, or the `--skills-source` directory does not exist", exit: 2, retryable: false, fix: "Name providers from claude, codex, omp, pi (or none), and keep the canon.yaml `skills:` section a mapping whose `providers` is a list of those names." },
    CodeInfo { code: "SPEC_ROOTS_UNREADABLE", meaning: "the scenario files a brief's flow steps point at could not be read", exit: 2, retryable: false, fix: "Make the scenario files the brief's flow steps point at readable, or correct the paths." },
    CodeInfo { code: "SPEC_ROOT_UNRESOLVED", meaning: "no single spec root could be chosen; pass --project", exit: 2, retryable: false, fix: "Pass --project with the id of the spec root to use." },
    CodeInfo { code: "STRATEGY_DEMOTED", meaning: "later contradictory evidence demoted an active strategy while its promotion was blocked", exit: 1, retryable: false, fix: "Review the contradicting trajectory before promoting the strategy again." },
    CodeInfo { code: "SUBJECT_ADOPT_REFUSED", meaning: "the change could not be adopted by the subject", exit: 2, retryable: false, fix: "Fix the problem the error names, then run `canon subject adopt` again." },
    CodeInfo { code: "SUBJECT_EXISTS", meaning: "a subject with that id already exists", exit: 2, retryable: false, fix: "Choose a different subject id, or use the existing subject (`canon subject show`)." },
    CodeInfo { code: "SUBJECT_GATE_VIOLATION", meaning: "the status transition is blocked by gate violations", exit: 1, retryable: false, fix: "Fix each gate violation reported, then run `canon subject move` again." },
    CodeInfo { code: "SUBJECT_INVALID_ARGUMENT", meaning: "a `subject` argument is malformed", exit: 2, retryable: false, fix: "Correct the malformed `canon subject` argument." },
    CodeInfo { code: "SUBJECT_INVALID_TRANSITION", meaning: "the status change is not an allowed transition", exit: 2, retryable: false, fix: "Check the subject's status with `canon subject show` and request an allowed transition." },
    CodeInfo { code: "SUBJECT_NOT_FOUND", meaning: "no subject with that id", exit: 2, retryable: false, fix: "Pass the id of an existing subject; `canon status` lists the subjects." },
    CodeInfo { code: "SUBJECT_OVERRIDE_ON_RECORD", meaning: "a subject stands in its status under a recorded `--override-reason` waiver of gate refusals", exit: 1, retryable: false, fix: "Close the gaps the waiver names, then move the subject again without an override." },
    CodeInfo { code: "SUBJECT_REASON_REQUIRED", meaning: "reopening a shipped subject needs --reason", exit: 2, retryable: false, fix: "Pass --reason when reopening a shipped subject with `canon subject move`." },
    CodeInfo { code: "SUBJECT_REOPENED", meaning: "a shipped subject was reopened for a follow-up change; shipping again runs every guard", exit: 1, retryable: false, fix: "Keep the follow-up in its change, then scenario, evidence and review, before shipping again." },
    CodeInfo { code: "SUBJECT_UNADOPT_REFUSED", meaning: "the change carries an approved brief, or a scenario the subject owns is tagged `@flow:<change>.*`, so it stays adopted", exit: 2, retryable: false, fix: "Leave the change adopted, or remove its approved brief and the `@flow:<change>.*` scenario tags first." },
    CodeInfo { code: "TASK_NOT_FOUND", meaning: "the task id names no task in the plan corpus", exit: 2, retryable: false, fix: "Pass a task id that exists in the plan corpus." },
    CodeInfo { code: "TASK_PROOF_IS_CANON_COMMAND", meaning: "a task was flipped on evidence that binds no file and cites a canon command, which reports on canon's own records and proves nothing", exit: 1, retryable: false, fix: "Bind the artifact or report that proves the task with `canon evidence add --artifact <path>`." },
    CodeInfo { code: "TASK_REIMPORT_INCOMPLETE", meaning: "a task was flipped but its change did not re-import cleanly, so the task status record may disagree with the plan", exit: 1, retryable: false, fix: "Run `canon ingest plans` to see why the change did not import." },
    CodeInfo { code: "TASK_ROW_NOT_FOUND", meaning: "the task's row is missing from its plan document", exit: 2, retryable: false, fix: "Restore the task's row in its plan document, then run `canon task done` again." },
    CodeInfo { code: "TASK_STATUS_NOT_REFRESHED", meaning: "a task was flipped but its status record was not refreshed, so `canon query --kind task` may not match the plan", exit: 1, retryable: false, fix: "Run `canon ingest plans` so the task records match the plan." },
    CodeInfo { code: "TIER_FAILED", meaning: "tier aging failed", exit: 1, retryable: false, fix: "Read the error message, fix its cause, then run tier aging again." },
    CodeInfo { code: "TRACE_NOT_FOUND", meaning: "no record says the id `trace` was given, or the sha prefix fits more than one commit", exit: 2, retryable: false, fix: "Pass the key of a finding (`canon query --kind finding`), a review id a review printed, or enough characters of a commit sha to name one commit." },
    CodeInfo { code: "TRACE_OPERAND_INVALID", meaning: "`trace` was given something that is none of a review id, a commit sha or a finding key (a placeholder node's id is not one)", exit: 2, retryable: false, fix: "Pass a review id (26 characters), a commit sha (7 to 40 hex digits) or a finding key (`<change>_<round>_<seq>`)." },
    CodeInfo { code: "TYPED_ATOMS_INVALID", meaning: "the typed-atoms file for the change does not parse", exit: 2, retryable: false, fix: "Fix the change's typed-atoms file so it parses." },
    CodeInfo { code: "UNKNOWN_COMMAND", meaning: "a command word is not in canon's command tree, or `canon schema` was given a path canon does not have; `expected` lists the valid words at that level", exit: 2, retryable: false, fix: "Run one of the commands `expected` lists; `canon schema` lists every command path." },
    CodeInfo { code: "UNKNOWN_ERROR_CODE", meaning: "`canon schema --error <CODE>` was given a code canon does not have", exit: 2, retryable: false, fix: "Run `canon schema --errors` to list every code." },
    CodeInfo { code: "UNKNOWN_ROLE", meaning: "`CANON_ROLE` holds a value that is not a role (planner, implementer, reviewer, orchestrator)", exit: 2, retryable: false, fix: "Set CANON_ROLE to planner, implementer, reviewer or orchestrator, or unset it." },
    CodeInfo { code: "UNKNOWN_ROLE_IGNORED", meaning: "`CANON_ROLE` holds a value that is not a role and was ignored, because the command (`schema`, `context`, `skills show`) does not depend on the role", exit: 2, retryable: false, fix: "Set CANON_ROLE to planner, implementer, reviewer or orchestrator, or unset it." },
    CodeInfo { code: "UNKNOWN_SPEC_ROOT", meaning: "`--project` names no spec root configured in canon.yaml, so nothing was written", exit: 2, retryable: false, fix: "Pass a --project that names a spec root configured in canon.yaml." },
    CodeInfo { code: "UNVERIFIABLE", meaning: "a check examined nothing, so it cannot report clean: data.unverifiable is true and meta.coverage says what was looked at", exit: 1, retryable: false, fix: "Widen the check so it examines something; meta.coverage says what was looked at." },
    CodeInfo { code: "USAGE", meaning: "the command line did not parse or breaks a constraint: an unknown option, a missing or malformed value, an unexpected operand, a `--fields` name that cannot be selected, `--input` given twice or without a value, `--input -` beside another option that reads stdin, a retrieval scoped by neither or both of `--regime` and `--domain`; `field` names the argument", exit: 2, retryable: false, fix: "Run the command with --help and correct the argument `field` names; `expected` lists the valid values when there is a closed set." },
    CodeInfo { code: "WRITE_FAILED", meaning: "a write began and failed: a file or ledger record could not be written after the command started changing things; what an earlier step wrote stays, and the message says which", exit: 1, retryable: false, fix: "Check that the target path is writable and has space, then run the command again." },
    CodeInfo { code: "WRITE_UNAVAILABLE", meaning: "a target the command writes cannot be written, known before anything was changed: it is read-only, its directory is missing or not a directory; nothing was written", exit: 2, retryable: false, fix: "Make the target writable (its permissions, a mounted volume, a missing parent), then run the command again." },
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
