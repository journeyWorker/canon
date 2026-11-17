//! `canon migrate` (0.17 D6) through the real binary, on repos laid out
//! the way canon 0.16 left them: the spec corpus under
//! `specs/features/kind=feature/`, `canon change new`'s changes under
//! `openspec/changes/`, approvals in `proposal.md`, no `skills:` key and a
//! `.gitignore` missing canon's newer lines.

#[path = "support/hermetic.rs"]
mod hermetic;
use std::path::Path;
use std::process::{Command, Output};

use canon_model::RecordKind;

/// `canon.yaml` as `canon init` 0.16 wrote it (comments trimmed).
const OLD_YAML: &str = "\
# canon.yaml -- scaffolded by `canon init` (s19 canon-init-scaffold).
tiers:
  local:
    backend: git
    root: .canon/ledger
  hot:
    backend: sqlite
    path: .canon/hot.db
routing:
  change: local
  subject: local
  scenario: local
  review: local
  divergence: local
  evidence: local
  finding: local
  strategy_item: local
  task: hot
  handoff: hot
  session: hot
  run: hot
  event: hot
  trajectory: local
specs:
  roots:
    - id: root
      root: specs
plans:
  # Where changes and their tasks live: `canon change new` scaffolds
  # openspec/changes/<slug>/, `canon ingest plans` imports it.
  sources:
    - dialect: openspec
      root: .
write:
  promote: auto
";

const OLD_FEATURE: &str = "specs/features/kind=feature/area=cart/add.feature";
const NEW_FEATURE: &str = ".canon/specs/kind=feature/area=cart/add.feature";

const CART: &str = r#"Feature: cart add
  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @subject:cart
  @case:happy
  @cart.add.01
  Scenario: Adding an item puts it in the cart
    When the shopper adds an item
    Then the cart holds that item
"#;

fn canon(repo: &Path, args: &[&str]) -> Output {
    hermetic::canon().args(args).env("CANON_ACTOR", "canon").current_dir(repo).output().expect("spawn canon")
}

fn text(out: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

fn ok(repo: &Path, args: &[&str]) -> String {
    let out = canon(repo, args);
    assert!(out.status.success(), "canon {args:?}: {}", text(&out));
    text(&out)
}

/// `canon migrate` as an agent applies it: the bare run previews, then
/// `--commit --plan-digest` acts on that plan. A preview that refuses is
/// returned as it is.
fn commit(repo: &Path) -> Output {
    let preview = canon(repo, &["migrate"]);
    let stdout = String::from_utf8_lossy(&preview.stdout).into_owned();
    let Some(digest) = stdout.lines().find_map(|line| line.strip_prefix("planDigest ")).and_then(|rest| rest.split(';').next()) else { return preview };
    canon(repo, &["migrate", "--commit", "--plan-digest", digest])
}

fn ok_commit(repo: &Path) -> String {
    let out = commit(repo);
    assert!(out.status.success(), "canon migrate --commit: {}", text(&out));
    text(&out)
}

fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(repo)
        .output()
        .expect("git must be on PATH");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn write(repo: &Path, path: &str, body: &str) {
    let file = repo.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, body).unwrap();
}

fn read(repo: &Path, path: &str) -> String {
    std::fs::read_to_string(repo.join(path)).unwrap()
}

fn commit_all(repo: &Path, message: &str) -> String {
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", message]);
    git(repo, &["rev-parse", "HEAD"])
}

/// A stored ledger record, written the way the ledger lays it out.
fn plant(repo: &Path, kind: RecordKind, record: serde_json::Value) {
    let path = repo.join(".canon/ledger").join(canon_store::partition::expected_relative_path(kind, &record).unwrap());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, serde_json::to_vec_pretty(&record).unwrap()).unwrap();
}

/// `proposal.md` and `tasks.md` as `canon change new` 0.14–0.16 wrote
/// them, with whatever the agent appended after.
fn scaffold(repo: &Path, slug: &str, title: &str, proposal_tail: &str, tasks_tail: &str) {
    write(
        repo,
        &format!("openspec/changes/{slug}/proposal.md"),
        &format!("# {title}\n\n## Why\n\n{title}\n\n## What Changes\n\n<!-- What this change adds or changes, and the scenarios it covers (`@subject` tag, failure paths included). -->\n{proposal_tail}"),
    );
    write(
        repo,
        &format!("openspec/changes/{slug}/tasks.md"),
        &format!("# Tasks: {title}\n\n<!-- One checkbox row per task, numbered: `- [ ] 1 Write the failure-path scenarios`. Each row imports as task `<slug>#<n>`; flip it with `canon gate task` once evidence exists, never by hand. -->\n{tasks_tail}"),
    );
}

/// The first version `canon change new` 0.16 stored: imported by the
/// openspec adapter and adopted in the same write.
fn plant_change(repo: &Path, slug: &str, title: &str, subject: Option<&str>) {
    let mut record = serde_json::json!({
        "schema": 1,
        "kind": "change",
        "at": "2026-10-01T00:00:00.000000Z",
        "actor": {"agent_id": "canon-plan-import-openspec"},
        "change_id": slug,
        "title": slug,
        "summary": title,
        "status": "proposed",
    });
    if let Some(subject) = subject {
        record["subject_id"] = subject.into();
    }
    plant(repo, RecordKind::Change, record);
}

/// A git repo laid out as canon 0.16 left it: the cart scenario under
/// `specs/features/kind=feature/`, subject `cart` adopting `add-cart`, a
/// change `canon change new` wrote (one task row added since), a
/// `.gitignore` with only the hot-db line, and an install lock for
/// Claude and Codex. Everything committed.
fn old_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q"]);
    write(repo, "canon.yaml", OLD_YAML);
    write(repo, ".gitignore", ".canon/hot.db*\n");
    write(repo, "openspec/changes/.gitkeep", "");
    write(repo, OLD_FEATURE, CART);
    write(repo, ".canon/skills/.install-lock.json", r#"{"version":1,"source_hash":"x","providers":["codex","claude"],"files":{}}"#);
    ok(repo, &["subject", "new", "cart", "--domain", "shop", "--title", "Cart"]);
    scaffold(repo, "add-cart", "Add the cart", "", "\n- [ ] 1 Write the cart scenarios\n");
    plant_change(repo, "add-cart", "Add the cart", Some("cart"));
    ok(repo, &["subject", "adopt", "cart", "--change", "add-cart"]);
    commit_all(repo, "a 0.16 repo");
    dir
}

/// Every file under `dir`, repo-relative, sorted.
fn files(repo: &Path, dir: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![repo.join(dir)];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path.strip_prefix(repo).unwrap().display().to_string());
            }
        }
    }
    out.sort();
    out
}

fn query(repo: &Path, kind: &str) -> Vec<serde_json::Value> {
    let out = ok(repo, &["query", "--kind", kind, "--json"]);
    let start = out.find('{').unwrap();
    let payload: serde_json::Value = serde_json::Deserializer::from_str(&out[start..]).into_iter().next().unwrap().unwrap();
    payload["data"]["records"].as_array().unwrap().clone()
}

