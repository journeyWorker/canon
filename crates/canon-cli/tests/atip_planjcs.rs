//! `canon-0-35-plan-jcs` (atip/1 1.2 spec 12.4-12.8): the digest of a destructive plan is `sha256:` and the SHA-256 of the
//! JCS of `{effects, inputs}`, a commit holds the repository write lock, and the `purge` preview lists its audit line.
//! Behaviour through the built `canon` binary (exit codes, JSON, what is and is not written). Each test names the scenario
//! it proves (`atip.planjcs.NN`).

mod support;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use canon_model::envelope::{Actor, Envelope, RecordKind};
use canon_model::ids::{RoleId, RunId};
use canon_model::records::Trajectory;
use canon_model::{Subject, SubjectId, SubjectStatus};
use canon_store::file_lock::FileLock;
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;
use chrono::Utc;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use support::Fixture;

const ROUTING: &str = "  trajectory: local\n  handoff: local\n  subject: local\n";
const AGING: &str = "  trajectory: { after: 1d, to: cold }\n";
const LOCK: &str = ".canon/repo-write.lock";

fn text(out: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

fn json_of(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("stdout is not one JSON document ({e}): {}", text(out)))
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"]).args(args).current_dir(dir).output().expect("spawn git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// Every file under `root` but `.git`: path to bytes.
fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            // The lock file is process state a call that locked leaves behind, refused or not, and no plan reads it: not part of the tree.
            if path.file_name().is_some_and(|name| name == ".git") || path.ends_with(LOCK) {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else {
                out.insert(path.strip_prefix(root).unwrap().to_string_lossy().into_owned(), std::fs::read(&path).unwrap());
            }
        }
    }
    out
}

fn canon_in(root: &Path, args: &[String], env: &[(String, String)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_canon"));
    command.args(args).current_dir(root).env_remove("CANON_ROLE").env_remove("CANON_TEST_REPO_LOCK_WAIT_MS").env_remove("CANON_TEST_HOLD_AFTER_RUN_MS").env("CANON_ACTOR", "canon");
    for (key, value) in env {
        command.env(key, value);
    }
    command.output().expect("spawn canon")
}

fn words(args: &[&str]) -> Vec<String> {
    args.iter().map(ToString::to_string).collect()
}

fn init_repo(dir: &Path, with_git: bool) {
    if with_git {
        git(dir, &["init", "-q", "-b", "main"]);
        git(dir, &["commit", "-q", "--allow-empty", "-m", "seed"]);
    }
    let out = canon_in(dir, &words(&["init", "--providers", "none", "--no-agents-md", "--no-policy"]), &[]);
    assert!(out.status.success(), "{}", text(&out));
    if with_git {
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "-q", "-m", "init"]);
    }
}

fn actor() -> Actor {
    Actor::new("test-agent", RoleId::parse("implementer").unwrap())
}

/// One destructive command with something to do, and a change under its plan that leaves its effects alone.
struct Case {
    leaf: &'static str,
    root: PathBuf,
    /// The arguments of the preview, without `--json`.
    args: Vec<String>,
    env: Vec<(String, String)>,
    /// A change to what the plan read that makes no effect appear or go (`None` for the commands that have none to make).
    change: Option<Box<dyn Fn()>>,
    _keep: Vec<Box<dyn std::any::Any>>,
}

impl Case {
    fn run(&self, extra: &[&str]) -> Output {
        let mut args = self.args.clone();
        args.extend(extra.iter().map(ToString::to_string));
        args.push("--json".to_string());
        canon_in(&self.root, &args, &self.env)
    }

    fn preview(&self) -> Value {
        let out = self.run(&[]);
        assert_eq!(out.status.code(), Some(0), "{}: {}", self.leaf, text(&out));
        json_of(&out)
    }

    fn commit_with(&self, digest: &str) -> Output {
        self.run(&["--commit", &format!("--plan-digest={digest}")])
    }

    fn leaf_words(&self) -> Vec<&str> {
        self.leaf.split(' ').collect()
    }
}

fn old_trajectories(fixture: &Fixture) {
    fixture.plant_trajectory_in_git(Utc::now() - chrono::Duration::days(5), 0.1);
    fixture.plant_trajectory_in_git(Utc::now() - chrono::Duration::days(4), 0.2);
}

fn purge_case() -> Case {
    let fixture = Fixture::new(ROUTING, AGING);
    std::fs::create_dir_all(fixture.root.path().join(".canon/learn")).unwrap();
    old_trajectories(&fixture);
    let root = fixture.root.path().to_path_buf();
    let cutoff = (Utc::now() - chrono::Duration::days(1)).to_rfc3339();
    let env = vec![("CANON_R2_LOCAL_ROOT".to_string(), fixture.r2_root().display().to_string())];
    let change_root = root.clone();
    // A record newer than the cutoff: not in the removal set, so no effect appears, but the plan read it.
    let change: Box<dyn Fn()> = Box::new(move || {
        let git = GitTier::new(change_root.join(".canon/ledger"));
        git.write(&Trajectory::new(Envelope::new(1, RecordKind::Trajectory, Utc::now(), actor()), RunId::new(), None, None, None, None, Some(0.9))).unwrap();
    });
    Case { leaf: "purge", root, args: words(&["purge", "--kind", "trajectory", "--before", &cutoff]), env, change: Some(change), _keep: vec![Box::new(fixture)] }
}

