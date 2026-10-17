//! 0.23 D3 through the real binary: evidence bytes off git. `canon.yaml`'s
//! `artifacts:` block selects an object-store tier; writes upload by digest
//! key and keep no local copy, the gate and `canon artifacts verify` ask the
//! store, and `canon artifacts migrate` moves a local store there.
//!
//! The object store is the debug-build local-filesystem substitute
//! (`CANON_R2_LOCAL_ROOT`, the seam `canon tier age` tests use), so these
//! run offline; the unreachable-store tests point `CANON_S3_ENDPOINT` at a
//! closed port instead. The same code runs against a real bucket in
//! `canon-store`'s `live-r2` CI job (`tests/r2_tier_live.rs`).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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

    fn canon(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_canon"))
            .args(args)
            .env("CANON_ACTOR", "canon")
            .env("CANON_R2_LOCAL_ROOT", self.store.path())
            .current_dir(self.path())
            .output()
            .expect("spawn canon")
    }

    /// `canon` with the substitute store removed and a real-looking S3
    /// configuration instead (what a release binary always uses).
    fn canon_s3(&self, args: &[&str], env: &[(&str, &str)]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_canon"));
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

    /// The object a bucket holds for `bytes`.
    fn object(&self, bytes: &[u8]) -> PathBuf {
        self.store.path().join("artifacts/sha256").join(sha(bytes))
    }

    /// The blob the local store holds for `bytes`.
    fn local_blob(&self, bytes: &[u8]) -> PathBuf {
        self.path().join(".canon/artifacts/sha256").join(sha(bytes))
    }

    fn attest(&self, extra: &[&str]) -> Output {
        let mut args = vec!["evidence", "add", "--scenario-id", "game.run.01", "--project-id", "root", "--kind", "test-run", "--ref", "npm run smoke", "--role", "implementer"];
        args.extend_from_slice(extra);
        let out = self.canon(&args);
        if out.status.success() {
            self.ok(&["gate", "promote"]);
        }
        out
    }

    fn canon_yaml(&self) -> String {
        std::fs::read_to_string(self.path().join("canon.yaml")).unwrap()
    }
}

fn text(output: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr))
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

const COLD_STANZA: &str = "  cold:\n    backend: s3\n    bucket_env: CANON_R2_BUCKET\n    prefix: \"canon/\"\n";
const ARTIFACTS_COLD: &str = "artifacts:\n  backend: cold\n  prefix: artifacts/\n";

