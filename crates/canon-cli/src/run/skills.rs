//! `canon skills` — the handlers of the group.

use crate::cli::skills::*;
use crate::cli_error::CliError;
use std::process::ExitCode;

pub fn run(cmd: SkillsCommand) -> ExitCode {
    match cmd {
        SkillsCommand::Install(SkillsInstallArgs { source, target, providers, json }) => run_skills_install(source.as_deref(), &target, providers.as_deref(), json),
        SkillsCommand::Check(SkillsCheckArgs { source, target, providers, json }) => run_skills_check(source.as_deref(), &target, providers.as_deref(), json),
        SkillsCommand::Doctor(SkillsDoctorArgs { source, target, providers, json }) => run_skills_doctor(source.as_deref(), &target, providers.as_deref(), json),
    }
}

fn run_skills_install(source: Option<&std::path::Path>, target: &std::path::Path, providers: Option<&str>, json: bool) -> ExitCode {
    let source = crate::skills::resolve_source(source);
    if !source.join("SKILL.src.md").is_file() {
        return match crate::skills::install(&source, target) {
            Ok(report) => {
                for skill in &report.installed {
                    let status = if skill.changed { "installed" } else { "unchanged" };
                    crate::say!(json, "{} v{} — {}", skill.name, skill.version, status);
                }
                if json {
                    let skills: Vec<_> = report.installed.iter().map(|s| serde_json::json!({ "name": s.name, "version": s.version, "changed": s.changed })).collect();
                    crate::json_out::data("skills install", &crate::json_out::untyped(serde_json::json!({ "skills": skills })));
                }
                ExitCode::SUCCESS
            }
            Err(err) => { ExitCode::from(CliError::failed("SKILLS_FAILED", err.to_string()).hint("check that the target directory is writable; `canon skills doctor` diagnoses the installation").emit("canon skills install", json) as u8) }
        };
    }
    match crate::skills::install_canonical(&source, target, providers) {
        Ok(report) => {
            let selection = crate::skills::ProviderSelection { providers: report.providers.clone(), origin: report.origin };
            let status = if report.changed { "installed" } else { "unchanged" };
            if report.providers.is_empty() {
                let hint = if report.origin == crate::skills::ProviderOrigin::Detected { format!("; {}", crate::skills::CHOOSE_PROVIDERS) } else { String::new() };
                let lock = if report.changed { "; install lock rewritten" } else { "" };
                crate::say!(json, "canon v1 — nothing installed: no provider selected ({}){lock}{hint}", selection.describe());
            } else if report.origin == crate::skills::ProviderOrigin::Flag {
                crate::say!(json, "canon v1 — {status} ({})", selection.names());
            } else {
                crate::say!(json, "canon v1 — {status} ({}; {})", selection.names(), selection.describe());
            }
            if let Some(path) = &report.config_written {
                crate::say!(json, "canon skills install: wrote `skills: {{ providers: [{}] }}` to {}", report.providers.iter().map(|p| p.as_str()).collect::<Vec<_>>().join(", "), path.display());
            }
            for orphan in &report.pruned {
                crate::say!(json, "pruned: {} ({}) — the bundle no longer ships it; {}", orphan.path.display(), orphan.provider.as_str(), orphan.reason());
            }
            for orphan in &report.kept {
                crate::say!(json, "kept: {} ({}) — the bundle no longer ships it; {}; {}", orphan.path.display(), orphan.provider.as_str(), orphan.reason(), orphan.by_hand());
            }
            let owned = crate::skills::owned_paths_lines(&report.providers);
            for line in &owned {
                crate::say!(json, "{line}");
            }
            if json {
                crate::json_out::data(
                    "skills install",
                    &crate::json_out::untyped(serde_json::json!({
                        "providers": report.providers.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
                        "selection": selection.describe(),
                        "changed": report.changed,
                        "configWritten": report.config_written.as_ref().map(|p| p.display().to_string()),
                        "pruned": report.pruned.iter().map(|o| serde_json::json!({ "path": o.path.display().to_string(), "provider": o.provider.as_str(), "reason": o.reason() })).collect::<Vec<_>>(),
                        "kept": report.kept.iter().map(|o| serde_json::json!({ "path": o.path.display().to_string(), "provider": o.provider.as_str(), "reason": o.reason(), "hint": o.by_hand() })).collect::<Vec<_>>(),
                        "ownedPaths": owned,
                    })));
            }
            ExitCode::SUCCESS
        }
        Err(err) => { ExitCode::from(CliError::failed("SKILLS_FAILED", err.to_string()).hint("check that the target directory is writable; `canon skills doctor` diagnoses the installation").emit("canon skills install", json) as u8) }
    }
}

