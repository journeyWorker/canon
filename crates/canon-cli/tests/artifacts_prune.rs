//! artifacts.prune.01–14 (0.38) through the real binary: `canon artifacts prune` deletes the blobs no record binds any more. The bare run
//! previews (what goes, what stays and why, one page of the list) and writes nothing; `--commit --plan-digest` deletes exactly that plan,
//! in the local store and, by the same rule, in an object-store tier.
//!
//! The object store is the debug-build local-filesystem substitute (`CANON_R2_LOCAL_ROOT`, the seam the tier tests use), so these run offline.

#[path = "support/hermetic.rs"]
mod hermetic;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use sha2::{Digest, Sha256};

struct Repo {
    dir: tempfile::TempDir,
    /// The object store's root: what a bucket holds.
    store: tempfile::TempDir,
}

impl Repo {
    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = hermetic::canon();
        command.args(args).env("CANON_ACTOR", "canon").env("CANON_R2_LOCAL_ROOT", self.store.path()).current_dir(self.path());
        command
    }

    fn canon(&self, args: &[&str]) -> Output {
        self.command(args).output().expect("spawn canon")
    }

    fn canon_env(&self, args: &[&str], env: &[(&str, &str)]) -> Output {
        let mut command = self.command(args);
        for (key, value) in env {
            command.env(key, value);
        }
        command.output().expect("spawn canon")
    }

    /// `canon` with the substitute store removed and a real-looking S3 configuration instead (what a release binary always uses).
    fn canon_s3(&self, args: &[&str], env: &[(&str, &str)]) -> Output {
        let mut command = hermetic::canon();
        command.args(args).env("CANON_ACTOR", "canon").env_remove("CANON_R2_LOCAL_ROOT").env_remove("CANON_R2_BUCKET").env_remove("S3_BUCKET").current_dir(self.path());
        for (key, value) in env {
            command.env(key, value);
        }
        command.output().expect("spawn canon")
    }

    fn ok(&self, args: &[&str]) -> Output {
        let out = self.canon(args);
        assert!(out.status.success(), "canon {args:?}: {}", text(&out));
        out
    }

    /// The preview of `artifacts prune` with `extra` options, as JSON.
    fn preview(&self, extra: &[&str]) -> Value {
        let mut args = vec!["artifacts", "prune"];
        args.extend_from_slice(extra);
        args.push("--json");
        let out = self.canon(&args);
        assert!(out.status.success(), "canon {args:?}: {}", text(&out));
        let body = json(&out);
        assert_eq!(body["data"]["committed"], false, "{body}");
        body
    }

    /// `--commit --plan-digest <D>` of the plan `preview` printed, with `extra` options: the command line `meta.next[0]` gives, run as given.
    fn commit(&self, preview: &Value) -> Output {
        let argv: Vec<String> = preview["meta"]["next"][0]["argv"].as_array().expect("the preview names the commit").iter().map(|word| word.as_str().unwrap().to_string()).collect();
        let args: Vec<&str> = argv.iter().map(String::as_str).collect();
        self.canon(&args)
    }

    fn blob(&self, bytes: &[u8]) -> PathBuf {
        self.path().join(".canon/artifacts/sha256").join(sha(bytes))
    }

    /// The object a bucket holds for `bytes`.
    fn object(&self, bytes: &[u8]) -> PathBuf {
        self.store.path().join("artifacts/sha256").join(sha(bytes))
    }

    /// A blob in the local store that no record binds.
    fn orphan(&self, bytes: &[u8]) {
        std::fs::create_dir_all(self.path().join(".canon/artifacts/sha256")).unwrap();
        std::fs::write(self.blob(bytes), bytes).unwrap();
    }

    /// An object in the bucket that no record binds.
    fn orphan_object(&self, bytes: &[u8]) {
        std::fs::create_dir_all(self.store.path().join("artifacts/sha256")).unwrap();
        std::fs::write(self.object(bytes), bytes).unwrap();
    }

    fn report(&self, bytes: &[u8]) {
        std::fs::create_dir_all(self.path().join("reports")).unwrap();
        std::fs::write(self.path().join("reports/smoke.json"), bytes).unwrap();
    }

    fn attest_args(extra: &[&str]) -> Vec<String> {
        let mut args: Vec<&str> = vec!["evidence", "add", "--scenario", "game.run.01", "--project", "root", "--class", "test-run", "--ref", "npm run smoke", "--role", "implementer", "--artifact", "reports/smoke.json"];
        args.extend_from_slice(extra);
        args.into_iter().map(str::to_string).collect()
    }

    /// Writes `bytes` as the report and binds it to the scenario's evidence; `promote` makes it the ledger's.
    fn attest(&self, bytes: &[u8], promote: bool) {
        self.report(bytes);
        // `canon init` writes `promote: auto`: `--no-promote` keeps the record staged.
        let args = Self::attest_args(if promote { &[] } else { &["--no-promote"] });
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        self.ok(&args);
        if promote {
            self.ok(&["gate", "promote"]);
        }
    }

    fn files(&self) -> BTreeMap<String, Vec<u8>> {
        fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
            for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.file_name().is_some_and(|name| name == ".git") {
                    continue;
                }
                if path.is_dir() {
                    walk(root, &path, out);
                } else {
                    out.insert(path.strip_prefix(root).unwrap().display().to_string(), std::fs::read(&path).unwrap_or_default());
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(self.path(), self.path(), &mut out);
        out
    }

    fn local_blobs(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.path().join(".canon/artifacts/sha256")).into_iter().flatten().flatten().map(|entry| entry.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }
}

