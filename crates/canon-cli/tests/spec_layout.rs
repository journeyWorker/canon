//! 0.17 D1 through the real binary: a spec root holds
//! `kind=feature/area=<area>/<surface>.feature` directly, a new repo's
//! root is `.canon/specs`, and a root still on the pre-0.17
//! `features/kind=feature/` layout is refused by `canon format`, `canon
//! inventory sync`, the readers' index refresh, `canon gate check`,
//! `canon status` and the scaffold, each naming the root and `canon
//! migrate`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn canon(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).env("CANON_ACTOR", "canon").current_dir(repo).output().expect("spawn canon")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn ok(repo: &Path, args: &[&str]) -> Output {
    let out = canon(repo, args);
    assert!(out.status.success(), "canon {args:?} failed:\n{}{}", stdout(&out), stderr(&out));
    out
}

fn git(repo: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(repo)
        .output()
        .expect("spawn git")
}

/// `git init` + `canon init --no-policy`.
fn init_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    assert!(git(dir.path(), &["init", "-q"]).status.success());
    assert!(git(dir.path(), &["commit", "-q", "--allow-empty", "-m", "x"]).status.success());
    ok(dir.path(), &["init", "--repo", ".", "--no-policy"]);
    dir
}

fn files_under(dir: PathBuf) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![dir];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn scenario_records(repo: &Path) -> Vec<serde_json::Value> {
    files_under(repo.join(".canon/ledger/kind=scenario")).iter().map(|p| serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()).collect()
}

const FEATURE: &str = ".canon/specs/kind=feature/area=world/hotdeal.feature";

/// A 0.16 repo: `canon.yaml` names the root `specs`, and its scenario
/// sits under the pre-0.17 `specs/features/kind=feature/`. `indexed`
/// syncs it while it is still on the 0.17 layout, so the index holds it
/// before the files move back.
fn pre_0_17_repo(indexed: bool) -> tempfile::TempDir {
    let dir = init_repo();
    let repo = dir.path();
    let yaml = std::fs::read_to_string(repo.join("canon.yaml")).unwrap().replace("root: .canon/specs", "root: specs");
    std::fs::write(repo.join("canon.yaml"), yaml).unwrap();
    ok(repo, &["scenario", "new", "world.hotdeal.01", "--title", "A coupon applies", "--case", "failure"]);
    if indexed {
        ok(repo, &["inventory", "sync"]);
    }
    std::fs::create_dir_all(repo.join("specs/features")).unwrap();
    std::fs::rename(repo.join("specs/kind=feature"), repo.join("specs/features/kind=feature")).unwrap();
    dir
}

const MIGRATE: &str = "spec root `root` (`specs`) is on the pre-0.17 layout (`features/kind=feature/area=world/hotdeal.feature` sits under `features/`); canon reads `kind=feature/area=<area>/<surface>.feature` directly under the root, and `canon migrate` moves `features/kind=feature/` there";

