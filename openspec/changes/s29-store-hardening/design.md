# s29 store-hardening — design

Review provenance: two parallel reviewer passes (store: canon-store
internals; cli-config: canon-cli seams + operator surface),
2026-07-14. Finding numbers below cite the review, not new analysis.

## D1 — S3 strict connection mode is a build-profile split, not a flag

The existing release-safety seam already splits on
`cfg!(debug_assertions)` (`canon-cli/src/tiers.rs`'s
`CANON_R2_LOCAL_ROOT` substitution is debug-only). D1 reuses exactly
that boundary rather than inventing a `--dev` flag or a new env var:

- `R2Tier::connect_live` delegates to a `lookup`-parameterized,
  `strict`-parameterized builder (same testability pattern as
  `resolve_pg_dsn`) so BOTH branches are unit-tested in a debug test
  profile; the real caller passes
  `strict = cfg!(not(debug_assertions))`.
- Strict mode: `CANON_S3_ENDPOINT`, `CANON_S3_ACCESS_KEY`,
  `CANON_S3_SECRET_KEY` all required. The error is ONE
  `BackendUnattached` naming EVERY unset var (an operator fixes the
  whole set in one round trip, not one var per failure).
  `CANON_S3_REGION` keeps its `us-east-1` default in both modes — a
  wrong region is not a silent-misdirection risk the way a defaulted
  loopback endpoint is.
- `with_allow_http(...)` becomes `endpoint.starts_with("http://")`:
  plaintext stays possible for an operator who explicitly configured
  a plaintext endpoint (MinIO), and impossible as an ambient default.
- Bucket resolution (`bucket_env` → `S3_BUCKET`, then hard failure)
  is unchanged.

Rejected alternative: startup bucket probe (HEAD). `object_store`'s
builder does no I/O by contract, every other tier defers
reachability to first use, and a probe would make `canon query` on a
git-routed kind pay a network round trip. Strict-mode explicit
credentials already remove the misdirection; reachability stays a
first-I/O error.

## D2 — forward-only aging, validated at parse time

`Rung` (s27) already carries the total order `local < hot < cold`
conceptually; D2 makes it explicit (`Ord` on `Rung` or an
`ordinal()` helper — implementer's choice, but ONE place). At
`TierPolicy::from_yaml`:

- every `aging.<kind>` requires a `routing.<kind>` entry (an aging
  rule for an unrouted kind was already meaningless; now loud);
- `aging.<kind>.to` must be STRICTLY colder than the routed rung.
  Same-rung (the dedupe-then-delete data-loss case) and backward
  (the silently-dead `cold → hot` case) both reject with a
  `PolicyError` naming kind, routed rung, target rung, and the rule
  (`local < hot < cold`).

This also retires the R2 silent-no-op concern: a cold-routed kind
can no longer carry an aging rule at all (`cold` has no colder rung).

## D3 — total duration parsing

`parse_aging_duration` rejects a negative magnitude before
construction and swaps `Duration::{days,hours,minutes,seconds}` for
their `try_*` forms; both failure modes map to `PolicyError` naming
the offending literal. No behavior change for valid configs.

## D4 — R2 read validates rows, never panics

`R2Tier::read`'s decode path currently JSON-parses `body` and hands
it straight to the `expect`-based `raw_record_at`. D4 inserts the
same envelope validation the other tiers run (kind/id/at/digest
presence + RFC3339 `at`), producing one `EvidenceViolation` per bad
row (naming the object path) and continuing — the exact
`TierReadResult` soft-violation contract `tier.rs` documents.
`raw_record_at`'s precondition ("envelope validated") becomes true at
this call site instead of assumed.

## D5 — `exists` distinguishes absent from failed

`object_store::Error::NotFound` → `Ok(false)`; every other HEAD error
propagates as `StoreError::ObjectStore`. Duplicate writes stay
dedupe-correct under credentials that permit PUT but deny HEAD
(today they re-PUT and report `deduped: false`).

## D6 — ingest builds only the rungs it needs, and says why it degraded

`canon ingest sessions` writes `session`/`run`/`event`; `canon
ingest artifacts` additionally reads/writes its artifact kinds.
Both currently call the all-or-nothing strict `build_tiers`
(contradicting `tiers.rs`'s own module doc that `tier age` is its
only caller). D6:

- both commands build via the kind-scoped lenient path (union of the
  rungs their kinds actually route to — `build_lenient_tiers_for_kind`
  generalized to a kind SET);
- malformed config (schema validation, class mismatch, bad aging)
  stays LOUD — "lenient" keeps meaning reachability only;
- when a NEEDED rung is unavailable, the outcome carries the
  build-time reason (including the env var name) and the CLI prints
  it — no more bare "store tiers unreachable" guess-string, and no
  more exit-0-with-silently-dropped-reason.

## D7 — pg connect outage is `TierUnavailable`

`PgTier::connect`'s initial `PgPoolOptions::connect` failure maps to
`StoreError::TierUnavailable { backend: Postgres, reason: <sqlx
display> }`; the post-connect DDL statements keep mapping to
`StoreError::Sql` (a reachable-but-broken database is NOT an
availability degrade). This makes `attach_postgres`'s existing
`TierUnavailable`-only catch actually cover the outage case its doc
promises, in one place, without the CLI string-matching sqlx errors.

## D8 / D9 — config correctness is never masked

- Strict `build_tiers` pg arm calls `validate_schema_ident` BEFORE
  the `dsn_env` lookup (the lenient arm already does; the strict arm
  currently reports "unset DSN" first, masking a malformed schema).
- `canon init --check-config` runs the same schema validation over
  every configured pg rung, so `[PASS] tiers/routing/aging` can no
  longer be printed over a schema `PgTier::connect` would reject.

## D10 — operator surface tells one story

- `docker-compose.yml` quick-start comment/exports say
  `CANON_R2_BUCKET` (matching `canon.yaml`, `init.rs`, `policy.rs`'s
  sample — `CANON_S3_BUCKET` appears nowhere in shipped config).
- README gains a "Live tiers: env contract" section: one table —
  `CANON_PG_DSN` (full DSN URL; why a URL: single atomic secret,
  audit Pattern 6), `CANON_R2_BUCKET`, `CANON_S3_ENDPOINT`,
  `CANON_S3_ACCESS_KEY`, `CANON_S3_SECRET_KEY`, `CANON_S3_REGION`,
  compose defaults, and the debug-vs-release strictness note.
- `tiers.rs`'s module doc names the real `build_tiers` caller set.

## Explicitly NOT in scope

- Per-adapter tokio runtimes (cost note, no correctness defect).
- sqlx-internal DSN redaction (no canon-owned leak found; dependency
  risk only).
- A startup bucket-reachability probe (see D1 rejected alternative).
