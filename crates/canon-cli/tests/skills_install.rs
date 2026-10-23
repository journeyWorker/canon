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

/// A projected `SKILL.md` for `source`: the source's frontmatter (so harnesses
/// still route to the skill), and a stub body that points at the guide the
/// binary carries and names the binary's version.
fn assert_stub(projected: &str, source: &str) {
    let expected = frontmatter(source).expect("the source opens with a closed frontmatter block");
    assert_eq!(frontmatter(projected).as_ref(), Some(&expected), "the stub keeps the source's frontmatter");
    assert!(projected.contains("canon skills show canon"), "the stub points at the guide in the binary: {projected}");
    assert!(projected.contains(env!("CARGO_PKG_VERSION")), "the stub carries the binary's version: {projected}");
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

    let report = skills::install(&source, target, false).expect("install should succeed");
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

    let first = skills::install(&source, target, false).expect("first install should succeed");
    let claude_after_first = fs::read_to_string(target.join(".claude/skills/example-skill/SKILL.md")).unwrap();
    let codex_after_first = fs::read_to_string(target.join(".agents/skills/example-skill/SKILL.md")).unwrap();
    let lock_after_first = fs::read_to_string(source.join(".install-lock.json")).unwrap();

    let second = skills::install(&source, target, false).expect("second install should succeed");
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

    let first = skills::install(&source, target, false).expect("first install should succeed");
    assert_eq!(first.installed[0].version, 1);

    // Mutate the skill's source content.
    let skill_md = source.join("example-skill/SKILL.md");
    let mut content = fs::read_to_string(&skill_md).unwrap();
    content.push_str("\nAppended content to force a hash change.\n");
    fs::write(&skill_md, content).unwrap();

    let second = skills::install(&source, target, false).expect("second install should succeed");
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

    let first = skills::install_canonical(Some(&source), &target, Some("codex"), false).unwrap();
    assert!(first.changed);
    assert!(!target.join(".claude/skills/canon").exists());
    let bundle = target.join(".agents/skills/canon");
    let entrypoint = fs::read_to_string(bundle.join("SKILL.md")).unwrap();
    assert_stub(&entrypoint, source_skill);
    let fields = frontmatter(&entrypoint).expect("the Codex SKILL.md must open with a closed frontmatter block");
    assert_eq!(fields.get("name").map(String::as_str), Some("canon"));
    assert!(fields.get("description").is_some_and(|description| !description.is_empty()), "Codex requires a description: {fields:?}");
    assert!(!bundle.join("reference").exists(), "a source's reference files are never projected: the guide comes from the binary");
    assert_eq!(fs::read_to_string(bundle.join("scripts/hook.sh")).unwrap(), "#!/bin/sh\n");
    assert!(!target.join(".codex").exists(), "nothing is written under .codex");
    assert!(!source.join(".install-lock.json").exists());

    let second = skills::install_canonical(Some(&source), &target, Some("codex"), false).unwrap();
    assert!(!second.changed);
    let check = skills::check(Some(&source), &target, Some("codex")).unwrap();
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

        let report = skills::install_canonical(Some(&source), &target, None, false).unwrap();
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

    let report = skills::install_canonical(Some(&source), &target, None, false).unwrap();
    assert_eq!(report.providers, vec![]);
    assert_eq!(report.origin, skills::ProviderOrigin::Detected);
    assert!(!report.changed);
    assert_eq!(fs::read_dir(&target).unwrap().count(), 0, "nothing at all is written");

    let install = canon(&["skills", "install"], &source, &target);
    let out = String::from_utf8_lossy(&install.stdout);
    assert_eq!(install.status.code(), Some(0), "{out}");
    assert!(out.contains("--providers=<list>"), "the output names the flag: {out}");
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
    skills::install_canonical(Some(&source), &target, Some("claude,codex"), false).unwrap();

    let projected = target.join(".claude/skills/canon/SKILL.md");
    fs::write(&projected, "mutated projection\n").unwrap();
    let legacy = target.join(".claude/skills/canon-old");
    fs::create_dir_all(&legacy).unwrap();

    let check = skills::check(Some(&source), &target, None).unwrap();
    assert!(check.statuses.iter().any(|status| status.state != "ok"));

    let doctor = skills::doctor(Some(&source), &target, None).unwrap();
    assert!(doctor.iter().any(|line| line.contains("stale:") || line.contains("missing:")));
    assert!(doctor.iter().any(|line| line.contains("legacy-remnant:") && line.contains("canon-old")));
    assert!(legacy.exists(), "doctor must report legacy remnants without deleting them");
}

