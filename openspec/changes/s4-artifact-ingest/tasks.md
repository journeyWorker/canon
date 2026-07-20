## 0. FOUNDATION (this wave — `ArtifactAdapter` trait, verdict-mapping table, `regime_key`)

Landed by `S4Foundation` per operator rescope directive (2026-07-11): the S4
design as originally authored had the ledger/divergence adapters reading a
hardcoded donor consumer-repo `spec/**` path and the handoff adapter reading the donor
monorepo's live hosted Postgres `handoffs` table — both violate canon's don't-read-live-consumer-
state posture. This FOUNDATION wave freezes the contract wave-2's four
concrete adapters (sections 1-4 below) build against, GENERIC and
`canon.yaml`-configured from the start. See design.md decision D6.

- [x] 0.1 Define the `ArtifactAdapter` trait (distinct from `SessionAdapter`,
      a verdict-deriving adapter: reads a configured source path/handle ->
      normalized canon-model-bound `Event`s keyed by the S1 join spine) +
      the generic `ArtifactSourceConfig` (`canon.yaml` paths:
      `ledger_root`/`divergences_root`/`openspec_root`, all `Option`, no
      donor consumer-repo / event-store hardcoding) + `ArtifactSourceHandle` (path-based OR
      already-fetched-records, so the handoff adapter never needs a
      `canon-store` dependency inside `canon-ingest`). Evidence:
      `crates/canon-ingest/src/artifact_adapter.rs` (`ArtifactAdapter`,
      `ArtifactSourceConfig`, `ArtifactSourceHandle`, `ArtifactEvent`,
      `ArtifactEventKind`, `ArtifactJoinKey`, `ArtifactParseOutcome`) +
      `artifact_adapter::tests` (config defaults to unconfigured, JSON
      round-trip, dyn-compatibility via a fixture adapter), all green under
      `cargo test -p canon-ingest`.
- [x] 0.2 Implement the review→verdict mapping table (specs/
      review-verdict-mapping) as a pure function from a normalized
      `ArtifactEventKind` to an optional verdict `{role, polarity,
      becomes}` — no verdict when the event doesn't match a mapped row
      (subsumes task 5.1 below). Evidence: `crates/canon-ingest/src/
      verdict.rs` (`derive_verdict`, `Polarity`, `Becomes`, `VerdictRow`) +
      `verdict::tests` (all 7 mapped rows + the non-verdict case + the
      review-promotion-with-no-authoring-role degrade-to-no-verdict case),
      all green under `cargo test -p canon-ingest`.
- [x] 0.3 Implement the shared `regime_key(role, repo, area, hash) ->
      String` function (`<role>/<repo>/<area>/<hash>`, S1 join-spine
      grammar; role leads the tuple) in `canon-model` as the SINGLE
      canonical serialization S4/S6/S7/S8 all reuse (subsumes the
      `regime_key` half of task 5.2 below). Evidence:
      `crates/canon-model/src/ids.rs` (`regime_key`,
      `canonicalize_regime_segment`) + `ids::tests::regime_key_*`
      (canonical format, role-leads ordering, write==read identity across
      casing/whitespace, role-scoped prefix sharing), all green under
      `cargo test -p canon-model`.
