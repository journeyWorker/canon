//! Integration test for `canon skills install` (S0 task 6.2), exercising
//! the `native-launcher`-adjacent `skill-materialization` spec scenarios:
//! verbatim `.claude/` and `.agents/` (Codex) copies + a timestamp-free
//! content-hash/version lock, idempotence across two consecutive runs with
//! no source change, and the migration of a legacy `.codex/skills` Codex
//! projection that canon 0.13.0 and earlier wrote.
//!
//! The checked-in fixture (`fixtures/skills-repo/`) is never mutated: each
//! test copies it into a fresh tempdir first, so `cargo test` stays
//! side-effect-free against the repo tree.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use canon_cli::skills::{self, Lock};

/// `key: value` pairs of a `SKILL.md` frontmatter block, or `None` when the
/// content does not open with a closed `---` block.
fn frontmatter(content: &str) -> Option<BTreeMap<String, String>> {
    let rest = content.strip_prefix("---\n")?;
    let end = rest.find("\n---\n")?;
    Some(
        rest[..end]
            .lines()
            .filter_map(|line| line.split_once(':'))
            .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
            .collect(),
    )
}

fn copy_fixture_into(tmp: &Path) {
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/skills-repo");
    copy_dir_recursive(&fixture_root, tmp);
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir_recursive(&src_path, &dst_path);
        } else {
            fs::copy(&src_path, &dst_path).unwrap();
        }
    }
}

#[test]
fn install_materializes_claude_and_codex_and_lock() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture_into(tmp.path());

    let source = tmp.path().join("canon/skills");
    let target = tmp.path();

    let report = skills::install(&source, target).expect("install should succeed");
    assert_eq!(report.installed.len(), 1);
    let skill = &report.installed[0];
    assert_eq!(skill.name, "example-skill");
    assert_eq!(skill.version, 1);
    assert!(skill.changed);

    // Verbatim Claude Code copy.
    let claude_path = target.join(".claude/skills/example-skill/SKILL.md");
    let original = fs::read_to_string(source.join("example-skill/SKILL.md")).unwrap();
    let materialized = fs::read_to_string(&claude_path).unwrap();
    assert_eq!(materialized, original, "claude materialization must be byte-verbatim");

    // Codex reads `.agents/skills/<name>/SKILL.md`: the same verbatim copy,
    // never a flattened `.codex/skills/<name>.md`.
    let codex_path = target.join(".agents/skills/example-skill/SKILL.md");
    assert_eq!(fs::read_to_string(&codex_path).unwrap(), original, "codex materialization must be byte-verbatim");
    assert!(!target.join(".codex/skills").exists());

    // Gemini is never touched (decision 11).
    assert!(!target.join(".gemini").exists());

    // Lock: content hash + monotonic version, no generatedAt field anywhere.
    let lock_path = source.join(".install-lock.json");
    let lock_raw = fs::read_to_string(&lock_path).unwrap();
    assert!(!lock_raw.contains("generatedAt"));
    let lock: Lock = serde_json::from_str(&lock_raw).unwrap();
    let entry = lock.skills.get("example-skill").expect("lock entry for example-skill");
    assert!(entry.content_hash.starts_with("sha256:"));
    assert_eq!(entry.version, 1);
}

#[test]
fn install_is_idempotent_with_no_source_change() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture_into(tmp.path());

    let source = tmp.path().join("canon/skills");
    let target = tmp.path();

    let first = skills::install(&source, target).expect("first install should succeed");
    let claude_after_first = fs::read_to_string(target.join(".claude/skills/example-skill/SKILL.md")).unwrap();
    let codex_after_first = fs::read_to_string(target.join(".agents/skills/example-skill/SKILL.md")).unwrap();
    let lock_after_first = fs::read_to_string(source.join(".install-lock.json")).unwrap();

    let second = skills::install(&source, target).expect("second install should succeed");
    let claude_after_second = fs::read_to_string(target.join(".claude/skills/example-skill/SKILL.md")).unwrap();
    let codex_after_second = fs::read_to_string(target.join(".agents/skills/example-skill/SKILL.md")).unwrap();
    let lock_after_second = fs::read_to_string(source.join(".install-lock.json")).unwrap();

    assert_eq!(claude_after_first, claude_after_second, "claude output must be byte-identical across reruns");
    assert_eq!(codex_after_first, codex_after_second, "codex output must be byte-identical across reruns");
    assert_eq!(lock_after_first, lock_after_second, "lock must be byte-identical across reruns");

    // Second run reports "unchanged" (content hash matched the existing lock entry).
    assert!(!second.installed[0].changed);
    assert_eq!(second.installed[0].version, first.installed[0].version);
}

#[test]
fn content_change_bumps_version_not_timestamp() {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture_into(tmp.path());

    let source = tmp.path().join("canon/skills");
    let target = tmp.path();

    let first = skills::install(&source, target).expect("first install should succeed");
    assert_eq!(first.installed[0].version, 1);

    // Mutate the skill's source content.
    let skill_md = source.join("example-skill/SKILL.md");
    let mut content = fs::read_to_string(&skill_md).unwrap();
    content.push_str("\nAppended content to force a hash change.\n");
    fs::write(&skill_md, content).unwrap();

    let second = skills::install(&source, target).expect("second install should succeed");
    assert!(second.installed[0].changed);
    assert_eq!(second.installed[0].version, 2, "version increments by exactly one");
}

