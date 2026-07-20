## Why

An agent authoring a canon artifact (a ledger record, a divergence review, an
inventory entry, a strategy item) needs to know the current vocabulary —
which record kinds exist, which envelope fields are required, which enum
values are valid, which join-key grammars apply, which partition layout to
write into, which policy-derived requirements apply — **before** it writes
anything, or every failed attempt is a blind guess corrected only after
`canon fmt`/`canon gate` rejects it. The donor CLI's context command already solved this for
the donor vocabulary project's own authoring surface; canon needs the same command, generalized
across its whole artifact family.

## What Changes

- New CLI command `canon context [--repo <dir>] [--json]` emitting the
  project-resolved **authoring surface**: record kinds + envelope fields,
  enum domains (verdicts, statuses, lanes, roles), join-key grammars,
  partition layout, policy-derived requirements, and a capability version.
- `--json` emits a machine-readable, deterministic document; the default
  (no flag) emits a compact human outline for prompt injection — both built
  from the identical resolved surface, never two independent renderers.
- Three invariants, lifted from the donor CLI's context command (the donor CLI's entry point
  `run_context`/`authoring_surface`/`context_outline`), reproduced below.
- Validator error messages (canon-gate, canon-model schema validation) embed
  `"expected one of: …"` domains pulled from the **same** schema/policy
  registry `canon context` reads — never a second, independently-maintained
  list of valid values.
- Companion skill instructs agents to run `canon context` before authoring
  any artifact (design §5 S12, last sentence).

### The three donor invariants (design §5 S12, verbatim)

1. A capability **query, not validation** — `canon context` emits even when
   the corpus has diagnostics (a broken corpus still has a valid authoring
   surface to describe).
2. Built from the **SAME schema registry + policy resolution** the validator
   (`canon fmt`/`canon gate`) uses, so the surface can never diverge from
   what those commands actually enforce.
3. **Deterministic output** (sorted maps, stable ordering) — `--json` for
   tools, compact human outline for prompt injection; both agree because
   both are rendered from the one resolved surface.

## Capabilities

### New Capabilities

- `context-authoring-surface`: the `canon context [--repo][--json]` command,
  its three donor-derived invariants, byte-stable/deterministic output, and
  the "expected one of: …" validator-error contract sharing the same schema
  registry.

### Modified Capabilities

_None — canon has no existing specs yet; S12 lands alongside S3/S4/S11 in
the W1 wave._

## Impact

- New CLI surface `canon context [--repo <dir>] [--json]` on `canon-cli`.
- Reads (never mutates) the schema/policy registry canon-model (S1) and
  canon-gate's policy resolution (S5) own — `canon context` is a pure
  capability query over that registry, not a new source of truth.
- Reads canon-model's schema validation error path to add the "expected one
  of: …" domain-listing format to every enum-mismatch diagnostic.
- New fixture: a fixture repo whose `canon context` output is checked in and
  byte-diffed for stability across runs and across `--json`/outline modes.
- Companion skill (design §5 cross-cutting deliverable, decision 9): a
  `canon context` authoring-surface skill under `canon/skills/`, instructing
  agents to run it before authoring any artifact.