/// authoring.migrate.01: `--dry-run` prints the whole plan and writes
/// nothing; `canon migrate` then moves the corpus under `.canon/specs`
/// with `git mv`, moves the change `canon change new` wrote into
/// `.canon/changes/` (`proposal.md` → `brief.md`), re-imports it under the
/// same change id with its adoption and task, drops the init openspec
/// source, records `former_roots`, `skills.providers` and the missing
/// gitignore lines, and rebuilds the index; a second run has nothing to
/// move.
#[test]
fn authoring_migrate_01_a_0_16_repo_moves_onto_the_0_17_layout() {
    let dir = old_repo();
    let repo = dir.path();
    let before: Vec<String> = files(repo, "").into_iter().filter(|f| !f.starts_with(".git/")).collect();

    let dry = ok(repo, &["migrate"]);
    for token in [
        "specs/features/kind=feature -> .canon/specs/kind=feature",
        "openspec/changes/add-cart -> .canon/changes/add-cart",
        "openspec/changes/.gitkeep",
        "skills.providers: [claude, codex]",
        ".canon/audit/",
        "canon migrate --commit --plan-digest=sha256:",
    ] {
        assert!(dry.contains(token), "missing {token:?} in:\n{dry}");
    }
    // 0.39: the `routing` keys of the two retired record kinds the 0.16 scaffold wrote are deleted too, by line.
    assert!(dry.contains("(`routing.session`)") && dry.contains("(`routing.event`)"), "the plan names the retired keys it deletes:\n{dry}");
    let after_dry: Vec<String> = files(repo, "").into_iter().filter(|f| !f.starts_with(".git/")).collect();
    assert_eq!(after_dry, before, "--dry-run writes nothing");

    ok_commit(repo);

    assert_eq!(read(repo, NEW_FEATURE), CART, "the feature file moved byte for byte");
    assert!(!repo.join("specs").exists() && !repo.join("openspec").exists(), "the emptied directories are gone");
    let status = git(repo, &["status", "--porcelain"]);
    assert!(status.contains(&format!("R  {OLD_FEATURE} -> {NEW_FEATURE}")), "moved with git mv:\n{status}");
    assert!(status.contains("R  openspec/changes/add-cart/proposal.md -> .canon/changes/add-cart/brief.md"), "{status}");

    let yaml = read(repo, "canon.yaml");
    assert_eq!(
        yaml,
        OLD_YAML
            .replace("      root: specs\n", "      root: .canon/specs\n      former_roots: [specs]\n")
            .replace("  sources:\n    - dialect: openspec\n      root: .\n", "  sources: []\n")
            .replace("  session: hot\n", "")
            .replace("  event: hot\n", "")
            .replace("  trajectory: local\n", "  trajectory: local\n  change_state: local\n")
            + "\n# Harnesses the canon skill is installed for: any of claude, codex, omp, pi\n# (`[]` for none). `canon skills install|check|doctor` follow this list;\n# `canon skills install --harness <name>` (once per name) or `--no-harness` rewrites it.\nskills:\n  providers: [claude, codex]\n",
        "every other byte of canon.yaml is kept"
    );
    let gitignore = read(repo, ".gitignore");
    for line in [".canon/hot.db*", ".canon/audit/", ".canon/ledger/kind=scenario/", ".canon/archive/kind=scenario/", ".canon/scenario-index.lock", ".canon/artifact-store.lock", ".canon/repo-write.lock", ".canon/dashboard-snapshot/", ".canon/learn/", ".canon/r2/"] {
        assert_eq!(gitignore.lines().filter(|l| l.trim() == line).count(), 1, "{line} once in:\n{gitignore}");
    }

    let changes = query(repo, "change");
    let latest = changes.iter().filter(|c| c["change_id"] == "add-cart").max_by_key(|c| c["at"].as_str().unwrap().to_string()).unwrap();
    assert_eq!(latest["actor"]["agent_id"], "canon-plan-import-canon", "re-imported from canon's own source: {changes:?}");
    assert_eq!(latest["subject_ids"], serde_json::json!(["cart"]), "the adoption carried over from the 0.16 record's `subject_id`: {changes:?}");
    assert!(query(repo, "task").iter().any(|t| t["task_id"] == "add-cart#1"), "the task id is unchanged");
    let subjects = query(repo, "subject");
    assert!(subjects.iter().any(|s| s["change_ids"].as_array().unwrap().iter().any(|c| c == "add-cart")));

    let tree: Vec<String> = files(repo, "").into_iter().filter(|f| !f.starts_with(".git/") && !f.starts_with(".canon/ledger/kind=scenario/")).collect();
    let again = commit(repo);
    assert_eq!(again.status.code(), Some(0), "{}", text(&again));
    assert_eq!(read(repo, "canon.yaml"), yaml, "a second run changes nothing");
    let tree_again: Vec<String> = files(repo, "").into_iter().filter(|f| !f.starts_with(".git/") && !f.starts_with(".canon/ledger/kind=scenario/")).collect();
    assert_eq!(tree_again, tree, "a second run moves and writes no file");
    let gate = canon(repo, &["gate", "check"]);
    assert!(!text(&gate).contains("canon migrate"), "nothing is left on the old layout: {}", text(&gate));
}

/// authoring.migrate.02: a root other than the repo-level `specs` stays
/// where it is and only drops its `features/` level; canon.yaml (CRLF,
/// comments) is left byte-identical, and a repo outside git is moved
/// with a plain rename.
#[test]
fn authoring_migrate_02_another_root_only_drops_its_features_level() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    let yaml = "# a game repo\r\nspecs:\r\n  roots:\r\n    - id: game # the only root\r\n      root: game/specs\r\nplans:\r\n  sources: []\r\nskills:\r\n  providers: []\r\n";
    write(repo, "canon.yaml", yaml);
    write(repo, "game/specs/features/kind=feature/area=cart/add.feature", CART);
    write(repo, "game/specs/README.md", "the game's specs\n");

    let moved = "game/specs/features/kind=feature/area=cart/add.feature";
    let inode = |path: &str| std::os::unix::fs::MetadataExt::ino(&std::fs::metadata(repo.join(path)).unwrap());
    let before = inode(moved);
    ok_commit(repo);
    assert_eq!(read(repo, "game/specs/kind=feature/area=cart/add.feature"), CART);
    assert_eq!(inode("game/specs/kind=feature/area=cart/add.feature"), before, "a plain rename: the same file, not a copy");
    assert!(!repo.join("game/specs/features").exists(), "the emptied features/ directory is removed");
    assert_eq!(read(repo, "game/specs/README.md"), "the game's specs\n", "other files stay");
    assert_eq!(read(repo, "canon.yaml"), yaml, "the root did not move, so canon.yaml is unchanged");

    ok_commit(repo);
    assert_eq!(inode("game/specs/kind=feature/area=cart/add.feature"), before);
    assert_eq!(read(repo, "canon.yaml"), yaml);
}

/// authoring.migrate.03: uncommitted changes under a path it moves
/// refuse the whole migration, naming them; nothing is written.
#[test]
fn authoring_migrate_03_a_dirty_path_it_moves_is_refused() {
    let dir = old_repo();
    let repo = dir.path();
    write(repo, OLD_FEATURE, &format!("{CART}\n  # an edit in progress\n"));
    write(repo, "specs/features/kind=feature/area=cart/new.feature", CART);
    let yaml = read(repo, "canon.yaml");

    let out = commit(repo);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("canon: MIGRATION_CONFLICT"), "{err}");
    assert!(err.contains(&format!(" M {OLD_FEATURE}")) && err.contains("?? specs/features/kind=feature/area=cart/new.feature"), "{err}");
    assert!(repo.join(OLD_FEATURE).is_file() && !repo.join(".canon/specs").exists(), "nothing moved");
    assert_eq!(read(repo, "canon.yaml"), yaml);
    assert!(!repo.join(".canon/ledger/kind=scenario").exists(), "no index written");
}

