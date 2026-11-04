//! `canon-0-36-codes-incomplete` (atip/1 spec 7.2.2, 7.2.3, 8.3.2): a command that works in steps and stops after an earlier step made its effects
//! answers with its subject's one `<SUBJECT>_INCOMPLETE` code (exit 1) and leads `meta.next` with a command that reads what was done; the reserved
//! midway code of 0.35 is neither registered nor said. Behaviour through the built `canon` binary (exit codes, JSON, what stays written). The tests that
//! need a step to fail after its predecessors really wrote use the debug seam `CANON_TEST_STOP_AT=<step>` (`incomplete::injected_stop`).
//! Each test names the scenario it proves (`atip.midway.NN` and `atip.incomplete.NN`).

mod support;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use canon_cli::cli_error::{
    all_codes, code_info, CliError, CliErrorExt as _, CodeInfo, ReasonInfo, EXIT_FAILED,
    EXIT_REFUSED,
};
use serde_json::Value;

// ------------------------------------------------------------------ helpers

fn canon_env(dir: &Path, args: &[&str], source: Option<&Path>, env: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_canon"));
    command
        .args(args)
        .current_dir(dir)
        .env_remove("CANON_ROLE")
        .env_remove("CANON_TEST_STOP_AT")
        .env("CANON_ACTOR", "canon");
    if let Some(source) = source {
        command.env("CANON_SKILLS_SOURCE", source);
    }
    for (key, value) in env {
        command.env(key, value);
    }
    command.output().expect("spawn canon")
}

fn canon(dir: &Path, args: &[&str], source: &Path) -> Output {
    canon_env(dir, args, Some(source), &[])
}

fn plain(dir: &Path, args: &[&str]) -> Output {
    canon_env(dir, args, None, &[])
}

