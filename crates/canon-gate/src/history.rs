//! The gate's explicit history reads (0.23 D2).
//!
//! The ledger holds the current record of each key; superseded versions
//! live in the archive beside it ([`canon_store::archive`]). A gate rule
//! reads the ledger only, through [`crate::GateContext`], unless it
//! genuinely needs a superseded version. These are the rules that do, and
//! the only places the gate opens the archive:
//!
//! - `coverage`: a `risk_routing` rule applies to an artifact when ANY of
//!   its evidence records satisfies the rule's predicate, and a required
//!   role is covered when ANY record carries it. Superseded records
//!   count, as they always did ([`evidence_history`]).
//! - `divergence accept` ([`crate::acceptance::refusal`],
//!   [`crate::acceptance::evidence_sha`]): the accepting reviewer must be
//!   independent of EVERY actor and session that attested the scenario,
//!   superseded attestations included, and the acceptance's `sha` is the
//!   newest evidence that carries one, which an earlier record can be.
//! - `release-trust-required` ([`crate::trust::ReleaseTrustCheck`]): every
//!   class-tagged record is held to the required trust, superseded or not
//!   (it reads [`canon_store::git_tier::GitTier::read_history`] itself).
//! - `canon gate task` (canon-cli): a task row counts every record bound
//!   to the task by verdict and scans every note it carries.
//!
//! Every other rule reads fields on the latest record, or a record the
//! archive keeps in the ledger because a rule reads it unfolded (a flagged
//! overlay, a `reviewed` lifecycle tag, a brief override, an approval at
//! the current evidence generation; see [`canon_store::archive`]).

use canon_model::{validate_evidence_batch, EvidenceRecord, RecordKind, Scenario};
use canon_store::tier::{Tier, TierQuery};

use crate::context::GateContext;

/// Every declared (non-audit) evidence record: the ledger's
/// (`ctx.evidence`) and the archive's, borrowed from the context. Order: the
/// ledger's first.
#[derive(Debug, Clone, Copy)]
pub struct EvidenceHistory<'a> {
    ledger: &'a [EvidenceRecord],
    archive: &'a [EvidenceRecord],
}

impl<'a> EvidenceHistory<'a> {
    pub fn iter(&self) -> impl Iterator<Item = &'a EvidenceRecord> + 'a {
        self.ledger.iter().chain(self.archive)
    }

    pub fn len(&self) -> usize {
        self.ledger.len() + self.archive.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// An owned copy, for a caller that must keep the records past the context.
    pub fn to_vec(&self) -> Vec<EvidenceRecord> {
        self.iter().cloned().collect()
    }
}

/// The ledger's and the archive's declared evidence, borrowed. The archive
/// is read the first time any rule asks and held by the context.
pub fn evidence_history(ctx: &GateContext) -> EvidenceHistory<'_> {
    EvidenceHistory { ledger: &ctx.evidence, archive: archived_evidence(ctx) }
}

/// The archive's declared evidence records alone, read once per context. A
/// repo with no archive, or an archive that cannot be read, contributes
/// none: the gate's own load already reported what the ledger could not give
/// it, and a rule that reads history must never fail where the ledger read
/// succeeded.
pub fn archived_evidence(ctx: &GateContext) -> &[EvidenceRecord] {
    &ctx.archive().records
}

/// Every declared evidence record of `scenario`, ledger first then archive:
/// those naming its id whose `project_id` is its own or absent. A lookup in
/// the per-scenario index, not a scan of the history.
pub fn scenario_history<'a>(ctx: &'a GateContext, scenario: &Scenario) -> Vec<&'a EvidenceRecord> {
    let id = scenario.scenario_id.as_str();
    let archive = ctx.archive();
    let ledger_rows = ctx.evidence_index().by_scenario.get(id).into_iter().flatten().map(|&i| &ctx.evidence[i]);
    let archive_rows = archive.by_scenario.get(id).into_iter().flatten().map(|&i| &archive.records[i]);
    ledger_rows.chain(archive_rows).filter(|e| e.project_id.as_ref().is_none_or(|p| *p == scenario.project_id)).collect()
}

/// What [`archived_evidence`] caches: read the archive and drop what the
/// ledger would have refused or classed as audit.
pub(crate) fn read_archive(ctx: &GateContext) -> Vec<EvidenceRecord> {
    let Some(archive) = canon_store::git_tier::GitTier::new(&ctx.ctx.ledger_root).archive_tier() else { return Vec::new() };
    let Ok(read) = archive.read(&TierQuery::kind(RecordKind::Evidence)) else { return Vec::new() };
    let (records, _violations) = validate_evidence_batch(&read.records);
    records.into_iter().filter(|record| !record.is_audit()).collect()
}