/// authoring.migrate.04: a destination that already exists refuses the
/// whole migration, naming both paths.
#[test]
fn authoring_migrate_04_an_existing_destination_is_refused() {
    let dir = old_repo();
    let repo = dir.path();
    write(repo, ".canon/specs/kind=feature/area=wish/save.feature", CART);
    commit_all(repo, "a corpus already at the destination");

    let out = canon(repo, &["migrate"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(text(&out).contains("canon: MIGRATION_INVALID"), "{}", text(&out));
    assert!(repo.join(OLD_FEATURE).is_file() && repo.join("openspec/changes/add-cart/proposal.md").is_file(), "nothing moved");
}

/// authoring.migrate.05: an external change whose `proposal.md` holds the
/// `Approved:` line 0.16 recorded gets a canon pointer brief with its
/// `Source:` and that line; the external file is not edited, the leftover
/// line is named, the openspec source stays because a change is left
/// under it, and `canon status` reads the approval from the canon file.
#[test]
fn authoring_migrate_05_an_external_approval_moves_into_a_pointer_brief() {
    let dir = old_repo();
    let repo = dir.path();
    let proposal = "## Why\n\nShip checkout.\n\nApproved: ana 2026-10-10 — \"go\"\n";
    write(repo, "openspec/changes/checkout/proposal.md", proposal);
    write(repo, "openspec/changes/checkout/tasks.md", "- [ ] 1 Pay\n");
    ok(repo, &["ingest", "plans"]);
    commit_all(repo, "an external change");

    ok_commit(repo);
    assert_eq!(read(repo, ".canon/changes/checkout/brief.md"), "Source: openspec/changes/checkout/proposal.md\n\nApproved: ana 2026-10-10 — \"go\"\n");
    assert_eq!(read(repo, "openspec/changes/checkout/proposal.md"), proposal, "the external plan is not edited");
    assert!(read(repo, "canon.yaml").contains("    - dialect: openspec\n      root: .\n"), "the source stays");

    ok(repo, &["subject", "adopt", "cart", "--change", "checkout"]);
    let status = ok(repo, &["status"]);
    assert!(status.contains(".canon/changes/checkout/brief.md") && status.contains("ana 2026-10-10"), "{status}");
    let again = ok(repo, &["migrate"]);
    assert!(!again.contains(".canon/changes/checkout/brief.md"), "a second run writes no brief: {again}");
}

/// authoring.migrate.06: a change that carries only one of the two
/// signals `canon change new` leaves (its scaffold, its first stored
/// version adopted) is never guessed at: it stays where it is and the
/// plan says which signal is missing.
#[test]
fn authoring_migrate_06_an_ambiguous_change_stays_and_is_named() {
    let dir = old_repo();
    let repo = dir.path();
    scaffold(repo, "half-done", "Half done", "", "");
    plant_change(repo, "half-done", "Half done", None);
    write(repo, "openspec/changes/hand-written/proposal.md", "## Why\n\nBy hand.\n");
    plant_change(repo, "hand-written", "By hand.", Some("cart"));
    commit_all(repo, "two ambiguous changes");

    let out = ok(repo, &["migrate"]);
    assert!(out.lines().any(|l| l.starts_with("  left: openspec/changes/half-done")), "{out}");
    assert!(out.lines().any(|l| l.starts_with("  left: openspec/changes/hand-written")), "{out}");
    assert!(out.contains("  move openspec/changes/add-cart -> .canon/changes/add-cart"), "the canon-written change still moves: {out}");
}

/// authoring.migrate.07: a review written before review pinning, pinned to
/// a commit from before the move, is pinned as current by `canon migrate`
/// after `specs` moves to `.canon/specs`: the lookup reads `former_roots` at
/// the pin, so the review is checked, not undecidable. Without
/// `former_roots` (the same move made by hand, nothing recording the old
/// root) the same review could not be checked.
#[test]
fn authoring_migrate_07_legacy_reviews_pinned_before_the_move_are_pinned_current() {
    // The 0.17 P1 layout at the old default root, as canon's own corpus was, with a legacy review pinned to it.
    let legacy_repo = || {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        git(repo, &["init", "-q"]);
        write(repo, "canon.yaml", &OLD_YAML.replace("  sources:\n    - dialect: openspec\n      root: .\n", "  sources: []\n"));
        write(repo, ".canon/policy.yaml", "spec_coverage:\n  require_review:\n    scope: []\n");
        write(repo, "specs/kind=feature/area=cart/add.feature", CART);
        let pin = commit_all(repo, "cart add");
        ok(repo, &["evidence", "add", "--scenario", "cart.add.01", "--project", "root", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--actor", "impl-agent"]);
        plant(
            repo,
            RecordKind::Review,
            serde_json::json!({
                "schema": 1, "kind": "review", "at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
                "actor": {"agent_id": "reviewer-2", "role": "reviewer"}, "project_id": "root", "scenario_id": "cart.add.01",
                "reviewer": "reviewer-2", "pin": pin, "provenance_ref": {"upstream_ref": "docs/plan.md#review"},
            }),
        );
        commit_all(repo, "evidence and a legacy review");
        assert_eq!(canon(repo, &["gate", "check"]).status.code(), Some(1), "a review that predates 0.23 pinning does not count before the migration");
        dir
    };

    let dir = legacy_repo();
    let repo = dir.path();
    ok_commit(repo);
    assert!(read(repo, "canon.yaml").contains("      root: .canon/specs\n      former_roots: [specs]\n"));
    ok(repo, &["gate", "check"]);
}

/// authoring.migrate.08: a `canon.yaml` whose `specs.roots` it cannot
/// rewrite with every other byte kept (a flow-style list) is refused with
/// the edit to make by hand; nothing moves.
#[test]
fn authoring_migrate_08_a_canon_yaml_it_cannot_edit_safely_is_refused() {
    let dir = old_repo();
    let repo = dir.path();
    let yaml = OLD_YAML.replace("specs:\n  roots:\n    - id: root\n      root: specs\n", "specs: { roots: [{ id: root, root: specs }] }\n");
    write(repo, "canon.yaml", &yaml);
    commit_all(repo, "a flow-style specs section");

    let out = commit(repo);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(text(&out).contains("canon: CONFIG_INVALID"), "{}", text(&out));
    assert!(repo.join(OLD_FEATURE).is_file());
    assert_eq!(read(repo, "canon.yaml"), yaml);
}

/// `canon.yaml` once the `old_repo()` migration is complete.
fn migrated_yaml() -> String {
    OLD_YAML
        .replace("      root: specs\n", "      root: .canon/specs\n      former_roots: [specs]\n")
        .replace("  sources:\n    - dialect: openspec\n      root: .\n", "  sources: []\n")
        .replace("  session: hot\n", "")
        .replace("  event: hot\n", "")
        .replace("  trajectory: local\n", "  trajectory: local\n  change_state: local\n")
        + "\n# Harnesses the canon skill is installed for: any of claude, codex, omp, pi\n# (`[]` for none). `canon skills install|check|doctor` follow this list;\n# `canon skills install --harness <name>` (once per name) or `--no-harness` rewrites it.\nskills:\n  providers: [claude, codex]\n"
}

fn set_mode(path: &Path, mode: u32) {
    std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(mode)).unwrap();
}

/// A repo root that cannot take a new entry: `canon.yaml` is written through a temp file in it and renamed over it, so that write fails while the
/// moves into `.canon/` (and the in-place writes of files already there) still work. Restores the mode when dropped. `None`, saying so, for a user
/// who can write there anyway (root): the test that asked for it then does not run its failure half.
struct ReadOnlyRoot(std::path::PathBuf);

impl ReadOnlyRoot {
    fn new(repo: &Path) -> Option<Self> {
        set_mode(repo, 0o555);
        let probe = repo.join(".root-probe");
        if std::fs::write(&probe, b"").is_ok() {
            let _ = std::fs::remove_file(&probe);
            set_mode(repo, 0o755);
            eprintln!("skipped: running as a user who can write a read-only directory (root)");
            return None;
        }
        Some(Self(repo.to_path_buf()))
    }
}

impl Drop for ReadOnlyRoot {
    fn drop(&mut self) {
        set_mode(&self.0, 0o755);
    }
}

/// The latest stored version of change `id`.
fn latest_change(repo: &Path, id: &str) -> serde_json::Value {
    query(repo, "change").into_iter().filter(|c| c["change_id"] == id).max_by_key(|c| c["at"].as_str().unwrap().to_string()).unwrap()
}

/// authoring.migrate.09: canon.yaml is written whole or not at all. A run
/// whose canon.yaml write fails after the corpus and the change moved
/// leaves canon.yaml byte-identical and exits 1 (it stopped after it had begun); the rerun sees the root
/// already at `.canon/specs` while canon.yaml still names `specs`, writes
/// the `root:`/`former_roots` edit and the source removal, and re-imports
/// the change that was moved but not re-imported.
#[test]
fn authoring_migrate_09_a_run_stopped_before_its_canon_yaml_write_is_finished_by_the_next() {
    let dir = old_repo();
    let repo = dir.path();
    // The repo root takes no new entry, so canon.yaml cannot be replaced (skipped as root).
    let Some(blocked) = ReadOnlyRoot::new(repo) else { return };
    let out = commit(repo);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert!(text(&out).contains("canon: MIGRATION_INCOMPLETE"), "{}", text(&out));
    assert_eq!(read(repo, "canon.yaml"), OLD_YAML, "never half-written");
    assert!(repo.join(NEW_FEATURE).is_file() && repo.join(".canon/changes/add-cart/brief.md").is_file(), "the moves happened before the write failed");

    drop(blocked);
    ok_commit(repo);
    assert_eq!(read(repo, "canon.yaml"), migrated_yaml());
    let latest = latest_change(repo, "add-cart");
    assert_eq!(latest["actor"]["agent_id"], "canon-plan-import-canon", "{latest}");
    assert_eq!(latest["subject_ids"], serde_json::json!(["cart"]), "{latest}");
    ok_commit(repo);
    assert_eq!(read(repo, "canon.yaml"), migrated_yaml());
}

/// tier.durability.07 (F4 failure path, the canon.yaml half): canon.yaml is written through `canon_store::write_atomic`, so a repo root that takes
/// no new entry stops `migrate --commit` as `MIGRATION_INCOMPLETE` / `STOPPED` (exit 1) with canon.yaml as it was, and the next run finishes it.
/// The same wording of the promise as `authoring_migrate_09_…` and `atip_incomplete_10_…`, which keep their own scenarios. Skipped as root.
#[test]
fn tier_durability_07_a_canon_yaml_that_cannot_be_replaced_stops_the_migrate_incomplete_and_the_next_run_finishes() {
    let dir = old_repo();
    let repo = dir.path();
    let Some(blocked) = ReadOnlyRoot::new(repo) else { return };
    let (out, body) = commit_json(repo, &[]);
    assert_eq!(out.status.code(), Some(1), "{body}");
    assert_eq!((body["error"]["code"].as_str(), body["error"]["reason"].as_str()), (Some("MIGRATION_INCOMPLETE"), Some("STOPPED")), "{body}");
    assert_eq!(read(repo, "canon.yaml"), OLD_YAML, "canon.yaml is as it was, never half-written");
    drop(blocked);
    let finished = commit(repo);
    assert_eq!(finished.status.code(), Some(0), "running it again finishes the rest: {}", text(&finished));
    assert_eq!(read(repo, "canon.yaml"), migrated_yaml());
}

/// authoring.migrate.10: when a pointer brief cannot be written, the
/// openspec source that still holds the approval stays configured and the
/// run exits 1 (it stopped after it had begun); the rerun writes the brief and only then removes the
/// source canon init wrote.
#[test]
fn authoring_migrate_10_a_failed_pointer_brief_keeps_the_source_that_holds_the_approval() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q"]);
    let yaml = OLD_YAML.replace("      root: .\n", "      root: .\n    - dialect: openspec\n      root: ext\n");
    write(repo, "canon.yaml", &yaml);
    write(repo, ".gitignore", ".canon/hot.db*\n.canon/audit/\n.canon/ledger/kind=scenario/\n.canon/scenario-index.lock\n.canon/artifact-store.lock\n");
    write(repo, "openspec/changes/.gitkeep", "");
    write(repo, "ext/openspec/changes/checkout/proposal.md", "## Why\n\nShip checkout.\n\nApproved: ana 2026-10-10 — \"go\"\n");
    write(repo, "ext/openspec/changes/checkout/tasks.md", "- [ ] 1 Pay\n");
    std::fs::create_dir_all(repo.join(".canon/changes")).unwrap();
    commit_all(repo, "an external approval and the init source");
    set_mode(&repo.join(".canon/changes"), 0o555);

    let out = commit(repo);
    set_mode(&repo.join(".canon/changes"), 0o755);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    assert!(text(&out).contains("canon: MIGRATION_INCOMPLETE"), "{}", text(&out));
    // The retired kind keys come first in the migration, so they are already deleted when the run stops.
    assert_eq!(read(repo, "canon.yaml"), yaml.replace("  session: hot\n", "").replace("  event: hot\n", "").replace("  trajectory: local\n", "  trajectory: local\n  change_state: local\n"), "the source canon init wrote stays while the approval is not copied");
    assert!(repo.join("openspec/changes/.gitkeep").is_file());

    ok_commit(repo);
    assert_eq!(read(repo, ".canon/changes/checkout/brief.md"), "Source: ext/openspec/changes/checkout/proposal.md\n\nApproved: ana 2026-10-10 — \"go\"\n");
    assert!(read(repo, "canon.yaml").contains("  sources:\n    - dialect: openspec\n      root: ext\n"), "{}", read(repo, "canon.yaml"));
    assert!(!read(repo, "canon.yaml").contains("      root: .\n"), "now the init source goes");
}

/// authoring.migrate.11: a change move that could stop halfway is refused
/// before anything moves (here its directory is not writable); and a
/// change an earlier run left at `.canon/changes/<id>/proposal.md` gets
/// its rename to `brief.md` and its re-import on the next run.
#[test]
fn authoring_migrate_11_a_change_move_is_preflighted_and_a_half_moved_change_is_finished() {
    let dir = old_repo();
    let repo = dir.path();
    set_mode(&repo.join("openspec/changes/add-cart"), 0o555);
    let out = commit(repo);
    set_mode(&repo.join("openspec/changes/add-cart"), 0o755);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(text(&out).contains("canon: MIGRATION_INVALID"), "{}", text(&out));
    assert!(repo.join(OLD_FEATURE).is_file() && repo.join("openspec/changes/add-cart/proposal.md").is_file(), "nothing moved");
    assert_eq!(read(repo, "canon.yaml"), OLD_YAML);

    // An earlier run moved the change dir and stopped before the rename.
    std::fs::create_dir_all(repo.join(".canon/changes")).unwrap();
    git(repo, &["mv", "openspec/changes/add-cart", ".canon/changes/add-cart"]);
    ok_commit(repo);
    assert!(repo.join(".canon/changes/add-cart/brief.md").is_file() && !repo.join(".canon/changes/add-cart/proposal.md").exists());
    assert_eq!(latest_change(repo, "add-cart")["actor"]["agent_id"], "canon-plan-import-canon");
    assert_eq!(read(repo, "canon.yaml"), migrated_yaml());
    ok_commit(repo);
    assert_eq!(read(repo, "canon.yaml"), migrated_yaml());
}

/// authoring.migrate.12: change material the openspec reader cannot read
/// (a directory with no `proposal.md`) and any other file under
/// `openspec/changes/` keep the source canon init wrote; each is named,
/// with the reader's diagnostic, and the canon-written change still moves.
#[test]
fn authoring_migrate_12_unread_openspec_material_keeps_the_source() {
    let dir = old_repo();
    let repo = dir.path();
    write(repo, "openspec/changes/broken/tasks.md", "- [ ] 1 Orphan task\n");
    write(repo, "openspec/changes/notes.txt", "loose notes\n");
    commit_all(repo, "unread material");

    let out = ok(repo, &["migrate"]);
    assert!(out.contains("openspec/changes/broken") && out.contains("missing-proposal-md"), "{out}");
    assert!(out.contains("  move openspec/changes/add-cart -> .canon/changes/add-cart"), "{out}");

    std::fs::remove_dir_all(repo.join("openspec/changes/broken")).unwrap();
    commit_all(repo, "only the notes are left");
    let out = ok_commit(repo);
    assert!(out.contains("openspec/changes/notes.txt"), "{out}");
    assert!(read(repo, "canon.yaml").contains("    - dialect: openspec\n      root: .\n"), "the source stays");
    assert_eq!(read(repo, "openspec/changes/notes.txt"), "loose notes\n");
    assert!(repo.join(".canon/changes/add-cart/brief.md").is_file());
}

/// authoring.migrate.13 (#105, #107): a repo whose `.gitignore` holds the
/// block canon 0.25 wrote gets `.canon/dashboard-snapshot/`, `.canon/learn/`
/// and `.canon/r2/` from `canon migrate`, each under its comment once, every
/// other byte kept; a second run adds none.
#[test]
fn authoring_migrate_13_adds_the_derived_directory_lines_to_an_existing_block_once() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q"]);
    ok(repo, &["init", "--repo", ".", "--no-policy"]);
    let old = "target/\n# canon's read-access audit log\n.canon/hot.db*\n.canon/audit/\n.canon/ledger/kind=scenario/\n.canon/scenario-index.lock\n.canon/artifact-store.lock\nnode_modules/\n";
    write(repo, ".gitignore", old);
    commit_all(repo, "a 0.25 gitignore");

    ok(repo, &["migrate"]);
    assert_eq!(read(repo, ".gitignore"), old, "--dry-run writes nothing");

    ok_commit(repo);
    let gitignore = read(repo, ".gitignore");
    assert!(gitignore.starts_with(old), "every existing byte is kept: {gitignore}");
    for line in [".canon/archive/kind=scenario/", ".canon/dashboard-snapshot/", ".canon/learn/", ".canon/r2/"] {
        assert_eq!(gitignore.lines().filter(|l| l.trim() == line).count(), 1, "{line} once in:\n{gitignore}");
    }

    ok_commit(repo);
    assert_eq!(read(repo, ".gitignore"), gitignore, "a second run adds none");
}

/// authoring.migrate.14 (review finding 12): a `.gitignore` that kept a
/// managed comment but lost its line gets only the line, directly under the
/// comment; no comment is written twice, and a second run changes nothing.
#[test]
fn authoring_migrate_14_a_kept_comment_gets_its_missing_line_and_no_second_comment() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q"]);
    ok(repo, &["init", "--repo", ".", "--no-policy"]);
    let full = read(repo, ".gitignore");
    let lost = full.replace(".canon/learn/\n", "").replace(".canon/archive/kind=scenario/\n", "");
    assert_ne!(lost, full);
    write(repo, ".gitignore", &lost);
    commit_all(repo, "a gitignore that lost two lines and kept their comments");

    ok_commit(repo);
    assert_eq!(read(repo, ".gitignore"), full, "the lines return under their own comments, byte for byte");
    for comment in full.lines().filter(|line| line.starts_with('#')) {
        assert_eq!(read(repo, ".gitignore").lines().filter(|l| *l == comment).count(), 1, "{comment} once");
    }

    ok_commit(repo);
    assert_eq!(read(repo, ".gitignore"), full, "a second run changes nothing");
}

