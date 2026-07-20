## 1. Lift mechanism kickoff (design doc §10 Q5 — RESOLVED)

- [x] 1.1 At S10 kickoff, apply the per-crate lift resolution from the
      crate-graph audit (design.md Open Questions §1;
      the donor adoption brief §1) — no
      open git-dep-vs-source-import binary remains to decide: (a) lift
      the donor's manifest crate+the donor's span crate as a git/path dependency OR a
      verbatim source-import (either is acceptable for these two leaf
      crates specifically — record which one and why, e.g. release-cadence
      tolerance); (b) the donor's checker crate/the donor CLI/the donor's compile crate are
      INSPIRE-only — reimplement their architecture against canon's own
      types, take NO crate dependency on them; (c) the donor's scene-DSL parser crate/
      the donor's CEL-binding crate/the donor's tree-sitter grammar/the donor's LSP crate are SKIPPED entirely. Record
      the (a)-choice and its rationale before crate scaffolding in §2
      depends on it.

## 2. Vocabulary manifest + resolution crate

- [x] 2.1 Scaffold `crates/canon-vocab` per the §1 kickoff decision: a
      git/path dependency on (or a verbatim source-import of)
      the donor's manifest crate+the donor's span crate for manifest-loading, plus a
      canon-native reimplementation of the donor's checker crate's snapshot-folding
      shape (INSPIRE-only per §1 — no crate dependency on the donor's checker crate
      itself).
- [x] 2.2 Port `plugin.yaml`/`directives/*.yaml`/`enums.yaml` loading (attr types:
      scalar, inline enum, list) unchanged in shape from the donor vocabulary project's manifest loader.
- [x] 2.3 Author `canon/vocab/canon.core/plugin.yaml` exporting `directives/` +
      `enums.yaml` — canon's own core plugin, analogous to the donor's core plugin.
- [x] 2.4 Implement `resolve_snapshot(project_dir, profile) -> (CapabilitySnapshot,
      Vec<Diagnostic>)` as the ONE resolution entry point (design.md D3), ported
      from `resolve_document_snapshot`/`fold_env`'s pure, total, non-panicking
      shape.
- [x] 2.5 Add a `canon.project.yaml` (or `.canon/canon.yaml`-embedded) analog of
      the donor's project manifest's `pluginsDir`/`defaultProfile`/`profiles` shape for
      consumer repos to activate/extend the vocabulary.
- [x] 2.6 Unit tests: a well-formed plugin resolves to a directive+enum index; an
      unresolvable `depends` range fails resolution with a diagnostic, not a
      panic.

## 3. Checker + diagnostics

- [x] 3.1 Port `check_directive`'s validation algorithm (unknown tag →
      `E-UNKNOWN-DIRECTIVE`, unknown attr → `E-UNKNOWN-ATTR`, missing required
      attr → `E-MISSING-ATTR`) against `CapabilitySnapshot`, lifted from
      the donor checker's directive validator.
- [x] 3.2 Port the enum-violation "expected one of: …" diagnostic message,
      verbatim from the donor checker's enum-violation message.
- [x] 3.3 Freeze these as canon's own stable failure-class strings (design §7):
      add them to the fixtures/hooks failure-class registry so a future rename
      requires an explicit migration.
- [x] 3.4 Fixture corpus: one YAML atom per diagnostic class (unknown directive,
      unknown attr, missing required attr, invalid enum value, well-formed
      atom) with an EXPECTED-diagnostics file (design §8 GateCtx-fixture
      pattern).

## 4. Typed task atoms

- [x] 4.1 Design + implement the task-atom YAML record shape: `{id, tag, attrs}`,
      with `evidence: {kind, ref}` as a structured attribute (design.md D4) whose
      `kind` domain resolves from S5's parsed `policy.yaml` — not a local
      duplicate enum.
- [x] 4.2 Implement the compiler: validated atom → S1 `Task` record (id,
      description, status, evidence requirement); an atom that fails vocabulary
      validation produces no `Task` record, only diagnostics.