fn tier_case() -> Case {
    let fixture = Fixture::new(ROUTING, AGING);
    old_trajectories(&fixture);
    fixture.plant_trajectory_in_git(Utc::now(), 0.9);
    let root = fixture.root.path().to_path_buf();
    let env = vec![("CANON_R2_LOCAL_ROOT".to_string(), fixture.r2_root().display().to_string())];
    let change_root = root.clone();
    let change: Box<dyn Fn()> = Box::new(move || {
        let git = GitTier::new(change_root.join(".canon/ledger"));
        git.write(&Trajectory::new(Envelope::new(1, RecordKind::Trajectory, Utc::now(), actor()), RunId::new(), None, None, None, None, Some(0.8))).unwrap();
    });
    Case { leaf: "tier age", root, args: words(&["tier", "age"]), env, change: Some(change), _keep: vec![Box::new(fixture)] }
}

fn compact_case() -> Case {
    let fixture = Fixture::new(ROUTING, AGING);
    let git = GitTier::new(fixture.git_root());
    let role = RoleId::parse("implementer").unwrap();
    // Three versions of one subject: a record is superseded by the next one of its key, and an append moves it to the archive.
    for (offset, title) in [(20, "Seeded one"), (10, "Seeded two"), (5, "Seeded three")] {
        let envelope = Envelope::new(1, RecordKind::Subject, Utc::now() - chrono::Duration::seconds(offset), actor());
        git.write(&Subject::new(envelope, SubjectId::parse("demo-subject").unwrap(), title, "s", "dev", SubjectStatus::Verifying, role.clone())).unwrap();
    }
    // A ledger written before 0.23: every version in the ledger, the archive empty.
    let archive = fixture.root.path().join(".canon/archive");
    let mut stack = vec![archive.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let target = fixture.git_root().join(path.strip_prefix(&archive).unwrap());
                std::fs::create_dir_all(target.parent().unwrap()).unwrap();
                std::fs::rename(&path, &target).unwrap();
            }
        }
    }
    let root = fixture.root.path().to_path_buf();
    let change_root = root.clone();
    // A trajectory is not a kind the archive keeps one file for: a new one changes what the pass read, and no effect.
    let change: Box<dyn Fn()> = Box::new(move || {
        let git = GitTier::new(change_root.join(".canon/ledger"));
        git.write(&Trajectory::new(Envelope::new(1, RecordKind::Trajectory, Utc::now(), actor()), RunId::new(), None, None, None, None, Some(0.8))).unwrap();
    });
    Case { leaf: "ledger compact", root, args: words(&["ledger", "compact"]), env: Vec::new(), change: Some(change), _keep: vec![Box::new(fixture)] }
}

const COLD_STANZA: &str = "  cold:\n    backend: s3\n    bucket_env: CANON_R2_BUCKET\n    prefix: \"canon/\"\n";

fn artifacts_case() -> Case {
    let dir = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    init_repo(&root, false);
    let yaml = std::fs::read_to_string(root.join("canon.yaml")).unwrap();
    let marker = "  # cold:\n  #   backend: s3\n  #   bucket_env: CANON_R2_BUCKET\n  #   prefix: \"canon/\"\n";
    assert!(yaml.contains(marker), "the skeleton's commented cold tier moved:\n{yaml}");
    std::fs::write(root.join("canon.yaml"), yaml.replace(marker, COLD_STANZA)).unwrap();
    let bytes: &[u8] = br#"{"checks":14}"#;
    let blobs = root.join(".canon/artifacts/sha256");
    std::fs::create_dir_all(&blobs).unwrap();
    std::fs::write(blobs.join(sha256_hex(bytes)), bytes).unwrap();
    let env = vec![("CANON_R2_LOCAL_ROOT".to_string(), store.path().display().to_string())];
    let change_root = root.clone();
    // Text after the block the move adds: the plan read canon.yaml and .gitignore, and lists the same two rewrites.
    let change: Box<dyn Fn()> = Box::new(move || {
        for file in ["canon.yaml", ".gitignore"] {
            let mut text = std::fs::read_to_string(change_root.join(file)).unwrap();
            text.push_str("# a line the plan did not list\n");
            std::fs::write(change_root.join(file), text).unwrap();
        }
    });
    Case { leaf: "artifacts move", root, args: words(&["artifacts", "move", "--tier", "cold"]), env, change: Some(change), _keep: vec![Box::new(dir), Box::new(store)] }
}

