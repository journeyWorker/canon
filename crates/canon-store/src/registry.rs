//! `TierRegistry`: resolves a parsed [`TierPolicy`] into live [`Tier`]
//! handles and is the ONE place a caller actually persists/queries/ages
//! records — the backing implementation `canon tier age` (tier-policy
//! spec) and `canon query` (unified-query spec) dispatch to.
//! `crates/canon-cli/src/tier.rs`/`query.rs` (S2, commit `5aa36920`)
//! wire those two subcommands onto this crate's library API; see
//! `canon/skills/tiered-storage/SKILL.md`'s "Running `canon tier age`
//! / `canon query`" section for real invocations.
//!
//! # Rung-keyed storage (s27 `tier-role-backend-split`, design D5)
//! Three named RUNG fields (`local`/`hot`/`cold`), each an
//! `Arc<dyn Tier>` — the concrete backend adapter (`GitTier`/`PgTier`/
//! `R2Tier`) `canon.yaml`'s `tiers.<rung>.backend` tag selected for
//! that rung. A dedicated `git` field is kept alongside (design D5's
//! trade-off mitigation) for a caller needing the git ADAPTER
//! specifically (e.g. `--plugin`'s git-tree resolution), independent
//! of which rung(s) actually route to it.

use std::sync::Arc;

use canon_model::envelope::{CanonRecord, RecordKind};

use crate::git_tier::GitTier;
use crate::pg_tier::PgTier;
use crate::policy::{Backend, BackendConfig, Rung, TierPolicy};
use crate::r2_tier::R2Tier;
use crate::sqlite_tier::SqliteTier;
use crate::tier::{AgeReport, AgingRule, StoreError, StoredRecord, Tier, TierQuery, TierReadResult, WriteReceipt};

pub struct TierRegistry {
    policy: TierPolicy,
    local: Option<Arc<dyn Tier>>,
    hot: Option<Arc<dyn Tier>>,
    cold: Option<Arc<dyn Tier>>,
    git: Option<Arc<GitTier>>,
}

impl TierRegistry {
    /// Resolve `policy` into live rung handles. `git`/`pg`/`r2`/
    /// `sqlite` are the (at most one each, by today's convention —
    /// design D1's own "not a type-level constraint" caveat) already-
    /// constructed backend adapters a caller (`canon-cli::tiers`)
    /// attached; each is wired into whichever RUNG's
    /// `tiers.<rung>.backend` names its backend — never assumed to be
    /// `local`→git/`hot`→postgres/`cold`→s3 specifically, and a `hot`
    /// rung may equally name `sqlite` (s32 `sqlite-hot-backend`).
    /// `GitTier` attaches unconditionally when configured (a local
    /// directory — no network, no credentials, design §9 local-
    /// first); `PgTier`/`R2Tier`/`SqliteTier` attach ONLY when a
    /// routing or aging entry actually needs them; an explicitly-
    /// required-but-unattachable rung (e.g. `routing.handoff: hot` but
    /// no live `CANON_PG_DSN`) is a startup-time hard error, never a
    /// silent skip (a prior session-store storage audit §3.2's
    /// fail-loud-on-explicit-pin contract) — resolved lazily via
    /// [`Self::handle`], not eagerly here, so a local-only
    /// consumer repo (no hot/cold entries at all) never needs live
    /// credentials to construct a registry.
    pub fn new(policy: TierPolicy, git: Option<GitTier>, pg: Option<PgTier>, r2: Option<R2Tier>, sqlite: Option<SqliteTier>) -> Self {
        let git = git.map(Arc::new);
        let pg = pg.map(Arc::new);
        let r2 = r2.map(Arc::new);
        let sqlite = sqlite.map(Arc::new);
        let resolve = |rung: Rung| -> Option<Arc<dyn Tier>> {
            match policy.tiers.get(&rung).map(BackendConfig::backend) {
                Some(Backend::Git) => git.clone().map(|t| t as Arc<dyn Tier>),
                Some(Backend::Postgres) => pg.clone().map(|t| t as Arc<dyn Tier>),
                Some(Backend::S3) => r2.clone().map(|t| t as Arc<dyn Tier>),
                Some(Backend::Sqlite) => sqlite.clone().map(|t| t as Arc<dyn Tier>),
                None => None,
            }
        };
        let local = resolve(Rung::Local);
        let hot = resolve(Rung::Hot);
        let cold = resolve(Rung::Cold);
        Self { policy, local, hot, cold, git }
    }

