//! canon-0-35-canon-stopped-midway: canon registers the reserved code `STOPPED_MIDWAY` (spec 7.2.1, 8.3.2) and
//! says it where a later step of a command that works in steps meets, after an earlier step made its effects, what the
//! command refuses before its work. `reason` is that refusal's code, `meta.next` leads with a command that reads what
//! was done, and the real I/O failure codes keep their names.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use canon_cli::cli_error::{all_codes, code_info, CliError, CliErrorExt as _, CodeInfo, ReasonInfo, EXIT_FAILED, EXIT_REFUSED};
use serde_json::Value;

// ------------------------------------------------------------------ helpers

fn canon(dir: &Path, args: &[&str], source: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(dir).env_remove("CANON_ROLE").env("CANON_SKILLS_SOURCE", source).output().expect("spawn canon")
}

fn document(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| panic!("one JSON document ({error}): {}\nstderr: {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)))
}

/// A valid skill source: the stub's text, which `canon skills install` projects.
fn skill_source(dir: &Path) -> PathBuf {
    let source = dir.join("source");
    std::fs::create_dir_all(source.join("reference")).expect("a source dir");
    std::fs::write(source.join("SKILL.src.md"), "---\nname: canon\ndescription: umbrella\n---\n\n# Canon\n").expect("SKILL.src.md");
    std::fs::write(source.join("reference/topic.md"), "# topic\n").expect("a reference");
    source
}

/// A fresh repo directory that holds a `.pi` directory, so `init` chooses to install the canon skill for pi.
fn pi_repo(dir: &Path, name: &str) -> PathBuf {
    let repo = dir.join(name);
    std::fs::create_dir_all(repo.join(".pi")).expect("a repo with .pi");
    repo
}

fn projected(repo: &Path) -> bool {
    repo.join(".pi/skills/canon/SKILL.md").is_file()
}

/// Makes `path` read-only and gives it back when dropped, so the temp dir can be removed.
struct ReadOnly(PathBuf);

impl ReadOnly {
    fn new(path: &Path) -> Self {
        let mut permissions = std::fs::metadata(path).expect("metadata").permissions();
        let mode = std::os::unix::fs::PermissionsExt::mode(&permissions);
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, mode & !0o222);
        std::fs::set_permissions(path, permissions).expect("make read-only");
        Self(path.to_path_buf())
    }
}

impl Drop for ReadOnly {
    fn drop(&mut self) {
        if let Ok(meta) = std::fs::metadata(&self.0) {
            let mut permissions = meta.permissions();
            let mode = std::os::unix::fs::PermissionsExt::mode(&permissions);
            std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, mode | 0o700);
            let _ = std::fs::set_permissions(&self.0, permissions);
        }
    }
}

/// What is wrong with the `STOPPED_MIDWAY` row of `rows` (spec 7.2.1, 8.3.2, 7.6, the checker's `L1.registry`): it is
/// registered with the reserved exit and retryability, and each reason is the name of a registered exit-2 code, once.
fn registry_problems(rows: &[CodeInfo]) -> Vec<String> {
    let Some(row) = rows.iter().find(|row| row.code == "STOPPED_MIDWAY") else { return vec!["STOPPED_MIDWAY is not registered".to_string()] };
    let mut problems = Vec::new();
    if row.exit != EXIT_FAILED || row.retryable {
        problems.push(format!("STOPPED_MIDWAY is exit {} retryable {}: the reserved row is exit 1, not retryable", row.exit, row.retryable));
    }
    if row.reasons.is_empty() {
        problems.push("STOPPED_MIDWAY has no reason".to_string());
    }
    let mut seen = BTreeSet::new();
    for reason in row.reasons {
        if !seen.insert(reason.reason) {
            problems.push(format!("the reason {} appears twice", reason.reason));
        }
        if reason.meaning.is_empty() || reason.fix.is_empty() {
            problems.push(format!("the reason {} has no meaning or no fix", reason.reason));
        }
        match rows.iter().find(|other| other.code == reason.reason) {
            None => problems.push(format!("the reason {} is not a registered code", reason.reason)),
            Some(other) if other.exit != EXIT_REFUSED => problems.push(format!("the reason {} is exit {}, a reason is the code of an exit-2 refusal", reason.reason, other.exit)),
            Some(_) => {}
        }
    }
    let names: Vec<&str> = row.reasons.iter().map(|reason| reason.reason).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    if names != sorted {
        problems.push(format!("the reasons are not sorted by name: {names:?}"));
    }
    problems
}

/// Reasons a registry must not hold under `STOPPED_MIDWAY`.
static NOT_A_CODE: [ReasonInfo; 1] = [ReasonInfo { reason: "NOT_A_CODE", meaning: "m", fix: "f" }];
static AN_EXIT_1_CODE: [ReasonInfo; 1] = [ReasonInfo { reason: "WRITE_FAILED", meaning: "m", fix: "f" }];
static UNSORTED: [ReasonInfo; 2] = [ReasonInfo { reason: "WRITE_DENIED", meaning: "m", fix: "f" }, ReasonInfo { reason: "SKILLS_SOURCE_NOT_FOUND", meaning: "m", fix: "f" }];

/// The registry with the reasons of its `STOPPED_MIDWAY` row replaced by `reasons`.
fn with_reasons(reasons: &'static [ReasonInfo]) -> Vec<CodeInfo> {
    all_codes().iter().map(|row| if row.code == "STOPPED_MIDWAY" { CodeInfo { reasons, ..*row } } else { *row }).collect()
}