#[test]
fn canonical_codex_projects_agents_skill_with_frontmatter_and_sidecars() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    fs::create_dir_all(source.join("reference")).unwrap();
    fs::create_dir_all(source.join("scripts")).unwrap();
    let source_skill = "---\nname: canon\ndescription: umbrella\n---\n\n# Canon\n";
    fs::write(source.join("SKILL.src.md"), source_skill).unwrap();
    fs::write(source.join("reference/topic.md"), "# topic\n").unwrap();
    fs::write(source.join("scripts/hook.sh"), "#!/bin/sh\n").unwrap();

    let first = skills::install_canonical(&source, &target, Some("codex")).unwrap();
    assert!(first.changed);
    assert!(!target.join(".claude/skills/canon").exists());
    let bundle = target.join(".agents/skills/canon");
    let entrypoint = fs::read_to_string(bundle.join("SKILL.md")).unwrap();
    assert_eq!(entrypoint, source_skill, "the Codex SKILL.md is the source, byte-verbatim");
    let fields = frontmatter(&entrypoint).expect("the Codex SKILL.md must open with a closed frontmatter block");
    assert_eq!(fields.get("name").map(String::as_str), Some("canon"));
    assert!(fields.get("description").is_some_and(|description| !description.is_empty()), "Codex requires a description: {fields:?}");
    assert_eq!(fs::read_to_string(bundle.join("reference/topic.md")).unwrap(), "# topic\n");
    assert_eq!(fs::read_to_string(bundle.join("scripts/hook.sh")).unwrap(), "#!/bin/sh\n");
    assert!(!target.join(".codex").exists(), "nothing is written under .codex");
    assert!(!source.join(".install-lock.json").exists());

    let second = skills::install_canonical(&source, &target, Some("codex")).unwrap();
    assert!(!second.changed);
    let check = skills::check(&source, &target, Some("codex")).unwrap();
    assert!(check.manifest_ok);
    assert!(check.remnants.is_empty());
    assert!(check.statuses.iter().all(|status| status.state == "ok"));
}

/// skills.install.50: `.codex` marks a Codex user; `.agents` alone, which
/// other tools use too, selects nothing (0.17 D5).
#[test]
fn skills_install_50_codex_is_detected_from_codex_never_from_agents_alone() {
    for (roots, expected) in [
        (vec![".codex"], vec![skills::Provider::Codex]),
        (vec![".agents", ".codex"], vec![skills::Provider::Codex]),
        (vec![".agents"], vec![]),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        let target = tmp.path().join("target");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("SKILL.src.md"), "---\nname: canon\ndescription: umbrella\n---\n").unwrap();
        for root in &roots {
            fs::create_dir_all(target.join(root)).unwrap();
        }

        let report = skills::install_canonical(&source, &target, None).unwrap();
        assert_eq!(report.providers, expected, "roots {roots:?}");
        assert_eq!(report.origin, skills::ProviderOrigin::Detected, "roots {roots:?}");
        assert_eq!(target.join(".agents/skills/canon/SKILL.md").is_file(), !expected.is_empty(), "roots {roots:?}");
        assert!(!target.join(".codex/skills").exists(), "roots {roots:?}");
    }
}

/// skills.install.51: a bare target with no marker directory gets nothing:
/// no projection, no lock, no undetected default; the CLI names the flag.
#[test]
fn skills_install_51_a_bare_target_installs_nothing_and_names_the_flag() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    write_canonical_source(&source);
    fs::create_dir_all(&target).unwrap();

    let report = skills::install_canonical(&source, &target, None).unwrap();
    assert_eq!(report.providers, vec![]);
    assert_eq!(report.origin, skills::ProviderOrigin::Detected);
    assert!(!report.changed);
    assert_eq!(fs::read_dir(&target).unwrap().count(), 0, "nothing at all is written");

    let install = canon(&["skills", "install"], &source, &target);
    let out = String::from_utf8_lossy(&install.stdout);
    assert_eq!(install.status.code(), Some(0), "{out}");
    assert_eq!(
        out,
        "canon v1 — nothing installed: no provider selected (no .claude, .codex, .omp or .pi directory found); choose with `canon skills install --providers=<list>` (any of claude, codex, omp, pi)\n"
    );
    assert_eq!(fs::read_dir(&target).unwrap().count(), 0, "the CLI writes nothing either");

    let check = canon(&["skills", "check"], &source, &target);
    let out = String::from_utf8_lossy(&check.stdout);
    assert_eq!(check.status.code(), Some(0), "nothing selected, nothing to drift: {out}");
    assert!(out.contains("--providers=<list>"), "{out}");
    let doctor = canon(&["skills", "doctor"], &source, &target);
    let out = String::from_utf8_lossy(&doctor.stdout);
    assert!(out.lines().any(|line| line.starts_with("no-providers: ") && line.contains("--providers=<list>")), "{out}");
    assert!(!out.contains("status: healthy"), "{out}");
}

#[test]
fn canonical_check_and_doctor_report_projected_drift_and_legacy_remnant() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    fs::create_dir_all(&source).unwrap();
    fs::write(
        source.join("SKILL.src.md"),
        "---\nname: canon\ndescription: umbrella\n---\n\n# Canon\n",
    )
    .unwrap();
    skills::install_canonical(&source, &target, Some("claude,codex")).unwrap();

    let projected = target.join(".claude/skills/canon/SKILL.md");
    fs::write(&projected, "mutated projection\n").unwrap();
    let legacy = target.join(".claude/skills/canon-old");
    fs::create_dir_all(&legacy).unwrap();

    let check = skills::check(&source, &target, None).unwrap();
    assert!(check.statuses.iter().any(|status| status.state != "ok"));

    let doctor = skills::doctor(&source, &target, None).unwrap();
    assert!(doctor.iter().any(|line| line.contains("stale:") || line.contains("missing:")));
    assert!(doctor.iter().any(|line| line.contains("legacy-remnant:") && line.contains("canon-old")));
    assert!(legacy.exists(), "doctor must report legacy remnants without deleting them");
}

