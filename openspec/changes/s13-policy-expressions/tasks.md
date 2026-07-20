## 1. Crate scaffold and upstream dependency

- [x] 1.1 Create the `canon-policy` crate under `crates/canon-policy`,
      depending on `cel-interpreter` (crates.io) — no the donor's CEL-binding crate,
      `cel-parser`, or the donor's scene-DSL parser crate dependency, direct or transitive
      (design D1).
      Evidence: `crates/canon-policy/Cargo.toml`. **Deviation, documented
      in `src/lib.rs`'s module doc**: depends on `cel` 0.14.0, not the
      literal `cel-interpreter` package name — verified against
      crates.io/GitHub, `cel-interpreter` 0.10.0 (its last release under
      that name) transitively depends on `cel-parser` 0.10.1, the EXACT
      crate this task's own spec requires excluding; `cel` is the same
      upstream project (`cel-rust/cel-rust`) renamed starting v0.11,
      with no `cel-parser` dependency. Depending on the literal
      `cel-interpreter` package would fail task 1.2's own audit.
- [x] 1.2 Add a workspace-level dependency audit check (or a documented
      `cargo tree` grep) confirming the donor's CEL-binding crate/`cel-parser`/the donor's scene-DSL parser crate
      never appear in canon's full dependency graph.
      Evidence: `crates/canon-policy/tests/dependency_audit.rs`
      (automated, not just documented — runs `cargo tree -p canon-policy`
      and asserts none of the donor's CEL-binding crate/`cel-parser`/the donor's scene-DSL parser crate/
      the donor's tree-sitter grammar appear); manually cross-checked with
      a workspace-wide `cargo tree` grep for the donor's CEL-binding and scene-DSL crates plus `cel-parser`
      — zero matches across the full workspace, not just canon-policy.

## 2. Binding generation (shared schema registry)

- [x] 2.1 Implement `canon-policy::bindings_for(kind: &RecordKind,
      registry: &SchemaRegistry) -> BindingSet`, deriving CEL variable
      declarations from the identical `SchemaRegistry` API S1/S11's schema
      validator and S12's `resolve_surface` call (design D2) — no second,
      hand-written field list.
      Evidence: `src/bindings.rs::bindings_for` +
      `src/registry.rs::{SchemaRegistry, record_fields}`. **Scope note**:
      S12's `SchemaRegistry::load(repo: &Path)` (in `canon-model`) has
      not landed as code (S12 is proposal-only) and `canon-model` is
      outside S13Policy's territory this batch (S11Finish/S13Policy
      split). `registry::SchemaRegistry` is a canon-policy-local adapter
      wrapping canon-model's ALREADY-existing single export function
      (`canon_model::schema_export::record_schemas()`) — no second,
      hand-written field list exists; ready to be replaced by a
      re-export of S12's registry with no `bindings_for` caller change.
      `bindings::tests::bindings_for_change_exposes_envelope_fields`
      green.
- [x] 2.2 Define the fixed pure-function allowlist (`age_days(timestamp) ->
      int`, `has(path) -> bool`, plus any other functions the named
      consumers require) in `canon-policy`, each function taking its inputs
      as explicit CEL arguments (never reading ambient state internally).
      Evidence: `src/bindings.rs::allowlisted_functions` (`age_days`,
      `has`) + `src/functions.rs::register`/`age_days` (the one
      Rust-registered function; `has` is CEL's own built-in macro, listed
      for completeness per its doc comment). No additional consumer
      functions beyond these two were added — none of the five named
      touchpoints (S5/S2/S4/S8/S1-S10) are wired by this change (out of
      scope, proposal.md), so no further function requirement is known
      yet; adding one later is a reviewed single-file diff (design D4).
