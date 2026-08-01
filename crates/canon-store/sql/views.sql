-- canon-store DuckDB views (S2 design D5, unified-query spec) — a
-- read-only QUERY CONVENIENCE over the git and r2 tiers' own physical
-- files, layered stg_/int_/mart_ exactly like the donor consumer repo's
-- spec DB views:
--   stg_*  thin, source-shaped, content-trusted extraction over one
--          physical source (never `hive_partitioning=true` — the git
--          and r2 tiers' Hive directory/`kind=`/`area=` layout is
--          layout-ENFORCED separately by canon-store's Rust
--          `partition`/`git_tier` modules; these views trust the
--          record's OWN JSON/parquet `kind`/`at`/`scenario_id`
--          columns, the donor's ACTUAL mechanism per the design doc's
--          Risk section — see the donor parity-harness audit's
--          duckdb-views notes §3.2).
--   int_*  gate-equivalent derivations mirroring `canon-gate` (S5).
--          S5 has not shipped yet (S2 lands first, per the wave order)
--          — `int_evidence_verdicts` below is an explicit STUB tally
--          over `evidence_record`'s own `verdict` field, not a
--          `canon-gate` mirror; replace it wholesale once S5 ships the
--          real Rust derivation to mirror (never let this stub silently
--          become load-bearing).
--   mart_* persona-facing, read by `canon-report`/the dashboard (S9).
--
-- Rebindable roots (parity-harness D17 `GateCtx`-equivalent pattern,
-- design doc §8 testing strategy): `CANON_GIT_ROOT`/`CANON_R2_ROOT` env
-- vars point at the git tier's `tiers.git.root` and the r2 tier's local
-- (or synced) parquet root respectively — set them before
-- `duckdb -init sql/views.sql` so the same file opens against a fixture
-- corpus or a real consumer-repo checkout without editing this file.
--
-- S9 addition (`canon-report`/dashboard marts, design D1/D5): a THIRD
-- rebindable root, `CANON_LEARN_ROOT`, points at S6/S7/S8's
-- `canon-learn`-owned operator-local parquet store root
-- (`crates/canon-learn/src/config.rs::DEFAULT_LEARN_ROOT`, i.e.
-- `<repo>/.canon/learn`) — a physical source distinct from the git/r2
-- tiers above; see `stg_strategy_items`/`stg_trajectories` below for
-- why it needs its own root instead of reusing `CANON_R2_ROOT`.
--
-- ── Multi-version records: which views FOLD and which read the raw
--    version stream ─────────────────────────────────────────────────
--
-- Every tier is APPEND-ONLY at a natural key
-- (`crates/canon-store/src/partition.rs::resolve_partition`): a git/s3
-- rung writes a second digest-suffixed object and a hot rung appends a
-- second `records_history` row, so ONE logical record whose state
-- changed is TWO physical rows in `stg_records` below. This is not an
-- edge case — it is the normal shape of every state transition canon
-- records: `canon dispatch begin`/`end` (a `run`, running -> terminal,
-- s42), `canon gate task` (a `task`, checkbox flip), `canon subject
-- status` (a `subject`, proposed -> … -> shipped), a handoff
-- transition (a `handoff`), a divergence being resolved (a
-- `divergence`), and a re-attestation (an `evidence_record`).
--
-- `crate::query`'s Rust readers fold those versions to one winner per
-- natural key (`canon_store::fold_latest_by_key`, greatest
-- `(at, schema, digest)`). The views in this file DO NOT inherit that
-- fold and never have: `stg_records` is a plain `UNION ALL` over both
-- physical roots and every mart reads it directly, so a mart is
-- exposed to raw versions even though `canon query` for the same kind
-- is not. Each view therefore states its own posture below — the s42
-- re-review found `mart_session_costs` silently multiplying
-- `sum(cost)`/`sum(tokens_total)` by a dispatched run's version count.
--
-- The rule, applied view by view: a superseded version is a BUG when
-- it either (a) multiplies a measure sourced from a DIFFERENT relation
-- through a join, or (b) emits a contradictory current-state row (one
-- `task_id` reading both `open` and `done`). It is CORRECT when the
-- version itself IS the event being counted.
--
-- ── How a folding view folds ─────────────────────────────────────────
--
-- Every folding view below does it the SAME one way:
--
--   QUALIFY row_number() OVER (
--       PARTITION BY <this view's own natural key>
--       ORDER BY version_rank DESC
--   ) = 1
--
-- `version_rank` is defined ONCE, in `stg_records`, as the
-- `(at, schema, digest)` triple `canon_store::fold::fold_latest_by_key`
-- compares, materialized as a DuckDB STRUCT. Struct comparison is
-- field-by-field lexicographic, and RECURSIVELY so through the nested
-- `at` member below — both verified directly against this DuckDB, and
-- the same total order that function's own
-- `(at, schema, digest) < item_order` tuple comparison performs — so
-- `ORDER BY version_rank DESC` IS the Rust fold's ordering, written in
-- one place rather than re-spelled at each of the thirteen fold sites,
-- where one could silently drift from the rest. Two of the three
-- rungs are exact by construction and the third, `digest`, carries a
-- named residual on each root — the rung-by-rung section below states
-- each one, and is the thing to read before changing a member.
--
-- Ordering by `at` ALONE (what the first cut of these folds did) is
-- not a weaker form of that rule, it is a DIFFERENT rule, and it
-- disagrees with `canon query` on ordinary corpora. Equal `at` is
-- ROUTINE here, not theoretical: a plan-derived record stamps `at`
-- from its SOURCE DOCUMENT's mtime (s20 D7 — byte-stable, which is
-- what makes re-importing an unchanged plan idempotent), and a canon
-- PARSER change does not touch that mtime, so the stale and fresh
-- record for one key tie on `at`. `RecordKind::Task`'s
-- `schema_version` is `2` for exactly this reason
-- (`canon_model::records::Task::depends_on`'s own doc comment:
-- without the bump the fold "surfaced this field on an arbitrary
-- SUBSET of one file's rows"). And only a DISPATCHED `run` gets s42's
-- strictly-greater version-`at` (`canon-cli::dispatch::
-- close_version_at`) — `task`, `session`, `handoff`, `subject`,
-- `event` and the `porting.coverage` overlay get nothing of the kind.
-- On such a corpus an `at`-only fold retains the STALE body while
-- `canon query --kind <k>` returns the fresh one, from the same files.
--
-- `row_number()` over the whole row (rather than `arg_max` per column)
-- is also what keeps a folded row HONEST: `arg_max(x, …)` SKIPS rows
-- whose `x` is NULL, so a per-column fold emitting `arg_max(body ->>
-- '$.a', …), arg_max(body ->> '$.b', …)` can take `a` from the winner
-- and `b` from an older version whenever the winner emptied an
-- optional field (a `scenario_refs` list, an `ended_at`, an
-- `actor.agent_id`) — minting a row no stored version ever held.
-- `QUALIFY` selects one physical ROW, so every column, `"at"`
-- included, necessarily comes off that single version.
--
-- ── Rung by rung: what each mirrors, and where it cannot ─────────────
--
-- `schema` — EXACT, by construction. `version_rank`'s member is the
-- `canon_store::tier::raw_record_schema` FUNCTION rewritten in SQL
-- (see its definition below), not the raw stored integer: that
-- function reads the field as a `u64` and then narrows to `u32`,
-- flooring everything outside that domain to `0`, and DuckDB's own
-- `BIGINT` domain is neither. Read it there; it is the one rung where
-- there is nothing left over.
--
-- `digest` — a RESIDUAL on each root, and they are DIFFERENT residuals.
-- `crate::query`'s readers RECOMPUTE `partition::content_digest12`
-- from the body; no staging view can, because canonical-form hashing
-- is not expressible here — `content_digest12` hashes serde_json's
-- alphabetical-key, compact serialization of the value, and DuckDB has
-- no recursive JSON key-sorting canonicalizer that could reproduce
-- those exact bytes to hash. Each root instead reads a STORED copy of
-- that number, and each copy is trustworthy for exactly the corpus its
-- OWN Rust reader will hand you — a different enforcement point per
-- root, so the claim has to be made twice, never once:
--
--   git — `stg_git_records.digest` is the FILENAME's `__{digest12}`
--     suffix, which `GitTier::write` writes from `content_digest12`
--     and `GitTier::scan_kind_where` REJECTS as a `layout` violation
--     for any file whose path is not the one its own content resolves
--     to (its `expected != relative` check), as `scan_namespaced_kind`
--     does for overlay records. RESIDUAL: a hand-planted or renamed
--     `.json` whose NAME disagrees with its body. `canon query` never
--     returns it (soft-skipped as a violation); these views run no
--     layout gate and fold it at whatever its filename claims.
--
--   r2 — `stg_r2_records.digest` is the parquet `digest` COLUMN,
--     materialized by `R2Tier::write` from the same `content_digest12`
--     call that names the object, and re-checked against the body on
--     every read by `R2Tier::read`'s `validate_row` ("a stale/tampered
--     `digest` column is exactly as much a violation as a stale/
--     tampered `body`"), which soft-skips that ROW. RESIDUAL: a
--     parquet row whose `digest` COLUMN disagrees with its own `body`.
--     `canon query` never returns it; these views run no row validator
--     and fold it at whatever the column claims.
--
-- The SHAPE is therefore the same on both roots — SQL reads the very
-- value that root's Rust reader enforces against the content, so the
-- two agree by enforcement for every record the reader HANDS YOU, and
-- the residual is exactly the corpus that reader REFUSES — but the
-- enforcing function, the granularity (a whole FILE vs one ROW) and
-- the artifact a tamperer edits (a filename vs a parquet column) are
-- all different. Do not carry the git sentence over to r2. Both
-- residuals are pinned executably, not just asserted here, by
-- `crates/canon-report/tests/multi_version_fold.rs::
-- both_roots_fold_a_digest_the_rust_reader_refuses_to_return`.
--
-- Considered and REJECTED for r2: cross-checking the `digest` column
-- against the `__{digest12}` suffix the OBJECT KEY also carries
-- (`partition::hive_object_key` names r2 objects exactly as it names
-- git files, from the same digest). It looks like the git-side gate,
-- and it is not one. A check here only shrinks divergence when the
-- Rust reader performs the SAME check, and `R2Tier::read` does not:
-- it lists by the `kind=` prefix and hands the object path to
-- `decode_rows` for VIOLATION MESSAGES ONLY, never validating the key
-- against the row. Enforcing key == column in SQL would newly demote
-- rows `canon query` reads happily (object renamed, column intact) in
-- order to buy back rows it already refuses — one divergence class
-- traded for another, in a note whose whole value is that its claim
-- is exact.
--
-- `at` — EXACT, by construction, and deliberately NOT the `"at"`
-- COLUMN sitting beside it. `canon_store::tier::raw_record_at` is
-- `get("at").and_then(as_str).and_then(DateTime::parse_from_rfc3339)
-- .map(|dt| dt.with_timezone(&Utc))`: it reads the record's own JSON
-- `at` STRING, APPLIES the RFC3339 offset, and compares INSTANTS at
-- nanosecond precision. `version_rank`'s member is that function
-- rewritten in SQL over the same `body ->> '$.at'` string — on BOTH
-- roots, because that string is what `raw_record_at` reads on both
-- (an r2 `RawRecord` is the parquet `body` column parsed as JSON, not
-- the `at` column beside it) — materialized as a nested
-- `{parsed, sec, nano}` struct whose lexicographic order IS
-- `DateTime<Utc>`'s.
--
-- The `"at"` COLUMN is a `TIMESTAMP` cast of that same text — a naive
-- MICROSECOND wall clock, and WAS this rung until the s42 re-review.
-- It is lossy twice over — verified directly against this DuckDB,
-- 2026-08-01:
--
--   * The offset is DISCARDED, not applied.
--     `'2026-01-01T12:00:00+05:00'::TIMESTAMP` is `12:00`, while
--     `raw_record_at` makes it `07:00Z`. Two versions of one key at
--     the SAME instant, one stamped with an offset, ranked in
--     OPPOSITE orders here and in `canon query`. Every canon writer
--     stamps a `DateTime<Utc>`, so the exposure is hand-authored or
--     imported JSON — `validate_envelope_shape` asks only that `at`
--     be SOME parseable RFC3339 string, never a UTC one.
--   * Sub-microsecond precision is TRUNCATED, so two versions
--     differing only below 1µs tied here and did not in Rust. That
--     one is not hand-authored at all:
--     `canon-cli::dispatch::close_version_at` bumps a closing run's
--     version stamp by exactly ONE NANOSECOND whenever the host clock
--     is not monotone, so a dispatched run's begin/close pair is
--     precisely this case.
--
-- The COLUMN keeps its `TIMESTAMP` type, name and position — every
-- downstream mart and the snapshot/dashboard column contract already
-- read it as one, so it could not be re-typed for the rung's sake.
-- The RUNG moved off it instead. `TIMESTAMPTZ` was rejected for the
-- rung too: it reinterprets an OFFSET-LESS string in the session
-- `TimeZone`, which would make `version_rank` depend on an
-- environment setting this file is otherwise careful to exclude (see
-- the `default_null_order` note below). The struct below reaches a
-- UTC instant with no time-zone-sensitive type anywhere in it — a
-- `DATE`, integer seconds, integer nanoseconds — and was probed
-- byte-identical under `SET TimeZone` = `UTC`, `America/New_York`,
-- `Asia/Kolkata` and `Pacific/Chatham` (a :45 zone), 2026-08-01.
--
-- The accepted SURFACE is chrono's, not DuckDB's, because a `CAST`
-- and `parse_from_rfc3339` do not admit the same strings. The regex
-- below is `chrono-0.4.45::format::parse::parse_rfc3339` read field
-- by field, so a string that function accepts parses here and a
-- string it rejects floors here:
--
--   * lowercase `t`/`z` and a SPACE date/time separator — chrono
--     takes all three; a DuckDB `TIMESTAMP` cast takes the space and
--     returns NULL for the lowercase forms, which under the old rung
--     sank a record `canon query` reads fine to `-infinity`;
--   * U+2212 MINUS SIGN as an offset sign
--     (`scan::timezone_offset`'s `allow_tz_minus_sign`);
--   * a fraction of ANY length, with digits past the 9th SKIPPED
--     (`scan::nanosecond`), never rounded;
--   * `:60` as a leap second — chrono stores it as `:59` plus a
--     `nano` of `1_000_000_000`, which is why `nano` is a member of
--     its OWN instead of being folded into `sec`: only the
--     lexicographic pair puts `23:59:60Z` after `23:59:59Z` and still
--     BEFORE the next day's `00:00:00Z`, which is what chrono's
--     `(date, secs, frac)` comparison does. Summing them would
--     silently INVERT that second pair;
--   * a MANDATORY offset with a MANDATORY `:` — `…T07:00:00` and
--     `…T07:00:00+0500` are both chrono errors, so both floor.
--
-- The range checks below are explicit for the same reason: DuckDB
-- does not share chrono's. `'2026-01-01 24:00:00'::TIMESTAMP`
-- silently rolls to the next day where `NaiveTime::from_hms_nano_opt`
-- returns `None`, and the offset bound is `FixedOffset::east_opt`'s
-- `|offset| < 24h`, not the two digits the grammar allows. Calendar
-- validity is left to DuckDB's `DATE` cast, which does agree with
-- `NaiveDate::from_ymd_opt` — probed across month `00`/`13`, day
-- `00`/`32`, `2025-02-29` (rejected by both) and `2024-02-29`
-- (accepted by both), plus year `0000`, which both read as the
-- proleptic `1 BC` day at epoch second `-62167219200`.
--
-- RESIDUAL, and it is not an ordering one: where `at` is absent or is
-- a non-string JSON value, `raw_record_at` PANICS on its own `expect`
-- and this rung floors instead — `parsed: 0`, which loses to every
-- well-formed record and so can only lose, never INVERT a pair.
-- `canon_model::evidence::validate_envelope_shape` is what keeps that
-- unreachable for any record either root's reader will hand you; the
-- floor exists so a hand-planted file cannot decide a fold.
--
-- Every rung's fallback — `parsed: 0`, `0`, `''` — is the value that
-- LOSES to every well-formed record, the posture
-- `canon_store::tier::raw_record_schema` documents for its own `0`
-- ("a record whose `schema` is missing or non-integer can never
-- out-rank a well-formed record of the same key"). An unreadable git
-- filename suffix reads `''`, which sorts below every real 12-hex
-- digest, for the identical reason. Reading that suffix is the ONE
-- thing this file derives from `record_path`, and it is narrow on
-- purpose — the digest suffix is kind-INDEPENDENT (one end-anchored
-- regex serves all 14 kinds and every namespaced overlay), unlike the
-- natural key, whose grammar is per-kind and is precisely the second
-- implementation the last section below refuses to write.
--
-- ── Fold inventory: all thirteen fold sites, and each one's key ──────
--
-- Each key below is `resolve_partition`'s natural key for that kind,
-- re-verified field by field against `partition.rs`:
--
--   `mart_trust_matrix`'s `evidence_current` — `task_id`, the key
--     `partition.rs`'s `EvidenceRecord` arm resolves for a task-scoped
--     attestation. Reads `version_rank` through `int_task_evidence`,
--     which passes the column through but does NOT itself fold (it is
--     on the raw-stream list below, because its grain is one row per
--     attestation).
--   `int_task_scenario_refs`'s `task_latest` — `task_id`.
--   `mart_trust_matrix`'s `tasks` — `task_id`.
--   `mart_scope_status`'s `cov` — `(project_id, scenario_id)`. NOT
--     `scenario_id` alone: that is the `porting.coverage` overlay's
--     own declared `join_key` (`.canon/plugins/porting/plugin.yaml`),
--     and the pair is what `GitTier::write_namespaced` builds its
--     `{project_id}__{scenario_id}` natural key from. Keying
--     `scenario_id` alone MERGES two spec roots that share a scenario
--     id and then returns an arbitrary winner — reporting one
--     project's coverage under another's, which is worse than the
--     double-count folding replaced.
--   `mart_session_costs`'s `token_usage` — `(run_id, seq)`, the two
--     fields `Event`'s `{run_id}-{seq:010}` key is built from.
--   `mart_session_costs`'s `runs` · `mart_session_run_handoff`'s
--     `runs` — `run_id`.
--   `mart_session_costs`'s `sessions` ·
--     `mart_session_run_handoff`'s `sessions` — the body's own
--     `session_id`. This is the one key that is deliberately NOT
--     literally `resolve_partition`'s output: that arm keys `Session`
--     by `sanitize_component(session_id)` (`/` -> `_`, so the key is
--     filename-safe). These folds group by the raw SEMANTIC id, one
--     rung FINER — sanitizing is many-to-one, so two distinct session
--     ids can share a filename key, and a fold at the finer key can
--     therefore never merge two records the coarser key would keep
--     apart. Finer is the safe direction; the reverse would not be.
--   `mart_session_run_handoff`'s `handoffs` — `id`.
--   `mart_subjects`'s `subjects` — `subject_id`.
--   `mart_subjects`'s `scenario_latest_verdict` — `scenario_id`, the
--     `partition.rs` `EvidenceRecord` arm's second fallback. Like
--     `evidence_current` this is a current-verdict pick, not a
--     natural-key fold: an attestation carrying BOTH a `task_id` and
--     a `scenario_id` partitions under the `task_id`, so grouping by
--     `scenario_id` deliberately spans every attestation ABOUT that
--     scenario — the same `(subject, role)` cell grain
--     `canon-gate::ledger::latest_verdicts` folds, which is the
--     function this pick and `mart_trust_matrix`'s `green` both cite.
--
--   `mart_review_rounds`'s `finding_latest` — `(change_id, round,
--     seq)`, the three fields `partition.rs`'s `Finding` arm builds
--     its `{change_id}__{round:04}__{seq:04}` key from. Grouped in the
--     raw text form `body ->> …` returns, exactly like
--     `mart_session_costs`' `token_usage` fold: JSON forbids a leading
--     zero on a number, so a well-formed `round` of 1 is the text `1`
--     on every root and the padding `resolve_partition` adds is a
--     filename concern, not a grouping one. `reviewed_sha` is
--     deliberately NOT in the key — a round that reviewed an
--     uncommitted worktree has none (canon-model's `Finding` doc), so
--     keying on it would give one round two identities, or none.
--     Folding matters here for the same reason it does on
--     `mart_subjects`: a finding's DISPOSITION is a lifecycle
--     (`open` -> `fixed`), so one finding re-authored as fixed is two
--     versions at one key, and unfolded it would be counted twice in
--     `findings` while appearing under BOTH `disposition_open` and
--     `disposition_fixed` — and its superseded version's absent
--     `introduced_by` would inflate the UNSOURCED bucket the panel
--     reports as a known unknown.
--     `mart_review_totals` reads THIS view, never `stg_records`, so
--     it inherits this one fold and adds no fold site of its own —
--     which is why it appears on neither list here.
--
--   READ THE RAW VERSION STREAM, deliberately — never "not yet
--   folded":
--     `stg_*` — source-shaped by definition (this header's own
--       contract, and `mart_records_by_kind`'s census depends on it);
--       the fold belongs in the consuming view, never here.
--     `mart_records_by_kind` — a PHYSICAL census, "how many records
--       live in each tier". Folding would destroy the question it
--       answers.
--     `mart_review_burndown` — the transition IS the curve. A
--       divergence opened day 1 and resolved day 3 must contribute
--       `+1` then `-1`; folding to its latest version contributes
--       only the `-1` and drives `divergence_open_running_total`
--       NEGATIVE.
--     `int_evidence_verdicts` and `int_task_evidence` (including
--       `mart_trust_matrix`'s `evidence_count`/`latest_at` over it) —
--       an `EvidenceRecord` is keyed BY its own join key
--       (`partition.rs`'s three-way fallback, in this precedence
--       order: `task_id`, then `scenario_id`, then `run_id`, and
--       `"unscoped"` when it carries none of the three), so canon
--       cannot hold two DISTINCT attestations for one subject at all:
--       a re-attestation is necessarily a new version, and counting
--       it counts the attestation that actually happened. Every
--       consumer needing current state instead of history
--       (`mart_trust_matrix`'s `green`/`who` via `evidence_current`,
--       `mart_subjects`'s `scenario_latest_verdict`) picks the winner
--       by `version_rank`, listed in the fold inventory above.
--     `mart_flywheel_funnel` — structurally immune, and verified so
--       against a two-version run: its retrieval stages are
--       `count(DISTINCT strategy_id)` over `retrieved_guidance` and a
--       `GROUP BY role, strategy_id` over `applied_scored`, so a run
--       stored twice collapses to the same single `(role, strategy)`
--       pair. `mart_role_memory` reads no `stg_records` at all (only
--       `stg_strategy_items`, whose store is one file per
--       content-derived id, rewritten wholesale by
--       `rebuild_namespace` rather than appended).
--
-- Why per-view folds and NOT one shared `stg_latest_records`: such a
-- view needs a `natural_key` column, and supplying it means
-- re-deriving the key from `body` in a 14-arm `CASE` reimplementing
-- `resolve_partition` (`Event`'s `{run_id}-{seq:010}`, the composite
-- `scenario`/`review`/`divergence`/`finding` keys, `Session`'s
-- `sanitize_component`, `EvidenceRecord`'s three-way fallback) — a
-- SECOND implementation of the join-spine key grammar that drifts
-- silently the moment `partition.rs` changes, the exact
-- second-derivation risk design D1 exists to prevent and the same
-- reason `mart_trust_matrix` refuses to replicate `TrustRung::
-- green()`. Parsing it back out of the git filename instead (where it
-- does sit, ahead of the `__{digest12}` suffix this file DOES read)
-- trades that drift for a different one: `{natural_key}__{digest12}`
-- is only unambiguously splittable from the RIGHT, so the recovered
-- key is an opaque string that no longer knows which fields it was
-- built from — enough to group by, useless for the per-field
-- reasoning `mart_scope_status`'s `(project_id, scenario_id)` fix
-- above required, and still a second key derivation to keep in sync.
-- It could not be a drop-in `stg_records` replacement either, because
-- the raw-stream readers above must NOT fold, so every consumer would
-- still have to pick a posture — which is the actual work. A per-view
-- fold needs no key grammar at all (each view already knows its own
-- key, in the fields it actually joins on) and forces that pick to be
-- written down where the view is read.

INSTALL json;
LOAD json;

-- ── stg_* ────────────────────────────────────────────────────────────

-- Thin, content-trusted extraction over the git tier's Hive-laid-out
-- JSON files — `read_text` + JSON-payload column pulls, NEVER
-- `hive_partitioning=true` (design doc's Risk section: the donor's
-- ACTUAL mechanism, not the aspirational one). `schema`/`kind`/`at`/
-- `scenario_id` come from the record's own body, RAW — `schema` in
-- particular is the stored integer, not the `u32` the fold compares;
-- `stg_records`' `version_rank` is where that narrowing happens.
--
-- `record_path` is kept for provenance, and `digest` is the ONE column
-- derived from it: the filename's own `__{digest12}` suffix, which
-- `GitTier::write` writes from `partition::content_digest12` and
-- `GitTier::scan_kind_where`/`scan_namespaced_kind` reject the record
-- outright for disagreeing with. It is the third rung of
-- `fold_latest_by_key`'s `(at, schema, digest)` order and cannot be
-- recomputed in SQL (no canonical-form JSON hashing in DuckDB) — see
-- this file's header for the enforcement chain that makes the
-- filename value identical to the content-derived one, and for the
-- exact residual gap. `''` when the suffix is absent or malformed,
-- which sorts below every real digest. NOTHING ELSE is taken from
-- `record_path`: `kind`/`at`/`scenario_id` in particular are never
-- read from the path, however legible it is.
CREATE OR REPLACE VIEW stg_git_records AS
WITH raw AS (
    SELECT
        filename                AS record_path,
        CAST(content AS JSON)   AS j
    FROM read_text(getenv('CANON_GIT_ROOT') || '/kind=*/**/*.json')
)
SELECT
    record_path,
    try_cast(j ->> '$.schema' AS BIGINT)                      AS schema,
    j ->> '$.kind'                                            AS kind,
    try_cast(j ->> '$.at' AS TIMESTAMP)                       AS "at",
    j ->> '$.scenario_id'                                     AS scenario_id,
    regexp_extract(record_path, '__([0-9a-f]{12})\.json$', 1) AS digest,
    j                                                         AS body
FROM raw;

-- Thin extraction over the r2 tier's parquet exports. Unlike the git
-- tier, `kind`/`natural_key`/`at`/`digest` are already real typed
-- parquet columns (materialized at write time by `canon-store`'s
-- `R2Tier`, not re-derived here) — and its `digest` is the SAME
-- `partition::content_digest12` value the git filename carries,
-- re-validated against the body on every read
-- (`r2_tier.rs::validate_row`: "a stale/tampered `digest` column is
-- exactly as much a violation as a stale/tampered `body`"), so the two
-- tiers' `digest` columns are directly comparable in one fold — see
-- this file's header for what that enforcement does and does NOT
-- cover on THIS root, which is not what it covers on the git one.
-- `schema` is the one fold rung r2 does NOT materialize, so it is
-- pulled from `body` here, raw and unnarrowed exactly as the git view
-- pulls it — `body` remains the JSON source of truth for anything
-- this view doesn't already surface as a column.
CREATE OR REPLACE VIEW stg_r2_records AS
SELECT
    kind,
    natural_key,
    CAST("at" AS TIMESTAMP)                              AS "at",
    try_cast(CAST(body AS JSON) ->> '$.schema' AS BIGINT) AS schema,
    digest,
    CAST(body AS JSON)                                   AS body
FROM read_parquet(getenv('CANON_R2_ROOT') || '/kind=*/**/*.parquet');

-- One normalized, source-tagged view over BOTH physical local roots —
-- the shape `canon query`'s own Rust fan-out/merge (D4) also
-- produces, so a dashboard reading this view and a CLI caller reading
-- `canon query` never disagree about what a merged read looks like.
--
-- This view is exhaustively `stg_git_records UNION ALL
-- stg_r2_records`, no third source: Postgres has ZERO SQL view here
-- (canon-report never opens a live DB connection, s25/s27/s28
-- design). `stg_r2_records` scans whatever parquet sits at
-- `CANON_R2_ROOT` (a LOCAL directory) — for an S3-backed rung this is
-- only a local MIRROR, never the live bucket itself, and canon has no
-- automatic sync keeping it current (s28 `rung-backend-capability`
-- design D2/D3, correcting s27's `offline_file_readable()`, which
-- wrongly treated S3 as always report-visible). Data routed to a
-- rung whose backend `crates/canon-store/src/policy.rs::Backend::
-- read_directly_by_report()` reports `false` for (Postgres always;
-- S3 unless a local `.canon/r2` mirror happens to be current) is NOT
-- lost: it stays live-readable via `canon query --kind <kind>`
-- (`canon-cli`'s own tier fan-out, s22 `query-tier-degradation`) —
-- this view deliberately never grows a live-backend-reading
-- counterpart (s25 `report-pg-tier-boundary` design D4, generalized
-- by s27 D2, corrected by s28 D2: no live, non-directly-read-backend
-- read in `canon-report`'s offline/deterministic rendering path).
-- `crates/canon-report/src/tier_boundary.rs`'s backend-capability-
-- keyed derivation reads `canon.yaml`'s `routing`/`tiers` tables
-- directly (never this view) to surface that gap LOUD in `canon
-- report`'s own `## Kinds not read directly` section + stderr `WARN`,
-- matching this file's own established "name every stub/proxy/gap it
-- contains" convention (`int_evidence_verdicts`'s STUB note below is
-- the direct precedent).
--
-- `version_rank` is defined here and NOWHERE else: the `(at, schema,
-- digest)` triple `canon_store::fold::fold_latest_by_key` compares,
-- as a STRUCT, so every folding view downstream orders by ONE
-- expression instead of restating three rungs it could get wrong
-- (header's "How a folding view folds"). Each member falls back to
-- the value that LOSES to every well-formed record — `parsed: 0`,
-- `0`, `''` — so no member is ever NULL and the order is total
-- without depending on DuckDB's `default_null_order` setting. The
-- header's rung-by-rung section states what each member does and does
-- not guarantee against the Rust fold; read it before changing one.
--
-- The `schema` member is `canon_store::tier::raw_record_schema`
-- rewritten, NOT the `schema` column beside it. That function is
-- `get("schema").and_then(as_u64).and_then(u32::try_from).unwrap_or(0)`
-- — a `u32` domain reached through a `u64` one — while the column is
-- a DuckDB `BIGINT` pulled with `->>`, which is signed, 64-bit, and
-- (because `->>` unquotes) also accepts JSON strings and rounds JSON
-- doubles. Every value in the gap between those domains is a value
-- the two folds ORDER DIFFERENTLY, and `validate_envelope_shape`
-- guards none of it: it admits any `is_u64() || is_i64()` integer, so
-- an overlay row may legally carry `schema: 4294967296` (Rust: `0`,
-- the bare column: the largest rank in the corpus) or `schema: -1`
-- (Rust: `0`, the bare column: below a malformed sibling's `0`).
-- `json_type` is what closes it: `as_u64()` succeeds for exactly the
-- JSON values DuckDB types `UBIGINT`, so the guard below floors the
-- same set the function floors — negatives (`BIGINT`), doubles
-- (`DOUBLE`), quoted numbers (`VARCHAR`), booleans, absence, and any
-- `UBIGINT` past `u32::MAX` — and passes everything else through
-- exactly, `u32::MAX` fitting `BIGINT` losslessly.
--
-- The bare `schema` COLUMN stays the raw stored value on purpose:
-- `stg_git_records`/`stg_r2_records` are thin extractions and this
-- view is where the fold semantics are added. Nothing may order by
-- the column — `version_rank` is the only fold rung, and the thirteen
-- fold sites all inherit it from here.
CREATE OR REPLACE VIEW stg_records AS
WITH unioned AS (
    SELECT kind, "at", scenario_id, 'git' AS source_tier, schema, digest, body FROM stg_git_records
    UNION ALL
    SELECT kind, "at", body ->> '$.scenario_id' AS scenario_id, 'r2' AS source_tier, schema, digest, body FROM stg_r2_records
),
-- `raw_record_at`'s RFC3339 grammar, read off chrono 0.4.45's
-- `format::parse::parse_rfc3339` field by field (header's `at` rung
-- note lists what that admits and what a DuckDB cast does not).
-- Applied to `body ->> '$.at'` — the record's OWN JSON string, which
-- is what `raw_record_at` reads on both roots — never the `"at"`
-- column, which is already a lossy naive-microsecond cast of it. A
-- non-match leaves every field `''`, which floors in `version_rank`.
at_text AS (
    SELECT
        unioned.*,
        regexp_extract(
            body ->> '$.at',
            '^(\d{4}-\d{2}-\d{2})[Tt ](\d{2}):(\d{2}):(\d{2})(?:\.(\d+))?(?:[Zz]|([-+\x{2212}])(\d{2}):(\d{2}))$',
            ['ymd', 'hh', 'mi', 'ss', 'frac', 'sign', 'oh', 'om']
        ) AS at_parts
    FROM unioned
),
-- Every field of that match as the number chrono derives from it.
-- `try_cast` for each one the regex leaves `''` on a non-match, so a
-- non-match yields NULLs rather than a cast error and the guard in
-- `version_rank` floors the row. `frac` needs none:
-- `rpad(left(…, 9), 9, '0')` is always exactly nine digits —
-- `'000000000'` when there is no fraction — which is also how chrono
-- scales a SHORT fraction (`.5` is 500000000ns) and drops a long
-- one's tenth digit onward. `ss` is left as the parsed `60` rather
-- than clamped here, because `version_rank` needs to see the leap
-- second to add chrono's `1_000_000_000` to `nano`.
at_utc AS (
    SELECT
        at_text.* EXCLUDE (at_parts),
        try_cast(at_parts['ymd'] AS DATE)                                AS at_date,
        try_cast(at_parts['hh'] AS BIGINT)                               AS at_hh,
        try_cast(at_parts['mi'] AS BIGINT)                               AS at_mi,
        try_cast(at_parts['ss'] AS BIGINT)                               AS at_ss,
        CAST(rpad(left(at_parts['frac'], 9), 9, '0') AS BIGINT)          AS at_frac_ns,
        CASE at_parts['sign'] WHEN '' THEN 0 WHEN '+' THEN 1 ELSE -1 END AS at_off_sign,
        coalesce(try_cast(at_parts['oh'] AS BIGINT), 0)                  AS at_off_hh,
        coalesce(try_cast(at_parts['om'] AS BIGINT), 0)                  AS at_off_mi
    FROM at_text
)
SELECT
    kind,
    "at",
    scenario_id,
    source_tier,
    schema,
    digest,
    body,
    {
        'at':     CASE
                      WHEN at_date IS NOT NULL
                       AND at_hh <= 23 AND at_mi <= 59 AND at_ss <= 60
                       AND at_off_mi <= 59
                       AND at_off_hh * 3600 + at_off_mi * 60 <= 86399
                      THEN {
                          'parsed': 1,
                          'sec':    date_diff('second', DATE '1970-01-01', at_date)
                                    + at_hh * 3600 + at_mi * 60 + least(at_ss, 59)
                                    - at_off_sign * (at_off_hh * 3600 + at_off_mi * 60),
                          'nano':   CASE WHEN at_ss = 60 THEN 1000000000 ELSE 0 END + at_frac_ns
                      }
                      ELSE { 'parsed': 0, 'sec': 0::BIGINT, 'nano': 0::BIGINT }
                  END,
        'schema': CASE
                      WHEN json_type(body, '$.schema') = 'UBIGINT'
                       AND schema BETWEEN 0 AND 4294967295 THEN schema
                      ELSE 0
                  END,
        'digest': coalesce(digest, '')
    } AS version_rank
FROM at_utc;

-- S9 addition: `canon-learn`'s (S6/S7/S8) own operator-local parquet
-- stores — `ParquetStrategyStore`/`ParquetTrajectoryStore`
-- (`crates/canon-learn/src/store/{parquet_strategy,parquet_trajectory}.rs`).
-- Verified against that crate's source, 2026-07-11: this store never
-- goes through `canon-store::TierRegistry` (S6 design's OQ2
-- parquet-first pivot; `crates/canon-learn/src/store/mod.rs` module
-- doc), so it does NOT appear under `CANON_GIT_ROOT`/`CANON_R2_ROOT`'s
-- `kind=*/**` layout above — it is Hive-nested
-- `<learn_root>/{strategies,trajectories}/<role>/<repo>/<area>/<hash>/
-- <id>.parquet` instead (`crates/canon-learn/src/store/path.rs::
-- namespace_dir`), hence `CANON_LEARN_ROOT` (module header). Both
-- stores share the identical 5-column encoding — `id`, `regime_key`,
-- `role`, `recorded_at` (RFC3339 text), `body` (one JSON blob column,
-- same "typed key columns + JSON body" shape `stg_r2_records` already
-- uses above) — so `body` carries every OTHER field (title/content/
-- demotion/verdicts/outcome/reward/…), read exactly like
-- `stg_r2_records.body`.
CREATE OR REPLACE VIEW stg_strategy_items AS
SELECT
    id,
    regime_key,
    role,
    CAST(recorded_at AS TIMESTAMP) AS recorded_at,
    CAST(body AS JSON)             AS body
FROM read_parquet(getenv('CANON_LEARN_ROOT') || '/strategies/*/*/*/*/*.parquet');

CREATE OR REPLACE VIEW stg_trajectories AS
SELECT
    id,
    regime_key,
    role,
    CAST(recorded_at AS TIMESTAMP) AS recorded_at,
    CAST(body AS JSON)             AS body
FROM read_parquet(getenv('CANON_LEARN_ROOT') || '/trajectories/*/*/*/*/*.parquet');

-- ── int_* ────────────────────────────────────────────────────────────

-- STUB — see the file header. Tallies `evidence_record` verdicts
-- straight from the record body; this is NOT yet a mirror of any real
-- `canon-gate` (S5) derivation (S5 doesn't exist at S2's own commit
-- time) — S5 must replace this view's body, not merely extend it, once
-- it ships the trust-spine logic this is meant to mirror.
CREATE OR REPLACE VIEW int_evidence_verdicts AS
SELECT
    body ->> '$.verdict' AS verdict,
    count(*)             AS n
FROM stg_records
WHERE kind = 'evidence_record'
GROUP BY 1;

-- S9 addition: one row per (task_id, evidence_record) — the join-spine
-- fold `mart_trust_matrix` groups from below, kept as its own `int_*`
-- layer so a future S5 real trust-ladder derivation (see
-- `int_evidence_verdicts`'s own STUB note above) has one obvious place
-- to extend rather than a second copy of this same join.
--
-- Deliberately UNFOLDED (header's raw-stream list): the grain is one
-- row per ATTESTATION, which is what `mart_trust_matrix`'s
-- `evidence_count` counts. `version_rank` is carried through so the
-- consumers that DO need the current attestation
-- (`mart_trust_matrix`'s `evidence_current`) pick their winner by the
-- same `(at, schema, digest)` order `canon-gate::ledger::
-- latest_verdicts` folds this same evidence by, rather than by `at`
-- alone — which would decide an equal-`at` pair of verdicts
-- arbitrarily while the gate it claims to mirror decided it by
-- generation.
CREATE OR REPLACE VIEW int_task_evidence AS
SELECT
    body ->> '$.task_id'               AS task_id,
    "at"                               AS evidence_at,
    body ->> '$.verdict'               AS verdict,
    body -> '$.actor' ->> '$.agent_id' AS who,
    version_rank
FROM stg_records
WHERE kind = 'evidence_record' AND (body ->> '$.task_id') IS NOT NULL;

-- s20 addition (task-scenario-join spec, design D1/D3): one row per
-- declared `(task_id, scenario_id)` pair, `UNNEST`ing `Task.
-- scenario_refs` (`canon_model::records::Task::scenario_refs`, an
-- additive, empty-by-default field — see that field's own doc comment)
-- straight from `stg_records`. A `Task` with no declared refs (the
-- overwhelming majority today, and every pre-s20 `Task`) contributes
-- no row here at all — this view is purely additive over the PLAN
-- side's own explicit `[covers: …]` declarations, never a heuristic
-- derivation (task-scenario-join spec's own "never inference over
-- prose" bar).
--
-- Folded to each `task_id`'s LATEST version (header's fold list): a
-- `canon gate task` checkbox flip appends a second `task` row at the
-- same key, and unfolded that emitted the declared pair TWICE — which
-- `mart_scope_status` then multiplied against its own unfolded task
-- join into four rows for one declared pair. What this view means is
-- "the refs the plan declares NOW", so a ref dropped by a later
-- rewrite must stop producing a row.
CREATE OR REPLACE VIEW int_task_scenario_refs AS
WITH task_latest AS (
    SELECT
        body ->> '$.task_id' AS task_id,
        body
    FROM stg_records
    WHERE kind = 'task'
    QUALIFY row_number() OVER (PARTITION BY body ->> '$.task_id' ORDER BY version_rank DESC) = 1
)
SELECT
    t.task_id            AS task_id,
    g ->> '$'            AS scenario_id
FROM task_latest t, UNNEST(from_json(t.body -> '$.scenario_refs', '["JSON"]')) AS u(g)
WHERE (t.body -> '$.scenario_refs') IS NOT NULL;

-- ── mart_* ───────────────────────────────────────────────────────────

-- Persona-facing: how many records of each kind live in each tier —
-- the smallest useful cross-tier rollup, and the one `canon-report`/the
-- dashboard (S9) can build on without re-deriving the stg_ layer.
CREATE OR REPLACE VIEW mart_records_by_kind AS
SELECT kind, source_tier, count(*) AS n
FROM stg_records
GROUP BY 1, 2
ORDER BY 1, 2;

-- S9 addition (design D5): the five dashboard-panel marts. Every one
-- reads ONLY `stg_*`/`int_*` views already defined in this file, never
-- a second Rust-side aggregation (design D1) — `canon-report` renders
-- these, it does not recompute them. Every interim proxy below (used
-- only where an upstream record shape does not YET carry the exact
-- field a panel calls for) is named explicitly in its own comment,
-- mirroring `int_evidence_verdicts`'s own "explicit STUB, never
-- silently load-bearing" precedent — replace the cited proxy wholesale,
-- not patch around it, once the upstream field lands.

-- Panel 1: change/task trust matrix (covered × green × who). `covered`
-- = at least one `evidence_record` exists for this `task_id`; `green`
-- = the LATEST such record's `verdict` is `faithful` (last-wins-by-
-- `at`, mirroring `canon-gate::ledger`'s own fold rule). `lifecycle`
-- data lives in an interim `trust_ladder` companion JSON key
-- `canon-gate` writes onto a raw `evidence_record` body
-- (`crates/canon-gate/src/trust_ladder.rs`'s own "INTERFACE REQUEST to
-- canon-model": `EvidenceRecord` carries no native `lifecycle`/
-- `flagged` field yet) — `green` here is deliberately NOT redefined
-- against it: replicating `TrustRung::green()`'s full multi-rung
-- classifier in SQL would recreate exactly the second-aggregation-
-- layer risk D1 exists to prevent. This mart's `green` is the plain
-- `verdict = 'faithful'` proxy, the same STUB posture
-- `int_evidence_verdicts` already establishes, until S5 ships
-- `lifecycle`/`flagged` as native `EvidenceRecord` fields (S1
-- follow-up).
--
-- `tasks` is folded to each `task_id`'s LATEST version (header's fold
-- list). Unfolded, a `canon gate task` checkbox flip made this mart
-- emit TWO rows for one task — one reading `task_status open`, one
-- `done`, each repeating the same `evidence_count` — i.e. a
-- contradictory current-state answer, and a doubled denominator for
-- any caller counting rows.
--
-- The evidence side is split in two on purpose, because the two halves
-- ask opposite questions of the same relation:
--   `evidence_counts`  — HISTORY. `evidence_count` counts
--                        ATTESTATIONS, versions included (header's
--                        raw-stream list): an `EvidenceRecord` is
--                        keyed by its `task_id`, so a re-attestation
--                        can only ever arrive as a new version, and
--                        counting it counts the attestation that
--                        actually happened. `latest_at` is that
--                        stream's high-water mark.
--   `evidence_current` — CURRENT STATE, feeding `green`/`who`. One
--                        WHOLE winner row per `task_id`, picked by
--                        `version_rank`. This was a pair of
--                        per-column `arg_max(…, evidence_at)` calls,
--                        wrong twice over: `arg_max(who, …)` SKIPS
--                        rows whose `who` is NULL (an actor with no
--                        `agent_id`), so it could report an older
--                        attestation's author beside the newer one's
--                        verdict — a pairing no stored record ever
--                        held — and ordering by `evidence_at` alone
--                        decided an equal-`at` pair of verdicts
--                        arbitrarily, while `canon-gate::ledger::
--                        latest_verdicts`, the fold this mart's
--                        `green` claims to mirror, decides it by
--                        `(at, schema, digest)`.
CREATE OR REPLACE VIEW mart_trust_matrix AS
WITH tasks AS (
    SELECT
        body ->> '$.task_id' AS task_id,
        body ->> '$.title'   AS title,
        body ->> '$.status'  AS task_status
    FROM stg_records
    WHERE kind = 'task'
    QUALIFY row_number() OVER (PARTITION BY body ->> '$.task_id' ORDER BY version_rank DESC) = 1
),
evidence_counts AS (
    SELECT
        task_id,
        count(*)         AS evidence_count,
        max(evidence_at) AS latest_at
    FROM int_task_evidence
    GROUP BY task_id
),
evidence_current AS (
    SELECT
        task_id,
        verdict AS latest_verdict,
        who     AS latest_who
    FROM int_task_evidence
    QUALIFY row_number() OVER (PARTITION BY task_id ORDER BY version_rank DESC) = 1
),
subjects AS (
    SELECT task_id FROM tasks
    UNION
    SELECT task_id FROM evidence_counts
)
SELECT
    s.task_id,
    split_part(s.task_id, '#', 1)                  AS change_id,
    t.title,
    t.task_status,
    coalesce(ec.evidence_count, 0) > 0             AS covered,
    coalesce(ecur.latest_verdict, '') = 'faithful' AS green,
    ecur.latest_who                                AS who,
    coalesce(ec.evidence_count, 0)                 AS evidence_count,
    ec.latest_at
FROM subjects s
LEFT JOIN tasks t               USING (task_id)
LEFT JOIN evidence_counts ec    USING (task_id)
LEFT JOIN evidence_current ecur USING (task_id)
ORDER BY change_id, s.task_id;

-- s20 addition (task-scenario-join spec, design D3): unifies
-- `mart_trust_matrix`'s evidence-PRESENCE `covered` (keyed `task_id`)
-- against `porting.coverage`'s spec-AUTHORSHIP `covered` (keyed
-- `(project_id, scenario_id)`) over `int_task_scenario_refs`' declared
-- join table — answering "is this scope DONE (checkbox), VERIFIED
-- (evidence-covered), and SPEC-COVERED (scenario-authored)" in a single
-- query. A `Task` with no `scenario_refs` never appears here (nothing
-- declared to unify) but still appears in `mart_trust_matrix`
-- unchanged — this view is additive, never a replacement.
--
-- GRAIN: one row per declared `(task_id, scenario_id)` pair per
-- COVERING PROJECT. The task side of the join carries no project at
-- all — `Task.scenario_refs` is a bare list of `ScenarioId`s — while
-- the coverage side is keyed `(project_id, scenario_id)`, so when two
-- spec roots each author the same scenario id there are genuinely TWO
-- coverage answers for one declared pair and this view reports both,
-- each under its own `spec_project_id`. It cannot pick one: nothing in
-- the plan says which project the task meant. Emitting the pair ONCE
-- with an arbitrarily-chosen `spec_covered` (what a `scenario_id`-only
-- fold did) reports one project's coverage under another's name;
-- emitting it twice with no `spec_project_id` would be two
-- indistinguishable rows disagreeing about `spec_covered`, the same
-- contradictory-row defect folding `tasks` exists to remove.
--
-- `LEFT JOIN` on both sides so an absent evidence record or absent
-- `porting.coverage` overlay row surfaces as an honest `NULL`, never a
-- dropped row or an invented `false` (mirrors `mart_trust_matrix`'s own
-- `LEFT JOIN` posture for a task with no evidence). Read
-- `spec_project_id IS NULL` as "NO overlay row exists for this
-- scenario", which is a DIFFERENT state from an overlay row that
-- exists and says `covered = false` — the first is unauthored, the
-- second is authored-and-not-covered, and `canon report` renders both.
-- A NULL `spec_covered` alongside a non-NULL `spec_project_id` is a
-- third, narrower state: that project's overlay row exists but its own
-- `covered` field is absent or non-boolean.
--
-- `porting.coverage` is read generically by its `kind` string — this
-- view never depends on the `porting` plugin being installed; a repo
-- with no coverage overlay simply gets `spec_project_id`/`spec_covered`
-- NULL throughout. This is an interim, explicitly-named coupling to the
-- ONE `porting.coverage` overlay identity — the same "explicit STUB,
-- never silently load-bearing" posture `int_evidence_verdicts` already
-- establishes for its own S5-shaped stand-in; a repo using a DIFFERENT
-- overlay identity for spec-coverage gets no `spec_covered` signal from
-- this view until a follow-up generalizes the join to
-- `canon.yaml`-declared overlay identities (named non-goal, s20
-- design.md R3). Read-only reporting ONLY — never a `canon-gate` input;
-- `canon gate check` verdicts are byte-identical before and after this
-- view exists (s20 acceptance).
--
-- Every joined side is folded to one row per key (header's fold
-- inventory): `int_task_scenario_refs` and `mart_trust_matrix` fold
-- their own `task_id`, and `cov` below folds the overlay's OWN declared
-- `join_key` — `(project_id, scenario_id)` per
-- `.canon/plugins/porting/plugin.yaml`, the same pair
-- `GitTier::write_namespaced` builds the `{project_id}__{scenario_id}`
-- natural key from. The fold is still needed at that full key:
-- `write_namespaced` appends a new object whenever an overlay row's
-- content changes (its own "logically different body, same join key,
-- appends never overwrites"), so one project re-syncing its coverage is
-- two physical rows at one key. Unfolded AND unprojected, this view was
-- QUADRATIC in stored versions — a task flipped once by `canon gate
-- task` crossed with its own re-synced coverage overlay emitted four
-- rows for ONE declared pair, two of them reading a `task_status` the
-- plan no longer holds.
CREATE OR REPLACE VIEW mart_scope_status AS
SELECT
    r.task_id,
    r.scenario_id,
    tm.task_status,
    tm.covered      AS evidence_covered,
    tm.green,
    cov.project_id  AS spec_project_id,
    cov.covered     AS spec_covered
FROM int_task_scenario_refs r
LEFT JOIN mart_trust_matrix tm ON tm.task_id = r.task_id
LEFT JOIN (
    SELECT
        body ->> '$.project_id'         AS project_id,
        body ->> '$.scenario_id'        AS scenario_id,
        (body ->> '$.covered')::BOOLEAN AS covered
    FROM stg_records
    WHERE kind = 'porting.coverage'
    QUALIFY row_number() OVER (
        PARTITION BY body ->> '$.project_id', body ->> '$.scenario_id'
        ORDER BY version_rank DESC
    ) = 1
) cov ON cov.scenario_id = r.scenario_id
ORDER BY r.task_id, r.scenario_id, cov.project_id;

-- Panel 2: session costs grouped by `(session_id, client, role,
-- workspace_label)` (S3 ingest, the donor's `session_id` join key).
-- There is no `repo` column and this panel never claims one. Neither
-- `Session` nor `Run` carries a native `role` field yet — verified
-- against `crates/canon-ingest/src/normalize.rs`, 2026-07-11: every
-- actor `canon-ingest` constructs is `Actor::new_unattributed` (`role`
-- always `NULL`), surfaced honestly here as `'unattributed'` rather
-- than hidden.
--
-- `workspace_label` is NOT a repo identity, and specifically is not
-- "the closest available" one — an earlier version of this comment
-- claimed that and it was wrong twice over (s42 gate re-review). What
-- it actually is: `canon_ingest::normalize::workspace_label_from_key`'s
-- output, i.e. the LAST non-empty path segment of the normalized
-- workspace key, and nothing more. Two concrete failure modes follow
-- directly from that definition — it SPLITS one repo whose main and
-- linked `git worktree`s sit in differently-named directories
-- (`/w/canon` + `/w/canon-feat` -> two rows for one repo), and it
-- MERGES two genuinely different repos that share a directory name
-- (`~/a/canon` + `~/b/canon` -> one row).
--
-- Two STRONGER fields exist today and this mart reads neither, which is
-- the honest framing rather than calling this one the best available:
--   `Session.project_key` — the field with actual repo semantics.
--     `canon-cli`'s ingest pass stamps it to the MAIN worktree's
--     normalized key (`crates/canon-cli/src/ingest.rs`'s
--     `project_key_for`), and `canon_model::records::Session`'s own doc
--     states the purpose: "so queries can aggregate a repo's main
--     worktree and its linked `git worktree`s as one project". The
--     `sessions` CTE below does not select it.
--   the `token_usage` event's own `workspace_key` — the full
--     normalized path, at THIS panel's exact grain (it sits in the same
--     `detail` object `workspace_label` is read from), so it
--     distinguishes same-named repos under different parents.
--
-- Why the label is nonetheless what this panel groups by: `project_key`
-- is session-level, and this GROUP BY is deliberately event-level so a
-- session whose runs span two workspaces yields two distinct rows
-- rather than one row with an arbitrary workspace silently standing in
-- for the other (pinned by `session_costs_multi_workspace.rs`).
-- Switching the column would collapse that split, so the gap stays a
-- NAMED one under this file's existing convention — the column is
-- called `workspace_label`, never renamed to `repo`, and a caller
-- needing repo-level truth must read `project_key`, not this. Replace
-- this derivation wholesale (not patch around it) if the panel ever
-- grows a genuine repo grain.
--
-- Cost/tokens come from `canon_ingest::normalize::TOKEN_USAGE_LABEL`
-- (`"token_usage"`) events, keyed `run_id` -> `session_id` (design D5's
-- `Session`/`Run` keyed by `session_id`).
--
-- BLOCKER FIXED HERE (s42 re-review). All three CTEs are folded to one
-- row per natural key (header's fold list), because every one of them
-- feeds a `sum()` and a multi-version row on ANY of the three fans the
-- join out and multiplies BOTH `total_cost` and `total_tokens`.
-- `run_count` never had the bug — it is `count(DISTINCT tu.run_id)`,
-- which absorbs duplicates — which is exactly why the double-count was
-- silent: `run_count 1 / total_cost 0.02` for a single $0.01 run reads
-- like an expensive run, not like a bug.
--
-- Each CTE's exposure, all three real rather than theoretical:
--   `runs`     — `canon dispatch begin`/`end` store TWO `run` versions
--                per dispatched run (s42 task group 1), so ONE
--                completed, locally-readable dispatch doubled both
--                totals. Key `run_id`.
--   `sessions` — `canon ingest sessions` re-persists a `Session` as
--                the transcript grows (an `ended_at` appearing on a
--                later pass is a second version at the same
--                `session_id`). The outer `GROUP BY` does NOT save
--                this: the join fans out BEFORE aggregation, so
--                regrouping two identical `(session_id, client, role)`
--                tuples still sums each cost twice. Key `session_id`.
--   `token_usage` — an `Event` is keyed `(run_id, seq)`
--                (`partition.rs`), so a re-ingested line whose body
--                CHANGED (a corrected cost) is a second version of the
--                same event, and summing both charges it twice. A
--                byte-identical re-ingest cannot get here at all —
--                `GitTier::write` rejects the unchanged path as
--                `DuplicatePath` — so the fold's job is precisely to
--                take the CORRECTION and drop the superseded figure.
CREATE OR REPLACE VIEW mart_session_costs AS
WITH token_usage AS (
    SELECT
        body ->> '$.run_id'                                            AS run_id,
        "at",
        CAST(body -> '$.detail' ->> '$.cost' AS DOUBLE)                AS cost,
        body -> '$.detail' ->> '$.workspace_label'                     AS workspace_label,
        CAST(body -> '$.detail' -> '$.tokens' ->> '$.total' AS BIGINT) AS tokens_total
    FROM stg_records
    WHERE kind = 'event' AND (body ->> '$.label') = 'token_usage'
    QUALIFY row_number() OVER (
        PARTITION BY body ->> '$.run_id', body ->> '$.seq'
        ORDER BY version_rank DESC
    ) = 1
),
runs AS (
    SELECT
        body ->> '$.run_id'     AS run_id,
        body ->> '$.session_id' AS session_id
    FROM stg_records
    WHERE kind = 'run'
    QUALIFY row_number() OVER (PARTITION BY body ->> '$.run_id' ORDER BY version_rank DESC) = 1
),
sessions AS (
    SELECT
        body ->> '$.session_id'        AS session_id,
        body ->> '$.client'            AS client,
        body -> '$.actor' ->> '$.role' AS actor_role
    FROM stg_records
    WHERE kind = 'session'
    QUALIFY row_number() OVER (PARTITION BY body ->> '$.session_id' ORDER BY version_rank DESC) = 1
)
SELECT
    s.session_id,
    s.client,
    coalesce(s.actor_role, 'unattributed') AS role,
    tu.workspace_label                     AS workspace_label,
    count(DISTINCT tu.run_id)              AS run_count,
    round(sum(tu.cost), 6)                 AS total_cost,
    CAST(sum(tu.tokens_total) AS BIGINT)   AS total_tokens,
    min(tu."at")                           AS first_event_at,
    max(tu."at")                           AS last_event_at
FROM sessions s
JOIN runs r         ON r.session_id = s.session_id
JOIN token_usage tu ON tu.run_id = r.run_id
GROUP BY s.session_id, s.client, s.actor_role, tu.workspace_label
ORDER BY s.session_id, tu.workspace_label;

-- Panel 3: role memory (per-namespace strategy counts, plus the
-- not-demoted share design D5 named "hit rate, effect"), over
-- `stg_strategy_items` (S6's `StrategyItem` store, one row per
-- distilled strategy). `hit_rate` is NOT a retrieval hit rate and no
-- effect is measured anywhere: `hit_rate` = the fraction of a
-- role/`regime_key` namespace's strategies NOT yet demoted (S7's
-- `demotion` soft-flag, `crates/canon-learn/src/strategy.rs`) — the
-- nearest available "did this hold up" signal; there is no separate
-- per-strategy reward/effect metric recorded today, so
-- `avg_source_trajectories` (the average breadth of trajectory
-- evidence each strategy is founded on) stands in for "effect" as an
-- interim, explicitly-named proxy, same posture as `mart_trust_matrix`
-- above.
CREATE OR REPLACE VIEW mart_role_memory AS
SELECT
    role,
    regime_key,
    count(*)                                                                             AS strategy_count,
    count(*) FILTER (WHERE (body -> '$.demotion') IS NULL)                                AS active_count,
    count(*) FILTER (WHERE (body -> '$.demotion') IS NOT NULL)                            AS demoted_count,
    round(count(*) FILTER (WHERE (body -> '$.demotion') IS NULL)::DOUBLE / count(*), 4)   AS hit_rate,
    round(avg(json_array_length(body -> '$.source_trajectory_ids')), 2)                   AS avg_source_trajectories,
    max(recorded_at)                                                                      AS latest_recorded_at
FROM stg_strategy_items
GROUP BY role, regime_key
ORDER BY role, regime_key;

-- Panel 4: flywheel health funnel (verdicts -> distilled -> retrieved
-- -> applied), over `stg_trajectories` (S4's already-derived
-- `VerdictRow`s each raw trajectory carries, its S7 rolled-up
-- `outcome`, and -- since s42 -- its own `run_id` attribution),
-- `stg_strategy_items` (S6's distill step), and `stg_records`'
-- `run.injected_guidance` (S8's retrieval-injection snapshot,
-- `canon_model::records::Run::injected_guidance` -- the ONE
-- physically-persisted "a strategy was retrieved and recorded as this
-- run's input" signal, written by `canon dispatch begin`;
-- `canon-learn::retrieve` is itself a pure, unlogged function (that
-- crate's own module doc), and nothing records whether the dispatched
-- agent ever read the snapshot).
--
-- What each column counts, and in WHICH unit (s40
-- (`plan-vs-actual-diff`) tasks 3.1/3.2; s42 (`close-the-open-loops`)
-- task 3.3). The last three stages are all counted in STRATEGIES, so
-- the funnel reads as one narrowing set:
--
--   verdicts  — `VerdictRow`s across this role's raw trajectories: the
--               evidence the distiller consumes. The one stage NOT
--               counted in strategies, and the natural upper bound —
--               a trajectory carries 1..n verdicts and
--               `canon-learn::distill_trajectory` emits at most one
--               item per verdict. Strictly fewer when one trajectory
--               repeats a byte-identical verdict: the two distil to
--               the same content, hence (ids being content-derived)
--               to one strategy, not two indistinguishable copies of
--               it. So a `distilled` below `verdicts` is duplicate
--               evidence collapsing, never the distiller dropping a
--               verdict on the floor.
--   distilled — this role's `stg_strategy_items` rows.
--   retrieved — DISTINCT strategies of this role that appear in at
--               least one `Run.injected_guidance` AND still exist as
--               a distilled row. Both halves are load-bearing; see
--               the rebuild note below.
--   applied   — that same distinct set, restricted to strategies whose
--               recipient run has SOMETHING recorded about how it
--               ended, under exactly one of the two rules below. So
--               "applied" asserts BOTH halves: the strategy is named
--               in that run's recorded `injected_guidance`, AND
--               something is recorded about how that run ended.
--
-- Which rule admitted a count is NOT left to the reader to guess (s42
-- task 3.3). `applied` is broken out by rule, and the two parts
-- PARTITION it — `applied = applied_attributed + applied_proxy`
-- exactly, because `applied_strategies` below assigns each counted
-- `(role, strategy)` pair ONE rule with attribution winning, rather
-- than tallying two overlapping sets. BOTH rules are CO-OCCURRENCE
-- inside one run; neither is causation (see "what this panel cannot
-- say" below):
--
--   applied_attributed
--             — SAME-RUN, SAME-ROLE EVIDENCE. The strategy is named
--               in some run's `injected_guidance`, and that SAME run
--               has at least one trajectory of the SAME role stamped
--               with that run's own `run_id`
--               (`canon_learn::Trajectory::run_id`, stamped only by
--               an explicit `canon ingest artifacts --run`) whose
--               `outcome` is one of the resolved variants. This is
--               s40 task 3.1's ORIGINAL wording — "a resolved
--               trajectory joined to its own run" — which s40 left
--               open as unimplementable, because the trajectory
--               feeding this panel carried no run id at all.
--   applied_proxy
--             — the s40 PROXY, retained and now LABELLED. The
--               recipient run has no such trajectory, so the panel
--               falls back to that run's own terminal `Run.status`.
--               Weaker on purpose: all it records is that the run
--               reached a terminal state. A repo that never passes
--               `--run` reads `applied_attributed 0` and every count
--               under `applied_proxy`, which is the honest
--               description of what its corpus supports.
--
-- What this panel CANNOT say, stated here because three earlier
-- panels on this release line shipped exactly this defect (s39's
-- model-level ceiling, s40's funnel columns, s41's burn-down read as
-- current state) and the s42 re-review caught the fourth. The join
-- below is `(run_id, role)` and NOTHING else. So one resolved
-- trajectory of a role admits EVERY still-distilled strategy of that
-- role injected into that run, alike — the one an agent followed and
-- the one it never read are indistinguishable here, and both are
-- indistinguishable from a run that would have reached the same
-- outcome with no guidance at all. `applied_attributed` is therefore
-- STRONGER than the proxy (it requires a judged outcome out of that
-- run, not merely a terminal state) and WEAKER than attributing the
-- outcome TO the guidance, which canon cannot compute at all: no
-- record kind carries a strategy -> outcome edge. Making the causal
-- claim requires ADDING that edge — a `StrategyId` stamped on the
-- `Trajectory`/`VerdictRow` at judgment time, joined here in place of
-- `run_id` alone — never a re-reading of this relation.
--
-- Why attribution requires the trajectory's ROLE to match the cited
-- strategy's, and not merely its run: this panel's grain IS the role.
-- A run dispatched with `dev` guidance out of which only a resolved
-- `content` trajectory came is evidence about `content`, and counting
-- it for `dev` would report same-run evidence for a role that has
-- none. Such a run falls back to the proxy for `dev` — the
-- documented, labelled degrade — rather than borrowing another role's
-- outcome.
--
-- Why the trajectory must be RESOLVED, by an allowlist of
-- `canon_learn::VerdictOutcome`'s non-`pending` variants (serialized
-- kebab-case) rather than `<> 'pending'`: every trajectory is minted
-- `pending` and only `canon-learn::mark_trajectory_verdict` writes a
-- covering outcome, so `pending` is "no judgment yet", not a
-- judgment. An S6-era row predating the field carries no `outcome`
-- key at all and reads NULL, which the allowlist excludes; a future
-- variant must not silently start counting as applied.
--
-- `applied <= retrieved <= distilled` holds BY CONSTRUCTION, not by
-- luck: `applied_strategies` groups a `WHERE`-restriction of the exact
-- rows `retrieved_counts` counts, over the same `(role, strategy_id)`
-- grain, so its `count(*)` cannot exceed that relation's
-- `count(DISTINCT strategy_id)`; and every strategy either stage
-- counts IS one `stg_strategy_items` row of that role. Adding the
-- attribution rule cannot widen the funnel. It DOES admit strategies
-- the proxy alone would refuse — that is the point of it — but only
-- ones `retrieved_guidance` already holds: it introduces no new row and
-- no second relation, so the ceiling `retrieved` counts is untouched.
-- Both properties were false before s40 — `applied` was `count(*)` over
-- resolved trajectories with no reference to retrieval at all (canon's
-- own corpus rendered `retrieved 0 / applied 16`, unreadable as a
-- funnel), and `retrieved` was `count(*)` over injection EVENTS, which
-- exceeds `distilled` the moment one strategy is injected into two runs.
-- A stage that can exceed the stage above it measures nothing about the
-- stage above it.
--
-- Why `retrieved` does not evaporate on a rebuild. This stage joins a
-- recorded `StrategyRef` — a snapshot frozen into `Run.
-- injected_guidance` at dispatch time — against the CURRENT
-- `stg_strategy_items`. Every `canon ingest artifacts` calls
-- `canon-learn`'s `rebuild_namespace`, which deletes a regime's whole
-- distilled layer and re-distills it, so that join only holds if a
-- re-derived strategy keeps its id. It does: `StrategyId` is a pure
-- function of the distilled row's own content
-- (`crates/canon-learn/src/ids.rs`'s `StrategyId::derive`), not a
-- freshly-minted ULID, so re-distilling unchanged evidence reproduces
-- the same id and the citation still resolves. What the stage
-- therefore counts is precisely "retrieved, and still derivable from
-- today's evidence" — a strategy whose source trajectory's own text
-- later changes is re-derived under a NEW id, and the old citation
-- stops counting, which is the honest answer: that strategy no longer
-- exists.
--
-- Both rules depend on `stg_records` actually CONTAINING the run that
-- carried the guidance, which is a s42 change too: before it, `canon
-- dispatch begin` wrote a `Run` only to the private
-- `<repo>/.canon/dispatch/<run_id>.json` side-channel and nothing
-- reconciled it into a tier, so a freshly dispatched run — the only
-- kind that carries `injected_guidance` — was invisible here and this
-- panel read `retrieved 0` on canon's own corpus no matter how many
-- dispatches had happened. `canon dispatch begin`/`end` now persist
-- the run through the same `TierRegistry` as every other record (s42
-- task group 1), so a dispatch is joinable from this view. A
-- `retrieved 0` now means what it says: no run's recorded guidance
-- names a strategy that exists today.
CREATE OR REPLACE VIEW mart_flywheel_funnel AS
WITH verdict_counts AS (
    SELECT role, CAST(sum(json_array_length(body -> '$.verdicts')) AS BIGINT) AS n
    FROM stg_trajectories
    GROUP BY role
),
distilled_counts AS (
    SELECT role, count(*) AS n
    FROM stg_strategy_items
    GROUP BY role
),
-- One row per (run, strategy actually injected into that run). Both
-- retrieval stages below are counted over THIS single relation,
-- `applied_*` differing only by a `WHERE` — that shared grain is what
-- makes `applied <= retrieved` structural.
retrieved_guidance AS (
    SELECT
        r.body ->> '$.run_id' AS run_id,
        r.body ->> '$.status' AS run_status,
        si.id                 AS strategy_id,
        si.role               AS role
    FROM stg_records r, unnest(from_json(r.body -> '$.injected_guidance', '["JSON"]')) AS u(g)
    JOIN stg_strategy_items si ON si.id = (g ->> '$.strategy_id')
    WHERE r.kind = 'run'
),
-- s42 task 3.3, the ATTRIBUTION side: `(run, role)` pairs for which
-- some RESOLVED trajectory of that role names that run. `DISTINCT`
-- because a run may legitimately have derived several trajectories;
-- this relation answers a membership question, never a count.
attributed_runs AS (
    SELECT DISTINCT
        t.body ->> '$.run_id' AS run_id,
        t.role                AS role
    FROM stg_trajectories t
    WHERE (t.body ->> '$.run_id') IS NOT NULL
      AND (t.body ->> '$.outcome') IN ('success', 'failure', 'rolled-back')
),
applied_scored AS (
    SELECT
        rg.role,
        rg.strategy_id,
        -- Attribution FIRST, the proxy only as a fallback, `NULL` (not
        -- applied) when neither holds. The terminal
        -- `canon_model::records::RunStatus` variants (serialized
        -- `snake_case`) are an allowlist, never `<> 'running'`:
        -- `pending` is non-terminal too, and a future non-terminal
        -- variant must not silently start counting as applied.
        CASE
            WHEN ar.run_id IS NOT NULL THEN 'attributed'
            WHEN rg.run_status IN ('succeeded', 'failed', 'aborted') THEN 'proxy'
        END AS rule
    FROM retrieved_guidance rg
    LEFT JOIN attributed_runs ar ON ar.run_id = rg.run_id AND ar.role = rg.role
),
-- One row per counted `(role, strategy)`, carrying the ONE rule that
-- earned it. Attribution WINS over the proxy for a strategy injected
-- into several runs, so the two split columns PARTITION `applied`
-- instead of overlapping — a plain pair of filtered
-- `count(DISTINCT strategy_id)`s would double-count exactly that
-- strategy and make the two parts sum above the whole.
applied_strategies AS (
    SELECT
        role,
        strategy_id,
        CASE WHEN bool_or(rule = 'attributed') THEN 'attributed' ELSE 'proxy' END AS rule
    FROM applied_scored
    WHERE rule IS NOT NULL
    GROUP BY role, strategy_id
),
retrieved_counts AS (
    SELECT role, count(DISTINCT strategy_id) AS n
    FROM retrieved_guidance
    GROUP BY role
),
applied_counts AS (
    SELECT
        role,
        count(*)                                    AS n,
        count(*) FILTER (WHERE rule = 'attributed') AS n_attributed,
        count(*) FILTER (WHERE rule = 'proxy')      AS n_proxy
    FROM applied_strategies
    GROUP BY role
),
roles AS (
    SELECT role FROM verdict_counts
    UNION SELECT role FROM distilled_counts
    UNION SELECT role FROM retrieved_counts
    UNION SELECT role FROM applied_counts
)
SELECT
    r.role,
    coalesce(vc.n, 0)            AS verdicts,
    coalesce(dc.n, 0)            AS distilled,
    coalesce(rc.n, 0)            AS retrieved,
    coalesce(ac.n, 0)            AS applied,
    coalesce(ac.n_attributed, 0) AS applied_attributed,
    coalesce(ac.n_proxy, 0)      AS applied_proxy
FROM roles r
LEFT JOIN verdict_counts   vc USING (role)
LEFT JOIN distilled_counts dc USING (role)
LEFT JOIN retrieved_counts rc USING (role)
LEFT JOIN applied_counts   ac USING (role)
ORDER BY r.role;

-- Panel 5: review-feedback burn-down over time (S4's verdict stream:
-- `evidence_record`/`divergence` records). `divergence_open_running_
-- total` is the actual burn-down curve — a running (opened - resolved)
-- total by day, over `Divergence.status`
-- (`canon_model::records::DivergenceStatus`); the `evidence_*` columns
-- break the SAME window down by `EvidenceRecord.verdict` for the
-- companion evidence-side trend.
CREATE OR REPLACE VIEW mart_review_burndown AS
WITH by_day AS (
    SELECT
        date_trunc('day', "at") AS day,
        count(*) FILTER (WHERE kind = 'evidence_record' AND (body ->> '$.verdict') = 'faithful')       AS evidence_faithful,
        count(*) FILTER (WHERE kind = 'evidence_record' AND (body ->> '$.verdict') = 'divergent')      AS evidence_divergent,
        count(*) FILTER (WHERE kind = 'evidence_record' AND (body ->> '$.verdict') = 'not_applicable') AS evidence_not_applicable,
        count(*) FILTER (WHERE kind = 'divergence' AND (body ->> '$.status') = 'open')                 AS divergence_opened,
        count(*) FILTER (WHERE kind = 'divergence' AND (body ->> '$.status') = 'resolved')              AS divergence_resolved
    FROM stg_records
    WHERE kind IN ('evidence_record', 'divergence')
    GROUP BY 1
)
SELECT
    day,
    evidence_faithful,
    evidence_divergent,
    evidence_not_applicable,
    divergence_opened,
    divergence_resolved,
    CAST(
        sum(divergence_opened - divergence_resolved) OVER (ORDER BY day ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)
        AS BIGINT
    ) AS divergence_open_running_total
FROM by_day
ORDER BY day;

-- Panel 6 (S2 hardening addition, data-stores Pattern 2 — "Second-hop
-- join keys (session ↔ run, session ↔ handoff) must be minted, typed,
-- and reachable — they are NOT currently wired anywhere"):
-- a prior session-store audit's §Pattern 2
-- names this donor's OWN unclosed gap as a cautionary tale — a prior
-- session store's DuckDB ATTACH surface exposes exactly ONE prior
-- session/event store table
-- (`dash.public.sessions`) and never `handoffs`, so "one fan-out query
-- reaches Session+Run+Handoff together" was never actually buildable
-- there even though the donor's own schema HELD the columns
-- (`sessions.workflowRunId`, `handoffs.createdBySessionId`) needed to.
-- `mart_session_costs` above already closes the Session⋈Run half over
-- `token_usage` events; this view closes the full Session⋈Run⋈Handoff
-- triple so no caller has to separately query 3 surfaces the way a
-- prior-session-store-shaped consumer would have had to (Pattern 2's own
-- "adoption sketch").
--
-- Handoff join-key caveat: `canon_model::records::Handoff` (S1) carries
-- no dedicated `session_id` field of its own today — verified against
-- `crates/canon-model/src/handoff.rs`, 2026-07-11: its own fields are
-- `id`/`state`/`chain_id`/`parent_handoff_id`/`seq`/`claimed_by`/
-- `openspec_change_slug`/`tags`/`title`/`body`, plus s37's
-- `from_role`/`to_role` edge endpoints — the endpoints type WHICH ROLE
-- handed to which, still not WHICH SESSION, so none of these is a
-- session key. The one currently-available, honest join key is every record's
-- OWN envelope `actor.session_id` (S1's structured-actor design,
-- `canon_model::envelope::Actor::session_id` — the "no artifact can
-- join to the session… that produced it" gap this exact field exists
-- to close) — the agent-CLI session that AUTHORED this handoff. This
-- is a proxy for "the handoff belongs to this session," not a
-- first-class `Handoff.session_id` field; replace this view's
-- `h.session_id` derivation wholesale (not patch around it) if/when S1
-- adds one — the same "explicit STUB, never silently load-bearing"
-- posture `int_evidence_verdicts` already establishes. `LEFT JOIN`
-- against handoffs (unlike `mart_session_costs`'s `JOIN` against
-- token_usage) is deliberate: not every session has an associated
-- handoff, and a session/run pair must never silently disappear from
-- this view for lacking one.
--
-- All three CTEs are folded to one row per natural key (header's fold
-- list). This view carries no `sum()`, so the symptom was not a
-- multiplied measure but a CONTRADICTORY row set: a run stored twice
-- (`canon dispatch begin`/`end`, s42) crossed with a handoff stored
-- twice (`open` -> `accepted`) emitted FOUR rows for one
-- session/run/handoff triple, simultaneously reporting `run_status`
-- `running` AND `succeeded`, `handoff_state` `open` AND `accepted`.
-- Since a caller reads this view to learn a triple's CURRENT state,
-- every one of those rows but the latest is a false answer, and no
-- column told the caller which. `session_at`/`handoff_at` follow the
-- fold: they are that surviving VERSION's `at`, so `handoff_at` now
-- reads when the handoff last changed state — the same posture
-- `mart_session_costs`' bounds already take toward the `token_usage`
-- event's own `at`.
CREATE OR REPLACE VIEW mart_session_run_handoff AS
WITH runs AS (
    -- Fold FIRST, filter second: the `session_id IS NOT NULL` test
    -- belongs to the surviving version, so it sits outside the folding
    -- subquery. Inlined into the `WHERE` it would run BEFORE the window
    -- function and resurrect a superseded version of a run whose
    -- CURRENT version carries no `session_id`.
    SELECT run_id, session_id, run_status
    FROM (
        SELECT
            body ->> '$.run_id'     AS run_id,
            body ->> '$.session_id' AS session_id,
            body ->> '$.status'     AS run_status
        FROM stg_records
        WHERE kind = 'run'
        QUALIFY row_number() OVER (PARTITION BY body ->> '$.run_id' ORDER BY version_rank DESC) = 1
    )
    WHERE session_id IS NOT NULL
),
sessions AS (
    SELECT
        body ->> '$.session_id' AS session_id,
        body ->> '$.client'     AS client,
        "at"                    AS session_at
    FROM stg_records
    WHERE kind = 'session'
    QUALIFY row_number() OVER (PARTITION BY body ->> '$.session_id' ORDER BY version_rank DESC) = 1
),
handoffs AS (
    -- Fold FIRST, filter second, for the same reason `runs` does.
    SELECT handoff_id, session_id, handoff_state, handoff_title, handoff_at
    FROM (
        SELECT
            body ->> '$.id'                      AS handoff_id,
            body -> '$.actor' ->> '$.session_id' AS session_id,
            body ->> '$.state'                   AS handoff_state,
            body ->> '$.title'                   AS handoff_title,
            "at"                                 AS handoff_at
        FROM stg_records
        WHERE kind = 'handoff'
        QUALIFY row_number() OVER (PARTITION BY body ->> '$.id' ORDER BY version_rank DESC) = 1
    )
    WHERE session_id IS NOT NULL
)
SELECT
    s.session_id,
    s.client,
    r.run_id,
    r.run_status,
    h.handoff_id,
    h.handoff_state,
    h.handoff_title,
    s.session_at,
    h.handoff_at
FROM sessions s
JOIN runs r          ON r.session_id = s.session_id
LEFT JOIN handoffs h ON h.session_id = s.session_id
ORDER BY s.session_id, r.run_id, h.handoff_id;

-- s36 (subject-domain-loop) addition: the per-domain subject rollup
-- panel (canon report's subject panel). One row per `subject` record
-- (the reviewed 13th kind), grouped/ordered by `domain` then
-- `subject_id` — the per-domain management view `canon report` renders
-- and `canon query --kind subject [--domain] [--status]` filters.
-- `scenario_count` is how many `scenario_ids` the subject links;
-- `covered_scenarios` is how many of those carry a latest NON-Divergent
-- `evidence_record` verdict (faithful | not_applicable), the same
-- last-wins-by-`at` fold rule `mart_trust_matrix`'s `green` and
-- `canon-gate::ledger::latest_verdicts` (the `verifying -> shipped`
-- gate) use — read-only reporting only, never a `canon-gate` input. A
-- subject with no linked scenarios yields `scenario_count = 0`,
-- `covered_scenarios = 0` (a valid, minimal row, never dropped); an
-- empty/absent subject corpus yields zero rows, never an error.
--
-- `subjects` is folded to one row per `subject_id` (header's fold
-- list). A `Subject`'s whole point is a STATUS LIFECYCLE — `canon
-- subject status` walks proposed -> specced -> building -> verifying ->
-- shipped — so a subject that reached shipped is FIVE `subject`
-- versions at one key, and unfolded this panel emitted five rows for
-- it, each claiming a different current `status`, while
-- `subject_scenarios` fanned out five-fold and multiplied BOTH
-- `scenario_count` and `covered_scenarios` by five. A two-scenario
-- shipped subject read `scenario_count 10`. That is the same shape as
-- `mart_session_costs`' cost double-count, on the panel whose grain is
-- literally "one row per subject".
CREATE OR REPLACE VIEW mart_subjects AS
WITH subjects AS (
    SELECT
        body ->> '$.subject_id'  AS subject_id,
        body ->> '$.domain'      AS domain,
        body ->> '$.title'       AS title,
        body ->> '$.status'      AS status,
        body -> '$.scenario_ids' AS scenario_ids
    FROM stg_records
    WHERE kind = 'subject'
    QUALIFY row_number() OVER (PARTITION BY body ->> '$.subject_id' ORDER BY version_rank DESC) = 1
),
subject_scenarios AS (
    SELECT
        s.subject_id,
        g ->> '$' AS scenario_id
    FROM subjects s, UNNEST(from_json(s.scenario_ids, '["JSON"]')) AS u(g)
    WHERE s.scenario_ids IS NOT NULL
),
-- The CURRENT verdict about each scenario, as one WHOLE winner row
-- ordered by `version_rank` — the `(at, schema, digest)` triple
-- `canon-gate::ledger::latest_verdicts` (the `verifying -> shipped`
-- gate this panel's `covered_scenarios` cites) folds by. Ordering by
-- `at` alone, as this did, decided an equal-`at` pair of verdicts
-- arbitrarily while the gate it claims to mirror decided it by format
-- generation. NOT a natural-key fold and deliberately so: an
-- attestation carrying BOTH a `task_id` and a `scenario_id` partitions
-- under its `task_id` (`partition.rs`'s fallback order), so grouping by
-- `scenario_id` spans every attestation ABOUT that scenario — which is
-- the `(subject, role)` cell grain `latest_verdicts` folds, not the
-- storage key.
scenario_latest_verdict AS (
    SELECT
        body ->> '$.scenario_id' AS scenario_id,
        body ->> '$.verdict'     AS latest_verdict
    FROM stg_records
    WHERE kind = 'evidence_record' AND (body ->> '$.scenario_id') IS NOT NULL
    QUALIFY row_number() OVER (PARTITION BY body ->> '$.scenario_id' ORDER BY version_rank DESC) = 1
),
coverage AS (
    SELECT
        ss.subject_id,
        count(*) AS scenario_count,
        count(*) FILTER (
            WHERE slv.latest_verdict IS NOT NULL AND slv.latest_verdict <> 'divergent'
        ) AS covered_scenarios
    FROM subject_scenarios ss
    LEFT JOIN scenario_latest_verdict slv USING (scenario_id)
    GROUP BY ss.subject_id
)
SELECT
    s.domain,
    s.subject_id,
    s.title,
    s.status,
    coalesce(c.scenario_count, 0)    AS scenario_count,
    coalesce(c.covered_scenarios, 0) AS covered_scenarios
FROM subjects s
LEFT JOIN coverage c USING (subject_id)
ORDER BY s.domain, s.subject_id;

-- Panel 8 (s43 `findings-are-records`): one row per review round of a
-- change THAT FOUND SOMETHING — `(change_id, round)` — over the
-- fourteenth kind, `canon_model::records::Finding`. Eleven review
-- rounds shipped v0.4.0 and canon recorded none of them; the release
-- summary's "49 real issues, four of them defects in the previous
-- round's fix" was then typed from memory and both numbers were wrong.
-- This view exists so the numbers are DERIVED, and every column below
-- is exactly one aggregate over the folded finding stream — nothing
-- here reads git, prose, or a stored flag.
--
-- ── It cannot count the rounds RUN ───────────────────────────────────
-- s43 round 1, seq 2. The source is `WHERE kind = 'finding'` and
-- nothing else, so a round that found NOTHING wrote no record and has
-- no row. s42's round 12 returned MERGEABLE with zero findings and is
-- absent; four of v0.4.0's eleven rounds have rows, and the other
-- seven are missing because nobody backfilled them, not because they
-- were clean. Row count is therefore rounds-that-found-something, and
-- a consumer must not read it as review effort or review coverage.
-- canon has no record kind that marks a round as RUN — `Review` is a
-- per-scenario attestation, not a round — so the rounds-run count is
-- not derivable from this corpus by any query. It would take a
-- round-completion record, and canon has none today.
--
-- `finding_latest` is folded to one row per natural key (header's fold
-- list) BEFORE anything is counted, so a finding re-authored from
-- `open` to `fixed` is one finding in one disposition bucket, not two.
--
-- ── `fix_of_fix` is DERIVED, and it BOUNDS NOTHING ──────────────────
-- A finding is a fix-of-fix when its `introduced_by` equals the
-- `resolution_sha` of an EARLIER finding of the SAME change. No record
-- stores that relationship and none may: `canon_model::records::
-- Finding`'s own doc forbids a stored flag, and its
-- `fix_of_fix_is_derived_by_round_order_and_the_count_bounds_nothing`
-- test IS the reference derivation this `EXISTS` clause implements.
--
-- EARLIER is `(round, seq)` — the ordinal pair the natural key is
-- built from — compared STRICTLY, and scoped to one `change_id`. Each
-- of those three choices is load-bearing:
--
--   * NOT `reviewed_sha`. A review round usually reads an UNCOMMITTED
--     working tree, so `reviewed_sha` is `Option` and absent is the
--     COMMON case (v0.4.0's round 8 reviewed a worktree; its fixes
--     landed as `f438c610`, which is a different thing). A
--     sha-ordered derivation drops exactly those rounds — a third of
--     the corpus this view exists to report.
--   * NOT `at`. That is when the RECORD was written, not when the
--     round happened. A backfill authors eleven rounds in one sitting,
--     in whatever order the author walked them, so `at` carries no
--     round order at all and ordering by it would be an assertion the
--     data does not support.
--   * STRICTLY. `(g.round, g.seq) < (f.round, f.seq)` is irreflexive,
--     so a finding can never be matched against its own
--     `resolution_sha` — the degenerate self-join a non-strict
--     comparison would admit, and the one case that would make this
--     count structurally meaningless.
--
-- Scoped to one change because `round` restarts at 1 per change, so
-- `(round, seq)` orders nothing BETWEEN changes and canon holds no
-- other round order it could substitute (`at` is authoring time, per
-- above; commit ancestry would need git history this file cannot see).
-- A fix in one change that breaks something first found while
-- reviewing a DIFFERENT change is therefore NOT counted here. That is
-- a real, named miss, not an oversight, and the panel says so — the
-- alternative is a column called "fix-of-fix count" that reads as
-- global while computing something narrower, which is the exact defect
-- class this release line has shipped eight times.
--
-- `EXISTS`, never a `JOIN`: one finding may match several earlier
-- resolutions, and a join would fan out and count that finding once
-- per match. The semi-join counts FINDINGS, which is the unit the
-- panel claims.
--
-- ── `introduced_by_unsourced` is the UNKNOWN bucket ──────────────────
-- `introduced_by` is `None` when the introducing commit could not be
-- SOURCED, and canon never infers one. So an unsourced finding is not
-- evidence of "no earlier fix involved" — it is evidence of nothing,
-- and folding it into "not a fix-of-fix" would silently convert a
-- known unknown into a negative. It gets its own column instead.
-- `introduced_by_sourced + introduced_by_unsourced = findings` and
-- `fix_of_fix <= introduced_by_sourced`, both by construction, so a
-- reader can size the unknown against the count.
--
-- ── The count bounds NOTHING, in EITHER direction ────────────────────
-- s43 round 1, seq 1: this comment and the panel over it both called
-- `fix_of_fix` a FLOOR, and the corpus they were written against
-- disproves the direction. s43 round 2, findings 5 and 7: the
-- correction then drifted again — this comment upgraded the population
-- claim into a PER-ROW verdict, saying round 9's mixed-commit matches
-- were not attributable while rounds 10 and 11's pure-fix matches
-- were. The predicate below establishes neither. So the claim is now
-- ONE sentence, stated identically by every surface that reports this
-- column (`canon_report::render::FIX_OF_FIX_MEANING`, which is where
-- the cross-surface pin reads it from):
--
--   `fix_of_fix` bounds NOTHING — not from below, not from above: it
--   UNDER-counts, because an unsourced finding is never counted and a
--   fix in one change that breaks something first found while
--   reviewing a DIFFERENT change is not counted at all; it
--   OVER-counts, because a `resolution_sha` commit may carry work
--   BEYOND the fix and every finding recording that commit is counted
--   regardless; and for any individual match the data cannot say
--   whether the fix or the other work in that commit introduced the
--   defect.
--
-- Clause by clause, below: the UNDER-counts are the
-- `f.introduced_by IS NOT NULL` filter and the
-- `g.change_id = f.change_id` scope; the OVER-count is that the
-- `EXISTS` predicate is `g.resolution_sha = f.introduced_by` and
-- NOTHING else. v0.4.0 has both live. `f438c610` closed s42's round 8
-- AND shipped s42's whole feature, so round 9's six matches cannot be
-- attributed either way; rounds 10 and 11 match on
-- `49d3eb4f`/`b22fe8f5`, which held no work but the fix — a narrower
-- commit, and still not an attributable match, because the predicate
-- compared two RECORDED ids and whether `introduced_by` names the true
-- cause is the author's sourcing, which canon never checks. The count
-- is EXACT for what it joins and is a bound on nothing.
--
-- `reviewed_sha` is `max()` over the round's findings: it is per-
-- finding provenance, the natural key does not include it, and this
-- view neither requires a round's findings to agree on it nor claims
-- they do — `max` is a deterministic pick. NULL means NO finding in
-- the round recorded one. A round that reviewed an uncommitted working
-- tree is the usual reason and the common case, but that is the
-- direction it holds in: nothing here distinguishes it from a round
-- whose findings simply left the field unset, so NULL is not evidence
-- that the round reviewed a worktree.
CREATE OR REPLACE VIEW mart_review_rounds AS
WITH finding_latest AS (
    SELECT
        body ->> '$.change_id'                 AS change_id,
        try_cast(body ->> '$.round' AS BIGINT) AS "round",
        try_cast(body ->> '$.seq' AS BIGINT)   AS seq,
        body ->> '$.reviewed_sha'              AS reviewed_sha,
        body ->> '$.severity'                  AS severity,
        body ->> '$.disposition'               AS disposition,
        body ->> '$.resolution_sha'            AS resolution_sha,
        body ->> '$.introduced_by'             AS introduced_by
    FROM stg_records
    WHERE kind = 'finding'
    QUALIFY row_number() OVER (
        PARTITION BY body ->> '$.change_id', body ->> '$.round', body ->> '$.seq'
        ORDER BY version_rank DESC
    ) = 1
),
-- `try_cast` above, not `CAST`: a hand-authored record with a
-- non-integer `round`/`seq` must not abort every other panel in the
-- same `duckdb -init` run. It reads NULL, and NULL fails every
-- comparison below, so such a record can only ever be UNCOUNTED in
-- `fix_of_fix` — never counted on a guess.
fix_of_fix AS (
    SELECT f.change_id, f."round", count(*) AS n
    FROM finding_latest f
    WHERE f.introduced_by IS NOT NULL
      AND EXISTS (
          SELECT 1
          FROM finding_latest g
          WHERE g.change_id      = f.change_id
            AND g.resolution_sha = f.introduced_by
            AND (g."round" < f."round" OR (g."round" = f."round" AND g.seq < f.seq))
      )
    GROUP BY f.change_id, f."round"
),
rounds AS (
    SELECT
        change_id,
        "round",
        max(reviewed_sha)                                 AS reviewed_sha,
        count(*)                                          AS findings,
        count(*) FILTER (WHERE severity = 'blocker')      AS severity_blocker,
        count(*) FILTER (WHERE severity = 'should_fix')   AS severity_should_fix,
        count(*) FILTER (WHERE severity = 'note')         AS severity_note,
        count(*) FILTER (WHERE disposition = 'open')      AS disposition_open,
        count(*) FILTER (WHERE disposition = 'fixed')     AS disposition_fixed,
        count(*) FILTER (WHERE disposition = 'rejected')  AS disposition_rejected,
        count(*) FILTER (WHERE disposition = 'deferred')  AS disposition_deferred,
        count(*) FILTER (WHERE introduced_by IS NOT NULL) AS introduced_by_sourced,
        count(*) FILTER (WHERE introduced_by IS NULL)     AS introduced_by_unsourced
    FROM finding_latest
    GROUP BY change_id, "round"
)
SELECT
    r.change_id,
    r."round",
    r.reviewed_sha,
    r.findings,
    r.severity_blocker,
    r.severity_should_fix,
    r.severity_note,
    r.disposition_open,
    r.disposition_fixed,
    r.disposition_rejected,
    r.disposition_deferred,
    CAST(coalesce(ff.n, 0) AS BIGINT) AS fix_of_fix,
    r.introduced_by_sourced,
    r.introduced_by_unsourced
FROM rounds r
LEFT JOIN fix_of_fix ff USING (change_id, "round")
ORDER BY r.change_id, r."round";

-- ── mart_review_totals ──────────────────────────────────────────────
--
-- One row per `change_id`: the per-change total of everything
-- `mart_review_rounds` reports per round. It exists for exactly one
-- reason. s43 shipped `mart_review_rounds` so a release narrative's
-- issue count would be DERIVED rather than typed, and then left the
-- deriving half-done: the per-change sentence a release note contains
-- ("N findings over R rounds") still needed someone to add the
-- per-round table up by hand. Hand arithmetic over a generated table is the operation that
-- put four wrong numbers into this release line's published notes. A
-- number worth copying has to already be a number.
--
-- `FROM mart_review_rounds`, never a second pass over `stg_records`.
-- Every column here is a `sum()` (or a `count(*)`/`max()`) over the
-- rows the reader is looking at, so over any ONE read of the corpus
-- the total and the rows it totals are one computation with one
-- implementation: there is nothing for them to disagree WITH. A
-- parallel aggregate over `finding_latest` would have been the second
-- place to compute one number, which is how every defect on this line
-- recurred. That also means this view adds NO fold site: it inherits
-- `mart_review_rounds`'s `finding_latest` fold (the inventory in this
-- file's header) and folds nothing of its own.
--
-- "One read of the corpus" is the reader's half of that guarantee and
-- it does not come free: this file's four `read_text`/`read_parquet`
-- globs are re-expanded per statement, so a caller that queries the
-- two views in two statements over a LIVE ledger has queried two
-- corpora and the structural argument above buys it nothing.
-- `canon-report` closes that half by materializing the four staging
-- views once per run and repointing them at the materialized copies
-- (`crates/canon-report/src/query.rs`, `PIN_CORPUS_SQL`), before any
-- mart is computed or exported. A different consumer of this file
-- owes itself the same step.
--
-- ── `rounds_recorded` is rounds that FOUND something ─────────────────
-- `count(*)` over `mart_review_rounds`, which has one row per
-- `(change_id, round)` for the rounds that recorded a finding and no
-- row for the rounds that did not: a round that found nothing wrote
-- no `Finding`, so it is in neither table. This column therefore
-- counts the rounds that RECORDED a finding, never the rounds RUN,
-- and canon cannot supply the second number from any view — `Review`
-- is a per-scenario attestation, not a round, and no record kind
-- marks a review round as run. Naming this column `rounds` would have
-- been the whole defect in one word.
--
-- `highest_round` is `max("round")` over the same rows: the greatest
-- round NUMBER that recorded a finding. It witnesses nothing about
-- rounds. `highest_round` exceeding `rounds_recorded` says only that
-- some round number below it has no row here, and this view cannot
-- say why: `round` is author-supplied, nothing requires a change's
-- rounds to be numbered from 1 or without gaps, so a change whose one
-- finding is labelled round 7 shows the same gap six silent rounds
-- would. The two being equal says nothing either way. Neither the gap
-- nor its absence evidences a round that RAN — a round that found
-- nothing wrote no `Finding`, and no record kind marks a round as
-- run, so the corpus holds no signal of one at all. And it is a round
-- NUMBER, not a count: it equals the rounds RUN only if a change's
-- rounds are numbered from 1 without gaps AND its last round found
-- something, and this view requires neither. Neither column is the
-- rounds-run count.
--
-- ── The splits ───────────────────────────────────────────────────────
-- `sum()` of the per-round `count(*) FILTER` columns. `severity` and
-- `disposition` are closed enums on `Finding`, and the per-round
-- columns partition each round's findings, so summing them partitions
-- the change's:
--   severity_blocker + severity_should_fix + severity_note = findings
--   disposition_open + fixed + rejected + deferred        = findings
--   introduced_by_sourced + introduced_by_unsourced       = findings
-- The dispositions are the LATEST recorded state of each finding, not
-- a history of it — the fold underneath keeps one version per
-- `{change_id}__{round}__{seq}`, so `disposition_open` is what is open
-- NOW and never what was ever opened.
--
-- ── `fix_of_fix` ─────────────────────────────────────────────────────
-- `sum(fix_of_fix)`. The relationship is DERIVED and nowhere stored:
-- no `Finding` carries a fix-of-fix field, and the only place the edge
-- exists is `mart_review_rounds`'s `EXISTS` semi-join above. Summing
-- it per change changes the grain and nothing else, because that join
-- is ALREADY scoped to one `change_id` — the total counts exactly the
-- findings the rows above count, and the cross-change case is no more
-- visible here than it is there. What the count means is the one
-- sentence every surface reporting it states verbatim
-- (`canon_report::render::FIX_OF_FIX_MEANING`):
--
--   `fix_of_fix` bounds NOTHING — not from below, not from above: it
--   UNDER-counts, because an unsourced finding is never counted and a
--   fix in one change that breaks something first found while
--   reviewing a DIFFERENT change is not counted at all; it
--   OVER-counts, because a `resolution_sha` commit may carry work
--   BEYOND the fix and every finding recording that commit is counted
--   regardless; and for any individual match the data cannot say
--   whether the fix or the other work in that commit introduced the
--   defect.
--
-- ── No `reviewed_sha`, and no claim about the change ─────────────────
-- `reviewed_sha` is deliberately absent. `max()` over one round's
-- findings is a documented deterministic pick; `max()` over a whole
-- change's would be a hex string with no meaning at all, and a column
-- a reader could mistake for "the commit this change was reviewed at"
-- when no such commit is recorded.
--
-- Nothing this view emits is a claim about the change. `findings` is
-- how many findings reviewers RECORDED against it, which moves with
-- how many rounds it got and how freely its reviewers wrote findings
-- down; `severity_blocker` is the severity a reviewer TYPED, which
-- canon stores and never checks; `disposition_rejected` records that a
-- finding was rejected, not that it was wrong. There is no defect
-- rate, no quality score and no comparison between changes here — a
-- row is one change's review HISTORY, and the denominator that would
-- turn any of it into a rate is not in this corpus.
CREATE OR REPLACE VIEW mart_review_totals AS
SELECT
    change_id,
    CAST(count(*) AS BIGINT)                     AS rounds_recorded,
    CAST(max("round") AS BIGINT)                 AS highest_round,
    CAST(sum(findings) AS BIGINT)                AS findings,
    CAST(sum(severity_blocker) AS BIGINT)        AS severity_blocker,
    CAST(sum(severity_should_fix) AS BIGINT)     AS severity_should_fix,
    CAST(sum(severity_note) AS BIGINT)           AS severity_note,
    CAST(sum(disposition_open) AS BIGINT)        AS disposition_open,
    CAST(sum(disposition_fixed) AS BIGINT)       AS disposition_fixed,
    CAST(sum(disposition_rejected) AS BIGINT)    AS disposition_rejected,
    CAST(sum(disposition_deferred) AS BIGINT)    AS disposition_deferred,
    CAST(sum(fix_of_fix) AS BIGINT)              AS fix_of_fix,
    CAST(sum(introduced_by_sourced) AS BIGINT)   AS introduced_by_sourced,
    CAST(sum(introduced_by_unsourced) AS BIGINT) AS introduced_by_unsourced
FROM mart_review_rounds
GROUP BY change_id
ORDER BY change_id;