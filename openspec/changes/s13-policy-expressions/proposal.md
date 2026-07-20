## Why

Every canon spec that needs a conditional knob — S5's risk→platform routing,
S2's tier-aging thresholds, S4's verdict-mapping edge cases, S8's retrieval
scoping, S1/S10's template `applies_when` sections — faces the same choice:
grow a bespoke mini-DSL per spec, or share one expression language. Decision
12 (design doc §2) settles it: **CEL is canon's single policy-expression
language.** The donor vocabulary project fences CEL out of its own scripting surface because a
content engine must not execute anonymous scripts at runtime; canon runs in
a trusted operator/CI context, so that constraint does not apply. Without a
shared engine now, each consumer spec would either invent its own predicate
grammar or defer conditional logic indefinitely.

## What Changes

- New `canon-policy` crate: a CEL expression engine on **upstream
  `cel-interpreter`** — explicitly **NOT the donor's CEL-binding crate**. The donor lift audit
  recommends skipping the donor's CEL-binding crate; its value-add (arena/handle pattern across
  the donor's scene-DSL parser crate/the donor's CEL-binding crate boundary, `@ref`/`$` DSL-sigil substitution,
  the closed "donor CEL profile" gate) exists to embed CEL fragments inside
  the donor vocabulary project's own line-oriented scene DSL — machinery with no canon target, since
  canon's CEL fragments are plain top-level YAML scalar values
  (`policy.yaml` predicates, `applies_when:` strings), never slots inside a
  larger non-YAML grammar. See
  the donor adoption brief (SKIP verdict on
  the donor's CEL-binding crate/`cel-parser`) and the donor CEL-integration audit (§5 recommended actions, §"Coupling" — the donor's checker crate
  hard-depends on the donor's CEL-binding crate, so even an INSPIRE-only lift of its "expected
  one of" pattern would drag the donor's CEL-binding crate back in).
- CEL variable/function bindings are generated from the **SAME schema
  registry** `canon fmt`/`canon context` already read (S1/S11/S12) — no
  second, independently-maintained binding list.
- Write-time validation: every stored CEL expression is parsed and
  type-checked against the current bindings before it is accepted, with
  "expected …" diagnostics naming the expected type or member set.
- `canon context` (S12) gains a CEL section listing the available variables
  and functions, sourced from the identical binding-generation call the
  write-time validator uses.
- Evaluation invariants: pure (no I/O), total (no panics — a function error
  is a value, not a crash), bounded by an eval budget, and deterministic
  under `canon selftest`.
- Explicit non-CEL boundary, enforced structurally, not just documented:
  reward functions (S7) stay versioned Rust — a CEL-configured reward could
  drift silently between runs with no code review catching the change;
  ingest transforms (S3/S4) stay Rust adapters; evidence-record content
  (S1) is pure data — an `EvidenceRecord` never carries an expression field.
- Names, but does not itself rewire, the spec's intended consumers: S5
  policy routing, S2 tier aging, S4 verdict-mapping guards, S8 retrieval
  filters, S1's Handoff template / S10's typed-task-atom `applies_when`
  sections. Each of those already-authored changes adopts `canon-policy` in
  its own follow-up change; S13 ships the shared engine, its invariants,
  and its fixture corpus only.

## Capabilities

### New Capabilities

- `policy-expression-engine`: the `canon-policy` crate — CEL parse/
  type-check/evaluate over schema-registry-generated bindings, write-time
  validation with "expected …" diagnostics, `canon context` integration,
  purity/totality/eval-budget/determinism invariants, and the structural
  non-CEL boundary (reward functions, ingest transforms, evidence records).

### Modified Capabilities

_None._ S13 ships the shared engine and its invariants as a standalone
addition; wiring S2/S4/S5/S8/S1/S10's already-authored, already-validated
specs to actually call `canon-policy` in place of their current static
mechanisms (S2's duration-map aging, S4's fixed verdict-mapping table, S5's
tag/fact routing, S1/S10's currently-nonexistent `applies_when` field) is
scoped to each of those specs' own follow-up changes, not this one.

## Impact

- New crate `canon-policy` in the `canon-cli` dependency graph; new
  upstream dependency `cel-interpreter` (crates.io) — no the donor's CEL-binding crate/
  `cel-parser`/the donor's scene-DSL parser crate dependency anywhere in canon.
- Reads (never duplicates) canon-model's (S1/S11) `SchemaRegistry` — the
  same API `canon fmt`'s validator and `canon context`'s (S12) authoring
  surface already call, extending S12 D2's "no second registration site"
  invariant to CEL bindings.
- `canon context`'s (S12) authoring surface gains a `policy` CEL section;
  its content is produced by the identical `bindings_for` call the
  write-time validator uses, so the two can never diverge.
- Establishes the intended consumer list (S5, S2, S4, S8, S1, S10) as a
  forward reference only — no other change's spec.md is edited by S13.
- New fixture corpus for `canon selftest`: CEL-predicate-vs-static-map
  equivalence, type-invalid-expression rejection, and evaluation
  determinism.
- Companion skill (design §5 cross-cutting deliverable, decision 9): a
  `canon-policy` authoring skill under `canon/skills/`, documenting how to
  write a CEL predicate against `canon context`'s CEL section and how to
  read a write-time "expected …" diagnostic.