    fn slot(&self, rung: Rung) -> &Option<Arc<dyn Tier>> {
        match rung {
            Rung::Local => &self.local,
            Rung::Hot => &self.hot,
            Rung::Cold => &self.cold,
        }
    }

    fn handle(&self, rung: Rung) -> Result<Arc<dyn Tier>, StoreError> {
        self.slot(rung).clone().ok_or_else(|| {
            let backend = self.policy.tiers.get(&rung).map(BackendConfig::backend);
            let reason = match backend {
                Some(b) => b.default_unattached_reason().to_string(),
                None => format!("no `tiers.{}` in canon.yaml", rung.as_str()),
            };
            StoreError::tier_unavailable(rung, backend, reason)
        })
    }

    /// Every rung `kind` might currently have records in: its
    /// `routing` destination, PLUS its `aging.to` destination if one
    /// exists (a kind ages out of its routed rung over time — both must
    /// be read for `canon query` to see the whole picture, unified-
    /// query spec: "A kind split across hot and cold tiers merges
    /// correctly").
    fn tiers_for_read(&self, kind: RecordKind) -> Result<Vec<Rung>, StoreError> {
        let routed = self.policy.tier_for(kind)?;
        let mut rungs = vec![routed];
        if let Some(rule) = self.policy.aging.get(&kind) {
            if rule.to != routed {
                rungs.push(rule.to);
            }
        }
        Ok(rungs)
    }

    /// `persist<T: CanonRecord>` (S2 assignment's S1-interface note) —
    /// the generic, ergonomic write path every caller (S3 ingest, S5
    /// gate, S6 learn) uses; resolves `T::KIND`'s rung from
    /// `TierPolicy.routing` and never branches on a literal kind name
    /// itself (tier-adapter-trait spec's title requirement). Goes
    /// through [`Tier::write_batch`] (s31 design D2, tasks.md 1.1)
    /// rather than `Tier::write` directly — a single-record batch of
    /// one, so `GitTier`/`R2Tier`'s default loop and `PgTier`'s
    /// chunked override both collapse to exactly one row either way;
    /// this keeps `write_batch` the ONE codepath every write flows
    /// through, never a parallel unbatched path that could drift from
    /// it. Signature/receipt shape are UNCHANGED, so every existing
    /// caller compiles and behaves exactly as before.
    pub fn persist<T: CanonRecord>(&self, record: &T) -> Result<WriteReceipt, StoreError> {
        let tier = self.handle(self.policy.tier_for(T::KIND)?)?;
        let record_ref: &dyn StoredRecord = record;
        let mut receipts = tier.write_batch(std::slice::from_ref(&record_ref))?;
        Ok(receipts.remove(0))
    }

    /// The batching counterpart to [`Self::persist`] (s31 design D2)
    /// for a caller writing MANY records of the SAME kind in one pass
    /// — canon-ingest's per-session events, for one. Resolves
    /// `T::KIND`'s rung exactly ONCE for the whole slice (never once
    /// per record, unlike a `records.iter().map(persist)` loop) and
    /// hands every record to the tier's [`Tier::write_batch`] in a
    /// single call; receipts come back in `records`' order. An empty
    /// slice short-circuits without resolving a rung or touching the
    /// tier at all.
    pub fn persist_many<T: CanonRecord>(&self, records: &[T]) -> Result<Vec<WriteReceipt>, StoreError> {
        if records.is_empty() {
            return Ok(Vec::new());
        }
        let tier = self.handle(self.policy.tier_for(T::KIND)?)?;
        let refs: Vec<&dyn StoredRecord> = records.iter().map(|r| r as &dyn StoredRecord).collect();
        tier.write_batch(&refs)
    }

