//! `canon tier age [--commit --plan-digest D]` (S2 task 3.3, preview and
//! commit 0.28 D6): a committed run calls
//! `canon_store::registry::TierRegistry::age_all()` — the
//! digest-keyed, write-then-destination-confirms-then-delete aging
//! mechanism already lives there (tier-policy spec); this module never
//! reimplements it. The bare run ([`preview`]) lists what a committed run
//! would select via a read-only [`canon_store::tier::Tier::read`] + the same
//! `after`-threshold predicate `Tier::age` applies internally, without
//! ever writing or deleting.


use canon_model::envelope::RecordKind;
use canon_store::policy::Rung;
use canon_store::tier::{StoreError, TierQuery};
use chrono::{DateTime, Duration, Utc};

use crate::json_out::Report;
use crate::safety::{Effect, Preview};
use crate::tiers::{self, TierCliError};
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum TierAgeError {
    #[error(transparent)]
    Tiers(#[from] TierCliError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// One `aging:` entry's outcome: what a preview found past the threshold
/// (`candidates`) and, once committed, what the run did with it.
#[derive(Debug)]
pub struct AgeRuleReport {
    pub kind: RecordKind,
    pub source: Rung,
    pub destination: Rung,
    pub after: Duration,
    /// Records in the source rung older than the threshold when the plan was made.
    pub candidates: usize,
    /// Records newly written to the destination and removed from the source (0 in a preview).
    pub moved: usize,
    /// Records the destination already held (0 in a preview: it never checks the destination).
    pub already_aged: usize,
}

/// What [`preview`] found: the report of each aging rule, the `move` effects and every record read.
pub type Planned = (Vec<AgeRuleReport>, Vec<Effect>, Vec<String>);

/// The plan: per aging rule, the records `age_all` would move, as `move`
/// effects (`<kind>:<id>:<digest12>` and where to), and every record it read to pick them (`<rung>:<kind>:<id>:<digest>`, the
/// set the plan's digest covers). Reads only.
pub fn preview(loaded: &tiers::LoadedTiers) -> Result<Planned, TierAgeError> {
    let mut entries: Vec<_> = loaded.policy.aging.iter().collect();
    entries.sort_by_key(|(kind, _)| kind.as_str());

    let now: DateTime<Utc> = Utc::now();
    let mut reports = Vec::with_capacity(entries.len());
    let mut effects = Vec::new();
    let mut scanned = Vec::new();
    for (kind, rule) in entries {
        let source = loaded.policy.tier_for(*kind)?;
        let cutoff = now - rule.after;
        let result = tiers::read_tier(source, loaded, &TierQuery::kind(*kind))?;
        let mut ids: Vec<String> = Vec::new();
        for raw in &result.records {
            let id = canon_store::partition::resolve_partition(*kind, &raw.0).ok().map(|p| p.natural_key).unwrap_or_default();
            let id = format!("{}:{id}:{}", kind.as_str(), canon_store::partition::content_digest(&raw.0));
            scanned.push(format!("{}:{id}", source.as_str()));
            if canon_store::tier::raw_record_at(raw) < cutoff {
                ids.push(id);
            }
        }
        ids.sort();
        reports.push(AgeRuleReport { kind: *kind, source, destination: rule.to, after: rule.after, candidates: ids.len(), moved: 0, already_aged: 0 });
        effects.extend(ids.into_iter().map(|id| Effect::record("move", id).detail(format!("{} -> {}", source.as_str(), rule.to.as_str()))));
    }
    Ok((reports, effects, scanned))
}

/// Apply every aging rule (the real run), reporting `candidates` from `planned`.
pub fn apply(loaded: tiers::LoadedTiers, planned: &[AgeRuleReport]) -> Result<Vec<AgeRuleReport>, TierAgeError> {
    let policy = loaded.policy.clone();
    let registry = loaded.into_registry();
    let age_reports = registry.age_all()?;
    // `tier-age-after-move` (a debug build's test seam, `incomplete::injected_stop`): the aging stops once the records are moved.
    if let Some(stop) = crate::incomplete::injected_stop("tier-age-after-move") {
        return Err(TierAgeError::Store(StoreError::Io(stop)));
    }

    let reports = age_reports
        .into_iter()
        .map(|r| {
            let source = policy.tier_for(r.kind).expect("age_all only reports kinds it resolved a source tier for");
            let rule = policy.aging.get(&r.kind).expect("age_all only reports kinds with an `aging` entry");
            let candidates = planned.iter().find(|p| p.kind == r.kind).map_or(0, |p| p.candidates);
            AgeRuleReport { kind: r.kind, source, destination: rule.to, after: rule.after, candidates, moved: r.moved, already_aged: r.already_aged }
        })
        .collect::<Vec<AgeRuleReport>>();
    if reports.iter().all(|report| report.moved == 0) {
        crate::json_out::mark_unchanged();
    }
    Ok(reports)
}

fn format_duration(d: Duration) -> String {
    let days = d.num_days();
    if days > 0 && Duration::days(days) == d {
        return format!("{days}d");
    }
    let hours = d.num_hours();
    if hours > 0 && Duration::hours(hours) == d {
        return format!("{hours}h");
    }
    let minutes = d.num_minutes();
    if minutes > 0 && Duration::minutes(minutes) == d {
        return format!("{minutes}m");
    }
    format!("{}s", d.num_seconds())
}

/// The human report `canon tier age` prints after a commit: what moved per
/// `aging:` entry, in the same kind-name-sorted order `age_all()` itself
/// iterates in.
pub fn format_report(reports: &[AgeRuleReport]) -> String {
    let mut out = String::new();
    if reports.is_empty() {
        out.push_str("canon tier age: no `aging` rules configured in canon.yaml — nothing to do.\n");
        return out;
    }
    out.push_str(&format!("canon tier age: {} aging rule(s) applied\n\n", reports.len()));
    for r in reports {
        out.push_str(&format!("{}  {} -> {}  (after {})\n", r.kind.as_str(), r.source.as_str(), r.destination.as_str(), format_duration(r.after)));
        out.push_str(&format!("  moved: {}, already_aged: {}\n\n", r.moved, r.already_aged));
    }
    out
}

/// One `aging:` rule in `canon tier age`'s report.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TierAgeRule {
    pub kind: &'static str,
    pub source: &'static str,
    pub destination: &'static str,
    /// The rule's threshold (`30d`).
    pub after: String,
    pub candidates: usize,
    pub moved: usize,
    pub already_aged: usize,
}

/// `canon tier age`'s `data`: the plan and one entry per `aging:` rule.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TierAgeReport {
    pub rules: Vec<TierAgeRule>,
    #[serde(flatten)]
    pub plan: Preview,
}

impl Report for TierAgeReport {
    /// 1.1: the plan is `atip::plan::Preview` (`planDigest` is `sha256:` and 64 hex digits).
    const SCHEMA_VERSION: &'static str = "1.1";
}

pub fn report(reports: &[AgeRuleReport], plan: Preview) -> TierAgeReport {
    let rules = reports
        .iter()
        .map(|r| TierAgeRule {
            kind: r.kind.as_str(),
            source: r.source.as_str(),
            destination: r.destination.as_str(),
            after: format_duration(r.after),
            candidates: r.candidates,
            moved: r.moved,
            already_aged: r.already_aged,
        })
        .collect();
    TierAgeReport { rules, plan }
}
