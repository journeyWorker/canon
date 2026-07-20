## ADDED Requirements

### Requirement: `canon-policy` evaluates CEL on upstream `cel-interpreter`, never the donor's CEL-binding crate
The `canon-policy` crate SHALL implement CEL parsing, type-checking, and
evaluation on upstream `cel-interpreter`. No canon crate SHALL depend on
the donor's CEL-binding crate, `cel-parser`, or the donor's scene-DSL parser crate, transitively or otherwise.

#### Scenario: canon-policy's dependency graph excludes the donor's CEL-binding crate
- **WHEN** `canon-policy`'s crate dependency graph is inspected
- **THEN** `cel-interpreter` is present and the donor's CEL-binding crate, `cel-parser`, and
  the donor's scene-DSL parser crate are absent from every transitive dependency

#### Scenario: A CEL predicate evaluates without a bespoke mini-DSL
- **WHEN** a consumer needs a conditional knob (a routing predicate, an
  aging threshold, a guard) and calls `canon-policy` with a CEL expression
- **THEN** `canon-policy` parses and evaluates it directly — the consumer
  does not implement its own predicate grammar

### Requirement: CEL bindings are generated from the same schema registry as `canon fmt`/`canon context`
`canon-policy` SHALL generate every CEL variable and function binding from
the identical `SchemaRegistry` API `canon fmt`'s validator and `canon
context`'s authoring surface call. No second, independently-maintained
binding list SHALL exist.

#### Scenario: A schema field change reflects in the CEL binding set with no second edit
- **WHEN** a field is added to a schema-registered record kind
- **THEN** that field becomes available as a CEL variable to any expression
  bound to that kind, and `canon context`'s CEL section lists it, from the
  single schema edit

#### Scenario: An undeclared field is not a valid CEL identifier
- **WHEN** a CEL expression references a field not present in the target
  kind's schema
- **THEN** the expression is rejected as referencing an undeclared
  identifier, never silently resolved to an empty/null value

### Requirement: Every stored CEL expression is validated at write time
`canon-policy` SHALL parse and type-check every CEL expression against its
target kind's bindings before the expression is accepted for storage. A
type-invalid expression SHALL be rejected at write time, naming the
expected type or member set.

#### Scenario: A type-invalid expression is rejected at write time
- **WHEN** a CEL expression is submitted for storage (e.g. in `policy.yaml`
  or an `applies_when:` field) that references an undeclared identifier, or
  calls a function with the wrong argument count or type
- **THEN** the write is rejected before storage, with a diagnostic naming
  the expected type or the expected member set — never accepted and left to
  fail on first evaluation

#### Scenario: A type-valid expression against declared bindings is accepted
- **WHEN** a CEL expression references only declared identifiers and calls
  only allowlisted functions with correctly typed arguments
- **THEN** the expression is accepted and stored

### Requirement: `canon context` lists available CEL variables and functions
`canon context` (S12) SHALL list the CEL variables and functions available
for a given kind, sourced from the identical binding-generation call the
write-time validator uses.

#### Scenario: canon context's CEL section matches the write-time validator's accepted set
- **WHEN** `canon context --json` is run against a repo and its `policy`
  section is compared to the write-time validator's accepted identifier/
  function set for the same kind
- **THEN** the two sets are identical, both produced from the same
  `canon-policy::bindings_for` call

### Requirement: CEL evaluation is pure, total, bounded, and deterministic
`canon-policy` evaluation SHALL perform no I/O, SHALL register only a fixed
allowlist of pure functions, SHALL never panic on malformed input (a bad
call SHALL produce an error value), SHALL be bounded by an eval budget, and
SHALL be deterministic under `canon selftest`.

#### Scenario: Evaluation is deterministic across repeated runs
- **WHEN** `canon selftest` evaluates the same CEL expression against the
  same input facts twice
- **THEN** the two evaluations produce byte-identical results

#### Scenario: A malformed function call produces an error value, not a crash
- **WHEN** a CEL expression calls an allowlisted function with input that
  the function cannot handle (e.g. `age_days` given a non-timestamp value
  through a binding mismatch)
- **THEN** evaluation returns an error value and the calling process does
  not crash

#### Scenario: A pathological expression is bounded before it can run away
- **WHEN** a CEL expression is structurally pathological (AST/nesting/
  comprehension depth beyond `canon-policy`'s compile-time complexity bound),
  or is evaluated against a record whose JSON node count exceeds the record cap
- **THEN** it is rejected at compile time (complexity bound) or before the
  evaluation thread is ever spawned (record cap) — no unbounded evaluation
  runs; because `cel` exposes no mid-evaluation interrupt, a wall-clock
  deadline is retained only as defense-in-depth, and the caller is never
  blocked indefinitely

### Requirement: A CEL policy.yaml derives the same required cells as an equivalent static-map fixture
A `policy.yaml` expressing its required-cell rules as CEL predicates SHALL
derive the same required-cell set as an equivalent policy expressed as a
static map, over the same fixture corpus.

#### Scenario: CEL predicates and a static map agree on required cells
- **WHEN** `canon selftest` evaluates a fixture corpus against both a CEL-
  predicate `policy.yaml` and an equivalent static-map `policy.yaml`
  encoding the same routing rules
- **THEN** the two required-cell sets produced are identical

### Requirement: Reward functions, ingest transforms, and evidence records never carry a CEL expression
`canon-policy` SHALL expose no reward-scoring entry point. `canon-ingest`
adapters SHALL have no `policy.yaml`-configurable CEL hook. `canon-model`'s
`EvidenceRecord` schema SHALL have no expression-typed field.

#### Scenario: EvidenceRecord's schema has no expression-typed field
- **WHEN** `canon-model`'s `EvidenceRecord` schema is inspected
- **THEN** no field accepts a CEL expression or free-form expression string
  — every field is typed data

#### Scenario: canon-learn has no CEL-configured reward path
- **WHEN** a reward function is defined in `canon-learn`
- **THEN** it exists as versioned Rust code, and `canon-policy` exposes no
  API a reward computation could call to evaluate a stored expression in
  its place
