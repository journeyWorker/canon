//! `canon plugin sync <plugin-id> [--spec-root <dir>] [--repo <dir>]`
//! (s16 P4, `openspec/changes/s16-plugin-extensibility/`, tasks.md
//! 4.2-4.3, design.md D5, `porting-plugin` spec): the GENERIC
//! overlay-sync dispatcher ([`OverlaySource`], [`run_sync`]) plus the
//! ONE data-producing source this change registers,
//! [`PortingOverlaySource`].
//!
//! # The dispatcher never names `porting` (design.md D5)
//!
//! [`run_sync`] resolves the plugin snapshot (`canon-plugin` P1),
//! looks up a registered [`OverlaySource`] by `plugin_id` STRING
//! EQUALITY against each source's own [`OverlaySource::plugin_id`],
//! then for every overlay identity the resolved plugin OWNS
//! (`PluginSnapshot::plugins[plugin_id].overlays`), resolves that
//! identity's `&OverlayDecl` FROM THE SNAPSHOT (never a second,
//! independently-computed declaration) and hands it to the source's
//! [`OverlaySource::overlay_candidates`], writing every returned
//! candidate through [`canon_plugin::overlay::write_overlay`] (P2's
//! validate-then-write pipeline). No line in this module, including
//! [`registry`]'s own body, tests `plugin_id == "porting"` or any
//! other plugin-specific string — the ONLY place the literal
//! `"porting"` appears is [`PortingOverlaySource::plugin_id`]'s own
//! return value, mirroring design.md D5's "every reference to
//! `porting` lives in exactly two places: `canon/plugins/porting/
//! plugin.yaml` (data) and one `PortingOverlaySource` Rust type". A
//! second donor-porting plugin would register its own [`OverlaySource`]
//! impl in [`registry`] and touch nothing else here.
//!
//! # `PortingOverlaySource` derives covered/surface_ref, never fabricates a decl
//!
//! [`PortingOverlaySource::overlay_candidates`] is HANDED the resolved
//! `&OverlayDecl` — it never constructs its own `OverlayDecl` or
//! assumes a field shape independent of `canon/plugins/porting/
//! plugin.yaml` (a source drifting from the manifest is exactly what
//! P1's "one resolution entry point" forbids). It reads a spec root's
//! `inventory/**/*.yaml` (`canon_model::family::inventory::
//! {InventoryFile, InventoryEntry}`, the SAME S11-validated files
//! `canon-fmt::check` already covers) and, for every `(project_id,
//! scenario_id)` [`crate::inventory::scan_feature_corpus`] would index
//! from that root's `.feature` corpus (the EXACT scan `canon inventory
//! sync` itself performs — reused, never re-derived), emits one
//! candidate body: `covered` is `true` iff that `scenario_id` appears
//! in ANY `InventoryEntry.covered_by` list anywhere under the root's
//! `inventory/`; `surface_ref` is every inventory-entry key whose
//! `covered_by` contains it (empty when `covered` is `false`).
//!
//! # Source-version `at` (idempotence + supersession)
//!
//! [`canon_store::git_tier::GitTier::write_namespaced`]'s dedup is a
//! CONTENT-DIGEST path match, so two candidate bodies for the SAME
//! `(project_id, scenario_id)` computed from an UNCHANGED inventory
//! serialize to EXACTLY the same bytes across separate `canon plugin
//! sync porting` invocations -- including the envelope's own `at`. But
//! P3's `project_overlay` folds overlay records latest-by-`at`, and
//! P2's `write_namespaced` is APPEND-ONLY (a logically different body
//! for the same join key writes a NEW digest path, never overwriting),
//! so a FIXED `at` would let a stale record tie-and-win the fold after
//! coverage changes. The `at` every candidate carries is therefore the
//! SOURCE VERSION: the max [`InventoryFile`] envelope `at` across the
//! root's `inventory/**/*.yaml` (EPOCH when the root has none, never
//! `Utc::now()`). Unchanged inventory -> same max -> byte-identical
//! body -> `deduped: true`, zero new files (tasks.md 4.4 idempotence).
//! A coverage edit -> the edited file's envelope `at` advances the max
//! (the S11 authoring discipline: editing an inventory record bumps its
//! own `at`) -> the new body sorts NEWER and wins the fold
//! (supersession, incl. a true->false coverage removal via a
//! covered_by edit). LIMITATION: this relies on the max envelope `at`
//! ADVANCING; deleting the newest inventory file outright (lowering the
//! max) leaves the prior overlay winning until a remaining file's `at`
//! advances -- consistent with the append-only snapshot model
//! (design.md R2).

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use canon_model::family::inventory::InventoryFile;
use canon_model::ids::ScenarioId;
use canon_model::{Actor, RawRecord};
use canon_plugin::{OverlayDecl, OverlayEnvelope, OverlayWriteError, compose_overlay_body, resolve_plugin_snapshot, write_overlay};
use canon_store::git_tier::GitTier;
use chrono::DateTime;
use serde_json::json;