#[test]
fn canonical_install_projects_omp_and_pi_verbatim_with_sidecars() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    fs::create_dir_all(source.join("reference")).unwrap();
    fs::create_dir_all(source.join("scripts")).unwrap();
    let source_skill = "---\nname: canon\ndescription: umbrella\nx-extra: preserved\n---\n\n# Canon\n";
    fs::write(source.join("SKILL.src.md"), source_skill).unwrap();
    fs::write(source.join("reference/topic.md"), "# topic\n").unwrap();
    fs::write(source.join("scripts/pre-dispatch.sh"), "#!/bin/sh\n").unwrap();

    let report = skills::install_canonical(&source, &target, Some("pi,OMP")).unwrap();

    assert_eq!(report.providers, vec![skills::Provider::Omp, skills::Provider::Pi]);
    for root in [".omp", ".pi"] {
        let bundle = target.join(root).join("skills/canon");
        assert_eq!(fs::read_to_string(bundle.join("SKILL.md")).unwrap(), source_skill);
        assert_eq!(fs::read_to_string(bundle.join("reference/topic.md")).unwrap(), "# topic\n");
        assert_eq!(
            fs::read_to_string(bundle.join("scripts/pre-dispatch.sh")).unwrap(),
            "#!/bin/sh\n"
        );
    }
    assert!(!target.join(".claude").exists());
    assert!(!target.join(".agents").exists());
    assert!(!target.join(".codex").exists());
}

#[test]
fn canonical_implicit_detection_selects_omp_pi_in_stable_order() {
    for (roots, expected) in [
        (vec![".omp"], vec![skills::Provider::Omp]),
        (vec![".pi"], vec![skills::Provider::Pi]),
        (vec![".omp", ".pi"], vec![skills::Provider::Omp, skills::Provider::Pi]),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        let target = tmp.path().join("target");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("SKILL.src.md"), "# canon\n").unwrap();
        for root in roots {
            fs::create_dir_all(target.join(root)).unwrap();
        }

        let report = skills::install_canonical(&source, &target, None).unwrap();
        assert_eq!(report.providers, expected);
    }
}

#[test]
fn canonical_check_and_doctor_report_omp_pi_drift_and_remnant() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.src.md"), "---\nname: canon\n---\n\n# Canon\n").unwrap();
    skills::install_canonical(&source, &target, Some("omp,pi")).unwrap();

    let projected = target.join(".omp/skills/canon/SKILL.md");
    fs::write(&projected, "mutated projection\n").unwrap();
    let legacy = target.join(".pi/skills/canon-old");
    fs::create_dir_all(&legacy).unwrap();

    let check = skills::check(&source, &target, Some("omp,pi")).unwrap();
    assert!(check.providers == vec![skills::Provider::Omp, skills::Provider::Pi]);
    assert!(check
        .statuses
        .iter()
        .any(|status| status.provider == skills::Provider::Omp && status.state == "stale"));

    let doctor = skills::doctor(&source, &target, Some("omp,pi")).unwrap();
    assert!(doctor.iter().any(|line| line.contains("stale:") && line.contains(".omp")));
    assert!(doctor.iter().any(|line| line.contains("legacy-remnant:") && line.contains("canon-old")));
    assert!(legacy.exists(), "doctor must report legacy remnants without deleting them");
}

/// Writes the projection canon 0.13.0 installed for Codex — a flattened
/// `.codex/skills/canon.md` plus `.codex/skills/canon/**` sidecars — with the
/// install lock that recorded each file's hash.
fn write_legacy_codex_install(target: &Path) -> Vec<&'static str> {
    let files: [(&str, &str); 3] = [
        (".codex/skills/canon.md", "# canon\n\n> umbrella\n\n# Canon\n"),
        (".codex/skills/canon/reference/topic.md", "# topic\n"),
        (".codex/skills/canon/scripts/hook.sh", "#!/bin/sh\n"),
    ];
    let mut recorded = serde_json::Map::new();
    for (relative, content) in files {
        let path = target.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, content).unwrap();
        recorded.insert(relative.into(), skills::content_hash(content.as_bytes()).into());
    }
    let lock = serde_json::json!({
        "version": 1,
        "source_hash": "sha256:legacy",
        "providers": ["codex"],
        "files": recorded,
    });
    fs::create_dir_all(target.join(".canon/skills")).unwrap();
    fs::write(target.join(".canon/skills/.install-lock.json"), serde_json::to_string_pretty(&lock).unwrap()).unwrap();
    files.iter().map(|(relative, _)| *relative).collect()
}

fn write_canonical_source(source: &Path) {
    fs::create_dir_all(source.join("reference")).unwrap();
    fs::create_dir_all(source.join("scripts")).unwrap();
    fs::write(source.join("SKILL.src.md"), "---\nname: canon\ndescription: umbrella\n---\n\n# Canon\n").unwrap();
    fs::write(source.join("reference/topic.md"), "# topic\n").unwrap();
    fs::write(source.join("scripts/hook.sh"), "#!/bin/sh\n").unwrap();
}

#[test]
fn canonical_legacy_codex_migration_keeps_user_files_and_is_idempotent() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    write_canonical_source(&source);
    let legacy = write_legacy_codex_install(&target);
    let user_file = target.join(".codex/skills/mine.md");
    fs::write(&user_file, "# my own codex note\n").unwrap();

    let first = skills::install_canonical(&source, &target, None).unwrap();
    assert_eq!(first.providers, vec![skills::Provider::Codex], "the legacy lock's providers");
    assert!(first.changed);
    assert!(target.join(".agents/skills/canon/SKILL.md").is_file());
    for relative in &legacy {
        assert!(!target.join(relative).exists(), "the lock-proven legacy file {relative} must be removed");
    }
    assert!(!target.join(".codex/skills/canon").exists(), "the emptied legacy bundle directory is removed");
    assert_eq!(fs::read_to_string(&user_file).unwrap(), "# my own codex note\n", "a user file under .codex/skills is kept");

    let lock_after_first = fs::read(target.join(".canon/skills/.install-lock.json")).unwrap();
    let second = skills::install_canonical(&source, &target, None).unwrap();
    assert!(!second.changed, "a rerun after migration changes nothing");
    assert_eq!(fs::read(target.join(".canon/skills/.install-lock.json")).unwrap(), lock_after_first);
    assert!(user_file.is_file());
    let check = skills::check(&source, &target, None).unwrap();
    assert!(check.manifest_ok);
    assert!(check.remnants.is_empty(), "{:?}", check.remnants);
    assert!(check.statuses.iter().all(|status| status.state == "ok"));
}

