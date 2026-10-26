//! `PgTier` against a REAL Postgres instance — compiled only under
//! `--features live-pg` (so `cargo test --workspace` with no extra
//! flags never even builds this file, let alone attempts network I/O)
//! AND additionally skip-if-down at runtime: a cheap TCP probe against
//! `CANON_PG_DSN` (docker-compose default
//! `postgres://canon:canon@127.0.0.1:55432/canon_v1`, the repo-root
//! `docker-compose.yml` `postgres` service — operator directive
//! 2026-07-10's local-first retrofit) runs BEFORE any real `PgTier`
//! I/O, exactly mirroring `tests/r2_tier_live.rs`'s own probe (see that
//! file's module doc for the reachable/unreachable/`CANON_REQUIRE_LIVE`
//! contract, identical here).
#![cfg(feature = "live-pg")]

use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration as StdDuration;

use canon_model::envelope::{Actor, Envelope, RecordKind};
use canon_model::ids::{RoleId, SessionId};
use canon_model::records::Session;
use canon_store::pg_tier::PgTier;
use canon_store::r2_tier::R2Tier;
use canon_store::tier::{AgingRule, StoreError, StoredRecord, Tier, TierQuery};
use chrono::{Duration as ChronoDuration, Utc};

fn require_live() -> bool {
    std::env::var("CANON_REQUIRE_LIVE").as_deref() == Ok("1")
}

/// `postgres://user:pass@host[:port]/db` -> `(host, port)`. Deliberately
/// tiny/hand-rolled — no `url`-crate dependency needed for a probe this
/// narrow (S2 assignment constraint: no new dependency for this
/// retrofit).
fn dsn_host_port(dsn: &str) -> (String, u16) {
    let after_scheme = dsn.split("://").nth(1).unwrap_or(dsn);
    let after_creds = after_scheme.rsplit_once('@').map_or(after_scheme, |(_, rest)| rest);
    let host_port = after_creds.split('/').next().unwrap_or(after_creds);
    match host_port.rsplit_once(':') {
        Some((host, port)) => (host.to_string(), port.parse().unwrap_or(5432)),
        None => (host_port.to_string(), 5432),
    }
}

/// Best-effort "is anything listening" check — deliberately cheap and
/// side-effect-free, run before any real `PgTier` I/O so an
/// unreachable docker-compose stack is told apart from a genuine
/// round-trip bug (this file's module doc).
fn tcp_reachable(host: &str, port: u16) -> bool {
    (host, port)
        .to_socket_addrs()
        .is_ok_and(|addrs| addrs.into_iter().any(|addr| TcpStream::connect_timeout(&addr, StdDuration::from_millis(800)).is_ok()))
}

