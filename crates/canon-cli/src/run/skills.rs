//! `canon skills` — the handlers of the group.

use crate::cli::skills::*;
use crate::cli_error::CliError;
use std::process::ExitCode;

pub fn run(cmd: SkillsCommand) -> ExitCode {
    match cmd {
        SkillsCommand::Install(SkillsInstallArgs { skills_source, repo, providers, dry_run, json, .. }) => run_skills_install(skills_source.as_deref(), &repo, providers.as_deref(), dry_run.dry_run, json),
        SkillsCommand::Check(SkillsCheckArgs { skills_source, repo, providers, json }) => run_skills_check(skills_source.as_deref(), &repo, providers.as_deref(), json),
        SkillsCommand::Doctor(SkillsDoctorArgs { skills_source, repo, providers, json }) => run_skills_doctor(skills_source.as_deref(), &repo, providers.as_deref(), json),
        SkillsCommand::Show(SkillsShowArgs { name, full, json }) => run_skills_show(&name, full, json),
    }
}

fn run_skills_install(source: Option<&std::path::Path>, target: &std::path::Path, providers: Option<&str>, dry_run: bool, json: bool) -> ExitCode {
    let source = crate::skills::resolve_source(source);
    if let Some(dir) = source.as_deref().filter(|dir| !dir.join("SKILL.src.md").is_file()) {
        return match crate::skills::install(dir, target, dry_run) {
            Ok(report) => {
                if !dry_run && report.installed.iter().all(|skill| !skill.changed) {
                    crate::json_out::mark_unchanged();
                }
                for skill in &report.installed {
                    let status = match (skill.changed, dry_run) {
                        (true, false) => "installed",
                        (true, true) => "would install",
                        (false, _) => "unchanged",
                    };
                    crate::say!(json, "{} v{} — {}", skill.name, skill.version, status);
                }
                if json {
                    let skills = report.installed.iter().map(|s| crate::skills::InstalledSkillEntry { name: &s.name, version: s.version, changed: s.changed }).collect();
                    crate::json_out::data("skills install", &crate::skills::SkillsInstallReport::Legacy(crate::skills::LegacyInstall { dry_run, skills }));
                }
                ExitCode::SUCCESS
            }
            Err(err) => ExitCode::from(skills_error(&err).emit("canon skills install", json) as u8),
        };
    }
    match crate::skills::install_canonical(source.as_deref(), target, providers, dry_run) {
        Ok(report) => {
            if !dry_run && !report.changed && report.config_written.is_none() {
                crate::json_out::mark_unchanged();
            }
            let selection = crate::skills::ProviderSelection { providers: report.providers.clone(), origin: report.origin };
            let status = match (report.changed, dry_run) {
                (true, false) => "installed",
                (true, true) => "would install",
                (false, _) => "unchanged",
            };
            if report.providers.is_empty() {
                let hint = if report.origin == crate::skills::ProviderOrigin::Detected { format!("; {}", crate::skills::CHOOSE_PROVIDERS) } else { String::new() };
                let lock = match (report.changed, dry_run) {
                    (true, false) => "; install lock rewritten",
                    (true, true) => "; install lock would be rewritten",
                    (false, _) => "",
                };
                crate::say!(json, "canon v1 — nothing installed: no provider selected ({}){lock}{hint}", selection.describe());
            } else if report.origin == crate::skills::ProviderOrigin::Flag {
                crate::say!(json, "canon v1 — {status} ({})", selection.names());
            } else {
                crate::say!(json, "canon v1 — {status} ({}; {})", selection.names(), selection.describe());
            }
            if let Some(path) = &report.config_written {
                crate::say!(json, "canon skills install: {} `skills: {{ providers: [{}] }}` to {}", if dry_run { "would write" } else { "wrote" }, report.providers.iter().map(|p| p.as_str()).collect::<Vec<_>>().join(", "), path.display());
            }
            for orphan in &report.pruned {
                crate::say!(json, "{}: {} ({}) — the bundle no longer ships it; {}", if dry_run { "would prune" } else { "pruned" }, orphan.path.display(), orphan.provider.as_str(), orphan.reason());
            }
            for orphan in &report.kept {
                crate::say!(json, "kept: {} ({}) — the bundle no longer ships it; {}; {}", orphan.path.display(), orphan.provider.as_str(), orphan.reason(), orphan.by_hand());
            }
            let owned = crate::skills::owned_paths_lines(&report.providers);
            for line in &owned {
                crate::say!(json, "{line}");
            }
            if json {
                let data = crate::skills::BundleInstall {
                    dry_run,
                    providers: report.providers.iter().map(|p| p.as_str()).collect(),
                    selection: selection.describe(),
                    changed: report.changed,
                    config_written: report.config_written.as_ref().map(|p| p.display().to_string()),
                    pruned: report.pruned.iter().map(|o| crate::skills::PrunedEntry { path: o.path.display().to_string(), provider: o.provider.as_str(), reason: o.reason() }).collect(),
                    kept: report.kept.iter().map(|o| crate::skills::KeptEntry { path: o.path.display().to_string(), provider: o.provider.as_str(), reason: o.reason(), hint: o.by_hand() }).collect(),
                    owned_paths: owned,
                };
                crate::json_out::data("skills install", &crate::skills::SkillsInstallReport::Bundle(data));
            }
            ExitCode::SUCCESS
        }
        Err(err) => ExitCode::from(skills_error(&err).emit("canon skills install", json) as u8),
    }
}