/// A git repo set up by `canon init` that lacks one of the managed `.gitignore` lines, so a migrate has one rewrite to make.
fn gitignore_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path(), true);
    let gitignore = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    assert!(gitignore.contains(".canon/r2/\n"), "{gitignore}");
    std::fs::write(dir.path().join(".gitignore"), gitignore.replace(".canon/r2/\n", "")).unwrap();
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "drop a managed gitignore line"]);
    dir
}

fn migrate_case() -> Case {
    let dir = gitignore_repo();
    let root = dir.path().to_path_buf();
    let change_root = root.clone();
    // The same rewrite, over a file with one more line: the digest of what the plan read changes, its effects do not.
    let change: Box<dyn Fn()> = Box::new(move || {
        let mut text = std::fs::read_to_string(change_root.join(".gitignore")).unwrap();
        text.push_str("# a line the plan did not list\n");
        std::fs::write(change_root.join(".gitignore"), text).unwrap();
        git(&change_root, &["add", "-A"]);
        git(&change_root, &["commit", "-q", "-m", "a line in .gitignore"]);
    });
    Case { leaf: "migrate", root, args: words(&["migrate"]), env: Vec::new(), change: Some(change), _keep: vec![Box::new(dir)] }
}

fn revert_case() -> Case {
    let dir = gitignore_repo();
    let root = dir.path().to_path_buf();
    let previewed = canon_in(&root, &words(&["migrate", "--json"]), &[]);
    assert!(previewed.status.success(), "{}", text(&previewed));
    let digest = json_of(&previewed)["data"]["planDigest"].as_str().unwrap().to_string();
    let committed = canon_in(&root, &words(&["migrate", "--commit", &format!("--plan-digest={digest}"), "--json"]), &[]);
    assert!(committed.status.success(), "{}", text(&committed));
    Case { leaf: "migrate", root, args: words(&["migrate", "--revert"]), env: Vec::new(), change: None, _keep: vec![Box::new(dir)] }
}

fn six() -> Vec<Case> {
    vec![purge_case(), artifacts_case(), migrate_case(), revert_case(), compact_case(), tier_case()]
}

fn is_digest(text: &str) -> bool {
    text.strip_prefix("sha256:").is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
}

fn next_argv(body: &Value) -> Vec<String> {
    body["meta"]["next"][0]["argv"].as_array().unwrap_or_else(|| panic!("no meta.next[0].argv: {body}")).iter().map(|word| word.as_str().unwrap().to_string()).collect()
}

/// atip.planjcs.01: the preview gives `sha256:` and 64 lower-case hex digits, the same plan the same digest; the call in
/// `meta.next[0]` commits it.
#[test]
fn atip_planjcs_01_the_preview_digest_is_prefixed_and_stable_and_its_next_call_commits() {
    for case in six() {
        let first = case.preview();
        let digest = first["data"]["planDigest"].as_str().unwrap().to_string();
        assert!(is_digest(&digest), "{}: {digest}", case.leaf);
        assert_eq!(first["data"]["committed"], false, "{first}");
        assert_eq!(case.preview()["data"]["planDigest"], digest.as_str(), "{}: the same plan has the same digest", case.leaf);

        let argv = next_argv(&first);
        assert!(argv.contains(&"--commit".to_string()) && argv.contains(&format!("--plan-digest={digest}")) && argv.last().is_some_and(|word| word == "--json"), "{}: {argv:?}", case.leaf);
        let before = tree(&case.root);
        let done = canon_in(&case.root, &argv, &case.env);
        assert_eq!(done.status.code(), Some(0), "{}: {}", case.leaf, text(&done));
        let body = json_of(&done);
        assert_eq!(body["data"]["committed"], true, "{}: {body}", case.leaf);
        assert_eq!(body["data"]["planDigest"], digest.as_str(), "{body}");
        assert_ne!(tree(&case.root), before, "{}: the commit made its effects", case.leaf);
    }
}

/// atip.planjcs.02: a digest without the prefix and one that differs in one character are PLAN_STALE.
#[test]
fn atip_planjcs_02_an_unprefixed_digest_and_a_changed_character_are_plan_stale_and_change_nothing() {
    for case in six() {
        let digest = case.preview()["data"]["planDigest"].as_str().unwrap().to_string();
        let hex = digest.strip_prefix("sha256:").unwrap();
        let last = hex.chars().last().unwrap();
        let flipped = format!("sha256:{}{}", &hex[..hex.len() - 1], if last == '0' { '1' } else { '0' });
        for (what, given) in [("without the prefix", hex.to_string()), ("one character changed", flipped)] {
            let before = tree(&case.root);
            let out = case.commit_with(&given);
            assert_eq!(out.status.code(), Some(2), "{} {what}: {}", case.leaf, text(&out));
            let body = json_of(&out);
            assert_eq!(body["error"]["code"], "PLAN_STALE", "{body}");
            assert_eq!(body["error"]["field"], "--plan-digest", "{body}");
            assert_eq!(tree(&case.root), before, "{} {what}: nothing was changed", case.leaf);
            let next = next_argv(&body);
            assert!(next.iter().all(|word| word != "--commit" && !word.starts_with("--plan-digest")), "{}: meta.next[0] is the preview: {next:?}", case.leaf);
            assert_eq!(next[..case.leaf_words().len()], case.leaf_words(), "{next:?}");
            assert!(body["error"]["hint"].as_str().unwrap().contains("again"), "{body}");
        }
    }
}

