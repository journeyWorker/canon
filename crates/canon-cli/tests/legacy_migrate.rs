//! `canon migrate` upgrades a 0.27 repo (0.28 D9): the hook entries, scripts and full skill
//! projection a 0.27 install leaves are rewritten to their 0.28 forms, with backups, and
//! `status`, `gate check` and `context` lead to it. The fixture is laid out from real 0.27
//! files (`tests/fixtures/v0_27/`, taken from the v0.27.0 tag): its skill sources, retrieve
//! hook script and the settings files `canon gate install-hooks` wrote.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/v0_27");
const REFERENCES: [&str; 4] = ["role-implementer", "role-planner", "role-reviewer", "role-orchestrator"];
const PROJECTIONS: [&str; 2] = [".claude/skills/canon", ".agents/skills/canon"];
const HOOK_COPY: &str = ".claude/hooks/canon-retrieve-pre-dispatch.sh";

fn canon(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(repo).env("CANON_ACTOR", "canon").env_remove("CANON_ROLE").env_remove("CANON_SKILLS_SOURCE").output().expect("spawn canon")
}

fn ok(repo: &Path, args: &[&str]) -> Output {
    let out = canon(repo, args);
    assert!(out.status.success(), "canon {args:?}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    out
}

fn json(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("not JSON ({e}): {}", String::from_utf8_lossy(&out.stdout)))
}

fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"]).args(args).current_dir(repo).output().expect("git on PATH");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn fixture(path: &str) -> Vec<u8> {
    std::fs::read(Path::new(FIXTURE).join(path)).unwrap_or_else(|e| panic!("fixture {path}: {e}"))
}

fn put(repo: &Path, path: &str, bytes: &[u8]) {
    let target = repo.join(path);
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::write(target, bytes).unwrap();
}

fn read(repo: &Path, path: &str) -> Vec<u8> {
    std::fs::read(repo.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// Every file under the repo except `.git`, as path -> bytes.
fn tree(repo: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![repo.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else {
                out.insert(path.strip_prefix(repo).unwrap().to_string_lossy().into_owned(), std::fs::read(&path).unwrap());
            }
        }
    }
    out
}

/// A repo `canon init` set up, then put back the way a 0.27 install left it: the full skill
/// (SKILL.md, reference files, retrieve script) in both projections with the install lock that
/// lists them, `canon gate task` hook entries in both settings files, a retrieve hook script copy
/// named by one of them, and the pre-commit script. Committed.
fn repo_027() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q"]);
    ok(repo, &["init", "--providers", "claude,codex", "--no-agents-md", "--no-policy"]);
    for projection in PROJECTIONS {
        std::fs::remove_dir_all(repo.join(projection)).unwrap();
    }
    std::fs::remove_dir_all(repo.join(".canon/skills")).unwrap();
    let mut files = BTreeMap::new();
    for projection in PROJECTIONS {
        let mut layout = vec![("SKILL.md".to_string(), "skill/SKILL.md".to_string()), ("scripts/canon-retrieve-pre-dispatch.sh".to_string(), "skill/scripts/canon-retrieve-pre-dispatch.sh".to_string())];
        layout.extend(REFERENCES.map(|name| (format!("reference/{name}.md"), format!("skill/reference/{name}.md"))));
        for (relative, source) in layout {
            let bytes = fixture(&source);
            files.insert(format!("{projection}/{relative}"), canon_cli::skills::content_hash(&bytes));
            put(repo, &format!("{projection}/{relative}"), &bytes);
        }
    }
    let lock = serde_json::json!({ "version": 1, "canon_version": "0.27.0", "source_hash": "sha256:0", "providers": ["claude", "codex"], "files": files });
    put(repo, ".canon/skills/.install-lock.json", format!("{}\n", serde_json::to_string_pretty(&lock).unwrap()).as_bytes());
    put(repo, ".claude/settings.json", &fixture("repo/.claude/settings.json"));
    put(repo, ".codex/hooks.json", &fixture("repo/.codex/hooks.json"));
    put(repo, ".canon/scripts/canon-gate-pre-commit.sh", &fixture("repo/.canon/scripts/canon-gate-pre-commit.sh"));
    put(repo, HOOK_COPY, &fixture("skill/scripts/canon-retrieve-pre-dispatch.sh"));
    // The AGENTS.md 0.27's `init` wrote, with the owner's own text before and after the canon block.
    put(repo, "AGENTS.md", &fixture("repo/AGENTS.md"));
    // The retrieve hook entry `canon gate install-hooks` documents, beside the gate entry.
    ok(repo, &["gate", "install-hooks", "--event", "PreToolUse", "--matcher", "Task", "--command", &format!("sh {HOOK_COPY}"), "--timeout", "15"]);
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "a 0.27 repo"]);
    dir
}