- [x] 2.3 Version `BindingSet` alongside the schema/policy capability
      version so a binding-set snapshot is reproducible for a given schema
      state.
      Evidence: `src/bindings.rs::{BindingSet::version, fingerprint}` — a
      SHA-256 content hash of the resolved `record_fields` tree.
      `bindings::tests::version_is_stable_across_repeated_calls` and
      `version_differs_across_kinds_with_different_fields` green;
      `tests/reflected_change.rs` additionally asserts the version
      changes when a field is added to the underlying schema.
- [x] 2.4 Write the reflected-change test: add a field to a fixture schema
      and assert it appears in `bindings_for`'s output with no second edit.
      Evidence: `tests/reflected_change.rs::
      a_field_added_to_the_schema_appears_in_bindings_for_with_no_second_edit`
      — a synthetic "before"/"after" schema pair routed through the real,
      unmodified `SchemaRegistry::single`/`bindings_for` call; green.

## 3. Write-time validation

- [x] 3.1 Implement the write-time validation pass: `Program::compile` for
      syntax, then walk `Program::references()` against the target kind's
      `BindingSet`, rejecting undeclared identifiers and arity/type
      mismatches against the function allowlist's declared signatures
      (design D3).
      Evidence: `src/validate.rs::{compile, walk, check_ident,
      check_select, check_call, resolve_static_type}`. **Deviation from
      the literal task text, documented in `validate.rs`'s module doc**:
      walks `Program::expression()`'s full AST rather than
      `Program::references()` alone — verified against `cel` 0.14.0's
      actual API, `references()` returns only flat top-level names
      (`record`), never `record.<field>` paths, so it cannot by itself
      catch an undeclared field like `record.severty`; the AST walk is
      the mechanism that actually satisfies design D3's stated
      acceptance criterion. 9 `validate::tests::*` unit tests green.
- [x] 3.2 Implement the "expected …" diagnostic format for both rejection
      classes: `` `<field>` is not a declared field of `<kind>` (expected
      one of: <members>) `` for undeclared identifiers, `` `<fn>` expects
      <n> argument(s) of type `<type>`, got <m> `` for function
      arity/type mismatches.
      Evidence: `src/diagnostics.rs::Diagnostic`'s `Display` impl (exact
      "expected one of: …" / "expects N argument(s)" shapes);
      `validate::tests::rejects_undeclared_field_with_expected_list`,
      `rejects_wrong_arity`, `rejects_wrong_argument_type` assert the
      literal message shape.
- [ ] 3.3 Wire write-time validation into every storage path that accepts a
      CEL expression (`policy.yaml` load, `applies_when:` field parse) so a
      type-invalid expression is rejected before it is ever persisted.
      **Partial — the `policy.yaml`-load half is now wired (emergent
      cross-change integration); the `applies_when:` half stays absent.**
      `canon-gate::PolicyResolution::resolve` (`policy.rs:317`) compiles
      every predicate via `canon_policy::compile` (`policy.rs:437`) against
      `bindings_for`-derived bindings at resolution/load, so a type-invalid
      `policy.yaml` predicate is rejected up-front — this landed as part of
      S5's `canon-gate` policy loader (S13 ships the shared engine; the
      consumer wiring is each consumer's own change, per proposal.md). NO
      `applies_when:` field parser/storage path exists anywhere in the repo
      (grep: `applies_when` appears only in a `canon-policy/src/lib.rs` doc
      comment), so that half has nothing to wire into yet; box stays
      unchecked until it does.
- [x] 3.4 Write the rejection test: submit an expression with an
      undeclared identifier and one with a wrong-typed function argument;
      assert both are rejected at write time with the expected diagnostic
      shape, never accepted and deferred to evaluation.
      Evidence: `tests/rejection.rs` (7-case fixture table: undeclared
      field, undeclared bare variable, arity too-few/too-many, wrong
      argument type, unknown function, syntax error) +
      `validate::tests::{rejects_undeclared_field_with_expected_list,
      rejects_wrong_argument_type}`; all green.

## 4. Evaluation: purity, totality, eval budget

