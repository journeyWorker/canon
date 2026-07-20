## Context

openspec task completion is self-reported (design doc §1). The donor parity
harness proves the fix works at scale: a static coverage gate
(`_ledger_layout_problem`, `_evidence_violations`), a dynamic verdict ledger
(append-only `spec/ledger/`, join key = scenario ID), a trust ladder (D21:
`draft → reviewed → ratified` + human-only `flagged`), policy-derived
required cells (D7 `spec/policy.yaml`), staleness (`staleness.surface_scoped`
/ `staleness.max_commits_behind`), and a serialized staging→promote step
(O13 `cmd_promote`: monotonic per-(lane, surface) `run_seq`, re-validate
before landing, refuse without consuming a seq). The donor monorepo's donor CLI independently
built an overlapping but narrower slice: its task-flip and marker-scan logic
enforce evidence-gated checkbox flips
in ONE worktree, with no coverage gate, no trust ladder, no staleness, no
staging/promote, and no cross-repo reuse (`hook run <kind>` dispatch is
donor-CLI-specific). `canon-gate` generalizes the donor parity harness's machinery into a
versioned, cross-repo Rust crate AND becomes canon's format authority for the
openspec checkbox grammar itself — the piece the donor CLI currently owns informally.

## Goals / Non-Goals

**Goals:**
- Generalize D3's covered-vs-green split into a domain-agnostic static +
  dynamic gate over any `EvidenceRecord`-shaped corpus (ledger records,
  openspec tasks, future artifact families).
- Generalize D21's trust ladder (`draft → reviewed → ratified`, `flagged`
  human-only sticky overlay) as `canon-model` types + gate logic.
- Generalize D7's policy-derived requirement routing via a versioned
  `policy.yaml` schema.
- Generalize staleness (surface-scoped git-diff / `max_commits_behind`
  ceiling).
- Generalize O13's staging→promote with monotonic per-(role, surface)
  `run_seq`, serialized, refusal-safe (no gaps on refusal).
- Own the openspec task/checkbox grammar (`- [ ] ` / `- [x] `) as canon's own
  parser + writer — not a wrapper on the donor CLI's.
- Ship `canon gate task <task_id>`, wired via a hook seam into consumer
  repos: the donor monorepo's `.claude/settings.json` / `.codex/hooks.json`, plus a
  generic pre-commit script for non-donor-CLI repos.
- Ship a fixture-corpus selftest with EXPECTED-violation files, one per
  stable failure-class string (GateCtx pattern).

**Non-Goals:**
- Rewriting the donor parity harness wholesale — S11 scopes only the sync-patch
  delegation boundary; the donor parity harness stays the consumer-side gate for
  platform/lane-specific checks (D5 lanes, D15 drills, D20 invention).
- Migrating the donor CLI's task-flip / marker-scan callers to
  `canon gate task` in THIS change — S5 builds the capability and the
  migration-target boundary; the donor-CLI-side cutover is a follow-up,
  donor-CLI-owned change.
- A UI/dashboard surface for trust state — S9.
- Role-namespaced retrieval or reward wiring — S6/S7/S8.
- Postgres/R2 hot/cold tiers for ledger writes — S5 writes through
  `canon-store`'s git tier only (S2); hot/cold aging is S2's concern,
  consumed but not owned here.

## Decisions

1. **Static/dynamic split as two independent checks (D3).** Coverage
   (policy-derived cell existence) and the verdict ledger (pass/fail
   evidence, by whom, how stale) stay separate passes with separate
   failure classes, mirroring the donor parity harness's `coverage` vs ledger-driven
   `report`. "A test exists" and "a test passed" are different facts with
   different staleness windows. *Alternative rejected:* one combined
   "is this done" boolean — re-creates the §22 failure where existence
   silently stands in for a pass/fail fact.

2. **Trust ladder as an enum on the evidence envelope, not a boolean.**
   Lifecycle tags, exactly one per artifact: `draft | reviewed | ratified`
   + an orthogonal `flagged` overlay (D21). `reviewed` requires an
   accompanying review-record — its absence is `unreviewed-promotion`.
   `flagged` is human-only, sticky, cleared only by a human-attributed
   clear-record staged in the same commit. *Alternative rejected:* a
   binary ratified/not-ratified gate (D14, superseded by D21 specifically
   because it does not scale to an operator hand-flipping every item).