/// What is wrong with `envelope` as the failure of `command` that stopped midway (spec 8.3.2): `meta.next` is not empty and its first
/// step is a command that reads, which is not the command that stopped.
fn first_step_problems(envelope: &Value, command: &str, root: &Path, source: &Path) -> Vec<String> {
    let Some(next) = envelope["meta"]["next"].as_array().filter(|next| !next.is_empty()) else { return vec!["meta.next is missing or empty".to_string()] };
    let argv: Vec<&str> = next[0]["argv"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let mut problems = Vec::new();
    if argv.first().copied() == Some(command) {
        problems.push(format!("the first step `{}` repeats the command that stopped", next[0]["command"]));
    }
    let leaf: Vec<&str> = argv.iter().copied().take_while(|word| !word.starts_with('-')).collect();
    let mut schema_args = vec!["schema"];
    schema_args.extend(&leaf);
    schema_args.push("--json");
    let described = document(&canon(root, &schema_args, source));
    if described["data"]["effect"] != "read" {
        problems.push(format!("the first step `{}` has effect {}, not read", next[0]["command"], described["data"]["effect"]));
    }
    problems
}

// ------------------------------------------------------------------ F1: the code and its reasons

/// atip.midway.01: `STOPPED_MIDWAY` is registered with exit 1 and not retryable, every reason is the name of a registered exit-2
/// code with a meaning and a fix, and `canon schema --errors --json` and `--error STOPPED_MIDWAY --json` give the same reasons.
#[test]
fn atip_midway_01_stopped_midway_is_registered_with_its_reasons_and_both_schema_views_agree() {
    assert_eq!(registry_problems(all_codes()), Vec::<String>::new());
    let row = code_info("STOPPED_MIDWAY").expect("registered");
    assert_eq!((row.exit, row.retryable), (1, false));
    let names: Vec<&str> = row.reasons.iter().map(|reason| reason.reason).collect();
    assert_eq!(names, ["CONFIG_INVALID", "PROMOTION_INVALID", "SKILLS_ARGUMENT_INVALID", "SKILLS_SOURCE_NOT_FOUND", "STRATEGY_PROMOTION_INVALID"], "the refusals a later step of canon meets after an earlier step wrote (a record staged and then refused by the gate, a strategy demoted and then blocked, skills refused after init wrote); an I/O category (`WRITE_UNAVAILABLE`, whose code for the work is `WRITE_FAILED`) is not a reason");
    for reason in row.reasons {
        let refusal = code_info(reason.reason).expect("a registered code");
        assert_ne!(reason.fix, refusal.fix, "{}: the fix says what to do once the caller has read what was done, not the refusal's own fix", reason.reason);
        assert!(reason.fix.contains("canon status"), "{}: the fix names the command that reads what was done: {}", reason.reason, reason.fix);
    }

    let dir = tempfile::tempdir().expect("a temp dir");
    let source = skill_source(dir.path());
    let all = document(&canon(dir.path(), &["schema", "--errors", "--json"], &source));
    let from_all = all["data"]["errors"].as_array().expect("errors").iter().find(|error| error["code"] == "STOPPED_MIDWAY").expect("the row in the registry").clone();
    let one = document(&canon(dir.path(), &["schema", "--error", "STOPPED_MIDWAY", "--json"], &source));
    let from_one = one["data"]["error"].clone();
    assert_eq!(from_all, from_one, "the registry and the one-code view are the same row");
    assert_eq!((from_one["exit"].clone(), from_one["retryable"].clone()), (Value::from(1), Value::Bool(false)));
    let reasons: Vec<&str> = from_one["reasons"].as_array().expect("reasons").iter().filter_map(|reason| reason["reason"].as_str()).collect();
    assert_eq!(reasons, names);
    assert!(atip::registry_entry_problems(&from_one).is_empty(), "the row validates against the contract's registry-entry schema");
}

/// atip.midway.02: a reason that is not a registered code, or is a code of exit 1, makes the registry check fail, and an error that
/// names a reason the registry does not hold under its code fails where it is rendered.
#[test]
fn atip_midway_02_a_reason_that_is_not_a_registered_exit_2_code_fails() {
    let unregistered = with_reasons(&NOT_A_CODE);
    assert!(registry_problems(&unregistered).iter().any(|problem| problem.contains("NOT_A_CODE") && problem.contains("not a registered code")), "{:?}", registry_problems(&unregistered));
    let exit_one = with_reasons(&AN_EXIT_1_CODE);
    assert!(registry_problems(&exit_one).iter().any(|problem| problem.contains("WRITE_FAILED") && problem.contains("exit 1")), "{:?}", registry_problems(&exit_one));
    let unsorted = with_reasons(&UNSORTED);
    assert!(registry_problems(&unsorted).iter().any(|problem| problem.contains("not sorted")), "{:?}", registry_problems(&unsorted));
    let missing: Vec<CodeInfo> = all_codes().iter().filter(|row| row.code != "STOPPED_MIDWAY").copied().collect();
    assert_eq!(registry_problems(&missing), ["STOPPED_MIDWAY is not registered"]);

    // The code is not registered with another exit: a row at exit 2, or one that can be retried, fails the check.
    let retryable: Vec<CodeInfo> = all_codes().iter().map(|row| if row.code == "STOPPED_MIDWAY" { CodeInfo { retryable: true, ..*row } } else { *row }).collect();
    assert!(!registry_problems(&retryable).is_empty());

    // A reason the registry does not hold under the code fails where the error is rendered (a debug build asserts it).
    let held = CliError::failed("STOPPED_MIDWAY", "m").reason("SKILLS_SOURCE_NOT_FOUND");
    assert_eq!(held.body().reason.as_deref(), Some("SKILLS_SOURCE_NOT_FOUND"));
    assert_eq!((held.exit, held.retryable), (1, false), "the exit and retryability come from the row");
    let made_up = CliError::failed("STOPPED_MIDWAY", "m").reason("MADE_UP");
    assert!(code_info("STOPPED_MIDWAY").expect("row").find_reason("MADE_UP").is_none());
    assert!(std::panic::catch_unwind(|| made_up.emit("canon init", true)).is_err(), "emitting a reason the registry does not hold fails");
    let exit_one_reason = CliError::failed("STOPPED_MIDWAY", "m").reason("WRITE_FAILED");
    assert!(std::panic::catch_unwind(|| exit_one_reason.emit("canon init", true)).is_err(), "an exit-1 code is not a reason");
}

// ------------------------------------------------------------------ the moved command: init

/// atip.midway.03: `canon init` writes canon.yaml, then its last step meets a skill source that `canon skills install` refuses before
/// its work: exit 1, `STOPPED_MIDWAY`, reason `SKILLS_SOURCE_NOT_FOUND`, not retryable, and what was written stays.
#[test]
fn atip_midway_03_init_stops_midway_when_the_skill_install_meets_a_refusal_after_the_writes() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let repo = pi_repo(dir.path(), "repo");
    let out = canon(&repo, &["init", "--json"], &dir.path().join("missing"));
    assert_eq!(out.status.code(), Some(1), "{}", String::from_utf8_lossy(&out.stdout));
    let envelope = document(&out);
    assert_eq!(envelope["ok"], false);
    let error = &envelope["error"];
    assert_eq!((error["code"].clone(), error["reason"].clone(), error["retryable"].clone()), (Value::from("STOPPED_MIDWAY"), Value::from("SKILLS_SOURCE_NOT_FOUND"), Value::Bool(false)), "{envelope}");
    assert!(error["message"].as_str().is_some_and(|message| message.contains("canon.yaml") && message.contains("did not install")), "the message says what was done: {error}");
    assert!(atip::envelope_problems(&envelope).is_empty(), "{envelope}");
    assert!(std::fs::read_to_string(repo.join("canon.yaml")).expect("canon.yaml stays").ends_with("skills:\n  providers: [pi]\n"));
    assert!(repo.join(".canon/policy.yaml").is_file() && repo.join("AGENTS.md").is_file(), "the other writes stay");
    assert!(!projected(&repo));
    // The text mode says the same code on stderr.
    let again = pi_repo(dir.path(), "again");
    let text = canon(&again, &["init"], &dir.path().join("missing"));
    assert_eq!(text.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&text.stderr).starts_with("canon: STOPPED_MIDWAY: "), "{}", String::from_utf8_lossy(&text.stderr));
}

