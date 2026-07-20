//! The static `PlanAdapter` registry seam (s17 P1 FOUNDATION, task
//! 1.2) — mirrors [`crate::artifact_registry`]'s `ArtifactAdapter`
//! registry shape (a static, declaration-ordered table; no dynamic
//! plugin loading, S3 design D1) over
//! [`crate::plan_adapter::PlanAdapter`] instead.
//!
//! P1 shipped exactly one entry — `openspec`, [`crate::plan_adapters::openspec`]'s
//! registry-seam placeholder, since replaced by the real change-dir
//! discovery + `Change`/`Task` mapping adapter (s17 P2). s30
//! `plan-dialect-superpowers` added the second —
//! [`crate::plan_adapters::superpowers`], s17 D9's named follow-up,
//! shipped against the superpowers `writing-plans` skill's grammar —
//! as exactly one more [`PlanAdapterEntry`] line below, no change to
//! [`PlanAdapterEntry`], [`registry`], [`find`], or the existing
//! `openspec` entry (`plan-import-connector` spec, "A second dialect
//! lands as one registry entry"). `donor-json` stays deferred (its own
//! deferral condition, a concrete donor plan corpus, has not resolved).
//!
//! `find`'s miss case returns `None` — the loud "unknown dialect id,
//! here are the registered ones" error is the CLI driver's job (P3),
//! never this registry's (task 1.4): a lookup helper that itself
//! panics or prints would make this module impossible to probe from a
//! caller that wants to build its OWN error message (e.g. one naming
//! every registered id, not just the one that missed).

use crate::plan_adapter::PlanAdapter;
use crate::plan_writeback::PlanWriteBack;
use crate::plan_adapters::openspec::OpenspecPlanAdapter;
use crate::plan_adapters::superpowers::SuperpowersPlanAdapter;

/// One registered plan-dialect adapter — its [`PlanAdapter`] plus its
/// OPTIONAL [`PlanWriteBack`] capability (s35 `gate-plan-dialect-seam`,
/// design D1). `write_back` is `None` for a dialect that supports plan
/// IMPORT but not the evidence-gated flip; `canon gate task` reports a
/// loud "this source's dialect has no write-back" rather than guessing.
pub struct PlanAdapterEntry {
    pub adapter: &'static dyn PlanAdapter,
    pub write_back: Option<&'static dyn PlanWriteBack>,
}

impl PlanAdapterEntry {
    pub fn dialect_id(&self) -> &'static str {
        self.adapter.dialect_id()
    }
}

/// The static registry, in declaration order (mirrors
/// `crate::artifact_registry::registry`'s "deterministic order, never
/// `HashMap`-iteration-order dependent"). Byte-lexical by
/// `dialect_id()` today (`"openspec"` < `"superpowers"`) — not a
/// requirement of this module (declaration order is authoritative,
/// module doc), but the natural order a new entry lands in when its id
/// happens to sort after every existing one. Both entries carry a
/// [`PlanWriteBack`] (s35): the SAME unit-struct adapter implements
/// both traits, so the write-back impl is `Some(&STATIC)` for each.
pub fn registry() -> &'static [PlanAdapterEntry] {
    static OPENSPEC: OpenspecPlanAdapter = OpenspecPlanAdapter;
    static SUPERPOWERS: SuperpowersPlanAdapter = SuperpowersPlanAdapter;
    const REGISTRY: &[PlanAdapterEntry] = &[
        PlanAdapterEntry { adapter: &OPENSPEC, write_back: Some(&OPENSPEC) },
        PlanAdapterEntry { adapter: &SUPERPOWERS, write_back: Some(&SUPERPOWERS) },
    ];
    REGISTRY
}

/// Look up one registered adapter by `dialect_id()` — the seam
/// `canon ingest plans --dialect <id>` and a `canon.yaml`
/// `plans.sources[].dialect` entry both resolve through. A miss
/// returns `None`; naming the unknown id (and the registered ones) in
/// a loud, operator-facing error is the CLI driver's job (P3), not
/// this registry's (task 1.4).
pub fn find(dialect_id: &str) -> Option<&'static PlanAdapterEntry> {
    registry().iter().find(|entry| entry.dialect_id() == dialect_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contains_exactly_the_openspec_and_superpowers_entries() {
        let entries = registry();
        assert_eq!(entries.len(), 2, "s17 P1 openspec + s30 superpowers");
        assert_eq!(entries[0].dialect_id(), "openspec");
        assert_eq!(entries[1].dialect_id(), "superpowers");
    }

    #[test]
    fn find_locates_the_openspec_entry() {
        let entry = find("openspec").expect("openspec is registered");
        assert_eq!(entry.dialect_id(), "openspec");
    }

    #[test]
    fn find_locates_the_superpowers_entry() {
        let entry = find("superpowers").expect("superpowers is registered");
        assert_eq!(entry.dialect_id(), "superpowers");
    }

    #[test]
    fn find_returns_none_on_a_miss_never_a_loud_error() {
        // The CLI layer (P3) owns the loud "unknown dialect, here are
        // the registered ids" error — the registry itself just answers
        // the lookup (task 1.4).
        assert!(find("no-such-dialect").is_none());
        assert!(find("").is_none());
    }

    #[test]
    fn registry_iteration_order_is_stable_across_calls() {
        // "Never HashMap-iteration-order dependent" — two calls (or two
        // passes, per the connector spec's "Registry iteration order is
        // deterministic" scenario) enumerate identically.
        let first: Vec<&str> = registry().iter().map(|e| e.dialect_id()).collect();
        let second: Vec<&str> = registry().iter().map(|e| e.dialect_id()).collect();
        assert_eq!(first, second);
        assert_eq!(first, vec!["openspec", "superpowers"]);
    }
}
