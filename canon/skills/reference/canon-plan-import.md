# canon-plan-import

`canon ingest plans` imports plan state into canon's `change`/`task`
records, through the same validated path every other ingest family uses:
canon's own changes under `.canon/changes/`, and any FOREIGN plan dialect
listed in `canon.yaml`. Imported rows carry no special marker — once
imported, `canon query` cannot tell an imported row from a hand-authored
one, and neither can `canon gate`.

## 1. Canon's own changes and external `plans:` sources

`.canon/changes/` is canon's native plan source (dialect id `canon`):
one `<slug>/` directory per change holding `brief.md` (and a `tasks.md`
only when someone adds one: an optional work note), written by
`canon change new` (see `canon-subject`). Every plan reader —
`canon ingest plans`, `canon task done`, `canon evidence add --task`,
`canon dispatch begin --task`, `canon status` and the brief check — reads it
first whenever the directory exists, with no `plans:` entry. Being first,
it owns a change id an external source also carries (the external copy
is the later duplicate, skipped and counted under `duplicate_change_id`).
Its records carry the actor `canon-plan-import-canon`; an external
openspec change's carry `canon-plan-import-openspec`. A `brief.md` with a
`Source: <path>` line is not a change of its own: it is the canon brief
of a change imported from an external plan (it points at the external
document and holds the human's `Approved:` line, written by
`canon brief approve`), so the native reader
skips that directory and the external source keeps the change id.

`plans.sources` lists only EXTERNAL plans — documents you prepare outside
canon, which canon only reads. The one exception: `canon task done`
flips the task's checkbox in an external plan's `tasks.md` (see
`canon-gate`). Their briefs and approvals live in
`.canon/changes/<change>/brief.md`, never in the external document.
`canon.yaml`:

```yaml
plans:
  sources:
    - dialect: openspec
      root: .              # resolved relative to canon.yaml's directory
```

- `canon init` writes `sources: []`, preceded by comments explaining
  `.canon/changes/` and commented examples for `openspec` (`root: .`) and
  `superpowers` (`root: docs/superpowers/plans`). It creates no
  `openspec/changes/` directory.
- An absent `plans:` section = zero external sources (a clean no-op;
  `.canon/changes/` is still read).
- A present section parses STRICTLY: a typo'd key, an unregistered
  `dialect`, or a nonexistent `root` fails the command loud, naming the
  offender. `canon` is not a registered dialect: a source naming it is
  refused, since `.canon/changes/` needs no entry.
- `root` is whatever the dialect's adapter scans — for `openspec`, a
  repo root containing `openspec/changes/`, or that changes dir directly.

One-shot override (bypasses `canon.yaml` and `.canon/changes/` for a
single ad hoc import, reading only the source it names; either flag alone
fails loud):

```bash
canon ingest plans --dialect openspec --plan-source ../other-repo
```

## 2. Run the import

```bash
canon ingest plans                 # .canon/changes/ + every configured source
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

**Done when:** `canon ingest plans` exits `0` with `malformed` and
`changes_unwritten` at `0` for every source; step 3 lists the imported rows.

## 3. Read the imported rows

```bash
canon query --kind change --json
canon query --kind task --json
```

No dialect/plugin filter exists or is needed — an imported `change`/
`task` is an ordinary core record.

## The `canon` dialect (`.canon/changes/`)

Uses the `openspec` change-dir grammar below, with three differences:
the brief is `brief.md` (the first paragraph of its intent section, or of
`## Why` in a pre-0.19 brief, is the summary; a
missing one is a `missing-brief-md` malformed dir), the source root is
always `.canon/changes/` itself (archived changes under
`.canon/changes/archive/`), and a `brief.md` carrying a `Source:` line is
skipped (a pointer brief, above). An existing `tasks.md` uses the same
checkbox row grammar; a change with none imports with zero tasks.

## The `openspec` dialect

Maps an external `openspec/changes/**` onto core kinds:

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
with zero tasks and zero diagnostics. An `Approved:` line in an external
`proposal.md` is not read: the change's approval lives in
`.canon/changes/<change>/brief.md` (see `canon-subject`).

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

- **Authoring a change** — `canon change new` writes
  `.canon/changes/<slug>/brief.md` (no `tasks.md`) and adopts the change
  into a subject (see `canon-subject`); it never scaffolds an openspec
  change dir. This skill covers IMPORTING plans.
- **`canon gate check`/`canon task done`** (the hand-authored one-task-at-
  a-time path this complements at bulk) — see `canon-gate`.
- **Wiring an imported `Task`/`Change` into a `canon gate` decision** —
  out of scope; needs its own reviewed change.