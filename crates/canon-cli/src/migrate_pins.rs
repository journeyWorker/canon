//! `canon migrate`'s review pinning (0.23 D6): rewrites the reviews and
//! accepted divergences that predate 0.23's pins to what the evidence says
//! now, so the gate needs one freshness rule and no compatibility branch.
//!
//! # What is converted
//! The gate reads one pin: the scenario's spec digest and, per role, the
//! verdict and checks of its latest evidence ([`canon_gate::freshness`]).
//! Two older shapes cannot be compared that way:
//!
//! - **0.15–0.22 reviews and acceptances** pin the evidence *record files*
//!   they saw. Current under that rule means the spec pin still matches the
//!   scenario and the latest evidence record per role is the pinned file.
//! - **pre-0.15 reviews** pin nothing, only the commit they reviewed.
//!   Current means the scenario's digest at that commit is its digest now
//!   ([`crate::migrate_legacy_pins`], read out of git) and no declared
//!   evidence is newer than the review. A commit or scenario this clone
//!   cannot read leaves the review undecidable; it counted unchecked before
//!   0.23, so it is pinned as it stands, and the count says how many.
//!
//! A *current* one is rewritten in place with the 0.23 pin of the scenario
//! as it reads now, keeping its envelope, reviewer and artifacts. A *stale*
//! one is left alone: it stays stale (the gate reads a review without the
//! 0.23 pin as stale and says why), and the scenario needs a new review, as
//! it did before. A review of a scenario no `.feature` file declares any
//! more reviews nothing the gate reads, and is left alone too.
//!
//! The old record is replaced, not appended to: the new one carries the
//! same `at`, so two versions would tie, and git keeps the history.
//! Running it again finds nothing more to convert.

use std::collections::BTreeMap;
use std::path::Path;

use canon_gate::freshness::{current_checks, current_pins, scenario_evidence};
use canon_gate::review_gate::in_effect_reviews;
use canon_gate::spec_coverage::{latest_by_key, latest_scenarios};
use canon_gate::{GateContext, GateCtx};
use canon_model::{Divergence, DivergenceStatus, EvidencePin, EvidenceRecord, PinnedSpec, ProjectId, RecordKind, Review, Scenario, ScenarioId};
use canon_policy::SchemaRegistry;
use canon_store::git_tier::GitTier;
use canon_store::partition::{content_digest12, expected_relative_path, resolve_partition};
use canon_store::tier::{Tier, TierQuery};
use chrono::Utc;

use crate::migrate_legacy_pins::{self, LegacyPins, PinnedScenario};

/// How one old review or acceptance stands under the rule it was written
/// under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Currency {
    Current,
    Stale,
    /// Only a pre-0.15 review: its commit or scenario at it is not here.
    Undecidable,
}

/// What a run found and (unless it was a dry run) did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Reviews pinned to evidence record files, now rewritten.
    pub record_pinned: usize,
    /// Pre-0.15 reviews that checked out against their commit, now rewritten.
    pub unpinned_current: usize,
    /// Pre-0.15 reviews whose commit could not be read, rewritten as they stand.
    pub undecidable: usize,
    /// Old reviews left as they are because they were already stale.
    pub stale: usize,
    /// Old reviews of a scenario no `.feature` file declares any more.
    pub gone: usize,
    /// Accepted divergences rewritten.
    pub acceptances: usize,
    /// Accepted divergences left stale.
    pub acceptances_stale: usize,
}

impl Report {
    fn converted(&self) -> usize {
        self.record_pinned + self.unpinned_current + self.undecidable
    }

    /// The lines `canon migrate` prints: nothing when nothing was old.
    pub fn lines(&self, dry_run: bool) -> Vec<String> {
        let mut lines = Vec::new();
        let verb = if dry_run { "would pin" } else { "pinned" };
        if self.converted() > 0 {
            let mut line = format!(
                "canon migrate: {verb} {} review(s) to what their evidence says (the verdict and the checks of its latest record): {} were pinned to evidence records, {} to a commit before 0.15",
                self.converted(),
                self.record_pinned,
                self.unpinned_current + self.undecidable
            );
            if self.undecidable > 0 {
                line.push_str(&format!(
                    "; {} of those could not be checked because their commit, or the scenario at it, is not in this clone, so they are pinned as the scenario stands (check out full history, `fetch-depth: 0` in CI, to check them)",
                    self.undecidable
                ));
            }
            lines.push(line);
        }
        if self.stale > 0 {
            lines.push(format!(
                "canon migrate: left {} stale review(s) as they are: the scenario or its evidence changed since the review, so each stays stale until it is reviewed again (`canon review scenario`)",
                self.stale
            ));
        }
        if self.gone > 0 {
            lines.push(format!("canon migrate: left {} review(s) of scenarios no .feature file declares any more", self.gone));
        }
        if self.acceptances > 0 {
            lines.push(format!("canon migrate: {verb} {} accepted divergence(s) to what their evidence says", self.acceptances));
        }
        if self.acceptances_stale > 0 {
            lines.push(format!("canon migrate: left {} stale accepted divergence(s) as they are; each needs `canon divergence accept` again", self.acceptances_stale));
        }
        lines
    }
}