3. **Policy-derived requirements via `policy.yaml`, never per-artifact
   judgment (D7).** `policy.yaml` carries `trust_required`, `trust_sample`,
   staleness settings, and risk-routing fields — the same shape as
   the donor parity harness's `spec/policy.yaml` (`severity_rigor`, `staleness.
   max_commits_behind`, `staleness.surface_scoped`, `trust_required: {p1:
   human, p2: agent, p3: agent}`, `trust_sample: {p1: 1.0, p2: 0.2, p3:
   0.05}`). Tightening coverage is a policy diff. *Alternative rejected:*
   encoding required-cell logic per corpus family in code — turns a P4-style
   tightening into a corpus-wide retag instead of a one-line reviewed diff.

4. **Staleness: surface-scoped git-diff when refs are declared, else a
   `max_commits_behind` ceiling** (DESIGN A3 pattern / policy.yaml
   `staleness` block). A green record degrades to STALE once the surface's
   own files changed since the record's evidence SHA; the ceiling is the
   hard fallback when no surface ref is declared. *Alternative rejected:*
   a single global "N commits" staleness window — too coarse; an unrelated
   surface's change would falsely stale everything.

5. **Staging→promote with monotonic `run_seq` (O13, the donor parity harness's
   `cmd_promote`).** Reviewers write unordered records under `_staging/`
   (no `run_seq`); `canon gate promote` assigns a monotonic per-(role,
   surface) `run_seq`, re-validates each candidate with the SAME check the
   gate applies BEFORE it lands (a malformed staging record is refused,
   exit non-zero, never committed — mirrors `_run_problems` re-validation),
   then writes the committed file and deletes the staging file. Refusals
   never consume a `run_seq` (no gaps). *Alternative rejected:*
   client-assigned `run_seq` — races under concurrent reviewers, defeating
   the entire point of a serialized integrator step.

6. **canon owns the openspec task/checkbox grammar.** `canon-gate` parses
   and writes `- [ ] ` / `- [x] ` rows directly — including the
   `**DEFERRED to §<to>**` / `**DROPPED**` annotation forms and the
   ` — ✅ <evidence>` suffix — rather than shelling out to or importing
   the donor CLI's task-flip logic. `canon gate task <task_id>` requires a matching
   `EvidenceRecord` (S1 join spine) before flipping; missing/malformed
   evidence fail-closed (§7 "malformed evidence is no evidence" — skip +
   violation, never a silent flip). Fabrication-marker scanning inherits
   `scanFakeMarkers`'s shape (structured evidence fields only, a bare
   `verified` with no attached command result still fails) re-implemented
   against `canon-model`'s `EvidenceRecord` schema, never imported from
   the donor CLI package. *Alternative rejected:* canon calling the donor CLI's
   task-flip helper as a library — couples canon's core to a donor-specific
   package and inverts the stated authority direction (the donor CLI depends on
   canon, never the reverse).

7. **The donor CLI's overlapping helpers are a migration target, not an immediate
   rewrite.** This change ships `canon gate task` as a fully working,
   independently-testable capability; a SEPARATE, donor-CLI-side change swaps
   its task-flip + marker-scan callers to shell out to
   `canon gate task` — the same treatment the donor parity harness gets at S11. This
   change only documents/tasks the boundary. *Rationale:* keeps S5
   file-scoped to canon's own repo; a cross-repo donor-CLI edit is outside
   this change's allowed-edit-root.

8. **Hook seam reuses the existing wiring shape, not the wiring itself.**
   New `.claude/settings.json` / `.codex/hooks.json` entries use the same
   `{matcher, hooks: [{type: "command", command, timeout}]}` shape the donor CLI's
   `hook run <kind>` entries already use. Non-donor-CLI repos get a generic
   `canon-gate-pre-commit.sh` mirroring the donor monorepo's lefthook `pre-commit:` job
   shape (a `run:` line invoking the canon binary; advisory vs blocking
   configurable per repo). *Alternative rejected:* canon shipping its own
   git-hook installer (a lefthook competitor) — out of scope; S5 is the
   gate logic + wiring seam, not a hook-manager product.

9. **Fail loud, stable failure classes (§7).** `canon-gate` ships a
   `FAILURE_CLASSES` constant (Rust `&'static str` set + exported
   JSON-schema enum), grep-stable like the donor parity harness's own: `uncovered-cell`,
   `unreviewed-promotion`, `trust-below-required`, `stale-evidence`,
   `malformed-evidence`, `flagged`, plus openspec-specific additions
   `unevidenced-flip`, `fabricated-evidence`. Never renamed without
   migrating fixtures + hooks together. The GATE fails loud (non-zero exit
   + stable string); the HOOK WRAPPER around it fails soft (an internal
   hook fault allows the action) — the two are deliberately different
   layers per §7's two separate bullets.