    /// `canon query --kind <k> [--since <t>]`'s backing implementation
    /// (unified-query spec D4): fan out across every rung `kind` may
    /// currently live in, issue each tier's native read, and merge by
    /// `at` — no cross-tier JOIN attempted here (the DuckDB views are
    /// for that).
    pub fn query(&self, query: &TierQuery) -> Result<TierReadResult, StoreError> {
        let mut merged = TierReadResult::default();
        for rung in self.tiers_for_read(query.kind)? {
            let tier = self.handle(rung)?;
            let mut result = tier.read(query)?;
            merged.records.append(&mut result.records);
            merged.violations.append(&mut result.violations);
        }
        merged.records.sort_by_key(crate::tier::raw_record_at);
        Ok(merged)
    }

    /// `canon tier age`'s backing implementation (tier-policy spec):
    /// run every `TierPolicy.aging` entry once, from its routed source
    /// rung to its configured destination.
    pub fn age_all(&self) -> Result<Vec<AgeReport>, StoreError> {
        let mut reports = Vec::new();
        // Iterate a stable (kind-name-sorted) order so report ordering
        // — and hence a caller's/CLI's printed output — is
        // deterministic across runs, not `HashMap`-iteration-order
        // dependent.
        let mut entries: Vec<_> = self.policy.aging.iter().collect();
        entries.sort_by_key(|(kind, _)| kind.as_str());
        for (kind, rule) in entries {
            let source_rung = self.policy.tier_for(*kind)?;
            let source = self.handle(source_rung)?;
            let destination = self.handle(rule.to)?;
            let report = source.age(&AgingRule { kind: *kind, after: rule.after, destination })?;
            reports.push(report);
        }
        Ok(reports)
    }

    pub fn git(&self) -> Option<&Arc<GitTier>> {
        self.git.as_ref()
    }
}


#[cfg(test)]
mod tests {
    use canon_model::envelope::{Actor, Envelope};
    use canon_model::ids::{ChangeId, RoleId, RunId};
    use canon_model::records::{Change, ChangeStatus, Trajectory};
    use chrono::Utc;

    use super::*;

    fn actor() -> Actor {
        Actor::new("test-agent", RoleId::parse("implementer").unwrap())
    }

    const POLICY_YAML: &str = r#"
tiers:
  local: { backend: git, root: canon/ledger }
  cold:      { backend: s3, bucket_env: CANON_R2_BUCKET, prefix: "canon/" }
routing:
  change: local
  trajectory: cold
aging: {}
"#;

    #[test]
    fn persist_routes_through_policy_never_a_literal_kind_branch() {
        let dir = tempfile::tempdir().unwrap();
        let r2_dir = tempfile::tempdir().unwrap();
        let policy = TierPolicy::from_yaml(POLICY_YAML).unwrap();
        let git = GitTier::new(dir.path());
        let r2 = R2Tier::local(r2_dir.path(), "canon/").unwrap();
        let registry = TierRegistry::new(policy, Some(git), None, Some(r2), None);

        let change = Change::new(
            Envelope::new(1, RecordKind::Change, Utc::now(), actor()),
            ChangeId::parse("s2-tiered-storage").unwrap(),
            "S2",
            "x",
            ChangeStatus::Proposed,
        );
        let receipt = registry.persist(&change).unwrap();
        assert!(receipt.location.starts_with("kind=change/"), "change is routed to the local (git) rung: {}", receipt.location);

        let trajectory = Trajectory::new(Envelope::new(1, RecordKind::Trajectory, Utc::now(), actor()), RunId::default(), None, None, None, None, None);
        let receipt = registry.persist(&trajectory).unwrap();
        assert!(receipt.location.contains("kind=trajectory"), "trajectory is routed to the cold (s3) rung: {}", receipt.location);
    }