fn document(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "one JSON document ({error}): {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("spawn git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A valid skill source: the stub's text, which `canon skills install` projects.
fn skill_source(dir: &Path) -> PathBuf {
    let source = dir.join("source");
    std::fs::create_dir_all(source.join("reference")).expect("a source dir");
    std::fs::write(
        source.join("SKILL.src.md"),
        "---\nname: canon\ndescription: umbrella\n---\n\n# Canon\n",
    )
    .expect("SKILL.src.md");
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

/// What is wrong with `envelope` as the failure of `command` that stopped part-way (spec 8.3.2): `meta.next` is not empty and its first
/// step is a command that reads (a `read` command, or the preview of a `--dry-run` that writes nothing), which is not the command that stopped.
fn first_step_problems(envelope: &Value, command: &str, root: &Path, source: &Path) -> Vec<String> {
    let Some(next) = envelope["meta"]["next"]
        .as_array()
        .filter(|next| !next.is_empty())
    else {
        return vec!["meta.next is missing or empty".to_string()];
    };
    let argv: Vec<&str> = next[0]["argv"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let mut problems = Vec::new();
    if argv.first().copied() == Some(command) {
        problems.push(format!(
            "the first step `{}` repeats the command that stopped",
            next[0]["command"]
        ));
    }
    let leaf: Vec<&str> = argv
        .iter()
        .copied()
        .take_while(|word| !word.starts_with('-'))
        .collect();
    let mut schema_args = vec!["schema"];
    schema_args.extend(&leaf);
    schema_args.push("--json");
    let described = document(&canon(root, &schema_args, source));
    if described["data"]["effect"] != "read" && !argv.contains(&"--dry-run") {
        problems.push(format!(
            "the first step `{}` has effect {}, not read",
            next[0]["command"], described["data"]["effect"]
        ));
    }
    problems
}

// ------------------------------------------------------------------ the registry

/// The `<SUBJECT>_INCOMPLETE` codes of canon (the contract's one per subject, spec 8.3.2), each with its retryability and the reasons registered
/// under it. `BATCH_INCOMPLETE` is the batch's own core code and is not in the table.
const INCOMPLETE: &[(&str, bool, &[&str])] = &[
    (
        "ARTIFACTS_MOVE_INCOMPLETE",
        false,
        &[
            "ARTIFACTS_BLOB_CORRUPT",
            "ARTIFACTS_STORE_DENIED",
            "ARTIFACTS_STORE_INVALID",
            "ARTIFACTS_STORE_REQUIRED",
            "ARTIFACTS_STORE_UNAVAILABLE",
        ],
    ),
    ("ARTIFACTS_STORE_INCOMPLETE", true, &["ARTIFACTS_STORE_FAILED", "ARTIFACTS_STORE_UNAVAILABLE"]),
    ("CHANGE_ADOPTION_INCOMPLETE", false, &["WRITE_FAILED"]),
    ("DIVERGENCE_INCOMPLETE", false, &["PROMOTION_INVALID"]),
    ("EVIDENCE_INCOMPLETE", false, &["PROMOTION_INVALID"]),
    ("FINDING_INCOMPLETE", false, &["PROMOTION_INVALID"]),
    ("HOOKS_INSTALL_INCOMPLETE", false, &["WRITE_FAILED"]),
    (
        "INIT_INCOMPLETE",
        false,
        &[
            "CONFIG_INVALID",
            "SKILLS_ARGUMENT_INVALID",
            "SKILLS_SOURCE_NOT_FOUND",
        ],
    ),
    ("INVENTORY_INCOMPLETE", false, &["INVENTORY_FAILED", "INVENTORY_INVALID"]),
    ("LEDGER_COMPACT_INCOMPLETE", false, &["REPO_DENIED", "REPO_INVALID", "REPO_UNAVAILABLE", "WRITE_FAILED"]),
    ("MIGRATION_INCOMPLETE", false, &["INDEX", "PINS", "STOPPED"]),
    ("PLUGIN_SYNC_INCOMPLETE", false, &["WRITE_FAILED"]),
    ("RETENTION_PURGE_INCOMPLETE", false, &["WRITE_FAILED"]),
    (
        "STRATEGY_INCOMPLETE",
        false,
        &["STRATEGY_PROMOTION_INVALID"],
    ),
    ("TIER_INCOMPLETE", false, &["WRITE_FAILED"]),
];

/// The codes whose reasons are the codes of the refusals the later step met (the others name where the work stopped).
const REFUSAL_REASONS: &[&str] = &[
    "ARTIFACTS_MOVE_INCOMPLETE",
    "DIVERGENCE_INCOMPLETE",
    "EVIDENCE_INCOMPLETE",
    "FINDING_INCOMPLETE",
    "INIT_INCOMPLETE",
    "STRATEGY_INCOMPLETE",
];

/// The reserved code of 0.35 and the names `canon-0-36-codes-incomplete` replaced: in no registry and no source.
const GONE: &[&str] = &[
    "EVIDENCE_STORE_FAILED",
    "HOOKS_INSTALL_FAILED",
    "INIT_SKILL_INSTALL_FAILED",
    "INVENTORY_ABORTED",
    "LEARN_PROMOTION_FAILED",
    "LEDGER_COMPACT_FAILED",
    "MIGRATE_INDEX_FAILED",
    "MIGRATE_PINS_FAILED",
    "MIGRATE_STOPPED",
    "PLUGIN_SYNC_ERRORS",
    "PROMOTE_FAILED",
    "RETENTION_PURGE_FAILED",
    "STOPPED_MIDWAY",
];

/// What is wrong with `rows` as the registry of the `_INCOMPLETE` codes (spec 7.2.1, 8.3.2, 7.6, the checker's `L1.registry`): the reserved midway
/// code is not registered; each code of the table is registered at exit 1 with its retryability and its reasons (each with a meaning and a fix, sorted
/// by name, and for a code of [`REFUSAL_REASONS`] the name of a registered exit-2 code); every registered `_INCOMPLETE` code is in the table.
fn registry_problems(rows: &[CodeInfo]) -> Vec<String> {
    let mut problems = Vec::new();
    if rows.iter().any(|row| row.code == "STOPPED_MIDWAY") {
        problems.push("STOPPED_MIDWAY is registered".to_string());
    }
    for (code, retryable, reasons) in INCOMPLETE {
        let Some(row) = rows.iter().find(|row| row.code == *code) else {
            problems.push(format!("{code} is not registered"));
            continue;
        };
        if (row.exit, row.retryable) != (EXIT_FAILED, *retryable) {
            problems.push(format!(
                "{code} is exit {} retryable {}, not exit 1 retryable {retryable}",
                row.exit, row.retryable
            ));
        }
        let names: Vec<&str> = row.reasons.iter().map(|reason| reason.reason).collect();
        if names != *reasons {
            problems.push(format!(
                "{code} registers the reasons {names:?}, not {reasons:?}"
            ));
        }
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        if sorted != names {
            problems.push(format!(
                "{code}: the reasons are not sorted by name, each once: {names:?}"
            ));
        }
        for reason in row.reasons {
            if reason.meaning.is_empty() || reason.fix.is_empty() {
                problems.push(format!(
                    "{code}: the reason {} has no meaning or no fix",
                    reason.reason
                ));
            }
            if REFUSAL_REASONS.contains(code) {
                match rows.iter().find(|other| other.code == reason.reason) {
                    None => problems.push(format!("{code}: the reason {} is not a registered code", reason.reason)),
                    Some(other) if other.exit != EXIT_REFUSED => problems.push(format!("{code}: the reason {} is exit {}, a reason is the code of an exit-2 refusal", reason.reason, other.exit)),
                    Some(_) => {}
                }
            }
        }
    }
    for row in rows
        .iter()
        .filter(|row| row.code.ends_with("_INCOMPLETE") && row.code != "BATCH_INCOMPLETE")
    {
        if !INCOMPLETE.iter().any(|(code, _, _)| *code == row.code) {
            problems.push(format!(
                "{} is an _INCOMPLETE code and is not in the table",
                row.code
            ));
        }
    }
    problems
}

/// What is wrong with the codes the leaves list (a `canon schema <leaf>` answer): a leaf lists at most one `_INCOMPLETE` code of its own (the batch's
/// `BATCH_INCOMPLETE` is the batch's), since there is one per subject.
fn leaf_problems(leaves: &[(String, Vec<String>)]) -> Vec<String> {
    leaves
        .iter()
        .filter_map(|(leaf, codes)| {
            let own: Vec<&String> = codes.iter().filter(|code| code.ends_with("_INCOMPLETE") && *code != "BATCH_INCOMPLETE").collect();
            (own.len() > 1).then(|| format!("`canon {leaf}` lists {} _INCOMPLETE codes of its own ({own:?}): one per subject", own.len()))
        })
        .collect()
}

/// Reasons a registry must not hold under an `_INCOMPLETE` code of [`REFUSAL_REASONS`].
static NOT_A_CODE: [ReasonInfo; 1] = [ReasonInfo {
    reason: "NOT_A_CODE",
    meaning: "m",
    fix: "f",
}];
static AN_EXIT_1_CODE: [ReasonInfo; 1] = [ReasonInfo {
    reason: "WRITE_FAILED",
    meaning: "m",
    fix: "f",
}];
static UNSORTED: [ReasonInfo; 2] = [
    ReasonInfo {
        reason: "WRITE_DENIED",
        meaning: "m",
        fix: "f",
    },
    ReasonInfo {
        reason: "SKILLS_SOURCE_NOT_FOUND",
        meaning: "m",
        fix: "f",
    },
];

/// The registry with the reasons of the row `code` replaced by `reasons`.
fn with_reasons(code: &str, reasons: &'static [ReasonInfo]) -> Vec<CodeInfo> {
    all_codes()
        .iter()
        .map(|row| {
            if row.code == code {
                CodeInfo { reasons, ..*row }
            } else {
                *row
            }
        })
        .collect()
}

/// atip.midway.01: the reserved midway code is neither registered nor listed by `canon schema --errors --json`; each `_INCOMPLETE` code is registered
/// at exit 1 with its retryability and its reasons; and the registry and the one-code view give the same row.
#[test]
fn atip_midway_01_the_reserved_code_is_gone_and_each_incomplete_code_is_registered_with_its_reasons(
) {
    assert_eq!(registry_problems(all_codes()), Vec::<String>::new());
    assert!(
        code_info("STOPPED_MIDWAY").is_none(),
        "the reserved code is not registered"
    );
    for (code, retryable, reasons) in INCOMPLETE {
        let row = code_info(code).expect("registered");
        assert_eq!((row.exit, row.retryable), (1, *retryable), "{code}");
        let names: Vec<&str> = row.reasons.iter().map(|reason| reason.reason).collect();
        assert_eq!(names, *reasons, "{code}");
    }
    // A reason of a refusal is not the refusal's own fix: it says what to do once the caller has read what was done.
    for code in REFUSAL_REASONS {
        for reason in code_info(code).expect("registered").reasons {
            let refusal = code_info(reason.reason).expect("a registered code");
            assert_ne!(reason.fix, refusal.fix, "{code}/{}: the fix says what to do once the caller has read what was done, not the refusal's own fix", reason.reason);
            assert!(
                reason.fix.contains("canon status")
                    || reason.fix.contains("canon artifacts verify")
                    || reason.fix.contains("canon strategy promote"),
                "{code}/{}: the fix names the command that reads what was done: {}",
                reason.reason,
                reason.fix
            );
        }
    }

    let dir = tempfile::tempdir().expect("a temp dir");
    let all = document(&plain(dir.path(), &["schema", "--errors", "--json"]));
    let rows = all["data"]["errors"].as_array().expect("errors");
    assert!(
        rows.iter().all(|error| error["code"] != "STOPPED_MIDWAY"),
        "schema --errors does not list the reserved code"
    );
    for (code, retryable, reasons) in INCOMPLETE {
        let from_all = rows
            .iter()
            .find(|error| error["code"] == *code)
            .unwrap_or_else(|| panic!("{code} is in the registry answer"))
            .clone();
        let one = document(&plain(dir.path(), &["schema", "--error", code, "--json"]));
        assert_eq!(
            from_all, one["data"]["error"],
            "{code}: the registry and the one-code view are the same row"
        );
        assert_eq!(
            (from_all["exit"].clone(), from_all["retryable"].clone()),
            (Value::from(1), Value::Bool(*retryable)),
            "{code}"
        );
        let listed: Vec<&str> = from_all["reasons"]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|reason| reason["reason"].as_str())
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(listed, *reasons, "{code}");
        assert!(
            atip::registry_entry_problems(&from_all).is_empty(),
            "{code} validates against the contract's registry-entry schema"
        );
    }
    // A sync that indexed nothing refuses before its work (spec 7.2.2, 8.3): exit 2 with details and the reasons of what it met.
    let invalid = code_info("INVENTORY_INVALID").expect("registered");
    assert_eq!((invalid.exit, invalid.retryable), (2, false));
    assert_eq!(invalid.reasons.iter().map(|reason| reason.reason).collect::<Vec<_>>(), ["DUPLICATE_SCENARIOS", "VIOLATIONS"]);
    let gone = plain(
        dir.path(),
        &["schema", "--error", "STOPPED_MIDWAY", "--json"],
    );
    assert_ne!(
        gone.status.code(),
        Some(0),
        "the reserved code is no code of canon: {}",
        text(&gone)
    );
}

/// atip.midway.02: a registry that registers the reserved code again, drops an `_INCOMPLETE` code, gives one another exit or retryability or a reason
/// that is no registered exit-2 code, an exit-1 code, or out of order, or holds an `_INCOMPLETE` code the table lacks, fails the check; and an error that
/// names a reason the registry does not hold under its code fails where it is rendered.
#[test]
fn atip_midway_02_a_registry_that_registers_the_reserved_code_or_breaks_a_row_fails_the_check() {
    let unregistered = with_reasons("INIT_INCOMPLETE", &NOT_A_CODE);
    assert!(
        registry_problems(&unregistered)
            .iter()
            .any(|problem| problem.contains("NOT_A_CODE")
                && problem.contains("not a registered code")),
        "{:?}",
        registry_problems(&unregistered)
    );
    let exit_one = with_reasons("INIT_INCOMPLETE", &AN_EXIT_1_CODE);
    assert!(
        registry_problems(&exit_one)
            .iter()
            .any(|problem| problem.contains("WRITE_FAILED") && problem.contains("exit 1")),
        "{:?}",
        registry_problems(&exit_one)
    );
    let unsorted = with_reasons("INIT_INCOMPLETE", &UNSORTED);
    assert!(
        registry_problems(&unsorted)
            .iter()
            .any(|problem| problem.contains("not sorted")),
        "{:?}",
        registry_problems(&unsorted)
    );
    let missing: Vec<CodeInfo> = all_codes()
        .iter()
        .filter(|row| row.code != "INIT_INCOMPLETE")
        .copied()
        .collect();
    assert_eq!(
        registry_problems(&missing),
        ["INIT_INCOMPLETE is not registered"]
    );
    // A row at another exit or retryability fails.
    let at_exit_two: Vec<CodeInfo> = all_codes()
        .iter()
        .map(|row| {
            if row.code == "TIER_INCOMPLETE" {
                CodeInfo {
                    exit: EXIT_REFUSED,
                    ..*row
                }
            } else {
                *row
            }
        })
        .collect();
    assert_eq!(
        registry_problems(&at_exit_two),
        ["TIER_INCOMPLETE is exit 2 retryable false, not exit 1 retryable false"]
    );
    let retryable: Vec<CodeInfo> = all_codes()
        .iter()
        .map(|row| {
            if row.code == "ARTIFACTS_STORE_INCOMPLETE" {
                CodeInfo {
                    retryable: false,
                    ..*row
                }
            } else {
                *row
            }
        })
        .collect();
    assert_eq!(
        registry_problems(&retryable),
        ["ARTIFACTS_STORE_INCOMPLETE is exit 1 retryable false, not exit 1 retryable true"]
    );
    // The reserved code registered again fails, and so does an `_INCOMPLETE` code the table lacks.
    let mut revived: Vec<CodeInfo> = all_codes().to_vec();
    revived.push(CodeInfo {
        code: "STOPPED_MIDWAY",
        meaning: "m",
        exit: 1,
        retryable: false,
        fix: "f",
        reasons: &[],
    });
    assert!(registry_problems(&revived)
        .iter()
        .any(|problem| problem == "STOPPED_MIDWAY is registered"));
    let mut grown: Vec<CodeInfo> = all_codes().to_vec();
    grown.push(CodeInfo {
        code: "SOMETHING_NEW_INCOMPLETE",
        meaning: "m",
        exit: 1,
        retryable: false,
        fix: "f",
        reasons: &[],
    });
    assert!(registry_problems(&grown)
        .iter()
        .any(|problem| problem.contains("SOMETHING_NEW_INCOMPLETE")
            && problem.contains("not in the table")));

    // A reason the registry does not hold under the code fails where the error is rendered (a debug build asserts it).
    let held = CliError::failed("INIT_INCOMPLETE", "m").reason("SKILLS_SOURCE_NOT_FOUND");
    assert_eq!(
        held.body().reason.as_deref(),
        Some("SKILLS_SOURCE_NOT_FOUND")
    );
    assert_eq!(
        (held.exit, held.retryable),
        (1, false),
        "the exit and retryability come from the row"
    );
    let made_up = CliError::failed("INIT_INCOMPLETE", "m").reason("MADE_UP");
    assert!(code_info("INIT_INCOMPLETE")
        .expect("row")
        .find_reason("MADE_UP")
        .is_none());
    assert!(
        std::panic::catch_unwind(|| made_up.emit("canon init", true)).is_err(),
        "emitting a reason the registry does not hold fails"
    );
    let exit_one_reason = CliError::failed("INIT_INCOMPLETE", "m").reason("WRITE_FAILED");
    assert!(
        std::panic::catch_unwind(|| exit_one_reason.emit("canon init", true)).is_err(),
        "an exit-1 code is not a reason"
    );
    // The reason of one code is not the reason of another: each code keeps its own list.
    let borrowed = CliError::failed("EVIDENCE_INCOMPLETE", "m").reason("SKILLS_SOURCE_NOT_FOUND");
    assert!(
        std::panic::catch_unwind(|| borrowed.emit("canon evidence add", true)).is_err(),
        "a reason of another code is not registered here"
    );
}

/// atip.incomplete.11: a command lists at most one `_INCOMPLETE` code of its own, since the contract has one per subject; a leaf that lists two
/// fails the check, and the batch's code is not counted.
#[test]
fn atip_incomplete_11_a_leaf_lists_one_incomplete_code_of_its_own_and_two_fail_the_check() {
    let leaves: Vec<(String, Vec<String>)> = canon_cli::schema::META
        .iter()
        .map(|meta| {
            (
                meta.path.to_string(),
                canon_cli::schema::error_codes(meta.path)
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
            )
        })
        .collect();
    assert!(leaves.len() > 50, "{} leaves", leaves.len());
    assert_eq!(leaf_problems(&leaves), Vec::<String>::new());
    // The leaves that stop part-way list their code.
    let listed = |leaf: &str, code: &str| {
        leaves
            .iter()
            .any(|(path, codes)| path == leaf && codes.iter().any(|listed| listed == code))
    };
    for (leaf, code) in [
        ("init", "INIT_INCOMPLETE"),
        ("evidence add", "EVIDENCE_INCOMPLETE"),
        ("evidence audit", "EVIDENCE_INCOMPLETE"),
        ("finding add", "FINDING_INCOMPLETE"),
        ("finding close", "FINDING_INCOMPLETE"),
        ("divergence add", "DIVERGENCE_INCOMPLETE"),
        ("strategy promote", "STRATEGY_INCOMPLETE"),
        ("artifacts move", "ARTIFACTS_MOVE_INCOMPLETE"),
        ("artifacts store", "ARTIFACTS_STORE_INCOMPLETE"),
        ("gate install-hooks", "HOOKS_INSTALL_INCOMPLETE"),
        ("purge", "RETENTION_PURGE_INCOMPLETE"),
        ("ledger compact", "LEDGER_COMPACT_INCOMPLETE"),
        ("tier age", "TIER_INCOMPLETE"),
        ("plugin sync", "PLUGIN_SYNC_INCOMPLETE"),
        ("inventory sync", "INVENTORY_INCOMPLETE"),
        ("migrate", "MIGRATION_INCOMPLETE"),
    ] {
        assert!(listed(leaf, code), "`canon {leaf}` lists {code}");
    }
    let mut doubled = leaves.clone();
    doubled.push((
        "made up".to_string(),
        vec![
            "ARTIFACTS_MOVE_INCOMPLETE".to_string(),
            "ARTIFACTS_STORE_INCOMPLETE".to_string(),
            "BATCH_INCOMPLETE".to_string(),
        ],
    ));
    let problems = leaf_problems(&doubled);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("made up") && problems[0].contains("one per subject"),
        "{problems:?}"
    );
    let batch_only = vec![(
        "made up".to_string(),
        vec![
            "ARTIFACTS_MOVE_INCOMPLETE".to_string(),
            "BATCH_INCOMPLETE".to_string(),
        ],
    )];
    assert_eq!(
        leaf_problems(&batch_only),
        Vec::<String>::new(),
        "the batch's code is the batch's"
    );
}

// ------------------------------------------------------------------ init

/// atip.midway.03: `canon init` writes canon.yaml, then its last step meets a skill source that `canon skills install` refuses before its work:
/// exit 1, `INIT_INCOMPLETE`, reason `SKILLS_SOURCE_NOT_FOUND`, not retryable, and what was written stays.
#[test]
fn atip_midway_03_init_is_incomplete_when_the_skill_install_meets_a_refusal_after_the_writes() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let repo = pi_repo(dir.path(), "repo");
    let out = canon(&repo, &["init", "--json"], &dir.path().join("missing"));
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let envelope = document(&out);
    assert_eq!(envelope["ok"], false);
    let error = &envelope["error"];
    assert_eq!(
        (
            error["code"].clone(),
            error["reason"].clone(),
            error["retryable"].clone()
        ),
        (
            Value::from("INIT_INCOMPLETE"),
            Value::from("SKILLS_SOURCE_NOT_FOUND"),
            Value::Bool(false)
        ),
        "{envelope}"
    );
    assert!(
        error["message"].as_str().is_some_and(
            |message| message.contains("canon.yaml") && message.contains("did not install")
        ),
        "the message says what was done: {error}"
    );
    assert!(atip::envelope_problems(&envelope).is_empty(), "{envelope}");
    assert!(std::fs::read_to_string(repo.join("canon.yaml"))
        .expect("canon.yaml stays")
        .ends_with("skills:\n  providers: [pi]\n"));
    assert!(
        repo.join(".canon/policy.yaml").is_file() && repo.join("AGENTS.md").is_file(),
        "the other writes stay"
    );
    assert!(!projected(&repo));
    // The text mode says the same code on stderr.
    let again = pi_repo(dir.path(), "again");
    let text = canon(&again, &["init"], &dir.path().join("missing"));
    assert_eq!(text.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&text.stderr).starts_with("canon: INIT_INCOMPLETE: "),
        "{}",
        String::from_utf8_lossy(&text.stderr)
    );
}

/// atip.midway.04: the same step meets a directory it cannot write: that is the I/O category, whose code for the work is `WRITE_FAILED`
/// (spec 8.3): exit 1, no reason, not `INIT_INCOMPLETE`; and `meta.next` still leads with the read.
#[test]
fn atip_midway_04_a_skill_directory_that_cannot_be_written_after_the_writes_is_write_failed() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let source = skill_source(dir.path());
    let repo = pi_repo(dir.path(), "repo");
    let _guard = ReadOnly::new(&repo.join(".pi"));
    let out = canon(&repo, &["init", "--json"], &source);
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let envelope = document(&out);
    assert_eq!(envelope["error"]["code"], "WRITE_FAILED", "{envelope}");
    assert!(envelope["error"].get("reason").is_none(), "{envelope}");
    assert!(
        repo.join("canon.yaml").is_file(),
        "canon.yaml was written before the install met the directory"
    );
    assert!(!projected(&repo));
    assert_eq!(
        first_step_problems(&envelope, "init", &repo, &source),
        Vec::<String>::new(),
        "{envelope}"
    );
}

