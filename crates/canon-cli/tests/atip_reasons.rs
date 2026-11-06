//! `canon-0-36-code-reasons` (scenarios `atip.reasons.01-09`): a reason belongs to one code, is registered under it, and never changes the exit or
//! the `retryable` value of its code (atip/1 spec 7.6, 15.3; the contract author's 1.3 rules; issue #135 section 5).
//!
//! The audit table (`AUDIT`) is the audit of the brief: for every code that has reasons, each situation the sources raise under it, the reason that
//! names it and the next action of the caller. The tests fail on a code whose audit says it has several next actions and no reasons, on two
//! situations with one next action under different reasons, on a reason the sources never raise and on a reason the audit does not know.
//!
//! Tests are named by scenario id (`atip_reasons_01_…` proves `atip.reasons.01`).

#[path = "support/hermetic.rs"]
mod hermetic;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use canon_cli::cli_error::{all_codes, code_info};
use serde_json::Value;
use tempfile::TempDir;

// ── The binary and the fixtures ──

fn canon(dir: &Path, args: &[&str]) -> Output {
    hermetic::canon().args(args).arg("--json").current_dir(dir).env_remove("CANON_ROLE").env_remove("CANON_ACTOR").output().expect("spawn canon")
}

fn body(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("stdout is one JSON document ({e}): {}\nstderr: {}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}

fn ok(dir: &Path, args: &[&str]) -> Value {
    let out = canon(dir, args);
    assert!(out.status.success(), "{args:?}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    body(&out)
}

fn git(dir: &Path, args: &[&str]) {
    assert!(Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@example.com"]).args(args).current_dir(dir).status().unwrap().success(), "git {args:?}");
}

/// A repository with a subject, a change with its (placeholder) brief, committed.
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    ok(repo, &["init", "--providers", "none"]);
    ok(repo, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(repo, &["change", "new", "add-login", "--subject", "demo", "--title", "Add login"]);
    git(repo, &["init", "-q"]);
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "fixture"]);
    dir
}

/// The refusal `out` is: exit 2, the code, the reason (`None` for no reason) and the exit and `retryable` of the code's registry row.
fn assert_refusal(out: &Output, code: &str, reason: Option<&str>) -> Value {
    let document = body(out);
    let row = code_info(code).unwrap_or_else(|| panic!("{code} is not registered"));
    assert_eq!(out.status.code(), Some(i32::from(row.exit)), "{document}");
    assert_eq!(document["error"]["code"], code, "{document}");
    assert_eq!(document["error"].get("reason").and_then(Value::as_str), reason, "{document}");
    assert_eq!(document["error"]["retryable"], row.retryable, "{document}");
    if let Some(reason) = reason {
        assert!(row.reasons.iter().any(|r| r.reason == reason), "{code} does not register {reason}: {document}");
    }
    document
}

/// `canon schema --error <CODE> --json`: the `reasons` of the registry's view of a code.
fn schema_reasons(code: &str) -> Vec<Value> {
    let out = hermetic::canon().args(["schema", "--error", code, "--json"]).output().expect("spawn canon");
    assert!(out.status.success(), "canon schema --error {code}: {}", String::from_utf8_lossy(&out.stdout));
    body(&out)["data"]["error"]["reasons"].as_array().cloned().unwrap_or_default()
}

// ── The audit ──

/// One situation a code is raised for: `(code, reason, next action)`. An empty reason is a situation with no reason. The next action is what the
/// caller does, in one phrase: two situations of one code with the same next action have the same reason, and two with a different next action have
/// each their own.
type Row = (&'static str, &'static str, &'static str);

const AUDIT: &[Row] = &[
    ("ARTIFACTS_MOVE_INCOMPLETE", "ARTIFACTS_BLOB_CORRUPT", "find what rewrote the blob, then preview and commit a new plan"),
    ("ARTIFACTS_MOVE_INCOMPLETE", "ARTIFACTS_STORE_DENIED", "make the blob readable, then preview and commit a new plan"),
    ("ARTIFACTS_MOVE_INCOMPLETE", "ARTIFACTS_STORE_INVALID", "replace the thing that is no blob, then preview and commit a new plan"),
    ("ARTIFACTS_MOVE_INCOMPLETE", "ARTIFACTS_STORE_REQUIRED", "set the tier's endpoint, credentials and bucket, then preview and commit a new plan"),
    ("ARTIFACTS_MOVE_INCOMPLETE", "ARTIFACTS_STORE_UNAVAILABLE", "wait for the object store, then preview and commit a new plan"),
    ("ARTIFACTS_PRUNE_INCOMPLETE", "ARTIFACTS_STORE_FAILED", "make the object store accept the delete, then preview and commit a new plan"),
    ("ARTIFACTS_PRUNE_INCOMPLETE", "WRITE_FAILED", "make the blob directory writable, then preview and commit a new plan"),
    ("ARTIFACTS_STORE_INCOMPLETE", "ARTIFACTS_STORE_FAILED", "find why the write stopped, then write again"),
    ("ARTIFACTS_STORE_INCOMPLETE", "ARTIFACTS_STORE_UNAVAILABLE", "wait for the store, then write again"),
    ("BRIEF_CHECK_FAILED", "BLOCKERS", "resolve every blocker finding of the check"),
    ("BRIEF_INVALID", "BLOCKERS", "resolve every blocker finding, then ask the human again"),
    ("BRIEF_INVALID", "UNAPPROVABLE", "close the construct that cannot take an Approved: line"),
    ("BRIEF_INVALID", "UNREADABLE", "put the brief.md, a UTF-8 file, where the message says"),
    ("CHANGE_ADOPTION_INCOMPLETE", "WRITE_FAILED", "finish the write the message names, then adopt again"),
    ("CONFIG_INVALID", "ARTIFACT_STORE", "fix the artifact_store: section"),
    ("CONFIG_INVALID", "CANON_YAML", "fix canon.yaml so it parses"),
    ("CONFIG_INVALID", "DASHBOARD", "fix the dashboard section"),
    ("CONFIG_INVALID", "POLICY", "fix the policy file"),
    ("CONFIG_INVALID", "SKILLS", "fix the skills: section"),
    ("CONFIG_INVALID", "SPECS", "fix the specs: section"),
    ("CONFIG_INVALID", "TIERS", "fix the tiers, routing and aging sections"),
    ("CONTEXT_INPUT_INVALID", "SECRET", "remove the plaintext secret or deselect the input"),
    ("CURSOR_INVALID", "ARGUMENTS_CHANGED", "start again without the cursor"),
    ("CURSOR_INVALID", "EXPIRED", "start again without the cursor, sooner"),
    ("CURSOR_INVALID", "FOREIGN", "use the cursor of this command"),
    ("DISPATCH_CONFLICT", "ALREADY_ENDED", "nothing: the run is closed"),
    ("DISPATCH_CONFLICT", "NOT_RUNNING", "close only a run that is running"),
    ("DIVERGENCE_CONFLICT", "NOT_DIVERGENT", "accept only a divergent scenario"),
    ("DIVERGENCE_DENIED", "NOT_REVIEWER", "act as the reviewer"),
    ("DIVERGENCE_DENIED", "SELF_ATTESTED", "have an independent reviewer accept"),
    ("DIVERGENCE_INCOMPLETE", "PROMOTION_INVALID", "fix the staged record, then run gate promote"),
    ("EVIDENCE_APPROVAL_INVALID", "MALFORMED", "give the approval arguments in their form"),
    ("EVIDENCE_APPROVAL_INVALID", "SIGNATURE_MISMATCH", "sign the payload with a listed key"),
    ("EVIDENCE_INCOMPLETE", "PROMOTION_INVALID", "fix the staged record, then run gate promote"),
    ("FINDING_INCOMPLETE", "PROMOTION_INVALID", "fix the staged record, then run gate promote"),
    ("HOOKS_INSTALL_INCOMPLETE", "WRITE_FAILED", "finish the write the message names, then install the hooks again"),
    ("INIT_INCOMPLETE", "CONFIG_INVALID", "fix the skills: section, then run skills install"),
    ("INIT_INCOMPLETE", "SKILLS_ARGUMENT_INVALID", "name a known provider, then run skills install"),
    ("INIT_INCOMPLETE", "SKILLS_SOURCE_NOT_FOUND", "name the skill source directory, then run skills install"),
    ("INPUT_INVALID", "BATCH_UNSUPPORTED", "pass one object and run once per item"),
    ("INPUT_INVALID", "EMPTY", "pass a document"),
    ("INPUT_INVALID", "FLAG_ONLY_KEY", "drop the key and pass the flag"),
    ("INPUT_INVALID", "INVALID_VALUE", "give the key a value of its type"),
    ("INPUT_INVALID", "NOT_JSON", "fix the JSON"),
    ("INPUT_INVALID", "NOT_OBJECT", "pass an object or an array of objects"),
    ("INPUT_INVALID", "STDIN_TTY", "pipe the document in"),
    ("INPUT_INVALID", "UNKNOWN_KEY", "use only the keys of the command"),
    ("INPUT_INVALID", "UNREADABLE", "give a readable UTF-8 file"),
    ("INPUT_TOO_LARGE", "TOO_MANY_BYTES", "shrink the document"),
    ("INPUT_TOO_LARGE", "TOO_MANY_ITEMS", "split the batch"),
    ("INVENTORY_INVALID", "DUPLICATE_SCENARIOS", "remove the duplicate scenario, then sync again"),
    ("INVENTORY_INVALID", "VIOLATIONS", "fix the violations of the root, then sync again"),
    ("INVENTORY_INCOMPLETE", "INVENTORY_FAILED", "find why the store stopped, then sync again"),
    ("INVENTORY_INCOMPLETE", "INVENTORY_INVALID", "fix the violations of the root, then sync again"),
    ("LEDGER_COMPACT_INCOMPLETE", "REPO_DENIED", "make the ledger writable, then compact again"),
    ("LEDGER_COMPACT_INCOMPLETE", "REPO_INVALID", "fix the file the message names, then compact again"),
    ("LEDGER_COMPACT_INCOMPLETE", "REPO_UNAVAILABLE", "wait for the disk, then compact again"),
    ("LEDGER_COMPACT_INCOMPLETE", "WRITE_FAILED", "find why the move stopped, then compact again"),
    ("MIGRATION_INCOMPLETE", "INDEX", "run inventory sync and fix what it reports"),
    ("MIGRATION_INCOMPLETE", "PINS", "fix what the message names, then run migrate again"),
    ("MIGRATION_INCOMPLETE", "STOPPED", "fix what the message names, then run migrate again to finish"),
    ("PLUGIN_SYNC_INCOMPLETE", "WRITE_FAILED", "finish the write the message names, then sync again"),
    ("REPORT_CHECK_FAILED", "STALE", "build the report again"),
    ("RETENTION_PURGE_INCOMPLETE", "WRITE_FAILED", "find why the purge stopped, then purge again"),
    ("SKILLS_CHECK_FAILED", "DRIFT", "install the skills again"),
    ("STRATEGY_INCOMPLETE", "STRATEGY_PROMOTION_INVALID", "record the evidence the gate asks for"),
    ("TIER_INCOMPLETE", "WRITE_FAILED", "find why the move stopped, then run the tier move again"),
    ("SUBJECT_CONFLICT", "APPROVED_BRIEF", "leave the change adopted"),
    ("SUBJECT_CONFLICT", "FLOW_TAGGED", "retag the scenarios, then unadopt"),
    ("SUBJECT_CONFLICT", "TRANSITION", "ask for an allowed transition"),
    ("SUBJECT_INVALID", "FALSE_SUCCESS", "fix the behavior the audit refuted and record a newer audit"),
    ("SUBJECT_INVALID", "MALFORMED_EVIDENCE", "fix or remove the unreadable record"),
    ("SUBJECT_INVALID", "OPEN_BLOCKER", "close the blocker finding"),
];

/// The codes whose situations all have one next action, so they have no reasons, though they are said where a reader could expect some: the codes
/// the brief's table of F2 names without a reason (`REPORT_CHECK_FAILED` has `STALE` for the report that is missing or out of date, with one next
/// action), and the gate's refusals of a record, whose one situation is the refusal.
const SINGLE_SITUATION: &[(&str, &str)] = &[
    ("PROMOTION_INVALID", "fix each record error.details names, then run gate promote again"),
    ("SUBJECT_APPROVAL_REQUIRED", "get what error.details names: an approval, a review or evidence"),
    ("EVIDENCE_STALE", "record the evidence again"),
    ("LAYOUT_MIGRATION_REQUIRED", "run canon migrate"),
    ("ARTIFACTS_STORE_UNAVAILABLE", "wait for the other process or the object store"),
    ("ARTIFACTS_STORE_REQUIRED", "set the endpoint and the credentials"),
    ("PROMOTION_FAILED", "fix the cause the message names, then run gate promote"),
];

/// The gate's 11 violation kinds (`canon_gate::FailureClass`) with the code and the reason a refused transition gives each, and the next action.
const GATE_AUDIT: &[(&str, &str, &str, &str)] = &[
    ("uncovered-cell", "SUBJECT_APPROVAL_REQUIRED", "", "get what error.details names: an approval, a review or evidence"),
    ("unreviewed-promotion", "SUBJECT_APPROVAL_REQUIRED", "", "get what error.details names: an approval, a review or evidence"),
    ("unevidenced-flip", "SUBJECT_APPROVAL_REQUIRED", "", "get what error.details names: an approval, a review or evidence"),
    ("stale-evidence", "EVIDENCE_STALE", "", "record the evidence again"),
    ("old-layout", "LAYOUT_MIGRATION_REQUIRED", "", "run canon migrate"),
    ("malformed-evidence", "SUBJECT_INVALID", "MALFORMED_EVIDENCE", "fix or remove the unreadable record"),
    ("open-blocker", "SUBJECT_INVALID", "OPEN_BLOCKER", "close the blocker finding"),
    ("false-success", "SUBJECT_INVALID", "FALSE_SUCCESS", "fix the behavior the audit refuted and record a newer audit"),
    // Reported by `gate check` as data, raised by no transition: no reason until one raises them.
    ("trust-below-required", "SUBJECT_INVALID", "", "raise the trust of the evidence"),
    ("flagged", "SUBJECT_INVALID", "", "ask the human to clear the flag"),
    ("fabricated-evidence", "SUBJECT_INVALID", "", "record the evidence again with its real result"),
];

fn audit_by_code() -> BTreeMap<&'static str, Vec<(&'static str, &'static str)>> {
    let mut by_code: BTreeMap<&'static str, Vec<(&'static str, &'static str)>> = BTreeMap::new();
    for (code, reason, action) in AUDIT {
        by_code.entry(code).or_default().push((reason, action));
    }
    by_code
}

fn is_upper_words(name: &str) -> bool {
    !name.is_empty() && name.split('_').all(|word| !word.is_empty() && word.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())) && name.as_bytes()[0].is_ascii_uppercase()
}

// ── F1: the registration ──

/// atip.reasons.01: a reason is `{reason, meaning, fix}` registered under its code, in upper-case words, unique inside the code and in the order of
/// the names, with a `fix` of its own; a reason that its code does not register is refused (a debug build asserts it on every answer).
#[test]
fn atip_reasons_01_a_reason_is_registered_under_its_code_with_a_name_a_meaning_and_a_fix_of_its_own() {
    let mut reasoned = 0;
    for info in all_codes() {
        let names: Vec<&str> = info.reasons.iter().map(|r| r.reason).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted, "{}: the reasons are listed in the order of their names", info.code);
        let mut fixes = BTreeSet::new();
        let mut seen = BTreeSet::new();
        for r in info.reasons {
            assert!(is_upper_words(r.reason), "{}: `{}` is not upper-case words and underscores", info.code, r.reason);
            assert!(seen.insert(r.reason), "{}: reason `{}` is registered twice", info.code, r.reason);
            assert!(!r.meaning.trim().is_empty() && !r.fix.trim().is_empty(), "{}: `{}` has no meaning or no fix", info.code, r.reason);
            assert!(fixes.insert(r.fix), "{}: reasons share the fix `{}`: a fix that is the same is one situation", info.code, r.fix);
        }
        reasoned += usize::from(!info.reasons.is_empty());
    }
    assert!(reasoned >= 23, "{reasoned} codes have reasons");
}

