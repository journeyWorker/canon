//! s17 P4 task 4.2 -- a fixture SECOND `PlanAdapter` dialect,
//! registered ONLY in this test file (never in the production
//! [`canon_ingest::plan_adapter_registry`]), proving design D9's
//! structural claim: adding a dialect touches exactly one registry
//! entry plus one adapter module, with `PlanAdapter`,
//! `PlanParseOutcome`, the driver's registry-lookup pattern, and the
//! `openspec` adapter all byte-identical. This file's ENTIRE seam
//! extension is [`FixtureLineDialectAdapter`] (one `PlanAdapter` impl)
//! plus one more [`PlanAdapterEntry`] pushed into a test-local `Vec` --
//! it never edits `crate::plan_registry`, `crate::plan_adapter`, or
//! `crate::plan_adapters::openspec`.
//!
//! # A trivial one-line-per-change text dialect
//! `changes.txt`: one `<change_id>|<status>|<title>` row per line
//! (`status` one of `proposed`/`in_progress`/`completed`/`archived`).
//! A row failing `ChangeId::parse` or naming an unrecognized status
//! token is skipped and counted (`malformed`/`unmapped`
//! respectively) -- the SAME fail-soft-per-construct discipline design
//! D3 documents for `openspec`, proving the discipline is a property
//! of the trait's CONTRACT, not something `openspec` alone happens to
//! implement.

use std::fs;

use canon_ingest::{PlanAdapter, PlanAdapterEntry, PlanParseOutcome, PlanSourceConfig, PlanSourceHandle};
use canon_model::envelope::{Actor, Envelope, RecordKind};
use canon_model::ids::ChangeId;
use canon_model::records::{Change, ChangeStatus};

const SCHEMA_VERSION: u32 = 1;

/// The fixture dialect (module doc): one `PlanAdapter` impl, nothing
/// else touched.
struct FixtureLineDialectAdapter;

impl PlanAdapter for FixtureLineDialectAdapter {
    fn dialect_id(&self) -> &'static str {
        "fixture-line"
    }

    fn resolve_source(&self, config: &PlanSourceConfig) -> Option<PlanSourceHandle> {
        // The SAME shared helper `plan_adapters::openspec` resolves
        // through -- proving the config-vs-handle seam is generic
        // infrastructure, not something the openspec module privately
        // owns.
        canon_ingest::plan_adapter::resolve_path_source(&config.root)
    }

    fn parse(&self, source: &PlanSourceHandle) -> PlanParseOutcome {
        let PlanSourceHandle::Path(root) = source;
        let mut outcome = PlanParseOutcome::empty();
        let Ok(text) = fs::read_to_string(root.join("changes.txt")) else {
            return outcome;
        };
        let at = chrono::DateTime::<chrono::Utc>::UNIX_EPOCH;
        let actor = Actor::new_unattributed("canon-plan-import-fixture-line");

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut fields = line.splitn(3, '|');
            let (Some(id_raw), Some(status_raw), Some(title)) = (fields.next(), fields.next(), fields.next()) else {
                // Not even three `|`-joined fields -- structurally
                // broken, never a guessed mapping (design D3).
                outcome.record_malformed(line, "missing-pipe-fields");
                continue;
            };
            let Ok(change_id) = ChangeId::parse(id_raw) else {
                outcome.record_malformed(line, "invalid-change-id-grammar");
                continue;
            };
            let Some(status) = parse_status(status_raw) else {
                // A recognized SHAPE (three fields) but an
                // unrecognized status token is unmappable, not
                // malformed -- exactly design D3's "genuinely
                // unmappable construct" case.
                outcome.record_unmapped("fixture-line-unknown-status");
                continue;
            };
            let envelope = Envelope::new(SCHEMA_VERSION, RecordKind::Change, at, actor.clone());
            outcome.changes.push(Change::new(envelope, change_id, title, "", status));
        }
        outcome
    }
}

fn parse_status(token: &str) -> Option<ChangeStatus> {
    match token {
        "proposed" => Some(ChangeStatus::Proposed),
        "in_progress" => Some(ChangeStatus::InProgress),
        "completed" => Some(ChangeStatus::Completed),
        "archived" => Some(ChangeStatus::Archived),
        _ => None,
    }
}