/// atip.midway.05: a stop that is not a refusal of `canon skills install` is a failure of the skills category and keeps its own code
/// (`SKILLS_FAILED`, exit 1), with no reason; it is not `INIT_INCOMPLETE`.
#[test]
fn atip_midway_05_a_skill_install_failure_that_is_not_a_refusal_keeps_the_skills_failure_code() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let source = skill_source(dir.path());
    let repo = pi_repo(dir.path(), "repo");
    let elsewhere = dir.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).expect("a directory");
    std::os::unix::fs::symlink(&elsewhere, repo.join(".pi/skills"))
        .expect("a symlink at the skills directory");
    let out = canon(&repo, &["init", "--json"], &source);
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let envelope = document(&out);
    assert_eq!(envelope["error"]["code"], "SKILLS_FAILED", "{envelope}");
    assert!(envelope["error"].get("reason").is_none(), "{envelope}");
    assert!(repo.join("canon.yaml").is_file());
    let problems = first_step_problems(&envelope, "init", &repo, &source);
    assert_eq!(
        problems,
        Vec::<String>::new(),
        "the failure after the writes reads what was done first too"
    );
}

/// atip.midway.06: nothing was written, so nothing stopped part-way: `init --dry-run` meets the same refusal and exits 2 with the
/// refusal's own code, never `INIT_INCOMPLETE`, and the repo is as it was.
#[test]
fn atip_midway_06_a_refusal_with_no_effect_is_exit_2_with_the_refusals_own_code() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let repo = pi_repo(dir.path(), "repo");
    let out = canon(
        &repo,
        &["init", "--dry-run", "--json"],
        &dir.path().join("missing"),
    );
    assert_eq!(
        out.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let envelope = document(&out);
    assert_eq!(
        envelope["error"]["code"], "SKILLS_SOURCE_NOT_FOUND",
        "{envelope}"
    );
    assert!(envelope["error"].get("reason").is_none());
    assert!(
        !repo.join("canon.yaml").exists(),
        "a preview writes nothing"
    );
    assert_eq!(
        std::fs::read_dir(&repo).expect("the repo").count(),
        1,
        "only the .pi directory the test made"
    );
    // And a refusal before any step is exit 2 too: an unknown provider names the refusal and writes nothing.
    let unknown = pi_repo(dir.path(), "unknown");
    let refused = canon(
        &unknown,
        &["init", "--providers", "gemini", "--json"],
        &dir.path().join("missing"),
    );
    assert_eq!(refused.status.code(), Some(2));
    assert_eq!(document(&refused)["error"]["code"], "INIT_ARGUMENT_INVALID");
    assert!(!unknown.join("canon.yaml").exists());
}

/// atip.midway.07: the failure's `meta.next` is not empty, its first step reads (a `read` command, never `init`), and running it shows
/// the repo `init` wrote; the repeat comes after it. A first step that repeats the write, or no step, fails the check.
#[test]
fn atip_midway_07_the_first_next_step_of_a_stopped_command_reads_what_was_done() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let source = skill_source(dir.path());
    let repo = pi_repo(dir.path(), "repo");
    let out = canon(&repo, &["init", "--json"], &dir.path().join("missing"));
    let envelope = document(&out);
    assert_eq!(
        first_step_problems(&envelope, "init", &repo, &source),
        Vec::<String>::new(),
        "{envelope}"
    );
    let next = envelope["meta"]["next"].as_array().expect("next");
    assert_eq!(next[0]["argv"], serde_json::json!(["status", "--json"]));
    assert_eq!(
        next[1]["argv"],
        serde_json::json!(["skills", "install", "--json"]),
        "the repeat of the install comes after the read"
    );
    assert_ne!(next[0]["argv"][0], "init");
    let read = canon(&repo, &["status", "--json"], &source);
    assert_eq!(
        read.status.code(),
        Some(0),
        "the first step runs: {}",
        String::from_utf8_lossy(&read.stderr)
    );
    assert_eq!(document(&read)["ok"], true);

    // The checks themselves fail on a bad envelope.
    let without_next =
        serde_json::json!({"ok": false, "error": {"code": "INIT_INCOMPLETE"}, "meta": {}});
    assert_eq!(
        first_step_problems(&without_next, "init", &repo, &source),
        ["meta.next is missing or empty"]
    );
    let empty = serde_json::json!({"ok": false, "meta": {"next": []}});
    assert_eq!(
        first_step_problems(&empty, "init", &repo, &source),
        ["meta.next is missing or empty"]
    );
    let repeats = serde_json::json!({"ok": false, "meta": {"next": [{"command": "canon init --json", "argv": ["init", "--json"], "reason": "again"}]}});
    let problems = first_step_problems(&repeats, "init", &repo, &source);
    assert!(
        problems
            .iter()
            .any(|problem| problem.contains("repeats the command"))
            && problems.iter().any(|problem| problem.contains("not read")),
        "{problems:?}"
    );
    let writes = serde_json::json!({"ok": false, "meta": {"next": [{"command": "canon skills install --json", "argv": ["skills", "install", "--json"], "reason": "install"}]}});
    assert!(first_step_problems(&writes, "init", &repo, &source)
        .iter()
        .any(|problem| problem.contains("not read")));
}

/// atip.midway.08: in a batch, an element that stops part-way is `failed` with its `_INCOMPLETE` code and its reason as its own error (the item
/// error, with the element's `hint`), the elements after it are `skipped`, the batch is `BATCH_INCOMPLETE` and what the element wrote stays
/// (spec 8.3.2, 11.11). The same holds in the unit test of `input.rs` that reads a child's envelope.
#[test]
fn atip_midway_08_a_batch_element_that_stops_part_way_is_failed_with_its_reason_and_the_batch_is_incomplete(
) {
    let dir = tempfile::tempdir().expect("a temp dir");
    let repo = pi_repo(dir.path(), "repo");
    let input =
        serde_json::json!([{"providers": "pi"}, {"providers": "pi", "language": "en"}]).to_string();
    let out = canon(
        &repo,
        &["init", "--input", &input, "--json"],
        &dir.path().join("missing"),
    );
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let envelope = document(&out);
    assert_eq!(envelope["error"]["code"], "BATCH_INCOMPLETE", "{envelope}");
    let items = envelope["data"]["items"].as_array().expect("items");
    assert_eq!(items[0]["status"], "failed", "{envelope}");
    assert_eq!(
        (
            items[0]["error"]["code"].clone(),
            items[0]["error"]["reason"].clone()
        ),
        (
            Value::from("INIT_INCOMPLETE"),
            Value::from("SKILLS_SOURCE_NOT_FOUND")
        ),
        "{envelope}"
    );
    assert_eq!(items[1]["status"], "skipped", "{envelope}");
    assert!(
        repo.join("canon.yaml").is_file(),
        "the element's effects stay"
    );
}

// ------------------------------------------------------------------ the audit

/// What the audit decided about an exit-1 code (spec 8.3.2, the brief's F1): the stop that left its effects is `INCOMPLETE`, the one whose work failed
/// whole is `FAILED`, and the rest are an answer, a bug of canon, or the batch's own code.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Class {
    /// The command ran, stopped with work left undone, and what it did stays done: the name ends in `_INCOMPLETE`.
    Incomplete,
    /// The command ran and one work failed whole, or everything it did was rolled back: the name does not end in `_INCOMPLETE`.
    Failed,
    /// The command ran to the end and its answer is negative: a finding, a verdict, a failed check.
    Answer,
    /// A bug of canon, whatever the command line was.
    Internal,
    /// The batch's own core code, which carries its items' errors (spec 11.11).
    Batch,
}

/// The audit of every exit-1 code of the registry, each with the line that says what the call site leaves behind (`canon-0-36-codes-incomplete`).
const AUDIT: &[(&str, Class, &str)] = &[
    ("ADAPTER_AUTHORIZATION_FAILED", Class::Answer, "`adapter authorize` answered that the response does not authorize execution"),
    ("ADAPTER_VALIDATION_FAILED", Class::Answer, "`adapter validate`'s negative answer on an envelope"),
    ("ARTIFACTS_MOVE_INCOMPLETE", Class::Incomplete, "`artifacts move --commit` uploaded blobs, then a later blob changed, could not be read or the store stopped answering: the uploads stay, nothing was removed, canon.yaml is unchanged"),
    ("ARTIFACTS_STORE_FAILED", Class::Failed, "a blob write or upload failed and nothing was kept: `evidence add` rolled the record or batch back (the code `EVIDENCE_STORE_FAILED` merged into), `artifacts store` and `artifacts move` had stored nothing yet"),
    ("ARTIFACTS_STORE_INCOMPLETE", Class::Incomplete, "`artifacts store` stored some blobs, then a later write or check of the store failed: the stored blobs stay and a repeat of the same call stores the rest"),
    ("ARTIFACTS_VERIFY_FAILED", Class::Answer, "`artifacts verify` found a bound file missing or altered"),
    ("BATCH_INCOMPLETE", Class::Batch, "the batch's own core code; an element that stops part-way is its item error"),
    ("BRIEF_CHECK_FAILED", Class::Answer, "`brief check`'s findings"),
    ("BRIEF_WRITE_FAILED", Class::Failed, "the brief file could not be written: one write, nothing of it kept"),
    ("CHANGE_ADOPTION_INCOMPLETE", Class::Incomplete, "the second record's store write failed after the first was written: the first stays and a repair command finishes the link"),
    ("CONFIG_CHECK_FAILED", Class::Answer, "`init --check` found a section that fails its loader"),
    ("CONTEXT_PACK_VERIFY_FAILED", Class::Answer, "`context-pack verify` found the pack changed: the check's negative answer"),
    ("DASHBOARD_FAILED", Class::Failed, "the dashboard could not start: nothing was written"),
    ("DIVERGENCE_INCOMPLETE", Class::Incomplete, "`divergence add` staged the record, then the gate refused to promote it: the record stays staged"),
    ("EVIDENCE_INCOMPLETE", Class::Incomplete, "`evidence add` and `evidence audit` staged the record, then the gate refused to promote it: the record stays staged"),
    ("FINDING_INCOMPLETE", Class::Incomplete, "`finding add` and `finding close` staged the record, then the gate refused to promote it: the record stays staged"),
    ("FORMAT_CHECK_FAILED", Class::Answer, "`format`'s findings"),
    ("GATE_CHECK_FAILED", Class::Answer, "`gate check`'s findings"),
    ("HOOKS_INSTALL_INCOMPLETE", Class::Incomplete, "`gate install-hooks` wrote the first hook file, then could not write the second: the first stays; the first file failing is `WRITE_FAILED`, one write that kept nothing"),
    ("INGEST_FAILED", Class::Failed, "an ingest read or write failed; the importer writes a record whole or not at all"),
    ("INIT_INCOMPLETE", Class::Incomplete, "`init` wrote canon.yaml and the files beside it, then the skill install met what `canon skills install` refuses: what init wrote stays"),
    ("INTERNAL", Class::Internal, "a bug of canon"),
    ("INVENTORY_FAILED", Class::Failed, "the record store failed while the index was read or written"),
    ("INVENTORY_INCOMPLETE", Class::Incomplete, "`inventory sync` left a spec root unindexed on violations or duplicate scenarios: the roots before it stay indexed and `data.roots` says per root; a preview reports the same code with nothing written, as the answer of the sync it previews"),
    ("LEDGER_COMPACT_INCOMPLETE", Class::Incomplete, "a version could not be moved to the archive, so the compaction stopped with versions left in the ledger: what moved stays moved"),
    ("MIGRATION_INCOMPLETE", Class::Incomplete, "judged: every site is a write, move, remove or re-import that stopped after `migrate` began changing the repo (reason STOPPED), the index rebuild after the files moved (INDEX) or the review pins after the index (PINS)"),
    ("PLUGIN_SYNC_INCOMPLETE", Class::Incomplete, "`plugin sync` wrote some candidates and could not write others: the written ones stay and `data.roots` names each error"),
    ("POLICY_APPROVAL_FAILED", Class::Failed, "the approval file could not be written: one write, nothing of it kept"),
    ("PROMOTION_FAILED", Class::Failed, "judged: `gate promote` (or the promotion of a record a command just staged) failed in the ledger, nothing reached the ledger and the staged record stays staged, which is the state the ordinary flow takes (`gate promote` again): no work is left half-done"),
    ("QUERY_FAILED", Class::Failed, "the ledger could not be read"),
    ("REPORT_CHECK_FAILED", Class::Answer, "`report check` found the report out of date"),
    ("REPORT_FAILED", Class::Failed, "the report could not be built or checked"),
    ("RETENTION_EXPORT_FAILED", Class::Failed, "the export's write failed"),
    ("RETENTION_PURGE_INCOMPLETE", Class::Incomplete, "the purge stopped after it began deleting: what was deleted stays deleted and `data` names the counts"),
    ("SEARCH_INDEX_FAILED", Class::Failed, "the records or the index could not be read or built"),
    ("SELFTEST_FAILED", Class::Answer, "a selftest suite did not match its expected result"),
    ("SERIALIZE_FAILED", Class::Internal, "a result that cannot be serialized is a bug of canon"),
    ("SKILLS_CHECK_FAILED", Class::Answer, "`skills check`'s finding"),
    ("SKILLS_FAILED", Class::Failed, "a skills command's own I/O failure; `init` says it too when the install fails and `canon skills install` would not have refused it (a failure of the one install, which the skills command projects whole)"),
    ("STRATEGY_DEMOTION_FAILED", Class::Failed, "the demotion's write failed and the strategy is still active"),
    ("STRATEGY_INCOMPLETE", Class::Incomplete, "`strategy promote` demoted a strategy, then the promotion gate blocked the promotion: the strategy stays demoted"),
    ("STRATEGY_PROMOTION_FAILED", Class::Failed, "judged at both sites: the decision could not be read (nothing was written) or the strategy file could not be written (the one write fails whole, and an activation removes its file again when the store update fails)"),
    ("STRATEGY_ROLLBACK_FAILED", Class::Failed, "the rollback could not be recorded: one record, nothing of it kept"),
    ("TIER_FAILED", Class::Failed, "tier aging failed before it moved any record: the plan's moves are all still there when it is counted again"),
    ("TIER_INCOMPLETE", Class::Incomplete, "tier aging stopped after records left their tier: what moved stays moved"),
    ("UNVERIFIABLE", Class::Answer, "a check that examined nothing"),
    ("WRITE_FAILED", Class::Failed, "a write began and failed: one write, nothing of it kept"),
];