- [x] 0.4 Register the `ArtifactAdapter` registry seam (mirrors
      `SessionAdapter`'s registry, S3 design D1) so wave-2's four adapters
      plug in without touching the trait. Evidence:
      `crates/canon-ingest/src/artifact_registry.rs` (`registry`, `find`,
      `resolve_and_parse`) — ships EMPTY in this wave
      (`registry_ships_empty` test), wave-2 appends entries.

## 1. Ledger adapter (S4 wave-2)

Source root resolved from `ArtifactSourceConfig.ledger_root`
(`canon.yaml`-configured, GENERIC) — the donor consumer repo's `spec/ledger/` is the
reference source and frozen-fixture origin, never a hardcoded path (see
FOUNDATION note above and design.md D6).

- [x] 1.1 Implement the ledger scan walking `kind=<kind>/
      [area=<area>/]*.json` under the configured root, honoring the
      Hive-partitioned (`review`/`design-review`/`code-review`/`clear`) vs.
      flat (`run`/`drill`) layout split as the source repo has it today.
      Evidence: `crates/canon-ingest/src/artifact_adapters/ledger.rs`
      (`LedgerAdapter`, `parse_ledger_file`, `parse_partitioned_record`,
      `parse_flat_record` — record's own `kind` dispatched against the
      directory's `kind=<kind>` segment; `area` recomputed from
      `scenario_id`, never trusted from the `area=` directory,
      ledger-reader.md §3.3), registered in `artifact_registry::registry()`.
- [x] 1.2 Normalize each ledger record kind into an `ArtifactEvent` keyed
      by `scenario_id`, reading fields by name via `Option<T>` (never a
      fixed-arity struct) so a field absent from today's schema (e.g. a
      `run` record's missing `actor`) degrades to `None` instead of a parse
      failure. Evidence: `ledger.rs::LedgerRecord` (every field
      `Option<T>`), `ledger.rs::parse_flat_record` (one `ArtifactEvent`
      per `scenario_ids` entry, `run`/`drill` always `NonVerdict`),
      `parse_partitioned_record` (`review`->`ReviewPromotion`,
      `clear`->`ClearAfterFlagged`, `design-review`/`code-review`->
      `*Finding` unless `verdict=="faithful"`) + `ledger::tests` (golden
      verdict stream over `tests/fixtures/ledger/`: review-promotion with
      no authoring role -> no verdict, non-faithful code-review ->
      `dev`/failure/guardrail-candidate, run -> `NonVerdict` per
      `scenario_ids` entry, idempotent re-parse), all green under
      `cargo test -p canon-ingest`.
- [x] 1.3 Skip a malformed ledger record (unparseable JSON, missing
      `scenario_id`/`kind`) as a violation, never a crash — continue with
      the remaining records. Evidence: `ledger.rs::parse_ledger_file`
      returns `None` (counted as one skip by the caller) on unparseable
      JSON, an unrecognized `kind`, a layout mismatch (directory vs.
      record `kind`, `area`, or basename), or a missing/unparseable
      `scenario_id`/`scenario_ids` — `ledger::tests::
      malformed_record_is_skipped_not_a_crash` +
      `fixture_corpus_yields_expected_events_and_skips_the_malformed_record`
      (one corrupt-JSON record in the fixture corpus, skipped and
      counted, the rest of the corpus still parsed).

## 2. Divergence adapter (S4 wave-2)

Source root resolved from `ArtifactSourceConfig.divergences_root`
(`canon.yaml`-configured, GENERIC) — see FOUNDATION note above.

- [x] 2.1 Implement the divergence `.jsonl` line reader distinguishing
      `"type":"manifest"` (round bookkeeping) from `"type":"review"` /
      `"type":"remediation"` lines within one file. Evidence:
      `crates/canon-ingest/src/artifact_adapters/divergence.rs`
      (`DivergenceAdapter`, `parse_divergence_file`,
      `parse_divergence_line`, per-line `"type"` dispatch), registered
      in `artifact_registry::registry()`.
- [x] 2.2 Normalize each review/remediation line into an `ArtifactEvent`
      keyed by `scenario_id`, preserving `status`, `port_ref`, `upstream_ref`,
      and the `aspects` array verbatim. Evidence:
      `divergence.rs::parse_divergence_line` (raw JSON line passed
      through verbatim as `ArtifactEvent.detail`; `area` folded in from
      the `area=` Hive path segment) + `divergence::tests` (golden
      verdict stream over `tests/fixtures/divergences/lane=code/
      area=world/surface=world-firstbuy-hotdeal/*.jsonl`: manifest ->
      no verdict, open review -> `dev`/failure/guardrail-candidate,
      resolved remediation -> `dev`/success/strategy-candidate,
      corrupt line skipped+counted, idempotent re-parse), all green
      under `cargo test -p canon-ingest`.

## 3. Handoff adapter (S4 wave-2)

Reads **canon's own** `handoffs` table (S1's `Handoff` type,
wire-compatible with the prior event store's schema) via `canon-store`'s Postgres tier
(`Tier::read`) — NEVER a live donor event-store / hosted Postgres connection (operator rescope
directive; see FOUNDATION note above and design.md D3/D6). A FUTURE
driver — DEFERRED past this wave (the `canon ingest` CLI-ingest wiring,
not yet built anywhere in this workspace) — resolves the `Tier::read`
query and hands the rows to the adapter as `ArtifactSourceHandle::Records`
— `canon-ingest` itself gains no `canon-store` dependency.

- [x] 3.1 Implement the handoff adapter reading canon's own `handoffs`
      table (via `canon-store`'s Postgres tier), emitting one
      `ArtifactEvent` per observed state transition (created / claimed /
      done / abandoned). Evidence:
      `crates/canon-ingest/src/artifact_adapters/handoff.rs`
      (`HandoffAdapter`, `events_for` — handle-based, `resolve_source`
      always `None` per design D6/task 0.1, `parse` reads
      `ArtifactSourceHandle::Records` produced by a `canon-store::Tier::read`
      call OUTSIDE this crate), registered in `artifact_registry::registry()`.
      Fixture: `tests/fixtures/handoffs/*.json` (four frozen, checked-in
      `Handoff` records — pending-never-claimed / in-progress-claimed /
      done / abandoned), round-tripped through a real `canon_store::GitTier`
      rooted at a `tempfile::tempdir()` in `tests/handoff_fixture.rs`
      (never a live hosted-Postgres connection — design §8 risk
      mitigation) — `canon-store` is a `[dev-dependencies]`-only addition
      to `crates/canon-ingest/Cargo.toml` (task 0.1's "canon-ingest never
      needs a canon-store dependency" holds for `cargo build`; only
      `cargo test` links it). All green under `cargo test -p canon-ingest`.
      **Honesty note (2026-07-11, `S4Fix`/`ReviewS4Full` P1):** this task
      shipped the handle-based adapter code + its fixture round-trip only
      — the production `canon-store::Tier::read` driver that would supply
      `ArtifactSourceHandle::Records` from canon's own live `handoffs`
      table was a DEFERRED residual (the future `canon ingest` CLI-ingest
      wiring) at the time, not built anywhere in this workspace yet.
      `artifact_registry::resolve_and_parse`'s config-driven scan path
      explicitly reports `handoff` as `Records`-source-only
      (`ArtifactDispatchOutcome::UnsupportedSource`) instead of silently
      folding it into an empty parse outcome.
      **Update (2026-07-11, `s14-artifact-ingest-cli`):** that deferred
      driver has now shipped — `crates/canon-cli/src/artifact_ingest.rs`
      reads canon's own `Handoff` records via
      `canon_store::registry::TierRegistry::query` and feeds them to this
      adapter's `parse` as `ArtifactSourceHandle::Records`; see that
      change's `tasks.md` group 3 for evidence. `canon-ingest` itself
      still has no `canon-store` dependency (`cargo tree -p canon-ingest
      -e no-dev` unchanged) — the driver lives entirely in `canon-cli`.
- [x] 3.2 Carry the table's own `id` column as `handoff_id` verbatim (no
      re-derived identity); carry `openspecChangeSlug` as `change_id` when
      present. Evidence: `handoff.rs::events_for`
      (`ArtifactJoinKey::Handoff(record.id.clone())`, never re-parsed/
      re-derived) + `handoff.rs::detail` (`openspec_change_slug` copied
      into `detail.change_id` only when `Some`) +
      `handoff::tests::handoff_id_is_carried_verbatim_on_every_event` /
      `openspec_change_slug_is_carried_into_detail_change_id_when_present`
      + `handoff_fixture.rs`'s corpus-level assertions of both.

## 4. Openspec task-state adapter (S4 wave-2)

Source root resolved from `ArtifactSourceConfig.openspec_root`
(`canon.yaml`-configured, GENERIC) — ordinarily the consumer repo's own
root; never a hardcoded path.

- [x] 4.1 Implement the adapter reading `tasks.md` checkbox rows, detecting
      a `- [ ]` → `- [x] … — ✅ <evidence>` flip and a `**DEFERRED**` /
      `**DROPPED**` rewrite (the donor CLI's `flipTaskDone`/`flipTaskDefer`/
      `flipTaskDrop` shape).
      — ✅ `crates/canon-ingest/src/artifact_adapters/openspec_task.rs`
      (`OpenspecTaskAdapter`, `parse_row`/`classify_evidence`), registered in
      `artifact_registry.rs`; 20+ in-file tests.
- [x] 4.2 Normalize a flip into an `ArtifactEvent` keyed by `task_id`
      (`<change_id>#<n>`); parse the evidence string for a mergeable
      PR/CI reference where present.
      — ✅ `openspec_task.rs` (`ArtifactJoinKey::Task` keying + `classify_evidence`
      PR-merge/CI-fail branches), fixture `tests/fixtures/openspec_task/**`.

## 5. Verdict derivation

- [x] 5.1 Implement the review→verdict mapping table (specs/
      review-verdict-mapping) as a pure function from a normalized `Event`
      to an optional verdict `{role, polarity, becomes}` — no verdict when
      the event doesn't match a mapped row (e.g. a prose-only task flip, a
      deferred/dropped task, a handoff transition alone). Duplicate of
      FOUNDATION task 0.2 — same evidence: `crates/canon-ingest/src/
      verdict.rs::derive_verdict` + `verdict::tests`.