#[test]
fn write_read_upsert_and_age_round_trip_against_a_live_instance() {
    let dsn = std::env::var("CANON_PG_DSN").unwrap_or_else(|_| "postgres://canon:canon@127.0.0.1:55432/canon_v1".to_string());
    let (host, port) = dsn_host_port(&dsn);

    if !tcp_reachable(&host, port) {
        if require_live() {
            panic!(
                "CANON_REQUIRE_LIVE=1 but Postgres at {host}:{port} is unreachable — \
                 run `docker compose up -d --wait postgres minio` (repo root) first"
            );
        }
        eprintln!("skipping: Postgres at {host}:{port} is unreachable (set CANON_REQUIRE_LIVE=1 to make this a hard failure)");
        return;
    }

    let tier = PgTier::connect_live("canon_v1_test").expect("connect to live Postgres");
    let actor = Actor::new("live-test", RoleId::parse("implementer").unwrap());

    // A per-run-unique natural key: the compose `postgres` volume is
    // named/persistent across `docker compose down`/`up`, so a fixed
    // literal session id would let one run's leftover row leak into
    // the next run's dedup/append assertions.
    let session_id_str = format!("live-pg-test-session-{}", uuid::Uuid::new_v4());
    let session_id = SessionId::parse(&session_id_str).unwrap();

    // 1. Fresh natural key: not deduped.
    let v1 = Session::new(Envelope::new(1, RecordKind::Session, Utc::now(), actor.clone()), session_id.clone(), "claude-code", Utc::now(), None);
    let receipt1 = tier.write(&v1).expect("write v1");
    assert!(!receipt1.deduped);

    // 2. Re-write IDENTICAL content: an unchanged-digest write is a no-op.
    let receipt1_again = tier.write(&v1).expect("re-write identical content");
    assert!(receipt1_again.deduped, "identical content re-write must be a digest-dedup no-op against a live instance too");
    assert_eq!(receipt1.digest, receipt1_again.digest);
    assert_eq!(receipt1.location, receipt1_again.location);

    // 3. A NEW digest at the SAME (kind, id): s21 D1 — this is now a
    //    genuine APPEND, never an update in place. Both versions must
    //    stay retrievable afterward.
    let v2 = Session::new(Envelope::new(1, RecordKind::Session, Utc::now(), actor.clone()), session_id.clone(), "claude-code", Utc::now(), Some(Utc::now()));
    let receipt2 = tier.write(&v2).expect("write v2 (new digest, same natural key)");
    assert!(!receipt2.deduped, "a new digest at the same (kind, id) must NOT be reported as deduped");
    assert_ne!(receipt1.digest, receipt2.digest, "v1/v2 differ (v2 has ended_at set) — digests must differ too");
    assert_ne!(receipt1.location, receipt2.location, "s21 D1: a new version is a NEW row/location, never the same pg row overwritten");

    let after_append = tier.read(&TierQuery::kind(RecordKind::Session)).expect("read after append");
    let matches: Vec<_> = after_append.records.iter().filter(|r| r.0["session_id"] == session_id_str).collect();
    assert_eq!(matches.len(), 2, "s21 D1: an append must leave BOTH versions retrievable, never overwrite v1's row");
    assert!(matches.iter().any(|r| r.0["ended_at"].is_null()), "v1 (no ended_at) must still be present");
    assert!(matches.iter().any(|r| !r.0["ended_at"].is_null()), "v2 (ended_at set) must also be present");

    // 3b. `fold_latest_by_key` over the raw read picks the NEWER `at`
    //     regardless of which write physically arrived last — the
    //     caller-side resolution s21 D5 requires every reader to
    //     apply. Write v3 with an OLDER `at` than v2, arriving AFTER
    //     v2: the fold must still resolve to v2 (the chronologically
    //     newer one), never v3 just because it landed last.
    let v2_at = v2.envelope.at;
    let v3_at = v2_at - ChronoDuration::hours(1);
    let v3 = Session::new(Envelope::new(1, RecordKind::Session, v3_at, actor.clone()), session_id.clone(), "claude-code", v3_at, None);
    tier.write(&v3).expect("write v3 (older `at`, arrives last)");

    let after_v3 = tier.read(&TierQuery::kind(RecordKind::Session)).expect("read after out-of-order arrival");
    let session_rows: Vec<_> = after_v3.records.into_iter().filter(|r| r.0["session_id"] == session_id_str).collect();
    assert_eq!(session_rows.len(), 3, "the older out-of-order write must ADD a version, never replace one");

    struct DigestedRow {
        raw: canon_model::RawRecord,
        digest: String,
    }
    let digested: Vec<DigestedRow> = session_rows.into_iter().map(|raw| DigestedRow { digest: canon_store::partition::content_digest(&raw.0), raw }).collect();
    let folded = canon_store::fold_latest_by_key(
        digested,
        |r| r.raw.0["session_id"].as_str().unwrap().to_string(),
        |r| canon_store::tier::raw_record_at(&r.raw),
        |r| canon_store::tier::raw_record_schema(&r.raw),
        |r| r.digest.as_str(),
    );
    let winner = folded.get(&session_id_str).expect("folded winner for this session_id");
    // Compare the PARSED instant (via the fold's own `raw_record_at`
    // extractor), never the serialized string: a live pg round-trips
    // `at` as RFC3339 `…Z`, while `to_rfc3339()` emits the equivalent
    // `…+00:00` — the same instant, two valid encodings. Asserting the
    // raw JSON string would fail on encoding alone, not on the fold
    // picking the wrong record.
    assert_eq!(
        canon_store::tier::raw_record_at(&winner.raw),
        v2_at,
        "the fold must pick the chronologically NEWER `at` (v2), regardless of write arrival order"
    );

    // 4. Age round-trip: a record whose envelope `at` already precedes
    //    the aging cutoff moves to the destination tier and is deleted
    //    from pg — `Tier::age`'s contract, exercised against a real
    //    instance rather than only pg_tier.rs's offline SQL-generation
    //    tests. Aging deletes by the FULL (kind, id, digest) row
    //    identity (s21 task 3.5) — only the aged-out version leaves,
    //    any sibling version under the SAME key that is still under
    //    the cutoff must stay.
    let aging_session_id_str = format!("live-pg-test-aging-{}", uuid::Uuid::new_v4());
    let aging_session_id = SessionId::parse(&aging_session_id_str).unwrap();
    let old_at = Utc::now() - ChronoDuration::days(90);
    let aging_candidate =
        Session::new(Envelope::new(1, RecordKind::Session, old_at, actor), aging_session_id.clone(), "claude-code", old_at, None);
    tier.write(&aging_candidate).expect("write aging candidate");

    let dest_dir = tempfile::tempdir().unwrap();
    let destination: std::sync::Arc<dyn Tier> = std::sync::Arc::new(R2Tier::local(dest_dir.path(), "canon/").unwrap());
    let rule = AgingRule { kind: RecordKind::Session, after: ChronoDuration::days(30), destination: std::sync::Arc::clone(&destination) };
    let report = tier.age(&rule).expect("age");
    assert!(report.moved >= 1, "expected the 90d-old aging candidate to have moved (moved={}, already_aged={})", report.moved, report.already_aged);

    let after_age = tier.read(&TierQuery::kind(RecordKind::Session)).expect("read after age");
    assert!(
        !after_age.records.iter().any(|r| r.0["session_id"] == aging_session_id_str),
        "an aged record must be deleted from its source (pg) tier"
    );
    let dest_read = destination.read(&TierQuery::kind(RecordKind::Session)).expect("read destination tier after age");
    assert!(
        dest_read.records.iter().any(|r| r.0["session_id"] == aging_session_id_str),
        "an aged record must land in its destination tier"
    );

    // The still-live session_id's versions (v1/v2/v3 from steps 1-3b)
    // must be untouched by aging — every one of them carries an `at`
    // well inside the 30d cutoff.
    let live_still_present = tier.read(&TierQuery::kind(RecordKind::Session)).expect("read after age");
    let live_count = live_still_present.records.iter().filter(|r| r.0["session_id"] == session_id_str).count();
    assert_eq!(live_count, 3, "aging must not sweep up non-eligible records, and must not delete a sibling version under the same key");
}