    #[test]
    fn persist_of_an_unrouted_kind_is_a_loud_error() {
        let dir = tempfile::tempdir().unwrap();
        let policy = TierPolicy::from_yaml(POLICY_YAML).unwrap();
        let registry = TierRegistry::new(policy, Some(GitTier::new(dir.path())), None, None, None);
        // `scenario` has no `routing` entry in POLICY_YAML.
        let scenario = canon_model::records::Scenario::new(
            Envelope::new(1, RecordKind::Scenario, Utc::now(), actor()),
            canon_model::ids::ProjectId::parse("root").unwrap(),
            canon_model::ids::ScenarioId::parse("world.x.01").unwrap(),
            "t",
            "",
            canon_model::ids::SpecDigest::of(b"fixture .feature bytes"),
        );
        let err = registry.persist(&scenario).unwrap_err();
        assert!(matches!(err, StoreError::UnroutedKind { kind: RecordKind::Scenario }));
    }

    #[test]
    fn query_fans_out_and_merges_when_a_kind_is_split_across_two_tiers() {
        let dir = tempfile::tempdir().unwrap();
        let r2_dir = tempfile::tempdir().unwrap();
        // `trajectory` here is routed to `local` but has an
        // `aging.to: cold` entry — simulating "some records already
        // aged to cold, others still in the routed rung" without
        // needing a live postgres instance.
        let yaml = r#"
tiers:
  local: { backend: git, root: canon/ledger }
  cold:      { backend: s3, bucket_env: CANON_R2_BUCKET, prefix: "canon/" }
routing:
  trajectory: local
aging:
  trajectory: { after: 9999d, to: cold }
"#;
        let policy = TierPolicy::from_yaml(yaml).unwrap();
        let git = GitTier::new(dir.path());
        let r2 = R2Tier::local(r2_dir.path(), "canon/").unwrap();

        let older = Trajectory::new(
            Envelope::new(1, RecordKind::Trajectory, Utc::now() - chrono::Duration::days(5), actor()),
            RunId::new(),
            None,
            None,
            None,
            None,
            Some(0.1),
        );
        let newer = Trajectory::new(
            Envelope::new(1, RecordKind::Trajectory, Utc::now(), actor()),
            RunId::new(),
            None,
            None,
            None,
            None,
            Some(0.2),
        );
        git.write(&older).unwrap();
        r2.write(&newer).unwrap();

        let registry = TierRegistry::new(policy, Some(git), None, Some(r2), None);
        let result = registry.query(&TierQuery::kind(RecordKind::Trajectory)).unwrap();
        assert_eq!(result.records.len(), 2, "must see records from BOTH the routed rung and the aging destination");
        assert!(result.violations.is_empty());
        // Merged and ordered by `at`.
        assert_eq!(result.records[0].0["reward"], 0.1);
        assert_eq!(result.records[1].0["reward"], 0.2);
    }