/// D6: the bare run previews (a digest, the effects, nothing written); the
/// commit needs the digest of the plan recomputed now, and a stale one writes
/// nothing; the 0.27 spelling is a usage error.
#[test]
fn migrate_previews_by_default_and_commits_only_the_previewed_plan() {
    let dir = old_repo();
    let repo = dir.path();
    let tree = || -> Vec<String> { files(repo, "").into_iter().filter(|f| !f.starts_with(".git/")).collect() };
    let before = tree();

    let preview = canon(repo, &["migrate", "--json"]);
    assert_eq!(preview.status.code(), Some(0), "{}", text(&preview));
    let body: serde_json::Value = serde_json::from_slice(&preview.stdout).unwrap();
    assert_eq!(body["data"]["committed"], false, "{body}");
    assert!(body["data"]["effects"].as_array().unwrap().iter().any(|e| e["action"] == "move"), "{body}");
    let digest = body["data"]["planDigest"].as_str().expect("a preview carries a planDigest").to_string();
    assert_eq!(body["meta"]["next"][0]["argv"], serde_json::json!(["migrate", "--commit", format!("--plan-digest={digest}"), "--json"]), "{body}");
    assert_eq!(tree(), before, "a preview writes nothing");

    let stale = canon(repo, &["migrate", "--commit", "--plan-digest", &"0".repeat(64), "--json"]);
    assert_eq!(stale.status.code(), Some(2), "{}", text(&stale));
    let stale: serde_json::Value = serde_json::from_slice(&stale.stdout).unwrap();
    assert_eq!(stale["error"]["code"], "PLAN_STALE", "{stale}");
    assert_eq!(tree(), before, "a stale digest writes nothing");

    let committed = canon(repo, &["migrate", "--commit", "--plan-digest", &digest, "--json"]);
    assert_eq!(committed.status.code(), Some(0), "{}", text(&committed));
    let committed: serde_json::Value = serde_json::from_slice(&committed.stdout).unwrap();
    assert_eq!(committed["data"]["committed"], true, "{committed}");
    assert!(repo.join(NEW_FEATURE).is_file() && !repo.join(OLD_FEATURE).exists(), "the plan moved the corpus");

    for old in [&["--dry-run"][..], &["--commit"], &["--plan-digest", "abc"]] {
        let mut args = vec!["migrate"];
        args.extend_from_slice(old);
        assert_eq!(canon(repo, &args).status.code(), Some(2), "{old:?}");
    }
}