/// s31 design D2, tasks.md 1.3: `PgTier::write_batch`'s chunked
/// multi-row path must (a) treat a byte-identical resubmission as a
/// no-op exactly like the single-row path, and (b) produce receipts
/// (`deduped`/`digest`/`location`, in order) that a plain loop over
/// [`Tier::write`] would have produced for the SAME content — proven
/// here on two DISJOINT, content-identically-shaped corpora (distinct
/// `session_id`s) so neither path's dedup bookkeeping leaks into the
/// other's assertions.
#[test]
fn write_batch_is_a_resubmission_no_op_and_matches_loop_write_semantics() {
    let dsn = std::env::var("CANON_PG_DSN").unwrap_or_else(|_| "postgres://canon:canon@127.0.0.1:55432/canon_v1".to_string());
    let (host, port) = dsn_host_port(&dsn);

    if !tcp_reachable(&host, port) {
        if require_live() {
            panic!(
                "CANON_REQUIRE_LIVE=1 but Postgres at {host}:{port} is unreachable — \
                 run `docker compose up -d --wait postgres minio` (repo root) first"
            );
        }
        eprintln!("skipping: Postgres at {host}:{port} is unreachable (set CANON_REQUIRE_LIVE=1 to make this a hard failure)");
        return;
    }

    let tier = PgTier::connect_live("canon_v1_test").expect("connect to live Postgres");
    let actor = Actor::new("live-test", RoleId::parse("implementer").unwrap());
    let prefix = format!("live-pg-batch-{}", uuid::Uuid::new_v4());
    let n = 7;

    let make_sessions = |tag: &str| -> Vec<Session> {
        (0..n)
            .map(|i| {
                let id = SessionId::parse(format!("{prefix}-{tag}-{i}")).unwrap();
                Session::new(Envelope::new(1, RecordKind::Session, Utc::now(), actor.clone()), id, "claude-code", Utc::now(), None)
            })
            .collect()
    };
    let loop_sessions = make_sessions("loop");
    let batch_sessions = make_sessions("batch");

    let loop_receipts: Vec<_> = loop_sessions.iter().map(|s| tier.write(s).expect("loop write")).collect();

    let batch_refs: Vec<&dyn StoredRecord> = batch_sessions.iter().map(|s| s as &dyn StoredRecord).collect();
    let batch_receipts = tier.write_batch(&batch_refs).expect("batch write");

    assert_eq!(batch_receipts.len(), n, "one receipt per record, in input order");
    for (loop_r, batch_r) in loop_receipts.iter().zip(&batch_receipts) {
        assert!(!loop_r.deduped, "fresh content via the loop path must not be reported deduped");
        assert!(!batch_r.deduped, "fresh content via the batch path must not be reported deduped — same semantics as the loop path");
    }

    // Re-submit the SAME batch content unchanged: s31 spec "Re-ingest
    // is still a no-op" — every receipt must report deduped, and the
    // store's row count for this corpus must stay unchanged.
    let resubmit_receipts = tier.write_batch(&batch_refs).expect("resubmit batch");
    assert_eq!(resubmit_receipts.len(), n);
    assert!(resubmit_receipts.iter().all(|r| r.deduped), "a byte-identical batch resubmission must be a no-op for every row");
    for (first, second) in batch_receipts.iter().zip(&resubmit_receipts) {
        assert_eq!(first.digest, second.digest);
        assert_eq!(first.location, second.location);
    }

    let after = tier.read(&TierQuery::kind(RecordKind::Session)).expect("read after resubmit");
    for session in &batch_sessions {
        let id = session.session_id.as_str();
        let count = after.records.iter().filter(|r| r.0["session_id"] == id).count();
        assert_eq!(count, 1, "record count for {id} must be unchanged by the resubmission (no double-write)");
    }
    for session in &loop_sessions {
        let id = session.session_id.as_str();
        let count = after.records.iter().filter(|r| r.0["session_id"] == id).count();
        assert_eq!(count, 1, "the loop-written corpus is untouched by the batch resubmission");
    }
}

