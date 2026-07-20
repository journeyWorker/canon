# s30 plan-dialect-superpowers — design

Implements s17 D9's named follow-up. The s17 foundation (PlanAdapter
trait, registry, CLI driver, SourceCursor, per-record lenient
persistence, D8 cross-source collision handling) is REUSED untouched;
every decision below is adapter-local.

## D1 — The `writing-plans` skill is the format authority

The grammar this adapter pins is the one the superpowers
`writing-plans` skill instructs authors to produce (the exact
deferral condition s17 D9 set — "no format authority yet" — now
resolved by citing that skill as the authority):

- Location: `docs/superpowers/plans/YYYY-MM-DD-<feature-name>.md`.
- `# <Feature Name> Implementation Plan` H1 (first H1 in the doc).
- `**Goal:** <one sentence>` header line.
- `### Task N: <Component Name>` sections; `N` is the stable task
  number (matches `openspec_rows::is_task_number` — plain or
  hierarchical digits).
- `- [ ]` / `- [x]` checkbox STEPS inside each task section (the
  skill's `**Step N:**` bolding is NOT load-bearing: any checkbox
  line inside the section counts, so hand-edited plans that drop the
  bold survive).

Grammar drift lands as a reviewed adapter update (same posture as
s17 R4 for the openspec CLI).

## D2 — Identity: slugified filename stem, verbatim otherwise

`change_id` = the filename stem (`.md` dropped), lowercased, with
each `[^a-z0-9]+` run collapsed to one `-` and edge `-` trimmed —
then validated through `ChangeId::parse` (the S1 grammar stays the
single authority; a stem that slugs to an invalid/empty id is
malformed, skipped + named). The date prefix is KEPT verbatim
(`2026-07-14-website-design`) — same "identity is the basename, no
date-prefix stripping" rule as s17 D4, and the natural collision
guard (two plans differing only by date import as distinct changes).
The H1 title is display prose, never identity.

## D3 — Tasks: heading number is the id, checkbox steps are the state

- `### Task N: <name>` → `Task` with `task_id` derived through the
  SAME shared `openspec_rows::task_id_for(change_id, N)` used by the
  openspec plan adapter AND the S4 verdict adapter — one join-key
  derivation for every reader (s17 D5/R5). A heading whose `N` fails
  `is_task_number` is malformed (named, skipped); the section's
  checkboxes then belong to NO task and are ignored.
- `TaskStatus::Done` iff the section contains ≥1 checkbox line and
  ALL are checked; `Open` otherwise (zero-checkbox sections are Open
  — a task not yet broken into steps is not done). Duplicate `Task
  N` headings in one doc: first wins, later ones malformed-named
  (mirrors D8's "never silently merge").
- `title` = `<name>` text after the colon, whitespace-normalized.

## D4 — Change summary and status

- `summary` = the `**Goal:**` line's remainder (whitespace-
  normalized). Absent Goal line → empty summary + named unmapped
  diagnostic `goal-missing` (never invented prose — mirrors
  `DIAG_PROPOSAL_MISSING_WHY`).
- Status via the openspec dialect's D6 rule minus the archive arm
  (superpowers has no archive convention): all tasks done and ≥1
  task → completed; else in-progress/draft exactly as
  `derive_status(false, done, open)` already derives. Factor or
  mirror — implementer's choice, but the tally semantics must be
  byte-identical.
- `at` = file mtime (`file_modified_at` convention, s17 D7/R3
  accepted trade-off).

## D5 — Discovery and shape tolerance

Mirrors the openspec adapter's posture: the configured root may be
(a) a repo root containing `docs/superpowers/plans/`, (b) the plans
dir itself, or (c) a fixture dir holding bare `*.md` plans.
Immediate-children `*.md` files only (the skill's flat layout;
subdirectories are not part of the authored grammar), byte-lexically
sorted for deterministic enumeration. Absent/unreadable root → zero
records, never an error. Non-plan siblings that are still markdown
(e.g. a stray README.md) import only if they carry ≥1 `### Task N:`
heading OR a Goal line; otherwise named unmapped `not-a-plan-doc`
and skipped — a docs dir is a plausible misconfiguration and must
degrade loud-but-soft, not import garbage Changes.

## D6 — Unmapped constructs are named, never guessed

Steps, `**Architecture:**`/`**Tech Stack:**` header prose, Global
Constraints, and non-task headings are the plan's HOW — no core
record exists for them (same posture as s17 D3's design-doc/
spec-delta drops). They are simply not read; only `goal-missing`
and `not-a-plan-doc` (D4/D5) plus the malformed vocabulary
(`invalid-change-id-slug`, `invalid-task-number`,
`duplicate-task-number`, `unreadable-file`) surface as diagnostics.
A construct-per-drop diagnostic for every step line would be noise,
not signal — steps are EXPECTED to be unmapped.

## Non-goals

- `donor-json` (deferral condition unresolved).
- Importing step-level state, plan header metadata beyond Goal, or
  `docs/superpowers/specs/` (specs are brainstorming output, not a
  plan corpus; nothing maps them to `Change`/`Task` without
  invention).
- Any canon-gate/canon-learn awareness of the new dialect (s17 R1's
  authority boundary; the acceptance pin stays green untouched).