use crate::inventory::{InventoryError, SpecRoot, SyncCtx, scan_feature_corpus};
use crate::context::resolve_repo_root;

/// [`PortingOverlaySource`]'s own unattributed actor -- a fixed
/// `agent_id`, never a per-run/per-session value, so an unchanged
/// inventory re-sync serializes to byte-identical overlay bodies
/// (module doc's "Source-version `at`" section: content-digest
/// idempotence depends on both this actor and the source-version `at`
/// being stable across runs over an unchanged inventory).
fn source_actor() -> Actor {
    Actor::new_unattributed("canon-plugin-porting-sync")
}

/// A data-producing plugin's overlay-record source (design.md D5,
/// tasks.md 4.2). One impl per plugin that actually WRITES overlay
/// records — [`run_sync`]'s [`registry`] holds every registered impl,
/// looked up by [`OverlaySource::plugin_id`] STRING EQUALITY, never a
/// hardcoded match arm (module doc).
pub trait OverlaySource {
    /// The `canon/plugins/<id>/plugin.yaml` `id` this source produces
    /// overlay records for — [`run_sync`] matches a `canon plugin sync
    /// <plugin-id>` invocation against this value, never the other way
    /// around.
    fn plugin_id(&self) -> &str;

    /// Produce every overlay candidate BODY (already envelope-composed
    /// via [`compose_overlay_body`], ready for [`write_overlay`]) this
    /// source derives from `spec_root` for the RESOLVED `decl` —
    /// handed the snapshot's own [`OverlayDecl`], never one this
    /// source constructs independently (module doc).
    fn overlay_candidates(&self, spec_root: &SpecRoot, decl: &OverlayDecl) -> Result<Vec<RawRecord>, OverlaySourceError>;
}

/// Everything an [`OverlaySource`] impl can fail with while deriving
/// candidates — [`PortingOverlaySource`] never actually returns this
/// today (a malformed `inventory/*.yaml` file is skipped, mirroring
/// `canon-fmt::check`'s/`canon inventory sync`'s own per-file
/// fail-soft discipline), but the trait stays fallible for a future
/// source that genuinely can fail (e.g. a network-backed donor).
#[derive(Debug, thiserror::Error)]
pub enum OverlaySourceError {
    #[error("{0}")]
    Other(String),
}

/// The porting plugin's `coverage` overlay source (design.md D5,
/// tasks.md 4.2, `porting-plugin` spec's "derives covered/surface_ref
/// from the donor inventory's covered_by join, inverted per scenario"
/// requirement) — canon-cli's own registered [`OverlaySource`] impl,
/// the ONE other place besides `canon/plugins/porting/plugin.yaml`
/// this change's `porting`-specific name lives (design.md D5).
#[derive(Debug, Clone, Copy, Default)]
pub struct PortingOverlaySource;

impl PortingOverlaySource {
    pub fn new() -> Self {
        Self
    }
}

impl OverlaySource for PortingOverlaySource {
    fn plugin_id(&self) -> &str {
        "porting"
    }