#[test]
fn canonical_install_projects_omp_and_pi_stubs_with_sidecars() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    fs::create_dir_all(source.join("reference")).unwrap();
    fs::create_dir_all(source.join("scripts")).unwrap();
    let source_skill = "---\nname: canon\ndescription: umbrella\nx-extra: preserved\n---\n\n# Canon\n";
    fs::write(source.join("SKILL.src.md"), source_skill).unwrap();
    fs::write(source.join("reference/topic.md"), "# topic\n").unwrap();
    fs::write(source.join("scripts/pre-dispatch.sh"), "#!/bin/sh\n").unwrap();

    let report = skills::install_canonical(Some(&source), &target, Some("pi,OMP"), false).unwrap();

    assert_eq!(report.providers, vec![skills::Provider::Omp, skills::Provider::Pi]);
    for root in [".omp", ".pi"] {
        let bundle = target.join(root).join("skills/canon");
        assert_stub(&fs::read_to_string(bundle.join("SKILL.md")).unwrap(), source_skill);
        assert!(!bundle.join("reference").exists(), "{root}: a source's reference files are never projected");
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

        let report = skills::install_canonical(Some(&source), &target, None, false).unwrap();
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
    skills::install_canonical(Some(&source), &target, Some("omp,pi"), false).unwrap();

    let projected = target.join(".omp/skills/canon/SKILL.md");
    fs::write(&projected, "mutated projection\n").unwrap();
    let legacy = target.join(".pi/skills/canon-old");
    fs::create_dir_all(&legacy).unwrap();

    let check = skills::check(Some(&source), &target, Some("omp,pi")).unwrap();
    assert!(check.providers == vec![skills::Provider::Omp, skills::Provider::Pi]);
    assert!(check
        .statuses
        .iter()
        .any(|status| status.provider == skills::Provider::Omp && status.state == "stale"));

    let doctor = skills::doctor(Some(&source), &target, Some("omp,pi")).unwrap();
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

    let first = skills::install_canonical(Some(&source), &target, None, false).unwrap();
    assert_eq!(first.providers, vec![skills::Provider::Codex], "the legacy lock's providers");
    assert!(first.changed);
    assert!(target.join(".agents/skills/canon/SKILL.md").is_file());
    for relative in &legacy {
        assert!(!target.join(relative).exists(), "the lock-proven legacy file {relative} must be removed");
    }
    assert!(!target.join(".codex/skills/canon").exists(), "the emptied legacy bundle directory is removed");
    assert_eq!(fs::read_to_string(&user_file).unwrap(), "# my own codex note\n", "a user file under .codex/skills is kept");

    let lock_after_first = fs::read(target.join(".canon/skills/.install-lock.json")).unwrap();
    let second = skills::install_canonical(Some(&source), &target, None, false).unwrap();
    assert!(!second.changed, "a rerun after migration changes nothing");
    assert_eq!(fs::read(target.join(".canon/skills/.install-lock.json")).unwrap(), lock_after_first);
    assert!(user_file.is_file());
    let check = skills::check(Some(&source), &target, None).unwrap();
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

    skills::install_canonical(Some(&source), &target, Some("claude,codex"), false).unwrap();

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

    skills::install_canonical(Some(&source), &target, Some("codex"), false).unwrap();

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

        skills::install_canonical(Some(&source), &target, Some("codex"), false).unwrap();

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
    skills::install_canonical(Some(&source), &target, Some("claude,codex"), false).unwrap();
    // A legacy projection no install lock records (e.g. a lock that was
    // never committed): install cannot prove canon wrote it.
    let remnant = target.join(".codex/skills/canon.md");
    fs::create_dir_all(remnant.parent().unwrap()).unwrap();
    fs::write(&remnant, "# canon\n\n> umbrella\n\n# Canon\n").unwrap();

    let check = skills::check(Some(&source), &target, Some("claude,codex")).unwrap();
    assert_eq!(check.remnants, vec![remnant.clone()]);
    assert!(check.statuses.iter().all(|status| status.state == "ok"), "the remnant is not projection drift");

    let doctor = skills::doctor(Some(&source), &target, Some("claude,codex")).unwrap();
    let line = doctor
        .iter()
        .find(|line| line.starts_with("legacy-remnant:") && line.contains(".codex/skills/canon.md"))
        .unwrap_or_else(|| panic!("doctor must flag the legacy Codex projection: {doctor:?}"));
    assert!(line.contains("canon skills install --providers=claude,codex"), "the remnant line names its fix: {line}");
    assert!(!doctor.iter().any(|line| line == "status: healthy"));

    let output = Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(["skills", "check", "--providers", "claude,codex", "--skills-source"])
        .arg(&source)
        .arg("--repo")
        .arg(&target)
        .output()
        .expect("spawn canon skills check");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(1), "a legacy remnant fails the read-only check: {stdout}");
    assert!(stdout.contains("legacy-remnant") && stdout.contains("canon skills install --providers=claude,codex"), "{stdout}");

    skills::install_canonical(Some(&source), &target, Some("claude,codex"), false).unwrap();
    assert!(remnant.is_file(), "an unproven legacy file is reported, never deleted");
}

/// The working loop's step names, in order: the skill's numbered steps
/// (`N. **Name.**`) and the `canon init` AGENTS.md block's (`N. Name:`)
/// must both say exactly these.
const LOOP_STEPS: [&str; 8] =
    ["Brief", "Subject", "Scenarios", "Units", "Implement", "Evidence", "Independent review", "Transition"];

/// The text of `canon skills show <args>`: the guide the binary carries.
fn show(args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_canon")).args(["skills", "show"]).args(args).output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

/// skills.install.11: canon's own skill claims build work in a canon repo,
/// ahead of generic builder skills: the stub every provider gets carries the
/// source's frontmatter, and the guide it points to opens with the working
/// loop the `canon init` AGENTS.md block names.
#[test]
fn skills_install_11_the_stub_claims_build_work_and_the_guide_opens_with_the_loop() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = fs::read_to_string(repo.join("canon/skills/SKILL.src.md")).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("target");
    let report = skills::install_canonical(None, &target, Some("claude,codex,omp,pi"), false).unwrap();
    assert_eq!(report.providers.len(), 4);
    for root in [".claude", ".agents", ".omp", ".pi"] {
        let projected = fs::read_to_string(target.join(root).join("skills/canon/SKILL.md")).unwrap();
        assert_stub(&projected, &source);
        assert!(!target.join(root).join("skills/canon/reference").exists(), "{root}: no reference files are projected");
    }

    let fields = frontmatter(&source).expect("SKILL.src.md opens with a closed frontmatter block");
    let description = fields.get("description").expect("a description");
    for claim in ["build", "implement", "feature", "fix", "canon.yaml"] {
        assert!(description.contains(claim), "the description must claim {claim:?}: {description}");
    }
    assert!(description.chars().count() <= 1024, "skill loaders cap a description at 1024 characters: {}", description.chars().count());

    let guide = show(&["canon"]);
    let body = &guide[guide.find("\n---\n").unwrap() + 5..];
    let sections: Vec<&str> = body.lines().filter(|line| line.starts_with("## ")).collect();
    assert_eq!(sections.first(), Some(&"## The working loop"), "the loop is the first section: {sections:?}");
    let loop_start = body.find("## The working loop").unwrap();
    let loop_end = body[loop_start + 3..].find("\n## ").map_or(body.len(), |at| loop_start + 3 + at);
    let loop_section = &body[loop_start..loop_end];
    let skill_steps: Vec<&str> = loop_section
        .lines()
        .filter_map(|line| line.split_once(". **").filter(|(n, _)| n.parse::<u8>().is_ok()))
        .filter_map(|(_, rest)| rest.split_once(".**").map(|(name, _)| name))
        .collect();
    assert_eq!(skill_steps, LOOP_STEPS, "the guide's loop steps");

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
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).arg("--skills-source").arg(source).arg("--repo").arg(target).output().unwrap()
}