/// The names of the sources of canon, outside the registry and the table derived from it.
fn source_text() -> String {
    fn walk(dir: &Path, text: &mut String) {
        for entry in std::fs::read_dir(dir).expect("a source directory") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                walk(&path, text);
            } else if path.extension().is_some_and(|extension| extension == "rs")
                && !path.ends_with("cli_error.rs")
                && !path.ends_with("schema_codes.rs")
            {
                text.push_str(&std::fs::read_to_string(&path).expect("a source file"));
            }
        }
    }
    let mut text = String::new();
    walk(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut text,
    );
    text
}

/// What is wrong between `audit`, the registry `rows` and the `sources`: every exit-1 code of the registry has a line, every audited code is registered at
/// exit 1 and built by a source, a code that ends in `_INCOMPLETE` is classed `Incomplete` (or the batch's own) and no other is, and the codes in `gone`
/// are in neither the registry nor a source.
fn audit_problems(
    rows: &[CodeInfo],
    audit: &[(&str, Class, &str)],
    gone: &[&str],
    sources: &str,
) -> Vec<String> {
    let mut problems = Vec::new();
    let audited: BTreeSet<&str> = audit.iter().map(|(code, _, _)| *code).collect();
    if audited.len() != audit.len() {
        problems.push("a code is audited twice".to_string());
    }
    let literal = |code: &str| sources.contains(&format!("\"{code}\""));
    for row in rows.iter().filter(|row| row.exit == EXIT_FAILED) {
        if !audited.contains(row.code) {
            problems.push(format!(
                "{} is an exit-1 code and is not in the audit",
                row.code
            ));
        }
    }
    for (code, class, why) in audit {
        if why.is_empty() {
            problems.push(format!("{code} has no line that says why"));
        }
        match rows.iter().find(|row| row.code == *code) {
            None => problems.push(format!("{code} is audited and is not in the registry")),
            Some(row) if row.exit != EXIT_FAILED => problems.push(format!(
                "{code} is audited as an exit-1 code and the registry has exit {}",
                row.exit
            )),
            Some(_) if !literal(code) => {
                problems.push(format!("{code} is audited and no source builds it"));
            }
            Some(_) => {}
        }
        let ends_incomplete = code.ends_with("_INCOMPLETE");
        match class {
            Class::Incomplete if !ends_incomplete => problems.push(format!(
                "{code} is INCOMPLETE and its name does not end in _INCOMPLETE"
            )),
            Class::Batch if *code != "BATCH_INCOMPLETE" => {
                problems.push(format!("{code} is classed as the batch's code"));
            }
            Class::Failed | Class::Answer | Class::Internal if ends_incomplete => problems.push(
                format!("{code} ends in _INCOMPLETE and is not classed INCOMPLETE"),
            ),
            _ => {}
        }
    }
    for code in gone {
        if rows.iter().any(|row| row.code == *code) {
            problems.push(format!("{code} is gone and is still in the registry"));
        }
        if literal(code) {
            problems.push(format!("{code} is gone and a source still builds it"));
        }
    }
    problems
}

/// atip.midway.09: the audit lists every exit-1 code of the registry in its class (INCOMPLETE, FAILED, answer, internal, batch), each audited code is
/// registered and built by a source, each `_INCOMPLETE` code is classed so and no other is, and the replaced codes are gone from both.
#[test]
fn atip_midway_09_the_audit_covers_every_exit_1_code_and_each_code_exists_where_the_table_says() {
    let sources = source_text();
    assert_eq!(
        audit_problems(all_codes(), AUDIT, GONE, &sources),
        Vec::<String>::new()
    );
    let incomplete: Vec<&str> = AUDIT
        .iter()
        .filter(|(_, class, _)| *class == Class::Incomplete)
        .map(|(code, _, _)| *code)
        .collect();
    let table: Vec<&str> = INCOMPLETE.iter().map(|(code, _, _)| *code).collect();
    assert_eq!(
        incomplete, table,
        "the audit and the registry table name the same _INCOMPLETE codes"
    );
    // Every call site the brief names was judged.
    for candidate in [
        "MIGRATION_INCOMPLETE",
        "CHANGE_ADOPTION_INCOMPLETE",
        "PROMOTION_FAILED",
        "STRATEGY_PROMOTION_FAILED",
        "INVENTORY_INCOMPLETE",
    ] {
        assert!(
            AUDIT.iter().any(|(code, _, why)| *code == candidate
                && (why.starts_with("judged")
                    || *code == "CHANGE_ADOPTION_INCOMPLETE"
                    || *code == "INVENTORY_INCOMPLETE")),
            "{candidate} is judged in the table"
        );
    }
    assert_audit_faults_are_named(&sources);
}

/// atip.midway.14 (the failure path of F1): an exit-1 code the table lacks, an audited code no source builds, a replaced code that comes back in the
/// registry or in a source, an `_INCOMPLETE` code classed as another class and another class's code named `_INCOMPLETE` each fail the audit check
/// with their own message.
#[test]
fn atip_midway_14_an_audit_that_misses_a_code_or_a_source_or_a_replaced_code_comes_back_fails_the_check(
) {
    assert_audit_faults_are_named(&source_text());
}

/// The faults the audit check must name, each with its own message; the audit as it stands passes (atip.midway.09 and .14).
fn assert_audit_faults_are_named(sources: &str) {
    // An exit-1 code the table lacks fails.
    let extra = CodeInfo {
        code: "SOMETHING_NEW_FAILED",
        meaning: "m",
        exit: 1,
        retryable: false,
        fix: "f",
        reasons: &[],
    };
    let mut grown: Vec<CodeInfo> = all_codes().to_vec();
    grown.push(extra);
    assert!(audit_problems(&grown, AUDIT, GONE, sources)
        .iter()
        .any(|problem| problem.contains("SOMETHING_NEW_FAILED")
            && problem.contains("not in the audit")));
    // An audited code that no source builds fails.
    assert!(audit_problems(all_codes(), AUDIT, GONE, "")
        .iter()
        .any(|problem| problem.contains("no source builds it")));
    // An audited code the registry lost fails.
    let lost: Vec<CodeInfo> = all_codes()
        .iter()
        .filter(|row| row.code != "TIER_INCOMPLETE")
        .copied()
        .collect();
    assert!(audit_problems(&lost, AUDIT, GONE, sources)
        .iter()
        .any(|problem| problem.contains("TIER_INCOMPLETE")
            && problem.contains("not in the registry")));
    // A replaced code that comes back, in the registry or in a source, fails.
    let mut back: Vec<CodeInfo> = all_codes().to_vec();
    back.push(CodeInfo {
        code: "MIGRATE_STOPPED",
        meaning: "m",
        exit: 1,
        retryable: false,
        fix: "f",
        reasons: &[],
    });
    assert!(audit_problems(&back, AUDIT, GONE, sources)
        .iter()
        .any(|problem| problem.contains("MIGRATE_STOPPED")
            && problem.contains("still in the registry")));
    let revived = format!("{sources}\nCliError::refused(\"STOPPED_MIDWAY\", message)");
    assert!(audit_problems(all_codes(), AUDIT, GONE, &revived)
        .iter()
        .any(|problem| problem.contains("STOPPED_MIDWAY")
            && problem.contains("a source still builds it")));
    // A code moved to exit 2 fails.
    let moved_to_refusal: Vec<CodeInfo> = all_codes()
        .iter()
        .map(|row| {
            if row.code == "WRITE_FAILED" {
                CodeInfo { exit: 2, ..*row }
            } else {
                *row
            }
        })
        .collect();
    assert!(audit_problems(&moved_to_refusal, AUDIT, GONE, sources)
        .iter()
        .any(|problem| problem.contains("WRITE_FAILED") && problem.contains("exit 2")));
    // The class and the name must agree.
    let misclassed: Vec<(&str, Class, &str)> = AUDIT
        .iter()
        .map(|(code, class, why)| {
            if *code == "HOOKS_INSTALL_INCOMPLETE" {
                (*code, Class::Failed, *why)
            } else {
                (*code, *class, *why)
            }
        })
        .collect();
    assert!(audit_problems(all_codes(), &misclassed, GONE, sources)
        .iter()
        .any(|problem| problem.contains("HOOKS_INSTALL_INCOMPLETE")
            && problem.contains("not classed INCOMPLETE")));
    let renamed: Vec<(&str, Class, &str)> = AUDIT
        .iter()
        .map(|(code, class, why)| {
            if *code == "TIER_FAILED" {
                (*code, Class::Incomplete, *why)
            } else {
                (*code, *class, *why)
            }
        })
        .collect();
    assert!(audit_problems(all_codes(), &renamed, GONE, sources)
        .iter()
        .any(|problem| problem.contains("TIER_FAILED")
            && problem.contains("name does not end in _INCOMPLETE")));
    // The audit as it stands passes.
    assert_eq!(
        audit_problems(all_codes(), AUDIT, GONE, sources),
        Vec::<String>::new()
    );
}

// ------------------------------------------------------------------ the I/O failure codes

/// The failure codes of the work that keep their own name (spec 8.3: a category with a during-the-work code keeps it): the code and whether it is
/// retryable.
const IO_CODES: [(&str, bool); 8] = [
    ("ARTIFACTS_STORE_FAILED", true),
    ("BRIEF_WRITE_FAILED", false),
    ("INVENTORY_FAILED", false),
    ("POLICY_APPROVAL_FAILED", false),
    ("PROMOTION_FAILED", false),
    ("RETENTION_EXPORT_FAILED", false),
    ("TIER_FAILED", false),
    ("WRITE_FAILED", false),
];

/// What is wrong with the failure codes of `rows`: each is registered at exit 1 with its retryability and has no reason of its own. An `_INCOMPLETE`
/// code with no refusal to name carries the code of the step that failed as its reason (`WRITE_FAILED`), so a failure code may be one.
fn io_problems(rows: &[CodeInfo]) -> Vec<String> {
    let mut problems = Vec::new();
    for (code, retryable) in IO_CODES {
        let Some(row) = rows.iter().find(|row| row.code == code) else {
            problems.push(format!("{code} is not registered"));
            continue;
        };
        if (row.exit, row.retryable) != (EXIT_FAILED, retryable) {
            problems.push(format!(
                "{code} is exit {} retryable {}, not exit 1 retryable {retryable}",
                row.exit, row.retryable
            ));
        }
        if !row.reasons.is_empty() {
            problems.push(format!("{code} has a reason of its own"));
        }
    }
    problems
}