/// atip.reasons.01 (failure path): an answer whose reason its code does not register fails the test that runs it (the debug assertion in `emit`).
#[cfg(debug_assertions)]
#[test]
fn atip_reasons_01_a_reason_its_code_does_not_register_is_refused_when_the_error_is_answered() {
    use canon_cli::cli_error::{CliError, CliErrorExt as _};
    let unregistered = std::panic::catch_unwind(|| CliError::refused("CONFIG_INVALID", "x").reason("NOT_A_REASON").emit("canon status", true));
    assert!(unregistered.is_err(), "a reason that CONFIG_INVALID does not register was answered");
    // A reason of another code is no reason of this one: the namespace is the code's.
    let foreign = std::panic::catch_unwind(|| CliError::refused("CONFIG_INVALID", "x").reason("BLOCKERS").emit("canon status", true));
    assert!(foreign.is_err(), "BLOCKERS is a reason of BRIEF_INVALID, not of CONFIG_INVALID");
    let registered = std::panic::catch_unwind(|| CliError::refused("CONFIG_INVALID", "x").reason("CANON_YAML").emit("canon status", false));
    assert!(registered.is_ok(), "a registered reason is answered");
}

fn source_files(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            source_files(&path, found);
        } else if path.extension().is_some_and(|e| e == "rs") {
            found.push(path);
        }
    }
}