/// `canon migrate --commit --json` as `commit` applies it (the preview first, its digest second), with the extra environment `env`.
fn commit_json(repo: &Path, env: &[(&str, &str)]) -> (Output, serde_json::Value) {
    let preview = canon(repo, &["migrate"]);
    let stdout = String::from_utf8_lossy(&preview.stdout).into_owned();
    let digest = stdout
        .lines()
        .find_map(|line| line.strip_prefix("planDigest "))
        .and_then(|rest| rest.split(';').next())
        .unwrap_or_else(|| panic!("a plan digest: {}", text(&preview)))
        .to_string();
    let mut command = hermetic::canon();
    command
        .args(["migrate", "--commit", "--plan-digest", &digest, "--json"])
        .env("CANON_ACTOR", "canon")
        .current_dir(repo);
    for (key, value) in env {
        command.env(key, value);
    }
    let out = command.output().expect("spawn canon");
    let body = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("one JSON document ({e}): {}", text(&out)));
    (out, body)
}

/// atip.incomplete.10: `migrate` stopped after it began changing the repo is `MIGRATION_INCOMPLETE` (exit 1, not retryable) and its reason says where:
/// `STOPPED` while it moves (a temp file it cannot create), `PINS` while it pins the reviews (the debug seam `migrate-pins`, after the index was
/// rebuilt) and `INDEX` while it rebuilds the scenario index (a corpus the rebuild finds violations in, with the report in `data.index`). What it did
/// stays done, `meta.next` leads with `canon status`, and running it again finishes the rest.
#[test]
fn atip_incomplete_10_a_migrate_that_stopped_after_it_began_is_incomplete_with_the_reason_where_it_stopped(
) {
    // STOPPED: canon.yaml cannot be replaced (the repo root takes no new entry; skipped as root), after the corpus and the change moved (authoring.migrate.09).
    let dir = old_repo();
    let repo = dir.path();
    if let Some(blocked) = ReadOnlyRoot::new(repo) {
        let (out, body) = commit_json(repo, &[]);
        assert_eq!(out.status.code(), Some(1), "{body}");
        assert_eq!(
            (
                body["error"]["code"].as_str(),
                body["error"]["reason"].as_str(),
                body["error"]["retryable"].as_bool()
            ),
            (Some("MIGRATION_INCOMPLETE"), Some("STOPPED"), Some(false)),
            "{body}"
        );
        assert_eq!(
            body["meta"]["next"][0]["argv"],
            serde_json::json!(["status", "--json"]),
            "{body}"
        );
        assert!(repo.join(".canon/specs").is_dir(), "what moved stays moved");
        drop(blocked);
        let finished = commit(repo);
        assert_eq!(
            finished.status.code(),
            Some(0),
            "running it again finishes the rest: {}",
            text(&finished)
        );
    }

    // PINS: the reviews could not be pinned after the index was rebuilt (the seam).
    let other = old_repo();
    let repo = other.path();
    let (out, body) = commit_json(repo, &[("CANON_TEST_STOP_AT", "migrate-pins")]);
    assert_eq!(out.status.code(), Some(1), "{body}");
    assert_eq!(
        (
            body["error"]["code"].as_str(),
            body["error"]["reason"].as_str()
        ),
        (Some("MIGRATION_INCOMPLETE"), Some("PINS")),
        "{body}"
    );
    assert!(
        repo.join(".canon/specs").is_dir(),
        "the files moved and stay moved"
    );
    let finished = commit(repo);
    assert_eq!(
        finished.status.code(),
        Some(0),
        "running it again finishes the reviews: {}",
        text(&finished)
    );

    // INDEX: a corpus the index rebuild finds violations in, after the files moved.
    let third = old_repo();
    let repo = third.path();
    write(
        repo,
        "specs/features/kind=feature/area=cart/broken.feature",
        "Feature: cart broken\n\n  @cart.broken.01\n  Scenario: no provenance\n    Given a step\n",
    );
    commit_all(repo, "a corpus with a violation");
    let (out, body) = commit_json(repo, &[]);
    assert_eq!(out.status.code(), Some(1), "{body}");
    assert_eq!(
        (
            body["error"]["code"].as_str(),
            body["error"]["reason"].as_str()
        ),
        (Some("MIGRATION_INCOMPLETE"), Some("INDEX")),
        "{body}"
    );
    assert!(
        body["data"]["index"].is_object(),
        "the rebuild's report is in data.index: {body}"
    );
    assert!(
        repo.join(".canon/specs").is_dir(),
        "the files moved and stay moved"
    );
}