/// atip.midway.10: the failure codes of the work keep their names, their exit 1 and their retryability, and none has a reason of its own; every
/// reason of an `_INCOMPLETE` code is a registered code (the refusal it met, or the failure of the step that stopped).
#[test]
fn atip_midway_10_the_io_failure_codes_keep_their_names_exits_and_retryability() {
    let problems = io_problems(all_codes());
    assert!(problems.is_empty(), "{problems:?}");
    for (code, _) in IO_CODES {
        assert!(
            AUDIT
                .iter()
                .any(|(audited, class, _)| *audited == code && *class == Class::Failed),
            "{code} is a FAILED code in the audit"
        );
        assert_eq!(code_info(code).expect("registered").exit, 1, "{code}");
    }
    // `EVIDENCE_STORE_FAILED` merged into `ARTIFACTS_STORE_FAILED`: one code, retryable, rolled back.
    assert!(code_info("EVIDENCE_STORE_FAILED").is_none());
    assert!(
        code_info("ARTIFACTS_STORE_FAILED")
            .expect("registered")
            .retryable
    );
}

/// atip.midway.13 (the failure path of F5): a failure code of the work that is renamed away, moved to a refusal's exit, made retryable or not, given a
/// reason, or made a reason of an `_INCOMPLETE` code fails the check of the failure codes, each with its own message; and a registry that holds them as
/// it does passes it.
#[test]
fn atip_midway_13_an_io_failure_code_that_moves_or_is_absorbed_fails_the_check() {
    static A_REASON: [ReasonInfo; 1] = [ReasonInfo {
        reason: "SKILLS_SOURCE_NOT_FOUND",
        meaning: "m",
        fix: "f",
    }];
    static WRITE_FAILED_AS_REASON: [ReasonInfo; 1] = [ReasonInfo {
        reason: "WRITE_FAILED",
        meaning: "m",
        fix: "f",
    }];
    let changed = |code: &str, with: &dyn Fn(&CodeInfo) -> CodeInfo| -> Vec<CodeInfo> {
        all_codes()
            .iter()
            .map(|row| if row.code == code { with(row) } else { *row })
            .collect()
    };
    assert!(
        io_problems(all_codes()).is_empty(),
        "the registry as it stands holds the failure codes as the audit says"
    );

    let absent: Vec<CodeInfo> = all_codes()
        .iter()
        .filter(|row| row.code != "WRITE_FAILED")
        .copied()
        .collect();
    assert_eq!(io_problems(&absent), ["WRITE_FAILED is not registered"]);
    let refusal = changed("TIER_FAILED", &|row| CodeInfo {
        exit: EXIT_REFUSED,
        ..*row
    });
    assert_eq!(
        io_problems(&refusal),
        ["TIER_FAILED is exit 2 retryable false, not exit 1 retryable false"]
    );
    let retryable = changed("WRITE_FAILED", &|row| CodeInfo {
        retryable: true,
        ..*row
    });
    assert_eq!(
        io_problems(&retryable),
        ["WRITE_FAILED is exit 1 retryable true, not exit 1 retryable false"]
    );
    let not_retryable = changed("ARTIFACTS_STORE_FAILED", &|row| CodeInfo {
        retryable: false,
        ..*row
    });
    assert_eq!(
        io_problems(&not_retryable),
        ["ARTIFACTS_STORE_FAILED is exit 1 retryable false, not exit 1 retryable true"]
    );
    let reasoned = changed("INVENTORY_FAILED", &|row| CodeInfo {
        reasons: &A_REASON,
        ..*row
    });
    assert_eq!(
        io_problems(&reasoned),
        ["INVENTORY_FAILED has a reason of its own"]
    );
    // A failure code is the reason of a stop that has no refusal to name: that is no fault of the failure code.
    let as_reason = changed("TIER_INCOMPLETE", &|row| CodeInfo {
        reasons: &WRITE_FAILED_AS_REASON,
        ..*row
    });
    assert!(io_problems(&as_reason).is_empty());
}

/// atip.midway.12: the adoption writes the subject record, then the change record. When the second store write fails (the change store
/// is read-only, which no up-front check can see in an object store), the failure keeps `CHANGE_ADOPTION_INCOMPLETE` (exit 1, reason `WRITE_FAILED`, one code
/// per subject): the first record stays and the repair `canon subject adopt` completes it.
#[cfg(unix)]
#[test]
fn atip_midway_12_a_half_written_adoption_keeps_change_adoption_incomplete_and_its_repair_completes_the_link(
) {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("a temp dir");
    for args in [
        &["init", "-q", "-b", "main"][..],
        &["config", "user.name", "t"],
        &["config", "user.email", "t@example.com"],
        &["commit", "-q", "--allow-empty", "-m", "x"],
    ] {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir.path())
            .output()
            .expect("spawn git");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let r2 = dir.path().join("r2-local");
    std::fs::write(
        dir.path().join("canon.yaml"),
        "tiers:\n  local: { backend: git, root: .canon/ledger }\n  cold: { backend: s3, bucket_env: CANON_R2_BUCKET_TEST_UNUSED, prefix: \"canon/\" }\nrouting:\n  subject: local\n  change: cold\n  scenario: local\n  evidence: local\n",
    )
    .expect("canon.yaml");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_canon"))
            .args(args)
            .current_dir(dir.path())
            .env_remove("CANON_ROLE")
            .env("CANON_R2_LOCAL_ROOT", &r2)
            .output()
            .expect("spawn canon")
    };
    assert!(run(&[
        "subject",
        "new",
        "demo-subject",
        "--domain",
        "dev",
        "--title",
        "Demo"
    ])
    .status
    .success());
    assert!(
        run(&["subject", "new", "other", "--domain", "dev", "--title", "Other"])
            .status
            .success()
    );
    assert!(run(&[
        "change",
        "new",
        "s36-demo",
        "--subject",
        "other",
        "--title",
        "Demo"
    ])
    .status
    .success());
    let stored_changes = r2.join("canon/kind=change");
    std::fs::set_permissions(&stored_changes, std::fs::Permissions::from_mode(0o555))
        .expect("make the change store read-only");
    if std::fs::write(stored_changes.join("probe"), "").is_ok() {
        // Root ignores directory permissions; there is no failure to force.
        std::fs::remove_file(stored_changes.join("probe")).expect("remove the probe");
        std::fs::set_permissions(&stored_changes, std::fs::Permissions::from_mode(0o755))
            .expect("restore");
        return;
    }
    let out = run(&[
        "subject",
        "adopt",
        "demo-subject",
        "--change",
        "s36-demo",
        "--json",
    ]);
    std::fs::set_permissions(&stored_changes, std::fs::Permissions::from_mode(0o755))
        .expect("restore");
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let envelope = document(&out);
    assert_eq!(
        envelope["error"]["code"], "CHANGE_ADOPTION_INCOMPLETE",
        "{envelope}"
    );
    assert_eq!(envelope["error"]["reason"], "WRITE_FAILED", "{envelope}");
    assert!(envelope["error"]["hint"].as_str().is_some_and(|hint| hint.contains("canon subject adopt demo-subject --change s36-demo")), "{envelope}");
    assert_eq!(
        code_info("CHANGE_ADOPTION_INCOMPLETE").map(|row| (row.exit, row.retryable)),
        Some((1, false))
    );
    let repair = run(&["subject", "adopt", "demo-subject", "--change", "s36-demo"]);
    assert_eq!(
        repair.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&repair.stderr)
    );
    let again = run(&["subject", "adopt", "demo-subject", "--change", "s36-demo"]);
    assert!(
        String::from_utf8_lossy(&again.stdout).contains("nothing written"),
        "{}",
        String::from_utf8_lossy(&again.stdout)
    );
}

// ------------------------------------------------------------------ the record the gate refuses after it was staged

/// A git repository with canon, one indexed scenario `world.a.01` and everything committed.
fn repo_with_scenario() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temp dir");
    let repo = dir.path();
    git(repo, &["init", "-q", "-b", "main"]);
    git(repo, &["commit", "-q", "--allow-empty", "-m", "seed"]);
    for args in [
        &["init", "--providers", "none"][..],
        &["feature", "new", "world.a", "--title", "A"],
        &["scenario", "new", "world.a.01", "--title", "one"],
        &["inventory", "sync"],
    ] {
        let out = plain(repo, args);
        assert!(out.status.success(), "canon {args:?}: {}", text(&out));
    }
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "a scenario"]);
    dir
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn staged(repo: &Path, kind: &str) -> Vec<String> {
    files_in(
        &repo
            .join(".canon/ledger/_staging")
            .join(format!("kind={kind}")),
    )
}

fn committed(repo: &Path, kind: &str) -> Vec<String> {
    files_in(&repo.join(".canon/ledger").join(format!("kind={kind}")))
}

/// atip.incomplete.01: a record a command staged and the gate then refuses to promote (the staged file was damaged between the two, the debug seam
/// `promote-staged-corrupt`) is its subject's `_INCOMPLETE` code (`EVIDENCE_INCOMPLETE` for `evidence add`, `FINDING_INCOMPLETE` for `finding add` and
/// `finding close`, `DIVERGENCE_INCOMPLETE` for `divergence add`): exit 1, not retryable, reason `PROMOTION_INVALID`, no `data`, the record stays staged
/// and nothing is committed, and `meta.next` leads with the read of what is staged, never with the repeat of the promotion. A batch element that stops
/// so is `failed` with that code and the batch is `BATCH_INCOMPLETE`. The same call without the damage passes, and `gate promote` of the damaged records
/// refuses them at exit 2 with the gate's code and no `data`.
#[test]
fn atip_incomplete_01_a_record_the_gate_refuses_after_it_was_staged_is_its_subjects_incomplete_code(
) {
    let dir = repo_with_scenario();
    let repo = dir.path();
    let seam = [("CANON_TEST_STOP_AT", "promote-staged-corrupt")];
    let run = |args: &[&str], role: &str, env: &[(&str, &str)]| {
        let mut with_role = vec![("CANON_ROLE", role)];
        with_role.extend_from_slice(env);
        canon_env(repo, args, None, &with_role)
    };
    // A committed finding for `finding close` to move, made without the damage.
    let seeded = run(
        &[
            "finding",
            "add",
            "--change",
            "c2",
            "--round",
            "1",
            "--seq",
            "1",
            "--severity",
            "note",
            "--actor",
            "a1",
            "--summary",
            "seeded",
            "--promote",
            "--json",
        ],
        "reviewer",
        &[],
    );
    assert_eq!(seeded.status.code(), Some(0), "{}", text(&seeded));
    assert_eq!(committed(repo, "finding").len(), 1);

    let cases: [(&str, &[&str], &str, &str, &str); 4] = [
        (
            "finding add",
            &[
                "finding",
                "add",
                "--change",
                "c1",
                "--round",
                "1",
                "--seq",
                "1",
                "--severity",
                "note",
                "--actor",
                "a1",
                "--summary",
                "s",
                "--promote",
                "--json",
            ],
            "reviewer",
            "finding",
            "FINDING_INCOMPLETE",
        ),
        (
            "finding close",
            &[
                "finding",
                "close",
                "c2",
                "--round",
                "1",
                "--seq",
                "1",
                "--disposition",
                "rejected",
                "--reason",
                "no",
                "--actor",
                "a1",
                "--promote",
                "--json",
            ],
            "reviewer",
            "finding",
            "FINDING_INCOMPLETE",
        ),
        (
            "evidence add",
            &[
                "evidence",
                "add",
                "--scenario",
                "world.a.01",
                "--class",
                "test-run",
                "--ref",
                "x",
                "--actor",
                "a1",
                "--role",
                "implementer",
                "--promote",
                "--json",
            ],
            "implementer",
            "evidence",
            "EVIDENCE_INCOMPLETE",
        ),
        (
            "divergence add",
            &[
                "divergence",
                "add",
                "world.a.01",
                "--actor",
                "a1",
                "--role",
                "reviewer",
                "--promote",
                "--json",
            ],
            "reviewer",
            "divergence",
            "DIVERGENCE_INCOMPLETE",
        ),
    ];
    let mut staged_before = 0;
    for (leaf, args, role, kind, code) in cases {
        let committed_before = committed(repo, kind);
        let out = run(args, role, &seam);
        let body = document(&out);
        assert_eq!(out.status.code(), Some(1), "{leaf}: {body}");
        let error = &body["error"];
        assert_eq!(
            (
                error["code"].as_str(),
                error["reason"].as_str(),
                error["retryable"].as_bool()
            ),
            (Some(code), Some("PROMOTION_INVALID"), Some(false)),
            "{leaf}: {body}"
        );
        assert!(
            body.get("data").is_none(),
            "{leaf}: the stop carries no data: {body}"
        );
        assert!(atip::envelope_problems(&body).is_empty(), "{leaf}: {body}");
        assert_eq!(
            body["meta"]["next"][0]["argv"],
            serde_json::json!(["gate", "promote", "--dry-run", "--json"]),
            "{leaf}: the first step reads what is staged: {body}"
        );
        assert!(
            error["hint"]
                .as_str()
                .is_some_and(|hint| hint.contains("still staged")),
            "{leaf}: {body}"
        );
        assert_eq!(
            committed(repo, kind),
            committed_before,
            "{leaf}: nothing was committed"
        );
        assert!(
            !staged(repo, kind).is_empty(),
            "{leaf}: the record stays staged"
        );
        staged_before += 1;
        assert!(staged(repo, kind).len() <= staged_before, "{leaf}");
    }

    // A batch element that stops so is failed with its own code and reason; the next is skipped; the batch is incomplete.
    let batch = serde_json::json!([
        {"change": "c3", "round": 1, "seq": 1, "severity": "note", "actor": "a1", "summary": "one", "promote": true},
        {"change": "c3", "round": 1, "seq": 2, "severity": "note", "actor": "a1", "summary": "two", "promote": true}
    ])
    .to_string();
    let out = run(
        &["finding", "add", "--input", &batch, "--json"],
        "reviewer",
        &seam,
    );
    let body = document(&out);
    assert_eq!(out.status.code(), Some(1), "{body}");
    assert_eq!(body["error"]["code"], "BATCH_INCOMPLETE", "{body}");
    let items = body["data"]["items"].as_array().expect("items");
    assert_eq!(
        (
            items[0]["status"].as_str(),
            items[0]["error"]["code"].as_str(),
            items[0]["error"]["reason"].as_str()
        ),
        (
            Some("failed"),
            Some("FINDING_INCOMPLETE"),
            Some("PROMOTION_INVALID")
        ),
        "{body}"
    );
    assert_eq!(items[1]["status"], "skipped", "{body}");

    // The same refusal with nothing staged is not an `_INCOMPLETE` code: `gate promote` of the damaged records refuses them at exit 2, no data.
    let promote = plain(repo, &["gate", "promote", "--json"]);
    let refused = document(&promote);
    assert_eq!(promote.status.code(), Some(2), "{refused}");
    assert_eq!(refused["error"]["code"], "PROMOTION_INVALID", "{refused}");
    assert!(refused.get("data").is_none(), "{refused}");
    assert!(
        !staged(repo, "finding").is_empty()
            && !staged(repo, "evidence").is_empty()
            && !staged(repo, "divergence").is_empty(),
        "the refused records stay staged"
    );

    // Without the damage the same call passes and commits.
    let fresh = run(
        &[
            "finding",
            "add",
            "--change",
            "c9",
            "--round",
            "1",
            "--seq",
            "1",
            "--severity",
            "note",
            "--actor",
            "a1",
            "--summary",
            "fine",
            "--promote",
            "--json",
        ],
        "reviewer",
        &[],
    );
    assert_eq!(fresh.status.code(), Some(0), "{}", text(&fresh));
    assert_eq!(committed(repo, "finding").len(), 2);
}

