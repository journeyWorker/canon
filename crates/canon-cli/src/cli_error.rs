//! The one error contract every `canon` command speaks (0.20 D3).
//!
//! A refusal or an operational failure is a [`CliError`]: a stable
//! SCREAMING_SNAKE `code` (the thing a caller branches on), a `message`, an
//! optional `hint` (the next step), whether a retry can succeed, and the
//! process exit code. [`CliError::emit`] is the single rendering path:
//!
//! - human mode, on stderr: `canon <command path>: <CODE> — <message>`, then
//!   an optional `hint: <hint>` line;
//! - JSON mode, on stdout: `{"error":{"code","message","hint","retryable"}}`
//!   (`hint` is `null` when there is none). The human line is still written
//!   to stderr, so a log reader sees the same failure.
//!
//! Exit codes keep their 0.19 meaning: `0` ok, `1` a gate failure or an
//! operational error ([`EXIT_FAILED`]), `2` a refusal or a usage error
//! ([`EXIT_REFUSED`]), plus the few documented per-command exceptions, which
//! build their error with [`CliError::with_exit`]. The code is the precise
//! contract; the exit code is the coarse one CI reads.
//!
//! [`all_codes`] is the registry of every code and its meaning; a test keeps
//! it in step with the codes the sources construct.

use serde_json::json;

/// A gate failure or an operational error.
pub const EXIT_FAILED: i32 = 1;
/// A refusal or a usage error.
pub const EXIT_REFUSED: i32 = 2;

/// One refusal or operational failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    pub code: &'static str,
    pub message: String,
    pub hint: Option<String>,
    /// Whether running the same command again, unchanged, can succeed.
    pub retryable: bool,
    pub exit: i32,
}

impl CliError {
    /// A refusal: the command was understood and declined (exit 2).
    pub fn refused(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), hint: None, retryable: false, exit: EXIT_REFUSED }
    }

    /// An operational failure: the command could not do its work (exit 1).
    pub fn failed(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), hint: None, retryable: false, exit: EXIT_FAILED }
    }

    /// The next step the caller can take.
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Marks the failure as transient: the same command may succeed later.
    pub fn retryable(mut self) -> Self {
        self.retryable = true;
        self
    }

    /// Overrides the exit code, for a command whose 0.19 contract differs.
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

    /// The `--json` body.
    pub fn to_json(&self) -> serde_json::Value {
        json!({ "error": { "code": self.code, "message": self.message, "hint": self.hint, "retryable": self.retryable } })
    }

    /// Renders the error (see the module docs) and returns the exit code.
    pub fn emit(&self, command: &str, json: bool) -> i32 {
        debug_assert!(is_registered(self.code), "error code {} is not in cli_error::all_codes()", self.code);
        eprintln!("{}", self.human(command));
        if json {
            println!("{}", self.to_json());
        }
        self.exit
    }

    /// [`CliError::emit`] for a failure that also has a report worth reading,
    /// such as the violations a failed gate found. The caller has already
    /// printed that report in its human form, so human mode adds nothing: the
    /// exit code is the whole signal there, as it was before `--json`. In JSON
    /// mode the one document carries `error` (the branch point), `data` (the
    /// detail) and `meta`.
    pub fn emit_with_data(&self, command: &str, json: bool, data: serde_json::Value) -> i32 {
        debug_assert!(is_registered(self.code), "error code {} is not in cli_error::all_codes()", self.code);
        if json {
            let mut body = self.to_json();
            body["data"] = data;
            body["meta"] = json!({ "command": command.strip_prefix("canon ").unwrap_or(command) });
            println!("{body}");
        }
        self.exit
    }
}

fn is_registered(code: &str) -> bool {
    all_codes().iter().any(|(c, _)| *c == code)
}

/// Every error code a command can emit, with its meaning, sorted by code.
/// A code appears once; `tests/cli_error.rs` keeps this in step with the
/// codes the sources construct.
pub fn all_codes() -> &'static [(&'static str, &'static str)] {
    CODES
}

