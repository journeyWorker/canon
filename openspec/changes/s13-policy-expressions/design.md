## Context

Design doc §5 S13 (last entry in the S0–S13 inventory) and decision 12
(design §2): "CEL is canon's single policy-expression language." The donor vocabulary project fences
CEL out of its own scripting surface because a content engine must not
execute anonymous scripts at runtime; canon runs in a trusted operator/CI
context, so that constraint does not apply — "every conditional knob that
would otherwise grow a bespoke mini-DSL uses CEL instead." Boundaries stated
in the same decision: reward functions stay versioned Rust (reproducibility)
and evidence records stay pure data. S13 ships in wave W1, parallel with
S3/S4/S11/S12 (design §6).

Open question 5 (design §10, resolved 2026-07-10 by crate-graph audit)
already settled the donor side of this: lift the donor's manifest crate +
the donor's span crate only; INSPIRE-only reimplementation for
the donor's checker crate/the donor CLI/the donor's compile crate ("top-of-graph, drag in the donor's scene-DSL parser crate
+ the donor's CEL-binding crate which canon doesn't need"); **SKIP the donor's scene-DSL parser crate/the donor's CEL-binding crate/
the donor's tree-sitter grammar/the donor's LSP crate for the initial lift.** The S13 design text
itself (design §5 S13) restates the verdict for CEL specifically: `` `cel-
interpreter` — NOT the donor's CEL-binding crate: the donor lift audit recommends skipping it; its
value is donor-specific bindings ``.

The donor CEL-integration audit is the audit backing that
verdict. Its own crate-boundary analysis (§"Crate-boundary verdict") finds
the donor's CEL-binding crate a genuinely separable leaf crate (depends only on
the donor's span crate + the donor's scene-DSL parser crate + `cel-parser`, never the donor's manifest crate) —
but every pattern it catalogs (§3.1 arena/handle across the donor's scene-DSL parser crate/
the donor's CEL-binding crate boundary, §3.2 length-preserving `@ref`/`$` DSL-token
substitution, §3.4 the closed "donor CEL profile" gate, §3.6 `defs/*.yaml`
shared typed-CEL macros) exists to embed CEL fragments **inside the donor vocabulary project's own
line-oriented scene DSL syntax** — none of which canon needs, since canon's
CEL fragments (`policy.yaml` predicates, `applies_when:` values) are plain
top-level YAML scalars, not slots inside a larger non-YAML grammar canon-
policy must scan around. The donor adoption brief's per-pattern table (row
the donor's CEL-binding crate (+`cel-parser`)) states the same verdict: "SKIP — No canon spec
needs an embedded expression/predicate language [at audit time]... its
value is donor-specific bindings." The audit predates decision 12; decision
12 supersedes its "canon needs no CEL at all" premise while keeping its
"skip the donor's CEL-binding crate specifically" verdict intact — canon does adopt CEL, sourced
directly from upstream `cel-interpreter`, never through the donor's CEL-binding crate's
DSL-embedding wrapper.

Upstream `cel-interpreter` (crates.io, `cel-rust/cel-rust` on GitHub) is a
structurally independent implementation from the donor's CEL-binding crate's `cel-parser`
0.10.1 dependency — a separate crate, separate maintainer, no shared code
path. Verified public API (docs.rs, `cel-interpreter` 0.10.0):
`Program::compile(source: &str) -> Result<Program, ParseErrors>` (parse
only), `Program::execute(&self, context: &Context) -> ResolveResult`
(evaluate), `Program::references(&self) -> ExpressionReferences` (the
variable/function set an expression touches — the hook S13's write-time
type-check pass is built on, D3 below); `Context::add_function` accepts an
arbitrary Rust closure with no built-in purity restriction;
`ExecutionError`'s variants (`UnexpectedType`, `UndeclaredReference`,
`InvalidArgumentCount`, …) are all **runtime** errors — `cel-interpreter`
has no compile-time type checker of its own.

Precedent for write-time-validated CEL already exists in-house: the donor monorepo's
spaces-lens surface validates a lens's `applies_when:` CEL expression at
write time (`packages/harp-core/src/domains/spaces-lens/
cel-parse-hook.ts:171-229` `parseCelExpression`; canonicalized via
`applies-when-canon.ts`), rejecting any expression outside a closed
allowed-identifier set (`time_of_day`, `weather`, `season`, `period`,
`event`, `function`, `observer`) before the file is ever written. S1/S10's
`applies_when` consumer (design §5 S13) follows this exact shape: closed,
schema-declared identifiers, write-time rejection, never a runtime-only
check.

