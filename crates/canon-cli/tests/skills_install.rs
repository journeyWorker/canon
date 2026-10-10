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

#[test]
fn canonical_codex_is_detected_from_agents_or_codex() {
    for roots in [vec![".agents"], vec![".codex"], vec![".agents", ".codex"]] {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        let target = tmp.path().join("target");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("SKILL.src.md"), "---\nname: canon\ndescription: umbrella\n---\n").unwrap();
        for root in &roots {
            fs::create_dir_all(target.join(root)).unwrap();
        }

        let report = skills::install_canonical(&source, &target, None).unwrap();
        assert_eq!(report.providers, vec![skills::Provider::Codex], "roots {roots:?}");
        assert!(target.join(".agents/skills/canon/SKILL.md").is_file(), "roots {roots:?}");
        assert!(!target.join(".codex/skills").exists(), "roots {roots:?}");
    }
}

#[test]
fn canonical_invalid_provider_fails_before_writing() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.src.md"), "# canon\n").unwrap();
    let error = skills::install_canonical(&source, &target, Some("gemini")).expect_err("an unknown provider must be refused");
    assert!(matches!(&error, skills::SkillsError::InvalidProvider(provider) if provider == "gemini"), "{error:?}");
    assert!(error.to_string().contains("`gemini`"), "the rendered diagnostic must name the provider: {error}");
    assert!(!target.exists(), "no projection or lock file may be written");
}
#[test]
fn canonical_install_without_provider_selects_claude_and_codex() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    fs::create_dir_all(&source).unwrap();
    fs::write(
        source.join("SKILL.src.md"),
        "---\nname: canon\ndescription: umbrella\n---\n\n# Canon\n",
    )
    .unwrap();

    let report = skills::install_canonical(&source, &target, None).unwrap();

    assert_eq!(report.providers, vec![skills::Provider::Claude, skills::Provider::Codex]);
    assert!(target.join(".claude/skills/canon/SKILL.md").is_file());
    assert!(target.join(".agents/skills/canon/SKILL.md").is_file());
    assert!(!target.join(".gemini").exists());
    let entrypoints = [
        target.join(".claude/skills/canon/SKILL.md"),
        target.join(".agents/skills/canon/SKILL.md"),
    ];
    assert_eq!(
        entrypoints.iter().filter(|path| path.is_file()).count(),
        2,
        "canonical install must create exactly the Claude and Codex entrypoints",
    );
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
    skills::install_canonical(&source, &target, None).unwrap();

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
    assert_eq!(first.providers, vec![skills::Provider::Codex], ".codex still marks a Codex user");
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
