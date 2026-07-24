# canon-policy

> How to write a CEL policy expression bound to a canon record kind — canon's single policy-expression language — where those expressions bind (policy.yaml, a template's applies_when:), and how to decode a write-time "expected …" rejection. Use when authoring a policy.yaml predicate or a template applies_when: field, or when a policy expression is rejected.

# canon-policy

Canon uses ONE expression language — CEL — for every conditional knob a
spec needs: policy-routing predicates, tier-aging thresholds, verdict-
mapping guards, retrieval filters, and a template's `applies_when:`
section. Wherever a `policy.yaml` predicate or an `applies_when:` string
appears, it is a CEL expression evaluated against the target record.

## The closed CEL profile

Every expression is evaluated against exactly one variable, `record` (the
target kind's own fields — always `record.<field>`, never the raw
envelope), plus one registered function:

- `record.<field>` — any field the target kind declares. Nested objects
  (e.g. `record.actor.agent_id`) resolve to a bounded depth; deeply-open
  fields (an unconstrained `detail` blob) are readable but not field-
  checked at write time.
- `age_days(ts) -> int` — whole days between a timestamp field and a
  caller-supplied "now" (never the wall clock read inside the function, so
  the same expression against the same facts and the same `now` always
  evaluates identically).
- `has(record.<field>)` — CEL's built-in field-presence macro. Validates
  like a normal field read.
- CEL's operator grammar (`==`, `!=`, `&&`, `||`, `>`, `<`, `in`, the
  ternary `? :`) and macros (`map`, `filter`, `all`, `exists`,
  `exists_one`) are always available.

Nothing else is. There is NO `size()`, `matches()`, `contains()`,
`string()`, or any other CEL standard-library method — the profile is a
closed allowlist of exactly the one function above. A
`record.title.contains('x')`-shaped expression is rejected at WRITE time
with an `UnknownFunction` diagnostic, not deferred to a runtime failure.
Express the same check with `record.<field> == <literal>` / `in` / `has`
instead.

## Discovering what's bindable for a kind

Run `canon context --json` and read its `cel` section: for each record
kind it lists the bindable `record.<field>` names + types and the
callable function allowlist a `policy.yaml` predicate may reference. This
is the source of truth for "what can a CEL predicate for this kind
reference?" — it's derived from the same schema `canon format` validates
against, so there is no second, hand-maintained list to drift. See the
`canon-context` skill.

## Writing and validating an expression

An expression is compiled BEFORE storage: both parsing AND a full walk
(checking every `record.<field>` chain and every function call) run up
front. There is no "accepted but fails on first evaluation" path — a
type-invalid expression is rejected at author time with every diagnostic,
not just the first.

Evaluation is bounded by a wall-clock deadline (default 200ms): a
pathological expression is reported as budget-exceeded rather than
blocking. `now` is always caller-supplied — one `canon gate` run scoring
many records uses the same `now` across the batch for mutually consistent
results.

## Reading a write-time "expected …" diagnostic

Every rejection names what was expected, never a bare "invalid
expression":

- **Undeclared field** — `` `record.severty` is not a declared field of
  `task` (expected one of: actor, at, evidence_note, kind, schema,
  status, task_id, title) `` — check the name against `canon context`'s
  field list; a typo is the usual cause.
- **Undeclared variable** — `` `foo` is not a declared variable (expected
  one of: record) `` — only `record` (plus a `map`/`filter` loop
  variable) is ever in scope.
- **Unknown function** — `` `contains` is not an allowlisted function
  (expected one of: age_days) `` — a typo, or a CEL standard-library
  method the closed profile doesn't register.
- **Arity mismatch** — `` `age_days` expects 1 argument(s), got 0 ``.
- **Type mismatch** — `` `age_days` expects argument 0 of type
  `timestamp`, got `string` `` — the field passed isn't the type the
  function's signature declares.

## What never gets a CEL hook

Some knobs deliberately stay code, not configurable CEL: reward-scoring
functions, ingest/verdict-mapping transforms, and the content of an
evidence record itself. If a task looks like "let this record configure
its own evaluation logic," it is out of scope — raise it as a design
question rather than adding an expression field. Conditional logic about
what a record *means* lives in `policy.yaml`/`applies_when:`, evaluated
against the record's fields, never inside the record.
