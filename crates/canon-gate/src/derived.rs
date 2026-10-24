//! What a [`GateContext`] derives from its corpus, computed once per command
//! (0.29 D2).
//!
//! [`GateContext::load`] reads each record kind once. Everything a rule
//! derives from a WHOLE kind (the latest generation of each scenario, review,
//! subject, change and finding; the indexes by subject, scenario and change;
//! the verdict ledger; the archive of superseded evidence; the staleness
//! result) lives here instead, behind a [`OnceLock`], and a per-item question
//! is a lookup into it. Before, each of those was refolded (every record
//! converted to JSON and hashed with SHA-256) once per subject or scenario that
//! asked.
//!
//! # Scale
//! With S subjects, Sc scenarios, C changes, R reviews, E evidence records and
//! A archived records, every fold below is built once: O(Sc + R + E + A) hashes
//! and a few hash maps, held for the command. A per-scenario question reads
//! only that scenario's own rows (about E/Sc evidence records, R/Sc reviews).
//!
//! # Indexes, not references
//! Each structure stores positions in the context's own vectors and the
//! accessors hand out `&'ctx T` borrowed from them, so nothing here is
//! self-referential and nothing is cloned.
//!
//! # Mutating a context
//! The corpus vectors on [`GateContext`] are `pub`. Code that changes one
//! after a derived value was read (a command that grades "the corpus with
//! this record added") MUST call [`GateContext::invalidate_derived`] after the
//! change, or later reads answer for the old corpus.

use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use canon_model::{ChangeId, DivergenceStatus, EvidenceRecord, ProjectId, ScenarioId, SubjectId};
use canon_store::fold::fold_latest_by_key;
use chrono::{DateTime, Utc};

use crate::context::GateContext;
use crate::failure_class::Violation;
use crate::ledger::{CellKey, LedgerEntry};

/// The three values the store's supersession rule orders a record by: its
/// `at`, its envelope `schema`, and the digest of its JSON body (module doc of
/// [`canon_store::fold`]).
pub(crate) struct Stamp {
    at: DateTime<Utc>,
    schema: u32,
    pub(crate) digest: String,
}

impl Stamp {
    /// `None` for a record that does not serialize to an object carrying a
    /// parseable `at`: such a record joins no fold, as before.
    pub(crate) fn of<T: serde::Serialize>(record: &T) -> Option<Self> {
        let body = serde_json::to_value(record).ok()?;
        let at = body.get("at")?.as_str()?.parse::<DateTime<Utc>>().ok()?;
        let schema = body.get("schema").and_then(serde_json::Value::as_u64).unwrap_or(1) as u32;
        let digest = canon_store::partition::content_digest12(&body);
        Some(Self { at, schema, digest })
    }
}

/// [`Stamp::of`] for every record, positionally.
pub(crate) fn stamps_of<T: serde::Serialize>(records: &[T]) -> Vec<Option<Stamp>> {
    records.iter().map(Stamp::of).collect()
}

/// The positions, among `candidates`, of the latest record per `key`, in key
/// order: the one fold ([`fold_latest_by_key`]) over precomputed `stamps`.
pub(crate) fn latest_of<K: Ord>(stamps: &[Option<Stamp>], candidates: impl IntoIterator<Item = usize>, key: impl Fn(usize) -> K) -> Vec<usize> {
    let rows = candidates.into_iter().filter_map(|i| stamps.get(i)?.as_ref().map(|stamp| (i, stamp)));
    fold_latest_by_key(rows, |&(i, _)| key(i), |&(_, stamp)| stamp.at, |&(_, stamp)| stamp.schema, |&(_, stamp)| stamp.digest.as_str())
        .into_values()
        .map(|(i, _)| i)
        .collect()
}

/// The latest generation of every scenario, and how to find one.
pub(crate) struct ScenarioFold {
    /// Positions in `ctx.scenarios`, ordered by `(project_id, scenario_id)`.
    pub(crate) latest: Vec<usize>,
    /// The same positions grouped by the subject the latest generation names,
    /// each group in `latest` order.
    pub(crate) by_subject: HashMap<SubjectId, Vec<usize>>,
    pub(crate) by_key: HashMap<(ProjectId, ScenarioId), usize>,
}

impl ScenarioFold {
    fn build(ctx: &GateContext) -> Self {
        let stamps = stamps_of(&ctx.scenarios);
        let latest = latest_of(&stamps, 0..ctx.scenarios.len(), |i| (ctx.scenarios[i].project_id.clone(), ctx.scenarios[i].scenario_id.clone()));
        let mut by_subject: HashMap<SubjectId, Vec<usize>> = HashMap::new();
        let mut by_key = HashMap::with_capacity(latest.len());
        for &i in &latest {
            let scenario = &ctx.scenarios[i];
            if let Some(subject_id) = &scenario.subject_id {
                by_subject.entry(subject_id.clone()).or_default().push(i);
            }
            by_key.insert((scenario.project_id.clone(), scenario.scenario_id.clone()), i);
        }
        Self { latest, by_subject, by_key }
    }
}