/// atip.midway.04: the same step meets a directory it cannot write: that is the I/O category, whose code for the work is `WRITE_FAILED`
/// (spec 8.3): exit 1, no reason, not `STOPPED_MIDWAY`; and `meta.next` still leads with the read.
#[test]
fn atip_midway_04_a_skill_directory_that_cannot_be_written_after_the_writes_is_write_failed() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let source = skill_source(dir.path());
    let repo = pi_repo(dir.path(), "repo");
    let _guard = ReadOnly::new(&repo.join(".pi"));
    let out = canon(&repo, &["init", "--json"], &source);
    assert_eq!(out.status.code(), Some(1), "{}", String::from_utf8_lossy(&out.stdout));
    let envelope = document(&out);
    assert_eq!(envelope["error"]["code"], "WRITE_FAILED", "{envelope}");
    assert!(envelope["error"].get("reason").is_none(), "{envelope}");
    assert!(repo.join("canon.yaml").is_file(), "canon.yaml was written before the install met the directory");
    assert!(!projected(&repo));
    assert_eq!(first_step_problems(&envelope, "init", &repo, &source), Vec::<String>::new(), "{envelope}");
}

/// atip.midway.05: a stop that is not a refusal of `canon skills install` is a failure of the skills category and keeps its own code
/// (`SKILLS_FAILED`, exit 1), with no reason; it is not `STOPPED_MIDWAY`.
#[test]
fn atip_midway_05_a_skill_install_failure_that_is_not_a_refusal_keeps_the_skills_failure_code() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let source = skill_source(dir.path());
    let repo = pi_repo(dir.path(), "repo");
    let elsewhere = dir.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).expect("a directory");
    std::os::unix::fs::symlink(&elsewhere, repo.join(".pi/skills")).expect("a symlink at the skills directory");
    let out = canon(&repo, &["init", "--json"], &source);
    assert_eq!(out.status.code(), Some(1), "{}", String::from_utf8_lossy(&out.stdout));
    let envelope = document(&out);
    assert_eq!(envelope["error"]["code"], "SKILLS_FAILED", "{envelope}");
    assert!(envelope["error"].get("reason").is_none(), "{envelope}");
    assert!(repo.join("canon.yaml").is_file());
    let problems = first_step_problems(&envelope, "init", &repo, &source);
    assert_eq!(problems, Vec::<String>::new(), "the failure after the writes reads what was done first too");
}

