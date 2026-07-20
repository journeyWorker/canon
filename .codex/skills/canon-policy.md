# canon-policy

> How to write, validate, and read diagnostics for a CEL policy expression bound to a canon-model record kind — canon's single policy-expression language (design decision 12). Use when authoring a policy.yaml predicate, a template applies_when: field, or any other conditional knob backed by crates/canon-policy, or when a write-time "expected …" rejection needs decoding.

# canon-policy

`canon-policy` (S13, `crates/canon-policy`) is canon's one shared CEL
expression engine. Every conditional knob a spec needs — S5's
policy-routing predicates, S2's tier-aging thresholds, S4's
verdict-mapping guards, S8's retrieval filters, S1/S10's template
`applies_when:` sections — uses this same engine instead of growing its
own bespoke mini-DSL (design decision 12).

## The closed CEL profile

Every expression is evaluated against exactly one variable, `record`
(the target record kind's own fields — never the raw JSON envelope
name, always `record.<field>`), plus one registered function:

- `record.<field>` — any field the target kind's schema declares.
  Nested objects (e.g. `record.actor.agent_id`) are reachable up to a
  bounded resolution depth; deeply-open fields (`Event.detail`, an
  unconstrained `serde_json::Value`) are readable but not
  field-checked at write time.
- `age_days(ts) -> int` — whole days between a timestamp field and the
  evaluation call's caller-supplied "now" (never the wall clock read
  inside the function itself — this is what keeps `canon selftest`'s
  determinism fixture green: the SAME expression against the SAME
  facts and the SAME `now` always evaluates identically).
- `has(record.<field>)` — CEL's own built-in field-presence macro
  (google/cel-spec), not a canon-policy-registered function. Works
  exactly like a normal field read for validation purposes.
- CEL's own operator grammar (`==`, `!=`, `&&`, `||`, `>`, `<`, `in`,
  the ternary `? :`) and macros (`map`, `filter`, `all`, `exists`,
  `exists_one`) are always available.

Nothing else is. There is **no** `size()`, `matches()`, `contains()`,
`string()`, or any other CEL standard-library method call — `canon-
policy` evaluates against an empty `cel::Context` plus exactly the one
registered function above (design D4's closed, reviewed, pure-function
allowlist; see `crates/canon-policy/src/functions.rs`'s module doc for
the full purity audit). A `record.title.contains('x')`-shaped
expression is rejected at write time with an `UnknownFunction`
diagnostic, not deferred to a runtime failure.

Adding a new function to the allowlist is a reviewed `canon-policy`
source change (one file, `functions.rs` + `bindings.rs`'s
`allowlisted_functions()`) — never a per-consumer registration inside
a `policy.yaml`.

## Discovering what's bindable for a kind

`canon context`'s CEL section (design D6) is the intended agent-facing
surface for this, but it depends on S12's `resolve_surface`/
`AuthoringSurface`, which had not landed as of S13. Until then, call
`canon-policy::bindings_for` directly:

```rust
use canon_model::RecordKind;
use canon_policy::{bindings_for, SchemaRegistry};

let registry = SchemaRegistry::load();
let bindings = bindings_for(RecordKind::Task, &registry);
for name in bindings.field_names() {
    println!("record.{name}");
}
for function in &bindings.functions {
    println!("{function}"); // e.g. "age_days(timestamp) -> int"
}
```

`bindings_for` derives every field from canon-model's own schema
export (`canon_model::schema_export::record_schemas()`) — the SAME
data `canon fmt`'s validator reads (S11) — so there is no second,
hand-maintained list of "what fields a policy expression can read" to
fall out of sync (design D2).

## Writing and validating an expression

```rust
use canon_policy::compile;

match compile("record.status == 'done' && age_days(record.at) > 7", &bindings) {
    Ok(policy) => { /* store `policy.source()` */ }
    Err(diagnostics) => { /* reject the write; show every diagnostic, not just the first */ }
}
```

`compile` never returns `Ok` for a type-invalid expression — parsing
AND a full AST walk (checking every `record.<field>` chain and every
function call) both happen before storage (design D3). There is no
"accepted but fails on first evaluation" path.

## Reading a write-time "expected …" diagnostic

Every rejection names what was expected, never a bare "invalid
expression":

- **Undeclared field**: `` `record.severty` is not a declared field of
  `task` (expected one of: actor, at, evidence_note, kind, schema,
  status, task_id, title) `` — check the field name against
  `bindings.field_names()`; a typo is the most common cause.
- **Undeclared variable**: `` `foo` is not a declared variable
  (expected one of: record) `` — only `record` (plus a `map`/`filter`
  macro's own local loop variable) is ever in scope.
- **Unknown function**: `` `contains` is not an allowlisted function
  (expected one of: age_days) `` — either a typo'd function name, or a
  CEL standard-library method that this crate's closed profile does
  not register (see above); express the same check with `record.<field>
  == <literal>` / `in` / `has` instead where possible, or raise the
  function's absence as a `canon-policy` allowlist change if the
  consumer spec genuinely needs it.
- **Arity mismatch**: `` `age_days` expects 1 argument(s), got 0 ``.
- **Type mismatch**: `` `age_days` expects argument 0 of type
  `timestamp`, got `string` `` — the field passed in isn't the type the
  function's signature declares; check `bindings.field_names()`'s
  corresponding type.

## Evaluating

```rust
use canon_policy::{evaluate, EvalBudget};
use chrono::Utc;
use serde_json::json;

let record = serde_json::to_value(&some_task)?; // any canon-model CanonRecord
let outcome = evaluate(&policy, &record, Utc::now(), EvalBudget::default())?;
assert_eq!(outcome.as_bool(), Some(true));
```

`evaluate` takes a `&CompiledPolicy` — never a raw source string — so
an unvalidated expression can never reach evaluation. It is bounded by
`EvalBudget` (a wall-clock deadline, default 200ms): a pathological
expression returns `PolicyError::BudgetExceeded` rather than blocking
the caller indefinitely (design D5). `now` is always caller-supplied,
never read from the wall clock internally — pass the SAME `now` across
a batch of evaluations that should be mutually consistent (e.g. one
`canon gate` run scoring many records).

## The non-CEL boundary (design D7) — what never gets a CEL hook

- **Reward functions (S7) stay versioned Rust.** `canon-policy` has no
  reward-scoring entry point; a CEL-configured reward could drift
  silently between runs with no code review catching the change.
- **Ingest transforms (S3/S4) stay Rust adapters.** `canon-ingest`
  normalization and S4's verdict-mapping table have no
  `policy.yaml`-configurable CEL hook.
- **Evidence records (S1) never carry an expression field.**
  `canon-model::EvidenceRecord`'s schema has no string/CEL-typed
  field — a record is pure data; conditional logic about what it
  *means* lives in `policy.yaml`/`applies_when:`, evaluated against
  the record's fields, never inside the record.

If a task looks like "let this record configure its own evaluation
logic," it is out of scope for `canon-policy` — raise it as a design
question, don't add an expression field to solve it locally.