/// The latest generation of every change, and the changes that name a subject.
pub(crate) struct ChangeFold {
    /// Positions in `ctx.changes`, ordered by `change_id`.
    pub(crate) latest: Vec<usize>,
    /// For each subject, the latest changes whose `subject_ids` name it, in
    /// `latest` order.
    pub(crate) naming: HashMap<SubjectId, Vec<usize>>,
}

impl ChangeFold {
    fn build(ctx: &GateContext) -> Self {
        let stamps = stamps_of(&ctx.changes);
        let latest = latest_of(&stamps, 0..ctx.changes.len(), |i| ctx.changes[i].change_id.as_str().to_string());
        let mut naming: HashMap<SubjectId, Vec<usize>> = HashMap::new();
        for &i in &latest {
            for subject_id in &ctx.changes[i].subject_ids {
                let named = naming.entry(subject_id.clone()).or_default();
                if named.last() != Some(&i) {
                    named.push(i);
                }
            }
        }
        Self { latest, naming }
    }
}

/// The latest review per reviewer of each scenario, and the corpus-wide
/// in-effect set.
pub(crate) struct ReviewFold {
    /// Per `(project_id, scenario_id)`: positions in `ctx.reviews` of the
    /// latest review of each reviewer, in reviewer order.
    pub(crate) by_scenario: HashMap<(ProjectId, ScenarioId), Vec<usize>>,
    /// The latest review per `(project_id, scenario_id, change_id, reviewer)`,
    /// in key order.
    pub(crate) in_effect: Vec<usize>,
}

impl ReviewFold {
    fn build(ctx: &GateContext) -> Self {
        let stamps = stamps_of(&ctx.reviews);
        let mut grouped: HashMap<(ProjectId, ScenarioId), Vec<usize>> = HashMap::new();
        for (i, review) in ctx.reviews.iter().enumerate() {
            if let Some(scenario_id) = &review.scenario_id {
                grouped.entry((review.project_id.clone(), scenario_id.clone())).or_default().push(i);
            }
        }
        let by_scenario = grouped.into_iter().map(|(key, group)| (key, latest_of(&stamps, group, |i| ctx.reviews[i].reviewer.clone()))).collect();
        let in_effect = latest_of(&stamps, 0..ctx.reviews.len(), |i| {
            let r = &ctx.reviews[i];
            (r.project_id.clone(), r.scenario_id.clone(), r.change_id.clone(), r.reviewer.clone())
        });
        Self { by_scenario, in_effect }
    }
}

/// The ledger's declared evidence, positionally stamped and grouped by the
/// scenario id it names.
pub(crate) struct EvidenceIndex {
    pub(crate) stamps: Vec<Option<Stamp>>,
    /// Positions in `ctx.evidence`, in ledger order.
    pub(crate) by_scenario: HashMap<String, Vec<usize>>,
}

impl EvidenceIndex {
    fn build(ctx: &GateContext) -> Self {
        Self { stamps: stamps_of(&ctx.evidence), by_scenario: group_by_scenario(&ctx.evidence) }
    }
}

/// The archive's declared evidence, read once.
pub(crate) struct Archive {
    pub(crate) records: Vec<EvidenceRecord>,
    pub(crate) by_scenario: HashMap<String, Vec<usize>>,
}

impl Archive {
    fn build(ctx: &GateContext) -> Self {
        let records = crate::history::read_archive(ctx);
        let by_scenario = group_by_scenario(&records);
        Self { records, by_scenario }
    }
}

fn group_by_scenario(records: &[EvidenceRecord]) -> HashMap<String, Vec<usize>> {
    let mut grouped: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, record) in records.iter().enumerate() {
        if let Some(scenario_id) = &record.scenario_id {
            grouped.entry(scenario_id.as_str().to_string()).or_default().push(i);
        }
    }
    grouped
}

/// Everything a [`GateContext`] derives from its corpus (module doc).
#[derive(Default)]
pub struct Derived {
    scenarios: OnceLock<ScenarioFold>,
    subjects: OnceLock<Vec<usize>>,
    changes: OnceLock<ChangeFold>,
    change_subjects: OnceLock<HashMap<ChangeId, Vec<usize>>>,
    reviews: OnceLock<ReviewFold>,
    pub(crate) current_reviews: OnceLock<Vec<usize>>,
    findings: OnceLock<Vec<usize>>,
    evidence: OnceLock<EvidenceIndex>,
    acceptances: OnceLock<HashMap<(ProjectId, ScenarioId), usize>>,
    pub(crate) verdicts: OnceLock<BTreeMap<CellKey, LedgerEntry>>,
    archive: OnceLock<Archive>,
    staleness: OnceLock<Vec<Violation>>,
}