fn commands(settings: &Value) -> Vec<String> {
    settings["hooks"]["PreToolUse"].as_array().unwrap().iter().flat_map(|group| group["hooks"].as_array().unwrap()).map(|hook| hook["command"].as_str().unwrap().to_string()).collect()
}

fn effect_paths(preview: &Value) -> Vec<String> {
    preview["data"]["effects"].as_array().unwrap().iter().filter_map(|effect| effect["path"].as_str().map(str::to_string)).collect()
}

fn commit(repo: &Path) -> Output {
    let preview = json(&ok(repo, &["migrate", "--json"]));
    let digest = preview["data"]["planDigest"].as_str().unwrap().to_string();
    ok(repo, &["migrate", "--commit", "--plan-digest", &digest, "--json"])
}

fn backup_dirs(repo: &Path) -> Vec<PathBuf> {
    match std::fs::read_dir(repo.join(".canon/migrate-backup")) {
        Ok(entries) => entries.flatten().map(|entry| entry.path()).collect(),
        Err(_) => Vec::new(),
    }
}

/// D9: the preview lists the hook, script and skill effects and writes nothing; its first next
/// step is the commit command with the actual digest.
#[test]
fn migrate_previews_the_027_effects_and_writes_nothing() {
    let dir = repo_027();
    let repo = dir.path();
    let before = tree(repo);
    let out = ok(repo, &["migrate", "--json"]);
    let preview = json(&out);
    assert_eq!(tree(repo), before, "the preview writes nothing");
    assert_eq!(preview["data"]["committed"], false);
    let paths = effect_paths(&preview);
    for expected in [".claude/settings.json", ".codex/hooks.json", HOOK_COPY, ".canon/skills/.install-lock.json", "AGENTS.md"] {
        assert!(paths.iter().any(|path| path == expected), "{expected} is an effect: {paths:?}");
    }
    for projection in PROJECTIONS {
        for file in ["SKILL.md", "scripts/canon-retrieve-pre-dispatch.sh"] {
            assert!(paths.contains(&format!("{projection}/{file}")), "{projection}/{file} is rewritten: {paths:?}");
        }
        for name in REFERENCES {
            assert!(paths.contains(&format!("{projection}/reference/{name}.md")), "{projection}/reference/{name}.md is removed: {paths:?}");
        }
    }
    assert!(!paths.iter().any(|path| path == ".canon/scripts/canon-gate-pre-commit.sh"), "the 0.27 pre-commit script already calls only spellings 0.28 keeps");
    // The stale 0.27 guide on disk is called out by the preview itself: it names the replacement route.
    let warnings = preview["meta"]["warnings"].as_array().unwrap_or_else(|| panic!("the preview warns: {}", preview["meta"]));
    assert!(warnings.iter().any(|w| w.as_str().is_some_and(|w| w.contains("canon migrate --json") && w.contains("canon skills show canon"))), "{warnings:?}");
    let digest = preview["data"]["planDigest"].as_str().unwrap();
    let step = &preview["meta"]["next"][0];
    assert_eq!(step["argv"], serde_json::json!(["canon", "migrate", "--commit", "--plan-digest", digest, "--json"]));
}

