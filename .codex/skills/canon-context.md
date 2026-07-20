# canon-context

> How to run canon context --repo/--json — the S12 (canon-context) authoring-surface capability query that resolves a repo's record kinds + envelope fields, enum domains, join-key grammars, partition layout, policy-derived requirements, the S10 typed-vocabulary index, and the S13 per-kind CEL binding surface into ONE deterministic AuthoringSurface, rendered as a compact outline (default) or machine-readable JSON. Use before authoring any canon artifact, when wiring an agent's pre-authoring context, or when decoding a canon fmt --check "expected one of" enum error.

# canon-context

S12 (`s12-canon-context`) ships `canon context` — a capability QUERY over
the SAME schema/policy/vocabulary registry `canon fmt`/`canon gate`
validate against, never validation itself. It answers "what CAN I author
in this repo, and how?" — record kinds + their envelope fields, enum
domains, join-key grammars, partition layout, policy-derived evidence
requirements, the typed authoring vocabulary (S10), and the CEL binding
surface (S13) — folded into one deterministic `AuthoringSurface`
(`crates/canon-cli/src/context.rs`).

## `canon context [--repo <dir>] [--json]`

```bash
canon context                 # compact human outline (prompt-injectable)
canon context --json          # the full machine-readable AuthoringSurface
canon context --repo ../other # resolve a specific repo's surface
```

- **A capability query, never validation (invariant 1):** `canon context`
  ALWAYS exits `0` with the full surface — even when `canon fmt --check`
  or `canon gate` would report diagnostics against the same repo. It reads
  only the schema/policy/vocabulary registry, never the corpus or the
  evidence ledger. (Proven by
  `context_exits_zero_with_a_full_surface_even_when_the_corpus_fails_fmt_check`.)
- `--repo` resolves through the same nearest-`canon.yaml`-ancestor walk
  `canon fmt`/`canon gate`/`canon retrieve` use (design D7): omitted or
  `.` walks up from cwd to the project root; any other explicit dir is
  used as-is.
- `--json` renders `serde_json::to_string_pretty` of the surface; the
  default renders a compact per-section outline. Both project from the
  IDENTICAL `resolve_surface` output (byte-stable across runs — every map
  is a `BTreeMap`).

## The surface sections

- **`kinds`** — each record kind's `{schema_version, envelope_fields,
  partition}` (from the S11 layout-descriptor registry).
- **`enums`** — each enum domain's members (verdicts, statuses, lanes,
  roles, polarity, …), sourced from the SAME schema registry
  `canon fmt`'s validator uses.
- **`joinKeys`** — the S1 join-spine grammar string per key
  (`regime_key`, `session_id`, `task_id`, …).
- **`policy`** — the resolved `PolicyResolution` for the repo (S5's
  `policy.yaml`-derived evidence/trust requirements per kind).
- **`vocab`** — the S10 typed authoring vocabulary (directive/enum/
  evidence-kind index) + its content-hash `capability_version`.
- **`cel`** — the S13 per-kind CEL binding surface: the bindable
  `record.<field>` names + their types and the callable pure-function
  allowlist a `policy.yaml` `applies_when:`/predicate may reference, from
  `canon_policy::bindings_for`. This is the agent-facing answer to "what
  can a CEL predicate for this kind reference?"

## Resolution is single-source (design D5, invariants 1/2)

`resolve_surface(repo, opts)` calls exactly three shared resolution
functions and nothing else: `canon_policy::SchemaRegistry::load()`
(schema), `canon_gate::PolicyResolution::resolve()` (policy.yaml), and
`canon_vocab::resolve_snapshot()` (typed vocabulary). It NEVER
re-derives a schema/policy/enum list of its own — so a schema edit
propagates to `canon context`'s output AND `canon fmt`'s diagnostics from
ONE edit site (the canon-model schema), never two.

## Reading an "expected one of: …" validator error

When `canon fmt --check` rejects an out-of-domain enum value it emits the
mandated shape:

```
`not-a-real-kind` is not a valid value for `kind` of `run` (expected one of: run, review, divergence, …)
```

The `expected one of:` member list is the SAME enum domain `canon context
--json`'s `enums.<field>` reports for that kind — so to fix the artifact,
pick a member `canon context` lists. (The diagnostic reads the member
list straight off the validated schema's own resolved enum; `canon
context` reads it from the same `SchemaRegistry` via `enum_domain` — one
source of truth, no hand-maintained second copy.)

## What this skill does NOT cover

- `canon fmt --check`'s full violation taxonomy — see the
  `format-authority` skill; this skill only covers how to read its enum
  diagnostic against the context surface.
- Authoring a typed task atom / vocabulary-defined handoff body — see the
  `typed-authoring-vocabulary` material; `canon context`'s `vocab`/`cel`
  sections tell you what's declarable, not how to declare it.
- `policy.yaml` CEL authoring semantics — see the `canon-policy` skill;
  this skill only surfaces the bindable identifier/function set per kind.
