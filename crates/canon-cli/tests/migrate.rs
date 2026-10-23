//! `canon migrate` (0.17 D6) through the real binary, on repos laid out
//! the way canon 0.16 left them: the spec corpus under
//! `specs/features/kind=feature/`, `canon change new`'s changes under
//! `openspec/changes/`, approvals in `proposal.md`, no `skills:` key and a
//! `.gitignore` missing canon's newer lines.

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
  evidence_record: local
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
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).env("CANON_ACTOR", "canon").current_dir(repo).output().expect("spawn canon")
}

fn text(out: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

fn ok(repo: &Path, args: &[&str]) -> String {
    let out = canon(repo, args);
    assert!(out.status.success(), "canon {args:?}: {}", text(&out));
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
    ok(repo, &["subject", "adopt", "add-cart", "--subject-id", "cart"]);
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

    let dry = ok(repo, &["migrate", "--dry-run"]);
    for line in [
        "canon migrate: plan:\n",
        "  move specs/features/kind=feature -> .canon/specs/kind=feature (spec root `root`; canon.yaml root: .canon/specs, former_roots: [specs])\n",
        "  move openspec/changes/add-cart -> .canon/changes/add-cart (proposal.md -> brief.md; written by `canon change new`)\n",
        "  canon.yaml: remove the plans source `{ dialect: openspec, root: . }` canon init wrote; no change is left under it\n",
        "  remove openspec/changes/.gitkeep (canon init's placeholder for that source)\n",
        "  canon.yaml: skills.providers: [claude, codex] (from the install lock)\n",
        "  .gitignore: add .canon/audit/, .canon/ledger/kind=scenario/, .canon/archive/kind=scenario/, .canon/scenario-index.lock, .canon/artifact-store.lock, .canon/dashboard-snapshot/, .canon/learn/, .canon/r2/\n",
        "  rebuild the scenario index (full sync)\n",
        "canon migrate: --dry-run, nothing written\n",
    ] {
        assert!(dry.contains(line), "missing {line:?} in:\n{dry}");
    }
    let after_dry: Vec<String> = files(repo, "").into_iter().filter(|f| !f.starts_with(".git/")).collect();
    assert_eq!(after_dry, before, "--dry-run writes nothing");

    let out = ok(repo, &["migrate"]);
    assert!(out.contains("canon migrate: moved specs/features/kind=feature -> .canon/specs/kind=feature\n"), "{out}");
    assert!(out.contains("canon migrate: moved openspec/changes/add-cart -> .canon/changes/add-cart (proposal.md -> brief.md)\n"), "{out}");
    assert!(out.contains("canon migrate: re-imported change `add-cart` from .canon/changes/add-cart"), "{out}");
    assert!(out.contains("canon migrate: rebuilt the scenario index: 1 record(s) written\n"), "{out}");
    assert!(out.contains("canon migrate: removed openspec/changes/.gitkeep\n"), "{out}");

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
            + "\n# Harnesses the canon skill is installed for: any of claude, codex, omp, pi\n# (`[]` for none). `canon skills install|check|doctor` follow this list;\n# `canon skills install --providers=<list>` rewrites it.\nskills:\n  providers: [claude, codex]\n",
        "every other byte of canon.yaml is kept"
    );
    let gitignore = read(repo, ".gitignore");
    for line in [".canon/hot.db*", ".canon/audit/", ".canon/ledger/kind=scenario/", ".canon/archive/kind=scenario/", ".canon/scenario-index.lock", ".canon/artifact-store.lock", ".canon/dashboard-snapshot/", ".canon/learn/", ".canon/r2/"] {
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
    let again = canon(repo, &["migrate"]);
    assert_eq!(again.status.code(), Some(0), "{}", text(&again));
    let again = text(&again);
    assert!(again.starts_with("canon migrate: nothing to migrate; the repo is on the 0.17 layout\n"), "{again}");
    assert!(again.contains("canon migrate: rebuilt the scenario index: 0 record(s) written\n"), "{again}");
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
    let out = ok(repo, &["migrate"]);
    assert!(out.contains("  move game/specs/features/kind=feature -> game/specs/kind=feature (spec root `game`)\n"), "{out}");
    assert_eq!(read(repo, "game/specs/kind=feature/area=cart/add.feature"), CART);
    assert_eq!(inode("game/specs/kind=feature/area=cart/add.feature"), before, "a plain rename: the same file, not a copy");
    assert!(!repo.join("game/specs/features").exists(), "the emptied features/ directory is removed");
    assert_eq!(read(repo, "game/specs/README.md"), "the game's specs\n", "other files stay");
    assert_eq!(read(repo, "canon.yaml"), yaml, "the root did not move, so canon.yaml is unchanged");
    assert!(out.contains("canon migrate: rebuilt the scenario index: 1 record(s) written\n"), "{out}");

    let again = ok(repo, &["migrate"]);
    assert!(again.starts_with("canon migrate: nothing to migrate; the repo is on the 0.17 layout\n"), "a root already on the new layout is a no-op: {again}");
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

    let out = canon(repo, &["migrate"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("canon migrate: MIGRATE_UNCOMMITTED — uncommitted changes under the paths it moves; commit or stash them, then run it again:"), "{err}");
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

    let out = canon(repo, &["migrate", "--dry-run"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(
        text(&out).contains("canon migrate: MIGRATE_REFUSED — spec root `root`: specs/features/kind=feature would move to .canon/specs/kind=feature, which already exists; move what is there out of the way first"),
        "{}",
        text(&out)
    );
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

    let out = ok(repo, &["migrate"]);
    assert!(
        out.contains("  write .canon/changes/checkout/brief.md (Source: openspec/changes/checkout/proposal.md; Approved: ana 2026-10-10 — \"go\"); the Approved: line stays in openspec/changes/checkout/proposal.md, which canon does not edit\n"),
        "{out}"
    );
    assert!(out.contains("  left: 1 openspec change(s) not written by `canon change new` stay where they are, read as an external plan: checkout\n"), "{out}");
    assert!(!out.contains("remove the plans source"), "a change is still under the source: {out}");
    assert_eq!(read(repo, ".canon/changes/checkout/brief.md"), "Source: openspec/changes/checkout/proposal.md\n\nApproved: ana 2026-10-10 — \"go\"\n");
    assert_eq!(read(repo, "openspec/changes/checkout/proposal.md"), proposal, "the external plan is not edited");
    assert!(read(repo, "canon.yaml").contains("    - dialect: openspec\n      root: .\n"), "the source stays");

    ok(repo, &["subject", "adopt", "checkout", "--subject-id", "cart"]);
    let status = ok(repo, &["status"]);
    assert!(status.contains("brief: approved in .canon/changes/checkout/brief.md — ana 2026-10-10 — \"go\""), "{status}");
    let again = ok(repo, &["migrate", "--dry-run"]);
    assert!(!again.contains("write .canon/changes/checkout/brief.md"), "a second run writes no brief: {again}");
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

    let out = ok(repo, &["migrate", "--dry-run"]);
    assert!(
        out.contains("  left: openspec/changes/half-done stays where it is: its files are `canon change new`'s scaffold, but its first stored version was not adopted into a subject, as `canon change new` adopts\n"),
        "{out}"
    );
    assert!(
        out.contains("  left: openspec/changes/hand-written stays where it is: its first stored version was adopted at once, as `canon change new` adopts, but its files are not that command's scaffold\n"),
        "{out}"
    );
    assert!(out.contains("  move openspec/changes/add-cart -> .canon/changes/add-cart"), "the canon-written change still moves: {out}");
    assert!(!out.contains("remove the plans source"), "{out}");
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
        ok(repo, &["evidence", "add", "--scenario-id", "cart.add.01", "--project-id", "root", "--kind", "test-run", "--ref", "npm test", "--role", "implementer", "--actor-id", "impl-agent"]);
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
    let out = ok(repo, &["migrate"]);
    assert!(read(repo, "canon.yaml").contains("      root: .canon/specs\n      former_roots: [specs]\n"));
    assert!(out.contains("canon migrate: pinned 1 review(s) to what their evidence says") && !out.contains("could not be checked"), "checked at its former root, so not undecidable: {out}");
    let after = ok(repo, &["gate", "check"]);
    assert!(after.contains("canon gate check: clean (0 violations)"), "after the move: {after}");

    // The same move without `former_roots`: the pin's commit has the scenario at a path the root no longer names.
    let dir = legacy_repo();
    let repo = dir.path();
    git(repo, &["mv", "specs", ".canon/specs"]);
    write(repo, "canon.yaml", &read(repo, "canon.yaml").replace("      root: specs\n", "      root: .canon/specs\n"));
    commit_all(repo, "move the root by hand");
    let out = ok(repo, &["migrate"]);
    assert!(!read(repo, "canon.yaml").contains("former_roots"), "{}", read(repo, "canon.yaml"));
    assert!(out.contains("canon migrate: pinned 1 review(s)") && out.contains("1 of those could not be checked"), "without former_roots the review is undecidable: {out}");
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

    let out = canon(repo, &["migrate"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(
        text(&out).contains("canon migrate: MIGRATE_REFUSED — could not rewrite canon.yaml `specs.roots` safely (its `roots:` entries are not a block list)"),
        "{}",
        text(&out)
    );
    assert!(text(&out).contains("set it by hand: `specs.roots[0]` with `root: .canon/specs` and `former_roots: [specs]`"), "{}", text(&out));
    assert!(repo.join(OLD_FEATURE).is_file());
    assert_eq!(read(repo, "canon.yaml"), yaml);
}

/// `canon.yaml` once the `old_repo()` migration is complete.
fn migrated_yaml() -> String {
    OLD_YAML
        .replace("      root: specs\n", "      root: .canon/specs\n      former_roots: [specs]\n")
        .replace("  sources:\n    - dialect: openspec\n      root: .\n", "  sources: []\n")
        + "\n# Harnesses the canon skill is installed for: any of claude, codex, omp, pi\n# (`[]` for none). `canon skills install|check|doctor` follow this list;\n# `canon skills install --providers=<list>` rewrites it.\nskills:\n  providers: [claude, codex]\n"
}

fn set_mode(path: &Path, mode: u32) {
    std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(mode)).unwrap();
}

/// The latest stored version of change `id`.
fn latest_change(repo: &Path, id: &str) -> serde_json::Value {
    query(repo, "change").into_iter().filter(|c| c["change_id"] == id).max_by_key(|c| c["at"].as_str().unwrap().to_string()).unwrap()
}

/// authoring.migrate.09: canon.yaml is written whole or not at all. A run
/// whose canon.yaml write fails after the corpus and the change moved
/// leaves canon.yaml byte-identical and exits 2; the rerun sees the root
/// already at `.canon/specs` while canon.yaml still names `specs`, writes
/// the `root:`/`former_roots` edit and the source removal, and re-imports
/// the change that was moved but not re-imported.
#[test]
fn authoring_migrate_09_a_run_stopped_before_its_canon_yaml_write_is_finished_by_the_next() {
    let dir = old_repo();
    let repo = dir.path();
    // The temp file canon.yaml is written through cannot be created.
    std::fs::create_dir(repo.join(".canon.yaml.canon-migrate.tmp")).unwrap();
    let out = canon(repo, &["migrate"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(text(&out).contains("canon migrate: MIGRATE_STOPPED — failed to write"), "{}", text(&out));
    assert!(text(&out).contains("canon.yaml is unchanged, and running `canon migrate` again finishes what this run started"), "{}", text(&out));
    assert_eq!(read(repo, "canon.yaml"), OLD_YAML, "never half-written");
    assert!(repo.join(NEW_FEATURE).is_file() && repo.join(".canon/changes/add-cart/brief.md").is_file(), "the moves happened before the write failed");

    std::fs::remove_dir(repo.join(".canon.yaml.canon-migrate.tmp")).unwrap();
    let out = ok(repo, &["migrate"]);
    assert!(
        out.contains("  spec root `root` is already at .canon/specs/kind=feature (an earlier run stopped before writing canon.yaml); canon.yaml root: .canon/specs, former_roots: [specs]\n"),
        "{out}"
    );
    assert!(out.contains("  re-import change `add-cart` from .canon/changes/add-cart (an earlier run moved it and stopped before re-importing it)\n"), "{out}");
    assert!(out.contains("  canon.yaml: remove the plans source `{ dialect: openspec, root: . }` canon init wrote"), "{out}");
    assert_eq!(read(repo, "canon.yaml"), migrated_yaml());
    let latest = latest_change(repo, "add-cart");
    assert_eq!(latest["actor"]["agent_id"], "canon-plan-import-canon", "{latest}");
    assert_eq!(latest["subject_ids"], serde_json::json!(["cart"]), "{latest}");
    assert!(ok(repo, &["migrate"]).starts_with("canon migrate: nothing to migrate; the repo is on the 0.17 layout\n"));
}

/// authoring.migrate.10: when a pointer brief cannot be written, the
/// openspec source that still holds the approval stays configured and the
/// run exits 2; the rerun writes the brief and only then removes the
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

    let out = canon(repo, &["migrate"]);
    set_mode(&repo.join(".canon/changes"), 0o755);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(text(&out).contains("canon migrate: MIGRATE_STOPPED — failed to write .canon/changes/checkout/brief.md"), "{}", text(&out));
    assert_eq!(read(repo, "canon.yaml"), yaml, "the source canon init wrote stays while the approval is not copied");
    assert!(repo.join("openspec/changes/.gitkeep").is_file());

    let out = ok(repo, &["migrate"]);
    assert!(out.contains("canon migrate: wrote .canon/changes/checkout/brief.md (Source: ext/openspec/changes/checkout/proposal.md)"), "{out}");
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
    let out = canon(repo, &["migrate"]);
    set_mode(&repo.join("openspec/changes/add-cart"), 0o755);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    assert!(
        text(&out).contains("canon migrate: MIGRATE_REFUSED — openspec/changes/add-cart is not writable, so change dir openspec/changes/add-cart cannot be moved"),
        "{}",
        text(&out)
    );
    assert!(repo.join(OLD_FEATURE).is_file() && repo.join("openspec/changes/add-cart/proposal.md").is_file(), "nothing moved");
    assert_eq!(read(repo, "canon.yaml"), OLD_YAML);

    // An earlier run moved the change dir and stopped before the rename.
    std::fs::create_dir_all(repo.join(".canon/changes")).unwrap();
    git(repo, &["mv", "openspec/changes/add-cart", ".canon/changes/add-cart"]);
    let out = ok(repo, &["migrate"]);
    assert!(out.contains("  rename .canon/changes/add-cart/proposal.md -> .canon/changes/add-cart/brief.md (an earlier run moved change `add-cart` and stopped before this rename)\n"), "{out}");
    assert!(out.contains("canon migrate: re-imported change `add-cart` from .canon/changes/add-cart"), "{out}");
    assert!(repo.join(".canon/changes/add-cart/brief.md").is_file() && !repo.join(".canon/changes/add-cart/proposal.md").exists());
    assert_eq!(latest_change(repo, "add-cart")["actor"]["agent_id"], "canon-plan-import-canon");
    assert_eq!(read(repo, "canon.yaml"), migrated_yaml());
    assert!(ok(repo, &["migrate"]).starts_with("canon migrate: nothing to migrate; the repo is on the 0.17 layout\n"));
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

    let out = ok(repo, &["migrate", "--dry-run"]);
    assert!(out.contains("  left: openspec/changes/broken stays where it is: the openspec reader cannot read it (missing-proposal-md)\n"), "{out}");
    assert!(out.contains("  move openspec/changes/add-cart -> .canon/changes/add-cart"), "{out}");
    assert!(!out.contains("remove the plans source"), "{out}");

    std::fs::remove_dir_all(repo.join("openspec/changes/broken")).unwrap();
    commit_all(repo, "only the notes are left");
    let out = ok(repo, &["migrate"]);
    assert!(out.contains("  left: the plans source `{ dialect: openspec, root: . }` stays: canon has not read openspec/changes/notes.txt\n"), "{out}");
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

    let dry = ok(repo, &["migrate", "--dry-run"]);
    assert!(dry.contains("  .gitignore: add .canon/archive/kind=scenario/, .canon/dashboard-snapshot/, .canon/learn/, .canon/r2/\n"), "{dry}");
    assert_eq!(read(repo, ".gitignore"), old, "--dry-run writes nothing");

    let out = ok(repo, &["migrate"]);
    assert!(out.contains("canon migrate: added to .gitignore: .canon/archive/kind=scenario/, .canon/dashboard-snapshot/, .canon/learn/, .canon/r2/\n"), "{out}");
    let gitignore = read(repo, ".gitignore");
    assert!(gitignore.starts_with(old), "every existing byte is kept: {gitignore}");
    for line in [".canon/archive/kind=scenario/", ".canon/dashboard-snapshot/", ".canon/learn/", ".canon/r2/"] {
        assert_eq!(gitignore.lines().filter(|l| l.trim() == line).count(), 1, "{line} once in:\n{gitignore}");
    }

    let again = ok(repo, &["migrate"]);
    assert!(!again.contains("added to .gitignore"), "{again}");
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

    let out = ok(repo, &["migrate"]);
    assert!(out.contains("canon migrate: added to .gitignore: .canon/archive/kind=scenario/, .canon/learn/\n"), "{out}");
    assert_eq!(read(repo, ".gitignore"), full, "the lines return under their own comments, byte for byte");
    for comment in full.lines().filter(|line| line.starts_with('#')) {
        assert_eq!(read(repo, ".gitignore").lines().filter(|l| *l == comment).count(), 1, "{comment} once");
    }

    ok(repo, &["migrate"]);
    assert_eq!(read(repo, ".gitignore"), full, "a second run changes nothing");
}