/// Two `canon` processes reaching a FRESH schema at the same time must
/// both come up.
///
/// `PgTier::connect` bootstraps with `CREATE SCHEMA/TABLE/INDEX IF NOT
/// EXISTS`, and Postgres's `IF NOT EXISTS` is NOT atomic against a
/// concurrent creator: both sessions pass the existence check, both
/// insert the catalog row, and the loser gets `duplicate key value
/// violates unique constraint "pg_type_typname_nsp_index"`. That is a
/// startup failure for a real operator, not just a test artifact —
/// team-scale concurrency is the whole reason the `hot` rung can be
/// postgres (s32 `sqlite-hot-backend`). CI surfaced it first as this
/// file's own two tests racing on two threads.
///
/// The schema name is unique per run so the DDL is genuinely executed
/// rather than short-circuiting on an already-bootstrapped schema —
/// without that, this test passes even with the fix reverted.
#[test]
fn a_fresh_schema_survives_concurrent_bootstrap() {
    let dsn = std::env::var("CANON_PG_DSN").unwrap_or_else(|_| "postgres://canon:canon@127.0.0.1:55432/canon_v1".to_string());
    let (host, port) = dsn_host_port(&dsn);

    if !tcp_reachable(&host, port) {
        if require_live() {
            panic!(
                "CANON_REQUIRE_LIVE=1 but Postgres at {host}:{port} is unreachable — \
                 run `docker compose up -d --wait postgres minio` (repo root) first"
            );
        }
        eprintln!("skipping: Postgres at {host}:{port} is unreachable (set CANON_REQUIRE_LIVE=1 to make this a hard failure)");
        return;
    }

    let nonce = Utc::now().timestamp_nanos_opt().unwrap_or_default().unsigned_abs();
    let schema = format!("canon_boot_{nonce}");

    let failures: Vec<String> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let dsn = dsn.clone();
                let schema = schema.clone();
                scope.spawn(move || PgTier::connect(&dsn, &schema).err().map(|e| e.to_string()))
            })
            .collect();
        handles.into_iter().filter_map(|h| h.join().expect("bootstrap thread must not panic")).collect()
    });

    assert!(
        failures.is_empty(),
        "every concurrent bootstrap of a fresh schema must succeed; got {} failure(s): {failures:#?}",
        failures.len()
    );
}