/// atip.planjcs.03: the digest is the SHA-256 of the JCS of `{effects, inputs: {arguments, reads}}`.
#[test]
fn atip_planjcs_03_the_digest_is_the_sha256_of_the_jcs_of_the_effects_and_the_inputs() {
    let fixture = Fixture::new(ROUTING, AGING);
    std::fs::create_dir_all(fixture.root.path().join(".canon/learn")).unwrap();
    let root = fixture.root.path();
    let yaml = std::fs::read(root.join("canon.yaml")).unwrap();
    let out = canon_in(root, &words(&["purge", "--kind", "trajectory", "--before", "2020-01-01", "--json"]), &[]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    let body = json_of(&out);
    // The fixture's cold tier is not attached: the plan skips it (an effect), and has nothing to delete.
    let effects = body["data"]["effects"].clone();
    assert_eq!(effects.as_array().unwrap().iter().map(|effect| effect["action"].as_str().unwrap()).collect::<Vec<_>>(), ["skip"], "{body}");
    let nothing = format!("sha256:{}", sha256_hex(b""));
    let plan = json!({
        "effects": effects,
        "inputs": {
            "arguments": {"before": "2020-01-01", "kind": "trajectory"},
            "reads": {"canon.yaml": format!("sha256:{}", sha256_hex(&yaml)), "learn-records": nothing, "records": nothing}
        }
    });
    let canonical = atip::jcs::to_string(&plan).unwrap();
    assert_eq!(body["data"]["planDigest"], format!("sha256:{}", sha256_hex(canonical.as_bytes())).as_str(), "{canonical}\n{body}");

    // The effects are in the digest: one record to purge is another digest, and recomputed by hand it is that one.
    fixture.plant_trajectory_in_git(Utc::now() - chrono::Duration::days(5), 0.1);
    let with_one = json_of(&canon_in(root, &words(&["purge", "--kind", "trajectory", "--before", "2020-01-01", "--json"]), &[]));
    assert_eq!(with_one["data"]["effects"], body["data"]["effects"], "the record is newer than the cutoff: not an effect");
    assert_ne!(with_one["data"]["planDigest"], body["data"]["planDigest"], "but the plan read it: the digest changed");
}

/// atip.planjcs.04: the same arguments and inputs commit; one argument changed is another digest, and a digest of another
/// call is PLAN_STALE.
#[test]
fn atip_planjcs_04_the_arguments_are_in_the_digest() {
    let case = purge_case();
    let first = case.preview();
    let digest = first["data"]["planDigest"].as_str().unwrap().to_string();
    // Another cutoff: another argument, the same records, the same effects.
    let other_cutoff = (Utc::now() - chrono::Duration::days(2)).to_rfc3339();
    let mut other_args = case.args.clone();
    *other_args.last_mut().unwrap() = other_cutoff;
    let other = |extra: &[&str]| {
        let mut args = other_args.clone();
        args.extend(extra.iter().map(ToString::to_string));
        args.push("--json".to_string());
        canon_in(&case.root, &args, &case.env)
    };
    let other_preview = json_of(&other(&[]));
    assert_ne!(other_preview["data"]["planDigest"], first["data"]["planDigest"], "one argument changed: another digest");

    let before = tree(&case.root);
    let wrong = other(&["--commit", &format!("--plan-digest={digest}")]);
    assert_eq!(wrong.status.code(), Some(2), "{}", text(&wrong));
    assert_eq!(json_of(&wrong)["error"]["code"], "PLAN_STALE");
    assert_eq!(tree(&case.root), before, "the digest of another call changed nothing");

    let done = case.commit_with(&digest);
    assert_eq!(done.status.code(), Some(0), "the same arguments and the same inputs commit: {}", text(&done));
    assert_eq!(json_of(&done)["data"]["committed"], true);
}

/// atip.planjcs.05: a file or record the plan read, changed after the preview, makes the commit PLAN_STALE.
#[test]
fn atip_planjcs_05_a_change_to_what_the_plan_read_is_plan_stale_though_its_effects_are_the_same() {
    for case in six().into_iter().filter(|case| case.change.is_some()) {
        let preview = case.preview();
        let digest = preview["data"]["planDigest"].as_str().unwrap().to_string();
        (case.change.as_ref().unwrap())();
        let after = case.preview();
        assert_eq!(after["data"]["effects"], preview["data"]["effects"], "{}: the effects are what they were", case.leaf);
        assert_ne!(after["data"]["planDigest"], preview["data"]["planDigest"], "{}: what the plan read is in its digest", case.leaf);
        let before = tree(&case.root);
        let out = case.commit_with(&digest);
        assert_eq!(out.status.code(), Some(2), "{}: {}", case.leaf, text(&out));
        assert_eq!(json_of(&out)["error"]["code"], "PLAN_STALE", "{}", case.leaf);
        assert_eq!(tree(&case.root), before, "{}: nothing was written", case.leaf);
        assert!(next_argv(&json_of(&out)).iter().all(|word| word != "--commit"), "{}: meta.next[0] is the preview", case.leaf);
    }
}

fn lock_file(root: &Path) -> PathBuf {
    root.join(LOCK)
}

/// Waits until `child` holds the repository write lock: the lock file is there and another process cannot take it. A child that
/// ends first never held it, and the test fails (a commit that does not take the lock is the failure this proves).
fn wait_until_held(root: &Path, child: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if lock_file(root).exists() && FileLock::acquire(&lock_file(root), Duration::ZERO).is_err() {
            return;
        }
        if let Some(status) = child.try_wait().unwrap() {
            panic!("the run ended ({status}) without the repository write lock ever being held");
        }
        assert!(Instant::now() < deadline, "nobody took the lock");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn spawn(root: &Path, args: &[String], env: &[(&str, &str)]) -> Child {
    let mut command = Command::new(env!("CARGO_BIN_EXE_canon"));
    command.args(args).current_dir(root).env_remove("CANON_ROLE").env_remove("CANON_TEST_REPO_LOCK_WAIT_MS").env_remove("CANON_TEST_HOLD_AFTER_RUN_MS").env("CANON_ACTOR", "canon").stdout(Stdio::piped()).stderr(Stdio::piped());
    for (key, value) in env {
        command.env(key, value);
    }
    command.spawn().expect("spawn canon")
}

/// A repo set up by `canon init` and a commit it can make: a purge of a kind that has no record.
fn lock_repo() -> (tempfile::TempDir, Vec<String>, String) {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path(), true);
    let args = words(&["purge", "--kind", "session", "--before", "2020-01-01"]);
    let mut preview = args.clone();
    preview.push("--json".to_string());
    let body = json_of(&canon_in(dir.path(), &preview, &[]));
    let digest = body["data"]["planDigest"].as_str().unwrap().to_string();
    (dir, args, digest)
}

fn commit_args(args: &[String], digest: &str) -> Vec<String> {
    let mut all = args.to_vec();
    all.extend(["--commit".to_string(), format!("--plan-digest={digest}"), "--json".to_string()]);
    all
}

/// atip.planjcs.06: a commit holds the lock for its whole run; another writer waits and writes after it; a preview does not wait.
#[test]
fn atip_planjcs_06_a_writer_waits_for_a_commit_and_a_preview_does_not() {
    let (dir, args, digest) = lock_repo();
    let root = dir.path();
    // The commit keeps the lock for 3s after its last effect (a seam of the debug build), as a long commit would.
    let mut commit = spawn(root, &commit_args(&args, &digest), &[("CANON_TEST_HOLD_AFTER_RUN_MS", "3000")]);
    wait_until_held(root, &mut commit);

    let started = Instant::now();
    let mut preview = args.clone();
    preview.push("--json".to_string());
    let seen = canon_in(root, &preview, &[]);
    assert_eq!(seen.status.code(), Some(0), "{}", text(&seen));
    assert!(commit.try_wait().unwrap().is_none(), "the preview did not wait for the commit: it was done while the commit still ran ({:?})", started.elapsed());

    let writer = canon_in(root, &words(&["subject", "new", "demo", "--domain", "dev", "--title", "Demo", "--json"]), &[]);
    assert_eq!(writer.status.code(), Some(0), "{}", text(&writer));
    let ended = commit.try_wait().unwrap();
    assert!(ended.is_some_and(|status| status.success()), "the writer waited for the commit: the commit had ended when the writer did");
    assert!(root.join(".canon/ledger/kind=subject").is_dir(), "and the writer wrote after it");
}

/// atip.planjcs.07: a commit whose wait runs out starts nothing; a killed holder releases the lock and the file stays.
#[test]
fn atip_planjcs_07_a_wait_that_runs_out_refuses_and_a_killed_holder_releases_the_lock() {
    let (dir, args, digest) = lock_repo();
    let root = dir.path();
    let held = FileLock::acquire(&lock_file(root), Duration::ZERO).unwrap();
    let before = tree(root);
    let out = canon_in(root, &commit_args(&args, &digest), &[("CANON_TEST_REPO_LOCK_WAIT_MS".to_string(), "300".to_string())]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    let body = json_of(&out);
    assert_eq!((body["error"]["code"].as_str(), body["error"]["retryable"].as_bool()), (Some("REPO_WRITE_BUSY"), Some(true)), "{body}");
    assert!(body["error"]["hint"].as_str().unwrap().contains("again in a moment"), "{body}");
    assert_eq!(tree(root), before, "a commit that waited in vain started nothing");
    drop(held);

    // A holder that dies without releasing: the next commit proceeds, and the lock file is still there.
    let mut holder = spawn(root, &commit_args(&args, &digest), &[("CANON_TEST_HOLD_AFTER_RUN_MS", "120000")]);
    wait_until_held(root, &mut holder);
    holder.kill().unwrap();
    holder.wait().unwrap();
    assert!(lock_file(root).is_file(), "the lock file is never deleted");
    let again = canon_in(root, &commit_args(&args, &digest), &[("CANON_TEST_REPO_LOCK_WAIT_MS".to_string(), "20000".to_string())]);
    assert_eq!(again.status.code(), Some(0), "{}", text(&again));
    assert_eq!(json_of(&again)["data"]["committed"], true);
}

/// atip.planjcs.08: the readers, the dry runs and the previews do not take the lock; a writer does and waits.
#[test]
fn atip_planjcs_08_only_a_run_that_writes_takes_the_lock() {
    // Every leaf of `canon schema` that writes has a row of the policy with its reason; a leaf without one is listed here.
    assert_eq!(canon_cli::write_lock::unjudged(), Vec::<&str>::new(), "a leaf that writes must be judged in `write_lock::POLICY`");
    assert!(canon_cli::write_lock::POLICY.iter().all(|(path, _, reason)| !path.is_empty() && !reason.is_empty()), "every row has its reason");
    let (dir, args, _) = lock_repo();
    let root = dir.path();
    let _held = FileLock::acquire(&lock_file(root), Duration::ZERO).unwrap();
    let short = [("CANON_TEST_REPO_LOCK_WAIT_MS".to_string(), "200".to_string())];
    let mut preview = args.clone();
    preview.push("--json".to_string());
    for (what, call) in [("a read", words(&["status", "--json"])), ("a preview", preview), ("a dry run", words(&["inventory", "sync", "--dry-run", "--json"])), ("init --check", words(&["init", "--check", "--json"]))] {
        let out = canon_in(root, &call, &short);
        assert_ne!(json_of(&out)["error"]["code"], "REPO_WRITE_BUSY", "{what} does not wait for the lock: {}", text(&out));
    }
    let writer = canon_in(root, &words(&["subject", "new", "demo", "--domain", "dev", "--title", "Demo", "--json"]), &short);
    assert_eq!(json_of(&writer)["error"]["code"], "REPO_WRITE_BUSY", "a writer waits for the lock: {}", text(&writer));
    assert_eq!(writer.status.code(), Some(2));
    assert!(!root.join(".canon/ledger/kind=subject").exists(), "and wrote nothing");
    drop(_held);

    // A run that polls holds the lock for each pass, not for the polling: a writer is not blocked by a watcher that is running.
    let mut watcher = spawn(root, &words(&["ingest", "records", "--watch", "--interval-secs", "1", "--json"]), &[]);
    std::thread::sleep(Duration::from_millis(2500));
    assert!(watcher.try_wait().unwrap().is_none(), "the watcher is still polling");
    let writer = canon_in(root, &words(&["subject", "new", "demo", "--domain", "dev", "--title", "Demo", "--json"]), &[]);
    let still_polling = watcher.try_wait().unwrap().is_none();
    watcher.kill().unwrap();
    watcher.wait().unwrap();
    assert_eq!(writer.status.code(), Some(0), "a watcher does not hold the lock between its passes: {}", text(&writer));
    assert!(still_polling, "and the writer was served while the watcher polled");
}

fn audit_lines(root: &Path) -> Vec<Value> {
    std::fs::read_to_string(root.join(".canon/audit/access.jsonl")).unwrap_or_default().lines().map(|line| serde_json::from_str(line).unwrap()).collect()
}

/// A commit that pauses between the check of its digest and its first effect, with another writer let in at the start of that
/// span: whether the writer is served before the commit ends tells whether the lock covers the span. Returns the commit's exit
/// status and whether it had ended when the writer was answered.
fn writer_during_the_span(root: &Path, args: &[String], digest: &str) -> (std::process::ExitStatus, bool, Output) {
    let mut commit = spawn(root, &commit_args(args, digest), &[("CANON_TEST_PAUSE_AFTER_GATE_MS", "3000")]);
    wait_until_held(root, &mut commit);
    let writer = canon_in(root, &words(&["subject", "new", "demo", "--domain", "dev", "--title", "Demo", "--json"]), &[]);
    let ended = commit.try_wait().unwrap();
    let status = ended.unwrap_or_else(|| commit.wait().unwrap());
    (status, ended.is_some(), writer)
}

/// atip.planjcs.14: the lock covers the span from the check of the digest to the first effect: a writer that arrives while a
/// commit sits between them waits for it.
#[test]
fn atip_planjcs_14_the_lock_covers_the_span_between_the_check_and_the_first_effect() {
    let (dir, args, digest) = lock_repo();
    let root = dir.path();
    let (status, ended, writer) = writer_during_the_span(root, &args, &digest);
    assert!(status.success(), "the commit committed");
    assert_eq!(writer.status.code(), Some(0), "{}", text(&writer));
    assert!(ended, "the writer was served only after the commit had ended: it did not enter the span between the check and the effects");
    assert!(root.join(".canon/ledger/kind=subject").is_dir(), "and then wrote");
}

/// atip.planjcs.15: a repository with no lock file (set up by 0.34, or never initialised by this canon) is locked from its first
/// call: the file is made, kept out of git without a tracked file edited, and a writer waits.
#[test]
fn atip_planjcs_15_a_repository_without_the_lock_file_is_locked_from_its_first_call() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root, true);
    // A 0.34 repository: no lock file, no line for it in .gitignore.
    let _ = std::fs::remove_file(root.join(LOCK));
    let ignore = std::fs::read_to_string(root.join(".gitignore")).unwrap();
    let ignore: String = ignore.lines().filter(|line| !line.contains("repo-write.lock")).map(|line| format!("{line}\n")).collect();
    std::fs::write(root.join(".gitignore"), &ignore).unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "a 0.34 repository"]);
    let args = words(&["purge", "--kind", "session", "--before", "2020-01-01"]);
    let mut preview = args.clone();
    preview.push("--json".to_string());
    let digest = json_of(&canon_in(root, &preview, &[]))["data"]["planDigest"].as_str().unwrap().to_string();
    assert!(!root.join(LOCK).exists(), "a preview takes no lock and makes no file");

    let (status, ended, writer) = writer_during_the_span(root, &args, &digest);
    assert!(status.success());
    assert_eq!(writer.status.code(), Some(0), "{}", text(&writer));
    assert!(ended, "the first commit of a repository with no lock file held the lock: the writer waited for it");
    assert!(root.join(LOCK).is_file(), "the first call made the lock file");
    let status = Command::new("git").args(["status", "--porcelain", "--", ".canon/repo-write.lock"]).current_dir(root).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&status.stdout), "", "and kept it out of git status");
    assert!(std::fs::read_to_string(root.join(".git/info/exclude")).unwrap().contains("/.canon/repo-write.lock"), "by the exclude file of the checkout");
    assert_eq!(std::fs::read_to_string(root.join(".gitignore")).unwrap(), ignore, "without editing a tracked file");
}

