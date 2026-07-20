# typed-authoring-vocabulary

> How to author against canon's typed vocabulary (S10) — declare a new directive or enum in a canon/vocab/<id> plugin, author a typed task atom with a policy-resolved evidence requirement, author a vocabulary-defined handoff body, and when to run canon context first. Use when adding a directive/enum, writing a `::task` or `::handoff-<domain>` atom, or wiring a consumer repo's vocabulary plugin.

# typed-authoring-vocabulary

S10 (`s10-typed-authoring-vocabulary`) is canon's typed authoring layer:
a donor authoring-vocabulary project's plugin/manifest/checker architecture retargeted at canon's task
atom + handoff template domain (`crates/canon-vocab`). It replaces a
freeform `tasks.md` checkbox's implicit evidence with a structured,
checker-validated, policy-resolved attribute — and gives handoff bodies a
declared, rendered template. The single resolution entry point is
`canon_vocab::resolve_snapshot(project_dir, profile)`; the checker
(`check_directive`) and S12's `canon context` both consume its output —
never a second, hand-maintained vocabulary view.

## Run `canon context` FIRST (S12 tie-in)

Before authoring any atom, run `canon context` (or `--json`) and read its
`vocab` section: it lists every declarable directive + enum for the repo
(the resolved `CapabilitySnapshot`) plus the `capabilityVersion`
content-hash. Authoring against a directive/enum `canon context` does not
list means the checker will reject it — the surface is the source of
truth for "what can I declare here?"

## Declaring a directive or enum

A vocabulary plugin lives at `canon/vocab/<id>/`:

```yaml
# canon/vocab/<id>/plugin.yaml
id: my.plugin
version: "0.1.0"
kind: project           # `core` for canon.core; `project` for a consumer plugin
exports:
  directives: directives/   # a dir of *.yaml directive files
  enums: enums.yaml
```

```yaml
# canon/vocab/<id>/directives/thing.yaml
directives:
  - name: thing
    attrs:
      - name: desc
        type: string           # scalar
        required: true
      - name: status
        type: { domain: task-status }   # value must be a member of the enum
        required: true
      - name: tags
        type: { list: string }          # a YAML list of that element type
        required: false
      - name: evidence
        type: evidence           # SPECIAL: {kind, ref}; `kind`'s domain
        required: true           # resolves LIVE from canon/policy.yaml (S5),
                                 # never a locally-declared enum (design D4)
```

```yaml
# canon/vocab/<id>/enums.yaml
enums:
  task-status: [open, done]
```

- **Type kinds**: `string`; `{ domain: <enum> }` (validated against
  `enums.yaml`); `{ list: <type> }`; and `evidence` (the `{kind, ref}`
  shape whose `kind` domain comes from `canon/policy.yaml`'s evidence
  kinds via `canon_vocab::policy_bridge`, NOT a local enum).
- The evidence-kind domain is DELIBERATELY absent from `enums.yaml` — do
  not add it there; it resolves dynamically from policy.

## Authoring a typed task atom

An atom file is a YAML list of `{ id, tag, attrs }` records. A task atom
tags `task` and must satisfy the `task` directive's attrs:

```yaml
- id: my-change#12.3            # <change_id>#<n> (the task_id join key)
  tag: task
  attrs:
    desc: "wire the checker against the real manifest"
    owner: alice                # optional
    status: open                # must be a task-status enum member
    evidence:
      kind: test-run            # must be a policy.yaml evidence kind
      ref: "cargo test -p canon-vocab"
```

`canon_vocab::compile_task(atom, snapshot, envelope)` validates it
against the resolved snapshot and compiles it to a `canon_model::Task`
(S1); `decompile_task` round-trips it back. A wrong tag, an unknown attr,
a missing required attr, an out-of-domain `status`, or an evidence `kind`
absent from `policy.yaml` each produces a checker `Diagnostic` (the
"expected one of: …" shape) — the atom is rejected at author time, never
compiled into a malformed Task.

## Authoring a vocabulary-defined handoff body

A handoff body tags `handoff-<domain>` (the domain selects the directive,
NOT an attr value) and satisfies that directive's attrs:

```yaml
- id: my-handoff-1
  tag: handoff-dev              # dev | design | content | test (canon.core)
  attrs:
    title: "wire canon-vocab into canon-cli"
    summary: "the foundation crate is ready for CLI integration"
    verification-steps:
      - "cargo build -p canon-vocab"
      - "cargo test -p canon-vocab"
```

`canon_vocab::compile_handoff_body` validates + compiles it to a
`canon_model::HandoffBody` (S1); `render_handoff_body` renders it. Each
`handoff-<domain>` directive declares its own attrs (e.g. `design` /
`content` domains may require `acceptance-criteria` instead of
`verification-steps`) — read the domain's directive (or `canon context`)
for the exact required set.

## What this skill does NOT cover

- The `canon context` surface itself — see the `canon-context` skill;
  this skill covers authoring AGAINST it.
- `policy.yaml` / CEL evidence-requirement authoring — see the
  `canon-policy` skill; here you only reference an evidence `kind` policy
  already declares.
- A scene-DSL grammar or embedded CEL expression layer — explicit S10
  non-goals (`canon-vocab` lifts the donor project's manifest/checker,
  never its scene-syntax/CEL-expression layers).