/// atip.midway.06: nothing was written, so nothing stopped midway: `init --dry-run` meets the same refusal and exits 2 with the
/// refusal's own code, never `STOPPED_MIDWAY`, and the repo is as it was.
#[test]
fn atip_midway_06_a_refusal_with_no_effect_is_exit_2_with_the_refusals_own_code() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let repo = pi_repo(dir.path(), "repo");
    let out = canon(&repo, &["init", "--dry-run", "--json"], &dir.path().join("missing"));
    assert_eq!(out.status.code(), Some(2), "{}", String::from_utf8_lossy(&out.stdout));
    let envelope = document(&out);
    assert_eq!(envelope["error"]["code"], "SKILLS_SOURCE_NOT_FOUND", "{envelope}");
    assert!(envelope["error"].get("reason").is_none());
    assert!(!repo.join("canon.yaml").exists(), "a preview writes nothing");
    assert_eq!(std::fs::read_dir(&repo).expect("the repo").count(), 1, "only the .pi directory the test made");
    // And a refusal before any step is exit 2 too: an unknown provider names the refusal and writes nothing.
    let unknown = pi_repo(dir.path(), "unknown");
    let refused = canon(&unknown, &["init", "--providers", "gemini", "--json"], &dir.path().join("missing"));
    assert_eq!(refused.status.code(), Some(2));
    assert_eq!(document(&refused)["error"]["code"], "INIT_ARGUMENT_INVALID");
    assert!(!unknown.join("canon.yaml").exists());
}

// ------------------------------------------------------------------ F2: meta.next reads what was done

/// atip.midway.07: the failure's `meta.next` is not empty, its first step reads (a `read` command, never `init`), and running it shows
/// the repo `init` wrote; the repeat comes after it. A first step that repeats the write, or no step, fails the check.
#[test]
fn atip_midway_07_the_first_next_step_of_a_stopped_command_reads_what_was_done() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let source = skill_source(dir.path());
    let repo = pi_repo(dir.path(), "repo");
    let out = canon(&repo, &["init", "--json"], &dir.path().join("missing"));
    let envelope = document(&out);
    assert_eq!(first_step_problems(&envelope, "init", &repo, &source), Vec::<String>::new(), "{envelope}");
    let next = envelope["meta"]["next"].as_array().expect("next");
    assert_eq!(next[0]["argv"], serde_json::json!(["status", "--json"]));
    assert_eq!(next[1]["argv"], serde_json::json!(["skills", "install", "--json"]), "the repeat of the install comes after the read");
    assert_ne!(next[0]["argv"][0], "init");
    let read = canon(&repo, &["status", "--json"], &source);
    assert_eq!(read.status.code(), Some(0), "the first step runs: {}", String::from_utf8_lossy(&read.stderr));
    assert_eq!(document(&read)["ok"], true);

    // The checks themselves fail on a bad envelope.
    let without_next = serde_json::json!({"ok": false, "error": {"code": "STOPPED_MIDWAY"}, "meta": {}});
    assert_eq!(first_step_problems(&without_next, "init", &repo, &source), ["meta.next is missing or empty"]);
    let empty = serde_json::json!({"ok": false, "meta": {"next": []}});
    assert_eq!(first_step_problems(&empty, "init", &repo, &source), ["meta.next is missing or empty"]);
    let repeats = serde_json::json!({"ok": false, "meta": {"next": [{"command": "canon init --json", "argv": ["init", "--json"], "reason": "again"}]}});
    let problems = first_step_problems(&repeats, "init", &repo, &source);
    assert!(problems.iter().any(|problem| problem.contains("repeats the command")) && problems.iter().any(|problem| problem.contains("not read")), "{problems:?}");
    let writes = serde_json::json!({"ok": false, "meta": {"next": [{"command": "canon skills install --json", "argv": ["skills", "install", "--json"], "reason": "install"}]}});
    assert!(first_step_problems(&writes, "init", &repo, &source).iter().any(|problem| problem.contains("not read")));
}

/// atip.midway.08: in a batch, an element that stops midway is `failed` with `STOPPED_MIDWAY` and its reason as its own error (the item
/// error, with the element's `hint`), the elements after it are `skipped`, the batch is `BATCH_INCOMPLETE` and what the element wrote stays
/// (spec 8.3.2, 11.11). The same holds in the unit test of `input.rs` that reads a child's envelope.
#[test]
fn atip_midway_08_a_batch_element_that_stops_midway_is_failed_with_its_reason_and_the_batch_is_incomplete() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let repo = pi_repo(dir.path(), "repo");
    let input = serde_json::json!([{"providers": "pi"}, {"providers": "pi", "language": "en"}]).to_string();
    let out = canon(&repo, &["init", "--input", &input, "--json"], &dir.path().join("missing"));
    assert_eq!(out.status.code(), Some(1), "{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let envelope = document(&out);
    assert_eq!(envelope["error"]["code"], "BATCH_INCOMPLETE", "{envelope}");
    let items = envelope["data"]["items"].as_array().expect("items");
    assert_eq!(items[0]["status"], "failed", "{envelope}");
    assert_eq!((items[0]["error"]["code"].clone(), items[0]["error"]["reason"].clone()), (Value::from("STOPPED_MIDWAY"), Value::from("SKILLS_SOURCE_NOT_FOUND")), "{envelope}");
    assert_eq!(items[1]["status"], "skipped", "{envelope}");
    assert!(repo.join("canon.yaml").is_file(), "the element's effects stay");
}

// ------------------------------------------------------------------ F3: the audit

/// What the audit decided about an exit-1 code of 0.34.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Class {
    /// The code is `STOPPED_MIDWAY` with this reason now, and the old name is gone.
    Move(&'static str),
    /// The code stays, for the kind of reason this says.
    Keep(Why),
}