fn run_skills_check(source: Option<&std::path::Path>, target: &std::path::Path, providers: Option<&str>, json: bool) -> ExitCode {
    let source = crate::skills::resolve_source(source);
    match crate::skills::check(source.as_deref(), target, providers) {
        Ok(report) => {
            let selection = crate::skills::ProviderSelection { providers: report.providers.clone(), origin: report.origin };
            if report.origin != crate::skills::ProviderOrigin::Flag {
                let hint = if report.providers.is_empty() && report.origin == crate::skills::ProviderOrigin::Detected { format!("; {}", crate::skills::CHOOSE_PROVIDERS) } else { String::new() };
                crate::say!(json, "providers: {} ({}){hint}", selection.names(), selection.describe());
            }
            for status in &report.statuses { crate::say!(json, "{} — {} ({})", status.path.display(), status.state, status.provider.as_str()); }
            for remnant in &report.remnants { crate::say!(json, "{} — legacy-remnant (codex reads .agents/skills; fix: `{}`)", remnant.display(), crate::skills::install_command(&report.providers)); }
            for orphan in &report.orphans { crate::say!(json, "{} — orphan ({}; the install lock lists it, but this canon's bundle no longer ships it; {}; {})", orphan.path.display(), orphan.provider.as_str(), orphan.reason(), orphan.fix(&report.providers)); }
            if let Some(outdated) = &report.outdated { crate::say!(json, "{} — outdated ({})", outdated.path.display(), outdated.describe(&report.providers)); }
            let clean = report.manifest_ok && report.orphans.is_empty() && report.remnants.is_empty() && report.outdated.is_none() && report.statuses.iter().all(|status| status.state == "ok");
            let data = crate::skills::SkillsCheckReport {
                clean,
                providers: report.providers.iter().map(|p| p.as_str()).collect(),
                selection: selection.describe(),
                manifest_ok: report.manifest_ok,
                statuses: report.statuses.iter().map(|s| crate::skills::SkillFileStatus { path: s.path.display().to_string(), state: s.state, provider: s.provider.as_str() }).collect(),
                orphans: report.orphans.iter().map(|o| crate::skills::OrphanEntry { path: o.path.display().to_string(), provider: o.provider.as_str(), modified: o.modified, reason: o.reason(), hint: o.fix(&report.providers) }).collect(),
                remnants: report.remnants.iter().map(|r| r.display().to_string()).collect(),
                outdated: report.outdated.as_ref().map(|o| crate::skills::OutdatedEntry { path: o.path.display().to_string(), installed_by: o.installed_by.clone(), message: o.describe(&report.providers) }),
            };
            let mut coverage = crate::coverage::Coverage::new();
            coverage.count("providers", report.providers.len()).count("skillFiles", report.statuses.len());
            if clean {
                if json {
                    crate::json_out::data_with_meta("skills check", &data, coverage.meta());
                }
                ExitCode::SUCCESS
            } else {
                let fix = crate::skills::install_command(&report.providers);
                let by_hand: Vec<String> = report.orphans.iter().filter(|o| o.modified).map(|o| o.path.display().to_string()).collect();
                let hint = if by_hand.is_empty() { format!("run `{fix}`") } else { format!("run `{fix}`, which prunes the orphans whose bytes still match the install lock; it keeps the modified ones ({}), so remove those by hand", by_hand.join(", ")) };
                let orphaned = if report.orphans.is_empty() { String::new() } else { format!("; orphaned (no longer shipped by the bundle): {}", report.orphans.iter().map(|o| o.path.display().to_string()).collect::<Vec<_>>().join(", ")) };
                ExitCode::from(CliError::failed("SKILLS_DRIFT", format!("an installed skill is missing, stale, a legacy remnant, an orphan the bundle no longer ships, or written by another canon version{orphaned}")).hint(hint).emit_with_data_and_meta("canon skills check", json, &data, coverage.meta()) as u8)
            }
        }
        Err(err) => ExitCode::from(skills_error(&err).emit("canon skills check", json) as u8),
    }
}

