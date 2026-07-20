# Design — s17 plan import (integration layer)

## Current state (accurate baseline, verified)

- **The two connector families this mirrors.** `canon-ingest` carries two
  proven trait + static-registry pairs: `SessionAdapter`/`registry` (S3 —
  omp/claude/codex/hermes transcripts → `Session`/`Run`/`Event`) and
  `ArtifactAdapter`/`artifact_registry` (S4/S15 — ledger/divergence/
  handoff/openspec-task/review/divergence-native artifacts → verdict
  trajectories). Both registries are static and declaration-ordered ("no
  dynamic plugin loading", S3 design D1; "deterministic order, never
  HashMap-iteration-order dependent", `artifact_registry.rs`), and the S4
  registry additionally proves the explicit-diagnostic discipline
  (`ArtifactDispatchOutcome::UnsupportedSource` exists precisely so an
  undrivable adapter is never folded into a silent empty outcome).
- **The drivers this mirrors.** `crates/canon-cli/src/ingest.rs` (sessions)
  is the one place canon-ingest meets canon-store: strict-on-present
  `canon.yaml` config (`RawIngest`, `deny_unknown_fields`, fail-loud on a
  typo'd key or unknown source id), the S3 §3 watermark gate
  (`canon_store::cursor::SourceCursor` — source-granular, content-digest,
  never mtime-trusting), `persist_idempotent` (git-tier `DuplicatePath` =
  successful no-op), and the documented `unwritten` seam (unreachable tier
  → print the normalized bundle, never a fatal error).
  `artifact_ingest.rs` (S14) generalizes the same shape to the artifact
  family. s17's driver is the third instance of this exact pattern.
- **The record kinds this maps onto — already stored, routed, and typed.**
  `Change { envelope, change_id, title, summary, status }` with
  `ChangeStatus { Proposed, InProgress, Completed, Archived }` — whose own
  doc comment says it "mirrors an openspec change's own proposal → tasks →
  archive flow" — and `Task { envelope, task_id, title, status,
  evidence_note }` with `TaskStatus { Open, Done }` and the "one-line
  evidence note" discipline (`canon-model/src/records.rs`). `canon.yaml`
  routes `change: git`, `task: pg` (S2 `TierPolicy`); `partition.rs`
  resolves their natural keys (`change_id`, `task_id`); the schema registry
  binds their fields for CEL policy (S13). NOTHING here changes — s17 only
  produces records into it.
- **The identity grammars.** `ChangeId` is a kebab slug; `TaskId` is
  `<change_id>#<n>` with `<n>` one or more dot-separated integers
  (`canon-model/src/ids.rs`); construction is the only rejection point.
- **The verdict-side reader of the SAME files.** S4's
  `artifact_adapters/openspec_task.rs` walks
  `<root>/openspec/changes/**/tasks.md`, derives `change_id` from the
  containing dir's basename (`change_id_for`), keys events
  `<change_id>#<n>`, and carries a LOCAL, independently-maintained mirror
  of canon-gate's canonical checkbox row grammar (`- [ ]`/`- [x]` + an
  optional `**DEFERRED to §<to>**`/`**DROPPED**` annotation + a checked
  row's ` — ✅ <evidence>` suffix) because canon-ingest deliberately has no
  canon-gate dependency (operator directive recorded in its module doc).
  s17's plan adapter reads the same rows for a DIFFERENT job — plan state,
  not verdict events — and must derive byte-identical `task_id` values or
  the join spine never joins.
- **Who produces Change/Task today.** No `Change` producer exists outside
  tests/fixtures/schema-walkers (verified by walking every
  `Change::new`/`RecordKind::Change` site). `Task` has exactly one
  production producer: canon-vocab's typed compile (`compile.rs`, the S10
  `canon gate task` path) — one hand-authored task at a time. Bulk plan
  state has no producer; that is the hole s17 fills.
- **The sibling boundary.** s16 shipped ledger-record OVERLAYS (foreign
  NAMESPACED kinds beside core dirs, projection at read time); s17 ships
  IMPORT (foreign dialects normalized INTO core kinds through the standard
  write path). The two never overlap: an import writes ordinary core
  records; an overlay never writes a core kind.

## Architecture — the third ingest family (sits beside the first two)