    fn overlay_candidates(&self, spec_root: &SpecRoot, decl: &OverlayDecl) -> Result<Vec<RawRecord>, OverlaySourceError> {
        // The (project_id, scenario_id) universe -- the EXACT scan
        // `canon inventory sync` performs over this SAME root, reused
        // rather than re-derived (module doc).
        let scenario_ids: BTreeSet<ScenarioId> = scan_feature_corpus(&spec_root.root).into_iter().map(|(id, _, _)| id).collect();

        // Invert every inventory-entry's `covered_by` list into a
        // per-scenario surface_ref set (module doc's "covered_by
        // inversion").
        let mut surface_refs: BTreeMap<ScenarioId, BTreeSet<String>> = BTreeMap::new();
        // Source-version `at`: the max envelope `at` across the root's
        // inventory files (module doc, "Source-version `at`"). EPOCH
        // when the root carries no inventory file.
        let mut source_at = DateTime::UNIX_EPOCH;
        for path in canon_fmt::util::walk_files(&spec_root.root, "inventory") {
            if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
                continue;
            }
            // `assets.lock.yaml` is a DIFFERENT family (the generated
            // asset lockfile, `InventoryLock`) -- never an
            // `InventoryFile`, skip it exactly like `canon-fmt::check`
            // does.
            if path.file_name().and_then(|f| f.to_str()) == Some("assets.lock.yaml") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let Ok(file) = serde_yaml::from_str::<InventoryFile>(&text) else { continue };
            source_at = source_at.max(file.envelope.at);
            for (entry_key, entry) in &file.entries {
                for scenario_id in &entry.covered_by {
                    surface_refs.entry(scenario_id.clone()).or_default().insert(entry_key.clone());
                }
            }
        }

        let mut candidates = Vec::with_capacity(scenario_ids.len());
        for scenario_id in scenario_ids {
            let refs: Vec<String> = surface_refs.remove(&scenario_id).unwrap_or_default().into_iter().collect();
            let covered = !refs.is_empty();

            let envelope = OverlayEnvelope::new(1, decl.identity.as_str(), source_at, source_actor());
            let mut fields = serde_json::Map::new();
            fields.insert("project_id".to_string(), json!(spec_root.id.as_str()));
            fields.insert("scenario_id".to_string(), json!(scenario_id.as_str()));
            fields.insert("covered".to_string(), json!(covered));
            fields.insert("surface_ref".to_string(), json!(refs));
            candidates.push(compose_overlay_body(&envelope, fields));
        }

        Ok(candidates)
    }
}

/// Every registered [`OverlaySource`] — TODAY exactly one,
/// [`PortingOverlaySource`] (design.md D5: "a SECOND donor-porting
/// plugin would add its own manifest + adapter the same way, touching
/// nothing this change built for `porting` specifically" — that future
/// addition is a second entry here, nothing else in this module
/// changes).
fn registry() -> Vec<Box<dyn OverlaySource>> {
    vec![Box::new(PortingOverlaySource::new())]
}

#[derive(Debug, thiserror::Error)]
pub enum PluginSyncError {
    #[error(transparent)]
    Inventory(#[from] InventoryError),
    #[error("no OverlaySource is registered for plugin `{0}`")]
    UnknownSource(String),
    #[error("plugin `{0}` has no installed manifest under `canon/plugins/`")]
    UnresolvedPlugin(String),
    #[error("plugin `{0}` declares no overlay")]
    NoOverlay(String),
}

/// One `(spec_root, overlay identity)` pair's write outcome.
#[derive(Debug, Clone, Default)]
pub struct RootOverlaySyncOutcome {
    pub root_id: String,
    pub root: PathBuf,
    pub overlay: String,
    pub candidates: usize,
    pub written: usize,
    pub deduped: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PluginSyncOutcome {
    pub plugin_id: String,
    pub roots: Vec<RootOverlaySyncOutcome>,
}

impl PluginSyncOutcome {
    pub fn is_clean(&self) -> bool {
        self.roots.iter().all(|r| r.errors.is_empty())
    }

    pub fn total_written(&self) -> usize {
        self.roots.iter().map(|r| r.written).sum()
    }