- [x] 5.2 Implement the shared `regime_key(role, repo, area, hash) ->
      String` function (`<role>/<repo>/<area>/<hash>`, S1) — FOUNDATION,
      same evidence as task 0.3: `crates/canon-model/src/
      ids.rs::regime_key`. **(S4 wave-2)** calling this function from each
      of the four concrete adapters' verdict emission is deferred to
      wave-2 (`crate::verdict::attach_regime_key` is the FOUNDATION-shipped
      call site wave-2 adapters use — no per-adapter derivation copy — but
      no adapter exists yet to call it).
- [x] 5.3 Attach the source record's trust-level tag (`@reviewed`/
      `@ratified` where applicable) to the verdict as a passthrough field,
      not collapsed into a single "success" bucket.
      — ✅ `crates/canon-ingest/src/artifact_adapters/ledger.rs` populates
      `ArtifactEvent.trust_level` from the ledger record's own field; the
      other three adapters emit `None` (their source records carry no
      trust-level concept).

## 6. Idempotence (S4 wave-2)

- [x] 6.1 Reuse S3's content-digest write-identity mechanism
      (`canon_ingest::normalize::content_digest`) for every normalized
      `ArtifactEvent` and verdict record across all four adapters.
      — ✅ `crates/canon-cli/src/artifact_ingest.rs` reuses `content_digest`
      for the regime_key `<hash>` (`regime_hash`) AND as the trajectory
      write-identity (`trajectory_content_digest` + `query_by_regime_key`
      existence check) — a re-ingest re-derives the identical id.
