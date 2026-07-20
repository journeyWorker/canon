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
-- `<repo>/canon/learn`) — a physical source distinct from the git/r2
-- tiers above; see `stg_strategy_items`/`stg_trajectories` below for
-- why it needs its own root instead of reusing `CANON_R2_ROOT`.

INSTALL json;
LOAD json;

-- ── stg_* ────────────────────────────────────────────────────────────

-- Thin, content-trusted extraction over the git tier's Hive-laid-out
-- JSON files — `read_text` + JSON-payload column pulls, NEVER
-- `hive_partitioning=true` (design doc's Risk section: the donor's
-- ACTUAL mechanism, not the aspirational one). `kind`/`at`/`scenario_id`
-- come from the record's own body; `record_path` is kept for provenance
-- only, never trusted as the source of those columns.
CREATE OR REPLACE VIEW stg_git_records AS
WITH raw AS (
    SELECT
        filename                AS record_path,
        CAST(content AS JSON)   AS j
    FROM read_text(getenv('CANON_GIT_ROOT') || '/kind=*/**/*.json')
)
SELECT
    record_path,
    try_cast(j ->> '$.schema' AS BIGINT) AS schema,
    j ->> '$.kind'                       AS kind,
    try_cast(j ->> '$.at' AS TIMESTAMP)  AS "at",
    j ->> '$.scenario_id'                AS scenario_id,
    j                                    AS body
FROM raw;

-- Thin extraction over the r2 tier's parquet exports. Unlike the git
-- tier, `kind`/`natural_key`/`at`/`digest` are already real typed
-- parquet columns (materialized at write time by `canon-store`'s
-- `R2Tier`, not re-derived here) — `body` is still the JSON source of
-- truth for anything this view doesn't already surface as a column.
CREATE OR REPLACE VIEW stg_r2_records AS
SELECT
    kind,
    natural_key,
    CAST("at" AS TIMESTAMP) AS "at",
    digest,
    CAST(body AS JSON)      AS body
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
-- S3 unless a local `canon/r2` mirror happens to be current) is NOT
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
CREATE OR REPLACE VIEW stg_records AS
SELECT kind, "at", scenario_id, 'git' AS source_tier, body FROM stg_git_records
UNION ALL
SELECT kind, "at", body ->> '$.scenario_id' AS scenario_id, 'r2' AS source_tier, body FROM stg_r2_records;

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
CREATE OR REPLACE VIEW int_task_evidence AS
SELECT
    body ->> '$.task_id'               AS task_id,
    "at"                               AS evidence_at,
    body ->> '$.verdict'               AS verdict,
    body -> '$.actor' ->> '$.agent_id' AS who
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
CREATE OR REPLACE VIEW int_task_scenario_refs AS
SELECT
    body ->> '$.task_id' AS task_id,
    g ->> '$'            AS scenario_id
FROM stg_records, UNNEST(from_json(body -> '$.scenario_refs', '["JSON"]')) AS u(g)
WHERE kind = 'task' AND (body -> '$.scenario_refs') IS NOT NULL;

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
CREATE OR REPLACE VIEW mart_trust_matrix AS
WITH tasks AS (
    SELECT
        body ->> '$.task_id' AS task_id,
        body ->> '$.title'   AS title,
        body ->> '$.status'  AS task_status
    FROM stg_records
    WHERE kind = 'task'
),
evidence_latest AS (
    SELECT
        task_id,
        count(*)                      AS evidence_count,
        arg_max(verdict, evidence_at) AS latest_verdict,
        arg_max(who, evidence_at)     AS latest_who,
        max(evidence_at)              AS latest_at
    FROM int_task_evidence
    GROUP BY task_id
),
subjects AS (
    SELECT task_id FROM tasks
    UNION
    SELECT task_id FROM evidence_latest
)
SELECT
    s.task_id,
    split_part(s.task_id, '#', 1)                AS change_id,
    t.title,
    t.task_status,
    coalesce(el.evidence_count, 0) > 0           AS covered,
    coalesce(el.latest_verdict, '') = 'faithful' AS green,
    el.latest_who                                AS who,
    coalesce(el.evidence_count, 0)               AS evidence_count,
    el.latest_at
FROM subjects s
LEFT JOIN tasks t            USING (task_id)
LEFT JOIN evidence_latest el USING (task_id)
ORDER BY change_id, s.task_id;