#[test]
fn canonical_legacy_codex_migration_removes_emptied_skills_dir_but_keeps_codex_root() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    write_canonical_source(&source);
    write_legacy_codex_install(&target);
    fs::write(target.join(".codex/config.toml"), "model = \"o3\"\n").unwrap();

    skills::install_canonical(&source, &target, Some("claude,codex")).unwrap();

    assert!(!target.join(".codex/skills").exists(), ".codex/skills is removed once it ends up empty");
    assert_eq!(fs::read_to_string(target.join(".codex/config.toml")).unwrap(), "model = \"o3\"\n");
    assert!(target.join(".agents/skills/canon/SKILL.md").is_file());
}

#[test]
fn canonical_legacy_codex_migration_keeps_files_the_lock_does_not_prove() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    write_canonical_source(&source);
    write_legacy_codex_install(&target);
    let edited = target.join(".codex/skills/canon.md");
    fs::write(&edited, "# canon\n\nlocally edited\n").unwrap();
    let unrecorded = target.join(".codex/skills/canon/reference/mine.md");
    fs::write(&unrecorded, "# mine\n").unwrap();

    skills::install_canonical(&source, &target, Some("codex")).unwrap();

    assert_eq!(fs::read_to_string(&edited).unwrap(), "# canon\n\nlocally edited\n", "an edited legacy file is kept");
    assert_eq!(fs::read_to_string(&unrecorded).unwrap(), "# mine\n", "an unrecorded file is kept");
    assert!(!target.join(".codex/skills/canon/reference/topic.md").exists());
    assert!(!target.join(".codex/skills/canon/scripts").exists(), "the emptied scripts directory is removed");
}

/// A legacy directory swapped for a symlink after the lock was written must
/// never let the migration delete through it: the outside files hold the
/// same names and the same recorded hashes, so only the no-follow walk keeps
/// them.
#[cfg(unix)]
#[test]
fn canonical_legacy_codex_migration_never_deletes_through_a_symlinked_legacy_dir() {
    for swapped in [".codex/skills/canon", ".codex/skills", ".codex"] {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        let target = tmp.path().join("target");
        let outside = tmp.path().join("outside");
        write_canonical_source(&source);
        let legacy = write_legacy_codex_install(&target);
        fs::create_dir_all(&outside).unwrap();
        let moved = outside.join("moved");
        fs::rename(target.join(swapped), &moved).unwrap();
        std::os::unix::fs::symlink(&moved, target.join(swapped)).unwrap();
        let outside_files: Vec<_> = legacy
            .iter()
            .filter_map(|relative| relative.strip_prefix(swapped)?.strip_prefix('/'))
            .map(|rest| moved.join(rest))
            .collect();
        assert!(!outside_files.is_empty() && outside_files.iter().all(|path| path.is_file()), "{swapped}");

        skills::install_canonical(&source, &target, Some("codex")).unwrap();

        for path in &outside_files {
            assert!(path.is_file(), "swapping {swapped} for a symlink let the migration delete {}", path.display());
        }
        assert!(fs::symlink_metadata(target.join(swapped)).unwrap().file_type().is_symlink(), "{swapped}");
        assert!(target.join(".agents/skills/canon/SKILL.md").is_file());
    }
}

#[test]
fn canonical_check_and_doctor_report_legacy_codex_remnant_with_fix() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    write_canonical_source(&source);
    skills::install_canonical(&source, &target, Some("claude,codex")).unwrap();
    // A legacy projection no install lock records (e.g. a lock that was
    // never committed): install cannot prove canon wrote it.
    let remnant = target.join(".codex/skills/canon.md");
    fs::create_dir_all(remnant.parent().unwrap()).unwrap();
    fs::write(&remnant, "# canon\n\n> umbrella\n\n# Canon\n").unwrap();

    let check = skills::check(&source, &target, Some("claude,codex")).unwrap();
    assert_eq!(check.remnants, vec![remnant.clone()]);
    assert!(check.statuses.iter().all(|status| status.state == "ok"), "the remnant is not projection drift");

    let doctor = skills::doctor(&source, &target, Some("claude,codex")).unwrap();
    let line = doctor
        .iter()
        .find(|line| line.starts_with("legacy-remnant:") && line.contains(".codex/skills/canon.md"))
        .unwrap_or_else(|| panic!("doctor must flag the legacy Codex projection: {doctor:?}"));
    assert!(line.contains("canon skills install --providers=claude,codex"), "the remnant line names its fix: {line}");
    assert!(!doctor.iter().any(|line| line == "status: healthy"));

    let output = Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(["skills", "check", "--providers", "claude,codex", "--source"])
        .arg(&source)
        .arg("--target")
        .arg(&target)
        .output()
        .expect("spawn canon skills check");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(1), "a legacy remnant fails the read-only check: {stdout}");
    assert!(stdout.contains("legacy-remnant") && stdout.contains("canon skills install --providers=claude,codex"), "{stdout}");

    skills::install_canonical(&source, &target, Some("claude,codex")).unwrap();
    assert!(remnant.is_file(), "an unproven legacy file is reported, never deleted");
}

/// The working loop's step names, in order: the skill's numbered steps
/// (`N. **Name.**`) and the `canon init` AGENTS.md block's (`N. Name:`)
/// must both say exactly these.
const LOOP_STEPS: [&str; 8] =
    ["Brief", "Subject", "Scenarios", "Units", "Implement", "Evidence", "Independent review", "Transition"];