/// skills.install.55: a `--skills-source` directory that does not exist is an invalid argument too: exit 2 from install, check and doctor, nothing written.
#[test]
fn skills_install_55_a_missing_skills_source_is_refused_with_exit_two() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("target");
    fs::create_dir_all(target.join(".claude")).unwrap();
    fs::write(target.join("canon.yaml"), CONFIG_WITHOUT_SKILLS).unwrap();
    let missing = tmp.path().join("no-such-source");
    for command in ["install", "check", "doctor"] {
        let output = canon(&["skills", command, "--providers", "claude", "--json"], &missing, &target);
        assert_eq!(output.status.code(), Some(2), "{command}: {}", String::from_utf8_lossy(&output.stdout));
        let doc: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(doc["error"]["code"], "SKILLS_INVALID_ARGUMENT", "{command}: {doc}");
        assert_eq!(doc["error"]["field"], "--skills-source", "{command}: {doc}");
    }
    assert!(!target.join(".claude/skills").exists() && !target.join(".canon").exists(), "nothing is written");
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

    assert!(skills::install_canonical(Some(&source), &target, Some("claude"), false).unwrap().changed);
    let lock = read_lock(&target);
    assert_eq!(lock["canon_version"], env!("CARGO_PKG_VERSION"), "{lock}");
    assert_eq!(lock["source_commit"], head.as_str(), "{lock}");
    let bytes = fs::read(target.join(LOCK)).unwrap();

    fs::write(checkout.join("unrelated.txt"), "x\n").unwrap();
    git(&checkout, &["add", "."]);
    git(&checkout, &["commit", "-q", "-m", "unrelated"]);
    assert!(!skills::install_canonical(Some(&source), &target, Some("claude"), false).unwrap().changed, "a commit that leaves the bundle as it was changes nothing");
    assert_eq!(fs::read(target.join(LOCK)).unwrap(), bytes, "the lock stays byte-identical");
    let check = skills::check(Some(&source), &target, Some("claude")).unwrap();
    assert!(check.manifest_ok && check.outdated.is_none(), "{check:?}");

    let plain = tmp.path().join("plain");
    let plain_target = tmp.path().join("plain-target");
    write_canonical_source(&plain);
    skills::install_canonical(Some(&plain), &plain_target, Some("claude"), false).unwrap();
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
    skills::install_canonical(Some(&source), &target, Some("claude,codex"), false).unwrap();
    set_lock_version(&target, Some("0.13.1"));
    let fix = "canon skills install --providers=claude,codex";

    let check = canon(&["skills", "check", "--providers", "claude,codex"], &source, &target);
    let out = String::from_utf8_lossy(&check.stdout);
    assert_eq!(check.status.code(), Some(1), "{out}");
    assert!(out.contains(LOCK) && out.contains("outdated") && out.contains("0.13.1") && out.contains(fix), "{out}");
    assert!(!out.contains("— stale") && !out.contains("— missing"), "only the lock's version differs: {out}");

    let doctor = canon(&["skills", "doctor", "--providers", "claude,codex"], &source, &target);
    let out = String::from_utf8_lossy(&doctor.stdout);
    assert_eq!(doctor.status.code(), Some(0), "{out}");
    assert!(out.lines().any(|line| line.starts_with("outdated: ") && line.contains("0.13.1") && line.contains(fix)), "{out}");
    assert!(!out.contains("status: healthy"), "{out}");

    assert!(skills::install_canonical(Some(&source), &target, Some("claude,codex"), false).unwrap().changed, "the install rewrites an outdated lock");
    assert_eq!(read_lock(&target)["canon_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(canon(&["skills", "check", "--providers", "claude,codex"], &source, &target).status.code(), Some(0));
    let doctor = canon(&["skills", "doctor", "--providers", "claude,codex"], &source, &target);
    let out = String::from_utf8_lossy(&doctor.stdout);
    assert_eq!(doctor.status.code(), Some(0), "{out}");
    assert!(out.starts_with("providers: claude,codex") && out.contains("status: healthy"), "{out}");
    assert_owned(&out, &[skills::Provider::Claude, skills::Provider::Codex]);
}

/// skills.install.42
#[test]
fn skills_install_42_a_lock_from_before_0_16_is_reported_as_an_unknown_version() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    write_canonical_source(&source);
    skills::install_canonical(Some(&source), &target, Some("claude"), false).unwrap();
    set_lock_version(&target, None);
    let expected = format!(
        "installed by an unknown canon version (the lock predates 0.16), this canon is {} — run `canon skills install --providers=claude`",
        env!("CARGO_PKG_VERSION")
    );

    let report = skills::check(Some(&source), &target, Some("claude")).unwrap();
    assert!(report.manifest_ok, "the projection itself is current");
    assert_eq!(report.outdated.as_ref().map(|o| o.installed_by.clone()), Some(None));
    let check = canon(&["skills", "check", "--providers", "claude"], &source, &target);
    let out = String::from_utf8_lossy(&check.stdout);
    assert_eq!(check.status.code(), Some(1), "{out}");
    assert!(out.contains(&expected), "{out}");
    let doctor = skills::doctor(Some(&source), &target, Some("claude")).unwrap();
    assert!(doctor.iter().any(|line| line.starts_with("outdated: ") && line.ends_with(&expected)), "{doctor:?}");

    skills::install_canonical(Some(&source), &target, Some("claude"), false).unwrap();
    assert_eq!(canon(&["skills", "check", "--providers", "claude"], &source, &target).status.code(), Some(0));
}

