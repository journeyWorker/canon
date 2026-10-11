//! review.add.40–43 (0.16 D5) through the real binary: `canon review add
//! --artifact` binds what the reviewer looked at exactly as `canon
//! evidence add --artifact` binds a file — stored under its digest,
//! recorded as `{path, sha256}` on the review — and `canon gate check`
//! re-hashes the stored blobs of every in-effect review.

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
        "review", "add", "--project-id", "root", "--scenario-id", "game.title.01", "--reviewer", reviewer, "--pin", "r1", "--original-spec-ref", "specs/title.feature", "--actor-id",
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
    assert!(text(&out).contains("canon review add: artifact store holds 2 bound file(s) under .canon/artifacts/sha256/ (2 newly stored)"), "{}", text(&out));
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

/// review.add.41: a missing or altered stored blob is `stale-evidence` on
/// the review's scenario, naming the review; an intact working-tree file
/// does not vouch for a review artifact.
#[test]
fn a_missing_or_altered_review_artifact_is_stale_evidence_naming_the_review() {
    let dir = setup();
    let repo = dir.path();
    assert!(review(repo, "reviewer-2", &["--artifact", "reports/title.png", "--artifact", "reports/hud.webm"]).status.success());

    std::fs::remove_file(blob(repo, TITLE)).unwrap();
    std::fs::write(blob(repo, HUD), b"tampered").unwrap();
    let gate = canon(repo, &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(1), "{}", text(&gate));
    let out = text(&gate);
    let missing = format!(
        "stale-evidence game.title.01 — review by `reviewer-2` (actor `reviewer-2`, {at}) bound file `reports/title.png` recorded sha256 {title}; stored blob .canon/artifacts/sha256/{title}: missing; working tree: {title} — the bound bytes are gone, look again and re-record the review with `canon review add --artifact <path>`",
        at = chrono::DateTime::parse_from_rfc3339(reviews(repo)[0]["at"].as_str().unwrap()).unwrap().to_utc().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        title = sha(TITLE)
    );
    assert!(out.contains(&missing), "a missing blob is stale even with the working-tree file intact:\n{out}\nexpected: {missing}");
    assert!(
        out.contains(&format!("bound file `reports/hud.webm` recorded sha256 {}; stored blob .canon/artifacts/sha256/{}: {}", sha(HUD), sha(HUD), sha(b"tampered")))
            && out.contains("the stored blob does not match its name, so its bytes were altered"),
        "{out}"
    );
}

/// review.add.42: a file outside the repository, a missing file, or one
/// over the size limit is refused (exit 2); nothing is written or stored.
#[test]
fn an_unbindable_review_artifact_is_refused_and_nothing_is_written() {
    let dir = setup();
    let repo = dir.path();
    let outside = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(outside.path(), b"elsewhere").unwrap();
    let big = vec![b'x'; 1024 * 1024 + 1];
    std::fs::write(repo.join("reports/big.webm"), &big).unwrap();

    for (args, expected) in [
        (vec!["--artifact", outside.path().to_str().unwrap()], "is outside the repository"),
        (vec!["--artifact", "reports/none.png"], "reports/none.png"),
        (vec!["--artifact", "reports/big.webm", "--max-artifact-mib", "1"], "over the artifact store's limit of 1048576 bytes; raise it with --max-artifact-mib"),
        (vec!["--artifact", "reports/title.png", "--artifact", "reports/none.png"], "reports/none.png"),
    ] {
        let out = review(repo, "reviewer-2", &args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", text(&out));
        assert!(text(&out).contains("canon review add: refused —") && text(&out).contains(expected), "{args:?}: {}", text(&out));
    }
    assert!(reviews(repo).is_empty(), "a refused review add writes nothing");
    assert!(!blob(repo, TITLE).exists(), "a refused review add stores nothing");
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
    assert_eq!(canon(repo, &["gate", "check"]).status.code(), Some(1));

    assert!(review(repo, "reviewer-2", &["--artifact", "reports/hud.webm"]).status.success());
    let gate = canon(repo, &["gate", "check"]);
    assert!(gate.status.success(), "{}", text(&gate));
}