/// The text of every non-test source of the workspace's crates that raise errors (`canon-cli`), without the registry and the generated table.
fn raising_sources() -> Vec<(String, String)> {
    let mut files = Vec::new();
    source_files(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    files
        .into_iter()
        .filter(|path| !path.ends_with("cli_error.rs") && !path.ends_with("schema_codes.rs"))
        .map(|path| {
            let text = fs::read_to_string(&path).unwrap();
            let live = text.split("#[cfg(test)]").next().unwrap().to_string();
            (path.display().to_string(), live)
        })
        .collect()
}

/// The names that stand as a reason in `.reason("NAME")` and in a `Some("NAME")` of the tuples that carry a code and its reason.
fn raised_reason_literals() -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for (_, text) in raising_sources() {
        for marker in [".reason(\"", "Some(\""] {
            for piece in text.split(marker).skip(1) {
                let name: String = piece.chars().take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_').collect();
                // A `Some("NAME")` that is the name of a code (`FileCodes { not_found: Some("BRIEF_NOT_FOUND") }`) is a code, not a reason.
                let is_code = marker == "Some(\"" && code_info(&name).is_some();
                if piece[name.len()..].starts_with('"') && is_upper_words(&name) && !is_code {
                    names.insert(name);
                }
            }
        }
    }
    names
}

