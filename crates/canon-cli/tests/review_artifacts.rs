//! review.add.40–43 (0.16 D5) and review.add.90–91 (0.18 D5) through the
//! real binary: `canon review scenario --artifact` binds what the reviewer
//! looked at exactly as `canon evidence add --artifact` binds a file —
//! stored under its digest, recorded as `{path, sha256}` on the review —
//! and `canon gate check` re-checks the files of every current in-effect
//! review by the evidence rule.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use sha2::{Digest, Sha256};

fn canon(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).env("CANON_ACTOR", "canon").current_dir(repo).output().expect("spawn canon")
}

fn text(output: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr))
}

fn ok(repo: &Path, args: &[&str]) -> String {
    let out = canon(repo, args);
    assert!(out.status.success(), "canon {args:?}: {}", text(&out));
    text(&out)
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn blob(repo: &Path, bytes: &[u8]) -> PathBuf {
    repo.join(".canon/artifacts/sha256").join(sha(bytes))
}

/// `git init` + `canon init --no-policy` + scenario `game.title.01`,
/// synced, and two screenshots under `reports/`. No policy: the
/// artifact check is not narrowed to any coverage scope.
fn setup() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    assert!(Command::new("git").args(["init", "-q"]).current_dir(repo).status().unwrap().success());
    ok(repo, &["init", "--repo", ".", "--no-policy"]);
    commit_all(repo);
    ok(repo, &["scenario", "new", "game.title.01", "--title", "The title screen shows the cat", "--case", "happy"]);
    ok(repo, &["inventory", "sync"]);
    std::fs::create_dir_all(repo.join("reports")).unwrap();
    std::fs::write(repo.join("reports/title.png"), TITLE).unwrap();
    std::fs::write(repo.join("reports/hud.webm"), HUD).unwrap();
    dir
}

const TITLE: &[u8] = b"\x89PNG title screen with the cat";
const HUD: &[u8] = b"webm recording of the HUD";

fn review(repo: &Path, reviewer: &str, extra: &[&str]) -> Output {
    let mut args = vec![
        "review", "scenario", "game.title.01", "--project", "root", "--original-spec-ref", "specs/title.feature", "--actor",
        reviewer, "--role", "reviewer",
    ];
    args.extend_from_slice(extra);
    canon(repo, &args)
}

/// Every review JSON under the ledger's `kind=review` partition.
fn reviews(repo: &Path) -> Vec<serde_json::Value> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "json") {
                out.push(path);
            }
        }
    }
    let mut paths = Vec::new();
    walk(&repo.join(".canon/ledger/kind=review"), &mut paths);
    paths.sort();
    paths.iter().map(|p| serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()).collect()
}