## Goals / Non-Goals

**Goals:**
- `canon-policy` crate: parse, statically type-check, and evaluate CEL
  expressions on upstream `cel-interpreter` — never the donor's CEL-binding crate.
- CEL bindings (variables + the fixed pure-function allowlist) generated
  from the SAME `SchemaRegistry` `canon fmt`/`canon context` already read
  (S1/S11/S12) — no second registration site.
- Write-time validation: parse + type-check every stored expression against
  current bindings, with "expected …" diagnostics naming the expected type
  or member set, before the expression is ever evaluated.
- `canon context` (S12) lists the available CEL variables and functions,
  sourced from the identical binding call the validator uses.
- Evaluation is pure (no I/O — the registered function set is a reviewed,
  fixed allowlist), total (a bad call is an `ExecutionError` value, never a
  panic), bounded by an eval budget, and deterministic under `canon
  selftest`.
- Explicit, structurally enforced non-CEL boundary: reward functions (S7)
  stay versioned Rust; ingest transforms (S3/S4) stay Rust adapters;
  evidence records (S1) never carry an expression field.

**Non-Goals:**
- Lifting any part of the donor's CEL-binding crate's DSL-embedding machinery (arena/handle
  split across a syntax-tree boundary, `@ref`/`$` sigil substitution, the
  closed "donor CEL profile" gate, `defs/*.yaml` shared typed-CEL macros) —
  all of it exists to embed CEL inside the donor vocabulary project's own scene-script DSL grammar;
  canon's CEL fragments are standalone YAML scalars with no enclosing DSL
  to scan around.
- Rewiring S2 (tier aging), S4 (verdict-mapping guards), S5 (policy
  routing), S8 (retrieval filters), S1 (Handoff templates), or S10 (typed
  task atoms) to actually replace their current static mechanisms with CEL
  predicates — each of those already-authored, already-`--strict`-valid
  changes adopts `canon-policy` in its own follow-up change; this change
  ships the engine, its invariants, and its fixtures only.
- A general-purpose scripting or plugin execution surface. `canon-policy`
  evaluates predicates/value expressions over a closed, schema-declared
  binding set; it is not an extension point for arbitrary host functions —
  the pure-function allowlist is fixed and versioned, never
  consumer-extensible per `policy.yaml`.
- LSP features for CEL (hover, go-to-def, semantic highlighting) — S10
  sequences its LSP surface last (wave W4) with no S-number of its own
  ("(later)", design §5 S10); `canon context`'s CEL listing is this
  change's only agent-facing surface.

## Decisions

**D1 — `canon-policy` wraps upstream `cel-interpreter` directly;
the donor's CEL-binding crate is never a dependency, transitively or otherwise.**
the donor's CEL-binding crate's entire value-add is embedding CEL fragments inside the donor vocabulary project's own
line-oriented scene DSL (arena/handle indirection so the DSL AST never
depends on `cel-parser`, length-preserving `@`/`$` token substitution so a
DSL sigil doesn't confuse the CEL grammar, a closed "donor CEL profile" gate
tied to the donor vocabulary project's `<branch>`/`<match>`/`::set` grammar). None of it has a
canon target: canon's CEL fragments (`policy.yaml` predicates,
`applies_when:` values) are top-level YAML scalars, never slots inside a
larger non-YAML DSL. Additionally, the donor's checker crate's own `Cargo.toml`
hard-depends on the donor's CEL-binding crate + `cel-parser` (the donor CEL-integration audit
§"Coupling") — so even an INSPIRE-only lift of the donor's checker crate's "expected one
of" diagnostic pattern (already the S12 D6 precedent, ported without any
the donor's CEL-binding crate dependency) confirms the pattern is separable *from* CEL, not
an argument *for* taking the donor's CEL-binding crate along.
*Alternative rejected:* depend on the donor's CEL-binding crate for its arena/parse layer
alone, reimplementing only the semantic layer. Rejected because the donor's CEL-binding crate's
own `CelAstHandle` type is defined in the donor's scene-DSL parser crate, not the donor's CEL-binding crate
(the donor CEL-integration audit §3.1) — using the donor's CEL-binding crate's parse API at all pulls
the donor's scene-DSL parser crate transitively regardless of which higher-level pattern is or
isn't reused.