/// D9: the commit rewrites each file, backs up the originals, and a second run has nothing left.
#[test]
fn migrate_commit_rewrites_backs_up_and_is_idempotent() {
    let dir = repo_027();
    let repo = dir.path();
    let originals = tree(repo);
    let committed = json(&commit(repo));
    assert_eq!(committed["data"]["committed"], true);

    for file in [".claude/settings.json", ".codex/hooks.json"] {
        let settings: Value = serde_json::from_slice(&read(repo, file)).unwrap();
        let found = commands(&settings);
        assert!(found.contains(&"canon task done".to_string()), "{file}: {found:?}");
        assert!(!found.iter().any(|command| command.contains("canon gate task")), "{file}: {found:?}");
    }
    let claude: Value = serde_json::from_slice(&read(repo, ".claude/settings.json")).unwrap();
    assert!(commands(&claude).contains(&format!("sh {HOOK_COPY}")), "the other hook entry stays");

    let embedded_script = canon_cli::guide::scripts().find(|(path, _)| path.ends_with("canon-retrieve-pre-dispatch.sh")).unwrap().1;
    assert_eq!(read(repo, HOOK_COPY), embedded_script.as_bytes(), "the retrieve script is regenerated from the embedded source");
    for projection in PROJECTIONS {
        assert_eq!(read(repo, &format!("{projection}/scripts/canon-retrieve-pre-dispatch.sh")), embedded_script.as_bytes());
        let skill = String::from_utf8(read(repo, &format!("{projection}/SKILL.md"))).unwrap();
        assert!(skill.starts_with("---\nname: canon\n"), "the stub keeps the frontmatter harnesses route by");
        assert!(skill.contains("canon skills show canon"), "the stub points at the binary's guide");
        assert!(!repo.join(projection).join("reference").exists(), "{projection}/reference is gone");
    }
    let lock: Value = serde_json::from_slice(&read(repo, ".canon/skills/.install-lock.json")).unwrap();
    assert_eq!(lock["canon_version"], env!("CARGO_PKG_VERSION"));
    assert!(!lock["files"].as_object().unwrap().keys().any(|key| key.contains("/reference/")));
    ok(repo, &["skills", "check"]);

    // AGENTS.md: the canon block is this canon's, the owner's text around it is byte-identical.
    let agents = String::from_utf8(read(repo, "AGENTS.md")).unwrap();
    let before = String::from_utf8(originals["AGENTS.md"].clone()).unwrap();
    let outside = |text: &str| (text[..text.find("<!-- canon:begin -->").unwrap()].to_string(), text[text.find("<!-- canon:end -->").unwrap() + "<!-- canon:end -->".len()..].to_string());
    assert_eq!(outside(&agents), outside(&before), "text outside the markers is untouched");
    assert!(agents.contains(canon_cli::init::AGENTS_BLOCK), "the block is the one this canon's init writes");
    assert_ne!(agents, before);

    // Backups: one directory holding every rewritten or removed file with its original bytes.
    let backups = backup_dirs(repo);
    assert_eq!(backups.len(), 1, "{backups:?}");
    for file in [".claude/settings.json", ".codex/hooks.json", HOOK_COPY, ".canon/skills/.install-lock.json", ".claude/skills/canon/SKILL.md", ".agents/skills/canon/reference/role-planner.md", ".claude/skills/canon/scripts/canon-retrieve-pre-dispatch.sh", "AGENTS.md"] {
        assert_eq!(std::fs::read(backups[0].join(file)).unwrap(), originals[file], "{file} is backed up with its original bytes");
    }

    // A second run finds no hook, script or skill left to rewrite and makes no new backup.
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "migrated"]);
    let again = json(&ok(repo, &["migrate", "--json"]));
    assert!(effect_paths(&again).is_empty(), "{:?}", again["data"]["effects"]);
    let tree_before = tree(repo);
    let digest = again["data"]["planDigest"].as_str().unwrap().to_string();
    ok(repo, &["migrate", "--commit", "--plan-digest", &digest, "--json"]);
    assert_eq!(backup_dirs(repo).len(), 1, "a run with nothing to rewrite backs nothing up");
    for (path, bytes) in tree_before.iter().filter(|(path, _)| !path.starts_with(".canon/ledger") && !path.starts_with(".canon/hot")) {
        assert_eq!(read(repo, path), *bytes, "{path} is untouched by a second run");
    }
}