/// skills.install.43
#[test]
fn skills_install_43_doctor_warns_about_skills_claiming_build_work_and_never_edits_them() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    write_canonical_source(&source);
    skills::install_canonical(Some(&source), &target, Some("claude,codex"), false).unwrap();
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
    assert!(out.contains("status: healthy"), "{out}");
    let competing: Vec<&str> = out.lines().filter(|line| line.starts_with("competing-skill: ")).collect();
    assert_eq!(competing.len(), 1, "only the skill claiming build work is named: {out}");
    assert!(competing[0].contains(&target.join(".claude/skills/sites").display().to_string()), "{out}");
    assert_owned(&out, &[skills::Provider::Claude, skills::Provider::Codex]);
    assert_eq!(fs::read_to_string(&sites).unwrap(), sites_text);
    assert_eq!(fs::read_to_string(&notes).unwrap(), notes_text);
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

    let report = skills::install_canonical(Some(&source), &target, None, false).unwrap();
    assert_eq!((report.providers.clone(), report.origin), (vec![skills::Provider::Codex, skills::Provider::Omp], skills::ProviderOrigin::Config));
    assert!(!report.changed && report.config_written.is_none());
    let check = canon(&["skills", "check"], &source, &target);
    let out = String::from_utf8_lossy(&check.stdout);
    assert_eq!(check.status.code(), Some(0), "{out}");
    assert!(out.starts_with("providers: codex,omp (from canon.yaml skills.providers)\n"), "{out}");

    // A second rewrite replaces the section in place.
    skills::install_canonical(Some(&source), &target, Some("pi"), false).unwrap();
    let config = fs::read_to_string(target.join("canon.yaml")).unwrap();
    assert!(config.starts_with(CONFIG_WITHOUT_SKILLS) && config.ends_with("skills:\n  providers: [pi]\n"), "{config}");
    assert_eq!(config.matches("skills:").count(), 1, "{config}");
    let report = skills::check(Some(&source), &target, None).unwrap();
    assert_eq!(report.providers, vec![skills::Provider::Pi]);

    // A section in the middle of the file, in flow style, is replaced there.
    fs::write(target.join("canon.yaml"), "a: 1\nskills: { providers: [claude] }\nb: 2\n").unwrap();
    skills::install_canonical(Some(&source), &target, Some("none"), false).unwrap();
    assert_eq!(fs::read_to_string(target.join("canon.yaml")).unwrap(), "a: 1\nskills:\n  providers: []\nb: 2\n");
    assert_eq!(skills::check(Some(&source), &target, None).unwrap().providers, vec![]);

    // A CRLF canon.yaml stays CRLF: appended, then replaced in place.
    let crlf = CONFIG_WITHOUT_SKILLS.replace('\n', "\r\n");
    fs::write(target.join("canon.yaml"), &crlf).unwrap();
    let rest = |text: &str| {
        let mut doc: serde_yaml::Value = serde_yaml::from_str(text).unwrap();
        doc.as_mapping_mut().unwrap().remove("skills");
        doc
    };
    for (list, expected) in [("codex,pi", vec![skills::Provider::Codex, skills::Provider::Pi]), ("omp", vec![skills::Provider::Omp])] {
        skills::install_canonical(Some(&source), &target, Some(list), false).unwrap();
        let config = fs::read_to_string(target.join("canon.yaml")).unwrap();
        assert!(config.starts_with(&crlf), "{list}: every byte before the section is kept: {config:?}");
        assert_eq!(config.matches('\n').count(), config.matches("\r\n").count(), "{list}: every line ends CRLF: {config:?}");
        assert_eq!(config.matches("skills:").count(), 1, "{list}: {config:?}");
        assert_eq!(rest(&config), rest(&crlf), "{list}: every other key is unchanged");
        assert_eq!(skills::check(Some(&source), &target, None).unwrap().providers, expected, "{list}: the providers parse back");
    }
    assert!(fs::read_to_string(target.join("canon.yaml")).unwrap().ends_with("skills:\r\n  providers: [omp]\r\n"));

    // No canon.yaml: install never creates one.
    let bare = tmp.path().join("bare");
    skills::install_canonical(Some(&source), &bare, Some("claude"), false).unwrap();
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
    skills::install_canonical(Some(&source), &target, Some("claude,codex"), false).unwrap();
    fs::write(target.join("canon.yaml"), CONFIG_WITHOUT_SKILLS).unwrap();
    fs::create_dir_all(target.join(".omp")).unwrap();
    let lock = fs::read(target.join(LOCK)).unwrap();

    let check = canon(&["skills", "check"], &source, &target);
    let out = String::from_utf8_lossy(&check.stdout);
    assert_eq!(check.status.code(), Some(0), "{out}");
    assert!(out.starts_with("providers: claude,codex (from the install lock; canon.yaml has no skills: key)\n"), "{out}");
    let doctor = canon(&["skills", "doctor"], &source, &target);
    let out = String::from_utf8_lossy(&doctor.stdout);
    assert_eq!(doctor.status.code(), Some(0), "{out}");
    assert!(out.starts_with("providers: claude,codex (from the install lock; canon.yaml has no skills: key)\n"), "{out}");
    assert!(out.contains("status: healthy"), "{out}");
    assert_owned(&out, &[skills::Provider::Claude, skills::Provider::Codex]);
    let install = canon(&["skills", "install"], &source, &target);
    let out = String::from_utf8_lossy(&install.stdout);
    assert_eq!(install.status.code(), Some(0), "{out}");
    assert_owned(&out, &[skills::Provider::Claude, skills::Provider::Codex]);
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
    skills::install_canonical(Some(&source), &target, Some("claude,codex"), false).unwrap();
    skills::install_canonical(Some(&source), &target, Some("claude"), false).unwrap();
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
    let line = out.lines().find(|line| line.starts_with(&format!("stray: {path}"))).unwrap_or_else(|| panic!("doctor names the stray projection: {out}"));
    assert!(line.contains(&format!("rm -r {path}")) && line.contains("canon skills install --providers=claude,codex"), "the stray line names both ways out: {line}");
    assert!(out.contains("status: healthy"), "{out}");
    assert_owned(&out, &[skills::Provider::Claude]);
    assert!(!out.lines().any(|line| line.trim() == ".agents/skills/canon/**"), "an unselected provider's projection is not an owned path: {out}");
    assert!(stray.join("SKILL.md").is_file(), "doctor never deletes");
    let doctor = skills::doctor(Some(&source), &target, None).unwrap();
    assert!(!doctor.iter().any(|line| line.contains(".claude/skills/canon ")), "a selected provider is never a stray: {doctor:?}");
}

/// skills.install.55: a malformed `skills:` section, or one naming an
/// unknown provider, is refused (exit 2) by install, check and doctor,
/// writes nothing, and fails `canon init --check`.
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
            assert_eq!(output.status.code(), Some(2), "{command} {section:?}: {err}");
            assert!(err.contains("SKILLS_INVALID_ARGUMENT"), "{command} {section:?}: {err}");
            assert!(err.contains(needle), "{command} {section:?}: {err}");
        }
        for command in ["install", "check", "doctor"] {
            let output = canon(&["skills", command, "--providers", "gemini"], &source, &target);
            assert_eq!(output.status.code(), Some(2), "{command} --providers gemini: {}", String::from_utf8_lossy(&output.stderr));
        }
        assert!(!target.join(".claude/skills").exists() && !target.join(".canon").exists(), "{section:?}: nothing is written");
        assert_eq!(fs::read_to_string(target.join("canon.yaml")).unwrap(), config);

        let check = Command::new(env!("CARGO_BIN_EXE_canon")).args(["init", "--check", "--repo"]).arg(&target).output().unwrap();
        let out = String::from_utf8_lossy(&check.stdout);
        assert_eq!(check.status.code(), Some(1), "{out}");
        assert!(out.lines().any(|line| line.starts_with("[FAIL] skills: ") && line.contains(needle)), "{out}");
    }
}

/// The owned-paths block install and doctor end with: the globs of the
/// `providers`' projections, then every other path canon writes (backups of
/// a migration included).
fn assert_owned(out: &str, providers: &[skills::Provider]) {
    assert!(out.lines().any(|line| line.starts_with("owned-paths:")), "{out}");
    let globs = skills::owned_path_globs(providers);
    assert!(globs.iter().any(|glob| glob == ".canon/migrate-backup/**"), "{globs:?}");
    for glob in globs {
        assert!(out.lines().any(|line| line.trim() == glob), "{glob} is listed: {out}");
    }
}