// ───────────── 0.42 `change-drop` F9: the `change_state` routing step ─────────────

const ROUTE_LINE: &str = "  change_state: local\n";

fn json_of(out: &Output) -> serde_json::Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("one JSON document ({e}): {}", text(out)))
}

/// `canon` as an agent that declared `role` (no human answer).
fn canon_role(repo: &Path, role: &str, args: &[&str]) -> Output {
    hermetic::canon().args(args).env("CANON_ACTOR", "agent-1").env("CANON_ROLE", role).current_dir(repo).output().expect("spawn canon")
}

/// The plan digest of `canon migrate --json` in `repo`.
fn plan_digest(repo: &Path) -> String {
    let preview = canon(repo, &["migrate", "--json"]);
    assert!(preview.status.success(), "{}", text(&preview));
    json_of(&preview)["data"]["planDigest"].as_str().expect("a planDigest").to_string()
}

/// How many effects of `canon migrate --json` in `repo` touch `canon.yaml`.
fn yaml_effects(repo: &Path) -> usize {
    let doc = json_of(&canon(repo, &["migrate", "--json"]));
    doc["data"]["effects"].as_array().unwrap().iter().filter(|e| e["path"] == "canon.yaml").count()
}

/// A git repo made by `canon init` whose `canon.yaml` is as canon 0.41 wrote it (no `change_state` routing line), with subject `demo` and
/// change `add-login`, committed; and that `canon.yaml`.
fn without_route_repo() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q", "-b", "main"]);
    ok(repo, &["init", "--repo", ".", "--no-harness", "--no-agents-md", "--no-policy"]);
    let fresh = read(repo, "canon.yaml");
    assert!(fresh.contains(ROUTE_LINE), "0.42 `canon init` routes change_state: {fresh}");
    let old = fresh.replacen(ROUTE_LINE, "", 1);
    write(repo, "canon.yaml", &old);
    commit_all(repo, "a canon.yaml of canon 0.41");
    ok(repo, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(repo, &["change", "new", "add-login", "--subject", "demo", "--title", "Add login"]);
    commit_all(repo, "a subject and a change");
    (dir, old)
}

/// The 1-based line of the last entry of `routing:` in `yaml`.
fn last_route_line(yaml: &str) -> usize {
    yaml.lines().enumerate().skip_while(|(_, line)| *line != "routing:").skip(1).take_while(|(_, line)| line.starts_with("  ")).last().expect("routing entries").0 + 1
}

/// `yaml` with the route line after the last entry of `routing:`: the old document and that one key.
fn with_route_line(yaml: &str) -> String {
    let at = last_route_line(yaml);
    let mut lines: Vec<&str> = yaml.split_inclusive('\n').collect();
    lines.insert(at, ROUTE_LINE);
    lines.concat()
}