- [x] 4.3 Implement the decompiler: `Task` record → atom, and prove round-trip
      equivalence (same `id`/`tag`/`attrs`, and the decompiled atom itself
      passes validation).
- [x] 4.4 Extend `canon gate task <task_id>` (S5) with the typed-evidence path:
      given a task compiled from a typed atom, check for a matching
      `EvidenceRecord` by `evidence.kind`/`ref` instead of requiring a free-string
      `--verify-via`; the existing free-string path stays available. — ✅
      `crates/canon-cli/src/gate.rs`'s `typed_atom_for_task`/`typed_path_evidence`
      (canon-cli orchestrates canon-vocab validation + canon-gate's UNCHANGED
      `gate_task`, evidence narrowed to the atom's declared `evidence.kind`/`ref`
      via a raw-ledger companion, no canon-gate edit); `cargo test -p canon-cli
      --test gate` (17/17 green, incl. 5 new typed-path tests).
- [x] 4.5 Fixture tests: evidence kind outside the policy-derived domain is
      rejected with the invalid-kinds list; valid evidence kind + matching
      `EvidenceRecord` passes the gate; valid evidence kind without a matching
      record blocks the gate with a stable failure-class message. — ✅ first
      clause already covered (canon-vocab checker); the other two now covered
      by `crates/canon-cli/tests/gate.rs`'s
      `gate_task_typed_path_passes_with_a_matching_typed_evidence_record` /
      `gate_task_typed_path_blocks_on_a_wrong_kind_evidence_record` /
      `gate_task_typed_path_rejects_an_evidence_kind_outside_the_policy_domain`
      (all green).
- [x] 4.6 Round-trip property test (proptest, per design §8): for a generated
      corpus of valid atoms, compile→decompile→compile is idempotent.

## 5. Vocabulary-defined handoff templates

- [x] 5.1 Author one directive per handoff domain (`handoff-dev`, `handoff-
      design`, `handoff-content`, `handoff-test`, …) in canon's core plugin,
      each declaring its body's required/optional fields.
- [x] 5.2 Wire a handoff's `domain` field to select the directive tag; validate
      the handoff body against that directive through the §3 checker, unchanged.
- [x] 5.3 Confirm (test) that body-validation failure never mutates or blocks the
      S1 `Handoff` state-machine fields (`id`, `state`, `chainId`,
      `parentHandoffId`, `seq`, `claimedBy`, `openspecChangeSlug`) — those stay
      wire-compatible with the donor monorepo's `handoffs` table regardless of body outcome.
- [x] 5.4 Fixture tests: missing required body field → `E-MISSING-ATTR`;
      undeclared domain → `E-UNKNOWN-DIRECTIVE`; well-formed body for each
      declared domain passes.

## 6. `canon context` (S12) integration

- [x] 6.1 Wire `canon context`'s snapshot source to `resolve_snapshot` (§2.4) —
      the same function the checker calls — so the emitted authoring surface
      (directive/enum domains) can never diverge from what §3's checker
      enforces. — ✅ `crates/canon-cli/src/context.rs`'s `resolve_surface` calls
      `canon_vocab::resolve_snapshot(repo, None)` and folds it into
      `AuthoringSurface::vocab` (`VocabularySurface { snapshot, diagnostics }`,
      literally `canon_vocab::CapabilitySnapshot`, `Serialize`-derived for this
      one purpose) — no second, hand-projected vocabulary view; `render_json`/
      `render_outline` both read it. `cargo test -p canon-cli --lib context`
      (9/9 green, incl. `vocab_surface_matches_a_fresh_independent_resolve_snapshot_call`).

## 7. Pilot consumer-repo change

