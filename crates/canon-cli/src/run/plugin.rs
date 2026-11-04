//! `canon plugin` — the handlers of the group.

use crate::cli::plugin::{PluginCommand, PluginSyncArgs};
use crate::cli_error::{CliError, CliErrorExt as _};
use std::process::ExitCode;

pub fn run(cmd: PluginCommand) -> ExitCode {
    match cmd {
        PluginCommand::Sync(PluginSyncArgs { plugin_id, repo, spec_root, dry_run, json, .. }) => run_plugin_sync(&repo, &plugin_id, spec_root.as_deref(), dry_run.dry_run, json),
    }
}

/// `canon plugin sync <plugin-id> [--spec-root <dir>]` (s16 P4,
/// `crate::plugin_sync::run_sync`'s own doc): `2` on a resolution
/// error (unresolved plugin id, no registered `OverlaySource`, or a
/// `specs:` config fault — mirrors `canon inventory sync`'s own
/// refused-invocation exit code), `1` when a write attempt itself
/// failed for some candidate (`outcome.is_clean()` false), `0`
/// otherwise (including the common zero-new-writes idempotent re-sync
/// case).
fn run_plugin_sync(repo: &std::path::Path, plugin_id: &str, spec_root: Option<&std::path::Path>, dry_run: bool, json: bool) -> ExitCode {
    match crate::plugin_sync::run_sync(repo, plugin_id, spec_root, dry_run) {
        Ok(outcome) => {
            crate::say_raw!(json, "{}", crate::plugin_sync::format_human(&outcome));
            let data = crate::plugin_sync::format_json(&outcome);
            if outcome.is_clean() {
                if json {
                    crate::json_out::data("plugin sync", &data);
                }
                ExitCode::SUCCESS
            } else {
                let error = CliError::failed("PLUGIN_SYNC_ERRORS", "some candidates could not be written").hint("data.roots names each error");
                ExitCode::from(error.emit_with_data("canon plugin sync", json, &data))
            }
        }
        Err(err) => {
            let error = match &err {
                crate::plugin_sync::PluginSyncError::Unavailable(error) => error.clone(),
                crate::plugin_sync::PluginSyncError::Inventory(crate::inventory::InventoryError::Config(_)) => CliError::refused("CONFIG_INVALID", err.to_string()).reason("SPECS").hint("fix the `specs:` section of canon.yaml, then run `canon plugin sync` again"),
                crate::plugin_sync::PluginSyncError::Inventory(crate::inventory::InventoryError::CanonYaml(_)) => crate::read_cause::config_error(err.to_string()),
                crate::plugin_sync::PluginSyncError::Inventory(_) => CliError::refused("INVENTORY_INDEX_UNAVAILABLE", err.to_string()).hint("another canon process holds the scenario index; wait for it, then run `canon plugin sync` again"),
                crate::plugin_sync::PluginSyncError::UnknownSource(_) | crate::plugin_sync::PluginSyncError::UnresolvedPlugin(_) | crate::plugin_sync::PluginSyncError::NoOverlay(_) => CliError::refused("PLUGIN_REQUIRED", err.to_string()).hint("check that the plugin is installed under `.canon/plugins/` and declares an overlay, then run `canon plugin sync` again"),
            };
            ExitCode::from(error.emit("canon plugin sync", json))
        }
    }
}