/// Why a code stays.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Why {
    /// A real I/O failure category of the work (a write, a store, a read that failed after the command started): its own `*_FAILED`
    /// name stays (spec 8.3).
    Io,
    /// The command ran to the end and its answer is negative or incomplete: a finding, a verdict, a failed check. No later step met a refusal.
    Answer,
    /// A bug of canon, whatever the command line was.
    Internal,
    /// The batch's own reserved code, which carries its items' errors (spec 11.11).
    Batch,
}

/// The audit of every exit-1 code of the registry (0.34's, `INIT_SKILL_INSTALL_FAILED` included), each with a line that says why.
/// `STOPPED_MIDWAY` itself is the destination and is not audited.
const AUDIT: &[(&str, Class, &str)] = &[
    ("ADAPTER_AUTHORIZATION_FAILED", Class::Keep(Why::Answer), "`adapter authorize` answered that the response does not authorize execution"),
    ("ADAPTER_VALIDATION_FAILED", Class::Keep(Why::Answer), "`adapter validate`'s negative answer on an envelope"),
    ("ARTIFACTS_STORE_FAILED", Class::Keep(Why::Io), "a blob write or upload failed after the command began writing"),
    ("ARTIFACTS_VERIFY_FAILED", Class::Keep(Why::Answer), "`artifacts verify` found a bound file missing or altered"),
    ("BATCH_INCOMPLETE", Class::Keep(Why::Batch), "the batch's own reserved code; an element that stops midway is its item error"),
    ("BRIEF_CHECK_FAILED", Class::Keep(Why::Answer), "`brief check`'s findings"),
    ("BRIEF_WRITE_FAILED", Class::Keep(Why::Io), "the brief file could not be written"),
    ("CHANGE_ADOPTION_INCOMPLETE", Class::Keep(Why::Io), "judged: the only cause is the second record's store write failing after the first was written, which is the I/O category (`WRITE_DENIED` or `WRITE_INVALID` before the work, `WRITE_FAILED` during it, spec 8.3), not a refusal of an argument; a tool keeps a code of its own for it until it chooses to move (migration guide 7)"),
    ("CONFIG_CHECK_FAILED", Class::Keep(Why::Answer), "`init --check` found a section that fails its loader"),
    ("CONTEXT_PACK_VERIFY_FAILED", Class::Keep(Why::Answer), "`context-pack verify` found the pack changed: the check's negative answer"),
    ("DASHBOARD_FAILED", Class::Keep(Why::Io), "the dashboard could not start (an I/O failure)"),
    ("EVIDENCE_STORE_FAILED", Class::Keep(Why::Io), "a bound file could not be written to the artifact store; the record or batch is rolled back"),
    ("FORMAT_CHECK_FAILED", Class::Keep(Why::Answer), "`format`'s findings"),
    ("GATE_CHECK_FAILED", Class::Keep(Why::Answer), "`gate check`'s findings"),
    ("HOOKS_INSTALL_FAILED", Class::Keep(Why::Io), "a hook file could not be written after the install began"),
    ("INGEST_FAILED", Class::Keep(Why::Io), "an ingest read or write failed"),
    ("INIT_SKILL_INSTALL_FAILED", Class::Move("SKILLS_SOURCE_NOT_FOUND"), "moved: `init` wrote canon.yaml, then its last step met what `canon skills install` refuses before its work (the skill source or `skills:` section: SKILLS_SOURCE_NOT_FOUND); an unwritable target is WRITE_FAILED and any other failure SKILLS_FAILED, which keep their names"),
    ("INTERNAL", Class::Keep(Why::Internal), "a bug of canon"),
    ("INVENTORY_ABORTED", Class::Keep(Why::Answer), "`inventory sync`'s findings"),
    ("INVENTORY_FAILED", Class::Keep(Why::Io), "the record store failed while the index was read or written"),
    ("STRATEGY_DEMOTION_FAILED", Class::Keep(Why::Io), "the demotion's write failed"),
    ("LEARN_PROMOTION_FAILED", Class::Keep(Why::Io), "the strategy store or file could not be written"),
    ("STRATEGY_ROLLBACK_FAILED", Class::Keep(Why::Io), "the rollback could not be recorded"),
    ("LEDGER_COMPACT_FAILED", Class::Keep(Why::Io), "a version could not be moved to the archive"),
    ("MIGRATE_INDEX_FAILED", Class::Keep(Why::Io), "the index rebuild failed after the files moved"),
    ("MIGRATE_PINS_FAILED", Class::Keep(Why::Io), "the pins could not be written after the files moved"),
    ("MIGRATE_STOPPED", Class::Keep(Why::Io), "judged: every site is a write, move, remove or re-import that failed after `migrate` began changing the repo; the one re-check of a destination at apply time repeats a plan-time refusal that only a concurrent writer can make true, which no test can build"),
    ("PLUGIN_SYNC_ERRORS", Class::Keep(Why::Io), "some candidates could not be written"),
    ("POLICY_APPROVAL_FAILED", Class::Keep(Why::Io), "the approval file could not be written"),
    ("PROMOTE_FAILED", Class::Keep(Why::Io), "judged: the staged record was written, then the ledger write failed (an I/O failure of the work)"),
    ("QUERY_FAILED", Class::Keep(Why::Io), "the ledger could not be read"),
    ("REPORT_FAILED", Class::Keep(Why::Io), "the report could not be built or checked"),
    ("REPORT_CHECK_FAILED", Class::Keep(Why::Answer), "`report check` found the report out of date"),
    ("RETENTION_EXPORT_FAILED", Class::Keep(Why::Io), "the export's write failed"),
    ("RETENTION_PURGE_FAILED", Class::Keep(Why::Io), "the purge stopped after it began deleting"),
    ("SEARCH_INDEX_FAILED", Class::Keep(Why::Io), "the records or the index could not be read or built"),
    ("SELFTEST_FAILED", Class::Keep(Why::Answer), "a selftest suite did not match its expected result"),
    ("SERIALIZE_FAILED", Class::Keep(Why::Internal), "a result that cannot be serialized is a bug of canon"),
    ("SKILLS_CHECK_FAILED", Class::Keep(Why::Answer), "`skills check`'s finding"),
    ("SKILLS_FAILED", Class::Keep(Why::Io), "a skills command's own I/O failure; `init` says it too when the install fails and `canon skills install` would not have refused it"),
    ("TIER_FAILED", Class::Keep(Why::Io), "tier aging failed"),
    ("UNVERIFIABLE", Class::Keep(Why::Answer), "a check that examined nothing"),
    ("WRITE_FAILED", Class::Keep(Why::Io), "a write began and failed"),
];