/// authoring.init.81: a fresh `canon init` repo's spec root is
/// `.canon/specs`; `canon scenario new` writes under
/// `.canon/specs/kind=feature/area=<area>/<surface>.feature`, the gate
/// grades it from the specs, and the next write indexes it.
#[test]
fn authoring_init_81_a_fresh_repo_authors_indexes_and_gates_under_canon_specs() {
    let dir = init_repo();
    let repo = dir.path();
    let yaml = std::fs::read_to_string(repo.join("canon.yaml")).unwrap();
    assert!(yaml.contains("specs:\n  # The .feature corpus: <root>/kind=feature/area=<area>/<surface>.feature.\n  roots:\n    - id: root\n      root: .canon/specs\n"), "{yaml}");

    let out = ok(repo, &["scenario", "new", "world.hotdeal.01", "--title", "A coupon applies", "--case", "failure"]);
    assert!(stdout(&out).contains(&format!("{FEATURE} —")), "{}", stdout(&out));
    assert!(repo.join(FEATURE).is_file());
    ok(repo, &["format", ".canon/specs"]);

    std::fs::write(repo.join(".canon/policy.yaml"), "spec_coverage:\n  require_evidence: true\n").unwrap();
    let gate = canon(repo, &["gate", "check"]);
    // The gate is a read command: it grades the new scenario from the specs, in memory, and writes no index record.
    assert!(!stderr(&gate).contains("refreshed the scenario index"), "{}", stderr(&gate));
    assert!(scenario_records(repo).is_empty(), "a read leaves the index as it found it: {:?}", scenario_records(repo));
    assert_eq!(gate.status.code(), Some(1), "{}", stdout(&gate));
    assert!(stdout(&gate).contains("world.hotdeal.01"), "the unevidenced scenario is graded: {}", stdout(&gate));

    let evidence = ok(repo, &["evidence", "add", "--scenario-id", "world.hotdeal.01", "--project-id", "root", "--class", "test-run", "--ref", "cargo test", "--role", "implementer", "--actor-id", "impl"]);
    assert!(stderr(&evidence).contains(&format!("canon evidence add: refreshed the scenario index from 1 changed feature file(s) ({FEATURE}): 1 record(s) written")), "a writer indexes the scenario: {}", stderr(&evidence));
    let records = scenario_records(repo);
    assert_eq!(records.len(), 1, "{records:?}");
    assert_eq!(records[0]["scenario_id"], "world.hotdeal.01");
    let gate = ok(repo, &["gate", "check"]);
    assert!(stdout(&gate).contains("canon gate check: clean (0 violations)"), "{}", stdout(&gate));
}

/// authoring.init.82: no `.gitignore` line canon writes, nor canon's own,
/// hides the `.canon/specs` corpus from git.
#[test]
fn authoring_init_82_the_canon_specs_corpus_is_never_ignored() {
    let dir = init_repo();
    let repo = dir.path();
    ok(repo, &["scenario", "new", "world.hotdeal.01", "--title", "A coupon applies", "--case", "failure"]);
    ok(repo, &["status"]);
    let ignored = |repo: &Path| git(repo, &["check-ignore", "-q", FEATURE]).status.code();
    assert_eq!(ignored(repo), Some(1), "the .gitignore `canon init` writes must not ignore {FEATURE}");
    let untracked = String::from_utf8(git(repo, &["status", "--porcelain", "--untracked-files=all"]).stdout).unwrap();
    assert!(untracked.lines().any(|line| line == format!("?? {FEATURE}")), "{untracked}");

    let canon_repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    assert_eq!(ignored(&canon_repo), Some(1), "canon's own .gitignore must not ignore {FEATURE}");
}