/// atip.planjcs.09: the purge preview lists its audit line, the commit writes that file and that line; nothing to delete, neither.
#[test]
fn atip_planjcs_09_the_purge_preview_lists_the_audit_line_the_commit_writes() {
    let case = purge_case();
    let preview = case.preview();
    let effects = preview["data"]["effects"].as_array().unwrap();
    assert_eq!(effects.len(), 3, "two deletes and the audit line: {preview}");
    let audit = effects.last().unwrap();
    assert_eq!(audit["action"], "append", "{preview}");
    assert_eq!(audit["path"], ".canon/audit/access.jsonl", "{preview}");
    assert_eq!(audit["line"]["operation"], "purge", "{preview}");
    assert!(effects[..2].iter().all(|effect| effect["action"] == "delete"), "{preview}");
    assert!(audit_lines(&case.root).is_empty(), "the preview wrote no audit line");

    let digest = preview["data"]["planDigest"].as_str().unwrap().to_string();
    let done = case.commit_with(&digest);
    assert_eq!(done.status.code(), Some(0), "{}", text(&done));
    let lines = audit_lines(&case.root);
    assert_eq!(lines.len(), 1, "one audit line: {lines:?}");
    let line = &lines[0];
    assert!(case.root.join(audit["path"].as_str().unwrap()).is_file());
    for key in ["operation", "kind", "scope", "sensitive", "result_counts"] {
        assert_eq!(line[key], audit["line"][key], "{key}: the line written is the line the effect lists: {line} / {audit}");
    }
    assert_eq!(line["result_counts"], json!({"matched": 2, "deleted": 2}));

    // Nothing before the cutoff: no audit effect, and the commit writes no line.
    let empty = purge_case();
    let mut args = empty.args.clone();
    *args.last_mut().unwrap() = "2000-01-01".to_string();
    let run = |extra: &[&str]| {
        let mut all = args.clone();
        all.extend(extra.iter().map(ToString::to_string));
        all.push("--json".to_string());
        canon_in(&empty.root, &all, &empty.env)
    };
    let nothing = json_of(&run(&[]));
    assert_eq!(nothing["data"]["effects"], json!([]), "{nothing}");
    let digest = nothing["data"]["planDigest"].as_str().unwrap().to_string();
    let done = run(&["--commit", &format!("--plan-digest={digest}")]);
    assert_eq!(done.status.code(), Some(0), "{}", text(&done));
    assert!(audit_lines(&empty.root).is_empty(), "a purge that deleted nothing writes no audit line");
}

