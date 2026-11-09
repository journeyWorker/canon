//! 0.40 `canon-0-40-object-store-zstd` through the real binary: the evidence blobs of an
//! object-store tier are zstd frames at `<prefix>sha256/<digest>.zst`; an older plain object stays
//! valid; the local git store is never compressed. Scenario `artifacts.object-zstd.NN` is the test
//! `artifacts_object_zstd_NN_…`; the unit half of 01-06, 09 and 10 is in `canon-store`'s
//! `artifact_blobs` tests.
//!
//! The object store is the debug-build local-filesystem substitute (`CANON_R2_LOCAL_ROOT`).

#[path = "support/hermetic.rs"]
mod hermetic;
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

    fn canon(&self, args: &[&str]) -> Output {
        hermetic::canon().args(args).env("CANON_ACTOR", "canon").env("CANON_R2_LOCAL_ROOT", self.store.path()).current_dir(self.path()).output().expect("spawn canon")
    }

    /// `canon` against a bucket nothing listens at (what a release binary uses, with the substitute store removed).
    fn canon_offline(&self, args: &[&str]) -> Output {
        hermetic::canon()
            .args(args)
            .env("CANON_ACTOR", "canon")
            .env_remove("CANON_R2_LOCAL_ROOT")
            .env("CANON_S3_ENDPOINT", "http://127.0.0.1:9")
            .env("CANON_R2_BUCKET", "evidence")
            .current_dir(self.path())
            .output()
            .expect("spawn canon")
    }

    fn ok(&self, args: &[&str]) -> Output {
        let out = self.canon(args);
        assert!(out.status.success(), "canon {args:?}: {}", text(&out));
        out
    }

    /// `artifacts move --tier cold`: the preview, then `--commit --plan-digest` of it; both `--json`.
    fn move_to_cold(&self) -> Output {
        let preview = self.canon(&["artifacts", "move", "--tier", "cold", "--json"]);
        assert!(preview.status.success(), "{}", text(&preview));
        let digest = json(&preview)["data"]["planDigest"].as_str().unwrap().to_string();
        self.canon(&["artifacts", "move", "--tier", "cold", "--commit", "--plan-digest", &digest, "--json"])
    }

    fn bucket(&self) -> PathBuf {
        self.store.path().join("artifacts/sha256")
    }

    /// The `.zst` object of `bytes`.
    fn zst(&self, bytes: &[u8]) -> PathBuf {
        self.bucket().join(format!("{}.zst", sha(bytes)))
    }

    /// The plain object of `bytes` (what canon 0.39 wrote).
    fn plain(&self, bytes: &[u8]) -> PathBuf {
        self.bucket().join(sha(bytes))
    }

    /// Put the object an older canon wrote for `bytes`.
    fn put_plain(&self, bytes: &[u8]) {
        std::fs::create_dir_all(self.bucket()).unwrap();
        std::fs::write(self.plain(bytes), bytes).unwrap();
    }

    fn put_zst(&self, bytes: &[u8]) {
        std::fs::create_dir_all(self.bucket()).unwrap();
        std::fs::write(self.zst(bytes), zstd::encode_all(bytes, 3).unwrap()).unwrap();
    }

    fn local_blob(&self, bytes: &[u8]) -> PathBuf {
        self.path().join(".canon/artifacts/sha256").join(sha(bytes))
    }

    fn objects(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.bucket()).map(|dir| dir.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
        names.sort();
        names
    }

    fn report(&self, name: &str, bytes: &[u8]) {
        std::fs::create_dir_all(self.path().join("reports")).unwrap();
        std::fs::write(self.path().join("reports").join(name), bytes).unwrap();
    }

    /// Bind `reports/<name>` to the scenario's evidence and promote it.
    fn attest(&self, name: &str, bytes: &[u8]) {
        self.report(name, bytes);
        let artifact = format!("reports/{name}");
        self.ok(&["evidence", "add", "--scenario", "game.run.01", "--project", "root", "--class", "test-run", "--ref", "npm run smoke", "--role", "implementer", "--artifact", &artifact]);
        self.ok(&["gate", "promote"]);
    }

    fn yaml(&self) -> String {
        std::fs::read_to_string(self.path().join("canon.yaml")).unwrap()
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

const COLD_STANZA: &str = "  cold:\n    backend: s3\n    bucket_env: CANON_R2_BUCKET\n    prefix: \"canon/\"\n";
const ARTIFACTS_COLD: &str = "artifact_store:\n  backend: cold\n  prefix: artifacts/\n";

fn new_repo(artifacts: Option<&str>) -> Repo {
    let repo = Repo { dir: tempfile::tempdir().unwrap(), store: tempfile::tempdir().unwrap() };
    assert!(Command::new("git").args(["init", "-q"]).current_dir(repo.path()).status().unwrap().success());
    repo.ok(&["init", "--repo", ".", "--no-policy"]);
    for args in [&["config", "user.name", "t"][..], &["config", "user.email", "t@example.com"], &["add", "-A"], &["commit", "-q", "-m", "x"]] {
        assert!(Command::new("git").args(args).current_dir(repo.path()).status().unwrap().success());
    }
    let marker = "  # cold:\n  #   backend: s3\n  #   bucket_env: CANON_R2_BUCKET\n  #   prefix: \"canon/\"\n";
    let mut yaml = repo.yaml();
    assert!(yaml.contains(marker), "the skeleton's commented cold tier moved:\n{yaml}");
    yaml = yaml.replace(marker, COLD_STANZA);
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
const O1: &[u8] = b"an orphan report, first";

/// artifacts.object-zstd.01 (F1): `evidence add --artifact` with a tier uploads `<digest>.zst`, a zstd frame that decompresses to the bound file; no plain
/// object and no local copy exist, the working tree can change and the gate still passes.
#[test]
fn artifacts_object_zstd_01_evidence_add_uploads_a_zst_object_and_keeps_no_local_copy() {
    let repo = new_repo(Some(ARTIFACTS_COLD));
    repo.attest("smoke.json", V1);
    assert_eq!(repo.objects(), vec![format!("{}.zst", sha(V1))], "one `.zst` object and no plain object");
    assert_eq!(zstd::decode_all(&std::fs::read(repo.zst(V1)).unwrap()[..]).unwrap(), V1);
    assert!(!repo.path().join(".canon/artifacts").exists(), "no local copy is kept");
    repo.report("smoke.json", br#"{"at":"later"}"#);
    let gate = repo.canon(&["gate", "check"]);
    assert!(gate.status.success() && !text(&gate).contains("stale-evidence"), "{}", text(&gate));
    let again = repo.canon(&["artifacts", "store"]);
    assert!(again.status.success(), "{}", text(&again));
    assert_eq!(repo.objects().len(), 1, "storing the same bytes again adds no object");
}

/// artifacts.object-zstd.02 (F1 failure): a file that changed after its digest was bound is not uploaded: the tier holds no object of either form
/// (the unit test of the same id drives `put` itself with bytes that do not hash to the digest).
#[test]
fn artifacts_object_zstd_02_a_file_that_changed_since_it_was_bound_leaves_the_tier_empty() {
    let repo = new_repo(Some(ARTIFACTS_COLD));
    repo.attest("smoke.json", V1);
    std::fs::remove_file(repo.zst(V1)).unwrap();
    repo.report("smoke.json", b"rewritten after the record bound it");
    let out = repo.canon(&["artifacts", "store", "--json"]);
    let text = text(&out);
    assert!(text.contains(&sha(V1)), "the digest is named as not stored: {text}");
    assert!(repo.objects().is_empty(), "no object of either form: {:?}", repo.objects());
}

/// artifacts.object-zstd.03 (F2): `.zst` only and plain only answer the same to `gate check` and `artifacts verify`.
#[test]
fn artifacts_object_zstd_03_gate_check_and_verify_answer_the_same_for_zst_and_plain_objects() {
    let repo = new_repo(Some(ARTIFACTS_COLD));
    repo.attest("smoke.json", V1);
    let answer = |repo: &Repo| (repo.canon(&["gate", "check"]).status.code(), repo.canon(&["artifacts", "verify"]).status.code(), repo.canon(&["artifacts", "verify", "--full"]).status.code());
    let new = answer(&repo);
    assert_eq!(new, (Some(0), Some(0), Some(0)));
    // The same digest as an older canon left it.
    std::fs::remove_file(repo.zst(V1)).unwrap();
    repo.put_plain(V1);
    assert_eq!(answer(&repo), new);
    // Both forms: still the same answer.
    repo.put_zst(V1);
    assert_eq!(answer(&repo), new);
    let verify = json(&repo.canon(&["artifacts", "verify", "--json"]));
    assert_eq!(verify["data"]["problems"].as_array().map_or(0, Vec::len), 0, "{verify}");
}

/// artifacts.object-zstd.04 (F2 failure): no object of either form is `missing` (exit 1, `stale-evidence`).
#[test]
fn artifacts_object_zstd_04_a_digest_with_no_object_is_missing_and_stale() {
    let repo = new_repo(Some(ARTIFACTS_COLD));
    repo.attest("smoke.json", V1);
    std::fs::remove_file(repo.zst(V1)).unwrap();
    let gate = repo.canon(&["gate", "check"]);
    assert_eq!(gate.status.code(), Some(1), "{}", text(&gate));
    assert!(text(&gate).contains("stale-evidence game.run.01") && text(&gate).contains(": missing"), "{}", text(&gate));
    let verify = repo.canon(&["artifacts", "verify", "--json"]);
    assert_eq!(verify.status.code(), Some(1));
    assert_eq!(json(&verify)["data"]["problems"][0]["sha256"], sha(V1));
    assert!(text(&gate).contains(&format!("{}.zst", sha(V1))), "the message names the `.zst` key the operator opens: {}", text(&gate));
}

/// artifacts.object-zstd.05 (F3): `verify --full` decompresses the `.zst` object and passes.
#[test]
fn artifacts_object_zstd_05_verify_full_passes_on_a_zst_object() {
    let repo = new_repo(Some(ARTIFACTS_COLD));
    repo.attest("smoke.json", V1);
    let full = repo.canon(&["artifacts", "verify", "--full"]);
    assert!(full.status.success() && text(&full).contains("re-hashed"), "{}", text(&full));
}

/// artifacts.object-zstd.06 (F3 failure): a changed byte, a cut tail and a non-zstd file are each altered with exit 1; an unreachable tier is exit 2.
#[test]
fn artifacts_object_zstd_06_a_damaged_zst_object_is_altered_and_an_unreachable_tier_is_exit_2() {
    let repo = new_repo(Some(ARTIFACTS_COLD));
    repo.attest("smoke.json", V1);
    let good = std::fs::read(repo.zst(V1)).unwrap();
    let mut flipped = good.clone();
    let middle = flipped.len() - 6;
    flipped[middle] ^= 0x01;
    for (what, body) in [("a changed byte", flipped), ("a cut tail", good[..good.len() - 4].to_vec()), ("not zstd", b"plain text, no frame".to_vec())] {
        std::fs::write(repo.zst(V1), &body).unwrap();
        let full = repo.canon(&["artifacts", "verify", "--full", "--json"]);
        assert_eq!(full.status.code(), Some(1), "{what}: {}", text(&full));
        assert_eq!(json(&full)["error"]["code"], "ARTIFACTS_VERIFY_FAILED", "{what}");
    }
    std::fs::write(repo.zst(V1), &good).unwrap();
    assert!(repo.canon(&["artifacts", "verify", "--full"]).status.success());
    let offline = repo.canon_offline(&["artifacts", "verify", "--full", "--json"]);
    assert_eq!(offline.status.code(), Some(2), "{}", text(&offline));
    assert_eq!(json(&offline)["error"]["code"], "ARTIFACTS_STORE_UNAVAILABLE");
}

/// artifacts.object-zstd.07 (F4): `artifacts move` uploads two local blobs as `.zst`, removes the local copies and passes `verify --full`; a digest the tier
/// already holds as a plain object is not uploaded again (no `.zst` appears) and its local copy goes.
#[test]
fn artifacts_object_zstd_07_move_uploads_zst_objects_and_skips_a_digest_the_tier_holds() {
    let repo = new_repo(None);
    repo.attest("smoke.json", V1);
    repo.attest("second.json", V2);
    assert!(repo.local_blob(V1).exists() && repo.local_blob(V2).exists());
    repo.put_plain(V2);
    let run = repo.move_to_cold();
    assert!(run.status.success(), "{}", text(&run));
    let body = json(&run);
    assert_eq!((body["data"]["uploaded"].as_u64(), body["data"]["alreadyInStore"].as_u64(), body["data"]["removedLocal"].as_u64()), (Some(1), Some(1), Some(2)), "{body}");
    assert_eq!(repo.objects().len(), 2);
    assert!(repo.zst(V1).exists() && repo.plain(V2).exists() && !repo.zst(V2).exists(), "{:?}", repo.objects());
    assert_eq!(zstd::decode_all(&std::fs::read(repo.zst(V1)).unwrap()[..]).unwrap(), V1);
    assert!(!repo.path().join(".canon/artifacts").exists());
    assert!(repo.canon(&["artifacts", "verify", "--full"]).status.success());
}

/// artifacts.object-zstd.08 (F4 failure): a `.zst` the tier already holds that does not decompress to its digest is `ARTIFACTS_BLOB_CORRUPT` and the local copy stays.
#[test]
fn artifacts_object_zstd_08_move_refuses_a_tier_zst_that_does_not_decompress_to_its_digest() {
    let repo = new_repo(None);
    repo.attest("smoke.json", V1);
    std::fs::create_dir_all(repo.bucket()).unwrap();
    std::fs::write(repo.zst(V1), zstd::encode_all(&b"some other bytes"[..], 3).unwrap()).unwrap();
    let yaml = repo.yaml();
    let run = repo.move_to_cold();
    assert_eq!(run.status.code(), Some(2), "{}", text(&run));
    assert_eq!(json(&run)["error"]["code"], "ARTIFACTS_BLOB_CORRUPT");
    assert!(repo.local_blob(V1).exists(), "the local copy stays");
    assert_eq!(repo.yaml(), yaml, "canon.yaml is untouched");
}

/// artifacts.object-zstd.09 (F5): a digest held as `.zst` and as a plain object, bound by no record, is one entry of the preview with both objects' bytes,
/// and `--commit` deletes both; the blob a record binds stays and passes `verify --full`.
#[test]
fn artifacts_object_zstd_09_prune_lists_both_forms_of_a_digest_once_and_deletes_both() {
    let repo = new_repo(Some(ARTIFACTS_COLD));
    repo.attest("smoke.json", V2);
    repo.put_zst(O1);
    repo.put_plain(O1);
    let both = std::fs::metadata(repo.zst(O1)).unwrap().len() + O1.len() as u64;
    let preview = repo.ok(&["artifacts", "prune", "--json"]);
    let body = json(&preview);
    assert_eq!((body["data"]["delete"]["blobs"].as_u64(), body["data"]["delete"]["tierObjects"].as_u64(), body["data"]["delete"]["bytes"].as_u64()), (Some(1), Some(1), Some(both)), "{body}");
    assert!(body["data"]["irreversible"].as_str().unwrap().contains("1 object(s)"), "{body}");
    assert!(repo.zst(O1).exists() && repo.plain(O1).exists(), "a preview deletes nothing");
    let argv: Vec<String> = body["meta"]["next"][0]["argv"].as_array().unwrap().iter().map(|word| word.as_str().unwrap().to_string()).collect();
    let args: Vec<&str> = argv.iter().map(String::as_str).collect();
    let done = repo.canon(&args);
    assert!(done.status.success(), "{}", text(&done));
    assert_eq!(repo.objects(), vec![format!("{}.zst", sha(V2))], "both names of the orphan are gone, the bound blob stays");
    assert!(repo.canon(&["artifacts", "verify", "--full"]).status.success());
    // The state the plan read includes the form of each object: a form that appears after the preview makes the plan stale.
    repo.put_zst(O1);
    let stale_preview = repo.ok(&["artifacts", "prune", "--json"]);
    let digest = json(&stale_preview)["data"]["planDigest"].as_str().unwrap().to_string();
    repo.put_plain(O1);
    let stale = repo.canon(&["artifacts", "prune", "--commit", "--plan-digest", &digest, "--json"]);
    assert_eq!(stale.status.code(), Some(2), "{}", text(&stale));
    assert_eq!(json(&stale)["error"]["code"], "PLAN_STALE");
}

/// artifacts.object-zstd.10 (F5 failure): the refused-second-delete case needs a store that refuses one name; `canon-store`'s test of the same id drives it.
/// Here: a repeat run after an interrupted one (only the plain name remains) deletes the name that is left, and never touches the blob a record binds.
#[test]
fn artifacts_object_zstd_10_a_repeat_prune_deletes_the_name_that_remains_and_keeps_the_bound_blob() {
    let repo = new_repo(Some(ARTIFACTS_COLD));
    repo.attest("smoke.json", V2);
    repo.put_plain(O1); // the `.zst` of it went in an earlier, interrupted run
    let preview = repo.ok(&["artifacts", "prune", "--json"]);
    let body = json(&preview);
    assert_eq!(body["data"]["delete"]["tierObjects"], 1, "{body}");
    let argv: Vec<String> = body["meta"]["next"][0]["argv"].as_array().unwrap().iter().map(|word| word.as_str().unwrap().to_string()).collect();
    let args: Vec<&str> = argv.iter().map(String::as_str).collect();
    assert!(repo.canon(&args).status.success());
    assert_eq!(repo.objects(), vec![format!("{}.zst", sha(V2))]);
    assert_eq!(json(&repo.ok(&["artifacts", "prune", "--json"]))["data"]["delete"]["blobs"], 0, "nothing is left to delete");
}

/// artifacts.object-zstd.11 (F6): with the local store `evidence add`, `artifacts store` and `prune` write the plain blob and never a `.zst`; the object store is untouched.
#[test]
fn artifacts_object_zstd_11_the_local_store_is_never_compressed() {
    let repo = new_repo(None);
    repo.attest("smoke.json", V1);
    repo.ok(&["artifacts", "store"]);
    assert_eq!(std::fs::read(repo.local_blob(V1)).unwrap(), V1, "the blob is the plain bytes");
    let dir = repo.path().join(".canon/artifacts/sha256");
    repo.ok(&["artifacts", "prune"]);
    let names: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, vec![sha(V1)], "{names:?}");
    assert!(repo.objects().is_empty());
}

/// artifacts.object-zstd.12 (F6 failure): a `<digest>.zst` in the local blob directory is no blob: the store reads the digest as missing, `verify` and
/// `move` skip it, `prune` does not delete it.
#[test]
fn artifacts_object_zstd_12_a_zst_file_in_the_local_directory_is_not_a_blob() {
    let repo = new_repo(None);
    repo.attest("smoke.json", V1);
    let dir = repo.path().join(".canon/artifacts/sha256");
    let stray = dir.join(format!("{}.zst", sha(O1)));
    std::fs::write(&stray, zstd::encode_all(O1, 3).unwrap()).unwrap();
    let verify = repo.canon(&["artifacts", "verify", "--json"]);
    assert!(verify.status.success() && json(&verify)["data"]["problems"].as_array().map_or(0, Vec::len) == 0, "{}", text(&verify));
    let preview = json(&repo.ok(&["artifacts", "move", "--tier", "cold", "--json"]));
    let uploads = preview["data"]["effects"].as_array().unwrap().iter().filter(|e| e["action"] == "upload").count();
    assert_eq!(uploads, 1, "only the real blob would move: {preview}");
    let prune = json(&repo.ok(&["artifacts", "prune", "--json"]));
    assert_eq!(prune["data"]["delete"]["blobs"], 0, "{prune}");
    assert!(stray.exists());
    // A record bound to V1 whose only local trace is `<digest>.zst` reads as missing.
    std::fs::remove_file(repo.local_blob(V1)).unwrap();
    std::fs::remove_file(repo.path().join("reports/smoke.json")).unwrap();
    std::fs::write(dir.join(format!("{}.zst", sha(V1))), zstd::encode_all(V1, 3).unwrap()).unwrap();
    let gate = repo.canon(&["gate", "check"]);
    assert_eq!(gate.status.code(), Some(1), "{}", text(&gate));
    assert!(text(&gate).contains("stale-evidence game.run.01"), "{}", text(&gate));
}

/// What the release lacks of the announcement of the `.zst` format: the Unreleased CHANGELOG says, under Added, the key, and under Changed, the order
/// of upgrade and the plain objects of older canons; both guides name the `.zst` key; the storage guide says how to decompress. The release check.
fn release_problems(guides: &[(&str, String)], changelog: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let flat = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    for (name, text) in guides {
        if !flat(text).contains("<prefix>sha256/<digest>.zst") {
            problems.push(format!("the guide `{name}` never names the key `<prefix>sha256/<digest>.zst`"));
        }
    }
    if let Some((_, text)) = guides.iter().find(|(name, _)| *name == "canon-storage") {
        if !flat(text).contains("zstd -d") {
            problems.push("the guide `canon-storage` never says how to decompress a blob (`zstd -d`)".to_string());
        }
    }
    let unreleased = changelog.split("\n## ").find(|section| section.starts_with("Unreleased"));
    let part = |name: &str| unreleased.and_then(|section| section.split("\n### ").find(|part| part.starts_with(name))).map(flat);
    match part("Added") {
        None => problems.push("the CHANGELOG has no Added section under Unreleased".to_string()),
        Some(added) => {
            for needle in ["<digest>.zst", "zstd -d", "level 19"] {
                if !added.contains(needle) {
                    problems.push(format!("the Added section never says `{needle}`"));
                }
            }
        }
    }
    match part("Changed") {
        None => problems.push("the CHANGELOG has no Changed section under Unreleased".to_string()),
        Some(changed) => {
            for (what, needle) in [("that every canon reading the tier is upgraded first", "before the first `.zst` is written"), ("that a canon 0.39 or older reads a `.zst` blob as missing", "0.39 or older"), ("that a plain object stays valid", "plain object")] {
                if !changed.contains(needle) {
                    problems.push(format!("the Changed section never says {what} (`{needle}`)"));
                }
            }
        }
    }
    problems
}

fn guides(repo: &Repo) -> Vec<(&'static str, String)> {
    ["canon-storage", "canon-gate"].iter().map(|guide| (*guide, String::from_utf8_lossy(&repo.ok(&["skills", "show", guide]).stdout).into_owned())).collect()
}

fn changelog() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CHANGELOG.md")).unwrap()
}

/// artifacts.object-zstd.13 (F7): the CHANGELOG and both guides say the `.zst` key, the order of upgrade, the older canon's reading and `zstd -d`.
#[test]
fn artifacts_object_zstd_13_the_guides_and_the_changelog_announce_the_format() {
    let repo = new_repo(None);
    assert_eq!(release_problems(&guides(&repo), &changelog()), Vec::<String>::new());
}

/// artifacts.object-zstd.14 (F7 failure): a guide or CHANGELOG that lacks one of the items is named, each by what it lacks.
#[test]
fn artifacts_object_zstd_14_a_release_without_the_announcement_fails_the_release_check() {
    let repo = new_repo(None);
    let (guides, changelog) = (guides(&repo), changelog());
    assert_eq!(release_problems(&guides, &changelog), Vec::<String>::new(), "the check passes on the real text");
    let mut bare = guides.clone();
    bare[0].1 = bare[0].1.replace("<prefix>sha256/<digest>.zst", "removed");
    assert!(release_problems(&bare, &changelog).iter().any(|p| p.contains("`canon-storage` never names the key")), "{:?}", release_problems(&bare, &changelog));
    let mut silent = guides.clone();
    silent[0].1 = silent[0].1.replace("zstd -d", "removed");
    assert!(release_problems(&silent, &changelog).iter().any(|p| p.contains("`zstd -d`")), "{:?}", release_problems(&silent, &changelog));
    let none = "## Unreleased\n\n### Fixed\n\n- Something else.\n\n## 0.39.0\n";
    let problems = release_problems(&guides, none);
    assert_eq!(problems, ["the CHANGELOG has no Added section under Unreleased", "the CHANGELOG has no Changed section under Unreleased"], "{problems:?}");
    let without_order = changelog.replace("before the first `.zst` is written", "later");
    assert!(release_problems(&guides, &without_order).iter().any(|p| p.contains("upgraded first")));
}