// ── ledger naming (change `canon-0-30-ledger-naming`) against a live Postgres ──

/// The live DSN when Postgres is reachable; `None` (skip) when it is not and
/// `CANON_REQUIRE_LIVE` is unset — the same contract as the tests above.
fn live_dsn() -> Option<String> {
    let dsn = std::env::var("CANON_PG_DSN").unwrap_or_else(|_| "postgres://canon:canon@127.0.0.1:55432/canon_v1".to_string());
    let (host, port) = dsn_host_port(&dsn);
    if tcp_reachable(&host, port) {
        return Some(dsn);
    }
    if require_live() {
        panic!("CANON_REQUIRE_LIVE=1 but Postgres at {host}:{port} is unreachable — run `docker compose up -d --wait postgres minio` (repo root) first");
    }
    eprintln!("skipping: Postgres at {host}:{port} is unreachable (set CANON_REQUIRE_LIVE=1 to make this a hard failure)");
    None
}

/// A schema no other run has touched, so its row set is exactly what the test wrote.
fn fresh_tier(dsn: &str, tag: &str) -> PgTier {
    PgTier::connect(dsn, &format!("canon_ln_{tag}_{}", uuid::Uuid::new_v4().simple())).expect("connect to a fresh schema")
}

fn well_formed(kind: &str) -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../canon-model/fixtures/well-formed").join(format!("{kind}.json"));
    serde_json::from_slice(&std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap()
}

fn finding_n(seq: u64) -> serde_json::Value {
    let mut finding = well_formed("finding");
    finding["seq"] = serde_json::json!(seq);
    finding
}

fn raw(body: &serde_json::Value) -> canon_store::tier::RawWrite {
    canon_store::tier::RawWrite(canon_model::RawRecord(body.clone()))
}

/// The `(kind, id, digest)` a finding body is stored under.
fn finding_key(body: &serde_json::Value) -> (String, String, String) {
    let name = canon_store::partition::resolve_name(RecordKind::Finding, body).unwrap();
    ("finding".to_string(), name.key, name.digest)
}

