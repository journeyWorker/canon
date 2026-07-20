# Tasks — s15 spec-ledger unification

Sequencing is load-bearing: **P1 (identity) + P2 land strictly before P3a
(sync)** — once the first real `sync` runs, the free fixture-only migration
window for `project_id` closes (design R1).

## 1. canon-model schema wave (P1 — no behavior; everything depends on it)

- [ ] 1.1 Add newtypes to `canon-model/src/ids.rs`: `ProjectId`
      (`[a-z0-9][a-z0-9-]*`, no `_`), `TotalOrder` (run_seq rank), `SpecDigest`
      (sha256-hex over `.feature` bytes — NOT `ids::Sha`, a 40-hex git sha).
- [ ] 1.2 Add REQUIRED `project_id: ProjectId` to `Scenario`, `Review`,
      `Divergence`; OPTIONAL `project_id: Option<ProjectId>` to `EvidenceRecord`
      (real records may exist via promote). New schema versions; wire-additive.
- [ ] 1.3 Add `Scenario` index field `source_digest: SpecDigest` (keep existing
      `title` AND the optional `description`). NOTE: P1 also shipped `covered`/
      `surface_ref`; P3a (task 3.3) REMOVES those — the Scenario index is general,
      no donor-inventory-derived fields; coverage stays the gate's authority.
- [ ] 1.4 Extend `DivergenceStatus` with `StillDivergent` and
      `Deferred { reason, expiry }` (additive; old `{open,resolved}` still parse).
- [ ] 1.5 Move the five gate companion fields onto `EvidenceRecord` as native,
      OPTIONAL, typed fields: `lifecycle: Option<TrustLifecycle>`,
      `flagged: Option<FlaggedOverlay>`, `evidence_sha: Option<Sha>`,
      `surface_ref: Vec<String>` (default-empty when absent — the `Vec`
      exception to the `Option<T>` shape the other four use),
      `run_seq: Option<TotalOrder>`. Read is THREE-way: an
      ABSENT field → documented safe default (`draft`/unflagged/
      staleness-unresolvable/empty/none, so old promoted records stay
      legitimate); a PRESENT well-formed field → typed; a PRESENT malformed
      value MUST fail the record loud (the `Option<T>` default fires ONLY for a
      missing key — never collapsing a present malformed value→absent). (`Review`
      needs only `project_id`.)
- [ ] 1.6 Pure `fold_to_current_state(records, live_bindings, as_of)` in
      canon-model: rank by `run_seq: TotalOrder` sole-primary, `round`
      tiebreak-only never `Ord`, group by `(project_id, scenario_id)`; emit a
      separate `FoldedState` output whose `ResolvedInvalid` is fold-derived from
      the passed-in live-binding re-check (no persisted status, no TOCTOU).
- [ ] 1.7 Migrate every `#[cfg(test)]`/fixture construction site to the new
      required fields (canon-store tests, canon-report `fixtures/corpus.rs`,
      canon-gate selftest corpora). No production producers exist to migrate.
- [ ] 1.8 canon-model unit tests: newtype round-trips; `DivergenceStatus`
      back-compat parse; fold ordering (lower run_seq at higher round);
      `ResolvedInvalid` derivation; malformed native field rejected loud.

## 2. canon-store wave (P2)

- [ ] 2.1 `resolve_partition` composite natural_keys: `Scenario`
      `<project_id>__<scenario_id>`, `Review` `…__<pin>`, `Divergence`
      `…__<run_seq>__<round>` — always prefixed, no new Hive `project=` dimension.
- [ ] 2.2 Hoist a generic `fold_latest_by_key` (last-wins by `envelope.at`)
      into canon-store, generalizing `canon-gate::ledger::latest_verdicts`; the
      ≥4 consumers (sync, fold, gate staleness, flywheel) reuse it.
- [ ] 2.3 Unknown-`kind=<x>/` directory → skip + report as foreign-namespace,
      never malformed core evidence (forward-compat for s16 plugin kinds).