- [x] 4.1 Bound evaluation cost by preflight limits (design D5): write-time
      `check_complexity` (AST node count, nesting depth, comprehension count +
      comprehension-nesting depth) rejects structurally pathological
      expressions before storage, and `evaluate` rejects an oversized record
      (`MAX_RECORD_JSON_NODES`) before spawning the eval thread; a wall-clock
      deadline + thread detach is kept only as defense-in-depth, since `cel`
      0.14 exposes no mid-evaluation interrupt.
      Evidence: `src/validate.rs::check_complexity`, `src/eval.rs::evaluate`
      (record-node cap before thread spawn; `recv_timeout` detach as backstop).
      `validate::tests::{rejects_pathologically_nested_comprehensions,
      a_comprehension_chain_is_not_penalized_as_nested}`,
      `eval::tests::{oversized_record_is_rejected_before_evaluation_ever_starts,
      a_pathological_expression_never_reaches_evaluation}` green.
- [x] 4.2 Wrap `Program::compile`/`execute` in `catch_unwind` as a defensive
      measure regardless of upstream panic-safety claims (design Risks).
      Evidence: `src/validate.rs::compile` wraps `Program::compile`;
      `src/eval.rs::run` (invoked inside the spawned eval thread via
      `catch_unwind(AssertUnwindSafe(...))`) wraps both re-`compile` and
      `execute`. `validate::tests::rejects_syntax_errors` (compile-time
      panic path exercised through the normal parse-error branch, since
      `cel` 0.14.0 did not panic on any malformed input tried).
- [x] 4.3 Confirm every allowlisted function (task 2.2) returns an
      `ExecutionError` value on malformed input rather than panicking; add
      a fixture case per function exercising its error path.
      Evidence: `functions::tests::
      age_days_on_non_timestamp_is_an_error_value_not_a_panic` (the one
      Rust-registered function; `has` is CEL's own macro, not
      canon-policy code, so it has no Rust error path to fixture).
- [x] 4.4 Audit `canon-policy`'s registered functions for purity (no
      filesystem/network access, no internal wall-clock reads) as a
      single-file review point (design D4) — document the audit result in
      the crate's own module doc comment.
      Evidence: `src/functions.rs`'s module doc, "# Purity audit (task
      4.4)" section.

## 5. `canon context` integration

- [x] 5.1 Extend S12's `resolve_surface`/`AuthoringSurface` with a `policy`
      section populated by `canon-policy::bindings_for` for every kind
      active in the target repo's `canon.yaml` (design D6).
      Evidence: `crates/canon-cli/src/context.rs::collect_cel` builds a
      `cel: BTreeMap<String, CelSurface>` from exactly one
      `canon_policy::bindings_for(kind, &registry)` call per kind, wired
      into `resolve_surface` (`cel: collect_cel(&registry)`). (The CEL
      bindable surface is rendered as the surface's own `cel` section; the
      S12 `policy` section carries the resolved `PolicyResolution`, per D6.)
- [x] 5.2 Extend `render_json`/`render_outline` (S12) to include the CEL
      section: variable names + types, function names + signatures.
      Evidence: `context.rs::CelSurface` (`fields`: `record.<field>` →
      `CelType`; `functions`: the allowlisted signatures) is serialized by
      `render_json` and summarized by `render_outline`; test
      `context.rs::json_and_outline_agree_on_the_cel_section`.
- [x] 5.3 Write the agreement test: assert `canon context --json`'s
      `policy` section for a kind is identical to the write-time
      validator's accepted identifier/function set for the same kind.
      Evidence: `crates/canon-cli/src/context.rs::cel_surface_matches_a_fresh_independent_bindings_for_walk`
      asserts the surface's `cel` section for each kind equals a FRESH,
      independent `bindings_for` walk — the SAME `BindingSet`
      `validate::compile` checks against — so the by-construction guarantee
      is now test-enforced, not merely "would hold".

## 6. Fixtures and `canon selftest`