const CODES: &[(&str, &str)] = &[
    ("ADAPTER_EXECUTION_NOT_AUTHORIZED", "the adapter response did not authorize execution"),
    ("ADAPTER_REFUSED", "an adapter response was malformed or refused"),
    ("APPROVAL_INCOMPLETE", "`brief approve` got an empty or multi-line --by or --note"),
    ("BRIEF_CHANGED_SINCE_APPROVAL", "the brief was edited after the human approved it, so it reads as not approved until approved again"),
    ("BRIEF_HAS_BLOCKERS", "the brief has blocker findings, so it cannot be approved yet"),
    ("BRIEF_NOT_FOUND", "the named change has no brief"),
    ("BRIEF_UNAPPROVABLE", "the brief's text cannot take an `Approved:` line (for example an open code fence)"),
    ("BRIEF_UNAPPROVED", "an attestation was refused because the subject's brief has no human approval"),
    ("BRIEF_UNREADABLE", "the brief file could not be read"),
    ("BRIEF_WRITE_FAILED", "the brief file could not be written"),
    ("CHANGE_ADOPTION_INCOMPLETE", "the change was written but its subject link was not; the hint names the repair"),
    ("CHANGE_AMBIGUOUS", "several live changes have a brief; pass --change <slug>"),
    ("CHANGE_EXISTS", "a change (or its archive) already uses that slug"),
    ("CHANGE_INVALID_ARGUMENT", "`change new` could not build a brief from the given arguments"),
    ("CONFIG_MISSING", "canon.yaml does not exist"),
    ("CONFIG_UNREADABLE", "canon.yaml, policy or a setting it carries could not be read or is invalid"),
    ("CONTEXT_GIT_UNAVAILABLE", "a context pack needs a git identity that is not available"),
    ("CONTEXT_INPUT_MISSING", "a selected context input does not exist"),
    ("CONTEXT_MANIFEST_INVALID", "the context input manifest has an unsupported version"),
    ("CONTEXT_OBJECT_CONFLICT", "an immutable context object already holds different bytes"),
    ("CONTEXT_PACK_NOT_FOUND", "no context pack with that id"),
    ("CONTEXT_PACK_TAMPERED", "a context pack is stale or was changed after it was created"),
    ("CONTEXT_SECRET_DETECTED", "a selected context input contains plaintext secret material"),
    ("CONTEXT_UNSAFE_PATH", "a selected path is absolute, leaves the repository, or contains a symlink"),
    ("DASHBOARD_CONFIG_INVALID", "the dashboard's canon.yaml section is present but unusable"),
    ("DASHBOARD_FAILED", "the dashboard could not start (not built, or an I/O failure)"),
    ("DEMO_NOT_INITIALIZED", "`demo attest` ran before `demo init`"),
    ("DISPATCH_ALREADY_ENDED", "the dispatch run was already ended"),
    ("DISPATCH_END_IN_PROGRESS", "another process is closing the run; retry"),
    ("DISPATCH_INVALID_ARGUMENT", "a dispatch flag combination is invalid"),
    ("DISPATCH_MANIFEST_MISMATCH", "the manifest's run id differs from its file name"),
    ("DISPATCH_MANIFEST_UNREADABLE", "the dispatch manifest is not a readable run record"),
    ("DISPATCH_NOT_RUNNING", "the run's recorded status cannot be closed"),
    ("DISPATCH_ROLE_REGIME_MISMATCH", "--role does not match the --regime's role"),
    ("DISPATCH_RUN_NOT_FOUND", "no dispatch manifest for that run id"),
    ("DIVERGENCE_INVALID_ARGUMENT", "a divergence flag value is invalid"),
    ("DIVERGENCE_REFUSED", "a divergence candidate was refused by a gate rule"),
    ("EVIDENCE_APPROVAL_INVALID", "the human approval attached to an attestation is missing, malformed or fails verification"),
    ("EVIDENCE_BATCH_REFUSED", "a batch attestation was refused and none of it was written"),
    ("EVIDENCE_BIND_REFUSED", "a --report or --file binding was refused"),
    ("EVIDENCE_CONTRACT_MISMATCH", "the record's kind and ref differ from the task's typed evidence contract"),
    ("EVIDENCE_FABRICATED", "the note carries a fabrication marker or an unsupported verified claim"),
    ("EVIDENCE_INVALID_ARGUMENT", "an `evidence add` argument is empty, malformed or contradicts another"),
    ("EVIDENCE_REFUSED", "an attestation was refused by an evidence rule"),
    ("EVIDENCE_REPORT_CONTRADICTION", "the bound report contradicts the verdict"),
    ("EVIDENCE_STORE_FAILED", "a bound file could not be stored in the artifact store"),
    ("EVIDENCE_VAULT_FAILED", "the evidence vault could not write a blob"),
    ("FEATURE_EXISTS", "the feature file already exists"),
    ("FEATURE_OUTSIDE_SPEC_ROOTS", "the feature path is under no configured specs.roots entry"),
    ("FINDING_INVALID_ARGUMENT", "a `finding` argument is empty, malformed or out of range"),
    ("FINDING_NOT_FOUND", "no committed finding at that change, round and seq"),
    ("FINDING_RESOLUTION_SHA_REQUIRED", "a fixed finding needs --resolution-sha"),
    ("FINDING_RESOLUTION_SHA_UNEXPECTED", "--resolution-sha was given with a disposition other than fixed"),
    ("FINDING_SEQ_OCCUPIED", "a finding already holds that change, round and seq"),
    ("FINDING_STAGED", "the finding is still staged and cannot be closed until committed"),
    ("FINDING_UNRESOLVABLE_COMMIT", "a commit reference names no commit in this repository"),
    ("FORMAT_VIOLATIONS", "`format` found spec-corpus violations; data.violations names each"),
    ("GATE_TASK_BLOCKED", "`gate task` refused to flip the task: its evidence does not support it"),
    ("GATE_TASK_EVIDENCE_INVALID", "the evidence or typed atom for the task is invalid"),
    ("GATE_TASK_INVALID_ARGUMENT", "the task id is malformed"),
    ("GATE_TASK_UNSUPPORTED", "the plan dialect cannot flip this task"),
    ("GATE_VIOLATIONS", "the evidence gate found violations; data.violations names each"),
    ("HOOKS_INSTALL_FAILED", "a hook entry could not be installed"),
    ("INGEST_FAILED", "an ingest command failed"),
    ("INGEST_SOURCE_UNUSABLE", "an `ingest plans` source was malformed and persisted nothing; check its root"),
    ("INIT_AGENTS_MD_CONFLICT", "AGENTS.md cannot take the canon block without overwriting content"),
    ("INIT_CONFIG_EXISTS", "canon.yaml already exists and there is nothing else to refresh"),
    ("INIT_INVALID_ARGUMENT", "an `init` option is invalid (language or skill providers)"),
    ("INIT_SKILL_INSTALL_FAILED", "canon.yaml was written but the canon skill was not installed"),
    ("INVALID_CURSOR", "`--cursor` is not one this listing issued, or the record it points after is gone"),
    ("INVENTORY_ABORTED", "`inventory sync` aborted a spec root on violations or duplicate scenarios; data.roots names each"),
    ("INVENTORY_FAILED", "the scenario index could not be built"),
    ("LAYOUT_MIGRATION_REQUIRED", "the spec root uses a pre-0.17 layout; run `canon migrate`"),
    ("LEARN_EVALUATION_REJECTED", "the promotion evaluation does not fit the candidate strategy"),
    ("LEARN_INVALID_ARGUMENT", "a `learn` flag value is invalid"),
    ("LEARN_INVALID_INPUT", "a `learn` evaluation or approval file is invalid"),
    ("LEARN_PROMOTION_BLOCKED", "the promotion gate blocks this strategy"),
    ("LEARN_PROMOTION_FAILED", "the strategy could not be promoted"),
    ("LEARN_ROLLBACK_FAILED", "the strategy could not be rolled back"),
    ("LEARN_SIGNATURE_UNREADABLE", "the detached signature file could not be read"),
    ("LEARN_STRATEGY_NOT_FOUND", "no strategy with that id"),
    ("LEARN_TRAJECTORY_NOT_FOUND", "the contradicting trajectory does not exist"),
    ("MIGRATE_INDEX_FAILED", "files moved but the scenario index was not rebuilt; run `canon inventory sync`"),
    ("MIGRATE_REFUSED", "`migrate` refused: the repository's layout or config cannot be moved safely"),
    ("MIGRATE_STOPPED", "`migrate` stopped partway; running it again finishes the rest"),
    ("MIGRATE_UNCOMMITTED", "`migrate` refused because paths it moves have uncommitted changes"),
    ("NO_BRIEF", "no live change has a brief"),
    ("PLAN_CHANGED", "the data changed since `purge --plan`, so the confirmed digest no longer matches; nothing was deleted"),
    ("PLAN_CORPUS_UNREADABLE", "the plan corpus could not be read to resolve a task"),
    ("PLAN_SOURCES_MISSING", "the repository configures no plan sources"),
    ("PLUGIN_SYNC_ERRORS", "`plugin sync` could not write some candidates; data.roots names each error"),
    ("PLUGIN_SYNC_FAILED", "plugin sync failed"),
    ("POLICY_APPROVAL_FAILED", "the policy approval could not be recorded"),
    ("PROMOTE_FAILED", "a staged record was written but promoting it to the ledger failed"),
    ("PROMOTE_REFUSED", "`gate promote` refused one or more staged records; data.refused names each"),
    ("PROMPT_BUNDLE_INVALID", "a prompt bundle is invalid"),
    ("PURGE_PLAN_REQUIRED", "`purge` deletes only a reviewed plan: run `purge --plan`, then `purge --confirm <planDigest>`"),
    ("QUERY_FAILED", "the query could not read the ledger"),
    ("QUERY_INVALID_ARGUMENT", "a query flag is invalid for this kind"),
    ("REGIME_KEY_INVALID", "a regime key part is malformed"),
    ("REPORT_FAILED", "the report could not be built or checked"),
    ("REPORT_STALE", "`report --check` found the report missing or out of date"),
    ("REPO_UNREADABLE", "the repository, its ledger or an input file could not be read"),
    ("RETENTION_EXPORT_FAILED", "the export failed"),
    ("RETENTION_PURGE_FAILED", "the purge failed or stopped partway"),
    ("RETRIEVE_FAILED", "retrieval failed"),
    ("REVIEW_ARTIFACT_REFUSED", "a review's artifact binding was refused"),
    ("REVIEW_INVALID_ARGUMENT", "a `review add` argument is missing or contradicts another"),
    ("REVIEW_SCENARIO_NOT_INDEXED", "a reviewed scenario has no indexed record; run `canon inventory sync`"),
    ("SCAFFOLD_INVALID_ARGUMENT", "a scaffold argument is invalid"),
    ("SCENARIO_TAG_EXISTS", "the scenario tag already exists"),
    ("SELFTEST_FAILED", "a selftest suite or fixture did not match its expected result"),
    ("SERIALIZE_FAILED", "the result could not be serialized"),
    ("SKILLS_DRIFT", "`skills check` found an installed skill missing, stale, a legacy remnant, or written by another canon version"),
    ("SKILLS_FAILED", "a skills command failed"),
    ("SPEC_ROOTS_UNREADABLE", "the scenario files a brief's flow steps point at could not be read"),
    ("SPEC_ROOT_UNRESOLVED", "no single spec root could be chosen; pass --project"),
    ("SUBJECT_ADOPT_REFUSED", "the change could not be adopted by the subject"),
    ("SUBJECT_EXISTS", "a subject with that id already exists"),
    ("SUBJECT_GATE_VIOLATION", "the status transition is blocked by gate violations"),
    ("SUBJECT_INVALID_ARGUMENT", "a `subject` argument is malformed"),
    ("SUBJECT_INVALID_TRANSITION", "the status change is not an allowed transition"),
    ("SUBJECT_NOT_FOUND", "no subject with that id"),
    ("SUBJECT_REASON_REQUIRED", "reopening a shipped subject needs --reason"),
    ("SUBJECT_REASON_UNEXPECTED", "--reason applies only to reopening a shipped subject"),
    ("TASK_NOT_FOUND", "the task id names no task in the plan corpus"),
    ("TASK_ROW_NOT_FOUND", "the task's row is missing from its plan document"),
    ("TIER_FAILED", "tier aging failed"),
    ("TYPED_ATOMS_INVALID", "the typed-atoms file for the change does not parse"),
    ("UNKNOWN_COMMAND", "`canon schema` was given a command path canon does not have"),
    ("UNKNOWN_FIELD", "`--fields` names a field the records do not have"),
    ("USAGE", "the command line did not parse: an unknown flag or subcommand, or a missing or malformed argument"),
    ("WRITE_FAILED", "a file or ledger record could not be written"),
];