/// A test-local registry carrying the REAL production `openspec` entry
/// (borrowed straight out of [`canon_ingest::plan_adapter_registry`],
/// never re-implemented) beside the fixture dialect -- D9's "one
/// registry entry" claim, made concrete: this `Vec` is the entire
/// diff a real second dialect would add to `plan_registry::registry`'s
/// own array literal.
fn local_registry() -> Vec<PlanAdapterEntry> {
    static FIXTURE: FixtureLineDialectAdapter = FixtureLineDialectAdapter;
    let openspec_adapter = canon_ingest::plan_adapter_registry().first().expect("production plan_registry ships the openspec entry").adapter;
    vec![PlanAdapterEntry { adapter: openspec_adapter, write_back: None }, PlanAdapterEntry { adapter: &FIXTURE, write_back: None }]
}

/// Mirrors `crate::plan_registry::find`'s exact lookup one-liner
/// (`crates/canon-ingest/src/plan_registry.rs`) -- the driver-side
/// pattern `canon ingest plans` uses, applied here to a 2-entry
/// registry instead of production's 1-entry one, with NO new lookup
/// logic.
fn local_find<'a>(registry: &'a [PlanAdapterEntry], dialect_id: &str) -> Option<&'a PlanAdapterEntry> {
    registry.iter().find(|entry| entry.dialect_id() == dialect_id)
}

#[test]
fn the_fixture_dialect_extends_the_seam_by_exactly_one_entry_beside_the_untouched_openspec_one() {
    let local = local_registry();
    let ids: Vec<&str> = local.iter().map(|e| e.dialect_id()).collect();
    assert_eq!(ids, vec!["openspec", "fixture-line"], "the fixture dialect is the ONLY addition -- openspec keeps its declared-order position");

    // Production's own registry is untouched by this test file's
    // existence -- the seam extension happened entirely in
    // `local_registry`'s test-local `Vec`, never in
    // `crate::plan_registry::registry`.
    let production_ids: Vec<&str> = canon_ingest::plan_adapter_registry().iter().map(|e| e.dialect_id()).collect();
    assert_eq!(
        production_ids,
        vec!["openspec", "superpowers"],
        "production plan_registry must still ship exactly openspec + superpowers (D9's proof: adding a THIRD dialect, or this test's fixture one, never touches it)"
    );
}

#[test]
fn the_fixture_dialect_resolves_and_parses_through_the_same_trait_and_outcome_type() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    fs::write(tmp.path().join("changes.txt"), "widget-fixture|in_progress|A fixture-line change\nbad basename!|proposed|skipped, bad ChangeId\nno-pipes-at-all\nanother-fixture|bogus-status|unmapped status token\n").expect("write changes.txt");

    let local = local_registry();
    let entry = local_find(&local, "fixture-line").expect("fixture-line is registered in the local registry");

    let config = PlanSourceConfig { root: Some(tmp.path().to_path_buf()) };
    let handle = entry.adapter.resolve_source(&config).expect("resolve_source over a configured root");
    let outcome: PlanParseOutcome = entry.adapter.parse(&handle);

    assert_eq!(outcome.changes.len(), 1, "only the one well-formed row emits a Change: {:?}", outcome.changes);
    let change = &outcome.changes[0];
    assert_eq!(change.change_id.as_str(), "widget-fixture");
    assert_eq!(change.title, "A fixture-line change");
    assert_eq!(change.status, ChangeStatus::InProgress);

    assert_eq!(outcome.malformed.len(), 2, "`bad basename!` (bad ChangeId) + the pipe-less line are both malformed, never a guessed mapping");
    assert_eq!(outcome.unmapped.get("fixture-line-unknown-status"), Some(&1), "the bogus-status row is unmappable, named and counted (design D3), never silently dropped");

    // The SAME registered openspec entry, reached through the SAME
    // local_find/resolve_source/parse seam, still behaves exactly as
    // `crate::plan_registry`'s own tests pin (an unconfigured root ->
    // None; here we only prove the seam is generic, not openspec's
    // own mapping logic, which `plan_adapters::openspec`'s own test
    // suite already covers exhaustively).
    let openspec_entry = local_find(&local, "openspec").expect("openspec still resolves through the SAME local_find helper");
    assert!(openspec_entry.adapter.resolve_source(&PlanSourceConfig::default()).is_none(), "openspec's own resolve_source contract (None when unconfigured) is untouched by this file's existence");
}

#[test]
fn an_unregistered_dialect_id_misses_via_the_same_find_contract_as_production() {
    let local = local_registry();
    // Mirrors `crate::plan_registry`'s own
    // `find_returns_none_on_a_miss_never_a_loud_error` test, applied
    // to the 2-entry local registry -- the miss contract is a property
    // of the lookup pattern, not of how many entries happen to be
    // registered.
    assert!(local_find(&local, "no-such-dialect").is_none());
    assert!(local_find(&local, "").is_none());
}