/// skills.install.11: canon's own projected SKILL.md (Claude, Codex, OMP,
/// Pi) claims build work in a canon repo, ahead of generic builder skills,
/// and opens with the working loop the `canon init` AGENTS.md block names.
#[test]
fn skills_install_11_projected_skill_claims_build_work_and_opens_with_the_loop() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = fs::read_to_string(repo.join("canon/skills/SKILL.src.md")).unwrap();
    for root in [".claude", ".agents", ".omp", ".pi"] {
        let projected = fs::read_to_string(repo.join(root).join("skills/canon/SKILL.md")).unwrap();
        assert_eq!(projected, source, "{root}: the projection must be the source byte for byte");
    }

    let fields = frontmatter(&source).expect("SKILL.src.md opens with a closed frontmatter block");
    let description = fields.get("description").expect("a description");
    for claim in ["build", "implement", "feature", "fix", "canon.yaml", "prefer it there over generic site, app or game builder skills"] {
        assert!(description.contains(claim), "the description must claim {claim:?}: {description}");
    }
    assert!(description.chars().count() <= 1024, "skill loaders cap a description at 1024 characters: {}", description.chars().count());

    let body = &source[source.find("\n---\n").unwrap() + 5..];
    let sections: Vec<&str> = body.lines().filter(|line| line.starts_with("## ")).collect();
    assert_eq!(sections.first(), Some(&"## The working loop"), "the loop is the first section: {sections:?}");
    assert!(sections.contains(&"## Command reference"), "the command reference follows the loop: {sections:?}");
    let loop_section = &body[body.find("## The working loop").unwrap()..body.find("## Command reference").unwrap()];
    let skill_steps: Vec<&str> = loop_section
        .lines()
        .filter_map(|line| line.split_once(". **").filter(|(n, _)| n.parse::<u8>().is_ok()))
        .filter_map(|(_, rest)| rest.split_once(".**").map(|(name, _)| name))
        .collect();
    assert_eq!(skill_steps, LOOP_STEPS, "the skill's loop steps");

    let tmp = tempfile::tempdir().unwrap();
    let init = Command::new(env!("CARGO_BIN_EXE_canon")).arg("init").arg("--repo").arg(tmp.path()).output().unwrap();
    assert!(init.status.success(), "{}", String::from_utf8_lossy(&init.stderr));
    let agents = fs::read_to_string(tmp.path().join("AGENTS.md")).unwrap();
    let block_steps: Vec<&str> = agents
        .lines()
        .filter_map(|line| line.split_once(". ").filter(|(n, _)| n.parse::<u8>().is_ok()))
        .filter_map(|(_, rest)| rest.split_once(':').map(|(name, _)| name))
        .collect();
    assert_eq!(block_steps, LOOP_STEPS, "the AGENTS.md block's loop steps");
}

const LOCK: &str = ".canon/skills/.install-lock.json";

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@example.com"]).args(args).current_dir(dir).output().unwrap();
    assert!(output.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn read_lock(target: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(target.join(LOCK)).unwrap()).unwrap()
}

/// Rewrite the lock's `canon_version` (`None` removes the key, as a lock
/// written before 0.16 has none).
fn set_lock_version(target: &Path, version: Option<&str>) {
    let mut lock = read_lock(target);
    let map = lock.as_object_mut().unwrap();
    match version {
        Some(version) => map.insert("canon_version".into(), version.into()),
        None => map.remove("canon_version"),
    };
    fs::write(target.join(LOCK), serde_json::to_string_pretty(&lock).unwrap()).unwrap();
}

fn canon(args: &[&str], source: &Path, target: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).arg("--source").arg(source).arg("--target").arg(target).output().unwrap()
}

/// skills.install.40
#[test]
fn skills_install_40_the_lock_records_the_canon_version_and_the_source_commit() {
    let tmp = tempfile::tempdir().unwrap();
    let checkout = tmp.path().join("checkout");
    let source = checkout.join("canon/skills");
    let target = tmp.path().join("target");
    write_canonical_source(&source);
    git(&checkout, &["init", "-q"]);
    git(&checkout, &["add", "."]);
    git(&checkout, &["commit", "-q", "-m", "skills"]);
    let head = git(&checkout, &["rev-parse", "HEAD"]);

    assert!(skills::install_canonical(&source, &target, Some("claude")).unwrap().changed);
    let lock = read_lock(&target);
    assert_eq!(lock["canon_version"], env!("CARGO_PKG_VERSION"), "{lock}");
    assert_eq!(lock["source_commit"], head.as_str(), "{lock}");
    let bytes = fs::read(target.join(LOCK)).unwrap();

    fs::write(checkout.join("unrelated.txt"), "x\n").unwrap();
    git(&checkout, &["add", "."]);
    git(&checkout, &["commit", "-q", "-m", "unrelated"]);
    assert!(!skills::install_canonical(&source, &target, Some("claude")).unwrap().changed, "a commit that leaves the bundle as it was changes nothing");
    assert_eq!(fs::read(target.join(LOCK)).unwrap(), bytes, "the lock stays byte-identical");
    let check = skills::check(&source, &target, Some("claude")).unwrap();
    assert!(check.manifest_ok && check.outdated.is_none(), "{check:?}");

    let plain = tmp.path().join("plain");
    let plain_target = tmp.path().join("plain-target");
    write_canonical_source(&plain);
    skills::install_canonical(&plain, &plain_target, Some("claude")).unwrap();
    let lock = read_lock(&plain_target);
    assert_eq!(lock["canon_version"], env!("CARGO_PKG_VERSION"), "{lock}");
    assert!(lock.get("source_commit").is_none(), "a source outside any git checkout records no commit: {lock}");
}

