---
name: canon-vocab
description: How to author against canon's typed vocabulary — declare a directive or enum in a .canon/vocab/<id> plugin, author a typed `::task` atom with a policy-resolved evidence requirement, author a `::handoff-<domain>` body, and why to run canon context first. Use when adding a directive/enum, or writing a typed task or handoff atom.
---

# canon-vocab

Canon's typed authoring layer replaces a freeform `tasks.md` checkbox's
implicit evidence with a structured, checker-validated, policy-resolved
attribute — and gives handoff bodies a declared, rendered template. You
declare a vocabulary in a `.canon/vocab/<id>/` plugin, then author atoms
that the checker validates against it.

## Run `canon context` FIRST

Before authoring any atom, run `canon context` (or `--json`) and read its
`vocab` section: it lists every declarable directive + enum for the repo
plus a content-hash version. Authoring against a directive/enum
`canon context` does not list means the checker will reject it — the
surface is the source of truth for "what can I declare here?" See
`canon-context`.

## Declaring a directive or enum

A vocabulary plugin lives at `.canon/vocab/<id>/`:

```yaml
# .canon/vocab/<id>/plugin.yaml
id: my.plugin
version: "0.1.0"
kind: project           # `core` for canon.core; `project` for a consumer plugin
exports:
  directives: directives/   # a dir of *.yaml directive files
  enums: enums.yaml
```

```yaml
# .canon/vocab/<id>/directives/thing.yaml
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
        required: true           # resolves LIVE from .canon/policy.yaml,
                                 # never a locally-declared enum
```

```yaml
# .canon/vocab/<id>/enums.yaml
enums:
  task-status: [open, done]
```

- **Type kinds**: `string`; `{ domain: <enum> }` (validated against
  `enums.yaml`); `{ list: <type> }`; and `evidence` (the `{kind, ref}`
  shape whose `kind` domain comes from `.canon/policy.yaml`'s evidence
  kinds, NOT a local enum).
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
      ref: "cargo test"
```

The atom is validated against the resolved vocabulary and compiled to a
canon `Task`. A wrong tag, an unknown attr, a missing required attr, an
out-of-domain `status`, or an evidence `kind` absent from `policy.yaml`
each produces a checker diagnostic (the "expected one of: …" shape) — the
atom is rejected at author time, never compiled into a malformed Task.

## Authoring a vocabulary-defined handoff body

A handoff body tags `handoff-<domain>` (the domain selects the directive,
NOT an attr value) and satisfies that directive's attrs:

```yaml
- id: my-handoff-1
  tag: handoff-dev              # dev | design | content | test (canon.core)
  attrs:
    title: "wire the vocabulary into the CLI"
    summary: "the foundation is ready for CLI integration"
    verification-steps:
      - "cargo build"
      - "cargo test"
```

It's validated and compiled to a canon handoff body. Each
`handoff-<domain>` directive declares its own attrs (e.g. `design` /
`content` domains may require `acceptance-criteria` instead of
`verification-steps`) — read the domain's directive (or `canon context`)
for the exact required set.

## What this skill does NOT cover

- The `canon context` surface itself — see `canon-context`; this skill
  covers authoring AGAINST it.
- `policy.yaml` / CEL evidence-requirement authoring — see `canon-policy`;
  here you only reference an evidence `kind` policy already declares.