/// Convert every old-pinned review and acceptance of `repo` (module doc).
/// `apply: false` reports what would change and writes nothing.
pub fn run(repo: &Path, apply: bool) -> Result<Report, String> {
    let ctx = GateCtx::from_repo(repo).map_err(|e| e.to_string())?;
    let ctx = GateContext::load(ctx, &SchemaRegistry::load(), Utc::now()).map_err(|e| e.to_string())?;
    let live: BTreeMap<(&ProjectId, &ScenarioId), &Scenario> = latest_scenarios(&ctx).into_iter().map(|s| ((&s.project_id, &s.scenario_id), s)).collect();
    let tier = GitTier::new(&ctx.ctx.ledger_root);
    let file_digests = evidence_file_digests(&tier)?;
    let legacy = migrate_legacy_pins::resolve(&ctx, || crate::inventory::spec_sources(repo));
    let mut report = Report::default();

    let on_disk = records_of::<Review>(&tier, RecordKind::Review)?;
    for review in in_effect_reviews(&ctx).into_iter().filter(|r| r.pins.as_ref().is_none_or(|p| p.checks.is_none())) {
        let Some(scenario_id) = &review.scenario_id else { continue };
        let Some(scenario) = live.get(&(&review.project_id, scenario_id)).copied() else {
            report.gone += 1;
            continue;
        };
        let currency = review_currency(&ctx, review, scenario, &legacy, &file_digests);
        match (currency, review.pins.is_some()) {
            (Currency::Stale, _) => report.stale += 1,
            (Currency::Current, true) => report.record_pinned += 1,
            (Currency::Current, false) => report.unpinned_current += 1,
            (Currency::Undecidable, _) => report.undecidable += 1,
        }
        if currency == Currency::Stale || !apply {
            continue;
        }
        let mut pinned = review.clone();
        pinned.pins = Some(current_pins(&ctx, scenario));
        replace(&tier, &ctx.ctx.ledger_root, &on_disk, review, &pinned, RecordKind::Review)?;
    }

    let on_disk = records_of::<Divergence>(&tier, RecordKind::Divergence)?;
    for scenario in latest_scenarios(&ctx) {
        let Some(latest) = ctx
            .divergences
            .iter()
            .filter(|d| d.project_id == scenario.project_id && d.scenario_id == scenario.scenario_id && matches!(d.status, DivergenceStatus::Accepted { .. }))
            .max_by_key(|d| (d.envelope.at, d.run_seq, d.round))
        else {
            continue;
        };
        let DivergenceStatus::Accepted { reason, tracking, expiry, checks: None, evidence } = &latest.status else { continue };
        let mut pinned_files = evidence.clone();
        pinned_files.sort();
        if pinned_files != legacy_evidence_pins(&ctx, scenario, &file_digests) {
            report.acceptances_stale += 1;
            continue;
        }
        report.acceptances += 1;
        if !apply {
            continue;
        }
        let mut pinned = latest.clone();
        pinned.status = DivergenceStatus::Accepted { reason: reason.clone(), tracking: tracking.clone(), expiry: *expiry, checks: Some(current_checks(&ctx, scenario)), evidence: Vec::new() };
        replace(&tier, &ctx.ctx.ledger_root, &on_disk, latest, &pinned, RecordKind::Divergence)?;
    }
    Ok(report)
}