/// skills.install.41
#[test]
fn skills_install_41_a_lock_another_canon_wrote_is_reported_with_the_install_fix() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    write_canonical_source(&source);
    skills::install_canonical(&source, &target, Some("claude,codex")).unwrap();
    set_lock_version(&target, Some("0.13.1"));
    let expected = format!(
        "installed by canon 0.13.1, this canon is {} — run `canon skills install --providers=claude,codex`",
        env!("CARGO_PKG_VERSION")
    );

    let check = canon(&["skills", "check", "--providers", "claude,codex"], &source, &target);
    let out = String::from_utf8_lossy(&check.stdout);
    assert_eq!(check.status.code(), Some(1), "{out}");
    assert!(out.contains(&format!("{LOCK} — outdated ({expected})")), "{out}");
    assert!(!out.contains("— stale") && !out.contains("— missing"), "only the lock's version differs: {out}");

    let doctor = canon(&["skills", "doctor", "--providers", "claude,codex"], &source, &target);
    let out = String::from_utf8_lossy(&doctor.stdout);
    assert_eq!(doctor.status.code(), Some(0), "{out}");
    assert!(out.lines().any(|line| line.starts_with("outdated: ") && line.ends_with(&expected)), "{out}");
    assert!(!out.contains("status: healthy"), "{out}");

    assert!(skills::install_canonical(&source, &target, Some("claude,codex")).unwrap().changed, "the install rewrites an outdated lock");
    assert_eq!(read_lock(&target)["canon_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(canon(&["skills", "check", "--providers", "claude,codex"], &source, &target).status.code(), Some(0));
    let doctor = canon(&["skills", "doctor", "--providers", "claude,codex"], &source, &target);
    assert_eq!(String::from_utf8_lossy(&doctor.stdout), "providers: claude,codex\nstatus: healthy\n");
}

/// skills.install.42
#[test]
fn skills_install_42_a_lock_from_before_0_16_is_reported_as_an_unknown_version() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    write_canonical_source(&source);
    skills::install_canonical(&source, &target, Some("claude")).unwrap();
    set_lock_version(&target, None);
    let expected = format!(
        "installed by an unknown canon version (the lock predates 0.16), this canon is {} — run `canon skills install --providers=claude`",
        env!("CARGO_PKG_VERSION")
    );

    let report = skills::check(&source, &target, Some("claude")).unwrap();
    assert!(report.manifest_ok, "the projection itself is current");
    assert_eq!(report.outdated.as_ref().map(|o| o.installed_by.clone()), Some(None));
    let check = canon(&["skills", "check", "--providers", "claude"], &source, &target);
    let out = String::from_utf8_lossy(&check.stdout);
    assert_eq!(check.status.code(), Some(1), "{out}");
    assert!(out.contains(&expected), "{out}");
    let doctor = skills::doctor(&source, &target, Some("claude")).unwrap();
    assert!(doctor.iter().any(|line| line.starts_with("outdated: ") && line.ends_with(&expected)), "{doctor:?}");

    skills::install_canonical(&source, &target, Some("claude")).unwrap();
    assert_eq!(canon(&["skills", "check", "--providers", "claude"], &source, &target).status.code(), Some(0));
}

/// skills.install.43
#[test]
fn skills_install_43_doctor_warns_about_skills_claiming_build_work_and_never_edits_them() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    write_canonical_source(&source);
    skills::install_canonical(&source, &target, Some("claude,codex")).unwrap();
    let sites = target.join(".claude/skills/sites/SKILL.md");
    let notes = target.join(".agents/skills/notes/SKILL.md");
    let sites_text = "---\nname: site-builder\ndescription: Build sites, web apps and games from a prompt\n---\n# Sites\n";
    let notes_text = "---\nname: notes\ndescription: Manage feature branches and take meeting notes\n---\n# Notes\n";
    for (path, text) in [(&sites, sites_text), (&notes, notes_text)] {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    let doctor = canon(&["skills", "doctor", "--providers", "claude,codex"], &source, &target);
    let out = String::from_utf8_lossy(&doctor.stdout);
    assert_eq!(doctor.status.code(), Some(0), "advisory only: {out}");
    assert_eq!(
        out,
        format!(
            "providers: claude,codex\nstatus: healthy\ncompeting-skill: {} (description claims: build, sites, apps, games) — if an agent picks `site-builder` for build work, canon's loop will not run\n",
            target.join(".claude/skills/sites").display()
        )
    );
    assert_eq!(fs::read_to_string(&sites).unwrap(), sites_text);
    assert_eq!(fs::read_to_string(&notes).unwrap(), notes_text);
}