    pub fn total_deduped(&self) -> usize {
        self.roots.iter().map(|r| r.deduped).sum()
    }
}

/// `canon plugin sync <plugin_id> [--spec-root <dir>]` (module doc) —
/// the GENERIC dispatcher. `repo`/`spec_root_override` resolution
/// reuses [`SyncCtx`] verbatim (`canon inventory sync`'s own ctx,
/// tasks.md 4.3: "Reuse `canon inventory sync`'s root/tier
/// resolution"), so a `canon plugin sync porting` run writes into the
/// SAME git tier `canon query --plugin porting` reads.
pub fn run_sync(repo: &std::path::Path, plugin_id: &str, spec_root_override: Option<&std::path::Path>) -> Result<PluginSyncOutcome, PluginSyncError> {
    let repo = resolve_repo_root(repo);
    let ctx = SyncCtx::from_repo(&repo);
    let spec_roots = ctx.spec_roots(spec_root_override)?;

    let (snapshot, _diags) = resolve_plugin_snapshot(&ctx.repo);
    let resolved = snapshot.plugins.get(plugin_id).ok_or_else(|| PluginSyncError::UnresolvedPlugin(plugin_id.to_string()))?;

    let sources = registry();
    let source = sources.iter().find(|s| s.plugin_id() == plugin_id).ok_or_else(|| PluginSyncError::UnknownSource(plugin_id.to_string()))?;

    let mut owned_overlays: Vec<&OverlayDecl> = resolved.overlays.iter().filter_map(|id| snapshot.overlays.get(id)).collect();
    owned_overlays.sort_by(|a, b| a.identity.cmp(&b.identity));
    if owned_overlays.is_empty() {
        return Err(PluginSyncError::NoOverlay(plugin_id.to_string()));
    }

    let git = GitTier::new(ctx.ledger_root.clone());

    let mut roots = Vec::new();
    for spec_root in &spec_roots {
        for decl in &owned_overlays {
            let mut outcome =
                RootOverlaySyncOutcome { root_id: spec_root.id.as_str().to_string(), root: spec_root.root.clone(), overlay: decl.identity.clone(), ..Default::default() };
            match source.overlay_candidates(spec_root, decl) {
                Ok(candidates) => {
                    outcome.candidates = candidates.len();
                    for body in candidates {
                        match write_overlay(&git, decl, body) {
                            Ok(receipt) if receipt.deduped => outcome.deduped += 1,
                            Ok(_) => outcome.written += 1,
                            Err(e) => outcome.errors.push(overlay_write_error_to_string(&e)),
                        }
                    }
                }
                Err(e) => outcome.errors.push(e.to_string()),
            }
            roots.push(outcome);
        }
    }

    Ok(PluginSyncOutcome { plugin_id: plugin_id.to_string(), roots })
}

fn overlay_write_error_to_string(err: &OverlayWriteError) -> String {
    err.to_string()
}

/// `canon plugin sync`'s stdout — one summary line plus one line per
/// `(root, overlay)` pair.
pub fn format_human(outcome: &PluginSyncOutcome) -> String {
    let total_candidates: usize = outcome.roots.iter().map(|r| r.candidates).sum();
    let mut out = format!("plugin {} — {} candidate(s), {} written, {} deduped\n", outcome.plugin_id, total_candidates, outcome.total_written(), outcome.total_deduped());
    for root in &outcome.roots {
        out.push_str(&format!(
            "  root {} ({}) overlay {}: {} candidate(s), {} written, {} deduped\n",
            root.root_id,
            root.root.display(),
            root.overlay,
            root.candidates,
            root.written,
            root.deduped
        ));
        for err in &root.errors {
            out.push_str(&format!("    ERROR: {err}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use canon_model::ids::{ProjectId, SpecDigest};

    use super::*;

    fn write(dir: &std::path::Path, rel: &str, contents: &str) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    fn decl() -> OverlayDecl {
        OverlayDecl {
            namespace: "porting".to_string(),
            kind: "coverage".to_string(),
            identity: "porting.coverage".to_string(),
            core_kind: "scenario".to_string(),
            join_key: vec!["project_id".to_string(), "scenario_id".to_string()],
            fields: vec![],
        }
    }

    fn feature(scenario_id: &str, title: &str) -> String {
        format!("Feature: f\n\n  @{scenario_id}\n  Scenario: {title}\n    Given a step\n")
    }

    #[test]
    fn a_covered_scenario_projects_covered_true_with_its_surface_ref() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "features/f.feature", &feature("idolive.hub.01", "Opening the hub"));
        write(
            dir.path(),
            "inventory/kind=inventory/area=idolive/surface=hub/hub.yaml",
            "schema: 1\nkind: inventory\nat: \"2026-07-10T00:00:00Z\"\nactor:\n  agent_id: test\nidolive.hub.hub-header:\n  upstream:\n    pin: a\n    file: f.tsx\n    symbol: S\n    lines: \"1-2\"\n  covered_by: [idolive.hub.01]\n",
        );

        let spec_root = SpecRoot { id: ProjectId::parse("root").unwrap(), root: dir.path().to_path_buf() };
        let source = PortingOverlaySource::new();
        let candidates = source.overlay_candidates(&spec_root, &decl()).unwrap();
        assert_eq!(candidates.len(), 1);
        let body = &candidates[0].0;
        assert_eq!(body["scenario_id"], "idolive.hub.01");
        assert_eq!(body["covered"], true);
        assert_eq!(body["surface_ref"], serde_json::json!(["idolive.hub.hub-header"]));
    }

    #[test]
    fn an_uncovered_scenario_projects_covered_false_with_an_empty_surface_ref() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "features/f.feature", &feature("world.hotdeal.99", "Never covered"));

        let spec_root = SpecRoot { id: ProjectId::parse("root").unwrap(), root: dir.path().to_path_buf() };
        let source = PortingOverlaySource::new();
        let candidates = source.overlay_candidates(&spec_root, &decl()).unwrap();
        assert_eq!(candidates.len(), 1);
        let body = &candidates[0].0;
        assert_eq!(body["scenario_id"], "world.hotdeal.99");
        assert_eq!(body["covered"], false);
        assert_eq!(body["surface_ref"], serde_json::json!([]));
    }

    #[test]
    fn a_scenario_covered_by_multiple_entries_collects_every_surface_ref() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "features/f.feature", &feature("world.map.01", "Loading the map"));
        write(
            dir.path(),
            "inventory/kind=inventory/area=world/surface=map/a.yaml",
            "schema: 1\nkind: inventory\nat: \"2026-07-10T00:00:00Z\"\nactor:\n  agent_id: test\nworld.map.pin:\n  upstream:\n    pin: a\n    file: a.tsx\n    symbol: S\n    lines: \"1-2\"\n  covered_by: [world.map.01]\n",
        );
        write(
            dir.path(),
            "inventory/kind=inventory/area=world/surface=map/b.yaml",
            "schema: 1\nkind: inventory\nat: \"2026-07-10T00:00:00Z\"\nactor:\n  agent_id: test\nworld.map.legend:\n  upstream:\n    pin: a\n    file: b.tsx\n    symbol: S\n    lines: \"1-2\"\n  covered_by: [world.map.01]\n",
        );