- [x] 7.1 Author one real openspec change in a consumer repo using the typed
      task-atom format end to end: atoms validate, compile to the S1 model, and
      round-trip, proving the mechanism outside canon's own fixtures. — ✅
      `openspec/changes/s10-vocab-pilot/` (this repo's own `canon/policy.yaml`
      + `tasks.vocab.yaml`, real files, not test fixtures): `canon context
      --repo .` and `canon gate task s10-vocab-pilot#1 --repo .` were run for
      real against the checked-in repo (flipped `tasks.md` row + a real ledger
      `EvidenceRecord`, `canon/ledger/kind=evidence_record/`); permanently
      re-verified read-only by `crates/canon-vocab/tests/
      pilot_consumer_change.rs` (4/4 green: validates, compiles to `Task`,
      round-trips, and its `evidence.kind` resolves in the real policy domain).

## 8. Companion skill + selftest fixture

- [x] 8.1 Author the companion skill under
      `canon/skills/typed-authoring-vocabulary/` (decision 9): how to declare a
      new directive/enum, how to author a typed task atom with an evidence
      requirement, how to author a vocabulary-defined handoff body, and when to
      run `canon context` first (S12 tie-in).
      — ✅ `canon/skills/typed-authoring-vocabulary/SKILL.md` (all four
      subsections: declare a directive/enum, author a `::task` atom with a
      policy-resolved `evidence` requirement, author a `::handoff-<domain>`
      body, run `canon context` first); materialized via `canon skills install`
      (`.claude/skills/` + `.codex/skills/` + `.install-lock.json`).
- [x] 8.2 Add all fixture corpora from §3.4, §4.5, §4.6, and §5.4 to `canon
      selftest` (design §8) so the vocabulary checker, typed-atom compiler/
      round-trip, and handoff-template validation are covered by the standard
      selftest run.
      — ✅ `crates/canon-vocab/src/selftest.rs::selftest()` resolves the real
      `canon.core` manifest and runs the checker (`validate_atoms` over
      `fixtures/atoms/{good,bad-*}.yaml`), the task-atom `compile_task` +
      `decompile_task` round-trip, and handoff-body validation/compile;
      registered in the unified aggregator (`canon_cli::selftest`) as the
      `typed-authoring-vocabulary` suite (10 checks), green under `canon selftest`.

## Part1/part2 status note (S10Core foundation + S10Part2 canon-cli integration)

§§1-3 and 5 are fully done, with tests, against the crate build/test suite
(`cargo test -p canon-vocab`: 47 unit tests across `checker`/`atom`/
`compile`/`handoff_compile`/`manifest::*`/`policy_bridge`/`resolve_snapshot`/
`span`, plus `tests/canon_core_selftest.rs` — 5 integration tests against the
REAL, checked-in `canon/vocab/canon.core/` manifest, `tests/
pilot_consumer_change.rs` — 4 integration tests against the REAL pilot
change (§7 below), and `tests/proptest_roundtrip.rs`'s generated-corpus
round-trip property test; 57 tests total, all green). §4 (typed task
atoms) is now FULLY done, including the gate-integration half S10Core
left deferred: 4.4/4.5 land in `crates/canon-cli/src/gate.rs`, orchestrating
canon-vocab validation over canon-gate's UNCHANGED `gate_task` (no
canon-gate edit, `cargo tree -p canon-gate` carries no `canon-vocab`
edge — the cycle-avoidance constraint holds). §6 (`canon context`) and §7
(pilot consumer-repo change) are done: `crates/canon-cli/src/context.rs`'s
`AuthoringSurface` now also carries `canon_vocab::resolve_snapshot`'s own
output verbatim (`VocabularySurface`), and
`openspec/changes/s10-vocab-pilot/` is a real, checked-in consumer change
(this repo's own new `canon/policy.yaml` + a typed `tasks.vocab.yaml`)
whose task was gated and flipped for real via the typed evidence path.
§8 (companion skill under `canon/skills/` + `canon selftest` CLI wiring)
is now DONE: the skill is authored + materialized (8.1) and the vocab
fixture corpora run under the unified `canon selftest` aggregator as the
`typed-authoring-vocabulary` suite (8.2).