/// canon's own skill source and the `canon init` AGENTS.md block, each
/// with its whitespace collapsed so wrapped lines read as one.
fn skill_and_block() -> [(&'static str, String); 2] {
    let words = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let skill = fs::read_to_string(repo.join("canon/skills/SKILL.src.md")).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let init = Command::new(env!("CARGO_BIN_EXE_canon")).arg("init").arg("--repo").arg(tmp.path()).output().unwrap();
    assert!(init.status.success(), "{}", String::from_utf8_lossy(&init.stderr));
    let block = fs::read_to_string(tmp.path().join("AGENTS.md")).unwrap();
    [("skill", words(&skill)), ("AGENTS.md block", words(&block))]
}

/// skills.install.20 (0.16 D1): the skill and the AGENTS.md block both
/// carry one human approval per scope, recorded by the agent at once in
/// the brief and the policy approval, then no stop inside the scope,
/// independent review on every change, and shipping by the agent.
#[test]
fn skills_install_20_skill_and_block_carry_one_approval_per_scope() {
    for (name, text) in skill_and_block() {
        for rule in [
            "for a new subject or a scope change",
            "Approved: <who> <when> — \"<",
            "canon policy approve --by <the human> --note \"<their words>\"",
            "Never approve without a human answer",
            "Inside an approved scope, never stop for a human",
            "When no human can answer",
            "on every change",
            "yourself once",
            "unless the brief says the human ships",
            "in the approved change",
            "tasks.md",
            "a new change is a new scope",
        ] {
            assert!(text.to_lowercase().contains(&rule.to_lowercase()), "the {name} must say {rule:?}: {text}");
        }
    }
}

/// skills.install.21 (0.16 D1): neither the skill nor the AGENTS.md
/// block keeps a second human stop: no fixed pair of stops, no
/// per-request brief approval, and no rule that the agent may never run
/// the policy approval for the human.
#[test]
fn skills_install_21_skill_and_block_name_no_second_human_stop() {
    for (name, text) in skill_and_block() {
        for stale in ["Two points stop for a human", "a human approves it.", "never approve it yourself", "A human approves the brief"] {
            assert!(!text.contains(stale), "the {name} still says {stale:?}: {text}");
        }
    }
}

/// skills.install.44 (0.16 M7, eval 3b): the skill and the AGENTS.md
/// block stop at the brief: write it, stop and report it, build and
/// attest nothing until the human answers; the approval is never assumed,
/// even when the human cannot answer before the report, since the brief
/// is that report; assumptions cover the brief's questions and work
/// inside an approved scope only; the ship and evidence refusals are
/// backstops. The pre-M7 unscoped assumption rule is gone.
#[test]
fn skills_install_44_skill_and_block_stop_at_the_brief_and_never_assume_the_approval() {
    for (name, text) in skill_and_block() {
        let lower = text.to_lowercase();
        for rule in [
            "then stop and report",
            "nothing for that scope",
            "never assume",
            "even when the human said they cannot answer until you report",
            "the brief is that report",
            "the brief's questions",
            "inside an approved scope, never",
            "canon evidence add",
            "backstops",
        ] {
            assert!(lower.contains(rule), "the {name} must say {rule:?}: {text}");
        }
        assert!(!text.contains("When no human can answer, write"), "the {name} still scopes the assumption rule to nothing: {text}");
    }
}

/// skills.install.22 (dogfood eval 3 H1, H2): the readers refresh the
/// scenario index and `canon init` repos promote on write, so the
/// working loop in the skill and the AGENTS.md block never sends the
/// agent to `canon format specs` or `canon ingest plans`, names
/// `canon inventory sync` only for a `NOT refreshed` line, and names
/// `canon gate promote` only under `write.promote: manual`; the write
/// help lists no promote step of its own.
#[test]
fn skills_install_22_skill_block_and_help_name_no_bookkeeping_step() {
    let [(_, skill), (_, block)] = skill_and_block();
    let loop_start = skill.find("## The working loop").expect("the skill has its loop");
    let loop_end = skill.find("## Command reference").expect("the reference follows the loop");
    for (name, text) in [("skill loop", &skill[loop_start..loop_end]), ("AGENTS.md block", block.as_str())] {
        assert!(!text.contains("canon format specs"), "the {name} names a format step: {text}");
        for sentence in text.split(". ") {
            if sentence.contains("canon ingest plans") {
                assert!(sentence.contains(" no "), "the {name} sends the agent to ingest plans: {sentence:?}");
            }
            if sentence.contains("canon inventory sync") {
                assert!(sentence.contains("NOT refreshed"), "the {name} names a sync step: {sentence:?}");
            }
            if sentence.contains("canon gate promote") {
                assert!(sentence.contains("write.promote: manual"), "the {name} names promote outside manual mode: {sentence:?}");
            }
        }
    }
    for args in [&["evidence", "--help"][..], &["finding", "--help"][..], &["finding", "add", "--help"][..]] {
        let out = Command::new(env!("CARGO_BIN_EXE_canon")).args(args).output().unwrap();
        let help = String::from_utf8_lossy(&out.stdout);
        let listing = &help[help.find("The loop:").unwrap_or_else(|| panic!("{args:?} shows the loop: {help}"))..];
        assert!(!listing.lines().any(|line| line.trim_start().starts_with("canon gate promote ")), "{args:?} lists promote as a step: {listing}");
        assert!(listing.contains("Under `write.promote: manual`"), "{args:?} says when promote is needed: {listing}");
    }
}

/// A `canon.yaml` with comments and other sections around where a
/// `skills:` section would go.
const CONFIG_WITHOUT_SKILLS: &str = "# my config\nspecs:\n  roots:\n    - id: root\n      root: specs\n# trailing note\nwrite:\n  promote: auto\n";

/// skills.install.52: an explicit `--providers` rewrites
/// `skills.providers` and keeps every other byte; without the flag,
/// install, check and doctor follow the list.
#[test]
fn skills_install_52_explicit_providers_rewrite_the_config_list_that_later_runs_follow() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    write_canonical_source(&source);
    fs::create_dir_all(target.join(".claude")).unwrap();
    fs::write(target.join("canon.yaml"), CONFIG_WITHOUT_SKILLS).unwrap();

    let install = canon(&["skills", "install", "--providers", "codex,omp"], &source, &target);
    let out = String::from_utf8_lossy(&install.stdout);
    assert_eq!(install.status.code(), Some(0), "{out}");
    assert!(out.contains("canon v1 — installed (codex,omp)\n"), "{out}");
    assert!(out.contains("wrote `skills: { providers: [codex, omp] }` to "), "{out}");
    let config = fs::read_to_string(target.join("canon.yaml")).unwrap();
    assert!(config.starts_with(CONFIG_WITHOUT_SKILLS), "every byte before the new section is kept: {config}");
    assert!(config.ends_with("skills:\n  providers: [codex, omp]\n"), "{config}");
    assert!(!target.join(".claude/skills/canon").exists(), "the .claude marker no longer decides once the list is set");

    let report = skills::install_canonical(&source, &target, None).unwrap();
    assert_eq!((report.providers.clone(), report.origin), (vec![skills::Provider::Codex, skills::Provider::Omp], skills::ProviderOrigin::Config));
    assert!(!report.changed && report.config_written.is_none());
    let check = canon(&["skills", "check"], &source, &target);
    let out = String::from_utf8_lossy(&check.stdout);
    assert_eq!(check.status.code(), Some(0), "{out}");
    assert!(out.starts_with("providers: codex,omp (from canon.yaml skills.providers)\n"), "{out}");

    // A second rewrite replaces the section in place.
    skills::install_canonical(&source, &target, Some("pi")).unwrap();
    let config = fs::read_to_string(target.join("canon.yaml")).unwrap();
    assert!(config.starts_with(CONFIG_WITHOUT_SKILLS) && config.ends_with("skills:\n  providers: [pi]\n"), "{config}");
    assert_eq!(config.matches("skills:").count(), 1, "{config}");
    let report = skills::check(&source, &target, None).unwrap();
    assert_eq!(report.providers, vec![skills::Provider::Pi]);

    // A section in the middle of the file, in flow style, is replaced there.
    fs::write(target.join("canon.yaml"), "a: 1\nskills: { providers: [claude] }\nb: 2\n").unwrap();
    skills::install_canonical(&source, &target, Some("none")).unwrap();
    assert_eq!(fs::read_to_string(target.join("canon.yaml")).unwrap(), "a: 1\nskills:\n  providers: []\nb: 2\n");
    assert_eq!(skills::check(&source, &target, None).unwrap().providers, vec![]);

    // No canon.yaml: install never creates one.
    let bare = tmp.path().join("bare");
    skills::install_canonical(&source, &bare, Some("claude")).unwrap();
    assert!(!bare.join("canon.yaml").exists());
}