// ------------------------------------------------------------------ artifacts store

/// atip.incomplete.03: `artifacts store` stores the blobs the records bind; when a later write of the store fails after one blob was stored (the debug
/// seam `artifacts-store-second`), the stored blob stays and the answer is `ARTIFACTS_STORE_INCOMPLETE`: exit 1, retryable, `meta.next` leads with
/// `artifacts verify`, and the same call stores the rest. A call that stores nothing before a failure is the rolled-back `ARTIFACTS_STORE_FAILED`
/// (`artifacts_object_store.rs` holds that one against an object store).
#[test]
fn atip_incomplete_03_an_artifact_store_that_stops_after_a_blob_is_incomplete_and_the_same_call_finishes(
) {
    let dir = repo_with_scenario();
    let repo = dir.path();
    std::fs::create_dir_all(repo.join("reports")).unwrap();
    std::fs::write(repo.join("reports/a.json"), br#"{"checks":1}"#).unwrap();
    std::fs::write(repo.join("reports/b.json"), br#"{"checks":2}"#).unwrap();
    let attested = canon_env(
        repo,
        &[
            "evidence",
            "add",
            "--scenario",
            "world.a.01",
            "--class",
            "test-run",
            "--ref",
            "x",
            "--actor",
            "a1",
            "--role",
            "implementer",
            "--artifact",
            "reports/a.json",
            "--artifact",
            "reports/b.json",
            "--promote",
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(attested.status.code(), Some(0), "{}", text(&attested));
    let blobs = repo.join(".canon/artifacts/sha256");
    assert_eq!(
        files_in(&blobs).len(),
        2,
        "the attestation stored both blobs"
    );
    std::fs::remove_dir_all(repo.join(".canon/artifacts")).unwrap();

    let out = canon_env(
        repo,
        &["artifacts", "store", "--json"],
        None,
        &[("CANON_TEST_STOP_AT", "artifacts-store-second")],
    );
    let body = document(&out);
    assert_eq!(out.status.code(), Some(1), "{body}");
    assert_eq!(
        (
            body["error"]["code"].as_str(),
            body["error"]["retryable"].as_bool()
        ),
        (Some("ARTIFACTS_STORE_INCOMPLETE"), Some(true)),
        "{body}"
    );
    assert_eq!(body["error"]["reason"], "ARTIFACTS_STORE_FAILED", "{body}");
    assert_eq!(
        body["meta"]["next"][0]["argv"],
        serde_json::json!(["artifacts", "verify", "--json"]),
        "{body}"
    );
    assert_eq!(files_in(&blobs).len(), 1, "the blob that was stored stays");
    let verify = canon_env(repo, &["artifacts", "verify", "--json"], None, &[]);
    let read = document(&verify);
    assert_eq!(
        verify.status.code(),
        Some(0),
        "the first step runs: {}",
        text(&verify)
    );
    assert_eq!(
        (
            read["data"]["bound"].as_u64(),
            read["data"]["stored"].as_u64(),
            read["data"]["workingTreeOnly"].as_u64()
        ),
        (Some(2), Some(1), Some(1)),
        "it shows one blob stored and one not: {read}"
    );

    // The retry value says yes: the same call stores the rest.
    let again = canon_env(repo, &["artifacts", "store", "--json"], None, &[]);
    assert_eq!(again.status.code(), Some(0), "{}", text(&again));
    assert_eq!(files_in(&blobs).len(), 2);
    assert_eq!(
        canon_env(repo, &["artifacts", "verify", "--json"], None, &[])
            .status
            .code(),
        Some(0)
    );
}

// ------------------------------------------------------------------ hooks

/// atip.incomplete.04: `gate install-hooks` writes the Claude hook file, then the Codex one; when the second write fails after the first was written
/// (the debug seam `hooks-second-file`), the first stays and the answer is `HOOKS_INSTALL_INCOMPLETE`: exit 1, not retryable, `meta.next` leads with
/// `canon status`. When the first file was already installed, the same failure kept nothing of its own: it is `WRITE_FAILED`, not `INCOMPLETE`.
#[test]
fn atip_incomplete_04_a_hook_install_that_stops_after_the_first_file_is_incomplete_and_one_that_wrote_nothing_is_write_failed(
) {
    let dir = repo_with_scenario();
    let repo = dir.path();
    let claude = repo.join(".claude/settings.json");
    let codex = repo.join(".codex/hooks.json");
    let out = canon_env(
        repo,
        &["gate", "install-hooks", "--json"],
        None,
        &[("CANON_TEST_STOP_AT", "hooks-second-file")],
    );
    let body = document(&out);
    assert_eq!(out.status.code(), Some(1), "{body}");
    assert_eq!(
        (
            body["error"]["code"].as_str(),
            body["error"]["retryable"].as_bool()
        ),
        (Some("HOOKS_INSTALL_INCOMPLETE"), Some(false)),
        "{body}"
    );
    assert_eq!(body["error"]["reason"], "WRITE_FAILED", "{body}");
    assert_eq!(
        body["meta"]["next"][0]["argv"],
        serde_json::json!(["status", "--json"]),
        "{body}"
    );
    assert!(
        claude.is_file()
            && std::fs::read_to_string(&claude)
                .unwrap()
                .contains("canon task done"),
        "the first hook file stays installed"
    );
    assert!(!codex.exists(), "the second was not written");

    // The rerun finishes the install.
    let rerun = canon_env(repo, &["gate", "install-hooks", "--json"], None, &[]);
    assert_eq!(rerun.status.code(), Some(0), "{}", text(&rerun));
    assert!(codex.is_file());

    // With both installed, a failing second write has kept nothing of its own: one write that failed is `WRITE_FAILED`.
    let failed = canon_env(
        repo,
        &["gate", "install-hooks", "--json"],
        None,
        &[("CANON_TEST_STOP_AT", "hooks-second-file")],
    );
    let failed_body = document(&failed);
    assert_eq!(failed.status.code(), Some(1), "{failed_body}");
    assert_eq!(
        failed_body["error"]["code"], "WRITE_FAILED",
        "{failed_body}"
    );
    assert!(
        failed_body["error"].get("reason").is_none(),
        "{failed_body}"
    );

    // A preview writes nothing, so the seam has nothing to stop.
    let preview = canon_env(
        repo,
        &["gate", "install-hooks", "--dry-run", "--json"],
        None,
        &[("CANON_TEST_STOP_AT", "hooks-second-file")],
    );
    assert_eq!(preview.status.code(), Some(0), "{}", text(&preview));
}

/// atip.incomplete.04 (the third step): the pre-commit script is written after both hook files; when its write fails (the debug seam `hooks-script`)
/// the hook files stay and the answer is `HOOKS_INSTALL_INCOMPLETE` with the reason `WRITE_FAILED` and `meta.next` leading with `canon status`, not
/// `WRITE_FAILED` alone; the rerun without the seam writes the script.
#[test]
fn atip_incomplete_04_a_script_write_that_fails_after_both_hook_files_is_incomplete() {
    let dir = repo_with_scenario();
    let repo = dir.path();
    let out = canon_env(repo, &["gate", "install-hooks", "--json"], None, &[("CANON_TEST_STOP_AT", "hooks-script")]);
    let body = document(&out);
    assert_eq!(out.status.code(), Some(1), "{body}");
    assert_eq!((body["error"]["code"].as_str(), body["error"]["reason"].as_str(), body["error"]["retryable"].as_bool()), (Some("HOOKS_INSTALL_INCOMPLETE"), Some("WRITE_FAILED"), Some(false)), "{body}");
    assert!(body["error"]["message"].as_str().is_some_and(|message| message.contains("failed to write")), "{body}");
    assert_eq!(body["meta"]["next"][0]["argv"], serde_json::json!(["status", "--json"]), "{body}");
    assert!(repo.join(".claude/settings.json").is_file() && repo.join(".codex/hooks.json").is_file(), "both hook files stay installed");

    // The rerun without the seam writes the script and exits 0.
    let rerun = canon_env(repo, &["gate", "install-hooks", "--json"], None, &[]);
    assert_eq!(rerun.status.code(), Some(0), "{}", text(&rerun));
}

// ------------------------------------------------------------------ purge, compaction and tier aging

const ROUTING: &str = "  trajectory: local\n  handoff: local\n  subject: local\n";
const AGING: &str = "  trajectory: { after: 1d, to: cold }\n";

fn old_trajectories(fixture: &support::Fixture) {
    fixture.plant_trajectory_in_git(chrono::Utc::now() - chrono::Duration::days(5), 0.1);
    fixture.plant_trajectory_in_git(chrono::Utc::now() - chrono::Duration::days(4), 0.2);
}

fn digest_of(preview: &Output) -> String {
    let body = document(preview);
    assert_eq!(preview.status.code(), Some(0), "{body}");
    body["data"]["planDigest"]
        .as_str()
        .unwrap_or_else(|| panic!("a preview names its plan digest: {body}"))
        .to_string()
}

fn trajectories_in_ledger(fixture: &support::Fixture) -> usize {
    fn count(dir: &Path) -> usize {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| {
                if entry.path().is_dir() {
                    count(&entry.path())
                } else {
                    1
                }
            })
            .sum()
    }
    count(&fixture.git_root().join("kind=trajectory"))
}

/// atip.incomplete.05: `purge --commit` deletes the records of its plan; when it stops after it began deleting (the debug seam `purge-after-delete`) what
/// was deleted stays deleted and the answer is `RETENTION_PURGE_INCOMPLETE`: exit 1, not retryable, `data` names the counts, `meta.next` leads with
/// `canon status`; the same `--plan-digest` is then stale (`PLAN_STALE`, exit 2). A purge that nothing stops deletes the same records and exits 0.
#[test]
fn atip_incomplete_05_a_purge_that_stops_after_it_deleted_is_incomplete_and_its_digest_is_stale() {
    let fixture = support::Fixture::new(ROUTING, AGING);
    std::fs::create_dir_all(fixture.root.path().join(".canon/learn")).unwrap();
    old_trajectories(&fixture);
    let root = fixture.root.path();
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(1)).to_rfc3339();
    let r2 = fixture.r2_root().display().to_string();
    let env = [("CANON_R2_LOCAL_ROOT", r2.as_str())];
    let preview = canon_env(
        root,
        &[
            "purge",
            "--kind",
            "trajectory",
            "--before",
            &cutoff,
            "--json",
        ],
        None,
        &env,
    );
    let digest = digest_of(&preview);
    assert_eq!(trajectories_in_ledger(&fixture), 2);

    let seam = [
        ("CANON_R2_LOCAL_ROOT", r2.as_str()),
        ("CANON_TEST_STOP_AT", "purge-after-delete"),
    ];
    let out = canon_env(
        root,
        &[
            "purge",
            "--kind",
            "trajectory",
            "--before",
            &cutoff,
            "--commit",
            "--plan-digest",
            &digest,
            "--json",
        ],
        None,
        &seam,
    );
    let body = document(&out);
    assert_eq!(out.status.code(), Some(1), "{body}");
    assert_eq!(
        (
            body["error"]["code"].as_str(),
            body["error"]["retryable"].as_bool()
        ),
        (Some("RETENTION_PURGE_INCOMPLETE"), Some(false)),
        "{body}"
    );
    assert!(
        body["data"].is_object(),
        "`data` names what was deleted: {body}"
    );
    assert_eq!(
        body["meta"]["next"][0]["argv"],
        serde_json::json!(["status", "--json"]),
        "{body}"
    );
    assert_eq!(
        trajectories_in_ledger(&fixture),
        0,
        "what was deleted stays deleted"
    );

    // The digest is stale now: the plan has nothing left to delete.
    let again = canon_env(
        root,
        &[
            "purge",
            "--kind",
            "trajectory",
            "--before",
            &cutoff,
            "--commit",
            "--plan-digest",
            &digest,
            "--json",
        ],
        None,
        &env,
    );
    let stale = document(&again);
    assert_eq!(again.status.code(), Some(2), "{stale}");
    assert_eq!(stale["error"]["code"], "PLAN_STALE", "{stale}");

    // Nothing stops the same purge on a fresh fixture: it deletes the records and exits 0.
    let other = support::Fixture::new(ROUTING, AGING);
    std::fs::create_dir_all(other.root.path().join(".canon/learn")).unwrap();
    old_trajectories(&other);
    let other_r2 = other.r2_root().display().to_string();
    let other_env = [("CANON_R2_LOCAL_ROOT", other_r2.as_str())];
    let other_digest = digest_of(&canon_env(
        other.root.path(),
        &[
            "purge",
            "--kind",
            "trajectory",
            "--before",
            &cutoff,
            "--json",
        ],
        None,
        &other_env,
    ));
    let done = canon_env(
        other.root.path(),
        &[
            "purge",
            "--kind",
            "trajectory",
            "--before",
            &cutoff,
            "--commit",
            "--plan-digest",
            &other_digest,
            "--json",
        ],
        None,
        &other_env,
    );
    assert_eq!(done.status.code(), Some(0), "{}", text(&done));
    assert_eq!(trajectories_in_ledger(&other), 0);
}

/// A ledger written before 0.23: three versions of one subject, every version in the ledger and the archive empty (`atip_planjcs.rs` makes the same).
fn compactable() -> support::Fixture {
    use canon_model::envelope::{Actor, Envelope, RecordKind};
    use canon_model::ids::RoleId;
    use canon_model::{Subject, SubjectId, SubjectStatus};
    use canon_store::git_tier::GitTier;
    use canon_store::tier::Tier;

    let fixture = support::Fixture::new(ROUTING, AGING);
    let git = GitTier::new(fixture.git_root());
    let role = RoleId::parse("implementer").unwrap();
    let actor = Actor::new("test-agent", role.clone());
    for (offset, title) in [(20, "Seeded one"), (10, "Seeded two"), (5, "Seeded three")] {
        let envelope = Envelope::new(
            1,
            RecordKind::Subject,
            chrono::Utc::now() - chrono::Duration::seconds(offset),
            actor.clone(),
        );
        git.write(&Subject::new(
            envelope,
            SubjectId::parse("demo-subject").unwrap(),
            title,
            "s",
            "dev",
            SubjectStatus::Verifying,
            role.clone(),
        ))
        .unwrap();
    }
    let archive = fixture.root.path().join(".canon/archive");
    let mut stack = vec![archive.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let target = fixture
                    .git_root()
                    .join(path.strip_prefix(&archive).unwrap());
                std::fs::create_dir_all(target.parent().unwrap()).unwrap();
                std::fs::rename(&path, &target).unwrap();
            }
        }
    }
    fixture
}