/// atip.reasons.02: every reason the sources raise by name is registered under some code, and (the failure path of F2) every reason the registry holds
/// is raised by a site: a reason no site raises is refused.
#[test]
fn atip_reasons_02_every_reason_a_source_raises_is_registered_and_every_registered_reason_is_raised() {
    let registered: BTreeSet<&str> = all_codes().iter().flat_map(|info| info.reasons.iter().map(|r| r.reason)).collect();
    let raised = raised_reason_literals();
    let unregistered: Vec<&String> = raised.iter().filter(|name| !registered.contains(name.as_str())).collect();
    assert!(unregistered.is_empty(), "raised as a reason and registered under no code: {unregistered:?}");

    // A reason is raised where its code is: some source file names both `"CODE"` and `"REASON"`. A reason that is also the name of a code (the
    // `_INCOMPLETE` codes say the code of the refusal a later step met: `WRITE_FAILED`, `REPO_DENIED`, `PROMOTION_INVALID`, ...) must be named there
    // other than as the code itself being built (`refused("REPO_DENIED", …)`, `Cause::new("PROMOTION_INVALID", …)`, `code: "…"`), so a registered
    // reason whose site was deleted is reported even when its name is still a live code in the same file. The cursor's own reasons come from the
    // `atip` crate.
    let sources = raising_sources();
    let external: &[&str] = &["ARGUMENTS_CHANGED", "EXPIRED", "FOREIGN"];
    const CODE_BUILDERS: [&str; 6] = ["refused(\"", "failed(\"", "refuse(\"", "Cause::new(\"", "code: \"", "code_info(\""];
    let names_as_reason = |text: &str, name: &str| -> bool {
        let quoted = format!("\"{name}\"");
        let is_code = code_info(name).is_some();
        text.match_indices(&quoted).any(|(at, _)| !is_code || !CODE_BUILDERS.iter().any(|builder| text[..at + 1].ends_with(builder)))
    };
    // The one reason raised from a variable: `promote_after_stage` gives the `_INCOMPLETE` code of the staged record the code of the gate's refusal
    // as its reason (`.reason(refusal.error.code)`), which is `PROMOTION_INVALID` for the three subjects below.
    let dynamic = |code: &str, name: &str| {
        ["DIVERGENCE_INCOMPLETE", "EVIDENCE_INCOMPLETE", "FINDING_INCOMPLETE"].contains(&code)
            && name == "PROMOTION_INVALID"
            && sources.iter().any(|(file, text)| file.ends_with("gate.rs") && text.contains(".reason(refusal.error.code)"))
    };
    let unraised: Vec<String> = all_codes()
        .iter()
        .flat_map(|info| info.reasons.iter().map(move |r| (info.code, r.reason)))
        .filter(|(_, name)| !external.contains(name))
        .filter(|(code, name)| !dynamic(code, name) && !sources.iter().any(|(_, text)| text.contains(&format!("\"{code}\"")) && names_as_reason(text, name)))
        .map(|(code, name)| format!("{code}/{name}"))
        .collect();
    assert!(unraised.is_empty(), "registered and raised by no site (no source file names both the code and the reason): {unraised:?}");
    // The check can fail: a reason no source names, and a code-named reason that sources name only as the code being built, are reported.
    let phantom = [("CONFIG_INVALID", "NO_SUCH_REASON_ANYWHERE")];
    assert!(phantom.iter().all(|(code, name)| !sources.iter().any(|(_, text)| text.contains(&format!("\"{code}\"")) && names_as_reason(text, name))));
    assert!(!names_as_reason("CliError::refused(\"REPO_DENIED\", message)", "REPO_DENIED"), "building the code is not raising it as a reason");
    assert!(names_as_reason("(\"ARTIFACTS_X\", Some(\"REPO_DENIED\"))", "REPO_DENIED"));
}

/// atip.reasons.03: a name may stand under several codes, because the namespace is the code's; the names the contract resolved by namespace stay.
#[test]
fn atip_reasons_03_the_same_name_may_stand_under_two_codes_and_the_names_the_contract_resolved_stay() {
    let owners = |name: &str| -> BTreeSet<&str> { all_codes().iter().filter(|info| info.reasons.iter().any(|r| r.reason == name)).map(|info| info.code).collect() };
    assert_eq!(owners("BLOCKERS"), BTreeSet::from(["BRIEF_CHECK_FAILED", "BRIEF_INVALID"]));
    assert_eq!(owners("UNREADABLE"), BTreeSet::from(["BRIEF_INVALID", "INPUT_INVALID"]));
    assert_eq!(owners("PROMOTION_INVALID"), BTreeSet::from(["DIVERGENCE_INCOMPLETE", "EVIDENCE_INCOMPLETE", "FINDING_INCOMPLETE"]));
    // Contract 15.3: `EXPIRED`, `STALE` and `UNREADABLE` keep their names (issue #135 section 5 asked for new ones; the namespace made it unnecessary).
    assert!(owners("EXPIRED").contains("CURSOR_INVALID"));
    assert!(owners("STALE").contains("REPORT_CHECK_FAILED"));
    assert!(owners("UNREADABLE").contains("INPUT_INVALID"));
    // The same name under two codes is two registrations, each with its own fix: `canon schema --error` of each says its own.
    let fix_of = |code: &str, name: &str| schema_reasons(code).into_iter().find(|r| r["reason"] == name).map(|r| r["fix"].as_str().unwrap().to_string());
    assert_ne!(fix_of("BRIEF_CHECK_FAILED", "BLOCKERS"), fix_of("BRIEF_INVALID", "BLOCKERS"));
    assert!(fix_of("BRIEF_CHECK_FAILED", "BLOCKERS").is_some() && fix_of("BRIEF_INVALID", "BLOCKERS").is_some());
}