/// skills.install.53: a repo set up before 0.17, with an install lock and
/// no `skills:` key, keeps working from the lock's providers, whatever
/// marker directories exist.
#[test]
fn skills_install_53_a_pre_0_17_repo_with_a_lock_keeps_working() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    write_canonical_source(&source);
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("canon.yaml"), CONFIG_WITHOUT_SKILLS).unwrap();
    // A 0.16 install: Claude plus Codex, projected to .agents.
    skills::install_canonical(&source, &target, Some("claude,codex")).unwrap();
    fs::write(target.join("canon.yaml"), CONFIG_WITHOUT_SKILLS).unwrap();
    fs::create_dir_all(target.join(".omp")).unwrap();
    let lock = fs::read(target.join(LOCK)).unwrap();

    let check = canon(&["skills", "check"], &source, &target);
    let out = String::from_utf8_lossy(&check.stdout);
    assert_eq!(check.status.code(), Some(0), "{out}");
    assert!(out.starts_with("providers: claude,codex (from the install lock; canon.yaml has no skills: key)\n"), "{out}");
    let doctor = canon(&["skills", "doctor"], &source, &target);
    assert_eq!(
        String::from_utf8_lossy(&doctor.stdout),
        "providers: claude,codex (from the install lock; canon.yaml has no skills: key)\nstatus: healthy\n"
    );
    let install = canon(&["skills", "install"], &source, &target);
    assert_eq!(
        String::from_utf8_lossy(&install.stdout),
        "canon v1 — unchanged (claude,codex; from the install lock; canon.yaml has no skills: key)\n"
    );
    assert_eq!(fs::read(target.join(LOCK)).unwrap(), lock, "the lock is untouched");
    assert_eq!(fs::read_to_string(target.join("canon.yaml")).unwrap(), CONFIG_WITHOUT_SKILLS, "only an explicit --providers writes the list");
    assert!(!target.join(".omp/skills").exists(), "a marker never widens the lock's providers");
}

/// skills.install.54: doctor names a canon projection left for a provider
/// that is not selected, with how to remove or keep it; check passes.
#[test]
fn skills_install_54_doctor_names_a_stray_projection_and_check_does_not_fail_on_it() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    write_canonical_source(&source);
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("canon.yaml"), CONFIG_WITHOUT_SKILLS).unwrap();
    skills::install_canonical(&source, &target, Some("claude,codex")).unwrap();
    skills::install_canonical(&source, &target, Some("claude")).unwrap();
    let stray = target.join(".agents/skills/canon");
    assert!(stray.join("SKILL.md").is_file(), "install never deletes a projection");

    let check = canon(&["skills", "check"], &source, &target);
    let out = String::from_utf8_lossy(&check.stdout);
    assert_eq!(check.status.code(), Some(0), "a stray projection is not drift: {out}");
    assert!(!out.contains(".agents"), "{out}");

    let doctor = canon(&["skills", "doctor"], &source, &target);
    let out = String::from_utf8_lossy(&doctor.stdout);
    assert_eq!(doctor.status.code(), Some(0), "{out}");
    let path = stray.display();
    assert_eq!(
        out,
        format!(
            "providers: claude (from canon.yaml skills.providers)\nstatus: healthy\nstray: {path} — codex is not a selected provider (from canon.yaml skills.providers), so canon no longer updates it; remove it with `rm -r {path}`, or select it with `canon skills install --providers=claude,codex`\n"
        )
    );
    assert!(stray.join("SKILL.md").is_file(), "doctor never deletes");
    let doctor = skills::doctor(&source, &target, None).unwrap();
    assert!(!doctor.iter().any(|line| line.contains(".claude/skills/canon ")), "a selected provider is never a stray: {doctor:?}");
}

/// skills.install.55: a malformed `skills:` section, or one naming an
/// unknown provider, fails install, check and doctor loud, writes
/// nothing, and fails `canon init --check-config`.
#[test]
fn skills_install_55_a_malformed_skills_section_fails_loud() {
    for (section, needle) in [
        ("skills: [claude]\n", "`skills:` section is malformed (expected `skills: { providers: [claude, codex, omp, pi] }`"),
        ("skills:\n  provider: [claude]\n", "`skills:` section is malformed"),
        ("skills:\n  providers: [claude, gemini]\n", "`skills.providers` names `gemini`; expected any of claude, codex, omp, pi"),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        let target = tmp.path().join("target");
        write_canonical_source(&source);
        fs::create_dir_all(target.join(".claude")).unwrap();
        let config = format!("{CONFIG_WITHOUT_SKILLS}{section}");
        fs::write(target.join("canon.yaml"), &config).unwrap();

        for command in ["install", "check", "doctor"] {
            let output = canon(&["skills", command], &source, &target);
            let err = String::from_utf8_lossy(&output.stderr);
            assert_eq!(output.status.code(), Some(1), "{command} {section:?}: {err}");
            assert!(err.contains(needle), "{command} {section:?}: {err}");
        }
        assert!(!target.join(".claude/skills").exists() && !target.join(".canon").exists(), "{section:?}: nothing is written");
        assert_eq!(fs::read_to_string(target.join("canon.yaml")).unwrap(), config);

        let check = Command::new(env!("CARGO_BIN_EXE_canon")).args(["init", "--check-config", "--repo"]).arg(&target).output().unwrap();
        let out = String::from_utf8_lossy(&check.stdout);
        assert_eq!(check.status.code(), Some(1), "{out}");
        assert!(out.lines().any(|line| line.starts_with("[FAIL] skills: ") && line.contains(needle)), "{out}");
    }
}