-- s20 addition (task-scenario-join spec, design D3): unifies
-- `mart_trust_matrix`'s evidence-PRESENCE `covered` (keyed `task_id`)
-- against `porting.coverage`'s spec-AUTHORSHIP `covered` (keyed
-- `scenario_id`) over `int_task_scenario_refs`' declared join table —
-- one row per declared `(task_id, scenario_id)` pair, answering
-- "is this scope DONE (checkbox), VERIFIED (evidence-covered), and
-- SPEC-COVERED (scenario-covered)" in a single query. A `Task` with no
-- `scenario_refs` never appears here (nothing declared to unify) but
-- still appears in `mart_trust_matrix` unchanged — this view is
-- additive, never a replacement. `LEFT JOIN` on both sides so an
-- absent evidence record or absent `porting.coverage` overlay row
-- surfaces as an honest `NULL`, never a dropped row or an invented
-- `false` (mirrors `mart_trust_matrix`'s own `LEFT JOIN` posture for a
-- task with no evidence). `porting.coverage` is read generically by its
-- `kind` string — this view never depends on the `porting` plugin being
-- installed; a repo with no coverage overlay simply gets
-- `spec_covered = NULL` throughout. This is an interim, explicitly-
-- named coupling to the ONE `porting.coverage` overlay identity — the
-- same "explicit STUB, never silently load-bearing" posture
-- `int_evidence_verdicts` already establishes for its own S5-shaped
-- stand-in; a repo using a DIFFERENT overlay identity for spec-coverage
-- gets no `spec_covered` signal from this view until a follow-up
-- generalizes the join to `canon.yaml`-declared overlay identities
-- (named non-goal, s20 design.md R3). Read-only reporting ONLY — never a
-- `canon-gate` input; `canon gate check` verdicts are byte-identical
-- before and after this view exists (s20 acceptance).
CREATE OR REPLACE VIEW mart_scope_status AS
SELECT
    r.task_id,
    r.scenario_id,
    tm.task_status,
    tm.covered   AS evidence_covered,
    tm.green,
    cov.covered  AS spec_covered
FROM int_task_scenario_refs r
LEFT JOIN mart_trust_matrix tm ON tm.task_id = r.task_id
LEFT JOIN (
    SELECT body ->> '$.scenario_id' AS scenario_id,
           (body ->> '$.covered')::BOOLEAN AS covered
    FROM stg_records
    WHERE kind = 'porting.coverage'
) cov ON cov.scenario_id = r.scenario_id
ORDER BY r.task_id, r.scenario_id;