/// skills.install.56: install and doctor print the globs of every path
/// canon writes, the selected providers' projections first.
#[test]
fn skills_install_56_install_and_doctor_print_the_owned_path_globs() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    write_canonical_source(&source);
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("canon.yaml"), CONFIG_WITHOUT_SKILLS).unwrap();
    let expect_block = |out: &str| {
        let globs: Vec<&str> = out.lines().skip_while(|line| !line.starts_with("owned-paths:")).skip(1).map(str::trim).collect();
        assert_eq!(globs[..2], [".claude/skills/canon/**", ".agents/skills/canon/**"], "the selected providers' projections come first, in provider order: {out}");
        let rest = &globs[2..];
        let mut sorted = rest.to_vec();
        sorted.sort();
        assert_eq!(rest, sorted.as_slice(), "every other path is sorted: {out}");
        for named in [".canon/ledger/**", ".canon/artifacts/**", ".canon/migrate-backup/**"] {
            assert!(rest.contains(&named), "{named} is listed: {out}");
        }
    };

    let install = canon(&["skills", "install", "--providers", "codex,claude"], &source, &target);
    let out = String::from_utf8_lossy(&install.stdout);
    assert_eq!(install.status.code(), Some(0), "{out}");
    expect_block(&out);
    assert!(out.contains("claude,codex"), "{out}");

    let rerun = canon(&["skills", "install"], &source, &target);
    let out = String::from_utf8_lossy(&rerun.stdout);
    expect_block(&out);

    let doctor = canon(&["skills", "doctor"], &source, &target);
    let out = String::from_utf8_lossy(&doctor.stdout);
    assert_eq!(doctor.status.code(), Some(0), "{out}");
    assert!(out.starts_with("providers: claude,codex"), "{out}");
    assert!(out.contains("status: healthy"), "{out}");
    expect_block(&out);

    let none = skills::owned_path_globs(&[]);
    assert!(!none.iter().any(|glob| glob.contains("/skills/canon/")), "no provider, no projection glob: {none:?}");
    for authored in [".canon/changes", ".canon/specs", ".canon/policy.yaml"] {
        assert!(!none.iter().any(|glob| glob.starts_with(authored)), "{authored} is written by hand and stays linted: {none:?}");
    }
}

/// skills.install.57: a projected file a formatter rewrote — same
/// content, different layout — is reported as formatter-rewritten with
/// the ignore entry to add, not as drift.
#[test]
fn skills_install_57_a_formatter_rewrite_is_named_with_the_ignore_entry() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    write_canonical_source(&source);
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("canon.yaml"), CONFIG_WITHOUT_SKILLS).unwrap();
    skills::install_canonical(Some(&source), &target, Some("claude"), false).unwrap();
    let skill = target.join(".claude/skills/canon/SKILL.md");
    let stub = fs::read_to_string(&skill).unwrap();
    // What a prettier-family formatter does to a markdown file: CRLF line
    // ends, trailing blanks, extra blank lines at the end.
    let rewritten: String = stub.lines().map(|line| format!("{line}  \r\n")).collect::<String>() + "\r\n\r\n";
    fs::write(&skill, rewritten).unwrap();

    let check = canon(&["skills", "check"], &source, &target);
    let out = String::from_utf8_lossy(&check.stdout);
    assert_eq!(check.status.code(), Some(1), "a rewritten projection still fails check: {out}");
    assert!(out.contains(&format!("{} — formatter-rewritten (claude)", skill.display())), "{out}");

    let doctor = canon(&["skills", "doctor"], &source, &target);
    let out = String::from_utf8_lossy(&doctor.stdout);
    assert_eq!(doctor.status.code(), Some(0), "{out}");
    let line = out.lines().find(|line| line.starts_with(&format!("formatter-rewritten: {} (claude)", skill.display()))).unwrap_or_else(|| panic!("{out}"));
    assert!(line.contains(".claude/skills/canon/**") && line.contains("canon skills install --providers=claude"), "the line names the ignore entry and the restore: {line}");
    assert!(!out.contains("stale: "), "a rewrite is not drift: {out}");
    assert!(!out.contains("status: healthy"), "{out}");
    assert!(out.contains("owned-paths: "), "{out}");
}

/// skills.install.58: a projected file whose content changed is still
/// drift, never called a formatter rewrite.
#[test]
fn skills_install_58_changed_content_stays_drift() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    write_canonical_source(&source);
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("canon.yaml"), CONFIG_WITHOUT_SKILLS).unwrap();
    fs::write(source.join("scripts/hook.sh"), "#!/bin/sh\nrm -rf a b\nexec canon \\\n  status\n").unwrap();
    skills::install_canonical(Some(&source), &target, Some("claude,codex"), false).unwrap();
    let hook = target.join(".claude/skills/canon/scripts/hook.sh");
    let skill = target.join(".claude/skills/canon/SKILL.md");
    let codex_skill = target.join(".agents/skills/canon/SKILL.md");
    let stub = fs::read_to_string(&skill).unwrap();
    let spaced = stub.replacen("`canon ", "`canon  ", 1);
    assert_ne!(spaced, stub, "the stub has an inline command to change");
    // Each would read as a rewrite if inner whitespace or backslashes were
    // normalized away: a joined argument, a joined continuation, a changed
    // inline command, and a new paragraph.
    fs::write(&hook, "#!/bin/sh\nrm -rfab\nexec canon status\n").unwrap();
    fs::write(&skill, format!("{stub}\nA paragraph canon never wrote.\n")).unwrap();
    fs::write(&codex_skill, spaced).unwrap();
    let changed = [(&hook, "claude"), (&skill, "claude"), (&codex_skill, "codex")];

    let check = canon(&["skills", "check"], &source, &target);
    let out = String::from_utf8_lossy(&check.stdout);
    assert_eq!(check.status.code(), Some(1), "{out}");
    for (path, provider) in changed {
        assert!(out.contains(&format!("{} — stale ({provider})", path.display())), "{out}");
    }

    let doctor = canon(&["skills", "doctor"], &source, &target);
    let out = String::from_utf8_lossy(&doctor.stdout);
    for (path, provider) in changed {
        assert!(out.contains(&format!("stale: {} ({provider})\n", path.display())), "{out}");
    }
    assert!(!out.contains("formatter-rewritten"), "{out}");
}

/// Where 0.27's install projected a Claude / Codex bundle.
const CLAUDE_PROJECTION: &str = ".claude/skills/canon";
const CODEX_PROJECTION: &str = ".agents/skills/canon";