/// review.add.40: the files are stored under their digests and pinned on
/// the review by path and sha256; the gate is clean while the blobs are
/// intact, even after the working-tree files change.
#[test]
fn review_add_stores_each_artifact_and_records_its_path_and_digest() {
    let dir = setup();
    let repo = dir.path();
    let out = review(repo, "reviewer-2", &["--artifact", "reports/title.png", "--artifact", "reports/hud.webm", "--artifact", "reports/title.png"]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(text(&out).contains("canon review scenario: artifact store holds 2 bound file(s) under .canon/artifacts/sha256/ (2 newly stored)"), "{}", text(&out));
    assert_eq!(std::fs::read(blob(repo, TITLE)).unwrap(), TITLE);
    assert_eq!(std::fs::read(blob(repo, HUD)).unwrap(), HUD);

    let written = reviews(repo);
    assert_eq!(written.len(), 1);
    assert_eq!(
        written[0]["artifacts"],
        serde_json::json!([{"path": "reports/title.png", "sha256": sha(TITLE)}, {"path": "reports/hud.webm", "sha256": sha(HUD)}]),
        "each file once, in the order given"
    );

    // A review without --artifact keeps the field off the wire.
    assert!(review(repo, "reviewer-3", &[]).status.success());
    let plain = reviews(repo).into_iter().find(|r| r["reviewer"] == "reviewer-3").unwrap();
    assert!(plain.get("artifacts").is_none(), "{plain}");

    std::fs::write(repo.join("reports/title.png"), b"a later capture").unwrap();
    let gate = canon(repo, &["gate", "check"]);
    assert!(gate.status.success(), "{}", text(&gate));
    assert!(!text(&gate).contains("stale-evidence"), "{}", text(&gate));
}

/// review.add.41: an altered stored blob, or a missing one whose
/// working-tree file changed, is `stale-evidence` on the review's
/// scenario, naming the review.
#[test]
fn a_missing_or_altered_review_artifact_is_stale_evidence_naming_the_review() {
    let dir = setup();
    let repo = dir.path();
    assert!(review(repo, "reviewer-2", &["--artifact", "reports/title.png", "--artifact", "reports/hud.webm"]).status.success());

    std::fs::remove_file(blob(repo, TITLE)).unwrap();
    std::fs::write(repo.join("reports/title.png"), b"a later capture").unwrap();
    std::fs::write(blob(repo, HUD), b"tampered").unwrap();
    let gate = canon(repo, &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(1), "{}", text(&gate));
    let out = text(&gate);
    let stale = |file: &str| out.lines().any(|l| l.contains("game.title.01") && l.contains(file) && l.contains("reviewer-2"));
    assert!(stale("reports/title.png"), "a missing blob with a changed working-tree file is stale:\n{out}");
    assert!(stale("reports/hud.webm"), "an altered blob is stale:\n{out}");
    assert!(!out.contains("canon artifacts store"), "neither file is vouched for by the working tree:\n{out}");
}

/// review.add.90 (0.18 D5, #36): a lost review blob whose working-tree
/// file still matches is a vault advisory, as for evidence; `canon
/// artifacts store` stores it back. Once the working-tree file changes too,
/// it is `stale-evidence`.
#[test]
fn a_lost_review_blob_with_a_matching_working_tree_is_a_vault_advisory() {
    let dir = setup();
    let repo = dir.path();
    assert!(review(repo, "reviewer-2", &["--artifact", "reports/title.png"]).status.success());
    std::fs::remove_file(blob(repo, TITLE)).unwrap();

    let gate = canon(repo, &["gate", "check"]);
    assert!(gate.status.success(), "a matching working tree vouches for a lost review blob: {}", text(&gate));
    let out = text(&gate);
    assert!(!out.contains("stale-evidence"), "{out}");
    assert!(out.contains("not failing the gate; run `canon artifacts store`"), "{out}");
    let advisory = format!("  unstored game.title.01 reports/title.png (sha256 {}, review by `reviewer-2`) is verified against the working tree only", sha(TITLE));
    assert!(out.contains(&advisory), "{out}\nexpected: {advisory}");

    let vault = ok(repo, &["artifacts", "store"]);
    assert!(vault.contains("stored 1, already stored 0, could not store 0"), "{vault}");
    assert_eq!(std::fs::read(blob(repo, TITLE)).unwrap(), TITLE);
    let gate = ok(repo, &["gate", "check"]);
    assert!(!gate.contains("canon artifacts store"), "the stored blob clears the advisory: {gate}");

    std::fs::remove_file(blob(repo, TITLE)).unwrap();
    std::fs::write(repo.join("reports/title.png"), b"a later capture").unwrap();
    let gate = canon(repo, &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(1), "{}", text(&gate));
    assert!(text(&gate).contains("stale-evidence game.title.01 — review by `reviewer-2`") && text(&gate).contains("the bound bytes are gone"), "{}", text(&gate));
}

/// review.add.42: a file outside the repository, a missing file, or one
/// over the size limit is refused (exit 2); nothing is written, and the
/// artifact store is byte-for-byte what it was before each refusal.
#[test]
fn an_unbindable_review_artifact_is_refused_and_nothing_is_written() {
    let dir = setup();
    let repo = dir.path();
    let outside = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(outside.path(), b"elsewhere").unwrap();
    let big = vec![b'x'; 1024 * 1024 + 1];
    std::fs::write(repo.join("reports/big.webm"), &big).unwrap();
    // The store already holds one blob, so "unchanged" is not just "absent".
    ok(repo, &["evidence", "add", "--scenario", "game.title.01", "--project", "root", "--class", "test-run", "--ref", "npm run e2e", "--role", "implementer", "--artifact", "reports/hud.webm"]);
    let before = store_snapshot(repo);
    assert_eq!(before.len(), 1, "{before:?}");

    for (args, expected) in [
        (vec!["--artifact", outside.path().to_str().unwrap()], "is outside the repository"),
        (vec!["--artifact", "reports/none.png"], "reports/none.png"),
        (vec!["--artifact", "reports/big.webm", "--max-artifact-mib", "1"], "over the artifact store's limit of 1048576 bytes; raise it with --max-artifact-mib"),
        (vec!["--artifact", "reports/title.png", "--artifact", "reports/none.png"], "reports/none.png"),
    ] {
        let out = review(repo, "reviewer-2", &args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", text(&out));
        assert!(text(&out).starts_with("canon: REVIEW_ARTIFACT_REFUSED: ") && text(&out).contains("\n  field: --artifact\n") && text(&out).contains(expected), "{args:?}: {}", text(&out));
        assert_eq!(store_snapshot(repo), before, "{args:?}: a refused review add stores no bytes at all");
    }
    assert!(reviews(repo).is_empty(), "a refused review add writes nothing");
}

/// Every entry under `.canon/artifacts/` (temp files included), with its
/// bytes, sorted by path.
fn store_snapshot(repo: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                out.push((path.clone(), std::fs::read(&path).unwrap()));
            }
        }
    }
    let mut out = Vec::new();
    walk(&repo.join(".canon/artifacts"), &mut out);
    out.sort();
    out
}