**D2 — CEL bindings are generated from the SAME `SchemaRegistry`
`canon fmt`/`canon context` already read (S12 D2 extended to CEL).**
`canon-policy::bindings_for(kind: &RecordKind, registry: &SchemaRegistry) ->
BindingSet` derives CEL variable declarations (e.g. `record.kind`,
`record.at`) from the identical `SchemaRegistry` canon-model's schema
validator and S12's `resolve_surface` call — never a second, hand-written
list of "what fields a policy expression can read." The fixed pure-function
allowlist (`age_days`, `has`, …) is enumerated alongside the variable set in
the same `BindingSet`, versioned with the schema it binds against.
Rationale: this is S12 D2's "one function, multiple callers" invariant
("no second registration site is a code-structure guarantee, not a
documentation promise") applied to CEL — a schema change that
adds/removes a bindable field is reflected in both `canon context`'s CEL
section and the write-time validator's accepted-identifier set from the
single schema edit.

**D3 — Write-time validation is canon-policy's own static pass over
`Program::references()`, because `cel-interpreter` has no compile-time type
checker.**
`Program::compile` only catches syntax errors (`ParseErrors`); type
mismatches (`ExecutionError::UnexpectedType`, `UndeclaredReference`,
`InvalidArgumentCount`, …) are runtime-only in `cel-interpreter`'s own API
— confirmed against docs.rs 0.10.0, no static type-checking method exists
on `Program` or `Context`. Since "every stored expression is validated at
write time" (design §5 S13) is a hard invariant, `canon-policy` adds its
own phase: after `Program::compile` succeeds, `Program::references()`
returns the variable/function set the expression touches; `canon-policy`
cross-checks that set against the target kind's `BindingSet` (D2) — an
undeclared identifier, or a function call with the wrong arity/argument
type against the allowlist's declared signature, is rejected before
storage. Diagnostics use the same "expected …" shape S12 D6 established
for enum mismatches: `` `age_days` expects 1 argument of type `timestamp`,
got 0 `` / `` `record.severty` is not a declared field of `run` (expected
one of: kind, at, severity, …) ``.
*Alternative rejected:* defer type errors to first-evaluation time.
Rejected because a stored-but-rarely-hit expression (an edge-case routing
branch, say) would sit as dead-until-triggered breakage — the design's own
acceptance criterion is explicit that a type-invalid expression is
"rejected at write time," not "rejected on first hit."

**D4 — Purity is a `canon-policy`-owned discipline: only a fixed, reviewed,
pure function set is ever registered; no consumer registers its own.**
`cel-interpreter`'s `Context::add_function` accepts any Rust closure with
full access to the call's `FunctionContext` — the crate itself enforces no
purity constraint. `canon-policy`'s guarantee is therefore structural, not
inherited: the function allowlist (`age_days(timestamp) -> int`,
`has(path) -> bool`, …) is enumerated once in `canon-policy`, each function
takes its inputs as explicit CEL arguments (e.g. `age_days` takes
`record.at`; evaluation's own "now" is passed in by the caller, never read
from the wall clock inside the function) and returns an `ExecutionError`
value on bad input rather than panicking (total). No `policy.yaml` or
`applies_when:` author can register a new function — the allowlist is
versioned alongside the binding set (D2), reviewed like any other
canon-policy source change.
*Alternative rejected:* let each consumer spec (S5, S2, …) register its own
ad hoc CEL functions. Rejected — this reopens exactly the "second
registration site" problem D2 closes for variables, and removes the
single-review-point guarantee that keeps every registered function
provably side-effect-free.

**D5 — Evaluation is bounded by preflight complexity + record-size limits,
not by a mid-evaluation abort.**
CEL's core grammar has no unbounded loops, but its `map`/`filter`/`all`/
`exists` macros iterate a list whose length is a runtime fact, not a static
bound — a pathologically large input list or a deeply nested comprehension
chain still costs real evaluation time. `cel` 0.14 exposes NO evaluation
interrupt, step-counting hook, or cancellable context (verified against the
crate's `Program::execute`/`Value::resolve` source), so a running `execute`
cannot be stopped from outside. `canon-policy` therefore bounds cost BEFORE
evaluation can run away: (a) write-time `check_complexity` rejects
structurally pathological expressions (AST node count, nesting depth,
comprehension count, and comprehension-nesting depth) so they can never be
stored; (b) `evaluate` rejects an oversized record (`MAX_RECORD_JSON_NODES`)
before spawning the eval thread. A wall-clock deadline + thread detach is
kept only as defense-in-depth — because every accepted expression is provably
bounded on both axes, a detach can no longer accumulate unbounded work.

**D6 — `canon context` (S12) lists CEL variables/functions from the
identical `BindingSet` D2 produces.**
`resolve_surface` (S12 D1) gains a `policy` section populated by
`canon-policy::bindings_for` for every kind active in the target repo's
`canon.yaml` — reusing S12's resolve-then-render split (resolution has one
call site; `--json` and the outline render the same resolved value). The
write-time validator's "expected …" diagnostics (D3) and `canon context`'s
`policy` section are therefore always in agreement by construction, the
same guarantee S12 D6 established for enum domains.

**D7 — The non-CEL boundary is structural, not only documented.**
- **Reward functions (S7) stay versioned Rust.** `canon-policy` exposes no
  reward-scoring entry point; a reward function is compiled, reviewed,
  versioned Rust code in `canon-learn`, because a CEL-configured reward
  could drift silently between runs with no code review catching the
  change (design §5 S13: "CEL-configured rewards drift silently").
- **Ingest transforms (S3/S4) stay Rust adapters.** `canon-ingest`'s event
  normalization and S4's review→verdict mapping table (a fixed Rust
  `match`, per the S4 design) have no `policy.yaml`-configurable CEL hook —
  adding one would let a policy edit silently change what an ingested event
  *means*, not just how it routes.
