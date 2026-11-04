//! `canon inventory` — the handlers of the group.

use crate::cli::inventory::{InventoryCommand, InventorySyncArgs};
use super::top::with_provenance_fix;
use crate::cli_error::{CliError, CliErrorExt as _};
use std::process::ExitCode;

pub fn run(cmd: InventoryCommand) -> ExitCode {
    match cmd {
        InventoryCommand::Sync(InventorySyncArgs { repo, spec_root, dry_run, json, .. }) => run_inventory_sync(&repo, spec_root.as_deref(), dry_run.dry_run, json),
    }
}

/// `canon inventory sync [--spec-root <dir>]` (s15 P3a): mirrors `canon
/// fmt`'s own 0-clean/nonzero-on-violation convention — `1` when ANY
/// configured root aborted on an S11 validation violation
/// (`crate::inventory::run_sync`'s module doc, "whole-root abort"),
/// `2` on a fail-loud config error (`specs:` present-but-malformed —
/// mirrors `canon review add`/`canon divergence add`'s own refused-
/// invocation exit code), `0` otherwise (including the common
/// zero-writes no-op re-sync case).
fn run_inventory_sync(repo: &std::path::Path, spec_root: Option<&std::path::Path>, dry_run: bool, json: bool) -> ExitCode {
    match if dry_run { crate::inventory::preview_sync(repo, spec_root) } else { crate::inventory::run_sync(repo, spec_root) } {
        Ok(outcome) => {
            crate::say_raw!(json, "{}", crate::inventory::format_human(&outcome));
            let data = crate::inventory::format_json(&outcome);
            let mut coverage = crate::coverage::Coverage::new();
            for root in &outcome.roots {
                let shown = root.root.strip_prefix(repo).unwrap_or(&root.root).display().to_string();
                if root.root.is_dir() {
                    coverage.root(root.id.as_str(), shown, root.scanned);
                } else {
                    coverage.skip(shown, format!("spec root `{}` is not a directory", root.id.as_str()));
                }
            }
            coverage.count("scenarios", outcome.roots.iter().map(|r| r.scanned).sum());
            if outcome.is_clean() {
                crate::next::finish_with_meta(repo, "inventory sync", &crate::next::Scope::Repo, false, json, &data, coverage.meta());
                ExitCode::SUCCESS
            } else {
                let missing_provenance = outcome.roots.iter().flat_map(|root| &root.violations).any(|v| v.class == canon_fmt::FmtFailureClass::MissingProvenance);
                let hint = with_provenance_fix("data.roots names each violation; fix them, then run `canon inventory sync` again", missing_provenance);
                // `INVENTORY_INCOMPLETE` needs work done before the stop: a commit that indexed an earlier root; its reason is the refusal the stopped root met (`INVENTORY_INVALID`).
                if !outcome.dry_run && outcome.total_written() > 0 {
                    crate::incomplete::read_status("read what `inventory sync` indexed before it stopped: the roots before the unindexed one stay indexed, `data.roots` says per root");
                    let error = CliError::failed("INVENTORY_INCOMPLETE", "a spec root was left unindexed on violations or duplicate scenarios; nothing was written for it, and the roots before it are indexed").reason("INVENTORY_INVALID").hint(hint);
                    return ExitCode::from(error.emit_with_data_and_meta("canon inventory sync", json, &data, coverage.meta()));
                }
                // A sync that indexed nothing (and every preview) refuses before its work (spec 7.2.2, 8.3): exit 2, no `data`, each violation and duplicate in `error.details`.
                let hint = hint.replace("data.roots names each violation", "error.details names each violation");
                let message = if outcome.dry_run { "a spec root would be left unindexed on violations or duplicate scenarios; this was a preview and nothing was written" } else { "a spec root has violations or duplicate scenarios, so the sync indexed nothing" };
                let refused = |reason: &'static str, text: String| CliError::refused("INVENTORY_INVALID", text).reason(reason).hint(hint.clone());
                let mut items: Vec<CliError> = Vec::new();
                for root in &outcome.roots {
                    for v in &root.violations {
                        items.push(refused("VIOLATIONS", format!("root `{}`: [{}] {} — {}", root.id, v.class.as_str(), v.path.display(), v.detail)));
                    }
                    for e in &root.sync_errors {
                        items.push(refused("DUPLICATE_SCENARIOS", format!("root `{}`: [duplicate-scenario] {e}", root.id)));
                    }
                }
                let reason = if outcome.roots.iter().any(|root| !root.violations.is_empty()) { "VIOLATIONS" } else { "DUPLICATE_SCENARIOS" };
                let top = refused(reason, message.to_string());
                let mut all = vec![top.clone()];
                all.extend(items);
                let error = if all.len() > 1 { top.details(all) } else { top };
                ExitCode::from(error.emit("canon inventory sync", json))
            }
        }
        Err(err) => {
            if let crate::inventory::InventoryError::Unavailable(error) = &err {
                return ExitCode::from(error.emit("canon inventory sync", json));
            }
            // The specs config, canon.yaml and the index lock are resolved before any record is read or written (exit 2); a store error comes while the sync works (exit 1).
            let (code, reason) = match &err {
                crate::inventory::InventoryError::Store(_) => ("INVENTORY_FAILED", None),
                crate::inventory::InventoryError::Stopped { .. } => ("INVENTORY_INCOMPLETE", Some("INVENTORY_FAILED")),
                crate::inventory::InventoryError::Config(_) => ("CONFIG_INVALID", Some("SPECS")),
                crate::inventory::InventoryError::CanonYaml(_) => (crate::read_cause::config_error(err.to_string()).code, None),
                crate::inventory::InventoryError::Locked(_) => ("INVENTORY_INDEX_UNAVAILABLE", None),
                crate::inventory::InventoryError::Unavailable(error) => (error.code, None),
            };
            if code == "INVENTORY_INCOMPLETE" {
                crate::incomplete::read_status("read what `inventory sync` indexed before it stopped: the records of the roots before the failing one stay indexed");
            }
            let hint = match code {
                "INVENTORY_INDEX_UNAVAILABLE" => "another canon process holds the scenario index: wait for it to finish, then run `canon inventory sync` again",
                _ => "fix the problems the message names, then run `canon inventory sync` again",
            };
            let error = CliError::refused(code, err.to_string()).hint(hint);
            let error = match reason {
                Some(reason) => error.reason(reason),
                None => error,
            };
            ExitCode::from(error.emit("canon inventory sync", json))
        }
    }
}