```
  foreign sources                    canon-ingest (pure scan/parse/normalize; no canon-store dep)
  ┌─ agent transcripts ────▶ SessionAdapter  + registry           ─┐
  ├─ verdict artifacts ────▶ ArtifactAdapter + artifact_registry  ─┼─▶ normalized candidates
  └─ PLAN dialects ────────▶ PlanAdapter     + plan_registry      ─┘        │
     (s17: openspec;                                                        ▼
      deferred: superpowers,      canon-cli driver (the ONE place adapter meets store)
      donor-JSON)                 canon.yaml plans: {sources: [{dialect, root}]}
                                  │ SourceCursor gate (content-digest, source-granular)
                                  ▼
                                  TierRegistry::persist  (validated tiered write)
                                  ├─ Change → git tier   (canon.yaml routing, S2)
                                  └─ Task   → pg tier    (unreachable → unwritten seam)

  authority plane — UNTOUCHED: canon-gate (uncovered-cell, trust ladder),
  S7 promotion, canon inventory sync (sole Scenario producer)
```

## Dialect → RecordKind mapping (openspec, the reference dialect)

| Foreign construct | Canon record | Identity | Derivation |
| --- | --- | --- | --- |
| `openspec/changes/<slug>/` dir (non-archive) | `Change` | `change_id` = dir basename, verbatim, via `ChangeId::parse` | `title` = slug; `summary` = first paragraph under proposal.md `## Why`, whitespace-normalized; `status` per D6 |
| `openspec/changes/archive/<basename>/` dir | `Change` | `change_id` = dir basename, verbatim (D4: NO date-prefix stripping) | `status` = `archived` regardless of checkbox tallies |
| `tasks.md` row `- [ ] <n> …` / `- [x] <n> …` | `Task` | `task_id` = `<change_id>#<n>` via `TaskId::parse` | `status` = `open`/`done` from the checkbox, verbatim; `title` = row text after id/annotation, evidence suffix excluded; `evidence_note` = ` — ✅ ` suffix when present, else the `**DEFERRED to §<to>**`/`**DROPPED**` annotation text when present, else absent |
| proposal.md prose beyond the first `## Why` paragraph | — (not imported) | — | authored document stays the source of truth; no diagnostic (deliberate partial read, not a drop) |
| `specs/**/spec.md` `#### Scenario:` blocks | — DROPPED | — | named diagnostic `spec-delta-scenario`, counted per source (D3) |
| `design.md` | — DROPPED | — | named diagnostic `design-doc`, counted per source (D3) |
| a dir whose basename fails `ChangeId::parse`, or an unreadable/missing proposal.md | — skipped | — | counted malformed (whole change dir), siblings unaffected |
| tasks.md absent | `Change` only | — | a legitimate proposal-stage change: zero `Task` records, `status` = `proposed`, no diagnostic |

Deferred dialects (D9) map onto the SAME two kinds; nothing in this table
is openspec-load-bearing beyond the adapter itself.

## Decisions