// ── F2: the audit table ──

/// atip.reasons.04: the audit says what the registry holds, and the registry says what the audit holds: every code with reasons has its situations in
/// the audit, with the reasons the registry lists; and `canon schema --error <CODE> --json` lists them, with a `fix` for each.
#[test]
fn atip_reasons_04_the_audit_table_and_the_registry_list_the_same_reasons_for_every_code_and_the_schema_says_them() {
    let audit = audit_by_code();
    let registry: BTreeMap<&str, BTreeSet<&str>> = all_codes().iter().filter(|info| !info.reasons.is_empty()).map(|info| (info.code, info.reasons.iter().map(|r| r.reason).collect())).collect();
    let audited: BTreeMap<&str, BTreeSet<&str>> = audit.iter().map(|(code, rows)| (*code, rows.iter().map(|(reason, _)| *reason).collect())).collect();
    assert_eq!(registry, audited, "the registry and the audit table list different reasons");
    for (code, reasons) in &registry {
        let listed = schema_reasons(code);
        assert_eq!(listed.iter().map(|r| r["reason"].as_str().unwrap()).collect::<BTreeSet<_>>(), *reasons, "canon schema --error {code}");
        let fixes: BTreeSet<&str> = listed.iter().map(|r| r["fix"].as_str().unwrap()).collect();
        assert_eq!(fixes.len(), listed.len(), "{code}: a fix for each reason, and each its own");
        assert!(listed.iter().all(|r| r["meaning"].as_str().is_some_and(|m| !m.is_empty())), "{code}: a meaning for each reason");
    }
}

/// atip.reasons.04 (failure path): the audit fails on a code whose situations have several next actions and no reasons, on two situations of one
/// code with one next action and two reasons, and on a situation whose next action is the same as another's with no reason of its own.
#[test]
fn atip_reasons_04_the_audit_refuses_a_code_with_several_next_actions_and_no_reasons_and_one_action_under_two_reasons() {
    fn problems(rows: &[(&str, &str, &str)]) -> Vec<String> {
        let mut by_code: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
        for (code, reason, action) in rows {
            by_code.entry(code).or_default().push((reason, action));
        }
        let mut found = Vec::new();
        for (code, situations) in by_code {
            let actions: BTreeSet<&str> = situations.iter().map(|(_, action)| *action).collect();
            if actions.len() > 1 && situations.iter().any(|(reason, _)| reason.is_empty()) {
                found.push(format!("{code}: several next actions and a situation with no reason"));
            }
            for action in &actions {
                let reasons: BTreeSet<&str> = situations.iter().filter(|(_, a)| a == action).map(|(reason, _)| *reason).collect();
                if reasons.len() > 1 {
                    found.push(format!("{code}: one next action `{action}` under reasons {reasons:?}"));
                }
            }
            let reasons: BTreeSet<&str> = situations.iter().map(|(reason, _)| *reason).filter(|reason| !reason.is_empty()).collect();
            if reasons.len() > actions.len() {
                found.push(format!("{code}: more reasons than next actions"));
            }
        }
        found
    }
    assert_eq!(problems(AUDIT), Vec::<String>::new(), "the audit table explains every reason");
    // The same rows with one defect each.
    assert_eq!(problems(&[("X_INVALID", "", "a"), ("X_INVALID", "", "b")]).len(), 1, "two next actions and no reasons");
    assert_eq!(problems(&[("X_INVALID", "A", "a"), ("X_INVALID", "", "b")]).len(), 1);
    assert_eq!(problems(&[("X_INVALID", "A", "same"), ("X_INVALID", "B", "same")]).len(), 2, "one next action cannot have two names");
}

/// atip.reasons.04: the audit of the gate's violation kinds is the code and the reason `canon_cli::gate_refusal` gives each, and a kind is a reason
/// only where the next action differs inside one `*_INVALID`.
#[test]
fn atip_reasons_04_each_gate_violation_kind_has_the_code_and_the_reason_the_audit_says() {
    use canon_cli::gate_refusal::{subject_code, subject_reason};
    assert_eq!(canon_gate::FailureClass::ALL.len(), GATE_AUDIT.len(), "every kind of the gate is audited");
    for class in canon_gate::FailureClass::ALL {
        let (_, code, reason, _) = GATE_AUDIT.iter().find(|(name, ..)| *name == class.as_str()).unwrap_or_else(|| panic!("{} is not in the audit", class.as_str()));
        assert_eq!(subject_code(class), *code, "{}", class.as_str());
        assert_eq!(subject_reason(class).unwrap_or(""), *reason, "{}", class.as_str());
        let row = code_info(code).unwrap();
        assert_eq!(row.exit, 2, "{code}: a refusal of a gate is exit 2");
        if !reason.is_empty() {
            assert!(row.reasons.iter().any(|r| r.reason == *reason), "{code} registers {reason}");
        }
    }
    // Inside one code: the kinds that share a next action share a reason (none), and the kinds with a reason each have their own next action.
    let mut by_code: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
    for (_, code, reason, action) in GATE_AUDIT {
        by_code.entry(code).or_default().push((reason, action));
    }
    let invalid = &by_code["SUBJECT_INVALID"];
    let named: Vec<&str> = invalid.iter().filter(|(reason, _)| !reason.is_empty()).map(|(_, action)| *action).collect();
    assert_eq!(named.iter().collect::<BTreeSet<_>>().len(), named.len(), "the kinds that are reasons ask for different fixes");
    // The registry holds exactly the reasons of the kinds a transition raises, and none for a kind that is not a gate kind.
    let registered: BTreeSet<&str> = code_info("SUBJECT_INVALID").unwrap().reasons.iter().map(|r| r.reason).collect();
    let raised: BTreeSet<&str> = GATE_AUDIT.iter().filter(|(_, code, reason, _)| *code == "SUBJECT_INVALID" && !reason.is_empty()).map(|(_, _, reason, _)| *reason).collect();
    assert_eq!(registered, raised);
    let kinds: BTreeSet<String> = canon_gate::FailureClass::ALL.iter().map(|class| class.as_str().to_uppercase().replace('-', "_")).collect();
    assert!(registered.iter().all(|reason| kinds.contains(*reason)), "a reason of SUBJECT_INVALID that is no kind of the gate: {registered:?}");
    // The situations of the single-situation codes are in no reason list.
    for (code, _) in SINGLE_SITUATION {
        assert!(code_info(code).is_some_and(|row| row.reasons.is_empty()), "{code} has one situation and no reasons");
    }
}

