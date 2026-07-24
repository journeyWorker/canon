---
name: canon-plan-import
description: How to configure a canon.yaml `plans:` source (or `--dialect`/`--source`), run `canon ingest plans` to import a foreign planning dialect (openspec change dirs; superpowers `writing-plans`-shaped plan docs) into canon's Change/Task records, and read them via `canon query --kind change`/`--kind task`. Use when importing a foreign plan corpus into canon, debugging a `canon ingest plans` failure, or reading imported change/task rows.
---

# canon-plan-import

`canon ingest plans` imports a FOREIGN plan dialect's on-disk state into
canon's `change`/`task` records, through the same validated path every
other ingest family uses. Imported rows carry no special marker — once
imported, `canon query` cannot tell an imported row from a hand-authored
one, and neither can `canon gate`.

## 1. Configure a `plans:` source

`canon.yaml`:

```yaml
plans:
  sources:
    - dialect: openspec
      root: .              # resolved relative to canon.yaml's directory
```

- An absent `plans:` section = zero sources (a clean no-op).
- A present section parses STRICTLY: a typo'd key, an unregistered
  `dialect`, or a nonexistent `root` fails the command loud, naming the
  offender.
- `root` is whatever the dialect's adapter scans — for `openspec`, a
  repo root containing `openspec/changes/`, or that changes dir directly.

One-shot override (bypasses `canon.yaml` for a single ad hoc import;
either flag alone fails loud):

```bash
canon ingest plans --dialect openspec --source ../other-repo
```

## 2. Run the import

```bash
canon ingest plans                 # canon.yaml-driven, every configured source
canon ingest plans --json          # machine-readable outcome
canon ingest plans --repo ../svc   # a specific repo root
```

Per source the summary reports `changes_parsed`/`tasks_parsed`,
`changes_persisted`/`tasks_persisted`, `changes_unwritten`/
`tasks_unwritten` (an unreachable tier — printed, never silently
dropped), `duplicate_change_id` (two sources producing the same
`change_id` this pass — the LATER is skipped and counted, never merged),
the adapter's `unmapped` drop counts, and `malformed` (structurally
broken constructs). A source whose content is byte-identical to its last
successful run is `skipped_unchanged` — re-importing writes zero records.

## 3. Read the imported rows

```bash
canon query --kind change [--json]
canon query --kind task [--json]
```

No dialect/plugin filter exists or is needed — an imported `change`/
`task` is an ordinary core record.

## The `openspec` dialect

Maps `openspec/changes/**` onto core kinds:

| openspec construct | Core mapping |
| --- | --- |
| change dir's basename | `change_id` (verbatim — no slug massaging) |
| `proposal.md`'s `## Why` first paragraph | `Change.summary` (absent heading → empty summary + a `proposal-missing-why` diagnostic) |
| archive location vs. checkbox tallies | `Change.status` (`archived` wins; else zero rows → `proposed`, all done → `completed`, mixed → `in_progress`) |
| each `tasks.md` checkbox row | `Task` (`task_id = <change_id>#<n>`, status verbatim, evidence_note from a ` — ✅ ` suffix or a `**DEFERRED**`/`**DROPPED**` annotation) |
| `specs/**/spec.md` `#### Scenario:` blocks | dropped, counted under `spec-delta-scenario` |
| `design.md` | dropped, counted under `design-doc` |

A missing/unreadable `proposal.md`, or a basename that isn't a valid
`change_id`, skips the WHOLE dir (counted `malformed`); siblings
unaffected. A proposal-only dir imports as `Change { status: proposed }`
with zero tasks and zero diagnostics.

## The `superpowers` dialect

Maps one plan document (an `*.md` immediate child of the plans root),
following the superpowers `writing-plans` skill's grammar:

| `writing-plans` construct | Core mapping |
| --- | --- |
| the filename stem, slugified (lowercased, each `[^a-z0-9]+` run → one `-`, edge `-` trimmed; date prefix kept verbatim) | `change_id` |
| the `**Goal:**` line's remainder, whitespace-normalized | `Change.summary` (absent → empty summary + a `goal-missing` diagnostic) |
| checkbox tallies across every `### Task N:` section | `Change.status` (zero tasks → `proposed`, all done → `completed`, mixed → `in_progress`) |
| each `### Task N: <name>` section | `Task` (`task_id = <change_id>#<N>`, title = text after the colon; status `done` iff the section has ≥1 checkbox line and ALL are checked, else `open`) |
| steps, `**Architecture:**`/`**Tech Stack:**` prose, Global Constraints, non-task headings | never read, never a diagnostic |

Worked example — `2026-07-14-website-design.md`:

```markdown
# Website Implementation Plan

**Goal:** Build the project website.

### Task 1: Layout

- [x] **Step 1: scaffold the grid**
- [x] **Step 2: wire the nav**

### Task 2: Copy

- [x] **Step 1: draft the homepage**
- [ ] **Step 2: proofread**
```

imports as `Change { change_id: "2026-07-14-website-design", summary:
"Build the project website.", status: in_progress }`, plus
`Task { task_id: "…#1", title: "Layout", status: done }` and
`Task { task_id: "…#2", title: "Copy", status: open }`. The `**Step N:**`
bolding is NOT load-bearing — a plain `- [ ]`/`- [x]` row still counts.

An invalid task-number heading is skipped and named `malformed`; a
duplicate `Task N` heading keeps the first section and names the later one
`malformed`. A filename stem that fails to slug into a valid `change_id`
is malformed too. A markdown file with neither a `**Goal:**` line nor any
`### Task N:` heading (a stray `README.md`) is skipped with a
`not-a-plan-doc` diagnostic, not imported as garbage. Only immediate-child
`*.md` files are read, byte-lexically sorted; an absent/unreadable root
yields zero records, never an error.

## The task_id join

An imported `Task.task_id` is byte-identical to the join key of the
openspec-task verdict events canon's artifact ingest derives from the same
checkbox tree — so `canon report`'s trust matrix joins the two sides on
`task_id` for its coverage columns, and `canon ingest plans` is that
join's bulk producer. `canon ingest plans` itself performs no join and
writes no verdict/evidence — joining plan state against evidence is a
downstream reader's job.

## What this skill does NOT cover

- **Authoring an openspec change dir** (`proposal.md`/`tasks.md`
  conventions) — see the openspec CLI's own docs; this skill covers
  IMPORTING that shape, not producing it.
- **`canon gate check`/`canon gate task`** (the hand-authored one-task-at-
  a-time path this complements at bulk) — see `canon-gate`.
- **Wiring an imported `Task`/`Change` into a `canon gate` decision** —
  out of scope; needs its own reviewed change.