/// The names of the sources of canon, outside the registry and the table derived from it.
fn source_text() -> String {
    fn walk(dir: &Path, text: &mut String) {
        for entry in std::fs::read_dir(dir).expect("a source directory") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                walk(&path, text);
            } else if path.extension().is_some_and(|extension| extension == "rs") && !path.ends_with("cli_error.rs") && !path.ends_with("schema_codes.rs") {
                text.push_str(&std::fs::read_to_string(&path).expect("a source file"));
            }
        }
    }
    let mut text = String::new();
    walk(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut text);
    text
}

/// What is wrong between `audit`, the registry `rows` and the `sources`: every exit-1 code of the registry has a line, every kept
/// code is registered and in the sources, every moved code is in neither and its reason is registered under `STOPPED_MIDWAY`.
fn audit_problems(rows: &[CodeInfo], audit: &[(&str, Class, &str)], sources: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let audited: BTreeSet<&str> = audit.iter().map(|(code, _, _)| *code).collect();
    if audited.len() != audit.len() {
        problems.push("a code is audited twice".to_string());
    }
    let literal = |code: &str| sources.contains(&format!("\"{code}\""));
    for row in rows.iter().filter(|row| row.exit == EXIT_FAILED && row.code != "STOPPED_MIDWAY") {
        if !audited.contains(row.code) {
            problems.push(format!("{} is an exit-1 code and is not in the audit", row.code));
        }
    }
    let stopped = rows.iter().find(|row| row.code == "STOPPED_MIDWAY");
    for (code, class, why) in audit {
        if why.is_empty() {
            problems.push(format!("{code} has no line that says why"));
        }
        let row = rows.iter().find(|row| row.code == *code);
        match class {
            Class::Keep(_) => match row {
                None => problems.push(format!("{code} is kept and is not in the registry")),
                Some(row) if row.exit != EXIT_FAILED => problems.push(format!("{code} is kept as an exit-1 code and the registry has exit {}", row.exit)),
                Some(_) if !literal(code) => problems.push(format!("{code} is kept and no source builds it")),
                Some(_) => {}
            },
            Class::Move(reason) => {
                if row.is_some() {
                    problems.push(format!("{code} is moved and is still in the registry"));
                }
                if literal(code) {
                    problems.push(format!("{code} is moved and a source still builds it"));
                }
                if !stopped.is_some_and(|stopped| stopped.reasons.iter().any(|registered| registered.reason == *reason)) {
                    problems.push(format!("{code} moves to the reason {reason}, which is not registered under STOPPED_MIDWAY"));
                }
            }
        }
    }
    problems
}

/// atip.midway.09: the audit lists every exit-1 code of the registry in two classes (moved, kept), each kept code is registered and
/// built by a source, each moved code is gone from both. A code the table lacks, a kept code no source builds, and a moved code that
/// comes back each fail the check.
#[test]
fn atip_midway_09_the_audit_covers_every_exit_1_code_and_each_code_exists_where_the_table_says() {
    let sources = source_text();
    assert_eq!(audit_problems(all_codes(), AUDIT, &sources), Vec::<String>::new());
    let moved: Vec<&str> = AUDIT.iter().filter(|(_, class, _)| matches!(class, Class::Move(_))).map(|(code, _, _)| *code).collect();
    assert_eq!(moved, ["INIT_SKILL_INSTALL_FAILED"], "the move of this release");
    let kept = AUDIT.iter().filter(|(_, class, _)| matches!(class, Class::Keep(_))).count();
    assert!(kept > 40, "{kept} kept codes");
    // Every candidate the brief names was judged.
    for candidate in ["MIGRATE_STOPPED", "CHANGE_ADOPTION_INCOMPLETE", "PROMOTE_FAILED"] {
        assert!(AUDIT.iter().any(|(code, _, why)| *code == candidate && why.starts_with("judged: ")), "{candidate} is judged in the table");
    }
    assert_audit_faults_are_named(&sources);
}

/// atip.midway.14 (the failure path of F3): an exit-1 code the table lacks, a kept code no source builds, a moved code that comes back
/// in the registry or in a source, and a kept code the registry moved to exit 2 each fail the audit check with their own message.
#[test]
fn atip_midway_14_an_audit_that_misses_a_code_or_a_source_or_a_moved_code_comes_back_fails_the_check() {
    assert_audit_faults_are_named(&source_text());
}

