# s36 — subject-domain-loop

## Why

Canon manages *changes* (imported plan units) and *scenarios* (behavior
ledger rows), but has no first-class notion of the durable product unit
a team actually plans, designs, builds, and measures across many
changes. Three consequences:

1. Planning docs ingested via `canon ingest plans` become `Change`/`Task`
   rows and stop there — nothing lifts them into a managed product
   surface.
2. There is no per-domain (planning / design / dev / data / test)
   management view: which units exist, who owns them, what state each is
   in, what evidence backs it.
3. The knowledge flywheel (S6-S8) keys regimes by
   `<role>/<repo>/<area>/<hash>`, but `area` is ad hoc — agent knowledge
   never accumulates against the product unit the work was actually for.
   The loop should START at a subject write: an agent's first act on any
   substantial work is authoring/updating the Subject record, and every
   downstream artifact (scenarios, tasks, reviews, trajectories,
   strategies) joins back to it.

### Naming: `Subject`, deliberately NOT "feature"

Gherkin `.feature` files are canon's behavior-spec corpus — when an
agent is told "read the feature docs", that MUST mean the Gherkin files,
unambiguously. The management unit is therefore named **Subject**, after
the musical canon's own structure: one subject, taken up by many voices
(roles), each developing it in its own register. A Subject is the
product unit; its `.feature` files are its behavior specs; they are
never the same word.

## What Changes

- **`RecordKind::Subject` — the reviewed 13th kind** (design D1's
  process: a new kind is a breaking, reviewed canon-model change):
  - `subject_id` (kebab slug, new join-spine key), `title`, `summary`
  - `domain` — vocabulary-defined enum (`canon/vocab` plugin declares
    the base set: `planning`, `design`, `dev`, `data`, `test`; consumer
    repos extend via their own vocab plugin — same S10 mechanism as
    handoff domains)
  - `status` lifecycle: `proposed → specced → building → verifying →
    shipped → retired` (policy-gated transitions, CEL)
  - `owner_role` (RoleId), envelope actor/provenance as usual
  - links: `change_ids: [ChangeId]`, `scenario_ids: [ScenarioId]`
- **Join spine extension:** `subject_id` becomes a spine key.
  `Change.subject_id: Option<SubjectId>` (set at adoption); Gherkin
  scenarios join via a `@subject:<id>` tag mapped by `canon inventory
  sync`; `regime_key`'s `<area>` segment gains a canonical hierarchical
  form `<domain>/<subject_id>` so strategy memory accumulates per
  subject AND falls back per domain.
- **Retrieval fallback hierarchy (diary-informed):** `canon retrieve`
  resolves guidance at `…/<domain>/<subject_id>/…` first, then falls
  back to `…/<domain>/…`, then the repo level — a subject is finite, so
  its lessons must outlive it: on `shipped`/`retired`, a consolidation
  pass promotes still-valid subject-scoped strategies to the domain
  level (the L3→L5 promotion analog, executed by an agent skill through
  the CLI, never an LLM call inside canon).
- **Deterministic CLI surface:**
  - `canon subject new <id> --domain <d> --title <t>` — authoring
    (envelope + policy validated at write, like every other record)
  - `canon subject adopt <change_id> --subject <id>` — links an imported
    plan change to a subject (planning docs → ingest plans → adopt; or
    `--derive` to stub a Subject straight from a Change's `## Why`)
  - `canon subject status <id> <state>` — policy-gated transition;
    `verifying → shipped` requires covered/green evidence via the
    existing gate (`canon gate check` scoped to the subject's scenarios)
  - `canon query --kind subject [--domain <d>] [--status <s>]` — the
    per-domain management view; `canon report` gains a subject panel
    (per-domain rollup: subjects × status × evidence coverage)
- **Subject-write-first agent loop (LLM skill + deterministic tools):**
  a new `canon-subject` skill (materialized like the others) fixes the
  contract for every domain agent:
  1. `canon context` → author or update the Subject record FIRST
  2. `canon retrieve --role <r> --regime <r>/<repo>/<domain>/<id>/…`
     before starting (knowledge in, with fallback)
  3. work → scenarios/tasks/reviews all pinned to `subject_id`
  4. verdicts/trajectories ingest with the subject-scoped regime, so
     `canon learn` distills strategies per subject (knowledge out)
  The skill also fixes the vocabulary rule: "feature docs" = Gherkin
  `.feature` files; "subject" = the management record.

## Impact

- Affected: `canon-model` (13th kind + SubjectId + schema export +
  JOIN_SPINE.md), `canon-store` (routing/aging for `subject`),
  `canon-cli` (subject subcommands, query/report surface), `canon-fmt`
  (subject record family check), `canon-vocab` (domain enum plugin),
  `canon-learn` (hierarchical regime fallback in retrieve), skills (+1
  new, several updated), website docs.
- Depends on: s35 (gate seam) only for the `status` gate wiring; the
  model/CLI work is independent.
- Non-goals: no workflow engine, no assignment/scheduling — canon
  records state + evidence; orchestration stays with the harness.
  Salience decay / vector retrieval stay deferred behind the existing
  store-trait seam (a later change).