// ── F3: which file ──

fn replace_section(yaml: &str, key: &str, replacement: &str) -> String {
    let mut out = Vec::new();
    let mut skipping = false;
    for line in yaml.lines() {
        if line.starts_with(&format!("{key}:")) {
            skipping = true;
            out.extend(replacement.lines().map(str::to_string));
            continue;
        }
        if skipping && (line.starts_with(' ') || line.starts_with('#') || line.trim().is_empty()) {
            continue;
        }
        skipping = false;
        out.push(line.to_string());
    }
    out.join("\n") + "\n"
}

/// atip.reasons.05: a wrong `specs:` section, a wrong policy and a wrong canon.yaml are `CONFIG_INVALID` with three different reasons, each the one
/// of its file; the same file with the same fault gives the same reason every time, and every one is exit 2 and not retryable.
#[test]
fn atip_reasons_05_a_wrong_specs_section_a_wrong_policy_and_a_wrong_canon_yaml_are_config_invalid_with_three_reasons() {
    let dir = repo();
    let d = dir.path();
    let original = fs::read_to_string(d.join("canon.yaml")).unwrap();
    let adapter = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/adapter/omp.json");

    // canon.yaml itself: it does not parse.
    fs::write(d.join("canon.yaml"), format!("{original}\nbroken: [unclosed\n")).unwrap();
    let first = assert_refusal(&canon(d, &["gate", "check"]), "CONFIG_INVALID", Some("CANON_YAML"));
    let again = assert_refusal(&canon(d, &["gate", "check"]), "CONFIG_INVALID", Some("CANON_YAML"));
    assert_eq!(first["error"]["message"], again["error"]["message"], "the same file, the same fault, the same answer");
    assert_refusal(&canon(d, &["divergence", "list"]), "CONFIG_INVALID", Some("CANON_YAML"));

    // The `specs:` section: it is not a mapping of roots.
    fs::write(d.join("canon.yaml"), replace_section(&original, "specs", "specs: nope")).unwrap();
    assert_refusal(&canon(d, &["inventory", "sync"]), "CONFIG_INVALID", Some("SPECS"));
    assert_refusal(&canon(d, &["inventory", "sync"]), "CONFIG_INVALID", Some("SPECS"));

    // The `skills:` section.
    fs::write(d.join("canon.yaml"), replace_section(&original, "skills", "skills: nope")).unwrap();
    let skills = canon(d, &["skills", "check"]);
    assert_refusal(&skills, "CONFIG_INVALID", Some("SKILLS"));

    // The `artifact_store:` section.
    fs::write(d.join("canon.yaml"), format!("{original}artifact_store: {{ backend: bogus }}\n")).unwrap();
    assert_refusal(&canon(d, &["artifacts", "verify"]), "CONFIG_INVALID", Some("ARTIFACT_STORE"));

    // The policy: `adapter_capabilities` is not a mapping.
    fs::write(d.join("canon.yaml"), &original).unwrap();
    let policy = d.join(".canon/policy.yaml");
    let good = fs::read_to_string(&policy).unwrap();
    fs::write(&policy, "adapter_capabilities: [unclosed\n").unwrap();
    assert_refusal(&canon(d, &["adapter", "authorize", "--response", adapter.to_str().unwrap()]), "CONFIG_INVALID", Some("POLICY"));
    fs::write(&policy, good).unwrap();

    // Each file's fault has its own name, and a dry run of `init` keeps the `skills:` reason it would have given as `INIT_INCOMPLETE`.
    let names: BTreeSet<&str> = ["CANON_YAML", "SPECS", "SKILLS", "ARTIFACT_STORE", "POLICY"].into_iter().collect();
    assert_eq!(names.len(), 5);
    let registered: BTreeSet<&str> = code_info("CONFIG_INVALID").unwrap().reasons.iter().map(|r| r.reason).collect();
    assert!(names.is_subset(&registered), "{registered:?}");
}

/// atip.reasons.06: a brief.md that cannot be read as a brief is `BRIEF_INVALID` with the reason `UNREADABLE` (a directory stands where the file
/// belongs, or it is not UTF-8), a brief with blocker findings is `BRIEF_INVALID` with `BLOCKERS`, and the three are different next actions.
#[test]
fn atip_reasons_06_a_brief_that_cannot_be_read_and_a_brief_with_blockers_are_brief_invalid_with_their_own_reasons() {
    let dir = repo();
    let d = dir.path();
    let brief = d.join(".canon/changes/add-login/brief.md");

    // Blockers: the placeholder brief cannot be approved yet.
    assert_refusal(&canon(d, &["brief", "approve", "add-login", "--approver", "human", "--note", "n"]), "BRIEF_INVALID", Some("BLOCKERS"));

    // Not UTF-8 text.
    let original = fs::read(&brief).unwrap();
    fs::write(&brief, b"\xff\xfe not utf-8").unwrap();
    for args in [&["brief", "show", "add-login", "--body"][..], &["brief", "approve", "add-login", "--approver", "human", "--note", "n"]] {
        assert_refusal(&canon(d, args), "BRIEF_INVALID", Some("UNREADABLE"));
    }

    // A directory where the file belongs.
    fs::remove_file(&brief).unwrap();
    fs::create_dir(&brief).unwrap();
    for args in [&["brief", "show", "add-login", "--body"][..], &["brief", "check", "add-login"]] {
        assert_refusal(&canon(d, args), "BRIEF_INVALID", Some("UNREADABLE"));
    }
    fs::remove_dir(&brief).unwrap();
    fs::write(&brief, original).unwrap();
}