/// D9: a file the commit would rewrite but the digest no longer matches is not touched.
#[test]
fn migrate_with_a_stale_digest_writes_nothing() {
    let dir = repo_027();
    let repo = dir.path();
    let before = tree(repo);
    let out = canon(repo, &["migrate", "--commit", "--plan-digest", "0000", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(json(&out)["error"]["code"], "PLAN_CHANGED");
    assert_eq!(tree(repo), before);
}

/// D9: `status`, `gate check` and `context` lead a 0.27 repo to the preview, with a warning.
#[test]
fn status_gate_check_and_context_put_migrate_first() {
    let dir = repo_027();
    let repo = dir.path();
    for args in [&["status", "--json"][..], &["gate", "check", "--json"][..], &["context", "--json"][..]] {
        let out = canon(repo, args);
        let document = json(&out);
        let step = &document["meta"]["next"][0];
        assert_eq!(step["argv"], serde_json::json!(["canon", "migrate", "--json"]), "{args:?}: {}", document["meta"]);
        let warnings = document["meta"]["warnings"].as_array().unwrap_or_else(|| panic!("{args:?} has warnings: {}", document["meta"]));
        assert!(warnings.iter().any(|warning| warning.as_str().unwrap().contains("canon migrate")), "{args:?}: {warnings:?}");
        assert!(warnings.iter().any(|warning| warning.as_str().unwrap().contains("canon skills show canon")), "the stale guide is named: {args:?}: {warnings:?}");
    }
    commit(repo);
    for args in [&["status", "--json"][..], &["context", "--json"][..]] {
        let document = json(&canon(repo, args));
        assert!(!document["meta"]["next"].as_array().is_some_and(|next| next.iter().any(|step| step["command"].as_str().unwrap().starts_with("canon migrate"))), "{args:?} after the migration: {}", document["meta"]);
    }
}

/// D9: a `canon gate task` hook entry alone is 0.27-era state.
#[test]
fn a_gate_task_hook_entry_alone_is_detected_and_rewritten() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q"]);
    ok(repo, &["init", "--providers", "none", "--no-agents-md", "--no-policy"]);
    let clean = json(&ok(repo, &["status", "--json"]));
    assert!(clean["meta"]["next"].as_array().is_none_or(|next| next.iter().all(|step| !step["command"].as_str().unwrap().starts_with("canon migrate"))));
    put(repo, ".codex/hooks.json", &fixture("repo/.codex/hooks.json"));
    let document = json(&ok(repo, &["status", "--json"]));
    assert_eq!(document["meta"]["next"][0]["argv"], serde_json::json!(["canon", "migrate", "--json"]));
    let committed = json(&commit(repo));
    assert_eq!(committed["data"]["committed"], true);
    let settings: Value = serde_json::from_slice(&read(repo, ".codex/hooks.json")).unwrap();
    assert_eq!(commands(&settings), vec!["canon task done".to_string()]);
}

/// D10: `skills install` on a 0.27 projection prunes the reference files whose bytes match the
/// lock and keeps the one that was edited.
#[test]
fn skills_install_prunes_the_old_references_and_keeps_an_edited_one() {
    let dir = repo_027();
    let repo = dir.path();
    let edited = ".claude/skills/canon/reference/role-planner.md";
    put(repo, edited, b"my own notes\n");
    let report = json(&ok(repo, &["skills", "install", "--json"]));
    let kept: Vec<&str> = report["data"]["kept"].as_array().unwrap().iter().map(|entry| entry["path"].as_str().unwrap()).collect();
    assert!(kept.iter().any(|path| path.ends_with(edited)), "{kept:?}");
    assert_eq!(read(repo, edited), b"my own notes\n");
    for name in ["role-implementer", "role-reviewer", "role-orchestrator"] {
        assert!(!repo.join(format!(".claude/skills/canon/reference/{name}.md")).exists(), "{name} is pruned");
    }
    assert_eq!(json(&ok(repo, &["skills", "install", "--json"]))["data"]["changed"], false, "a second install changes nothing");
}

/// D9: an AGENTS.md without the canon markers is the owner's file: migrate does not add the block, and a
/// repo whose block is already current has nothing to rewrite there.
#[test]
fn migrate_leaves_an_agents_file_without_markers_or_with_a_current_block_alone() {
    let dir = repo_027();
    let repo = dir.path();
    put(repo, "AGENTS.md", b"# Mine\n\nNo canon block here.\n");
    let preview = json(&ok(repo, &["migrate", "--json"]));
    assert!(!effect_paths(&preview).iter().any(|path| path == "AGENTS.md"), "{:?}", effect_paths(&preview));
    let current = format!("# Mine\n\n{}\n\nafter\n", canon_cli::init::AGENTS_BLOCK);
    put(repo, "AGENTS.md", current.as_bytes());
    let preview = json(&ok(repo, &["migrate", "--json"]));
    assert!(!effect_paths(&preview).iter().any(|path| path == "AGENTS.md"), "{:?}", effect_paths(&preview));
    let committed = json(&commit(repo));
    assert_eq!(committed["data"]["committed"], true);
    assert_eq!(read(repo, "AGENTS.md"), current.as_bytes());
}

/// D9: a repo whose only 0.27 leftover is an old AGENTS.md block is 0.27-era state: `status` says so.
#[test]
fn an_old_agents_block_alone_is_detected_and_migrated() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q"]);
    ok(repo, &["init", "--providers", "none", "--no-agents-md", "--no-policy"]);
    put(repo, "AGENTS.md", &fixture("repo/AGENTS.md"));
    let status = json(&canon(repo, &["status", "--json"]));
    assert_eq!(status["meta"]["next"][0]["argv"], serde_json::json!(["canon", "migrate", "--json"]), "{}", status["meta"]);
    let preview = json(&ok(repo, &["migrate", "--json"]));
    assert_eq!(effect_paths(&preview), ["AGENTS.md"]);
    commit(repo);
    let status = json(&canon(repo, &["status", "--json"]));
    assert!(!status["meta"]["next"].as_array().is_some_and(|next| next.iter().any(|s| s["command"].as_str().unwrap().starts_with("canon migrate"))), "{}", status["meta"]);
}