- **Evidence records (S1) never carry an expression field.** `canon-model`'s
  `EvidenceRecord` envelope has no string/CEL-typed field anywhere in its
  schema; a record is pure data describing what happened. Conditional logic
  about what a record *means* for policy purposes lives in `policy.yaml`/
  `applies_when:`, evaluated against the record's fields — never inside the
  record itself.

## Risks / Trade-offs

- **[Risk]** `cel-interpreter` is a younger, less-audited dependency than
  the antlr4rust-backed `cel-parser` the donor's CEL-binding crate wraps (different upstream
  project — `cel-rust/cel-rust` — no shared code with `cel-parser`, so
  the donor CEL-integration audit's documented antlr4rust "known-benign `unreachable!
  ()` panic on malformed input" finding does not transfer either way; it
  has not been independently re-verified against `cel-interpreter`).
  **Mitigation:** D3's write-time parse+type-check pass is the only place
  untrusted-shaped input reaches the parser before storage; `canon-policy`
  wraps `Program::compile` in `catch_unwind` regardless of upstream
  panic-safety claims, mirroring the defensive posture the donor's CEL-binding crate itself
  uses for its own parser dependency (the donor CEL-integration audit §"Risks &
  incompatibilities").
- **[Risk]** D4's purity guarantee depends entirely on review discipline at
  the point a new function is added to the allowlist — nothing in
  `cel-interpreter` or Rust's type system prevents a future allowlist
  addition from doing I/O.
  **Mitigation:** the allowlist lives in one file in `canon-policy`
  (D2/D4), so a purity regression is a single-file diff to review, not a
  scattered set of per-consumer function registrations; `canon selftest`'s
  determinism fixture (task group 6) fails loudly if a registered function
  ever produces different output for identical input across two runs — a
  cheap, mechanical (if imperfect) purity smoke test.
- **[Risk]** A wall-clock deadline alone would be timing-dependent (the same
  expression could pass on a fast machine and fail on a loaded one), making
  budget failures non-reproducible across environments.
  **Resolution:** the primary bound is now DETERMINISTIC — a static
  compile-time complexity limit (AST/nesting/comprehension) plus a runtime
  record-node cap, both reproducible across environments. `cel` 0.14 exposes
  no step-counting/interrupt hook (verified against the crate source), so the
  wall-clock deadline is retained only as a non-authoritative defense-in-depth
  backstop, not the primary contract. The complexity/record bounds are set
  generously relative to the closed, non-recursive expression shapes canon's
  own consumers (S5/S2/S4/S8/S1/S10) need.
- **[Risk]** Naming the intended consumer list (S5/S2/S4/S8/S1/S10) without
  wiring any of them in this change means `canon-policy` ships with no
  real caller until a follow-up change lands — the CEL-vs-static-map
  equivalence fixture (task group 6) is `canon-policy`'s only proof of
  real-world applicability until then.
  **Trade-off accepted:** wave W1 sequences S13 parallel with S3/S4/S11/
  S12, ahead of W2's S5/S8 and W0's already-landed S1/S2 — S13 cannot wait
  for consumer changes to land first without breaking the wave order the
  design doc already fixes (design §6).