/// format.corpus.10: `canon format` refuses a root still on the pre-0.17
/// layout with one `layout-grammar` violation naming the root and
/// `canon migrate`, and reads none of its files: a malformed one under
/// `features/` raises no violation of its own.
#[test]
fn format_corpus_10_format_refuses_the_pre_0_17_layout() {
    let dir = pre_0_17_repo(false);
    // A second old-layout file that, if it were read, would raise its own
    // layout-grammar (no tag to resolve an area from) and missing-provenance
    // violations. It sorts after hotdeal, so the named first file is unchanged.
    let broken = "specs/features/kind=feature/area=world/zzz-broken.feature";
    std::fs::write(dir.path().join(broken), "Feature: broken\n\n  Scenario: no tag, no provenance\n    Given a step\n").unwrap();
    let out = canon(dir.path(), &["format", "specs"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    let text = stdout(&out);
    assert!(text.starts_with("canon format: 0 file(s) checked, 1 violation(s)\n"), "no old-layout file is read: {text}");
    assert!(!text.contains("zzz-broken") && !text.contains("missing-provenance"), "the unread file raises nothing of its own: {text}");
    assert!(text.contains("features"), "{text}");
    assert!(
        text.contains("spec root `specs` is on the pre-0.17 layout (`features/kind=feature/area=world/hotdeal.feature` sits under `features/`)"),
        "{text}"
    );
    assert!(text.contains("`canon migrate` moves `features/kind=feature/` there"), "{text}");
    assert!(text.contains("layout-grammar"), "{text}");
}

/// inventory.sync.48: `canon inventory sync` aborts a pre-0.17 root
/// whole, naming the root and `canon migrate`, and writes nothing.
#[test]
fn inventory_sync_48_sync_refuses_the_pre_0_17_layout() {
    let dir = pre_0_17_repo(false);
    let out = canon(dir.path(), &["inventory", "sync"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    let text = stdout(&out);
    assert!(text.contains("canon inventory sync: root `root` ("), "{text}");
    assert!(text.contains("ABORTED, 1 violation(s), 0 sync error(s), 0 record(s) written"), "{text}");
    assert!(text.contains("[layout-grammar] features — spec root `") && text.contains("`canon migrate` moves `features/kind=feature/` there"), "{text}");
    assert!(scenario_records(dir.path()).is_empty());
}

/// inventory.sync.49: a pre-0.17 root fails `canon gate check` closed
/// (an `uncovered-cell` naming the root and `canon migrate`), whether or
/// not the index already holds its scenarios; the refresh says the root
/// is not refreshed and writes no tombstone; `canon status`'s only step
/// is `canon migrate`. Moving the files to `kind=feature/` lifts it.
#[test]
fn inventory_sync_49_the_pre_0_17_layout_fails_the_gate_closed_and_status_names_migrate() {
    for indexed in [false, true] {
        let dir = pre_0_17_repo(indexed);
        let repo = dir.path();
        let gate = canon(repo, &["gate", "check"]);
        assert_eq!(gate.status.code(), Some(1), "indexed={indexed}: {}{}", stdout(&gate), stderr(&gate));
        assert!(stdout(&gate).contains(&format!("uncovered-cell root — {MIGRATE}; until then its scenarios are not graded")), "{}", stdout(&gate));
        let records = scenario_records(repo);
        assert_eq!(records.len(), usize::from(indexed), "{records:?}");
        assert!(records.iter().all(|r| r.get("removed").is_none()), "a refused root writes no tombstone: {records:?}");

        let report: serde_json::Value = serde_json::from_slice(&ok(repo, &["status", "--json"]).stdout).unwrap();
        let next = report["meta"]["next"].as_array().unwrap();
        assert_eq!(next.len(), 1, "{report}");
        assert_eq!(next[0]["command"], "canon migrate");
        assert!(next[0]["why"].as_str().unwrap().contains("spec root(s) `root`"), "{report}");

        std::fs::rename(repo.join("specs/features/kind=feature"), repo.join("specs/kind=feature")).unwrap();
        // Whatever the gate then finds (an empty corpus is `unverifiable`), it no longer names the old layout.
        let gate = canon(repo, &["gate", "check"]);
        assert!(!stdout(&gate).contains("pre-0.17"), "{}", stdout(&gate));
    }
}

/// authoring.scaffold.13: `canon scenario new` and `canon feature new`
/// refuse to write under a root still on the pre-0.17 layout (exit 2,
/// nothing written), naming the root and `canon migrate`.
#[test]
fn authoring_scaffold_13_the_scaffold_refuses_a_pre_0_17_root() {
    let dir = pre_0_17_repo(false);
    let repo = dir.path();
    let before = files_under(repo.join("specs"));

    let out = canon(repo, &["scenario", "new", "world.hotdeal.02", "--title", "Another coupon", "--case", "failure"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains(&format!("canon scenario new: LAYOUT_MIGRATION_REQUIRED — {MIGRATE}; nothing written")), "{}", stderr(&out));

    let out = canon(repo, &["scenario", "new", "world.cart.01", "--title", "A cart", "--feature", "specs/kind=feature/area=world/cart.feature"]);
    assert_eq!(out.status.code(), Some(2), "an explicit --feature under the root is refused too: {}", stderr(&out));
    assert!(stderr(&out).contains(&format!("canon scenario new: LAYOUT_MIGRATION_REQUIRED — {MIGRATE}; nothing written")), "{}", stderr(&out));
    assert!(!repo.join("specs/kind=feature/area=world/cart.feature").exists(), "the explicit --feature target is not written");
    assert_eq!(files_under(repo.join("specs")), before, "nothing written by the explicit --feature refusal");

    let out = canon(repo, &["feature", "new", "world.cart", "--title", "Cart"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains(&format!("canon feature new: LAYOUT_MIGRATION_REQUIRED — {MIGRATE}; nothing written")), "{}", stderr(&out));

    assert_eq!(files_under(repo.join("specs")), before, "nothing written");
}