    #[test]
    fn age_all_moves_records_past_threshold_and_is_idempotent_on_rerun() {
        let dir = tempfile::tempdir().unwrap();
        let r2_dir = tempfile::tempdir().unwrap();
        let yaml = r#"
tiers:
  local: { backend: git, root: canon/ledger }
  cold:      { backend: s3, bucket_env: CANON_R2_BUCKET, prefix: "canon/" }
routing:
  trajectory: local
aging:
  trajectory: { after: 1d, to: cold }
"#;
        let policy = TierPolicy::from_yaml(yaml).unwrap();
        let git = GitTier::new(dir.path());
        let r2 = R2Tier::local(r2_dir.path(), "canon/").unwrap();

        let old = Trajectory::new(
            Envelope::new(1, RecordKind::Trajectory, Utc::now() - chrono::Duration::days(30), actor()),
            RunId::new(),
            None,
            None,
            None,
            None,
            Some(0.3),
        );
        git.write(&old).unwrap();

        let registry = TierRegistry::new(policy, Some(git), None, Some(r2), None);
        let reports = registry.age_all().unwrap();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].moved, 1);
        assert_eq!(reports[0].already_aged, 0);

        // Re-running immediately: the local-rung source row is
        // already gone (the record physically left the local rung
        // because the first call above already moved+deleted the
        // source-tier file). A second run finds nothing left to age.
        let second = registry.age_all().unwrap();
        assert_eq!(second[0].moved, 0);
        assert_eq!(second[0].already_aged, 0, "nothing left in the source rung to re-select");
    }

    /// s31 design D2, tasks.md 1.1/1.3: `persist_many` resolves
    /// `T::KIND`'s rung once and hands the whole slice to
    /// [`Tier::write_batch`] — fresh, distinct records all land as new
    /// writes, receipts come back one-per-record in input order.
    #[test]
    fn persist_many_resolves_rung_once_and_returns_one_receipt_per_record_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let r2_dir = tempfile::tempdir().unwrap();
        let policy = TierPolicy::from_yaml(POLICY_YAML).unwrap();
        let git = GitTier::new(dir.path());
        let r2 = R2Tier::local(r2_dir.path(), "canon/").unwrap();
        let registry = TierRegistry::new(policy, Some(git), None, Some(r2), None);

        let trajectories: Vec<Trajectory> = (0..5)
            .map(|_| Trajectory::new(Envelope::new(1, RecordKind::Trajectory, Utc::now(), actor()), RunId::new(), None, None, None, None, Some(0.5)))
            .collect();

        let receipts = registry.persist_many(&trajectories).unwrap();
        assert_eq!(receipts.len(), 5, "one receipt per record");
        assert!(receipts.iter().all(|r| !r.deduped), "fresh distinct-run-id records must all land as new writes");
        assert!(receipts.iter().all(|r| r.location.contains("kind=trajectory")));
    }

    /// s31 tasks.md 1.3: "batch no-op on byte-identical resubmission
    /// (record count unchanged)".
    #[test]
    fn persist_many_resubmission_is_a_no_op_and_record_count_is_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let r2_dir = tempfile::tempdir().unwrap();
        let policy = TierPolicy::from_yaml(POLICY_YAML).unwrap();
        let git = GitTier::new(dir.path());
        let r2 = R2Tier::local(r2_dir.path(), "canon/").unwrap();
        let registry = TierRegistry::new(policy, Some(git), None, Some(r2), None);

        let trajectories: Vec<Trajectory> = (0..4)
            .map(|_| Trajectory::new(Envelope::new(1, RecordKind::Trajectory, Utc::now(), actor()), RunId::new(), None, None, None, None, Some(0.7)))
            .collect();

        let first = registry.persist_many(&trajectories).unwrap();
        let second = registry.persist_many(&trajectories).unwrap();
        assert!(second.iter().all(|r| r.deduped), "a byte-identical batch resubmission must be a no-op for every row");
        for (a, b) in first.iter().zip(&second) {
            assert_eq!(a.digest, b.digest);
            assert_eq!(a.location, b.location);
        }

        let after = registry.query(&TierQuery::kind(RecordKind::Trajectory)).unwrap();
        assert_eq!(after.records.len(), 4, "record count must be unchanged after the resubmission — never a double-write");
    }

    /// s31 tasks.md 1.3: "batch vs loop write equivalence on the same
    /// corpus" — a `persist_many` batch and a `persist`-per-record
    /// loop over content-identically-shaped, disjoint corpora produce
    /// the SAME receipt semantics (fresh write, never deduped) and
    /// leave the SAME number of records behind.
    #[test]
    fn persist_many_matches_a_persist_loop_over_an_equivalent_corpus() {
        let dir = tempfile::tempdir().unwrap();
        let r2_dir = tempfile::tempdir().unwrap();
        let policy = TierPolicy::from_yaml(POLICY_YAML).unwrap();
        let git = GitTier::new(dir.path());
        let r2 = R2Tier::local(r2_dir.path(), "canon/").unwrap();
        let registry = TierRegistry::new(policy, Some(git), None, Some(r2), None);

        let make = || -> Vec<Trajectory> {
            (0..6).map(|_| Trajectory::new(Envelope::new(1, RecordKind::Trajectory, Utc::now(), actor()), RunId::new(), None, None, None, None, Some(0.9))).collect()
        };
        let looped = make();
        let batched = make();

        let loop_receipts: Vec<_> = looped.iter().map(|t| registry.persist(t).unwrap()).collect();
        let batch_receipts = registry.persist_many(&batched).unwrap();

        assert_eq!(loop_receipts.len(), batch_receipts.len());
        for (loop_r, batch_r) in loop_receipts.iter().zip(&batch_receipts) {
            assert_eq!(loop_r.deduped, batch_r.deduped, "fresh content must be a new write under both codepaths");
            assert!(!batch_r.deduped);
        }

        let after = registry.query(&TierQuery::kind(RecordKind::Trajectory)).unwrap();
        assert_eq!(after.records.len(), 12, "both corpora (6 looped + 6 batched) must be fully persisted, none dropped or duplicated");
    }

    /// An empty batch short-circuits before resolving a rung at all —
    /// even a kind whose rung has no live tier attached must not error
    /// on a genuinely empty write.
    #[test]
    fn persist_many_of_an_empty_slice_short_circuits_without_touching_any_tier() {
        let dir = tempfile::tempdir().unwrap();
        let policy = TierPolicy::from_yaml(POLICY_YAML).unwrap();
        // No r2/pg tier attached — `trajectory`'s `cold` rung is
        // configured but UNattached; a non-empty batch would fail.
        let registry = TierRegistry::new(policy, Some(GitTier::new(dir.path())), None, None, None);
        let empty: Vec<Trajectory> = Vec::new();
        assert_eq!(registry.persist_many(&empty).unwrap(), Vec::new());
    }

    /// s27 `query-tier-degradation` spec: `TierUnavailable` names both
    /// the rung and, when configured, the backend behind it.
    #[test]
    fn handle_of_a_configured_but_unattached_rung_names_rung_and_backend() {
        let yaml = r#"
tiers:
  hot: { backend: postgres, dsn_env: CANON_PG_DSN_REGISTRY_UNIT_UNSET, schema: canon_v1 }
routing:
  task: hot
"#;
        let policy = TierPolicy::from_yaml(yaml).unwrap();
        // No live `PgTier` handle attached — `tiers.hot` is configured
        // but never reached a live DSN.
        let registry = TierRegistry::new(policy, None, None, None, None);
        let err = registry.query(&TierQuery::kind(RecordKind::Task)).unwrap_err();
        match &err {
            StoreError::TierUnavailable { rung, backend, .. } => {
                assert_eq!(*rung, Rung::Hot);
                assert_eq!(*backend, Some(Backend::Postgres));
            }
            other => panic!("expected TierUnavailable, got {other:?}"),
        }
        let text = err.to_string().to_lowercase();
        assert!(text.contains("hot"), "{text}");
        assert!(text.contains("postgres"), "{text}");
    }

    /// s27 spec: a routed rung with NO `tiers.<rung>` block at all
    /// fails naming the rung alone — never a fabricated backend name.
    #[test]
    fn handle_of_an_unconfigured_rung_names_the_rung_alone() {
        let yaml = r#"
routing:
  task: hot
"#;
        let policy = TierPolicy::from_yaml(yaml).unwrap();
        let registry = TierRegistry::new(policy, None, None, None, None);
        let err = registry.query(&TierQuery::kind(RecordKind::Task)).unwrap_err();
        match err {
            StoreError::TierUnavailable { rung, backend, .. } => {
                assert_eq!(rung, Rung::Hot);
                assert_eq!(backend, None, "an unconfigured rung must never fabricate a backend name");
            }
            other => panic!("expected TierUnavailable, got {other:?}"),
        }
    }
}