/// review.add.43: only in-effect reviews are checked. A reviewer's later
/// review replaces the earlier one, so the earlier review's lost blob no
/// longer fails the gate.
#[test]
fn a_superseded_reviews_artifacts_are_no_longer_checked() {
    let dir = setup();
    let repo = dir.path();
    assert!(review(repo, "reviewer-2", &["--artifact", "reports/title.png"]).status.success());
    std::fs::remove_file(blob(repo, TITLE)).unwrap();
    std::fs::write(repo.join("reports/title.png"), b"a later capture").unwrap();
    assert_eq!(canon(repo, &["gate", "check"]).status.code(), Some(1));

    assert!(review(repo, "reviewer-2", &["--artifact", "reports/hud.webm"]).status.success());
    let gate = canon(repo, &["gate", "check"]);
    assert!(gate.status.success(), "{}", text(&gate));
}

/// review.add.91 (0.18 D5, #36, FINDINGS-3 T5): a review that went stale
/// when the scenario's evidence moved, and that another reviewer has
/// reviewed again since, is superseded: its lost files are neither
/// checked, nor listed as an advisory, nor vaulted. The newer, current
/// review's files still are checked.
#[test]
fn a_stale_review_superseded_by_another_reviewers_newer_review_is_not_checked() {
    let dir = setup();
    let repo = dir.path();
    assert!(review(repo, "reviewer-2", &["--artifact", "reports/title.png"]).status.success());
    ok(
        repo,
        &[
            "evidence", "add", "--scenario", "game.title.01", "--project", "root", "--class", "test-run", "--ref", "vitest", "--role", "implementer", "--actor", "impl",
            "--promote",
        ],
    );
    assert!(review(repo, "reviewer-4", &["--artifact", "reports/hud.webm"]).status.success());

    // Lost while the working-tree file still matches: no advisory, and
    // the vault stores only the current review's file (already stored).
    std::fs::remove_file(blob(repo, TITLE)).unwrap();
    let gate = ok(repo, &["gate", "check"]);
    assert!(!gate.contains("reports/title.png") && !gate.contains("canon artifacts store"), "{gate}");
    let vault = ok(repo, &["artifacts", "store"]);
    assert!(vault.contains("canon artifacts store: 1 bound file(s) — stored 0, already stored 1, could not store 0"), "{vault}");
    assert!(!blob(repo, TITLE).exists(), "a superseded review's file is not vaulted");

    std::fs::write(repo.join("reports/title.png"), b"a later capture").unwrap();
    let gate = canon(repo, &["gate", "check"]);
    assert!(gate.status.success(), "the stale reviewer-2 review is superseded, so its lost file is not checked: {}", text(&gate));
    assert!(!text(&gate).contains("reports/title.png"), "{}", text(&gate));

    std::fs::remove_file(blob(repo, HUD)).unwrap();
    std::fs::write(repo.join("reports/hud.webm"), b"a later recording").unwrap();
    let gate = canon(repo, &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(1), "the current review's lost file is still checked: {}", text(&gate));
    let out = text(&gate);
    assert!(out.contains("stale-evidence game.title.01 — review by `reviewer-4`") && out.contains("`reports/hud.webm`"), "{out}");
    assert!(!out.contains("review by `reviewer-2`"), "{out}");
}

/// Commit the fixture so revision-taking commands have a commit to bind.
fn commit_all(repo: &std::path::Path) {
    for args in [&["config", "user.name", "t"][..], &["config", "user.email", "t@example.com"], &["add", "-A"], &["commit", "-q", "-m", "x"]] {
        assert!(std::process::Command::new("git").args(args).current_dir(repo).status().unwrap().success());
    }
}