fn text(output: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr))
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {}", text(output)))
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn commit_all(repo: &Path) {
    for args in [&["config", "user.name", "t"][..], &["config", "user.email", "t@example.com"], &["add", "-A"], &["commit", "-q", "-m", "x"]] {
        assert!(Command::new("git").args(args).current_dir(repo).status().unwrap().success());
    }
}

const COLD_STANZA: &str = "  cold:\n    backend: s3\n    bucket_env: CANON_R2_BUCKET\n    prefix: \"canon/\"\n";
const ARTIFACTS_COLD: &str = "artifact_store:\n  backend: cold\n  prefix: artifacts/\n";

/// `git init` + `canon init --no-policy` + the scenario `game.run.01`, synced, with the `cold` tier declared. `artifacts` is appended to
/// canon.yaml when given (the object-store backend).
fn new_repo(artifacts: Option<&str>) -> Repo {
    let repo = Repo { dir: tempfile::tempdir().unwrap(), store: tempfile::tempdir().unwrap() };
    assert!(Command::new("git").args(["init", "-q"]).current_dir(repo.path()).status().unwrap().success());
    repo.ok(&["init", "--repo", ".", "--no-policy"]);
    commit_all(repo.path());
    let yaml = std::fs::read_to_string(repo.path().join("canon.yaml")).unwrap();
    let marker = "  # cold:\n  #   backend: s3\n  #   bucket_env: CANON_R2_BUCKET\n  #   prefix: \"canon/\"\n";
    assert!(yaml.contains(marker), "the skeleton's commented cold tier moved:\n{yaml}");
    let mut yaml = yaml.replace(marker, COLD_STANZA);
    if let Some(block) = artifacts {
        yaml.push_str(block);
    }
    std::fs::write(repo.path().join("canon.yaml"), yaml).unwrap();
    repo.ok(&["scenario", "new", "game.run.01", "--title", "A run ends at zero health", "--case", "failure"]);
    repo.ok(&["inventory", "sync"]);
    repo
}

const V1: &[u8] = br#"{"at":"2026-10-10T14:00:00Z","checks":16}"#;
const V2: &[u8] = br#"{"at":"2026-10-11T14:00:00Z","checks":17}"#;
const V3: &[u8] = br#"{"at":"2026-10-12T14:00:00Z","checks":18}"#;
const O1: &[u8] = b"an orphan report, first";
const O2: &[u8] = b"an orphan report, second, a little longer";
const O3: &[u8] = b"an orphan report, third";

fn digests(items: &Value) -> Vec<String> {
    items.as_array().unwrap().iter().map(|item| item["sha256"].as_str().unwrap().to_string()).collect()
}

/// The next step that carries `--commit`: none when there is nothing to delete.
fn commit_steps(body: &Value) -> usize {
    body["meta"]["next"].as_array().map_or(0, |next| next.iter().filter(|step| step["argv"].as_array().is_some_and(|argv| argv.iter().any(|word| word == "--commit"))).count())
}

/// artifacts.prune.01: a store of three blobs, one the latest evidence binds and two no record binds. The preview says two go and one stays, and
/// why; it exits 0, says `committed: false`, offers the commit as the first next step and changes no file.
#[test]
fn artifacts_prune_01_the_preview_names_what_goes_and_what_stays_and_writes_nothing() {
    let repo = new_repo(None);
    repo.attest(V1, true);
    repo.orphan(O1);
    repo.orphan(O2);
    let before = repo.files();

    let body = repo.preview(&[]);
    let data = &body["data"];
    assert_eq!(data["backend"], "local");
    assert_eq!((data["delete"]["blobs"].as_u64(), data["delete"]["bytes"].as_u64()), (Some(2), Some((O1.len() + O2.len()) as u64)), "{body}");
    assert_eq!((data["keep"]["blobs"].as_u64(), data["keep"]["bytes"].as_u64()), (Some(1), Some(V1.len() as u64)), "{body}");
    assert_eq!(data["keep"]["reasons"]["latest-evidence"], 1, "{body}");
    assert_eq!(data["keep"]["items"][0]["sha256"], sha(V1));
    assert_eq!(data["keep"]["items"][0]["reasons"], serde_json::json!(["latest-evidence"]));
    let mut gone = vec![sha(O1), sha(O2)];
    gone.sort();
    assert_eq!(digests(&data["blobs"]), gone);
    assert_eq!(data["deleted"]["blobs"], 0);
    assert!(data["gitNote"].as_str().unwrap().contains("git history"), "{body}");
    assert_eq!(body["meta"]["page"]["total"], 2);

    let first = &body["meta"]["next"][0];
    let argv: Vec<&str> = first["argv"].as_array().unwrap().iter().map(|word| word.as_str().unwrap()).collect();
    assert_eq!(&argv[..3], ["artifacts", "prune", "--commit"], "{body}");
    assert!(argv.iter().any(|word| *word == format!("--plan-digest={}", data["planDigest"].as_str().unwrap())), "{body}");
    assert_eq!(repo.files(), before, "a preview writes nothing");

    let human = repo.canon(&["artifacts", "prune"]);
    assert_eq!(human.status.code(), Some(0), "{}", text(&human));
    let said = text(&human);
    assert!(said.contains("delete 2 blob(s)") && said.contains("keep 1 blob(s)") && said.contains("keep 1 for latest-evidence") && said.contains("--commit --plan-digest"), "{said}");
    assert_eq!(repo.files(), before);
}

