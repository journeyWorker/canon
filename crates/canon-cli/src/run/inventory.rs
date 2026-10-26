//! `canon inventory` — the handlers of the group.

use crate::cli::inventory::{InventoryCommand, InventorySyncArgs};
use super::top::with_provenance_fix;
use crate::cli_error::CliError;
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
                let error = CliError::failed("INVENTORY_ABORTED", "a spec root was aborted on violations or duplicate scenarios; nothing was written for it").hint(hint);
                ExitCode::from(error.emit_with_data_and_meta("canon inventory sync", json, &data, coverage.meta()) as u8)
            }
        }
        Err(err) => {
            ExitCode::from(CliError::refused("INVENTORY_FAILED", err.to_string()).hint("fix the problems the message names, then run `canon inventory sync` again").emit("canon inventory sync", json) as u8)
        }
    }
}