// ── F4: the gate ──

/// atip.reasons.07: a transition the gate refuses answers `SUBJECT_INVALID` with the reason of the kind that asks for its own fix: `OPEN_BLOCKER`
/// for an open blocker finding, and, when a ship is refused for several causes (an open blocker and a refuted audit), every entry of `error.details`
/// carries its own code and its own registered reason (`OPEN_BLOCKER`, `FALSE_SUCCESS`), which the debug assertion of `emit` also checks.
#[test]
fn atip_reasons_07_a_refused_transition_names_the_kind_that_asks_for_a_different_fix_and_every_detail_has_its_own_code() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let case = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/cases/review-gate-open-blocker/repo");
    copy_tree(&case, d);
    git(d, &["init", "-q", "-b", "main"]);
    git(d, &["add", "-A"]);
    git(d, &["commit", "-q", "-m", "fixture"]);
    let out = canon(d, &["subject", "move", "cart", "--to", "verifying"]);
    let document = assert_refusal(&out, "SUBJECT_INVALID", Some("OPEN_BLOCKER"));
    assert!(document.get("data").is_none(), "a refusal carries no data: {document}");
    assert!(document["error"]["message"].as_str().unwrap().contains("open-blocker"), "{document}");

    // The blocker is fixed and the scenario evidenced: the subject reaches verifying.
    ok(d, &["gate", "promote"]);
    ok(d, &["evidence", "add", "--scenario", "cart.add.01", "--project", "root", "--class", "test-run", "--ref", "cargo test cart_add", "--role", "implementer", "--actor", "impl-agent", "--promote"]);
    ok(d, &["subject", "move", "cart", "--to", "verifying"]);

    // Several causes: an evaluator refutes the scenario and a reviewer raises a new blocker, so the ship is refused for both.
    fs::create_dir_all(d.join("reports")).unwrap();
    fs::write(d.join("reports/c.xml"), "<testsuite><testcase name=\"cart.add.01: x\"><failure/></testcase></testsuite>").unwrap();
    let digest = "0f1e2d3c4b5a69788796a5b4c3d2e1f00f1e2d3c4b5a69788796a5b4c3d2e1f0";
    ok(
        d,
        &["evidence", "audit", "cart.add.01", "--project", "root", "--ref", "testjack run 42", "--role", "auditor", "--actor", "testjack", "--evaluator", "testjack", "--evaluator-version", "1.4.0", "--evaluator-digest", digest, "--audit-result", "refuted", "--report", "junit:reports/c.xml", "--promote"],
    );
    ok(d, &["finding", "add", "--change", "c-cart", "--round", "2", "--seq", "1", "--severity", "blocker", "--introduced-unknown", "--actor", "rev", "--summary", "blocks again", "--promote"]);
    let out = canon(d, &["subject", "move", "cart", "--to", "shipped"]);
    let document = assert_refusal(&out, "SUBJECT_INVALID", Some("FALSE_SUCCESS"));
    let details = document["error"]["details"].as_array().unwrap_or_else(|| panic!("several causes are details: {document}"));
    let reasons: BTreeSet<&str> = details.iter().filter_map(|detail| detail["reason"].as_str()).collect();
    assert_eq!(reasons, BTreeSet::from(["FALSE_SUCCESS", "OPEN_BLOCKER"]), "{document}");
    for detail in details {
        let code = detail["code"].as_str().unwrap();
        let row = code_info(code).unwrap_or_else(|| panic!("{code} is not registered"));
        assert_eq!((detail["retryable"].as_bool().unwrap_or(false), out.status.code()), (row.retryable, Some(i32::from(row.exit))), "{detail}");
        let reason = detail["reason"].as_str().unwrap();
        assert!(row.reasons.iter().any(|r| r.reason == reason), "{code} does not register {reason}: {detail}");
    }
}