/// The faults the audit check must name, each with its own message; the audit as it stands passes (atip.midway.09 and .14).
fn assert_audit_faults_are_named(sources: &str) {
    // An exit-1 code the table lacks fails.
    let extra = CodeInfo { code: "SOMETHING_NEW_FAILED", meaning: "m", exit: 1, retryable: false, fix: "f", reasons: &[] };
    let mut grown: Vec<CodeInfo> = all_codes().to_vec();
    grown.push(extra);
    assert!(audit_problems(&grown, AUDIT, sources).iter().any(|problem| problem.contains("SOMETHING_NEW_FAILED") && problem.contains("not in the audit")));
    // A kept code that no source builds fails.
    assert!(audit_problems(all_codes(), AUDIT, "").iter().any(|problem| problem.contains("no source builds it")));
    // A moved code that comes back, in the registry or in a source, fails.
    let mut back: Vec<CodeInfo> = all_codes().to_vec();
    back.push(CodeInfo { code: "INIT_SKILL_INSTALL_FAILED", meaning: "m", exit: 1, retryable: false, fix: "f", reasons: &[] });
    assert!(audit_problems(&back, AUDIT, sources).iter().any(|problem| problem.contains("still in the registry")));
    let revived = format!("{sources}\nCliError::refused(\"INIT_SKILL_INSTALL_FAILED\", message)");
    assert!(audit_problems(all_codes(), AUDIT, &revived).iter().any(|problem| problem.contains("a source still builds it")));
    // A kept code that the registry moved to exit 2 fails.
    let moved_to_refusal: Vec<CodeInfo> = all_codes().iter().map(|row| if row.code == "WRITE_FAILED" { CodeInfo { exit: 2, ..*row } } else { *row }).collect();
    assert!(audit_problems(&moved_to_refusal, AUDIT, sources).iter().any(|problem| problem.contains("WRITE_FAILED") && problem.contains("exit 2")));
    // The audit as it stands passes.
    assert_eq!(audit_problems(all_codes(), AUDIT, sources), Vec::<String>::new());
}

// ------------------------------------------------------------------ F4: the I/O failure codes

/// The real I/O failure codes as 0.34 registers them: the code and whether it is retryable.
const IO_CODES: [(&str, bool); 12] = [
    ("ARTIFACTS_STORE_FAILED", true),
    ("BRIEF_WRITE_FAILED", false),
    ("EVIDENCE_STORE_FAILED", true),
    ("HOOKS_INSTALL_FAILED", false),
    ("INVENTORY_FAILED", false),
    ("LEDGER_COMPACT_FAILED", false),
    ("POLICY_APPROVAL_FAILED", false),
    ("RETENTION_EXPORT_FAILED", false),
    ("RETENTION_PURGE_FAILED", false),
    ("TIER_FAILED", false),
    ("WRITE_FAILED", false),
    ("MIGRATE_PINS_FAILED", false),
];

/// What is wrong with the I/O failure codes of `rows`: each is registered at exit 1 with the retryability 0.34 gave it, has no reason of
/// its own, and is no reason of `STOPPED_MIDWAY` (an I/O failure is a failure of the work, not the name of a refusal).
fn io_problems(rows: &[CodeInfo]) -> Vec<String> {
    let mut problems = Vec::new();
    let stopped = rows.iter().find(|row| row.code == "STOPPED_MIDWAY");
    for (code, retryable) in IO_CODES {
        let Some(row) = rows.iter().find(|row| row.code == code) else {
            problems.push(format!("{code} is not registered"));
            continue;
        };
        if (row.exit, row.retryable) != (EXIT_FAILED, retryable) {
            problems.push(format!("{code} is exit {} retryable {}, not exit 1 retryable {retryable}", row.exit, row.retryable));
        }
        if !row.reasons.is_empty() {
            problems.push(format!("{code} has a reason of its own"));
        }
        if stopped.is_some_and(|stopped| stopped.find_reason(code).is_some()) {
            problems.push(format!("{code} is a reason of STOPPED_MIDWAY"));
        }
    }
    problems
}

/// atip.midway.10: the real I/O failure codes keep their names, their exit 1 and their retryability, none is a reason of
/// `STOPPED_MIDWAY`, and none has a reason of its own that stands for a refusal.
#[test]
fn atip_midway_10_the_io_failure_codes_keep_their_names_exits_and_retryability() {
    let problems = io_problems(all_codes());
    assert!(problems.is_empty(), "{problems:?}");
    for (code, _) in IO_CODES {
        assert!(AUDIT.iter().any(|(audited, class, _)| *audited == code && *class == Class::Keep(Why::Io)), "{code} is kept as an I/O failure in the audit");
    }
    // The Io codes of the audit are exactly the codes whose names or situations are failures of the work: none is exit 2.
    for (code, class, _) in AUDIT {
        if let Class::Keep(Why::Io) = class {
            assert_eq!(code_info(code).expect("registered").exit, 1, "{code}");
        }
    }
}