-- Panel 2: session costs by role/repo/session (S3 ingest, the donor's
-- `session_id` join key). Neither `Session` nor `Run` carries a native
-- `role`/`repo` field yet — verified against `crates/canon-ingest/src/
-- normalize.rs`, 2026-07-11: every actor `canon-ingest` constructs is
-- `Actor::new_unattributed` (`role` always `NULL`), and no record kind
-- has a `repo` field at all today. This mart groups by the nearest
-- AVAILABLE proxies instead of silently asserting fields that do not
-- exist: `role` reads the session actor's own `actor.role` (currently
-- always `NULL`, surfaced honestly as `'unattributed'` rather than
-- hidden); `workspace_label` (the `token_usage` event's own field,
-- S3's closest analog to a repo identifier) stands in for `repo` under
-- its own honest column name, not renamed to `repo` — and, being the
-- `repo` proxy, is part of the GROUP BY, not merely an `any_value()`
-- pick: a session whose runs span two workspaces must yield two
-- distinct rows, never one row with an arbitrary workspace label
-- silently standing in for the other. Cost/tokens come from
-- `canon_ingest::normalize::TOKEN_USAGE_LABEL` (`"token_usage"`)
-- events, keyed `run_id` -> `session_id` (design D5's `Session`/`Run`
-- keyed by `session_id`).
CREATE OR REPLACE VIEW mart_session_costs AS
WITH token_usage AS (
    SELECT
        body ->> '$.run_id'                                             AS run_id,
        "at",
        CAST(body -> '$.detail' ->> '$.cost' AS DOUBLE)                 AS cost,
        body -> '$.detail' ->> '$.workspace_label'                      AS workspace_label,
        CAST(body -> '$.detail' -> '$.tokens' ->> '$.total' AS BIGINT)  AS tokens_total
    FROM stg_records
    WHERE kind = 'event' AND (body ->> '$.label') = 'token_usage'
),
runs AS (
    SELECT body ->> '$.run_id' AS run_id, body ->> '$.session_id' AS session_id
    FROM stg_records
    WHERE kind = 'run'
),
sessions AS (
    SELECT
        body ->> '$.session_id'        AS session_id,
        body ->> '$.client'            AS client,
        body -> '$.actor' ->> '$.role' AS actor_role
    FROM stg_records
    WHERE kind = 'session'
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

-- Panel 3: role memory (strategies, hit rate, effect per role
-- namespace), over `stg_strategy_items` (S6's `StrategyItem` store,
-- one row per distilled strategy). `hit_rate` = the fraction of a
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
-- `VerdictRow`s each raw trajectory carries), `stg_strategy_items`
-- (S6's distill step), and `stg_records`' `run.injected_guidance`
-- (S8's retrieval-injection snapshot, `canon_model::records::Run::
-- injected_guidance` — the ONE physically-persisted "a strategy was
-- retrieved and shown to an agent" signal; `canon-learn::retrieve` is
-- itself a pure, unlogged function, that crate's own module doc).
-- `applied` = the count of trajectories whose S7 roll-up verdict was
-- actually written back (`outcome` present and not `pending`,
-- `crates/canon-learn/src/mark_verdict.rs`) — i.e. the guidance-
-- informed run's real-world consequence was recorded, the nearest
-- available "the retrieved guidance's effect was judged" signal (no
-- direct run_id<->trajectory_id join exists yet to require the SAME
-- run that received guidance also produced the trajectory).
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
retrieved_counts AS (
    SELECT si.role AS role, count(*) AS n
    FROM stg_records r, unnest(from_json(r.body -> '$.injected_guidance', '["JSON"]')) AS u(g)
    JOIN stg_strategy_items si ON si.id = (g ->> '$.strategy_id')
    WHERE r.kind = 'run'
    GROUP BY si.role
),
applied_counts AS (
    SELECT role, count(*) AS n
    FROM stg_trajectories
    WHERE (body ->> '$.outcome') IS NOT NULL AND (body ->> '$.outcome') <> 'pending'
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
    coalesce(vc.n, 0) AS verdicts,
    coalesce(dc.n, 0) AS distilled,
    coalesce(rc.n, 0) AS retrieved,
    coalesce(ac.n, 0) AS applied
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
-- `openspec_change_slug`/`tags`/`title`/`body`, none of them a session
-- key. The one currently-available, honest join key is every record's
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
CREATE OR REPLACE VIEW mart_session_run_handoff AS
WITH runs AS (
    SELECT
        body ->> '$.run_id'     AS run_id,
        body ->> '$.session_id' AS session_id,
        body ->> '$.status'     AS run_status
    FROM stg_records
    WHERE kind = 'run' AND (body ->> '$.session_id') IS NOT NULL
),
sessions AS (
    SELECT
        body ->> '$.session_id' AS session_id,
        body ->> '$.client'     AS client,
        "at"                    AS session_at
    FROM stg_records
    WHERE kind = 'session'
),
handoffs AS (
    SELECT
        body ->> '$.id'                      AS handoff_id,
        body -> '$.actor' ->> '$.session_id' AS session_id,
        body ->> '$.state'                   AS handoff_state,
        body ->> '$.title'                   AS handoff_title,
        "at"                                 AS handoff_at
    FROM stg_records
    WHERE kind = 'handoff' AND (body -> '$.actor' ->> '$.session_id') IS NOT NULL
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
),
subject_scenarios AS (
    SELECT
        s.subject_id,
        g ->> '$' AS scenario_id
    FROM subjects s, UNNEST(from_json(s.scenario_ids, '["JSON"]')) AS u(g)
    WHERE s.scenario_ids IS NOT NULL
),
scenario_latest_verdict AS (
    SELECT
        scenario_id,
        arg_max(verdict, evidence_at) AS latest_verdict
    FROM (
        SELECT
            body ->> '$.scenario_id' AS scenario_id,
            "at"                     AS evidence_at,
            body ->> '$.verdict'     AS verdict
        FROM stg_records
        WHERE kind = 'evidence_record' AND (body ->> '$.scenario_id') IS NOT NULL
    )
    GROUP BY scenario_id
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