/// Every file of the repo but `.git`, the backups and the write lock, with its bytes.
fn snapshot(repo: &Path) -> Vec<(String, Vec<u8>)> {
    files(repo, "")
        .into_iter()
        .filter(|f| !f.starts_with(".git/") && !f.starts_with(".canon/migrate-backup/") && !f.ends_with("repo-write.lock"))
        .map(|f| {
            let bytes = std::fs::read(repo.join(&f)).unwrap();
            (f, bytes)
        })
        .collect()
}

fn yaml_value(text: &str) -> serde_yaml::Value {
    serde_yaml::from_str(text).unwrap()
}

/// change.drop.26 (F9): without the `change_state` routing line the closing command is refused (`CONFIG_INVALID`, reason `TIERS`, the hint names
/// `canon migrate`); the preview says the line and where it goes; the commit adds that line and no other, the result is the old document plus the
/// key, `change defer` then runs, and the next plan is empty.
#[test]
fn change_drop_26_migrate_adds_the_change_state_route_line_and_the_next_plan_is_empty() {
    let (dir, old) = without_route_repo();
    let repo = dir.path();
    let (line, want) = (last_route_line(&old) + 1, with_route_line(&old));

    let refused = canon(repo, &["change", "defer", "add-login", "--reason", "not this release", "--ref", "chat 2026-11-16", "--approver", "Ana", "--json"]);
    let refused_doc = json_of(&refused);
    assert_eq!((refused.status.code(), refused_doc["error"]["code"].as_str(), refused_doc["error"]["reason"].as_str()), (Some(2), Some("CONFIG_INVALID"), Some("TIERS")), "{refused_doc}");
    assert!(refused_doc["error"]["hint"].as_str().unwrap().contains("canon migrate"), "the hint names the step: {refused_doc}");

    assert_eq!(read(repo, "canon.yaml"), old, "a preview writes nothing");
    let doc = json_of(&canon(repo, &["migrate", "--json"]));
    let effects = doc["data"]["effects"].as_array().unwrap();
    assert!(effects.iter().any(|e| e["action"] == "rewrite" && e["path"] == "canon.yaml"), "the rewrite effect: {doc}");
    assert!(!effects.iter().any(|e| e["path"] == ".gitignore"), "only the route line is planned: {doc}");

    ok_commit(repo);
    let after = read(repo, "canon.yaml");
    assert_eq!(after, want, "exactly one line was added, after the last entry, in its indentation");
    assert_eq!(after.lines().nth(line - 1), Some("  change_state: local"));
    let mut expected = yaml_value(&old);
    expected["routing"]["change_state"] = "local".into();
    assert_eq!(yaml_value(&after), expected, "the result is the old document plus the key");

    assert_eq!(yaml_effects(repo), 0, "the next plan is empty");
    commit_all(repo, "the route line");
    let deferred = canon(repo, &["change", "defer", "add-login", "--reason", "not this release", "--ref", "chat 2026-11-16", "--approver", "Ana", "--json"]);
    assert!(deferred.status.success(), "`change defer` runs once the line is there: {}", text(&deferred));
    assert_eq!(yaml_effects(repo), 0, "still empty with the record written");
    assert_eq!(read(repo, "canon.yaml"), want);
}

/// change.drop.26: `canon migrate --revert` restores the `canon.yaml` bytes the step rewrote.
#[test]
fn change_drop_26_revert_restores_the_canon_yaml_bytes() {
    let (dir, old) = without_route_repo();
    let repo = dir.path();
    let before = snapshot(repo);
    ok_commit(repo);
    assert_eq!(read(repo, "canon.yaml"), with_route_line(&old));

    let preview = canon(repo, &["migrate", "--revert", "--json"]);
    assert!(preview.status.success(), "{}", text(&preview));
    let digest = json_of(&preview)["data"]["planDigest"].as_str().expect("a planDigest").to_string();
    assert_eq!(read(repo, "canon.yaml"), with_route_line(&old), "the revert preview writes nothing");
    let reverted = canon(repo, &["migrate", "--revert", "--commit", "--plan-digest", &digest, "--json"]);
    assert!(reverted.status.success(), "{}", text(&reverted));
    assert_eq!(read(repo, "canon.yaml"), old, "the file is back, byte for byte");
    assert_eq!(snapshot(repo), before, "and so is everything else");
}

/// change.drop.26: a repo that has the line already, whatever its value, has an empty plan and keeps the value.
#[test]
fn change_drop_26_an_existing_line_is_skipped_and_its_value_kept() {
    let (dir, old) = without_route_repo();
    let repo = dir.path();
    // The line in the middle of the section, with a value that is not `local` (`hot` is the rung every init repo has besides `local`).
    let own = old.replacen("  finding: local\n", "  change_state: hot\n  finding: local\n", 1);
    assert_ne!(own, old);
    write(repo, "canon.yaml", &own);
    commit_all(repo, "change_state routed to hot");
    assert_eq!(yaml_effects(repo), 0, "an existing line is not planned again");
    ok_commit(repo);
    assert_eq!(read(repo, "canon.yaml"), own, "the value is not changed and no second line is added");
    assert_eq!(read(repo, "canon.yaml").matches("change_state").count(), 1);
}

/// change.drop.27 (failure path): a `routing:` that is a flow mapping or has an anchor cannot take a line: `CONFIG_INVALID` (reason `CANON_YAML`), the
/// message names the line to add by hand, the hint says to rerun `canon migrate`, and nothing is written, not even by the run's other steps.
#[test]
fn change_drop_27_a_routing_section_that_cannot_take_a_line_is_refused_and_blocks_the_other_steps() {
    let (dir, old) = without_route_repo();
    let repo = dir.path();
    let lines: Vec<&str> = old.split_inclusive('\n').collect();
    let header = lines.iter().position(|l| *l == "routing:\n").unwrap();
    let last = last_route_line(&old);
    let entries: Vec<String> = lines[header + 1..last].iter().map(|l| l.trim().to_string()).collect();
    let flow = format!("{}routing: {{ {} }}\n{}", lines[..header].concat(), entries.join(", "), lines[last..].concat());
    let anchored = old.replacen("routing:\n", "routing: &route\n", 1);
    // Other steps would act on this repo: its .gitignore lacks canon's lines.
    write(repo, ".gitignore", "target/\n");
    for (form, yaml) in [("flow mapping", flow), ("anchor", anchored)] {
        write(repo, "canon.yaml", &yaml);
        commit_all(repo, form);
        let before = snapshot(repo);
        let preview = canon(repo, &["migrate", "--json"]);
        let doc = json_of(&preview);
        assert_eq!((preview.status.code(), doc["error"]["code"].as_str(), doc["error"]["reason"].as_str()), (Some(2), Some("CONFIG_INVALID"), Some("CANON_YAML")), "{form}: {doc}");
        assert!(doc["error"]["message"].as_str().unwrap().contains("`change_state: local`"), "{form}: the message names the line to add: {doc}");
        assert!(doc["error"]["hint"].as_str().unwrap().contains("canon migrate"), "{form}: the hint says to rerun: {doc}");
        let commit = canon(repo, &["migrate", "--commit", "--plan-digest", &format!("sha256:{}", "0".repeat(64)), "--json"]);
        let commit_doc = json_of(&commit);
        assert_eq!((commit.status.code(), commit_doc["error"]["code"].as_str(), commit_doc["error"]["reason"].as_str()), (Some(2), Some("CONFIG_INVALID"), Some("CANON_YAML")), "{form}: {commit_doc}");
        assert_eq!(snapshot(repo), before, "{form}: canon.yaml, .gitignore and everything else are untouched");
    }
    // Added by hand, the rerun proceeds (the other steps run too).
    write(repo, "canon.yaml", &old);
    ok_commit(repo);
    assert_eq!(read(repo, "canon.yaml"), with_route_line(&old));
    assert!(read(repo, ".gitignore").contains(".canon/audit/"), "the other steps ran once the refusal was gone");
}