- [ ] 2.4 Delete `canon-store::git_tier::migrate_write` (dead code, zero
      non-test callers — the killed writer's last overwrite exception).
- [ ] 2.5 canon-store tests: composite-key layout round-trip; two roots sharing
      a `scenario_id` produce distinct paths; unknown-kind dir skipped+reported.

## 3. inventory sync (P3a — after P2)

- [ ] 3.1 `specs.roots[]` config resolution from `canon.yaml` (`[{id, root}]`,
      default single `{id: root, root: specs}`, `id` a stable literal never the
      checkout dir name); fail-loud on a present-but-malformed section, missing →
      default. Reuse ONLY the fail-loud SEMANTICS of `IngestSourceConfig::load`.
- [ ] 3.2 Gherkin-scan surfacing in `canon-fmt`: attach each `@<area>.<surface>.
      <nn>` tag to its following header and expose the header label as `title`
      (retention of what the line-scan already reads — NOT a new parser); add a
      `source_digest` (sha256 over `.feature` bytes) helper.
- [ ] 3.3 REMOVE `Scenario.covered` + `surface_ref` from canon-model (P1 added
      them; dropped as non-general) + `Scenario::new`/builder + every callsite and
      fixture. Core sync derives the index from the `.feature` corpus ALONE — NO
      `upstream`/`InventoryEntry.covered_by` read (a donor-repo porting
      concern, never core canon, which is a general tool).
- [ ] 3.4 `canon inventory sync [--spec-root <dir>]`: per root, run
      `canon-fmt::check` (ANY violation aborts the whole root — no partial sync),
      then upsert one `Scenario` index per `(project_id, scenario_id)` via the
      Tier write. Logical idempotence: fold latest-per-key, no-op when
      `source_digest` (and the derived `title`) match.
- [ ] 3.5 `Command::Inventory` arm + `inventory.rs`; tests: materialize one
      record per scenario with correct `source_digest`/`title`; sync ignores any
      inventory directory (general, feature-corpus only); re-sync unchanged → zero writes; changed
      doc → one new record; malformed corpus → no writes + loud.

## 4. gate wave + native verdict producers (P3b — parallel with P3a after P2)

- [ ] 4.1 Gate reads the five native `EvidenceRecord` fields off `ctx.evidence`;
      DELETE `trust.rs::{TrustLadderTag, trust_ladder_tag_of}`,
      `staleness.rs::{SurfaceHint, surface_hint_of}`, and the second `GitTier`
      construction in `fold_latest_green_cells`. Read is THREE-way (absent →
      documented default, present well-formed → typed, present malformed →
      visible `malformed-evidence`); the human-only `flagged` ratchet
      (`attempt_clear`) is untouched. NOTE `promote.rs:85` is a SECOND caller of
      `trust_ladder_tag_of` (its pre-`run_seq` raw malformed-present guard) —
      rewire it to the native three-way read (preserving the present-malformed
      refusal) BEFORE deleting the helper, so this task never removes a live
      safety check.
- [ ] 4.2 Extend `canon-gate::promote` (hardcoded to `EvidenceRecord`) to
      `RecordKind::Divergence`: the DIVERGENCE path partitions run_seq by
      `(project_id, role, surface)` (`Divergence.project_id` is required); the
      existing `EvidenceRecord` path keeps its current `(role, surface)` axis
      UNCHANGED (its `project_id` is optional — not folded into the key here).
      The malformed-present guard reads the native field (per 4.1's rewire), not
      the deleted `trust_ladder_tag_of`; refusals never consume a `run_seq`.
- [ ] 4.3 `canon review add`: native `Review` producer (actor-attributed,
      provenance-ref enforced) + `Command` arm.
- [ ] 4.4 `canon divergence {stage,promote,resolve,defer}`: stage →
      `promote` assigns the monotonic `run_seq` → `resolve`/`defer` transitions +
      `Command` arm.
- [ ] 4.5 Consume `fold_to_current_state` in the S9 divergence burn-down (the
      canon-store/canon-report caller supplies the live-binding re-check map).
- [ ] 4.6 gate/producer tests: a present-but-malformed field → `malformed-
      evidence` (never green); an ABSENT field → documented default (never
      malformed — an old record with no native fields stays legitimate);
      `stage`+`promote` assigns a monotonic run_seq; refusal consumes none; fold
      downgrades a stale `Resolved` to `ResolvedInvalid`; `Deferred` honors
      `as_of` expiry.
- [ ] 4.7 Make the gate's review index project-aware: `trust.rs::review_index`
      keys by `(project_id, scenario_id)` for evidence carrying `Some(project_id)`
      (a review for one project never satisfies another project's same-`scenario_id`
      evidence); `project_id = None` legacy evidence falls back to the
      bare-`scenario_id` match (no regression). Test: a multi-root `(app-a, X)`
      review does NOT review `(app-b, X)` evidence; legacy None-evidence still
      matches by `scenario_id`.

## 5. flywheel (P4)

- [x] 5.1 Native verdict records-source adapters — ONE per verdict kind
      (`Review`, `Divergence`), each a single-`RecordKind` registry entry +
      `record_kind_for_records_adapter` arm (matching the existing
      one-adapter-one-kind dispatch), mirroring the `Handoff` handle-based shape.
      `Scenario` is NOT a source (it is a no-verdict index materialized by sync).
- [x] 5.2 Regime derivation for the adapter (role ← `actor.role`, area ←
      `scenario_id.area()`, repo ← root) since `Trajectory` is `regime_key`-only;
      fold verdicts via `store_trajectory` + `rebuild_namespace`.
- [x] 5.3 `artifacts.native_records: bool` (default false) switch enabling the
      `Review`/`Divergence` verdict adapters against canon's own tiers; XOR with
      the raw-artifact path fields
      (`ledger_root`/`divergences_root`/`openspec_root`) — config validation
      rejects `native_records:true` + ANY raw path before any read. Scopes ONLY
      the native verdict adapters; the existing `Handoff` Records adapter is
      UNAFFECTED. The driver runs the verdict adapters only when the switch is on.
- [x] 5.4 flywheel tests: a `Review` AND a `Divergence` in the SAME run BOTH
      produce trajectories (neither dropped by single-kind dispatch; parity with
      the S4 path); `native_records:true` + a raw path fails the XOR check; the
      `Handoff` adapter is unaffected by the switch.

## 6. closure (P5)

- [x] 6.1 Selftest fixture corpora with TWO-SIDED exact-set oracles (missing AND
      extra), a rebindable-roots `SyncCtx` (two constructors, offline tempdir),
      and a frozen-incident slot; register in `canon selftest`.
- [x] 6.2 Companion skill `canon/skills/canon-inventory/SKILL.md` (the unified
      loop: author corpus → `sync` → `review`/`gate` → flywheel); materialize via
      `canon skills install` + install-lock bump.
- [x] 6.3 Reconcile design docs / capability skills touched; `bunx openspec
      validate --strict` green for this change.

## 7. Verification

- [ ] 7.1 `cargo build --workspace` + `cargo clippy --workspace --all-targets --
      -D warnings` + `cargo test --workspace --no-fail-fast` (bare, no pipe
      masking) all green.
- [ ] 7.2 `canon selftest` all suites green including the new `inventory-*`/
      native-record fixtures.