impl GateContext {
    /// Forget every derived value. REQUIRED after changing a corpus vector of
    /// a context whose derived values may already have been read (module doc).
    pub fn invalidate_derived(&mut self) {
        self.derived = Derived::default();
    }

    /// Forget what derives from `self.reviews` alone: for a caller that swaps
    /// the reviews many times over one large, unchanged evidence corpus and
    /// must not pay to re-derive that corpus each time.
    pub fn invalidate_reviews(&mut self) {
        self.derived.reviews = OnceLock::new();
        self.derived.current_reviews = OnceLock::new();
    }

    /// Forget what derives from `self.evidence`, after it changed.
    pub fn invalidate_evidence(&mut self) {
        self.derived.evidence = OnceLock::new();
        self.derived.verdicts = OnceLock::new();
        self.derived.staleness = OnceLock::new();
        self.derived.current_reviews = OnceLock::new();
    }

    pub(crate) fn scenario_fold(&self) -> &ScenarioFold {
        self.derived.scenarios.get_or_init(|| ScenarioFold::build(self))
    }

    /// Positions in `self.subjects` of the latest record of every subject,
    /// ordered by `subject_id`.
    pub(crate) fn subject_fold(&self) -> &[usize] {
        self.derived.subjects.get_or_init(|| {
            let stamps = stamps_of(&self.subjects);
            latest_of(&stamps, 0..self.subjects.len(), |i| self.subjects[i].subject_id.as_str().to_string())
        })
    }

    pub(crate) fn change_fold(&self) -> &ChangeFold {
        self.derived.changes.get_or_init(|| ChangeFold::build(self))
    }

    /// For each change, the positions in `self.subjects` of the latest
    /// subjects whose [`crate::spec_coverage::subject_change_ids`] hold it, in
    /// subject order.
    pub(crate) fn change_subject_fold(&self) -> &HashMap<ChangeId, Vec<usize>> {
        self.derived.change_subjects.get_or_init(|| {
            let mut holders: HashMap<ChangeId, Vec<usize>> = HashMap::new();
            for &i in self.subject_fold() {
                for change_id in crate::spec_coverage::subject_change_ids(self, &self.subjects[i]) {
                    let held = holders.entry(change_id).or_default();
                    if held.last() != Some(&i) {
                        held.push(i);
                    }
                }
            }
            holders
        })
    }

    pub(crate) fn review_fold(&self) -> &ReviewFold {
        self.derived.reviews.get_or_init(|| ReviewFold::build(self))
    }

    /// Positions in `self.findings` of the latest version of each finding,
    /// in `(change_id, round, seq)` order.
    pub(crate) fn finding_fold(&self) -> &[usize] {
        self.derived.findings.get_or_init(|| {
            let stamps = stamps_of(&self.findings);
            latest_of(&stamps, 0..self.findings.len(), |i| (self.findings[i].change_id.as_str().to_string(), self.findings[i].round, self.findings[i].seq))
        })
    }

    pub(crate) fn evidence_index(&self) -> &EvidenceIndex {
        self.derived.evidence.get_or_init(|| EvidenceIndex::build(self))
    }

    /// The position in `self.divergences` of each scenario's latest
    /// acceptance, by record time then `(run_seq, round)`; the last of equals
    /// wins.
    pub(crate) fn acceptance_fold(&self) -> &HashMap<(ProjectId, ScenarioId), usize> {
        self.derived.acceptances.get_or_init(|| {
            let mut latest: HashMap<(ProjectId, ScenarioId), usize> = HashMap::new();
            for (i, d) in self.divergences.iter().enumerate() {
                if !matches!(d.status, DivergenceStatus::Accepted { .. }) {
                    continue;
                }
                let order = |d: &canon_model::Divergence| (d.envelope.at, d.run_seq, d.round);
                let slot = latest.entry((d.project_id.clone(), d.scenario_id.clone())).or_insert(i);
                if order(d) >= order(&self.divergences[*slot]) {
                    *slot = i;
                }
            }
            latest
        })
    }

    pub(crate) fn archive(&self) -> &Archive {
        self.derived.archive.get_or_init(|| Archive::build(self))
    }

    /// What [`crate::staleness::StalenessCheck`] reports for this context,
    /// computed once: the check itself and the risk check, which needs the
    /// stale subjects, share it.
    pub fn staleness_violations(&self) -> &[Violation] {
        self.derived.staleness.get_or_init(|| crate::staleness::compute(self))
    }
}
