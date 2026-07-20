# s29 store-hardening — tasks

## 1. R2 tier hardening (crates/canon-store/src/r2_tier.rs)

- [x] 1.1 D1: `lookup`+`strict`-parameterized S3 connection builder;
      strict requires `CANON_S3_ENDPOINT`/`CANON_S3_ACCESS_KEY`/
      `CANON_S3_SECRET_KEY`, one `BackendUnattached` naming every
      unset var; `connect_live` passes
      `strict = cfg!(not(debug_assertions))`; `allow_http` only for
      an explicit `http://` endpoint. Unit tests cover both branches
      from a debug test profile.
- [x] 1.2 D4: `read` validates each row's envelope before
      `raw_record_at`; malformed row/object → `EvidenceViolation`
      naming the object path; remaining objects still read; test with
      a `{}` body.
- [x] 1.3 D5: `exists` maps only `object_store::Error::NotFound` to
      `false`; other HEAD errors propagate as
      `StoreError::ObjectStore`; test via an erroring `ObjectStore`.

## 2. Policy hardening (crates/canon-store/src/policy.rs)

- [x] 2.1 D2: explicit `Rung` ordering (`local < hot < cold`);
      `from_yaml` rejects aging for unrouted kinds and non-forward
      `to` rungs with a `PolicyError` naming kind/routed/target/rule.
      Tests: same-rung, backward-from-cold, unrouted-kind, and the
      valid `hot → cold` case.
- [x] 2.2 D3: duration parsing rejects negative magnitudes and uses
      chrono `try_*` constructors; out-of-range → `PolicyError`
      naming the literal. Tests: `-1d`, `9223372036854775807d`.

## 3. CLI + pg seams (crates/canon-cli, crates/canon-store/src/pg_tier.rs)

- [x] 3.1 D7: `PgTier::connect` maps the initial pool-connect failure
      to `TierUnavailable { backend: Postgres, .. }`; DDL failures
      stay `Sql`.
- [x] 3.2 D6: `ingest sessions`/`ingest artifacts` build kind-scoped
      lenient tiers (kind-set generalization of
      `build_lenient_tiers_for_kind`); malformed config stays loud;
      degrade reason (env-var name included) carried into the
      outcome and printed. Offline tests: unrelated-unset-cold
      persists sessions; degraded outcome names the variable.
- [x] 3.3 D8: strict `build_tiers` pg arm validates schema before the
      `dsn_env` lookup; stale "only caller is `tier age`" module doc
      corrected.
- [x] 3.4 D9: `canon init --check-config` runs
      `validate_schema_ident` over every configured pg rung; test
      with `Bad-Schema`.
- [x] 3.5 D10: `docker-compose.yml` quick start exports
      `CANON_R2_BUCKET`; README "Live tiers: env contract" section
      (CANON_PG_DSN as DSN URL + why, CANON_R2_BUCKET, CANON_S3_*,
      compose defaults, debug-vs-release strictness).

## 4. Verification

- [x] 4.1 `cargo test --workspace` green with no live services and no
      `CANON_*` env vars exported.
- [x] 4.2 `canon selftest` and `canon gate selftest` still pass.