fn run_skills_doctor(source: Option<&std::path::Path>, target: &std::path::Path, providers: Option<&str>, json: bool) -> ExitCode {
    let source = crate::skills::resolve_source(source);
    match crate::skills::doctor(source.as_deref(), target, providers) {
        Ok(lines) => {
            for line in &lines { crate::say!(json, "{line}"); }
            if json {
                crate::json_out::data("skills doctor", &crate::skills::SkillsDoctorReport { lines: &lines });
            }
            ExitCode::SUCCESS
        }
        Err(err) => ExitCode::from(skills_error(&err).emit("canon skills doctor", json) as u8),
    }
}

/// `canon skills show <NAME> [--full]`: the guide built into the binary, as markdown on stdout, or enveloped under `--json`.
fn run_skills_show(name: &str, full: bool, json: bool) -> ExitCode {
    let markdown: std::borrow::Cow<'static, str> = if full && name == crate::guide::CORE {
        crate::guide::full().into()
    } else {
        // clap admits only the embedded names (`possibleValues`).
        crate::guide::guide(name).expect("clap admits only embedded guide names").into()
    };
    if json {
        crate::json_out::data("skills show", &crate::skills::SkillsShowReport { name: name.to_string(), version: crate::skills::CANON_VERSION, markdown });
    } else {
        print!("{markdown}");
    }
    ExitCode::SUCCESS
}

/// A failed skills command. An unknown provider, a malformed `skills:` section or a `--skills-source` directory that does not exist is an invalid argument, a refusal (exit 2, D4); anything else is an operational failure (exit 1).
fn skills_error(err: &crate::skills::SkillsError) -> CliError {
    use crate::skills::SkillsError;
    match err {
        SkillsError::InvalidProvider(value) => CliError::refused("SKILLS_INVALID_ARGUMENT", err.to_string())
            .field("--providers")
            .received(value.clone())
            .expected(["claude", "codex", "omp", "pi", "none"])
            .hint("pass `--providers` any subset of claude, codex, omp, pi (comma-separated), or `none`"),
        SkillsError::Config(_) => CliError::refused("SKILLS_INVALID_ARGUMENT", err.to_string()).hint("fix the `skills:` section of canon.yaml (`skills: { providers: [claude, codex, omp, pi] }`, any subset, `[]` for none), or pass `--providers`"),
        SkillsError::SourceNotFound(_) | SkillsError::CanonicalSourceMissing(_) => CliError::refused("SKILLS_INVALID_ARGUMENT", err.to_string()).field("--skills-source").hint("point `--skills-source` at a directory that holds `SKILL.src.md`, or drop it to use the text built into canon"),
        _ => CliError::failed("SKILLS_FAILED", err.to_string()).hint("check that the target directory is writable; `canon skills doctor` diagnoses the installation"),
    }
}
