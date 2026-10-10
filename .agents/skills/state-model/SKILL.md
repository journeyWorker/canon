---
name: state-model
description: How to extend canon-model's closed record-kind set, join-spine keys, and Handoff body template registry — adding/bumping a record kind, adding a join-key newtype, and registering a new Handoff domain template. Use when touching crates/canon-model, adding a new canon.yaml handoff_templates entry, or regenerating JOIN_SPINE.md / schemas/*.schema.json.
---

# state-model

`canon-model` (`crates/canon-model`) is canon's closed, versioned
artifact-family type set (S1). This skill covers the three ways it
grows, and the generated-output discipline every change to it must
respect.

## The record-kind set is closed — review before extending

`RecordKind` (`src/envelope.rs`) has exactly fourteen variants (design
D1): `Change`, `Task`, `Scenario`, `Session`, `Run`, `Event`, `Handoff`,
`Review`, `Divergence`, `Trajectory`, `StrategyItem`, `EvidenceRecord`,
`Subject`, `Finding`.
This is deliberate friction, not an oversight — an open `kind: String` +
untyped `payload` escape hatch is exactly what let an internal monorepo accumulate three
uncoordinated management systems before canon existed (design D1's
rejected alternative).

Before adding a fifteenth kind:

1. Confirm the new artifact family genuinely doesn't fit an existing
   kind's fields (extending an existing kind's `schema` version, below,
   is almost always the right move first).
2. If it truly needs a new kind, this is a reviewed, explicitly-scoped
   `canon-model` change — not a drive-by addition inside an unrelated
   spec's implementation. Add the variant to `RecordKind` AND to
   `RecordKind::ALL` (both are asserted in sync by
   `envelope::tests::all_fourteen_kinds_present_exactly_once`), add the
   struct in `src/records.rs` (or its own module, for something
   `Handoff`-sized), implement `CanonRecord` for it, add it to
   `schema_export::record_schemas()`, and add a well-formed fixture
   under `fixtures/well-formed/<kind>.json`.
3. Every record type composes `Envelope` via `#[serde(flatten)]` — never
   add an ad hoc `actor`/`by` field. `envelope::CanonRecord` is the one
   dispatch trait every kind implements; use it instead of re-deriving a
   kind ↔ type mapping in a new caller.

## Bumping a kind's `schema` version

`Envelope.schema: u32` (design D2) is the per-kind FORMAT GENERATION.
Bump it when:

- A required field is added/removed/retyped on an existing record kind.
- A `FailureClass` string is renamed (evidence-integrity spec: "renaming
  a failure class requires a coordinated migration" — ship the rename
  together with updated fixtures referencing the old string, in the same
  change).
- **A kind whose `at` is BYTE-STABLE gains any field that changes its
  derived content** — even a purely additive `Option<T>`/`Vec<T>`. See
  the trap below; this one is not about wire compatibility at all.

### The additive-field trap on byte-stable-`at` kinds