/// atip.reasons.07 (failure path): the debug assertion of `emit` looks at every entry of `error.details`, not only at the error: an entry with a
/// reason its code does not register, or with another exit or `retryable` than its row, fails the answer.
#[cfg(debug_assertions)]
#[test]
fn atip_reasons_07_the_emit_assertion_checks_every_detail_entry() {
    use canon_cli::cli_error::{CliError, CliErrorExt as _};
    let good = CliError::refused("SUBJECT_INVALID", "a").reason("OPEN_BLOCKER");
    let unregistered = CliError::refused("SUBJECT_INVALID", "b").reason("NOT_A_REASON");
    let foreign = CliError::refused("SUBJECT_INVALID", "c").reason("BLOCKERS");
    for bad in [unregistered, foreign] {
        let all = vec![good.clone(), bad];
        let top = good.clone().details(all);
        let answered = std::panic::catch_unwind(|| top.emit("canon status", false));
        assert!(answered.is_err(), "a detail entry with a reason its code does not register was answered");
    }
    let fine = good.clone().details(vec![good.clone(), CliError::refused("SUBJECT_INVALID", "d").reason("FALSE_SUCCESS")]);
    assert!(std::panic::catch_unwind(|| fine.emit("canon status", false)).is_ok(), "registered reasons in details are answered");
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

// ── F5: a reason never changes the exit or the retryable value ──

/// atip.reasons.08: a reason does not change the exit or `retryable` of its code. The guard is the debug assertion of `emit` (it compares both with
/// the registry row for the error and every detail entry; `exit` and `retryable` are public fields, so a site could set them): every error built for
/// a code and each of its reasons is answered as built, and the same error with the other exit, or the other `retryable`, is refused. A different
/// `retryable` is a different code (`ARTIFACTS_STORE_UNAVAILABLE` / `_REQUIRED`) and so is a different exit (`PROMOTION_INVALID` / `_FAILED`).
#[test]
fn atip_reasons_08_a_reason_never_changes_the_exit_or_the_retryable_value_and_a_different_one_is_a_different_code() {
    use canon_cli::cli_error::{CliError, CliErrorExt as _};
    #[cfg(debug_assertions)]
    for info in all_codes().iter().filter(|info| !info.reasons.is_empty()) {
        for r in info.reasons {
            let built = CliError::refused(info.code, "x").reason(r.reason);
            assert_eq!((built.exit, built.retryable, built.reason_name()), (info.exit, info.retryable, Some(r.reason)), "{}/{}", info.code, r.reason);
            let answered = std::panic::catch_unwind(|| built.emit("canon status", false));
            assert!(answered.is_ok(), "{}/{}: an error with the row's exit and retryable was refused", info.code, r.reason);
            let mut other_exit = CliError::refused(info.code, "x").reason(r.reason);
            other_exit.exit = if info.exit == 1 { 2 } else { 1 };
            let mut other_retryable = CliError::refused(info.code, "x").reason(r.reason);
            other_retryable.retryable = !info.retryable;
            for changed in [other_exit, other_retryable] {
                assert!(std::panic::catch_unwind(|| changed.emit("canon status", false)).is_err(), "{}/{}: an error with another exit or retryable than its row was answered", info.code, r.reason);
            }
        }
    }
    let unavailable = code_info("ARTIFACTS_STORE_UNAVAILABLE").unwrap();
    let required = code_info("ARTIFACTS_STORE_REQUIRED").unwrap();
    assert!(unavailable.retryable && !required.retryable, "a store that is locked or remote is retried, a store with no settings is not");
    assert!(unavailable.reasons.is_empty() && required.reasons.is_empty(), "the two situations are two codes, not one code with a reason");
    let (invalid, failed) = (code_info("PROMOTION_INVALID").unwrap(), code_info("PROMOTION_FAILED").unwrap());
    assert_eq!((invalid.exit, failed.exit), (2, 1), "a refusal and a failed write are two codes");
    // No reason of the registry is the name of another code's condition with a different exit: a reason that is a code (the `_INCOMPLETE` codes
    // say the code of the refusal a later step met) is never the exit or retryable of the answer.
    for info in all_codes().iter().filter(|info| info.code.ends_with("_INCOMPLETE")) {
        assert_eq!(info.exit, 1, "{} is a command that ran and stopped", info.code);
    }
}

/// atip.reasons.08 (failure path): every send site of a code with reasons takes its exit and `retryable` from the registry: no site sets either.
#[test]
fn atip_reasons_08_no_source_sets_an_exit_or_a_retryable_value_beside_the_registry_row() {
    for (file, text) in raising_sources() {
        for marker in [".exit = ", ".retryable = ", ".exit += "] {
            assert!(!text.contains(marker), "{file} sets `{marker}…`: the exit and `retryable` come from the code's registry row");
        }
    }
}

// ── F6: the changelog ──

/// The rows `| `CODE` | `A`, `B` | … |` of the table whose header starts with `| Code | Reasons now |`.
fn changelog_reason_rows() -> BTreeMap<String, BTreeSet<String>> {
    let text = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CHANGELOG.md")).unwrap();
    let unreleased = text.split("\n## 0.35.0").next().unwrap();
    let mut rows = BTreeMap::new();
    let mut in_table = false;
    for line in unreleased.lines() {
        if line.starts_with("| Code | Reasons now |") {
            in_table = true;
            continue;
        }
        if !in_table {
            continue;
        }
        if !line.starts_with('|') {
            break;
        }
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if cells.len() < 3 || cells[1].starts_with("---") || cells[1].starts_with(":--") || cells[1].starts_with("--") {
            continue;
        }
        let ticked = |cell: &str| -> Vec<String> { cell.split('`').skip(1).step_by(2).map(str::to_string).collect() };
        rows.insert(ticked(cells[1]).remove(0), ticked(cells[2]).into_iter().collect());
    }
    rows
}

/// atip.reasons.09: the table of the CHANGELOG lists the reasons the registry holds for each code it names, and `STOPPED_MIDWAY` is in no registry
/// view and no output; the reason that went with it (`SKILLS_INVALID_ARGUMENT`) is in no registry either.
#[test]
fn atip_reasons_09_the_changelog_table_is_the_registry_and_stopped_midway_and_its_reason_are_gone() {
    let rows = changelog_reason_rows();
    assert!(rows.len() >= 14, "the CHANGELOG lists the codes whose reasons the change settled: {rows:?}");
    for (code, reasons) in &rows {
        let info = code_info(code).unwrap_or_else(|| panic!("the CHANGELOG names {code}, which is no registered code"));
        let registered: BTreeSet<String> = info.reasons.iter().map(|r| r.reason.to_string()).collect();
        assert_eq!(&registered, reasons, "{code}: the CHANGELOG and the registry list different reasons");
    }
    // Each code of the brief's table of F2 is in the CHANGELOG table.
    for code in [
        "CONFIG_INVALID", "SUBJECT_CONFLICT", "BRIEF_INVALID", "BRIEF_CHECK_FAILED", "CONTEXT_INPUT_INVALID", "SKILLS_CHECK_FAILED", "REPORT_CHECK_FAILED", "DISPATCH_CONFLICT", "DIVERGENCE_DENIED",
        "DIVERGENCE_CONFLICT", "MIGRATION_INCOMPLETE", "INVENTORY_INCOMPLETE", "INIT_INCOMPLETE",
    ] {
        assert!(rows.contains_key(code), "{code} is in the brief's table and not in the CHANGELOG's");
    }
    assert!(code_info("STOPPED_MIDWAY").is_none(), "STOPPED_MIDWAY is a code of 0.35");
    assert!(all_codes().iter().all(|info| info.reasons.iter().all(|r| r.reason != "SKILLS_INVALID_ARGUMENT")), "SKILLS_INVALID_ARGUMENT is a reason of 0.35");
    let out = hermetic::canon().args(["schema", "--errors", "--json"]).output().unwrap();
    assert!(!String::from_utf8_lossy(&out.stdout).contains("STOPPED_MIDWAY"), "the registry view names STOPPED_MIDWAY");
}