/// atip.planjcs.10: a preview and a refused commit write nothing.
#[test]
fn atip_planjcs_10_a_purge_preview_and_a_refused_commit_leave_the_tree_as_it_was() {
    let case = purge_case();
    let before = tree(&case.root);
    let preview = case.preview();
    assert_eq!(tree(&case.root), before, "the preview wrote nothing, the audit line included");
    let digest = preview["data"]["planDigest"].as_str().unwrap().to_string();
    let stale = case.commit_with(&format!("{digest}0"));
    assert_eq!(stale.status.code(), Some(2), "{}", text(&stale));
    assert_eq!(tree(&case.root), before, "the refused commit wrote nothing");
}

/// The digest 0.34 made: the SHA-256 of the JSON of the effects, as hex, with no prefix.
fn digest_034(effects: &Value) -> String {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct Effect034 {
        action: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        record: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    }
    let effects: Vec<Effect034> = serde_json::from_value(effects.clone()).unwrap();
    sha256_hex(&serde_json::to_vec(&effects).unwrap())
}

/// atip.planjcs.11: a digest made the 0.34 way is PLAN_STALE, and the hint says to run the preview again.
#[test]
fn atip_planjcs_11_a_digest_of_0_34_is_plan_stale_on_the_six_commands() {
    for case in six() {
        let preview = case.preview();
        let old = digest_034(&preview["data"]["effects"]);
        assert_eq!(old.len(), 64);
        let before = tree(&case.root);
        let out = case.commit_with(&old);
        assert_eq!(out.status.code(), Some(2), "{}: {}", case.leaf, text(&out));
        let body = json_of(&out);
        assert_eq!(body["error"]["code"], "PLAN_STALE", "{body}");
        assert_eq!(tree(&case.root), before, "{}: nothing was changed", case.leaf);
        let hint = body["error"]["hint"].as_str().unwrap();
        assert!(hint.contains("again") && hint.contains(case.leaf), "{}: the hint says to plan again: {hint}", case.leaf);
    }
}