/// atip.midway.13 (the failure path of F4): an I/O failure code that is renamed away, moved to a refusal's exit, made retryable or
/// not, given a reason, or made a reason of `STOPPED_MIDWAY` fails the check of the I/O codes, each with its own message; and a
/// registry that holds them as 0.34 did passes it.
#[test]
fn atip_midway_13_an_io_failure_code_that_moves_or_is_absorbed_fails_the_check() {
    static A_REASON: [ReasonInfo; 1] = [ReasonInfo { reason: "SKILLS_SOURCE_NOT_FOUND", meaning: "m", fix: "f" }];
    static WRITE_FAILED_AS_REASON: [ReasonInfo; 1] = [ReasonInfo { reason: "WRITE_FAILED", meaning: "m", fix: "f" }];
    let changed = |code: &str, with: &dyn Fn(&CodeInfo) -> CodeInfo| -> Vec<CodeInfo> { all_codes().iter().map(|row| if row.code == code { with(row) } else { *row }).collect() };
    assert!(io_problems(all_codes()).is_empty(), "the registry as it stands holds the I/O codes as 0.34 did");

    let absent: Vec<CodeInfo> = all_codes().iter().filter(|row| row.code != "WRITE_FAILED").copied().collect();
    assert_eq!(io_problems(&absent), ["WRITE_FAILED is not registered"]);
    let refusal = changed("TIER_FAILED", &|row| CodeInfo { exit: EXIT_REFUSED, ..*row });
    assert_eq!(io_problems(&refusal), ["TIER_FAILED is exit 2 retryable false, not exit 1 retryable false"]);
    let retryable = changed("WRITE_FAILED", &|row| CodeInfo { retryable: true, ..*row });
    assert_eq!(io_problems(&retryable), ["WRITE_FAILED is exit 1 retryable true, not exit 1 retryable false"]);
    let not_retryable = changed("EVIDENCE_STORE_FAILED", &|row| CodeInfo { retryable: false, ..*row });
    assert_eq!(io_problems(&not_retryable), ["EVIDENCE_STORE_FAILED is exit 1 retryable false, not exit 1 retryable true"]);
    let reasoned = changed("INVENTORY_FAILED", &|row| CodeInfo { reasons: &A_REASON, ..*row });
    assert_eq!(io_problems(&reasoned), ["INVENTORY_FAILED has a reason of its own"]);
    let absorbed = changed("STOPPED_MIDWAY", &|row| CodeInfo { reasons: &WRITE_FAILED_AS_REASON, ..*row });
    assert_eq!(io_problems(&absorbed), ["WRITE_FAILED is a reason of STOPPED_MIDWAY"]);
}

/// atip.midway.12: the adoption writes the subject record, then the change record. When the second store write fails (the change store
/// is read-only, which no up-front check can see in an object store), the failure is the I/O category and keeps `CHANGE_ADOPTION_INCOMPLETE`
/// (exit 1, no reason, never `STOPPED_MIDWAY`): the audit judged it, the first record stays and the repair `canon subject adopt` completes it.
#[cfg(unix)]
#[test]
fn atip_midway_12_a_half_written_adoption_keeps_change_adoption_incomplete_and_its_repair_completes_the_link() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("a temp dir");
    for args in [&["init", "-q", "-b", "main"][..], &["config", "user.name", "t"], &["config", "user.email", "t@example.com"], &["commit", "-q", "--allow-empty", "-m", "x"]] {
        let out = Command::new("git").args(args).current_dir(dir.path()).output().expect("spawn git");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
    let r2 = dir.path().join("r2-local");
    std::fs::write(
        dir.path().join("canon.yaml"),
        "tiers:\n  local: { backend: git, root: .canon/ledger }\n  cold: { backend: s3, bucket_env: CANON_R2_BUCKET_TEST_UNUSED, prefix: \"canon/\" }\nrouting:\n  subject: local\n  change: cold\n  scenario: local\n  evidence: local\n",
    )
    .expect("canon.yaml");
    let run = |args: &[&str]| Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(dir.path()).env_remove("CANON_ROLE").env("CANON_R2_LOCAL_ROOT", &r2).output().expect("spawn canon");
    assert!(run(&["subject", "new", "demo-subject", "--domain", "dev", "--title", "Demo"]).status.success());
    assert!(run(&["subject", "new", "other", "--domain", "dev", "--title", "Other"]).status.success());
    assert!(run(&["change", "new", "s36-demo", "--subject", "other", "--title", "Demo"]).status.success());
    let stored_changes = r2.join("canon/kind=change");
    std::fs::set_permissions(&stored_changes, std::fs::Permissions::from_mode(0o555)).expect("make the change store read-only");
    if std::fs::write(stored_changes.join("probe"), "").is_ok() {
        // Root ignores directory permissions; there is no failure to force.
        std::fs::remove_file(stored_changes.join("probe")).expect("remove the probe");
        std::fs::set_permissions(&stored_changes, std::fs::Permissions::from_mode(0o755)).expect("restore");
        return;
    }
    let out = run(&["subject", "adopt", "demo-subject", "--change", "s36-demo", "--json"]);
    std::fs::set_permissions(&stored_changes, std::fs::Permissions::from_mode(0o755)).expect("restore");
    assert_eq!(out.status.code(), Some(1), "{}", String::from_utf8_lossy(&out.stdout));
    let envelope = document(&out);
    assert_eq!(envelope["error"]["code"], "CHANGE_ADOPTION_INCOMPLETE", "{envelope}");
    assert!(envelope["error"].get("reason").is_none(), "{envelope}");
    assert!(envelope["error"]["hint"].as_str().is_some_and(|hint| hint.contains("canon subject adopt demo-subject --change s36-demo")), "{envelope}");
    assert_eq!(code_info("CHANGE_ADOPTION_INCOMPLETE").map(|row| (row.exit, row.retryable)), Some((1, false)));
    let repair = run(&["subject", "adopt", "demo-subject", "--change", "s36-demo"]);
    assert_eq!(repair.status.code(), Some(0), "{}", String::from_utf8_lossy(&repair.stderr));
    let again = run(&["subject", "adopt", "demo-subject", "--change", "s36-demo"]);
    assert!(String::from_utf8_lossy(&again.stdout).contains("nothing written"), "{}", String::from_utf8_lossy(&again.stdout));
}
