//! `canon tier age [--dry-run]` (S2 task 3.3): a real run calls
//! `canon_store::registry::TierRegistry::age_all()` — the
//! digest-keyed, write-then-destination-confirms-then-delete aging
//! mechanism already lives there (tier-policy spec); this module never
//! reimplements it. `--dry-run` previews what a real run would select
//! via a read-only [`canon_store::tier::Tier::read`] + the same
//! `after`-threshold predicate `Tier::age` applies internally, without
//! ever writing or deleting.

use std::path::Path;

use canon_model::envelope::RecordKind;
use canon_store::policy::Rung;
use canon_store::registry::TierRegistry;
use canon_store::tier::{StoreError, TierQuery};
use chrono::{DateTime, Duration, Utc};

use crate::tiers::{self, TierCliError};

#[derive(Debug, thiserror::Error)]
pub enum TierAgeError {
    #[error(transparent)]
    Tiers(#[from] TierCliError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// One `aging:` entry's outcome — a real move (`dry_run: false`,
/// `moved`/`already_aged` straight from [`canon_store::tier::AgeReport`])
/// or a read-only candidate count (`dry_run: true`, `moved` holds the
/// "would move" count and `already_aged` is always 0: a dry run never
/// performs the destination digest check a real write does).
pub struct AgeRuleReport {
    pub kind: RecordKind,
    pub source: Rung,
    pub destination: Rung,
    pub after: Duration,
    pub moved: usize,
    pub already_aged: usize,
    pub dry_run: bool,
}

pub fn run(canon_yaml: &Path, dry_run: bool) -> Result<Vec<AgeRuleReport>, TierAgeError> {
    let loaded = tiers::build_tiers(canon_yaml)?;

    if dry_run {
        run_dry(loaded)
    } else {
        run_real(loaded)
    }
}

fn run_dry(loaded: tiers::LoadedTiers) -> Result<Vec<AgeRuleReport>, TierAgeError> {
    let mut entries: Vec<_> = loaded.policy.aging.iter().collect();
    entries.sort_by_key(|(kind, _)| kind.as_str());

    let now: DateTime<Utc> = Utc::now();
    let mut reports = Vec::with_capacity(entries.len());
    for (kind, rule) in entries {
        let source = loaded.policy.tier_for(*kind)?;
        let cutoff = now - rule.after;
        let result = tiers::read_tier(source, &loaded, &TierQuery::kind(*kind))?;
        let candidates = result.records.iter().filter(|raw| canon_store::tier::raw_record_at(raw) < cutoff).count();
        reports.push(AgeRuleReport { kind: *kind, source, destination: rule.to, after: rule.after, moved: candidates, already_aged: 0, dry_run: true });
    }
    Ok(reports)
}

fn run_real(loaded: tiers::LoadedTiers) -> Result<Vec<AgeRuleReport>, TierAgeError> {
    let policy = loaded.policy.clone();
    let registry = TierRegistry::new(loaded.policy, loaded.git, loaded.pg, loaded.r2, loaded.sqlite);
    let age_reports = registry.age_all()?;

    let reports = age_reports
        .into_iter()
        .map(|r| {
            let source = policy.tier_for(r.kind).expect("age_all only reports kinds it resolved a source tier for");
            let rule = policy.aging.get(&r.kind).expect("age_all only reports kinds with an `aging` entry");
            AgeRuleReport { kind: r.kind, source, destination: rule.to, after: rule.after, moved: r.moved, already_aged: r.already_aged, dry_run: false }
        })
        .collect();
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

/// The human report `canon tier age` prints — what moved (or, under
/// `--dry-run`, what would move) per `aging:` entry, in the same
/// kind-name-sorted order `age_all()` itself iterates in.
pub fn format_report(reports: &[AgeRuleReport], dry_run: bool) -> String {
    let mut out = String::new();
    if reports.is_empty() {
        out.push_str("canon tier age: no `aging` rules configured in canon.yaml — nothing to do.\n");
        return out;
    }

    if dry_run {
        out.push_str(&format!("canon tier age --dry-run: {} aging rule(s), no writes performed\n\n", reports.len()));
    } else {
        out.push_str(&format!("canon tier age: {} aging rule(s) applied\n\n", reports.len()));
    }

    for r in reports {
        out.push_str(&format!("{}  {} -> {}  (after {})\n", r.kind.as_str(), r.source.as_str(), r.destination.as_str(), format_duration(r.after)));
        if r.dry_run {
            out.push_str(&format!("  would move: {} candidate(s)\n\n", r.moved));
        } else {
            out.push_str(&format!("  moved: {}, already_aged: {}\n\n", r.moved, r.already_aged));
        }
    }
    out
}