/// atip.planjcs.12: the CHANGELOG has the upgrade note, and no source makes a digest without the prefix.
#[test]
fn atip_planjcs_12_the_changelog_has_the_upgrade_note_and_no_source_makes_the_old_digest() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let changelog = std::fs::read_to_string(root.join("CHANGELOG.md")).unwrap();
    let unreleased = changelog.split("\n## ").nth(1).expect("an Unreleased section");
    for needle in ["canon-0-35-plan-jcs", "sha256:", "JCS", "PLAN_STALE", "REPO_WRITE_BUSY", ".canon/repo-write.lock", "0.34", "inputs", "atip.planjcs.01"] {
        assert!(unreleased.contains(needle), "the note of this change names `{needle}`");
    }
    let safety = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/safety.rs")).unwrap();
    let code = safety.split("#[cfg(test)]").next().unwrap();
    assert!(!code.contains("fn digest(effects") && !code.contains("to_vec(effects") && !code.contains("to_vec(&effects"), "no code hashes the effects alone");
    assert!(code.contains("DIGEST_PREFIX"), "every digest carries the prefix the crate names");
    for entry in std::fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("src")).unwrap().flatten() {
        if entry.path().extension().is_some_and(|ext| ext == "rs") && entry.file_name() != "safety.rs" {
            let source = std::fs::read_to_string(entry.path()).unwrap();
            assert!(!source.contains("fn digest(effects"), "{}", entry.path().display());
        }
    }
}