fn archived(fixture: &support::Fixture) -> usize {
    fn count(dir: &Path) -> usize {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| {
                if entry.path().is_dir() {
                    count(&entry.path())
                } else {
                    1
                }
            })
            .sum()
    }
    count(&fixture.root.path().join(".canon/archive"))
}

/// atip.incomplete.06: `ledger compact --commit` moves the superseded versions to the archive; when it stops after it moved them (the debug seam
/// `ledger-compact-after-move`) what moved stays in the archive and the answer is `LEDGER_COMPACT_INCOMPLETE`: exit 1, not retryable, `meta.next` leads
/// with `canon status`, and the same `--plan-digest` is stale (`PLAN_STALE`, exit 2): the rest starts from a new preview. Nothing that stops it moves
/// the same versions and exits 0.
#[test]
fn atip_incomplete_06_a_compaction_that_stops_after_it_moved_is_incomplete_and_its_digest_is_stale()
{
    let fixture = compactable();
    let root = fixture.root.path();
    assert_eq!(archived(&fixture), 0);
    let digest = digest_of(&canon_env(
        root,
        &["ledger", "compact", "--json"],
        None,
        &[],
    ));

    let out = canon_env(
        root,
        &[
            "ledger",
            "compact",
            "--commit",
            "--plan-digest",
            &digest,
            "--json",
        ],
        None,
        &[("CANON_TEST_STOP_AT", "ledger-compact-after-move")],
    );
    let body = document(&out);
    assert_eq!(out.status.code(), Some(1), "{body}");
    assert_eq!(
        (
            body["error"]["code"].as_str(),
            body["error"]["retryable"].as_bool()
        ),
        (Some("LEDGER_COMPACT_INCOMPLETE"), Some(false)),
        "{body}"
    );
    assert_eq!(
        body["meta"]["next"][0]["argv"],
        serde_json::json!(["status", "--json"]),
        "{body}"
    );
    assert!(
        archived(&fixture) >= 2,
        "the versions that moved stay in the archive"
    );

    let again = canon_env(
        root,
        &[
            "ledger",
            "compact",
            "--commit",
            "--plan-digest",
            &digest,
            "--json",
        ],
        None,
        &[],
    );
    let stale = document(&again);
    assert_eq!(again.status.code(), Some(2), "{stale}");
    assert_eq!(stale["error"]["code"], "PLAN_STALE", "{stale}");

    // Nothing stops the same compaction on a fresh fixture.
    let other = compactable();
    let other_digest = digest_of(&canon_env(
        other.root.path(),
        &["ledger", "compact", "--json"],
        None,
        &[],
    ));
    let done = canon_env(
        other.root.path(),
        &[
            "ledger",
            "compact",
            "--commit",
            "--plan-digest",
            &other_digest,
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(done.status.code(), Some(0), "{}", text(&done));
    assert!(archived(&other) >= 2);
}

/// atip.incomplete.07: `tier age --commit` moves the records past their threshold; when it stops after records moved (the debug seam `tier-age-after-move`)
/// they stay moved and the answer is `TIER_INCOMPLETE`: exit 1, not retryable, `meta.next` leads with `canon status`. A run that fails before any record
/// moved (the cold tier cannot be attached) is `TIER_FAILED` and every record is where it was.
#[test]
fn atip_incomplete_07_tier_aging_that_stops_after_records_moved_is_incomplete_and_one_that_moved_none_is_failed(
) {
    let fixture = support::Fixture::new(ROUTING, AGING);
    old_trajectories(&fixture);
    fixture.plant_trajectory_in_git(chrono::Utc::now(), 0.9);
    let root = fixture.root.path();
    let r2 = fixture.r2_root().display().to_string();
    let env = [("CANON_R2_LOCAL_ROOT", r2.as_str())];
    assert_eq!(trajectories_in_ledger(&fixture), 3);

    // The cold tier is not attached: the aging fails before it moved anything.
    let failed = canon_env(root, &["tier", "age", "--json"], None, &[]);
    let failed_body = document(&failed);
    assert_eq!(failed.status.code(), Some(1), "{failed_body}");
    assert_eq!(
        (
            failed_body["error"]["code"].as_str(),
            failed_body["error"]["retryable"].as_bool()
        ),
        (Some("TIER_FAILED"), Some(false)),
        "{failed_body}"
    );
    assert_eq!(
        trajectories_in_ledger(&fixture),
        3,
        "every record is where it was"
    );

    let digest = digest_of(&canon_env(root, &["tier", "age", "--json"], None, &env));
    let seam = [
        ("CANON_R2_LOCAL_ROOT", r2.as_str()),
        ("CANON_TEST_STOP_AT", "tier-age-after-move"),
    ];
    let out = canon_env(
        root,
        &[
            "tier",
            "age",
            "--commit",
            "--plan-digest",
            &digest,
            "--json",
        ],
        None,
        &seam,
    );
    let body = document(&out);
    assert_eq!(out.status.code(), Some(1), "{body}");
    assert_eq!(
        (
            body["error"]["code"].as_str(),
            body["error"]["retryable"].as_bool()
        ),
        (Some("TIER_INCOMPLETE"), Some(false)),
        "{body}"
    );
    assert_eq!(
        body["meta"]["next"][0]["argv"],
        serde_json::json!(["status", "--json"]),
        "{body}"
    );
    assert_eq!(
        trajectories_in_ledger(&fixture),
        1,
        "the records that moved stay moved: only the recent one is left"
    );
}

/// atip.incomplete.05 (nothing deleted): a purge that fails before it deleted its first record (the debug seam `purge-before-delete`) is answered by
/// its cause, the refusal `RETENTION_TIERS_INVALID` (exit 2), never `RETENTION_PURGE_INCOMPLETE`: nothing was deleted, no `meta.next` read of what
/// was deleted, and the records stay.
#[test]
fn atip_incomplete_05_a_purge_that_fails_before_it_deleted_anything_is_not_incomplete() {
    let fixture = support::Fixture::new(ROUTING, AGING);
    std::fs::create_dir_all(fixture.root.path().join(".canon/learn")).unwrap();
    old_trajectories(&fixture);
    let root = fixture.root.path();
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(1)).to_rfc3339();
    let r2 = fixture.r2_root().display().to_string();
    let env = [("CANON_R2_LOCAL_ROOT", r2.as_str())];
    let digest = digest_of(&canon_env(root, &["purge", "--kind", "trajectory", "--before", &cutoff, "--json"], None, &env));
    let seam = [("CANON_R2_LOCAL_ROOT", r2.as_str()), ("CANON_TEST_STOP_AT", "purge-before-delete")];
    let out = canon_env(root, &["purge", "--kind", "trajectory", "--before", &cutoff, "--commit", "--plan-digest", &digest, "--json"], None, &seam);
    let body = document(&out);
    assert_eq!(out.status.code(), Some(2), "{body}");
    assert_eq!(body["error"]["code"], "RETENTION_TIERS_INVALID", "{body}");
    assert!(body["meta"]["next"].as_array().is_none_or(|next| next.iter().all(|step| step["argv"] != serde_json::json!(["status", "--json"]))), "{body}");
    assert_eq!(trajectories_in_ledger(&fixture), 2, "nothing was deleted");
}

/// atip.incomplete.06 (nothing moved): a ledger whose kind directories this user may not read makes the compaction fail in its read of the ledger,
/// before the first move, and a preview never moves. Neither is `LEDGER_COMPACT_INCOMPLETE`: the answer is the refusal of the read
/// (`REPO_DENIED`, exit 2), with no `meta.next` read of what moved, and nothing moved. (Skipped for a user the mode does not stop, such as root.)
#[cfg(unix)]
#[test]
fn atip_incomplete_06_a_compaction_that_cannot_read_the_ledger_before_it_moved_anything_is_not_incomplete() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = compactable();
    let root = fixture.root.path();
    let kinds: Vec<PathBuf> = std::fs::read_dir(fixture.git_root()).unwrap().flatten().map(|entry| entry.path()).filter(|path| path.is_dir() && path.file_name().is_some_and(|name| name.to_string_lossy().starts_with("kind="))).collect();
    assert!(!kinds.is_empty());
    for kind in &kinds {
        std::fs::set_permissions(kind, std::fs::Permissions::from_mode(0o000)).unwrap();
    }
    let restore = || {
        for kind in &kinds {
            let _ = std::fs::set_permissions(kind, std::fs::Permissions::from_mode(0o755));
        }
    };
    if std::fs::read_dir(&kinds[0]).is_ok() {
        restore();
        return;
    }
    let preview = canon_env(root, &["ledger", "compact", "--json"], None, &[]);
    let commit = canon_env(root, &["ledger", "compact", "--commit", "--plan-digest", "sha256:0000000000000000000000000000000000000000000000000000000000000000", "--json"], None, &[]);
    restore();
    for (what, out) in [("preview", preview), ("commit", commit)] {
        let body = document(&out);
        assert_eq!(out.status.code(), Some(2), "{what}: {body}");
        assert_eq!(body["error"]["code"], "REPO_DENIED", "{what}: {body}");
        assert!(body["meta"]["next"].as_array().is_none_or(|next| next.iter().all(|step| step["argv"] != serde_json::json!(["status", "--json"]))), "{what}: {body}");
    }
    assert_eq!(archived(&fixture), 0, "nothing moved");
}