/// Whether `review` is current under the rule it was written under.
fn review_currency(ctx: &GateContext, review: &Review, scenario: &Scenario, legacy: &LegacyPins, file_digests: &BTreeMap<String, String>) -> Currency {
    match &review.pins {
        Some(pins) => {
            let spec_current = match pins.scenario_digest_scope {
                PinnedSpec::File => pins.scenario_digest == scenario.source_digest,
                PinnedSpec::Scenario => scenario.scenario_digest.as_ref() == Some(&pins.scenario_digest),
                PinnedSpec::Brief => false,
            };
            let mut pinned = pins.evidence.clone();
            pinned.sort();
            if spec_current && pinned == legacy_evidence_pins(ctx, scenario, file_digests) { Currency::Current } else { Currency::Stale }
        }
        None => {
            let (Some(PinnedScenario::Digest(at_pin)), Some(now)) = (legacy.get(review), scenario.scenario_digest.as_ref()) else {
                return Currency::Undecidable;
            };
            let newer_evidence = scenario_evidence(ctx, scenario).iter().any(|e| e.envelope.at > review.envelope.at);
            if at_pin != now || newer_evidence { Currency::Stale } else { Currency::Current }
        }
    }
}

/// What a 0.15–0.22 review or acceptance of `scenario` pinned right now:
/// the latest evidence record per role, each named by its ledger file, the
/// natural key and digest of `kind=evidence_record/<key>__<digest>.json`.
fn legacy_evidence_pins(ctx: &GateContext, scenario: &Scenario, file_digests: &BTreeMap<String, String>) -> Vec<EvidencePin> {
    let evidence = scenario_evidence(ctx, scenario);
    let mut pins: Vec<EvidencePin> = latest_by_key(&evidence, |e| e.envelope.actor.role.as_ref().map(|r| r.as_str().to_string()))
        .into_iter()
        .map(|record| {
            let body = serde_json::to_value(record).unwrap_or_default();
            let key = resolve_partition(RecordKind::EvidenceRecord, &body).expect("an evidence record's natural key always resolves (`unscoped` at worst)").natural_key;
            let typed = content_digest12(&body);
            let digest = file_digests.get(&typed).cloned().unwrap_or(typed);
            EvidencePin { key, digest }
        })
        .collect();
    pins.sort();
    pins
}

/// The ledger file digest of each evidence record (the 12 hex characters
/// of `<natural_key>__<digest>.json`), keyed by the content digest of the
/// record as deserialized: the typed record leaves out raw-only keys
/// (`evidence.ref`, `staging_id`, …), so its own digest is not its file's.
fn evidence_file_digests(tier: &GitTier) -> Result<BTreeMap<String, String>, String> {
    let read = tier.read(&TierQuery::kind(RecordKind::EvidenceRecord)).map_err(|e| format!("cannot read the evidence ledger: {e}"))?;
    Ok(read
        .records
        .iter()
        .filter_map(|raw| {
            let typed: EvidenceRecord = serde_json::from_value(raw.0.clone()).ok()?;
            let typed = serde_json::to_value(&typed).ok()?;
            Some((content_digest12(&typed), content_digest12(&raw.0)))
        })
        .collect())
}

/// Every record of `kind` in the ledger that reads as `T`, with the path it
/// is filed at (relative to the ledger root).
fn records_of<T: serde::de::DeserializeOwned>(tier: &GitTier, kind: RecordKind) -> Result<Vec<(T, String)>, String> {
    let read = tier.read(&TierQuery::kind(kind)).map_err(|e| format!("cannot read the {} ledger: {e}", kind.as_str()))?;
    Ok(read
        .records
        .iter()
        .filter_map(|raw| Some((serde_json::from_value::<T>(raw.0.clone()).ok()?, expected_relative_path(kind, &raw.0).ok()?.display().to_string())))
        .collect())
}

/// Write `new` and remove the file `old` was read from. Unarchived: the
/// migrated record keeps `old`'s time, so the archive's latest-version
/// rule could pick either copy; this replacement removes `old` itself.
fn replace<T: canon_model::CanonRecord + PartialEq + serde::Serialize>(tier: &GitTier, ledger_root: &Path, on_disk: &[(T, String)], old: &T, new: &T, kind: RecordKind) -> Result<(), String> {
    let old_path = on_disk.iter().find(|(record, _)| record == old).map(|(_, path)| path.clone());
    tier.write_unarchived(new).map_err(|e| format!("cannot write the migrated {}: {e}", kind.as_str()))?;
    if let Some(path) = old_path {
        std::fs::remove_file(ledger_root.join(&path)).map_err(|e| format!("cannot replace {path}: {e}"))?;
    }
    Ok(())
}