/// The reference files a 0.27-era install projected and this canon's bundle
/// never ships (a stub install projects no `reference/**`).
const RETIRED: [(&str, &str); 2] = [("reference/retired/old.md", "# old\n"), ("reference/retired/older.md", "# older\n")];

/// `write_canonical_source`, an install of `providers`, then the state an
/// older install left behind: the retired reference files written under each
/// of `projections` and listed in the install lock. Returns `(source, target)`.
fn installed_with_retired_references(tmp: &Path, providers: &str, projections: &[&str]) -> (std::path::PathBuf, std::path::PathBuf) {
    let (source, target) = (tmp.join("source"), tmp.join("target"));
    write_canonical_source(&source);
    skills::install_canonical(Some(&source), &target, Some(providers), false).unwrap();
    let mut lock = read_lock(&target);
    for projection in projections {
        for (relative, text) in RETIRED {
            let key = format!("{projection}/{relative}");
            let path = target.join(&key);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, text).unwrap();
            lock["files"][&key] = skills::content_hash(text.as_bytes()).into();
        }
    }
    fs::write(target.join(LOCK), serde_json::to_string_pretty(&lock).unwrap()).unwrap();
    (source, target)
}

fn stdout_json(output: &std::process::Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|err| panic!("{err}: {}", String::from_utf8_lossy(&output.stdout)))
}

/// skills.install.59: a file the install lock lists and the bundle never
/// ships is pruned when its bytes still match the lock.
#[test]
fn skills_install_59_install_prunes_a_dropped_file_whose_bytes_match_the_lock() {
    let tmp = tempfile::tempdir().unwrap();
    let (source, target) = installed_with_retired_references(tmp.path(), "claude", &[CLAUDE_PROJECTION]);
    let retired = target.join(CLAUDE_PROJECTION).join("reference/retired");
    assert!(retired.join("old.md").is_file() && retired.join("older.md").is_file());
    assert!(read_lock(&target)["files"][format!("{CLAUDE_PROJECTION}/reference/retired/old.md")].is_string());

    let out = canon(&["skills", "install", "--providers", "claude", "--json"], &source, &target);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    let json = stdout_json(&out);
    let pruned: Vec<&str> = json["data"]["pruned"].as_array().unwrap().iter().map(|entry| entry["path"].as_str().unwrap()).collect();
    assert_eq!(pruned.len(), 2, "{json}");
    for name in ["old.md", "older.md"] {
        assert!(pruned.iter().any(|path| path.ends_with(&format!("{CLAUDE_PROJECTION}/reference/retired/{name}"))), "{json}");
    }
    assert_eq!(json["data"]["kept"], serde_json::json!([]), "{json}");
    assert_eq!(json["data"]["changed"], true);

    assert!(!retired.exists(), "the directory the deletion left empty is removed");
    let projection = target.join(CLAUDE_PROJECTION);
    assert!(projection.join("SKILL.md").is_file() && projection.join("scripts/hook.sh").is_file(), "shipped files stay");
    assert!(!projection.join("reference/topic.md").exists(), "a source's reference files are never projected");
    let files = read_lock(&target)["files"].as_object().unwrap().keys().cloned().collect::<Vec<_>>();
    assert!(files.iter().all(|key| !key.contains("retired") && !key.contains("/reference/")), "the lock lists only shipped files: {files:?}");

    let rerun = stdout_json(&canon(&["skills", "install", "--providers", "claude", "--json"], &source, &target));
    assert_eq!(rerun["data"]["changed"], false, "{rerun}");
    assert_eq!(rerun["data"]["pruned"], serde_json::json!([]));
    assert_eq!(canon(&["skills", "check", "--providers", "claude"], &source, &target).status.code(), Some(0));
}

/// skills.install.60
#[test]
fn skills_install_60_install_keeps_a_dropped_file_whose_bytes_changed_and_reports_it() {
    let tmp = tempfile::tempdir().unwrap();
    let (source, target) = installed_with_retired_references(tmp.path(), "claude", &[CLAUDE_PROJECTION]);
    let retired = target.join(CLAUDE_PROJECTION).join("reference/retired");
    fs::write(retired.join("old.md"), "# old, with my own notes\n").unwrap();

    let out = canon(&["skills", "install", "--providers", "claude", "--json"], &source, &target);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    let json = stdout_json(&out);
    let kept = json["data"]["kept"].as_array().unwrap();
    assert_eq!(kept.len(), 1, "{json}");
    let path = kept[0]["path"].as_str().unwrap();
    assert!(path.ends_with(&format!("{CLAUDE_PROJECTION}/reference/retired/old.md")), "{json}");
    assert_eq!(kept[0]["provider"], "claude");
    assert!(kept[0]["reason"].as_str().is_some_and(|reason| !reason.is_empty()), "{json}");
    assert!(kept[0]["hint"].as_str().unwrap().contains(&format!("rm {path}")), "{json}");
    let pruned = json["data"]["pruned"].as_array().unwrap();
    assert_eq!(pruned.len(), 1, "the unmodified sibling is still pruned: {json}");
    assert!(pruned[0]["path"].as_str().unwrap().ends_with("older.md"));

    assert_eq!(fs::read_to_string(retired.join("old.md")).unwrap(), "# old, with my own notes\n", "the edited file is kept byte for byte");
    assert!(!retired.join("older.md").exists());
    let files = read_lock(&target)["files"].as_object().unwrap().keys().cloned().collect::<Vec<_>>();
    assert!(files.iter().all(|key| !key.contains("retired")), "the lock no longer lists the kept file: {files:?}");
    let rerun = stdout_json(&canon(&["skills", "install", "--providers", "claude", "--json"], &source, &target));
    assert_eq!(rerun["data"]["changed"], false, "{rerun}");
    assert_eq!(rerun["data"]["kept"], serde_json::json!([]));
}

/// skills.install.60: a dropped file swapped for a symlink to a file with
/// the very bytes the lock recorded is not the file canon wrote: install
/// keeps the link and never deletes through it.
#[cfg(unix)]
#[test]
fn skills_install_60_a_dropped_file_replaced_by_a_symlink_is_kept_and_never_followed() {
    let tmp = tempfile::tempdir().unwrap();
    let (source, target) = installed_with_retired_references(tmp.path(), "claude", &[CLAUDE_PROJECTION]);
    let outside = tmp.path().join("outside.md");
    fs::write(&outside, "# old\n").unwrap();
    let link = target.join(CLAUDE_PROJECTION).join("reference/retired/old.md");
    fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink(&outside, &link).unwrap();

    let report = skills::install_canonical(Some(&source), &target, Some("claude"), false).unwrap();
    assert_eq!(report.kept.len(), 1, "{report:?}");
    assert_eq!(report.kept[0].path, link);
    assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink(), "the link stays");
    assert_eq!(fs::read_to_string(&outside).unwrap(), "# old\n", "the file behind it stays");
}

