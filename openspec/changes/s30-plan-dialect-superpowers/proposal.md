## Why

s17 (`plan-import`) shipped the plan-import pillar with ONE dialect —
openspec — and explicitly deferred the second one to a named
follow-up: "`superpowers` (named follow-up:
`plan-dialect-superpowers`): a plan doc maps `# <title>` → `Change`
… `### Task N: <name>` sections → `Task` … Deferred because the
checklist grammar has no format authority yet — pinning it here
would freeze a speculative shape; the adapter is one registry entry
when it lands" (s17 design.md D9, proposal.md:160-165).

Both deferral conditions have since resolved:

1. **The grammar now has an authority.** The superpowers
   `writing-plans` skill pins the plan-document shape every plan in a
   superpowers-disciplined repo (this one included) is authored
   against: `docs/superpowers/plans/YYYY-MM-DD-<feature-name>.md`,
   a `# <Feature Name> Implementation Plan` H1, a one-sentence
   `**Goal:**` header line, `### Task N: <Component Name>` sections,
   and `- [ ] **Step N: …**` checkbox steps inside each task. That
   is a citable, versioned grammar — not a speculative one.
2. **The seam was built for exactly this.** s17 proved (D9, via its
   fixture second-dialect test) that a new dialect is one
   `PlanAdapterEntry` line plus one adapter module — no re-plumb of
   the registry, driver, cursor, or CLI. The operator asked for the
   dialect (2026-07-14); this change is that one entry + module.

Without it, a superpowers-disciplined repo's plan corpus (goal,
task decomposition, completion state) is invisible to `canon query
--kind change/task` and to every downstream join (S4 verdicts,
`mart_trust_matrix`) that imported openspec plans already enjoy.

## What Changes

- **New `superpowers` plan dialect adapter**
  (`crates/canon-ingest/src/plan_adapters/superpowers.rs`),
  registered as one `PlanAdapterEntry` in `plan_registry` — D9's
  sketched mapping, now pinned against the `writing-plans` grammar:
  - one plan document (`*.md` under the configured root) → one
    `Change`: `change_id` = slugified filename stem, `summary` = the
    `**Goal:**` line's text, status derived from task checkbox
    tallies;
  - each `### Task N: <name>` section → one `Task`: `task_id` =
    `<change_id>#<N>` through the SAME shared
    `openspec_rows::task_id_for` join-key derivation (design R5's
    one-join discipline extends to every dialect), done when the
    section's checkbox steps are all checked (and at least one
    exists).
- **Fail-soft per construct, loud per name** (s17 D3 / s18): a
  malformed plan file or task heading is skipped AND named
  (`MalformedEntry` path+reason); unmappable prose (steps,
  architecture blocks) is never guessed onto records.
- **`canon ingest plans` gains the dialect for free** — config
  (`plans.sources[].dialect: superpowers`) and `--dialect
  superpowers --source <root>` both resolve through the existing
  registry lookup; a CLI integration test proves the end-to-end
  import against a fixture plan doc.
- **`canon-plan-import` skill updated**: superpowers moves from the
  "deferred" section to a shipped-dialect section (grammar, worked
  example, task_id join note); re-materialized via `canon skills
  install`.

## Impact

- Affected specs: `plan-dialect-superpowers` (new capability delta).
- Affected code: `crates/canon-ingest/src/plan_adapters/{mod,superpowers}.rs`,
  `plan_registry.rs`, doc-comment updates where s17 named the dialect
  deferred (`plan_adapter.rs`), `crates/canon-cli/tests/plans_ingest.rs`,
  `canon/skills/canon-plan-import/SKILL.md` (+ materialized copies).
- Non-goals: `donor-json` (its deferral condition — a concrete donor
  plan corpus — has NOT resolved); any authority creep (imported rows
  stay ordinary `Change`/`Task` records, s17 R1's acceptance pin
  still holds).