// ------------------------------------------------------------------ inventory

const PROVENANCE: &str = "  # canon: {\"schema\":1,\"at\":\"2026-07-10T00:00:00Z\",\"actor\":{\"agent_id\":\"canon-fmt\"}}";

fn feature(title: &str, tags: &[&str]) -> String {
    let mut text = format!("Feature: {title}\n{PROVENANCE}\n");
    for tag in tags {
        text.push_str(&format!(
            "\n  @{tag}\n  Scenario: {tag}\n{PROVENANCE}\n    Given a step\n"
        ));
    }
    text
}

fn write(dir: &Path, rel: &str, body: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

const TWO_ROOTS: &str = "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\n  scenario: local\nspecs:\n  roots:\n    - id: first\n      root: first\n    - id: second\n      root: second\n";

/// A repository with two spec roots, the first clean and the second holding `second` as its corpus.
fn two_roots(second: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "canon.yaml", TWO_ROOTS);
    write(dir.path(), "first/kind=feature/area=one/a.feature", &feature("one a", &["one.a.01"]));
    write(dir.path(), "second/kind=feature/area=two/b.feature", second);
    dir
}

fn ledger_has_scenarios(repo: &Path) -> bool {
    std::fs::read_dir(repo.join(".canon/ledger/kind=scenario")).is_ok_and(|entries| entries.count() > 0)
}

/// atip.incomplete.09: `inventory sync` indexes each spec root in turn. A commit that indexed an earlier root and then left a later one unindexed is
/// `INVENTORY_INCOMPLETE` (exit 1, not retryable, the reason `INVENTORY_INVALID`, `data.roots` per root, `meta.next` leading with
/// `canon status`) and the earlier root's records stay. When this run indexed nothing before the stop (the earlier root was indexed by a run before),
/// the answer is the refusal `INVENTORY_INVALID` (exit 2, the reasons `VIOLATIONS` and `DUPLICATE_SCENARIOS`, each one in `error.details`, no `data`), not an `_INCOMPLETE` code. A corpus with neither is indexed
/// whole.
#[test]
fn atip_incomplete_09_a_spec_root_left_unindexed_after_an_earlier_one_was_indexed_is_incomplete_with_its_reason() {
    let dir = two_roots(&feature("two b", &["two.b.01", "two.b.01"]));
    let repo = dir.path();
    let out = canon_env(repo, &["inventory", "sync", "--json"], None, &[]);
    let body = document(&out);
    assert_eq!(out.status.code(), Some(1), "{body}");
    assert_eq!((body["error"]["code"].as_str(), body["error"]["reason"].as_str(), body["error"]["retryable"].as_bool()), (Some("INVENTORY_INCOMPLETE"), Some("INVENTORY_INVALID"), Some(false)), "{body}");
    let roots = body["data"]["roots"].as_array().expect("data.roots");
    assert_eq!(roots.len(), 2, "{body}");
    assert!(roots[0]["written"].as_u64().is_some_and(|written| written > 0), "the first root was indexed: {body}");
    assert_eq!(roots[1]["written"].as_u64(), Some(0), "the second was not: {body}");
    assert_eq!(body["meta"]["next"][0]["argv"], serde_json::json!(["status", "--json"]), "{body}");
    assert!(ledger_has_scenarios(repo), "the earlier root's records stay");

    // The first root is indexed now: a rerun writes nothing before it meets the second root again, so the answer is the corpus's refusal.
    let again = canon_env(repo, &["inventory", "sync", "--json"], None, &[]);
    let again_body = document(&again);
    assert_eq!(again.status.code(), Some(2), "{again_body}");
    assert_eq!((again_body["error"]["code"].as_str(), again_body["error"]["reason"].as_str(), again_body["error"]["retryable"].as_bool()), (Some("INVENTORY_INVALID"), Some("DUPLICATE_SCENARIOS"), Some(false)), "{again_body}");
    assert!(again_body.get("data").is_none(), "a refusal carries no data: {again_body}");
    let details = again_body["error"]["details"].as_array().expect("error.details names each duplicate");
    assert!(details.len() >= 2 && details.iter().all(|detail| detail["code"] == "INVENTORY_INVALID"), "{again_body}");
    assert!(details.iter().skip(1).any(|detail| detail["reason"] == "DUPLICATE_SCENARIOS" && detail["message"].as_str().is_some_and(|message| message.contains("root `second`"))), "{again_body}");

    // A second root with a violation (a scenario with no provenance line).
    write(repo, "second/kind=feature/area=two/b.feature", "Feature: two b\n\n  @two.b.01\n  Scenario: two\n    Given a step\n");
    let violated = canon_env(repo, &["inventory", "sync", "--json"], None, &[]);
    let violated_body = document(&violated);
    assert_eq!(violated.status.code(), Some(2), "{violated_body}");
    assert_eq!((violated_body["error"]["code"].as_str(), violated_body["error"]["reason"].as_str()), (Some("INVENTORY_INVALID"), Some("VIOLATIONS")), "{violated_body}");

    // A corpus without either is indexed whole.
    write(repo, "second/kind=feature/area=two/b.feature", &feature("two b", &["two.b.01"]));
    let done = canon_env(repo, &["inventory", "sync", "--json"], None, &[]);
    assert_eq!(done.status.code(), Some(0), "{}", text(&done));
}

/// atip.incomplete.09 (nothing indexed): one spec root with a violation, the usual case, indexes nothing: `INVENTORY_INVALID` (exit 2, reason
/// `VIOLATIONS`, not retryable) with the violation in `error.details`, no `data` and a hint that fixes the corpus, never `INVENTORY_INCOMPLETE`, no read-what-was-indexed
/// `meta.next`, and no ledger is created. The preview of a sync that would stop on a second root after indexing the first says it is a preview and
/// answers the same refusal, with nothing written.
#[test]
fn atip_incomplete_09_a_sync_that_indexed_nothing_answers_the_corpus_refusal_and_a_preview_never_answers_incomplete() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    write(repo, "canon.yaml", "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\n  scenario: local\nspecs:\n  roots:\n    - id: first\n      root: first\n");
    write(repo, "first/kind=feature/area=one/a.feature", "Feature: one a\n\n  @one.a.01\n  Scenario: no provenance\n    Given a step\n");
    let out = canon_env(repo, &["inventory", "sync", "--json"], None, &[]);
    let body = document(&out);
    assert_eq!(out.status.code(), Some(2), "{body}");
    assert_eq!((body["error"]["code"].as_str(), body["error"]["reason"].as_str(), body["error"]["retryable"].as_bool()), (Some("INVENTORY_INVALID"), Some("VIOLATIONS"), Some(false)), "{body}");
    assert!(body.get("data").is_none(), "a refusal carries no data: {body}");
    assert!(body["error"]["details"].as_array().is_some_and(|details| details.len() >= 2 && details[1]["reason"] == "VIOLATIONS" && details[1]["message"].as_str().is_some_and(|message| message.contains("missing-provenance") || message.contains("provenance"))), "each violation is in error.details: {body}");
    assert!(body["error"]["message"].as_str().is_some_and(|message| message.contains("indexed nothing")), "the message follows the real outcome: {body}");
    assert!(body["meta"]["next"].as_array().is_none_or(|next| next.iter().all(|step| step["argv"] != serde_json::json!(["status", "--json"]))), "no read of what was indexed: {body}");
    assert!(!repo.join(".canon/ledger").exists(), "nothing was written");

    // A preview over two roots (the second duplicates a scenario): never `_INCOMPLETE`, says it is a preview, writes nothing.
    let two = two_roots(&feature("two b", &["two.b.01", "two.b.01"]));
    let preview = canon_env(two.path(), &["inventory", "sync", "--dry-run", "--json"], None, &[]);
    let preview_body = document(&preview);
    assert_eq!(preview.status.code(), Some(2), "{preview_body}");
    assert_eq!((preview_body["error"]["code"].as_str(), preview_body["error"]["reason"].as_str()), (Some("INVENTORY_INVALID"), Some("DUPLICATE_SCENARIOS")), "{preview_body}");
    assert!(preview_body["error"]["message"].as_str().is_some_and(|message| message.contains("preview")), "{preview_body}");
    assert!(preview_body["meta"]["next"].as_array().is_none_or(|next| next.iter().all(|step| step["argv"] != serde_json::json!(["status", "--json"]))), "{preview_body}");
    assert!(!ledger_has_scenarios(two.path()), "a preview writes nothing");
}

/// atip.incomplete.09 (the store): the record store failing while a later root is indexed (the debug seam `inventory-second-root`) is
/// `INVENTORY_INCOMPLETE` with the reason `INVENTORY_FAILED` when an earlier root was indexed by this run, and `INVENTORY_FAILED` (exit 1) when
/// nothing was indexed before the failure (the first root was indexed by a run before).
#[test]
fn atip_incomplete_09_a_store_failure_after_an_earlier_root_was_indexed_is_incomplete_and_one_before_any_is_inventory_failed() {
    let dir = two_roots(&feature("two b", &["two.b.01"]));
    let repo = dir.path();
    let seam = [("CANON_TEST_STOP_AT", "inventory-second-root")];
    let out = canon_env(repo, &["inventory", "sync", "--json"], None, &seam);
    let body = document(&out);
    assert_eq!(out.status.code(), Some(1), "{body}");
    assert_eq!((body["error"]["code"].as_str(), body["error"]["reason"].as_str(), body["error"]["retryable"].as_bool()), (Some("INVENTORY_INCOMPLETE"), Some("INVENTORY_FAILED"), Some(false)), "{body}");
    assert_eq!(body["meta"]["next"][0]["argv"], serde_json::json!(["status", "--json"]), "{body}");
    assert!(ledger_has_scenarios(repo), "the earlier root's records stay");

    // The first root is indexed by the run before: the same failure indexed nothing in this run.
    let again = canon_env(repo, &["inventory", "sync", "--json"], None, &seam);
    let again_body = document(&again);
    assert_eq!(again.status.code(), Some(1), "{again_body}");
    assert_eq!(again_body["error"]["code"], "INVENTORY_FAILED", "{again_body}");
    assert!(again_body["error"].get("reason").is_none(), "{again_body}");

    // Without the seam the sync finishes.
    let done = canon_env(repo, &["inventory", "sync", "--json"], None, &[]);
    assert_eq!(done.status.code(), Some(0), "{}", text(&done));
}

// ------------------------------------------------------------------ the CHANGELOG

/// The `## Unreleased` section of the CHANGELOG.
fn unreleased() -> String {
    let changelog =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CHANGELOG.md"))
            .expect("the CHANGELOG");
    let start = changelog
        .find("\n## Unreleased")
        .expect("the CHANGELOG has an Unreleased section");
    let end = changelog[start + 1..]
        .find("\n## 0.")
        .map_or(changelog.len(), |at| start + 1 + at);
    changelog[start..end].to_string()
}

/// What the replacement table of `canon-0-36-codes-incomplete` lacks in `section`: every replaced name, as the first cell of a table row, and every
/// `_INCOMPLETE` code of the registry somewhere in it.
fn changelog_problems(section: &str) -> Vec<String> {
    let mut problems = Vec::new();
    for old in GONE {
        if *old == "INIT_SKILL_INSTALL_FAILED" {
            continue;
        }
        if !section
            .lines()
            .any(|line| line.starts_with(&format!("| `{old}`")))
        {
            problems.push(format!("the table has no row for the replaced code {old}"));
        }
    }
    for (code, _, _) in INCOMPLETE {
        if !section.contains(&format!("`{code}`")) {
            problems.push(format!("the section never says {code}"));
        }
    }
    problems
}

/// atip.incomplete.12: the CHANGELOG's replacement table has a row for each replaced code, and says each `_INCOMPLETE` code and its retry value; a table
/// that drops a row, or a section that leaves a code out, is named.
#[test]
fn atip_incomplete_12_the_changelog_replacement_table_covers_every_replaced_code() {
    let section = unreleased();
    assert_eq!(changelog_problems(&section), Vec::<String>::new());
    let without_row = section
        .lines()
        .filter(|line| !line.starts_with("| `MIGRATE_PINS_FAILED`"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        changelog_problems(&without_row),
        ["the table has no row for the replaced code MIGRATE_PINS_FAILED"]
    );
    let without_code = section.replace("`TIER_INCOMPLETE`", "`tier incomplete`");
    assert_eq!(
        changelog_problems(&without_code),
        ["the section never says TIER_INCOMPLETE"]
    );
}