fn run_skills_check(source: Option<&std::path::Path>, target: &std::path::Path, providers: Option<&str>, json: bool) -> ExitCode {
    let source = crate::skills::resolve_source(source);
    match crate::skills::check(&source, target, providers) {
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
            let data = serde_json::json!({
                "clean": clean,
                "providers": report.providers.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
                "selection": selection.describe(),
                "manifestOk": report.manifest_ok,
                "statuses": report.statuses.iter().map(|s| serde_json::json!({ "path": s.path.display().to_string(), "state": s.state, "provider": s.provider.as_str() })).collect::<Vec<_>>(),
                "orphans": report.orphans.iter().map(|o| serde_json::json!({ "path": o.path.display().to_string(), "provider": o.provider.as_str(), "modified": o.modified, "reason": o.reason(), "hint": o.fix(&report.providers) })).collect::<Vec<_>>(),
                "remnants": report.remnants.iter().map(|r| r.display().to_string()).collect::<Vec<_>>(),
                "outdated": report.outdated.as_ref().map(|o| serde_json::json!({ "path": o.path.display().to_string(), "installedBy": o.installed_by, "message": o.describe(&report.providers) })),
            });
            if clean {
                if json {
                    crate::json_out::data("skills check", &crate::json_out::untyped(data));
                }
                ExitCode::SUCCESS
            } else {
                let fix = crate::skills::install_command(&report.providers);
                let by_hand: Vec<String> = report.orphans.iter().filter(|o| o.modified).map(|o| o.path.display().to_string()).collect();
                let hint = if by_hand.is_empty() { format!("run `{fix}`") } else { format!("run `{fix}`, which prunes the orphans whose bytes still match the install lock; it keeps the modified ones ({}), so remove those by hand", by_hand.join(", ")) };
                let orphaned = if report.orphans.is_empty() { String::new() } else { format!("; orphaned (no longer shipped by the bundle): {}", report.orphans.iter().map(|o| o.path.display().to_string()).collect::<Vec<_>>().join(", ")) };
                ExitCode::from(CliError::failed("SKILLS_DRIFT", format!("an installed skill is missing, stale, a legacy remnant, an orphan the bundle no longer ships, or written by another canon version{orphaned}")).hint(hint).emit_with_data("canon skills check", json, &crate::json_out::untyped(data)) as u8)
            }
        }
        Err(err) => { ExitCode::from(CliError::failed("SKILLS_FAILED", err.to_string()).hint("check that the target directory is writable; `canon skills doctor` diagnoses the installation").emit("canon skills check", json) as u8) }
    }
}

fn run_skills_doctor(source: Option<&std::path::Path>, target: &std::path::Path, providers: Option<&str>, json: bool) -> ExitCode {
    let source = crate::skills::resolve_source(source);
    match crate::skills::doctor(&source, target, providers) {
        Ok(lines) => {
            for line in &lines { crate::say!(json, "{line}"); }
            if json {
                crate::json_out::data("skills doctor", &crate::json_out::untyped(serde_json::json!({ "lines": lines })));
            }
            ExitCode::SUCCESS
        }
        Err(err) => { ExitCode::from(CliError::failed("SKILLS_FAILED", err.to_string()).hint("check that the target directory is writable; `canon skills doctor` diagnoses the installation").emit("canon skills doctor", json) as u8) }
    }
}