        let spec_root = SpecRoot { id: ProjectId::parse("root").unwrap(), root: dir.path().to_path_buf() };
        let source = PortingOverlaySource::new();
        let candidates = source.overlay_candidates(&spec_root, &decl()).unwrap();
        assert_eq!(candidates.len(), 1);
        let refs = candidates[0].0["surface_ref"].as_array().unwrap();
        assert_eq!(refs.len(), 2);
        assert!(refs.contains(&serde_json::json!("world.map.legend")));
        assert!(refs.contains(&serde_json::json!("world.map.pin")));
    }

    #[test]
    fn two_calls_over_an_unchanged_root_produce_byte_identical_candidates() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "features/f.feature", &feature("idolive.hub.01", "Opening the hub"));
        write(
            dir.path(),
            "inventory/kind=inventory/area=idolive/surface=hub/hub.yaml",
            "schema: 1\nkind: inventory\nat: \"2026-07-10T00:00:00Z\"\nactor:\n  agent_id: test\nidolive.hub.hub-header:\n  upstream:\n    pin: a\n    file: f.tsx\n    symbol: S\n    lines: \"1-2\"\n  covered_by: [idolive.hub.01]\n",
        );

        let spec_root = SpecRoot { id: ProjectId::parse("root").unwrap(), root: dir.path().to_path_buf() };
        let source = PortingOverlaySource::new();
        let first = source.overlay_candidates(&spec_root, &decl()).unwrap();
        let second = source.overlay_candidates(&spec_root, &decl()).unwrap();
        assert_eq!(first, second, "deterministic `at`/`actor` must make repeated derivations byte-identical (idempotence precondition)");
    }

    #[test]
    fn scan_feature_corpus_reuse_matches_inventory_syncs_own_universe() {
        let dir = tempfile::tempdir().unwrap();
        let combined = feature("a.b.01", "One") + &feature("a.b.02", "Two");
        write(dir.path(), "features/f.feature", &combined);
        let scanned = scan_feature_corpus(dir.path());
        let ids: Vec<String> = scanned.iter().map(|(id, _, _): &(ScenarioId, String, SpecDigest)| id.as_str().to_string()).collect();
        assert_eq!(ids, vec!["a.b.01".to_string(), "a.b.02".to_string()]);
    }
}