/// change.drop.27: a `canon.yaml` without a `routing:` section is skipped, byte for byte (see also authoring.migrate.02), and the next migrate has
/// nothing to do.
#[test]
fn change_drop_27_a_canon_yaml_without_routing_is_left_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    let yaml = "# a game repo\r\nspecs:\r\n  roots:\r\n    - id: game # the only root\r\n      root: game/specs\r\nplans:\r\n  sources: []\r\nskills:\r\n  providers: []\r\n";
    write(repo, "canon.yaml", yaml);
    write(repo, "game/specs/kind=feature/area=cart/add.feature", CART);
    let dry = ok(repo, &["migrate"]);
    assert!(!dry.contains("change_state"), "no routing section, nothing to add: {dry}");
    ok_commit(repo);
    assert_eq!(read(repo, "canon.yaml"), yaml);
    ok_commit(repo);
    assert_eq!(read(repo, "canon.yaml"), yaml);

    // The closing command on such a file says to add the section by hand: `canon migrate` adds nothing to it, so its preview is no way out.
    let refused = canon(repo, &["change", "defer", "add-login", "--reason", "not this release", "--ref", "chat 2026-11-16", "--approver", "Ana", "--json"]);
    let doc = json_of(&refused);
    assert_eq!((refused.status.code(), doc["error"]["code"].as_str(), doc["error"]["reason"].as_str()), (Some(2), Some("CONFIG_INVALID"), Some("TIERS")), "{doc}");
    let hint = doc["error"]["hint"].as_str().unwrap();
    assert!(hint.contains("`routing:`") && hint.contains("`change_state: local`"), "{hint}");
}

/// change.drop.27 (failure path): a `canon.yaml` changed after the preview makes the old digest `PLAN_STALE`, and the file stays as it was.
#[test]
fn change_drop_27_a_stale_digest_is_plan_stale_and_the_file_is_untouched() {
    let (dir, old) = without_route_repo();
    let repo = dir.path();
    let digest = plan_digest(repo);
    let edited = format!("{old}# edited after the preview\n");
    write(repo, "canon.yaml", &edited);
    commit_all(repo, "canon.yaml edited");
    let before = snapshot(repo);
    let out = canon(repo, &["migrate", "--commit", "--plan-digest", &digest, "--json"]);
    let doc = json_of(&out);
    assert_eq!((out.status.code(), doc["error"]["code"].as_str()), (Some(2), Some("PLAN_STALE")), "{doc}");
    assert_eq!(read(repo, "canon.yaml"), edited);
    assert_eq!(snapshot(repo), before, "nothing was written");
    // The plan of the file as it is now applies.
    ok_commit(repo);
    assert_eq!(read(repo, "canon.yaml"), with_route_line(&old) + "# edited after the preview\n");
}

/// change.drop.28 (failure path, no new human-only rule): an agent that declared a role runs the migrate confirm of this step without a human's
/// answer; the closing command stays human-only for it (`ACTOR_DENIED`, nothing written) and passes with the human's answer relayed.
#[test]
fn change_drop_28_an_agent_role_runs_the_route_step_without_a_human_answer() {
    for role in ["planner", "orchestrator"] {
        let (dir, old) = without_route_repo();
        let repo = dir.path();
        let digest = plan_digest(repo);
        let out = canon_role(repo, role, &["migrate", "--commit", "--plan-digest", &digest, "--json"]);
        assert_eq!(out.status.code(), Some(0), "{role}: {}", text(&out));
        assert_eq!(read(repo, "canon.yaml"), with_route_line(&old), "{role}");
        assert!(!json_of(&out).to_string().contains("ACTOR_DENIED"), "{role}");
        commit_all(repo, "the route line");

        // The migrate confirm opened nothing else: closing a change is still the human's.
        let args = ["change", "defer", "add-login", "--reason", "not this release", "--ref", "chat 2026-11-16", "--approver", "Ana"];
        let denied = canon_role(repo, role, &[&args[..], &["--json"][..]].concat());
        let denied_doc = json_of(&denied);
        assert_eq!((denied.status.code(), denied_doc["error"]["code"].as_str()), (Some(2), Some("ACTOR_DENIED")), "{role}: {denied_doc}");
        assert!(!repo.join(".canon/ledger/kind=change_state").exists(), "{role}: nothing was written");
        let relayed = canon_role(repo, role, &[&args[..], &["--relayed-via", "ask", "--actor", "agent-1", "--json"][..]].concat());
        assert!(relayed.status.success(), "{role}: the human's answer relayed passes: {}", text(&relayed));
    }
}

/// change.drop.28 (failure path): a plan whose other step re-binds an `Approved:` line stays the human's to commit, whether or not it adds the route
/// line too; the agent is refused (`ACTOR_DENIED`) and `canon.yaml` is untouched; the human's own commit adds the line.
#[test]
fn change_drop_28_a_plan_that_also_re_binds_an_approval_is_still_human_only() {
    let (dir, old) = without_route_repo();
    let repo = dir.path();
    ok(repo, &["change", "new", "c", "--subject", "demo", "--title", "C"]);
    let body = "# C\n\n<!-- canon:kind chore -->\n\n<!-- canon:section intent role=context -->\n## Intent\n\nC\n\nThe client has a security fix.\n\n<!-- canon:section category role=context -->\n## Category\n\nci\n\n<!-- canon:section invariants role=normative -->\n## Invariants\n\n- Requests and responses stay byte-identical. ==**No behavior changes.**==\n\n<!-- canon:section checks role=normative -->\n## Checks\n\n- The suite runs again at the chore commit. ==**The suite is the check.**==\n\n<!-- canon:section out-of-scope role=context -->\n## Out of scope\n\nNone.\n";
    write(repo, ".canon/changes/c/brief.md", &format!("{body}Approved: Ana Kim 2026-10-12T09:00:00Z — \"ship it\" (brief {})\n", canon_cli::brief::brief_digest(body)));
    commit_all(repo, "an approved chore in the 0.30 format");
    let digest = plan_digest(repo);
    let before = snapshot(repo);
    let out = canon_role(repo, "planner", &["migrate", "--commit", "--plan-digest", &digest, "--json"]);
    let doc = json_of(&out);
    assert_eq!((out.status.code(), doc["error"]["code"].as_str()), (Some(2), Some("ACTOR_DENIED")), "{doc}");
    assert_eq!(read(repo, "canon.yaml"), old, "the route line was not added either");
    assert_eq!(snapshot(repo), before, "nothing was written");

    let own = canon(repo, &["migrate", "--commit", "--plan-digest", &digest, "--json"]);
    assert!(own.status.success(), "the human's own commit runs: {}", text(&own));
    assert_eq!(read(repo, "canon.yaml"), with_route_line(&old));
}