- [x] 6.2 Write the re-ingest test: run the artifact-ingest adapters twice
      over an unchanged fixture corpus and assert zero new/duplicate events
      or verdicts on the second run.
      — ✅ `crates/canon-cli/tests/artifact_ingest.rs::a_second_ingest_over_an_unchanged_corpus_persists_zero_new_trajectories`
      (second pass: zero new trajectories, `trajectories_skipped_duplicate == 1`).

## 7. Fixtures, golden file, and selftest (S4 wave-2)

- [ ] 7.1 Capture a frozen fixture corpus: a `spec/ledger/**` +
      `spec/divergences/**` sample (point-in-time export from the donor consumer repo,
      the reference source), a `handoffs` table export snapshot (from
      canon's own Postgres-tier table, never a live donor event-store / hosted Postgres query),
      and a `tasks.md` snapshot (point-in-time, never a live query —
      reproducible under `canon selftest`).
- [ ] 7.2 Generate and check in the expected golden verdict-stream JSON for
      the fixture corpus.
- [ ] 7.3 Write the golden-file diff test comparing a fresh run's verdict
      stream against the checked-in golden file byte-for-byte.
- [ ] 7.4 Wire the S4 fixtures into `canon selftest` (design §8: fixture
      corpora with rebindable roots + expected-output diff).
      **Partial — an `artifact-ingest` suite IS registered, but not the
      golden corpus this task names.** `crates/canon-cli/src/artifact_ingest.rs::selftest()`
      is registered in the Wave-3 unified `canon selftest` aggregator
      (`canon_cli::selftest`) as the `artifact-ingest` suite (3 checks
      over the pure `regime_hash` / `trajectory_content_digest`
      write-identity invariants). But 7.4's literal ask — wiring the
      GOLDEN verdict-stream fixture corpus of tasks 7.1-7.3 — stays
      blocked because that corpus is unbuilt: 7.1 needs a point-in-time
      `spec/ledger` + `spec/divergences` export from the donor consumer repo (the
      donor is untouched here) plus a `handoffs` Postgres-table snapshot
      (no live db in this sandbox). Box stays unchecked until 7.1-7.3 land.

## 8. Companion skill (S4 wave-2)

- [x] 8.1 Author the `canon` artifact-ingest / verdict-stream companion
      skill under `canon/skills/` (decision 9): documents the four
      adapters, the verdict mapping table, and how a role-scoped agent
      reads its own verdict stream — materialized for Claude Code + Codex
      only via the content-hash + version install lock.
      — ✅ `canon/skills/canon-artifact-ingest/SKILL.md`, materialized via
      `canon skills install` (`.claude/skills/canon-artifact-ingest/` +
      `.codex/skills/canon-artifact-ingest.md` + `.install-lock.json` bump).