/// skills.install.61
#[test]
fn skills_install_61_install_prunes_only_selected_providers_and_only_what_the_lock_listed() {
    let tmp = tempfile::tempdir().unwrap();
    let (source, target) = installed_with_retired_references(tmp.path(), "claude,codex", &[CLAUDE_PROJECTION, CODEX_PROJECTION]);
    let mine = target.join(CLAUDE_PROJECTION).join("reference/retired/mine.md");
    fs::write(&mine, "# mine, never in the lock\n").unwrap();
    let elsewhere = target.join(CLAUDE_PROJECTION).join("reference/notes.md");
    fs::write(&elsewhere, "# notes\n").unwrap();
    let codex_old = target.join(CODEX_PROJECTION).join("reference/retired/old.md");

    let report = skills::install_canonical(Some(&source), &target, Some("claude"), false).unwrap();
    assert_eq!(report.pruned.len(), 2, "{report:?}");
    assert!(report.pruned.iter().all(|orphan| orphan.provider == skills::Provider::Claude), "{report:?}");
    assert!(!target.join(CLAUDE_PROJECTION).join("reference/retired/old.md").exists());
    assert_eq!(fs::read_to_string(&mine).unwrap(), "# mine, never in the lock\n", "a file the lock never listed is never touched, nor the directory holding it");
    assert!(elsewhere.is_file());
    assert!(codex_old.is_file() && codex_old.with_file_name("older.md").is_file(), "an unselected provider's projection is untouched");
    assert!(target.join(CODEX_PROJECTION).join("SKILL.md").is_file());
    assert_eq!(canon(&["skills", "check", "--providers", "claude"], &source, &target).status.code(), Some(0));
}

/// skills.install.62
#[test]
fn skills_install_62_check_and_doctor_name_each_orphan_with_the_fix() {
    let tmp = tempfile::tempdir().unwrap();
    let (source, target) = installed_with_retired_references(tmp.path(), "claude", &[CLAUDE_PROJECTION]);
    let retired = target.join(CLAUDE_PROJECTION).join("reference/retired");
    fs::write(retired.join("older.md"), "# older, edited\n").unwrap();
    let fix = "canon skills install --providers=claude";

    let check = canon(&["skills", "check", "--providers", "claude"], &source, &target);
    let out = String::from_utf8_lossy(&check.stdout);
    assert_eq!(check.status.code(), Some(1), "{out}");
    let old = retired.join("old.md").display().to_string();
    let older = retired.join("older.md").display().to_string();
    let line_for = |path: &str| out.lines().find(|line| line.contains(path) && line.contains("orphan")).unwrap_or_else(|| panic!("{path} is named as an orphan: {out}")).to_string();
    assert!(line_for(&old).contains(&format!("run `{fix}`")), "an unmodified orphan names the install that removes it: {out}");
    assert!(line_for(&older).contains(&format!("rm {older}")), "a modified orphan names the removal by hand: {out}");

    let json = stdout_json(&canon(&["skills", "check", "--providers", "claude", "--json"], &source, &target));
    assert_eq!(json["error"]["code"], "SKILLS_DRIFT");
    assert!(json["error"]["message"].as_str().unwrap().contains(&old), "the error names the orphan: {json}");
    assert!(json["error"]["hint"].as_str().unwrap().contains(&older), "the hint names what to remove by hand: {json}");
    let orphans = json["data"]["orphans"].as_array().unwrap();
    assert_eq!(orphans.len(), 2, "{json}");
    assert_eq!(orphans.iter().filter(|o| o["modified"] == true).count(), 1, "{json}");
    for (entry, path, modified, how) in [
        (&orphans[0], &old, false, format!("run `{fix}`, which removes it")),
        (&orphans[1], &older, true, format!("rm {older}")),
    ] {
        assert_eq!(entry["path"], path.as_str(), "{json}");
        assert_eq!(entry["provider"], "claude", "{json}");
        assert_eq!(entry["modified"], modified, "{json}");
        assert!(entry["hint"].as_str().unwrap().contains(&how), "{json}");
    }
    let lock_path = target.join(".canon/skills/.install-lock.json");
    let (lock_before, files_before) = (fs::read(&lock_path).unwrap(), (fs::read(retired.join("old.md")).unwrap(), fs::read(retired.join("older.md")).unwrap()));
    canon(&["skills", "check", "--providers", "claude", "--json"], &source, &target);
    canon(&["skills", "check", "--providers", "claude"], &source, &target);
    assert_eq!(fs::read(&lock_path).unwrap(), lock_before, "check never rewrites the lock");
    assert_eq!(files_before, (fs::read(retired.join("old.md")).unwrap(), fs::read(retired.join("older.md")).unwrap()), "check never touches a file");

    let doctor = canon(&["skills", "doctor", "--providers", "claude"], &source, &target);
    let out = String::from_utf8_lossy(&doctor.stdout);
    assert_eq!(doctor.status.code(), Some(0), "{out}");
    assert!(out.lines().any(|line| line.starts_with(&format!("orphan: {old} (claude)")) && line.ends_with(&format!("run `{fix}`, which removes it"))), "{out}");
    assert!(out.lines().any(|line| line.starts_with(&format!("orphan: {older} (claude)")) && line.contains(&format!("rm {older}"))), "{out}");
    assert!(!out.contains("status: healthy"), "{out}");
    assert!(retired.join("old.md").is_file() && retired.join("older.md").is_file(), "check and doctor never delete");

    skills::install_canonical(Some(&source), &target, Some("claude"), false).unwrap();
    assert!(!retired.join("old.md").exists() && retired.join("older.md").is_file());
    assert_eq!(canon(&["skills", "check", "--providers", "claude"], &source, &target).status.code(), Some(0));
    let doctor = canon(&["skills", "doctor", "--providers", "claude"], &source, &target);
    assert!(String::from_utf8_lossy(&doctor.stdout).contains("status: healthy"));
}