/// atip.planjcs.13: the report schemas take their preview members from the contract's Effect and raise the minor.
#[test]
fn atip_planjcs_13_the_report_schemas_hold_the_preview_and_their_minor_is_raised() {
    for command in ["purge", "artifacts move", "migrate", "ledger compact", "tier age"] {
        let mut args = words(&["schema"]);
        args.extend(command.split(' ').map(ToString::to_string));
        args.push("--json".to_string());
        let out = canon_in(Path::new("."), &args, &[]);
        let body = json_of(&out);
        let doc = &body["data"];
        let report = doc["outputs"][0]["schemaVersion"].as_str().unwrap();
        let minor: u32 = report.split('.').nth(1).unwrap().parse().unwrap();
        assert!(minor >= 1 && report.starts_with("1."), "{command}: {report}");
        let name = doc["outputs"][0]["schemaRef"].as_str().unwrap().rsplit('/').next().unwrap();
        let schema = &doc["schemas"][name];
        for member in ["planDigest", "effects", "committed"] {
            assert!(schema["properties"].get(member).is_some(), "{command} has {member}: {schema}");
        }
        assert_eq!(schema["properties"]["effects"]["items"]["$ref"], "#/schemas/Effect", "{command}");
        let effect = &doc["schemas"]["Effect"]["properties"];
        assert!(["action", "path", "record", "detail"].iter().all(|key| effect.get(key).is_some()), "{command}: {effect}");
    }
}