- **D1 — Crate boundary: extend `canon-ingest` with a third family; no new
  `canon-import` crate.** canon-ingest IS canon's connector crate: it
  already holds two trait + static-registry families, the shared
  scanner/normalize/content-digest plumbing a plan adapter needs, and the
  load-bearing boundary discipline s17 must inherit — no canon-store
  dependency (pure scan/parse/normalize; canon-cli is "the one place the
  two meet", `ingest.rs`'s own words). A new `canon-import` crate would
  either duplicate that plumbing or force canon-ingest to export it,
  splitting one pillar across two crates while gaining zero boundary
  (nothing in the new crate could legitimately depend on anything the old
  one couldn't). Precedent is explicit: S4 added the artifact family to
  canon-ingest rather than minting `canon-artifacts`. Rejected
  alternative: a `canon-import` crate "for symmetry with canon-plugin" —
  false symmetry; canon-plugin exists because overlay manifests are a NEW
  content domain with no home crate, whereas plan import is a third
  instance of a domain canon-ingest already owns.
- **D2 — CLI surface: `canon ingest plans`, not a new `canon import`
  top-level verb.** "Bring external data into canon's record model" is
  already spelled `canon ingest <family>` twice (`sessions`, `artifacts`);
  a plan import is that same role for a third source family — the
  Jira-importing-GitHub analogy describes the connector's ROLE, not a verb.
  A second top-level verb meaning the same thing would fragment the
  surface and force every future doc to explain why import≠ingest.
  Config-driven by default (`canon.yaml` `plans:` — mirrors
  `ingest.sources`/`artifacts:`), with a targeted one-shot escape hatch:
  `canon ingest plans --dialect <id> --source <path>` (both flags
  required together, fail loud otherwise; unknown dialect id fails loud
  naming the registered ids). No `--watch` (proposal non-goal): plans are
  operator-pulled, not streamed; the cursor gate keeps re-runs cheap.
- **D3 — Mapping targets are `Change` + `Task` ONLY; every other foreign
  construct is dropped with a NAMED diagnostic — never an overlay, never a
  new kind.** The closed-kind rule is s16's hardest-won lesson applied to
  import: `RecordKind`'s 12-member closure is the acceptance bar. For
  openspec's unmappable constructs the overlay question was checked, not
  hand-waved: an s16 overlay attaches to an EXISTING core record —
  concretely `attaches_to.core_kind: scenario` only (s16 task 1.4) — on
  `(project_id, scenario_id)`. A spec-delta `#### Scenario:` block has no
  core `Scenario` record (it fails the `<area>.<surface>.<NN>` grammar and
  `canon inventory sync` is the sole producer, s15 P3a), so there is
  nothing to attach to: overlay is structurally unavailable, and
  drop-with-diagnostic is the honest remainder. `PlanParseOutcome` carries
  the drop counts keyed by construct name (`spec-delta-scenario`,
  `design-doc`) so a consumer can SEE what import deliberately left
  behind. If a future dialect carries per-change data that genuinely wants
  projection onto Change/Task views, that is an s16 plugin-manifest
  EXTENSION (widening `attaches_to.core_kind` — its own reviewed change),
  never importer-private logic.
- **D4 — Identity: `change_id` = the change dir's basename, VERBATIM;
  `task_id` = `<change_id>#<n>`; parity with the S4 verdict adapter is a
  REQUIREMENT, not a preference.** S4's `change_id_for` uses the parent
  dir basename with no normalization; s17 does exactly the same, because
  the entire point of importing plans is that `Task.task_id` equals the
  `task_id` S4's verdict events already carry — one string mismatch and
  plan ↔ evidence ↔ trajectory never joins. Consequence, accepted and
  documented (R2): the openspec CLI renames a dir on archive
  (`archive/YYYY-MM-DD-<slug>/`), so an archived change forks identity
  from its live predecessor — in BOTH readers, consistently. Rejected
  alternative: stripping the date prefix on import — it would repair the
  fork on the plan side while the verdict side (frozen, S4) keeps the
  prefixed id, silently BREAKING the join this change exists to create.
  Identity fidelity to the shipped verdict layer wins over cosmetic
  continuity. `ChangeId::parse`/`TaskId::parse` remain the only
  constructors — a basename that fails the grammar skips the whole dir,
  counted, mirroring `parse_tasks_file`'s existing discipline.
- **D5 — One checkbox grammar inside canon-ingest, two consumers;
  canon-gate stays the format authority.** The row grammar
  (`- [ ]`/`- [x]` + annotation + evidence suffix) currently lives twice:
  canon-gate's `checkbox.rs` (the AUTHORITY — writes and validates) and
  openspec_task.rs's local read-only mirror (operator directive: no
  canon-gate dep for canon-ingest). s17 needs the same rows a third time.
  Adding a second in-crate mirror would be drift-by-construction, so the
  existing mirror is EXTRACTED into a shared canon-ingest module
  (`openspec_rows.rs`) that both the verdict adapter and the plan adapter
  consume — code motion, zero behavior change, pinned by the verdict
  adapter's existing tests. The crate-boundary directive is preserved
  (still no canon-gate dependency); the mirror is maintained in ONE place
  instead of two.
- **D6 — `ChangeStatus` derivation is a pure function of the snapshot.**
  A dir under `changes/archive/` → `archived`, unconditionally. Otherwise:
  zero parseable checkbox rows (including tasks.md absent — a legitimate
  proposal-stage change) → `proposed`; ≥1 done and ≥1 open →
  `in_progress`; all done (≥1 row) → `completed`; none done → `proposed`.
  A `**DEFERRED**`/`**DROPPED**` row counts by its CHECKBOX state alone
  (the annotation is scheduling metadata, carried in `evidence_note`,
  never a status the importer invents — mirrors S4's "deferral/drop is a
  scheduling fact" discipline). Deterministic: same tree bytes → same
  status, no wall-clock input.
- **D7 — Determinism + idempotence: source-derived `at`, fixed actor,
  cursor gate + digest-tolerant persist.** Envelope `at` is NEVER
  `Utc::now()` in the body-derivation path: `Task.at` = its tasks.md
  file's mtime; `Change.at` = the max mtime across the files its
  derivation read (proposal.md + tasks.md) — the same "file's own mtime is
  the best available 'when observed' signal" idiom every existing
  file-sourced adapter in this crate uses, and a checkbox flip advances
  tasks.md's mtime so the refreshed Change/Task bodies sort NEWER in
  fold-latest reads (supersession). Actor is a FIXED
  `Actor::new_unattributed("canon-plan-import-<dialect>")` per dialect —
  provenance visible in every record, byte-stable across runs. Idempotence
  is the proven two-layer defense: (a) the SourceCursor content-digest
  gate skips an unchanged source wholesale (so mtime churn without byte
  churn — a `git checkout`, a `touch` — never even reaches parse), and
  (b) `persist_idempotent` treats a byte-identical git-tier resubmission
  (`DuplicatePath`) as a successful no-op while pg/r2 report
  `deduped: true` natively. Net contract: re-importing an unchanged
  foreign plan writes ZERO new records.
- **D8 — Cross-source `change_id` collision: first-configured wins the
  pass, later occurrence diagnosed.** Two configured sources can both
  carry a change dir with the same basename (two repos, or a repo plus
  its fork). Within one pass, the first occurrence in deterministic
  config/registry order imports; every later same-`change_id` occurrence
  in that pass is skipped + diagnosed naming both sources (mirrors
  canon-plugin's duplicate-id "drop the later, never silently merge"
  discipline, s16 R6). Across passes, fold-latest-by-`at` governs like
  any other record history — the ledger is append-only and both histories
  remain inspectable.
- **D9 — Deferred dialects, sketched so the seam is provably sufficient.**
  `superpowers` (named follow-up: `plan-dialect-superpowers`): a plan doc
  maps `# <title>` → `Change` (`change_id` = slugified filename;
  `summary` = overview section), `### Task N: <name>` sections → `Task`
  (`task_id` = `<change_id>#<N>` — the heading numbers are the stable ids;
  completion from the section's own checkbox state). Deferred because the
  checklist grammar has no format authority yet — pinning it here would
  freeze a speculative shape; the adapter is one registry entry when it
  lands. `donor-JSON` re-homing (named follow-up:
  `plan-dialect-donor-json`): deferred until a concrete donor PLAN corpus
  exists — the donor JSON canon knows today (ledger/divergence/handoff/
  task-state) is EVIDENCE and is already ingested by S4 as verdicts; s17
  formally adopts those adapters as the evidence-side of this pillar,
  zero code. That the registry seam suffices is proven IN this change by
  a second, fixture dialect adapter registered in tests: adding it
  touches exactly one registry entry + one adapter module.

## Risks

- **R1 authority creep (highest; mitigated structurally).** An importer
  that gate/promotion logic starts reading becomes a second authority.
  Mitigation: canon-gate/canon-learn source carries zero references to the
  plan family; acceptance test pins `canon gate check` verdicts
  byte-identical with and without a prior `canon ingest plans` run.
  Imported records are ordinary `Change`/`Task` rows — anything that
  already reads those kinds (S9 marts, `canon query`) sees them exactly as
  it sees native ones, which is the POINT of mapping onto core kinds; what
  is forbidden is any consumer branching on the importer's actor string to
  treat imported rows specially.
- **R2 slug-trusted identity.** `change_id` is the foreign dir's basename:
  an archive rename forks identity (accepted, D4 — consistent across both
  readers); two unrelated repos can reuse a slug (handled per-pass by D8,
  per-history by append-only fold). The residual risk — one repo's plan
  history interleaving another's under a shared slug across passes — is
  documented and accepted for s17 (openspec slugs are long and prefixed in
  practice, e.g. `s17-plan-import`); a per-source namespace would break
  task_id join parity with the S4 verdict layer and is explicitly rejected.
- **R3 mtime-derived `at` across machines.** A fresh clone re-stamps
  mtimes; a second machine importing the same unchanged repo writes bodies
  whose `at` differs → new digest paths (record inflation), though
  fold-latest state stays logically identical and the per-machine cursor
  prevents same-machine churn. Accepted: it is this crate's established
  convention (every file-sourced adapter), and the alternative — a
  content-derived constant `at` — breaks supersession ordering outright.
- **R4 dialect drift.** The openspec CLI's on-disk format can evolve.
  Fail-soft per construct (skip + count) keeps import degrading instead of
  crashing; a grammar change lands as a reviewed adapter update. The
  shared-grammar module (D5) means one fix serves both consumers.
- **R5 two readers of the same files, different jobs.** S4's verdict
  adapter (events for the reward flywheel) and s17's plan adapter
  (plan-state records) both read `openspec/changes/**`. Confusing them
  invites double-counting fears; in fact they write DISJOINT outputs
  (trajectories vs Change/Task records) keyed by the SAME `task_id` —
  that shared key is the join, not a collision. Both module docs
  cross-reference each other; the companion skill shows the two-sided
  join as the worked example.
- **R6 pg-routed `Task` writes.** `task: pg` means a plan import without a
  live DSN cannot persist tasks. Mitigated by the SAME documented seam
  sessions ingest already has: the normalized bundle degrades to the
  `unwritten` output (printed, never silently dropped, never fatal to the
  git-routed `Change` writes that already landed); the cursor only
  advances after a fully-persisted pass, so nothing is lost.

## Sequencing

- **P1 — plan-connector foundation (canon-ingest).** `PlanAdapter` trait +
  `PlanParseOutcome` (candidates + named drop counts + malformed count);
  static `plan_registry` (declaration-ordered); the shared
  `openspec_rows.rs` grammar extraction with openspec_task.rs re-pointed
  at it (zero behavior change, existing tests pin it). Pure canon-ingest;
  no store, no CLI.
- **P2 — openspec dialect adapter (after P1).** Change-dir discovery
  (mirrors `discover_task_files`'s root-shape tolerance), proposal.md
  title/summary extraction, D6 status derivation, tasks.md → Task mapping
  with task_id parity, D3 drop diagnostics, archive handling.
- **P3 — CLI wiring + persistence (canon-cli, after P2).**
  `IngestCommand::Plans`; strict `plans:` config parse; per-source cursor
  gate; `persist_idempotent` through `TierRegistry::persist`; `unwritten`
  seam; `--dialect`/`--source` one-shot pair; human/JSON output.
- **P4 — closure (after P1-P3).** Selftest fixture (a synthetic openspec
  change tree with live/archive/malformed/proposal-only dirs, rebindable
  root, registered in `canon selftest`); the fixture second dialect
  proving one-entry registry extension; companion skill
  `canon/skills/canon-plan-import/SKILL.md` + install-lock bump; doc
  reconciliation (s15/s16's s17 forward pointers read true against what
  actually got built).

## Testing

- Registry/trait: declaration order deterministic; unknown dialect id
  fails loud naming registered ids; the fixture second dialect lands as
  one registry entry + one module (structural proof of the seam).
- Grammar extraction: the S4 verdict adapter's full existing test suite
  passes unchanged against the shared module (zero behavior change).
- openspec mapping: a live change dir round-trips to one Change + N Tasks
  with byte-exact task_id parity against `openspec_task.rs`'s derivation
  over the same fixture; archive dir → `archived`; proposal-only dir →
  `proposed`, zero tasks, zero diagnostics; all-done → `completed`; mixed
  → `in_progress`; DEFERRED/DROPPED rows keep checkbox-derived status with
  the annotation in `evidence_note`; a non-`ChangeId` basename or missing
  proposal.md skips that dir, counted, siblings import; spec-delta
  scenarios + design.md increment their NAMED drop counts and produce no
  records.
- Idempotence/determinism: a second `canon ingest plans` over an unchanged
  source writes zero new records (cursor skip); byte-churn-free mtime
  churn never reaches parse; a checkbox flip produces exactly the
  refreshed Change + flipped Task, which win fold-latest; two runs over
  the same snapshot produce byte-identical bodies (fixed actor, mtime
  `at`, no wall-clock).
- Config/CLI: absent `plans:` → zero sources, clean exit; typo'd key or
  unknown dialect → loud error; `--dialect` without `--source` (and vice
  versa) → loud error; unreachable pg → Change persisted to git, Tasks
  reported via the unwritten seam, cursor NOT advanced.
- Authority: `canon gate check` verdicts byte-identical with/without a
  prior plan import; no canon-gate/canon-learn source reference to the
  plan family; `RecordKind::ALL.len() == 12` assertions untouched.
- selftest: the fixture corpus registered in `canon selftest`, rebindable
  root (mirrors s15/s16's pattern).