/// ledger.naming.08 (Postgres): a record landing on a key held with other JSON refuses the whole batch —
/// an earlier record of the batch and every row of an earlier 500-row chunk included — and the table
/// is as it was. An identical resubmission is a no-op, not a conflict.
#[test]
fn ledger_naming_08_a_key_conflict_rolls_the_whole_batch_back() {
    let Some(dsn) = live_dsn() else { return };
    let tier = fresh_tier(&dsn, "n08");

    // Plant a row on the key `held` resolves to, holding other JSON: write another finding, then re-key it there.
    let held = finding_n(2);
    let planted = finding_n(900);
    tier.write(&raw(&planted)).unwrap();
    tier.rekey(&[canon_store::tier::RowRekey { from: finding_key(&planted), to: finding_key(&held), body: serde_json::json!({"other": "json"}) }]).unwrap();
    let before = tier.all_rows().unwrap();
    assert_eq!(before.len(), 1);

    // A batch of two: the first is fine, the second lands on the held key.
    let err = tier.write_batch(&[&raw(&finding_n(1)), &raw(&held)]).expect_err("the later record lands on a held key");
    assert!(matches!(err, StoreError::KeyConflict { .. }), "{err:?}");
    assert_eq!(tier.all_rows().unwrap(), before, "no row of the batch is stored");

    // The same through the single-row path.
    let err = tier.write(&raw(&held)).expect_err("a single write onto the held key");
    assert!(matches!(err, StoreError::KeyConflict { .. }), "{err:?}");
    assert_eq!(tier.all_rows().unwrap(), before);

    // More than one 500-row chunk: the conflict sits in the last chunk, so the first chunk's rows
    // were inserted before the conflict was found and must not survive it.
    let held_last = finding_n(1501);
    let planted = finding_n(901);
    tier.write(&raw(&planted)).unwrap();
    tier.rekey(&[canon_store::tier::RowRekey { from: finding_key(&planted), to: finding_key(&held_last), body: serde_json::json!({"other": "json"}) }]).unwrap();
    let before = tier.all_rows().unwrap();
    assert_eq!(before.len(), 2);
    let bodies: Vec<serde_json::Value> = (1001..=1501).map(finding_n).collect();
    let writes: Vec<_> = bodies.iter().map(raw).collect();
    let refs: Vec<&dyn StoredRecord> = writes.iter().map(|w| w as &dyn StoredRecord).collect();
    assert_eq!(refs.len(), 501);
    let err = tier.write_batch(&refs).expect_err("the last chunk's record lands on a held key");
    assert!(matches!(err, StoreError::KeyConflict { .. }), "{err:?}");
    assert_eq!(tier.all_rows().unwrap(), before, "the rows of the first chunk were rolled back too");

    // The same record twice is a no-op, not a conflict.
    assert!(!tier.write(&raw(&finding_n(1))).unwrap().deduped);
    assert!(tier.write(&raw(&finding_n(1))).unwrap().deduped);
}

/// ledger.naming.18 (Postgres): evidence naming no task, scenario or run is refused by the model before
/// any row is written, alone or in a batch with a good record.
#[test]
fn ledger_naming_18_keyless_evidence_is_refused_before_any_row() {
    let Some(dsn) = live_dsn() else { return };
    let tier = fresh_tier(&dsn, "n18");
    let keyless = serde_json::json!({"schema": 1, "kind": "evidence", "at": "2026-07-10T12:00:00Z", "actor": {"agent_id": "a", "role": "implementer"}, "verdict": "faithful"});

    let err = tier.write(&raw(&keyless)).expect_err("keyless evidence");
    assert!(matches!(err, StoreError::Layout(_)), "{err:?}");
    let message = err.to_string();
    assert!(message.contains("task_id") && message.contains("scenario_id") && message.contains("run_id"), "{message}");
    assert!(tier.all_rows().unwrap().is_empty(), "no row was written");

    let err = tier.write_batch(&[&raw(&well_formed("scenario")), &raw(&keyless)]).expect_err("a batch holding keyless evidence");
    assert!(matches!(err, StoreError::Layout(_)), "{err:?}");
    assert!(tier.all_rows().unwrap().is_empty(), "the good record of the batch was not written either");

    let typed = canon_model::EvidenceRecord::new(
        Envelope::new(1, RecordKind::Evidence, "2026-07-10T12:00:00Z".parse().unwrap(), Actor::new("a", RoleId::parse("implementer").unwrap())),
        None,
        None,
        None,
        canon_model::EvidenceVerdict::Faithful,
    );
    assert!(matches!(tier.write(&typed), Err(StoreError::Layout(_))));
    assert!(tier.all_rows().unwrap().is_empty());
}