10. **`policy.yaml` routing predicates MAY be CEL via S13's `canon-policy`
    crate, once S13 lands; decision 3's D7 discipline is unchanged.**
    Whether a given `trust_required`/`staleness`/risk-routing field is a
    flat value or a CEL predicate over scenario facts, tightening coverage
    stays "a policy diff, never a corpus retag" (design doc §5 S13:
    "S5 policy routing — risk→platform fan-out, severity rigor,
    trust_required, quarantine/staleness exceptions as CEL predicates over
    scenario facts (D7 discipline kept: facts on artifacts, routing in
    policy)"). This change does not itself add CEL evaluation — S13 owns
    `canon-policy`; this is a forward-compatibility pointer only.

## Risks / Trade-offs

- [Risk] Re-implementing the checkbox grammar in Rust duplicates logic that
  already works in TS (the donor CLI) → [Mitigation] golden-fixture parity: the
  selftest corpus covers every row shape the donor CLI's task-flip test suite covers
  (checkbox, `DEFERRED`, `DROPPED`, evidence-suffix stripping), and the
  eventual donor-CLI-side cutover diffs its own suite against canon's CLI output
  before switching.
- [Risk] Serialized `promote` becomes a bottleneck under concurrent
  reviewer agents → [Mitigation] `_staging/` writes stay unordered and
  parallel-safe; only the final `run_seq` assignment is serialized — the
  same design the donor parity harness already runs at the donor consumer repo's review cadence.
- [Risk] Hook-seam wiring drifts from the donor CLI's `hook run <kind>` conventions
  over time (two CLIs editing the same settings.json) → [Mitigation]
  `canon gate install-hooks` is idempotent and diff-only; the companion
  skill instructs agents to run it instead of hand-editing settings.json.
- [Risk] `flagged`'s one-way ratchet requires reliable human attribution —
  an agent could mint a fake "human-attributed" clear-record →
  [Mitigation] clear-records require an attested actor field the gate
  itself validates as never agent-originated; the CLI path for a human
  clear is a separate, interactively-confirmed flag, not a field an agent
  process can set.

## Migration Plan

- Step 1: `canon-gate` ships standalone with its own fixture corpus — zero
  consumer-repo changes required for it to exist.
- Step 2: the donor monorepo opts in by adding hook-seam entries (this change's task) —
  additive, does not remove the donor CLI's existing `hook run <kind>` entries.
- Step 3 (future, out of this change): a donor-CLI-side change swaps
  its task-flip callers to `canon gate task`; the donor CLI's own test suite is the
  acceptance bar for that cutover.
- Rollback: hook-seam entries are additive JSON; removing the
  `canon gate task` command line from settings.json fully reverts to
  the donor CLI's existing enforcement with no data loss — canon's evidence records
  live in canon's own git-tier ledger, independent of the donor CLI's task-flip
  annotations.

## Open Questions

- The exact openspec-specific `FAILURE_CLASSES` additions
  (`unevidenced-flip`, `fabricated-evidence`) are provisional pending S1's
  `EvidenceRecord` schema landing — finalized once `canon-gate` can depend
  on the real crate instead of a stub.
- Whether `canon gate promote`'s run_seq partition key is `(role, surface)`
  (canon's S6 role-namespacing) or `(lane, surface)` (the donor parity harness's original)
  for non-donor consumers — decided when S11 wires the donor consumer repo's
  actual corpus through `canon-gate`.
