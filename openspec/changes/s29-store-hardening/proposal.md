## Why

A two-reviewer audit of canon's DB/env/credential seams (operator
request, 2026-07-14) found that the pg and s3 tier arms do NOT honor
the same fail-loud contract, that `TierPolicy::from_yaml` accepts
destructive aging configurations, and that several store/CLI paths
violate their own documented degrade contracts. Two findings are
data-loss class:

1. **Release S3 default credentials.** `R2Tier::connect_live`
   (`crates/canon-store/src/r2_tier.rs:191-211`) requires only a
   bucket name; endpoint/credentials silently default to the
   docker-compose MinIO dev stack (`http://127.0.0.1:59000`,
   `canon`/`canoncanon`). `canon-cli`'s release-build
   `build_r2_tier` reaches this path unchanged, and the s3 client
   builder performs no I/O — so a release binary with
   `CANON_R2_BUCKET` set but `CANON_S3_*` unset ATTACHES
   "successfully" pointed at loopback. If a local MinIO with dev
   creds happens to be up, `canon tier age` writes cold records there
   and then DELETES the Postgres source rows. The pg arm fails loud
   on a missing DSN; the s3 arm must be held to the same
   startup-time-hard-failure rule (data-stores audit §3.2) in
   release builds.

2. **Non-forward aging accepted.** `TierPolicy::from_yaml`
   (`crates/canon-store/src/policy.rs:463-468`) never compares
   `aging.<kind>.to` against the kind's routed rung. `routing.task:
   hot` + `aging.task: { to: hot }` hands `TierRegistry::age_all` the
   SAME `PgTier` as source and destination: the destination insert
   dedupes on `(kind, id, digest)`, then the source delete removes
   the only row — silent record loss. Backward rules from `cold` are
   silently ignored (`R2Tier::age` reports zero).

Plus five contract violations in the same neighborhood: R2 `read`
panics on a malformed stored body (violates `Tier::read`'s
never-panic soft-violation contract); R2 `exists` collapses every
HEAD error to "absent"; aging-duration parsing accepts negatives and
panics on overflow; `canon ingest sessions`/`ingest artifacts` use
the all-or-nothing strict tier builder so an UNRELATED unset cold
bucket silently degrades every session to unwritten (exit 0, no
reason reported) while a malformed config is swallowed instead of
loud; and the lenient pg builder only degrades `TierUnavailable`, so
a genuinely unreachable Postgres hard-fails paths that promise to
degrade. Operator-surface drift rounds it out: `docker-compose.yml`
tells operators to export `CANON_S3_BUCKET` while every shipped
config names `CANON_R2_BUCKET`, `canon init --check-config` passes a
malformed `tiers.pg.schema`, and the README documents no env
contract at all.

## What Changes

- **`R2Tier` strict connection mode** (D1): in release builds
  (`cfg!(not(debug_assertions))`), `CANON_S3_ENDPOINT`,
  `CANON_S3_ACCESS_KEY`, and `CANON_S3_SECRET_KEY` become REQUIRED —
  a missing one is a `StoreError::BackendUnattached` naming every
  unset var. Debug builds keep the zero-env MinIO defaults. HTTP is
  permitted only for an explicit `http://` endpoint.
- **Forward-only aging** (D2): `TierPolicy::from_yaml` rejects any
  `aging.<kind>` whose `to` rung is not strictly colder
  (`local < hot < cold`) than the kind's routed rung, with a
  `PolicyError` naming kind, routed rung, and target rung.
- **Total aging-duration parsing** (D3): negative magnitudes are
  rejected; chrono `try_*` constructors replace the panicking ones.
- **R2 read validates before decoding** (D4): malformed rows/objects
  degrade to `EvidenceViolation`s, never a panic.
- **R2 `exists` propagates non-NotFound errors** (D5).
- **Kind-scoped, reason-carrying ingest tier builds** (D6): `canon
  ingest sessions`/`ingest artifacts` build only the rungs their
  kinds route to; a malformed config fails loud; an unavailable
  needed rung degrades to unwritten WITH the reason (env var name
  included) carried into the outcome and printed.
- **Pg connection outage classifies as `TierUnavailable`** (D7): the
  initial pool connect failure (not DDL/query failures) maps to
  `TierUnavailable`, so lenient builders degrade it as documented and
  strict paths report the named class.
- **Strict pg arm validates schema before DSN lookup** (D8), matching
  the lenient arm's already-fixed ordering.
- **`canon init --check-config` validates `tiers.pg.schema`** (D9).
- **Operator-surface repairs** (D10): `docker-compose.yml` quick
  start exports `CANON_R2_BUCKET`; README gains a "Live tiers: env
  contract" section (CANON_PG_DSN, CANON_R2_BUCKET, CANON_S3_*);
  `tiers.rs`'s stale "only caller is `tier age`" comment is
  corrected.

## Impact

- Affected specs: `store-hardening` (new capability delta).
- Affected code: `crates/canon-store/src/{r2_tier,policy,pg_tier}.rs`,
  `crates/canon-cli/src/{tiers,ingest,artifact_ingest,init}.rs`,
  `docker-compose.yml`, `README.md`.
- **Breaking (accepted):** a release deployment that relied on
  implicit MinIO defaults now fails loud until `CANON_S3_*` are
  exported; a `canon.yaml` carrying a same-rung/backward aging rule
  now fails to parse (it was destructive or dead config before).