/// artifacts.prune.02: a store with nothing to delete. The preview says so, offers no `--commit` command and writes nothing; committing its
/// digest anyway deletes nothing.
#[test]
fn artifacts_prune_02_nothing_to_delete_is_said_and_offers_no_commit() {
    let repo = new_repo(None);
    repo.attest(V1, true);
    let before = repo.files();

    let body = repo.preview(&[]);
    assert_eq!(body["data"]["delete"]["blobs"], 0, "{body}");
    assert_eq!(body["data"]["effects"], serde_json::json!([]), "{body}");
    assert_eq!(commit_steps(&body), 0, "no commit command when nothing goes: {body}");
    let human = text(&repo.canon(&["artifacts", "prune"]));
    assert!(human.contains("nothing to delete") && !human.contains("--commit"), "{human}");

    let digest = body["data"]["planDigest"].as_str().unwrap();
    let out = repo.canon(&["artifacts", "prune", "--commit", &format!("--plan-digest={digest}"), "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    assert_eq!(json(&out)["data"]["deleted"]["blobs"], 0);
    assert_eq!(repo.local_blobs(), vec![sha(V1)]);
    assert_eq!(repo.files(), before);

    let empty = new_repo(None);
    let body = empty.preview(&[]);
    assert_eq!((body["data"]["delete"]["blobs"].as_u64(), body["data"]["keep"]["blobs"].as_u64()), (Some(0), Some(0)), "{body}");
    assert_eq!(commit_steps(&body), 0);
}

/// artifacts.prune.03: the list of blobs to delete is cut at `--limit` and continues by `meta.page.nextCursor`; the totals count the whole, the
/// first next step is still the commit of the whole plan (no paging option in it) and the plan digest is the same on every page.
#[test]
fn artifacts_prune_03_the_list_is_cut_at_the_limit_and_continues_by_cursor() {
    let repo = new_repo(None);
    repo.attest(V1, true);
    for orphan in [O1, O2, O3] {
        repo.orphan(orphan);
    }
    let mut all = vec![sha(O1), sha(O2), sha(O3)];
    all.sort();

    let first = repo.preview(&["--limit", "2"]);
    assert_eq!(first["data"]["delete"]["blobs"], 3, "the totals count the whole list: {first}");
    assert_eq!(digests(&first["data"]["blobs"]), all[..2].to_vec(), "{first}");
    assert_eq!((first["meta"]["page"]["returned"].as_u64(), first["meta"]["page"]["total"].as_u64()), (Some(2), Some(3)), "{first}");
    let cursor = first["meta"]["page"]["nextCursor"].as_str().expect("more blobs follow").to_string();
    let commit: Vec<&str> = first["meta"]["next"][0]["argv"].as_array().unwrap().iter().map(|word| word.as_str().unwrap()).collect();
    assert!(commit.contains(&"--commit") && !commit.iter().any(|word| word.starts_with("--limit") || word.starts_with("--cursor")), "the commit is of the whole plan: {commit:?}");
    assert!(first["meta"]["next"][1]["argv"].as_array().unwrap().iter().any(|word| word.as_str().unwrap().starts_with("--cursor")), "{first}");

    let second = repo.preview(&["--limit", "2", "--cursor", &cursor]);
    assert_eq!(digests(&second["data"]["blobs"]), all[2..].to_vec(), "{second}");
    assert!(second["meta"]["page"]["nextCursor"].is_null(), "{second}");
    assert_eq!(second["data"]["planDigest"], first["data"]["planDigest"]);

    let everything = repo.preview(&["--all"]);
    assert_eq!(digests(&everything["data"]["blobs"]), all);

    // The commit that came with the first page deletes all three.
    let done = repo.commit(&first);
    assert_eq!(done.status.code(), Some(0), "{}", text(&done));
    assert_eq!(json(&done)["data"]["deleted"]["blobs"], 3);
    assert_eq!(repo.local_blobs(), vec![sha(V1)]);
}

fn state(repo: &Repo) -> (Output, Output) {
    (repo.canon(&["artifacts", "verify", "--json"]), repo.canon(&["gate", "check", "--json"]))
}

/// artifacts.prune.04: the digest of the preview deletes the blobs no record binds (a superseded record's among them); `artifacts verify` and
/// `gate check` answer as they did, and what was deleted is told in counts and bytes with the git-history note.
#[test]
fn artifacts_prune_04_committing_the_plan_deletes_the_unbound_blobs_and_verify_and_the_gate_answer_as_before() {
    let repo = new_repo(None);
    repo.attest(V1, true);
    repo.attest(V2, true);
    repo.orphan(O1);
    repo.orphan(O2);
    assert_eq!(repo.local_blobs().len(), 4);
    let (verify_before, gate_before) = state(&repo);
    assert_eq!((verify_before.status.code(), gate_before.status.code()), (Some(0), Some(0)));

    let preview = repo.preview(&[]);
    assert_eq!(preview["data"]["delete"]["blobs"], 3, "the superseded V1 and the two orphans: {preview}");
    let done = repo.commit(&preview);
    assert_eq!(done.status.code(), Some(0), "{}", text(&done));
    let body = json(&done);
    assert_eq!(body["data"]["committed"], true);
    assert_eq!((body["data"]["deleted"]["blobs"].as_u64(), body["data"]["deleted"]["bytes"].as_u64()), (Some(3), Some((V1.len() + O1.len() + O2.len()) as u64)), "{body}");
    assert_eq!(body["data"]["deleted"]["localCopies"], 3);
    assert!(body["data"]["gitNote"].as_str().unwrap().contains("git history"));
    assert_eq!(repo.local_blobs(), vec![sha(V2)]);
    assert_eq!(std::fs::read(repo.blob(V2)).unwrap(), V2);

    let (verify_after, gate_after) = state(&repo);
    assert_eq!(verify_after.status.code(), verify_before.status.code());
    assert_eq!(gate_after.status.code(), gate_before.status.code());
    let (b, a) = (json(&verify_before), json(&verify_after));
    assert_eq!((&a["data"]["bound"], &a["data"]["stored"], &a["data"]["problems"]), (&b["data"]["bound"], &b["data"]["stored"], &b["data"]["problems"]));
    assert_eq!(json(&gate_after)["data"]["violations"], json(&gate_before)["data"]["violations"]);
    let full = repo.canon(&["artifacts", "verify", "--full", "--json"]);
    assert_eq!(full.status.code(), Some(0), "{}", text(&full));

    let human = repo.canon(&["artifacts", "prune", "--commit", "--plan-digest", "sha256:0000000000000000000000000000000000000000000000000000000000000000"]);
    assert_eq!(human.status.code(), Some(2), "a stale digest is refused: {}", text(&human));
    let again = repo.preview(&[]);
    assert_eq!(again["data"]["delete"]["blobs"], 0, "{again}");
}

/// artifacts.prune.05: a ledger or a staging area that cannot be read in full is no keep list: the preview and the commit refuse with exit 2
/// and nothing is deleted.
#[test]
fn artifacts_prune_05_records_that_cannot_be_read_delete_nothing_and_are_refused() {
    let repo = new_repo(None);
    repo.attest(V1, true);
    repo.orphan(O1);
    let preview = repo.preview(&[]);
    let before = repo.local_blobs();

    // A committed evidence record that is not where its own content puts it.
    let dir = repo.path().join(".canon/ledger/kind=evidence");
    let record = std::fs::read_dir(&dir).unwrap().flatten().next().unwrap().path();
    let moved = dir.join("root_scenario_game.run.01_00000000.json");
    std::fs::rename(&record, &moved).unwrap();
    let out = repo.canon(&["artifacts", "prune", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(json(&out)["error"]["message"].as_str().unwrap().contains("evidence"), "{}", text(&out));
    let out = repo.commit(&preview);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert_eq!(repo.local_blobs(), before, "nothing is deleted when the keep list is not known");
    std::fs::rename(&moved, &record).unwrap();
    assert_eq!(repo.preview(&[])["data"]["delete"]["blobs"], 1, "the repo reads again once the record is back");

    // A staged record that is no record.
    let staging = repo.path().join(".canon/ledger/_staging/kind=evidence");
    std::fs::create_dir_all(&staging).unwrap();
    std::fs::write(staging.join("root_scenario_game.run.01_deadbeef.json"), b"{ not json").unwrap();
    let out = repo.canon(&["artifacts", "prune", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(json(&out)["error"]["message"].as_str().unwrap().contains("staged evidence"), "{}", text(&out));
    assert_eq!(repo.local_blobs(), before);
}

/// artifacts.prune.06: a staged (not yet promoted) record's blob stays, a superseded record's blob goes, and a blob that the latest evidence and
/// a current review both bind names both reasons.
#[test]
fn artifacts_prune_06_each_kept_blob_names_every_reason_that_holds() {
    let repo = new_repo(None);
    repo.attest(V1, true);
    repo.attest(V2, true);
    // A review of the scenario that looked at the same report the latest evidence binds.
    repo.ok(&["review", "scenario", "game.run.01", "--project", "root", "--original-spec-ref", "specs/run.feature", "--actor", "reviewer-1", "--role", "reviewer", "--artifact", "reports/smoke.json"]);
    // A third report, attested but not promoted yet.
    repo.attest(V3, false);

    let body = repo.preview(&[]);
    let kept: BTreeMap<String, Vec<String>> = body["data"]["keep"]["items"].as_array().unwrap().iter().map(|item| (item["sha256"].as_str().unwrap().to_string(), item["reasons"].as_array().unwrap().iter().map(|reason| reason.as_str().unwrap().to_string()).collect())).collect();
    assert_eq!(kept.get(&sha(V2)), Some(&vec!["latest-evidence".to_string(), "current-review".to_string()]), "{body}");
    assert_eq!(kept.get(&sha(V3)), Some(&vec!["staged".to_string()]), "{body}");
    assert!(!kept.contains_key(&sha(V1)), "the superseded record's blob is not kept: {body}");
    assert_eq!(digests(&body["data"]["blobs"]), vec![sha(V1)], "{body}");
    assert_eq!((body["data"]["keep"]["reasons"]["latest-evidence"].as_u64(), body["data"]["keep"]["reasons"]["current-review"].as_u64(), body["data"]["keep"]["reasons"]["staged"].as_u64()), (Some(1), Some(1), Some(1)));

    let done = repo.commit(&body);
    assert_eq!(done.status.code(), Some(0), "{}", text(&done));
    assert_eq!(repo.local_blobs(), {
        let mut kept = vec![sha(V2), sha(V3)];
        kept.sort();
        kept
    });
    // Promoting the staged record afterwards still finds its blob.
    repo.ok(&["gate", "promote"]);
    let full = repo.canon(&["artifacts", "verify", "--full", "--json"]);
    assert_eq!(full.status.code(), Some(0), "{}", text(&full));
    assert_eq!(repo.canon(&["gate", "check", "--json"]).status.code(), Some(0));
}

/// artifacts.prune.07: a commit that stops after it deleted some blobs says what it deleted (`ARTIFACTS_PRUNE_INCOMPLETE`, exit 1, the state read
/// first); a new preview lists what remains and its commit finishes, and no blob a record binds was touched.
#[test]
fn artifacts_prune_07_a_delete_that_stops_part_way_says_what_was_deleted_and_the_rest_finishes() {
    let repo = new_repo(None);
    repo.attest(V1, true);
    for orphan in [O1, O2, O3] {
        repo.orphan(orphan);
    }
    let preview = repo.preview(&[]);
    let argv: Vec<String> = preview["meta"]["next"][0]["argv"].as_array().unwrap().iter().map(|word| word.as_str().unwrap().to_string()).collect();
    let args: Vec<&str> = argv.iter().map(String::as_str).collect();
    let stopped = repo.canon_env(&args, &[("CANON_TEST_STOP_AT", "artifacts-prune-after-delete")]);
    assert_eq!(stopped.status.code(), Some(1), "{}", text(&stopped));
    let body = json(&stopped);
    assert_eq!((body["error"]["code"].as_str(), body["error"]["reason"].as_str(), body["error"]["retryable"].as_bool()), (Some("ARTIFACTS_PRUNE_INCOMPLETE"), Some("WRITE_FAILED"), Some(false)), "{body}");
    assert_eq!(body["data"]["deleted"]["blobs"], 1, "{body}");
    assert_eq!(body["meta"]["next"][0]["argv"][0], "artifacts", "the state is read first: {body}");
    assert_eq!(body["meta"]["next"][0]["argv"][1], "verify", "the state is read first: {body}");
    assert_eq!(repo.local_blobs().len(), 3, "the bound blob and two orphans remain");
    assert!(repo.blob(V1).exists());

    let again = repo.preview(&[]);
    assert_eq!(again["data"]["delete"]["blobs"], 2, "{again}");
    let stale = repo.canon(&args);
    assert_eq!(stale.status.code(), Some(2), "the old digest is stale now: {}", text(&stale));
    let done = repo.commit(&again);
    assert_eq!(done.status.code(), Some(0), "{}", text(&done));
    assert_eq!(repo.local_blobs(), vec![sha(V1)]);
    assert_eq!(repo.canon(&["artifacts", "verify", "--full"]).status.code(), Some(0));
}

/// Rewrites the evidence record at `path` to say it was written `days` ago and bound `digest`, under the file name its new content resolves to, in
/// the archive (a superseded version: only its time can keep its blob).
fn aged_copy(repo: &Repo, days: i64, digest: &str) {
    let archive = repo.path().join(".canon/archive/kind=evidence");
    let source = std::fs::read_dir(&archive).unwrap().flatten().next().expect("an archived (superseded) record").path();
    let mut record: Value = serde_json::from_slice(&std::fs::read(&source).unwrap()).unwrap();
    record["at"] = Value::String((chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
    record["attachments"][0]["sha256"] = Value::String(digest.to_string());
    let relative = canon_store::partition::expected_relative_path(canon_model::RecordKind::Evidence, &record).expect("the aged record has a layout name");
    let target = repo.path().join(".canon/archive").join(relative);
    std::fs::write(&target, serde_json::to_vec_pretty(&record).unwrap()).unwrap();
}

/// artifacts.prune.08: `--keep-days` keeps the blobs the records written in that many days bind (a superseded record's among them) and deletes the
/// older ones; without it only the gate's set stays; the commit command of the preview carries the option.
#[test]
fn artifacts_prune_08_keep_days_keeps_the_blobs_of_recent_records() {
    let repo = new_repo(None);
    repo.attest(V1, true);
    repo.attest(V2, true);
    repo.orphan(O1);
    // A superseded record from 90 days ago that bound O1.
    aged_copy(&repo, 90, &sha(O1));

    let plain = repo.preview(&[]);
    let mut doomed = vec![sha(V1), sha(O1)];
    doomed.sort();
    assert_eq!(digests(&plain["data"]["blobs"]), doomed, "without the option only the latest evidence stays: {plain}");
    assert!(plain["data"].get("keepDays").is_none());

    let recent = repo.preview(&["--keep-days", "30"]);
    assert_eq!(recent["data"]["keepDays"], 30);
    assert_eq!(digests(&recent["data"]["blobs"]), vec![sha(O1)], "the 90-day-old record's blob goes, the 3-minute-old superseded record's stays: {recent}");
    let reasons = |digest: &str| -> Vec<String> { recent["data"]["keep"]["items"].as_array().unwrap().iter().find(|item| item["sha256"] == digest).unwrap()["reasons"].as_array().unwrap().iter().map(|reason| reason.as_str().unwrap().to_string()).collect() };
    assert_eq!(reasons(&sha(V1)), vec!["within-keep-days"], "{recent}");
    assert_eq!(reasons(&sha(V2)), vec!["latest-evidence", "within-keep-days"], "{recent}");
    let commit: Vec<&str> = recent["meta"]["next"][0]["argv"].as_array().unwrap().iter().map(|word| word.as_str().unwrap()).collect();
    assert!(commit.contains(&"--keep-days=30"), "the commit carries the option it was planned with: {commit:?}");

    let long = repo.preview(&["--keep-days", "120"]);
    assert_eq!(long["data"]["delete"]["blobs"], 0, "{long}");
    assert_eq!(commit_steps(&long), 0);

    let done = repo.commit(&recent);
    assert_eq!(done.status.code(), Some(0), "{}", text(&done));
    let mut kept = vec![sha(V1), sha(V2)];
    kept.sort();
    assert_eq!(repo.local_blobs(), kept);
    // A commit planned without the option is not the plan with it.
    let stale = repo.canon(&["artifacts", "prune", "--keep-days", "30", "--commit", "--plan-digest", plain["data"]["planDigest"].as_str().unwrap()]);
    assert_eq!(stale.status.code(), Some(2), "{}", text(&stale));
}

/// artifacts.prune.09: `--keep-days` is a whole number of 1 or more: 0, a negative number, a fraction and a word are refused with exit 2 and
/// nothing is deleted.
#[test]
fn artifacts_prune_09_keep_days_below_one_is_refused() {
    let repo = new_repo(None);
    repo.attest(V1, true);
    repo.orphan(O1);
    let before = repo.files();
    for value in ["0", "-1", "1.5", "many", ""] {
        let args = ["artifacts", "prune", &format!("--keep-days={value}"), "--json"];
        let out = repo.canon(&args);
        assert_eq!(out.status.code(), Some(2), "--keep-days={value}: {}", text(&out));
        let body = json(&out);
        assert_eq!(body["error"]["code"], "USAGE", "{body}");
        assert!(body["error"]["message"].as_str().unwrap().contains("keep-days"), "{body}");
    }
    let out = repo.canon(&["artifacts", "prune", "--keep-days", "-1", "--commit", "--plan-digest", "sha256:0000000000000000000000000000000000000000000000000000000000000000", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert_eq!(repo.files(), before);
}

/// artifacts.prune.10: with an object-store tier the unneeded objects are deleted by the same rule (a stray local copy by it too); the preview says
/// the delete cannot be undone and counts the objects; the objects the records bind pass `artifacts verify --full`.
#[test]
fn artifacts_prune_10_an_object_store_tier_is_pruned_by_the_same_rule() {
    let repo = new_repo(Some(ARTIFACTS_COLD));
    repo.attest(V1, true);
    repo.attest(V2, true);
    repo.orphan_object(O1);
    repo.orphan_object(O2);
    repo.orphan(O3); // a copy left in the local directory, bound by nothing
    assert!(repo.object(V1).exists() && repo.object(V2).exists());

    let body = repo.preview(&[]);
    let data = &body["data"];
    assert_eq!(data["backend"], "cold");
    assert_eq!((data["delete"]["blobs"].as_u64(), data["delete"]["tierObjects"].as_u64(), data["delete"]["localCopies"].as_u64()), (Some(4), Some(3), Some(1)), "V1 (superseded), O1 and O2 are objects, O3 is local: {body}");
    assert!(data["irreversible"].as_str().unwrap().contains("cannot be undone") && data["irreversible"].as_str().unwrap().contains("3 object(s)"), "{body}");
    assert_eq!(data["keep"]["items"][0]["sha256"], sha(V2));
    assert!(repo.object(O1).exists() && repo.blob(O3).exists(), "a preview deletes nothing");
    let human = text(&repo.canon(&["artifacts", "prune"]));
    assert!(human.contains("cannot be undone") && human.contains("3 object(s)"), "{human}");

    let done = repo.commit(&body);
    assert_eq!(done.status.code(), Some(0), "{}", text(&done));
    let result = json(&done);
    assert_eq!((result["data"]["deleted"]["blobs"].as_u64(), result["data"]["deleted"]["tierObjects"].as_u64(), result["data"]["deleted"]["localCopies"].as_u64()), (Some(4), Some(3), Some(1)), "{result}");
    for gone in [V1, O1, O2] {
        assert!(!repo.object(gone).exists());
    }
    assert!(!repo.blob(O3).exists());
    assert_eq!(std::fs::read(repo.object(V2)).unwrap(), V2);
    let full = repo.canon(&["artifacts", "verify", "--full", "--json"]);
    assert_eq!(full.status.code(), Some(0), "{}", text(&full));
    assert_eq!(repo.canon(&["gate", "check", "--json"]).status.code(), Some(0));
    assert_eq!(repo.preview(&[])["data"]["delete"]["blobs"], 0);
}

/// artifacts.prune.11: a tier that refuses the delete stops the run before it deletes anything (`ARTIFACTS_STORE_FAILED`, exit 1) and no kept object
/// is touched; a tier that cannot be listed is refused with exit 2 before a plan exists.
#[test]
fn artifacts_prune_11_a_tier_that_refuses_the_delete_or_cannot_be_listed_stops_the_run() {
    use std::os::unix::fs::PermissionsExt as _;
    let repo = new_repo(Some(ARTIFACTS_COLD));
    repo.attest(V1, true);
    repo.orphan_object(O1);
    repo.orphan_object(O2);
    let preview = repo.preview(&[]);

    let bucket = repo.store.path().join("artifacts/sha256");
    std::fs::set_permissions(&bucket, std::fs::Permissions::from_mode(0o555)).unwrap();
    let refused = repo.commit(&preview);
    std::fs::set_permissions(&bucket, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(refused.status.code(), Some(1), "{}", text(&refused));
    let body = json(&refused);
    assert_eq!(body["error"]["code"], "ARTIFACTS_STORE_FAILED", "{body}");
    assert!(body["error"]["message"].as_str().unwrap().contains("nothing was deleted"), "{body}");
    for kept in [V1, O1, O2] {
        assert!(repo.object(kept).exists(), "a refused delete removes nothing");
    }
    assert_eq!(repo.canon(&["artifacts", "verify", "--full"]).status.code(), Some(0));

    // A tier that gives out after its first delete: what was deleted is said, and the rest finishes with a new plan.
    let argv: Vec<String> = preview["meta"]["next"][0]["argv"].as_array().unwrap().iter().map(|word| word.as_str().unwrap().to_string()).collect();
    let args: Vec<&str> = argv.iter().map(String::as_str).collect();
    let stopped = repo.canon_env(&args, &[("CANON_TEST_STOP_AT", "artifacts-prune-object-after-delete")]);
    assert_eq!(stopped.status.code(), Some(1), "{}", text(&stopped));
    let body = json(&stopped);
    assert_eq!((body["error"]["code"].as_str(), body["error"]["reason"].as_str()), (Some("ARTIFACTS_PRUNE_INCOMPLETE"), Some("ARTIFACTS_STORE_FAILED")), "{body}");
    assert_eq!((body["data"]["deleted"]["blobs"].as_u64(), body["data"]["deleted"]["tierObjects"].as_u64()), (Some(1), Some(1)), "{body}");
    assert!(repo.object(V1).exists(), "the kept object is untouched");
    assert_eq!([O1, O2].iter().filter(|orphan| repo.object(orphan).exists()).count(), 1, "one orphan object is gone, one remains");
    let rest = repo.preview(&[]);
    assert_eq!(rest["data"]["delete"]["tierObjects"], 1, "{rest}");
    assert_eq!(repo.commit(&rest).status.code(), Some(0));
    assert!(!repo.object(O1).exists() && !repo.object(O2).exists() && repo.object(V1).exists());
    assert_eq!(repo.canon(&["artifacts", "verify", "--full"]).status.code(), Some(0));
    repo.orphan_object(O1);

    // The store does not answer: no plan, exit 2, nothing deleted.
    let env = [("CANON_S3_ENDPOINT", "http://127.0.0.1:9"), ("CANON_R2_BUCKET", "evidence")];
    let out = repo.canon_s3(&["artifacts", "prune", "--json"], &env);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert_eq!(json(&out)["error"]["code"], "ARTIFACTS_STORE_UNAVAILABLE");
    for kept in [V1, O1] {
        assert!(repo.object(kept).exists());
    }
}

/// artifacts.prune.12: evidence bound after the preview changes the plan: the old digest is `PLAN_STALE` (exit 2) and no blob is deleted; the
/// new preview has a digest that commits.
#[test]
fn artifacts_prune_12_a_plan_whose_records_changed_since_its_preview_is_refused() {
    let repo = new_repo(None);
    repo.attest(V1, true);
    repo.orphan(O1);
    repo.orphan(O2);
    let preview = repo.preview(&[]);
    let blobs = repo.local_blobs();

    // New evidence binds an orphan: the plan's list and the records it read both changed.
    repo.report(O1);
    let args = Repo::attest_args(&[]);
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    repo.ok(&args);
    repo.ok(&["gate", "promote"]);
    let before = repo.files();
    let stale = repo.commit(&preview);
    assert_eq!(stale.status.code(), Some(2), "{}", text(&stale));
    let body = json(&stale);
    assert_eq!(body["error"]["code"], "PLAN_STALE", "{body}");
    assert!(body["error"]["hint"].as_str().unwrap().contains("artifacts prune"), "it says to run the preview again: {body}");
    assert_eq!(repo.files(), before, "a stale plan deletes nothing");
    assert_eq!(repo.local_blobs(), blobs);

    let fresh = repo.preview(&[]);
    assert_ne!(fresh["data"]["planDigest"], preview["data"]["planDigest"]);
    let mut expected = vec![sha(V1), sha(O2)];
    expected.sort();
    assert_eq!(digests(&fresh["data"]["blobs"]), expected, "the blob the new evidence binds is kept now: {fresh}");
    let done = repo.commit(&fresh);
    assert_eq!(done.status.code(), Some(0), "{}", text(&done));
    assert_eq!(repo.local_blobs(), vec![sha(O1)]);

    // A record that is only staged changes the plan too.
    let again = new_repo(None);
    again.attest(V1, true);
    again.orphan(O1);
    let preview = again.preview(&[]);
    again.attest(V2, false);
    let stale = again.commit(&preview);
    assert_eq!(stale.status.code(), Some(2), "{}", text(&stale));
    assert_eq!(json(&stale)["error"]["code"], "PLAN_STALE");
    assert!(again.blob(O1).exists());
}

/// artifacts.prune.13: `--commit` takes its plan digest, and the digest its `--commit`: either alone is refused with exit 2 and nothing is deleted.
#[test]
fn artifacts_prune_13_commit_without_a_plan_digest_is_refused() {
    let repo = new_repo(None);
    repo.attest(V1, true);
    repo.orphan(O1);
    let before = repo.files();
    let out = repo.canon(&["artifacts", "prune", "--commit", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    let body = json(&out);
    assert_eq!(body["error"]["code"], "USAGE", "{body}");
    assert!(body["error"]["message"].as_str().unwrap().contains("plan-digest"), "{body}");
    let digest = repo.preview(&[])["data"]["planDigest"].as_str().unwrap().to_string();
    let out = repo.canon(&["artifacts", "prune", "--plan-digest", &digest, "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert_eq!(repo.files(), before);
}

/// What the release lacks of the announcement of the command: each guide (`(name, text)`) names it and says a deleted local blob stays in git
/// history, and the Added section of the Unreleased CHANGELOG says both. The release check: a missing item is named.
fn release_problems(guides: &[(&str, String)], changelog: &str) -> Vec<String> {
    let mut problems = Vec::new();
    for (name, text) in guides {
        let said = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if !said.contains("canon artifacts prune") {
            problems.push(format!("the guide `{name}` never names `canon artifacts prune`"));
        }
        if !said.contains("git history") {
            problems.push(format!("the guide `{name}` never says a deleted local blob stays in git history"));
        }
    }
    let unreleased = changelog.split("\n## ").find(|section| section.starts_with("0.38.0"));
    let added = unreleased.and_then(|section| section.split("\n### ").find(|part| part.starts_with("Added")));
    match added {
        None => problems.push("the CHANGELOG has no Added section under 0.38.0".to_string()),
        Some(added) => {
            if !added.contains("canon artifacts prune") {
                problems.push("the Added section never names `canon artifacts prune`".to_string());
            }
            if !added.contains("git history") {
                problems.push("the Added section never says a deleted local blob stays in git history".to_string());
            }
        }
    }
    problems
}

const GUIDES: [&str; 3] = ["canon", "canon-storage", "role-orchestrator"];

fn guides(repo: &Repo) -> Vec<(&'static str, String)> {
    GUIDES.iter().map(|guide| (*guide, String::from_utf8_lossy(&repo.ok(&["skills", "show", guide]).stdout).into_owned())).collect()
}

fn changelog() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CHANGELOG.md")).unwrap()
}

/// artifacts.prune.14: the guides name the command and say that a deleted local blob stays in git history, and the CHANGELOG lists it under Added.
#[test]
fn artifacts_prune_14_the_guides_and_the_changelog_name_the_command_and_git_history() {
    let repo = new_repo(None);
    assert_eq!(release_problems(&guides(&repo), &changelog()), Vec::<String>::new());
}

/// artifacts.prune.15: the release check fails when the announcement is missing: a guide without the command or without the git-history sentence,
/// a CHANGELOG without an Added section, or an Added section without either, is named, each by what it lacks.
#[test]
fn artifacts_prune_15_a_release_without_the_announcement_fails_the_release_check() {
    let repo = new_repo(None);
    let (guides, changelog) = (guides(&repo), changelog());
    assert_eq!(release_problems(&guides, &changelog), Vec::<String>::new(), "the check passes on the real text");

    let without = |needle: &str, text: &str| text.replace(needle, "removed");
    // A guide that loses the command, and one that loses the sentence.
    let mut bare = guides.clone();
    bare[0].1 = without("canon artifacts prune", &bare[0].1);
    let problems = release_problems(&bare, &changelog);
    assert_eq!(problems, ["the guide `canon` never names `canon artifacts prune`"], "{problems:?}");
    let mut silent = guides.clone();
    silent[1].1 = without("git history", &silent[1].1);
    assert_eq!(release_problems(&silent, &changelog), ["the guide `canon-storage` never says a deleted local blob stays in git history"]);

    // A CHANGELOG with no Added section under 0.38.0, and with an Added section that says neither thing.
    let no_added = changelog.replacen("\n### Added\n", "\n### Changed\n", 1);
    assert_eq!(release_problems(&guides, &no_added), ["the CHANGELOG has no Added section under 0.38.0"]);
    let start = changelog.find("\n### Added\n").expect("an Added section");
    let end = changelog[start + 1..].find("\n### ").map_or(changelog.len(), |at| start + 1 + at);
    let emptied = format!("{}\n### Added\n\n- something else\n{}", &changelog[..start], &changelog[end..]);
    let problems = release_problems(&guides, &emptied);
    assert_eq!(problems, ["the Added section never names `canon artifacts prune`", "the Added section never says a deleted local blob stays in git history"], "{problems:?}");
    // A CHANGELOG with no 0.38.0 section at all.
    assert_eq!(release_problems(&guides, "# Changelog\n\n## 0.37.0\n\n### Added\n").len(), 1);
}