- [x] 6.1 Build the CEL-vs-static-map equivalence fixture: a `policy.yaml`
      expressing required-cell rules as CEL predicates, and an equivalent
      `policy.yaml` expressing the same rules as a static map, evaluated
      over one shared fixture corpus — assert identical required-cell
      output from both.
      Evidence: `tests/equivalence.rs` — two required-cell rules
      expressed both as CEL predicates (evaluated via `canon-policy`)
      and as an equivalent Rust `if`/match static map, checked across a
      10-record fixture corpus (every status × age-threshold-crossing
      combination). **Deviation**: no literal `policy.yaml` file
      exists (S5's `policy.yaml` loader is a future change, out of
      S13's scope per proposal.md) — the fixture expresses the two
      "policies" as in-crate Rust data instead, which is what task 6.1
      can actually exercise given `canon-policy` ships the engine only.
      Green.
- [x] 6.2 Build the type-invalid-rejection fixture: a set of expressions
      each violating one rejection class (undeclared identifier, wrong
      function arity, wrong argument type) — assert each is rejected at
      write time with its expected diagnostic.
      Evidence: `tests/rejection.rs` (see task 3.4's evidence — the same
      fixture satisfies both tasks). Green.
- [x] 6.3 Build the determinism fixture: evaluate a set of CEL expressions
      against fixed input facts twice in the same `canon selftest` run and
      assert byte-identical results — the mechanical purity/determinism
      smoke test design D4/D5's risk mitigations rely on.
      Evidence: `tests/determinism.rs` — 5 expressions × 2 fixture
      records, each evaluated twice (plus a 25-repetition variant) with
      a fixed `now`, asserting byte-identical `PolicyValue`/error output.
      Green. (Now runs BOTH under `cargo test -p canon-policy` and inside
      `canon selftest` — see task 6.4.)
- [x] 6.4 Wire all three fixtures into `canon selftest` (design §8: fixture
      corpora with rebindable roots + expected-output diff), matching the
      pattern S12's context fixture and S4's verdict-mapping golden file
      already establish.
      Evidence: `crates/canon-policy/src/selftest.rs::selftest()` wraps all
      three fixture flows (equivalence 6.1, rejection 6.2, determinism 6.3)
      as the SAME checks those files exercise — each `tests/*.rs` is now a
      thin `#[test]` wrapper over this function, never a second copy. The
      Wave-2 unified `canon selftest` aggregator (`canon_cli::selftest`)
      registers it as the `policy-expressions` suite (3 checks); `canon
      selftest` and `canon selftest --json` run it against the crate's own
      in-memory corpus (side-effect-free, no rebindable root needed —
      every fixture is a synthetic expression + hand-built record).

## 7. Companion skill

- [x] 7.1 Author the `canon-policy` companion skill under `canon/skills/`
      (decision 9): how to write a CEL predicate against `canon context`'s
      `policy` section, how to read a write-time "expected …" diagnostic,
      the closed pure-function allowlist, and the explicit non-CEL
      boundary (reward functions stay versioned Rust, ingest transforms
      stay Rust adapters, evidence records never carry an expression) —
      materialized for Claude Code + Codex only via the content-hash +
      version install lock.
      Evidence: `canon/skills/canon-policy/SKILL.md`. **Note**: documents
      calling `bindings_for` directly (not `canon context`'s CEL
      section, which does not exist yet per task group 5's blockers) —
      updating the skill to point at `canon context` once S12 lands is
      that future change's job. Materialized via `canon skills install
      --source canon/skills --target .`: `.claude/skills/canon-policy/
      SKILL.md` (byte-diff-verified verbatim copy) + `.codex/skills/
      canon-policy.md` (flattened, frontmatter stripped) +
      `canon/skills/.install-lock.json` (new `"canon-policy"` entry,
      version 1, content-hash only — the other four entries unchanged,
      confirmed via `git diff`).