/// `git init` + `canon init --no-policy` + one scenario, with the `cold`
/// s3 tier declared. `artifacts` is appended to canon.yaml when given.
fn new_repo(artifacts: Option<&str>) -> Repo {
    let repo = Repo { dir: tempfile::tempdir().unwrap(), store: tempfile::tempdir().unwrap() };
    assert!(Command::new("git").args(["init", "-q"]).current_dir(repo.path()).status().unwrap().success());
    repo.ok(&["init", "--repo", ".", "--no-policy"]);
    let yaml = repo.canon_yaml();
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

fn write_report(repo: &Repo, bytes: &[u8]) {
    std::fs::create_dir_all(repo.path().join("reports")).unwrap();
    std::fs::write(repo.path().join("reports/smoke.json"), bytes).unwrap();
}

const V1: &[u8] = br#"{"at":"2026-10-10T14:00:00Z","checks":16}"#;

fn stdout_json(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {}", text(output)))
}

/// Acceptance: with an object backend `evidence add` stores no local blob
/// and the gate passes — from the store, not from the working tree.
#[test]
fn evidence_add_with_an_object_backend_uploads_by_digest_keeps_no_local_blob_and_the_gate_passes() {
    let repo = new_repo(Some(ARTIFACTS_COLD));
    write_report(&repo, V1);

    let added = repo.attest(&["--artifact", "reports/smoke.json"]);
    assert!(added.status.success(), "{}", text(&added));
    assert!(text(&added).contains("1 newly uploaded") && text(&added).contains("nothing to commit"), "{}", text(&added));
    assert_eq!(std::fs::read(repo.object(V1)).unwrap(), V1, "the bytes sit in the store under artifacts/sha256/<digest>");
    assert!(!repo.path().join(".canon/artifacts").exists(), "no local copy is kept");

    // The working tree changes: only the store can prove the record now.
    std::fs::write(repo.path().join("reports/smoke.json"), br#"{"at":"later","checks":16}"#).unwrap();
    let gate = repo.canon(&["gate", "check"]);
    assert!(gate.status.success(), "{}", text(&gate));
    assert!(!text(&gate).contains("stale-evidence"), "{}", text(&gate));

    let verify = repo.canon(&["artifacts", "verify"]);
    assert!(verify.status.success() && text(&verify).contains("stored 1"), "{}", text(&verify));
    let full = repo.canon(&["artifacts", "verify", "--full"]);
    assert!(full.status.success() && text(&full).contains("re-hashed"), "{}", text(&full));

    // The same bytes again upload nothing.
    let again = repo.attest(&["--artifact", "reports/smoke.json", "--artifact", "reports/smoke.json"]);
    assert!(again.status.success(), "{}", text(&again));
}

/// Acceptance: a missing remote object refuses at the gate with a hint, even
/// though the working-tree file still matches (the local store's back-compat
/// fallback does not apply to an object store); `evidence vault` uploads it
/// again from the working tree.
#[test]
fn a_missing_remote_object_fails_the_gate_with_a_hint_and_vault_uploads_it_again() {
    let repo = new_repo(Some(ARTIFACTS_COLD));
    write_report(&repo, V1);
    assert!(repo.attest(&["--artifact", "reports/smoke.json"]).status.success());
    std::fs::remove_file(repo.object(V1)).unwrap();

    let gate = repo.canon(&["gate", "check"]);
    assert_eq!(gate.status.code(), Some(1), "{}", text(&gate));
    let out = text(&gate);
    assert!(out.contains("stale-evidence game.run.01") && out.contains("`reports/smoke.json`"), "{out}");
    assert!(out.contains(&sha(V1)) && out.contains("cold:artifacts/sha256/") && out.contains(": missing"), "names the digest and the store key: {out}");
    assert!(out.contains("not in the artifact store") && out.contains("canon evidence vault"), "carries the fix: {out}");

    let verify = repo.canon(&["artifacts", "verify", "--json"]);
    assert_eq!(verify.status.code(), Some(1), "{}", text(&verify));
    let body = stdout_json(&verify);
    assert_eq!(body["error"]["code"], "ARTIFACTS_VERIFY_FAILED");
    assert_eq!(body["data"]["problems"][0]["sha256"], sha(V1));

    let vault = repo.ok(&["evidence", "vault"]);
    assert!(text(&vault).contains("stored 1, already stored 0") && !text(&vault).contains("commit .canon/artifacts"), "{}", text(&vault));
    assert_eq!(std::fs::read(repo.object(V1)).unwrap(), V1);
    assert!(repo.canon(&["gate", "check"]).status.success());
}

/// The default HEAD check trusts the object's size and digest metadata, which
/// a store that cannot keep metadata does not have; `--full` is the byte check.
#[test]
fn verify_full_downloads_and_rehashes_what_a_head_cannot_judge() {
    let repo = new_repo(Some(ARTIFACTS_COLD));
    write_report(&repo, V1);
    assert!(repo.attest(&["--artifact", "reports/smoke.json"]).status.success());
    std::fs::write(repo.object(V1), b"someone else's bytes").unwrap();

    assert!(repo.canon(&["artifacts", "verify"]).status.success(), "presence alone is what a HEAD proves here");
    let full = repo.canon(&["artifacts", "verify", "--full"]);
    assert_eq!(full.status.code(), Some(1), "{}", text(&full));
    assert!(text(&full).contains("does not match its name") && text(&full).contains(&sha(b"someone else's bytes")), "{}", text(&full));
}

/// An unattachable or unreachable store never passes: the write refuses with
/// a hint naming the variables, the gate and verify refuse rather than assume.
#[test]
fn an_unreachable_or_unattachable_store_refuses_with_a_hint_and_never_passes() {
    let repo = new_repo(Some(ARTIFACTS_COLD));
    write_report(&repo, V1);
    assert!(repo.attest(&["--artifact", "reports/smoke.json"]).status.success());

    // No bucket configured.
    let gate = repo.canon_s3(&["gate", "check"], &[("CANON_S3_ENDPOINT", "http://127.0.0.1:9")]);
    assert_eq!(gate.status.code(), Some(1), "{}", text(&gate));
    assert!(text(&gate).contains("cannot be attached") && text(&gate).contains("CANON_R2_BUCKET"), "{}", text(&gate));

    // A bucket, but nothing listens.
    let env = [("CANON_S3_ENDPOINT", "http://127.0.0.1:9"), ("CANON_R2_BUCKET", "evidence")];
    let gate = repo.canon_s3(&["gate", "check"], &env);
    assert_eq!(gate.status.code(), Some(1), "an offline store is not a pass: {}", text(&gate));
    assert!(text(&gate).contains("could not be reached") && text(&gate).contains("CANON_S3_ENDPOINT"), "{}", text(&gate));
    let verify = repo.canon_s3(&["artifacts", "verify", "--json"], &env);
    assert_eq!(verify.status.code(), Some(2), "{}", text(&verify));
    let body = stdout_json(&verify);
    assert_eq!(body["error"]["code"], "ARTIFACTS_STORE_UNREACHABLE");
    assert!(body["error"]["hint"].as_str().unwrap().contains("CANON_S3_ACCESS_KEY"), "{body}");

    // A write refuses and records nothing.
    std::fs::write(repo.path().join("reports/other.json"), br#"{"other":1}"#).unwrap();
    let before = std::fs::read_dir(repo.path().join(".canon/ledger/kind=evidence_record")).unwrap().count();
    let mut args = vec!["evidence", "add", "--scenario-id", "game.run.01", "--project-id", "root", "--kind", "test-run", "--ref", "x", "--role", "implementer", "--artifact", "reports/other.json"];
    args.push("--json");
    let add = repo.canon_s3(&args, &env);
    assert_eq!(add.status.code(), Some(2), "{}", text(&add));
    let body = stdout_json(&add);
    assert_eq!(body["error"]["code"], "EVIDENCE_STORE_FAILED");
    assert!(body["error"]["hint"].as_str().unwrap().contains("reachable"), "{body}");
    assert_eq!(std::fs::read_dir(repo.path().join(".canon/ledger/kind=evidence_record")).unwrap().count(), before, "no record was written");
}

#[test]
fn an_artifacts_block_naming_no_s3_tier_is_refused_where_it_is_used() {
    let repo = new_repo(Some("artifacts:\n  backend: hot\n"));
    write_report(&repo, V1);
    let add = repo.attest(&["--artifact", "reports/smoke.json"]);
    assert_eq!(add.status.code(), Some(2), "{}", text(&add));
    assert!(text(&add).contains("EVIDENCE_STORE_FAILED") && text(&add).contains("which is not an object store"), "{}", text(&add));
    assert!(text(&add).contains("`artifacts: { backend: local }`"), "the hint names the way out: {}", text(&add));
}

/// Local stays the default: no `artifacts:` block, or `backend: local`, is
/// today's git-tracked store and touches no object store.
#[test]
fn the_local_default_is_unchanged() {
    for block in [None, Some("artifacts:\n  backend: local\n")] {
        let repo = new_repo(block);
        write_report(&repo, V1);
        let added = repo.attest(&["--artifact", "reports/smoke.json"]);
        assert!(added.status.success(), "{}", text(&added));
        assert!(text(&added).contains(".canon/artifacts/sha256/") && text(&added).contains("commit them with the ledger"), "{}", text(&added));
        assert_eq!(std::fs::read(repo.local_blob(V1)).unwrap(), V1);
        assert!(!repo.object(V1).exists() && std::fs::read_dir(repo.store.path()).unwrap().count() == 0, "nothing reaches the object store");
        let verify = repo.canon(&["artifacts", "verify"]);
        assert!(verify.status.success() && text(&verify).contains("stored 1"), "{}", text(&verify));
    }
}

/// Acceptance: migrate moves the local blobs, verifies them, removes the
/// local copies, ignores the directory — and is idempotent and resumable.
#[test]
fn migrate_moves_verifies_removes_local_copies_and_is_idempotent_and_resumable() {
    let repo = new_repo(None);
    write_report(&repo, V1);
    assert!(repo.attest(&["--artifact", "reports/smoke.json"]).status.success());
    let v2: &[u8] = br#"{"at":"2026-10-10T15:00:00Z","checks":15}"#;
    std::fs::write(repo.path().join("reports/second.json"), v2).unwrap();
    assert!(repo.attest(&["--artifact", "reports/second.json"]).status.success());
    assert!(repo.local_blob(V1).exists() && repo.local_blob(v2).exists());
    let yaml_before = repo.canon_yaml();

    // A dry run writes nothing.
    let dry = repo.ok(&["artifacts", "migrate", "--to", "cold", "--dry-run"]);
    assert!(text(&dry).contains("would upload 2") && text(&dry).contains("would remove 2 local copies"), "{}", text(&dry));
    assert!(repo.local_blob(V1).exists() && !repo.object(V1).exists() && repo.canon_yaml() == yaml_before, "a dry run changes nothing");

    let run = repo.ok(&["artifacts", "migrate", "--to", "cold", "--json"]);
    let body = stdout_json(&run);
    assert_eq!(body["data"]["uploaded"], 2);
    assert_eq!(body["data"]["removedLocal"], 2);
    assert_eq!(body["data"]["configWritten"], true);
    assert!(body["meta"]["next"].is_array(), "a write names what comes next: {body}");
    assert_eq!(std::fs::read(repo.object(V1)).unwrap(), V1);
    assert_eq!(std::fs::read(repo.object(v2)).unwrap(), v2);
    assert!(!repo.path().join(".canon/artifacts").exists(), "the local store is gone");
    assert!(repo.canon_yaml().ends_with(ARTIFACTS_COLD), "{}", repo.canon_yaml());
    let ignore = std::fs::read_to_string(repo.path().join(".gitignore")).unwrap();
    assert!(ignore.lines().any(|l| l == ".canon/artifacts/"), "{ignore}");

    // The gate now asks the store, and is green.
    std::fs::remove_file(repo.path().join("reports/smoke.json")).unwrap();
    let gate = repo.canon(&["gate", "check"]);
    assert!(gate.status.success(), "{}", text(&gate));
    assert!(repo.canon(&["artifacts", "verify", "--full"]).status.success());

    // Idempotent: nothing left to move.
    let yaml_after = repo.canon_yaml();
    let second = repo.ok(&["artifacts", "migrate", "--to", "cold", "--json"]);
    let body = stdout_json(&second);
    assert_eq!((body["data"]["blobs"].as_u64(), body["data"]["uploaded"].as_u64(), body["data"]["configWritten"].as_bool()), (Some(0), Some(0), Some(false)), "{body}");
    assert_eq!(repo.canon_yaml(), yaml_after);

    // Resumable: a run that stopped after switching the config but before
    // removing a copy leaves a local blob; the next run removes it.
    std::fs::create_dir_all(repo.path().join(".canon/artifacts/sha256")).unwrap();
    std::fs::write(repo.local_blob(V1), V1).unwrap();
    let resumed = repo.ok(&["artifacts", "migrate", "--to", "cold", "--json"]);
    let body = stdout_json(&resumed);
    assert_eq!((body["data"]["uploaded"].as_u64(), body["data"]["alreadyInStore"].as_u64(), body["data"]["removedLocal"].as_u64()), (Some(0), Some(1), Some(1)), "{body}");
    assert!(!repo.path().join(".canon/artifacts").exists());
}

/// A local blob that does not hash to its name is never migrated: the run
/// stops with nothing removed and canon.yaml untouched.
#[test]
fn migrate_refuses_a_corrupt_local_blob_and_changes_nothing() {
    let repo = new_repo(None);
    write_report(&repo, V1);
    assert!(repo.attest(&["--artifact", "reports/smoke.json"]).status.success());
    let bad = repo.path().join(".canon/artifacts/sha256").join(sha(b"what it should hold"));
    std::fs::write(&bad, b"what it holds").unwrap();
    let yaml = repo.canon_yaml();

    let run = repo.canon(&["artifacts", "migrate", "--to", "cold", "--json"]);
    assert_eq!(run.status.code(), Some(2), "{}", text(&run));
    let body = stdout_json(&run);
    assert_eq!(body["error"]["code"], "ARTIFACTS_MIGRATE_REFUSED");
    assert!(body["error"]["message"].as_str().unwrap().contains(&sha(b"what it should hold")), "{body}");
    assert!(repo.local_blob(V1).exists() && bad.exists(), "nothing is removed");
    assert_eq!(repo.canon_yaml(), yaml, "canon.yaml is unchanged");
    assert!(repo.canon(&["gate", "check"]).status.success(), "the repo still passes on its local store");
}

#[test]
fn migrate_refuses_a_target_that_is_not_an_s3_tier_and_never_rewrites_a_block_it_did_not_write() {
    let repo = new_repo(None);
    let hot = repo.canon(&["artifacts", "migrate", "--to", "hot"]);
    assert_eq!(hot.status.code(), Some(2), "{}", text(&hot));
    assert!(text(&hot).contains("ARTIFACTS_MIGRATE_REFUSED") && text(&hot).contains("not an object store"), "{}", text(&hot));

    let explicit = new_repo(Some("artifacts:\n  backend: local\n"));
    let out = explicit.canon(&["artifacts", "migrate", "--to", "cold"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(text(&out).contains("set it to `backend: cold` yourself"), "{}", text(&out));

    let local = repo.canon(&["artifacts", "migrate", "--to", "local"]);
    assert!(local.status.success() && text(&local).contains("already local"), "{}", text(&local));
}

/// `canon init --check-config` judges the `artifacts:` block like every other
/// section, so a bad backend fails there and not at the first evidence write.
#[test]
fn check_config_reports_the_artifacts_block() {
    let good = new_repo(Some(ARTIFACTS_COLD));
    let out = good.canon(&["init", "--check-config", "--repo", "."]);
    assert!(out.status.success() && text(&out).contains("[PASS] artifacts (backend: cold, prefix: artifacts/)"), "{}", text(&out));

    let none = new_repo(None);
    assert!(!text(&none.canon(&["init", "--check-config", "--repo", "."])).contains("artifacts"), "an absent block is the local default and says nothing");

    let bad = new_repo(Some("artifacts:\n  backend: warm\n"));
    let out = bad.canon(&["init", "--check-config", "--repo", "."]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert!(text(&out).contains("[FAIL] artifacts:") && text(&out).contains("neither `local` nor the name of a tier"), "{}", text(&out));
}