A plan-derived record's `Envelope.at` is `file_modified_at(<source
doc>)` — deliberately byte-stable, never wall-clock (s20 D7), which is
what makes re-importing an unchanged source idempotent. The consequence
(found the hard way in s38 `evidence-bearing-memory`): a canon CODE
change does not advance the source file's mtime, so the stale and fresh
records for one natural key carry an IDENTICAL `at`.

`canon_store::fold::fold_latest_by_key` orders by `(at, schema,
digest)`. With no schema bump both generations tie on `at` AND on
`schema`, so the winner falls to the lexicographic `digest` tie-break —
**arbitrary per row**. The symptom is brutal to diagnose: the new field
appears on some records and not others within one source file, with zero
import diagnostics, looking exactly like a parser bug. Three plausible
structural hypotheses were chased before the store fold was identified.

So for `Task`/`Change` (and any future kind projected from a source
document), a field addition IS a generation change and MUST bump
`schema`. `Task` is at `2` for exactly this reason.

Kinds whose `at` is derivation-time (`Run`, `Handoff`, `Session`,
`Event`, …) cannot tie this way, so a purely additive `Option<T>` field
with `#[serde(default)]` needs no bump there — `schema_export`'s own
scenario ("a field addition is reflected without a second registration
site") still holds for them.

### Additive fields must also skip when empty

Independent of the bump: a new field MUST be
`#[serde(default, skip_serializing_if = "Option::is_none")]` (or
`"Vec::is_empty"`), never a bare `#[serde(default)]`. Bare default makes
every EXISTING record reserialize with a spurious `null`/`[]`, which
changes its `content_digest12` and breaks the write-time idempotence that
ingest watermark cursors, `trajectory_content_digest`, and `canon report
--check`'s byte-diff drift gate all rest on. Note
`fixtures/well-formed/*.json` assert a LOSSLESS round-trip
(`to_value(value) == fixture`), so a fixture demonstrating the full field
surface must carry real values rather than nulls.

### A parser change needs a cursor bump too

Bumping `schema` makes the fresh record WIN, but something still has to
make it get WRITTEN. The plan-import cursor is keyed on dialect + root +
per-file content digests, so a parser change looks like "unchanged" and
the source is skipped. `PlanAdapter::parse_version()` is folded into the
cursor id for that reason — bump it whenever a dialect's parse output
changes for identical input.

## Adding a join-spine key newtype

The nine join-spine keys (`src/ids.rs`) are declared through the
`join_key_newtype!` macro: one literal `grammar`/`joins` pair per
invocation, expanded into the type's own rustdoc comment, its
`GRAMMAR`/`JOINS` associated constants, and its `JsonSchema` impl — all
three can never drift relative to each other because they come from the
same macro-invocation literal. `crate::join_spine_doc::rows()` reads
those same constants to build the generated `JOIN_SPINE.md`.

Adding a tenth key (should the design ever call for one) means: a new
`join_key_newtype!` invocation, a hand-written `parse`/grammar-check
`impl` block below it (kept out of the macro so grammars stay ordinary,
testable Rust), unit tests for accept/reject cases, and a new row added
to `join_spine_doc::rows()`.

## Registering a new Handoff domain template

`Handoff`'s state-machine fields (`id`, `state`, `chain_id`, …) are
fixed and wire-compatible with a prior session store's `handoffs` table; the body
(`HandoffBody { domain, template_version, fields }`) is per-domain and
template-validated (design D4/D5). To register a new domain (디자인,
개발, 테스트, …):

1. Implement `handoff::HandoffTemplate` for the new domain (see
   `GihoekTemplate` in `src/handoff.rs` for the 기획 reference
   implementation): `domain()`, `validate(fields) -> Result<(), Vec<EvidenceViolation>>`,
   `render(fields) -> String`.
2. Add the domain string to this repo's root `canon.yaml`'s
   `handoff_templates:` list — a template compiled into `canon-model`
   but absent from `canon.yaml` is treated as unregistered
   (`TemplateRegistry::from_manifest`'s per-repo activation gate).
3. Construct the registry with the new template in `available`:
   `TemplateRegistry::from_manifest(canon_yaml, vec![Box::new(GihoekTemplate), Box::new(YourTemplate)])`.
4. A `Handoff` whose `body.domain` isn't both compiled AND listed in
   `canon.yaml` fails construction with a structured
   `unregistered-handoff-domain` `EvidenceViolation` — never a silent
   accept.

## Regenerating `JOIN_SPINE.md` / `schemas/*.schema.json`

Both are generated, never hand-edited (design D3). After changing a
join-key grammar doc comment or a record kind's fields:

```bash
cargo xtask write          # regenerate + overwrite the committed files
cargo xtask check-generated # regenerate in memory, diff, exit non-zero on drift
```

`cargo test --workspace` already runs the same check
(`canon_model::gen::tests::committed_generated_output_matches_current_source`)
— drift fails the test suite directly, not only a separate CI step.