/// skills.install.62: an orphan of a provider that is not selected is not
/// named, and a lock entry whose file is already gone is no orphan.
#[test]
fn skills_install_62_check_ignores_unselected_providers_and_files_already_gone() {
    let tmp = tempfile::tempdir().unwrap();
    let (source, target) = installed_with_retired_references(tmp.path(), "claude,codex", &[CLAUDE_PROJECTION, CODEX_PROJECTION]);
    let report = skills::check(Some(&source), &target, Some("claude,codex")).unwrap();
    assert_eq!(report.orphans.len(), 4, "two dropped files for each of two providers: {:?}", report.orphans);

    let report = skills::check(Some(&source), &target, Some("claude")).unwrap();
    assert_eq!(report.orphans.len(), 2);
    assert!(report.orphans.iter().all(|orphan| orphan.provider == skills::Provider::Claude));

    fs::remove_file(target.join(CLAUDE_PROJECTION).join("reference/retired/old.md")).unwrap();
    let report = skills::check(Some(&source), &target, Some("claude")).unwrap();
    assert_eq!(report.orphans.len(), 1, "a file the user already removed is not an orphan: {:?}", report.orphans);
}

/// skills.install.61: an install for a subset of providers keeps the lock
/// entries of the providers it did not select, so a later install for one of
/// them still prunes its dropped files and `check` still names them.
#[test]
fn skills_install_61_a_subset_install_keeps_the_lock_entries_of_unselected_providers() {
    let tmp = tempfile::tempdir().unwrap();
    let (source, target) = installed_with_retired_references(tmp.path(), "claude,codex", &[CLAUDE_PROJECTION, CODEX_PROJECTION]);
    skills::install_canonical(Some(&source), &target, Some("claude"), false).unwrap();
    let codex_retired = target.join(CODEX_PROJECTION).join("reference/retired");
    assert!(codex_retired.join("old.md").is_file(), "the unselected projection is untouched");

    let report = skills::check(Some(&source), &target, Some("codex")).unwrap();
    assert_eq!(report.orphans.len(), 2, "check for codex still names its dropped files: {:?}", report.orphans);

    let report = skills::install_canonical(Some(&source), &target, Some("codex"), false).unwrap();
    assert_eq!(report.pruned.len(), 2, "{report:?}");
    assert!(!codex_retired.join("old.md").exists() && !codex_retired.join("older.md").exists());
    let lock = read_lock(&target);
    assert!(lock["files"].as_object().unwrap().keys().all(|key| key.starts_with(".agents/")), "{lock}");
    let retained = lock["retained_files"].as_object().unwrap();
    assert!(!retained.is_empty() && retained.keys().all(|key| key.starts_with(".claude/") && !key.contains("retired")), "claude's shipped files stay recorded, its dropped ones are gone: {lock}");
}

/// skills.install.63: lock keys that are not plain relative paths inside a
/// canon projection are ignored, named by doctor, and never fail an install.
#[test]
fn skills_install_63_a_tampered_lock_key_is_ignored_and_named_by_doctor() {
    let tmp = tempfile::tempdir().unwrap();
    let (source, target) = installed_with_retired_references(tmp.path(), "claude", &[CLAUDE_PROJECTION]);
    let victim = target.join("victim.txt");
    fs::write(&victim, "keep\n").unwrap();
    let lock_path = target.join(".canon/skills/.install-lock.json");
    let mut lock = read_lock(&target);
    let hash = skills::content_hash(b"keep\n");
    let keys = [
        ".claude/skills/canon/../../../victim.txt",
        ".claude/skills/canon//reference/x.md",
        ".claude/skills/canon/./reference/x.md",
        ".claude/skills/canon/reference/",
    ];
    for key in keys {
        lock["files"][key] = serde_json::Value::String(hash.clone());
    }
    fs::write(&lock_path, serde_json::to_string_pretty(&lock).unwrap()).unwrap();

    let report = skills::check(Some(&source), &target, Some("claude")).unwrap();
    assert!(report.orphans.iter().all(|orphan| !orphan.path.ends_with("victim.txt")), "{:?}", report.orphans);
    assert_eq!(report.orphans.len(), 2, "only the two real dropped files: {:?}", report.orphans);
    assert_eq!(report.invalid_entries.len(), keys.len(), "{:?}", report.invalid_entries);
    let doctor = String::from_utf8_lossy(&canon(&["skills", "doctor", "--providers", "claude"], &source, &target).stdout).into_owned();
    for key in keys {
        assert!(doctor.lines().any(|line| line.starts_with("invalid-lock-entry:") && line.contains(&format!("{key:?}"))), "{doctor}");
    }

    let out = canon(&["skills", "install", "--providers", "claude"], &source, &target);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(fs::read_to_string(&victim).unwrap(), "keep\n");
    let rewritten = read_lock(&target);
    assert!(rewritten["files"].as_object().unwrap().keys().all(|key| !key.contains("victim") && !key.ends_with('/') && !key.contains("//") && !key.contains("/./")), "{rewritten}");
}

/// skills.install.64 (#105, #107): `canon skills doctor` in a canon repo
/// whose `.gitignore` lacks a line `canon init` manages names the missing
/// entries and the fix (`canon migrate`) in place of `status: healthy`;
/// with the line present it is healthy again.
#[test]
fn skills_install_64_doctor_reports_a_missing_gitignore_line_with_the_fix() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    write_canonical_source(&source);
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("canon.yaml"), CONFIG_WITHOUT_SKILLS).unwrap();
    let install = canon(&["skills", "install", "--providers", "claude"], &source, &target);
    assert_eq!(install.status.code(), Some(0), "{}", String::from_utf8_lossy(&install.stdout));
    let managed = ".canon/hot.db*\n.canon/audit/\n.canon/ledger/kind=scenario/\n.canon/archive/kind=scenario/\n.canon/scenario-index.lock\n.canon/artifact-store.lock\n.canon/dashboard-snapshot/\n.canon/learn/\n.canon/r2/\n";

    fs::write(target.join(".gitignore"), managed.replace(".canon/learn/\n", "").replace(".canon/r2/\n", "")).unwrap();
    let doctor = canon(&["skills", "doctor"], &source, &target);
    let out = String::from_utf8_lossy(&doctor.stdout);
    assert_eq!(doctor.status.code(), Some(0), "{out}");
    let line = out.lines().find(|line| line.starts_with("gitignore: ")).unwrap_or_else(|| panic!("{out}"));
    assert!(line.contains(".canon/learn/, .canon/r2/") && line.contains("fix: `canon migrate`"), "{line}");
    assert!(!out.contains("status: healthy"), "{out}");

    fs::write(target.join(".gitignore"), managed).unwrap();
    let doctor = canon(&["skills", "doctor"], &source, &target);
    let out = String::from_utf8_lossy(&doctor.stdout);
    assert!(out.contains("status: healthy") && !out.contains("gitignore:"), "{out}");
}