/// ledger.migrate.09 (Postgres tier): `rekey` moves rows to their new keys and bodies, and rekeying
/// back with the old keys and bodies restores the table exactly.
#[test]
fn ledger_migrate_09_rekey_moves_rows_and_the_old_keys_restore_them() {
    let Some(dsn) = live_dsn() else { return };
    let tier = fresh_tier(&dsn, "m09");
    let (a, b) = (finding_n(1), finding_n(2));
    tier.write(&raw(&a)).unwrap();
    tier.write(&raw(&b)).unwrap();
    let before = tier.all_rows().unwrap();
    assert_eq!(before.len(), 2);

    // The new keys: other ids and digests, other bodies.
    let new_a = (finding_key(&a).0, "moved_a_00000001".to_string(), "00000001".to_string());
    let new_b = (finding_key(&b).0, "moved_b_00000002".to_string(), "00000002".to_string());
    let moves = [
        canon_store::tier::RowRekey { from: finding_key(&a), to: new_a.clone(), body: serde_json::json!({"moved": "a"}) },
        canon_store::tier::RowRekey { from: finding_key(&b), to: new_b.clone(), body: serde_json::json!({"moved": "b"}) },
    ];
    tier.rekey(&moves).unwrap();
    let moved = tier.all_rows().unwrap();
    let keys: Vec<_> = moved.iter().map(|r| (r.kind.clone(), r.id.clone(), r.digest.clone())).collect();
    assert_eq!(keys.len(), 2);
    assert!(keys.contains(&new_a) && keys.contains(&new_b), "{keys:?}");
    assert!(moved.iter().any(|r| r.body == serde_json::json!({"moved": "a"})) && moved.iter().any(|r| r.body == serde_json::json!({"moved": "b"})));

    // The revert: the old keys and the old bodies.
    let undo: Vec<_> = moves.iter().zip([&a, &b]).map(|(m, old)| canon_store::tier::RowRekey { from: m.to.clone(), to: m.from.clone(), body: old.clone() }).collect();
    tier.rekey(&undo).unwrap();
    assert_eq!(tier.all_rows().unwrap(), before, "the rows have their old keys, bodies and times back");
}

/// ledger.migrate.10 (Postgres tier): a re-key whose new key another row holds with other JSON is
/// refused as a key conflict and nothing of the call is kept — a change made earlier in the same call
/// included.
#[test]
fn ledger_migrate_10_a_rekey_onto_a_held_key_is_refused_and_changes_nothing() {
    let Some(dsn) = live_dsn() else { return };
    let tier = fresh_tier(&dsn, "m10");
    let (a, b, held) = (finding_n(1), finding_n(2), finding_n(3));
    for body in [&a, &b, &held] {
        tier.write(&raw(body)).unwrap();
    }
    let before = tier.all_rows().unwrap();
    assert_eq!(before.len(), 3);

    let free = (finding_key(&a).0, "moved_a_00000001".to_string(), "00000001".to_string());
    let err = tier
        .rekey(&[
            canon_store::tier::RowRekey { from: finding_key(&a), to: free, body: serde_json::json!({"moved": "a"}) },
            // `held` already stores `finding_key(&held)` with its own JSON; `b` would take that key with other JSON.
            canon_store::tier::RowRekey { from: finding_key(&b), to: finding_key(&held), body: serde_json::json!({"moved": "b"}) },
        ])
        .expect_err("the second change lands on a held key");
    assert!(matches!(err, StoreError::KeyConflict { .. }), "{err:?}");
    assert_eq!(tier.all_rows().unwrap(), before, "the first change was rolled back too");

    // The same JSON under the new key is not a conflict: the old row is dropped, the held row stays.
    tier.rekey(&[canon_store::tier::RowRekey { from: finding_key(&b), to: finding_key(&held), body: held.clone() }]).unwrap();
    assert_eq!(tier.all_rows().unwrap().len(), 2);
}
