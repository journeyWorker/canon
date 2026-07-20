## Context

Design doc §5 S10 (`docs/superpowers/specs/2026-07-10-canon-design.md:331-345`):
The donor vocabulary project's plugin system, adopted nearly as-is — `plugin.yaml` + `directives/*.yaml` +
`enums.yaml` manifests declare the vocabulary; a capability-snapshot resolution
(the donor vocabulary project's `build_input`/`fold_env` shape) feeds checker, `canon context`, and (later)
LSP from one source. The checker validates authored task/spec atoms with "expected
one of: …" diagnostics. Target: replace freeform `tasks.md` task lines with typed
task atoms that carry their own evidence requirements (closing the loop with S5);
handoff body templates (S1) also become vocabulary-defined here. Sequenced last
(wave W4) but not dropped (decision 8). Acceptance: a canon-vocabulary tasks file
validates, compiles to the S1 model, and round-trips; one consumer repo authors a
real change with it.

Donor (§3): The donor vocabulary project — "Typed authoring vocabulary: plugin
manifests declare directives+enums in YAML, checker validates, LSP serves — the S10
model." The donor CLI's context command (its `run_context` entry point) is separately cited
as the S12 model; S10 borrows the same resolution primitive S12 consumes.

Verified the donor vocabulary project shapes this design lifts:

- **`plugin.yaml`** (the donor's core plugin manifest): `id`,
  `version`, `kind`, `exports: {directives: <dir>/, enums: <file>.yaml, …}`. A
  project-level plugin adds `depends: [{id, range}]` and `options:
  [{name, type, default}]` (an example project's plugin manifest).
- **`directives/*.yaml`** (the donor's core plugin `directives/staging.yaml`): a
  list of `{name, attrs: [{name, type, default?}], semantics?,
  lower}` entries. `type` is one of a bare scalar (`string`, `number`, `bool`), an
  inline `{enum: […]}`, or a `{list: <type>}`.
- **`enums.yaml`** (the donor's core plugin `enums.yaml`): a flat map of
  enum name → member list.
- **The donor's project manifest** (an example project manifest):
  `pluginsDir`, `defaultProfile`, `profiles: {name: {extends?, plugins: {id: true
  | {option: value, …}}}}` — a profile activates a set of plugins with optional
  per-plugin option overrides.
- **Capability-snapshot resolution**: the donor CLI's entry point `build_input`
  (lines 211-263) resolves `--project`'s the donor's project manifest + the scene's own
  frontmatter `profile`/`plugins` keys into one `CapabilitySnapshot` via
  `resolve_document_snapshot`, then threads that SAME snapshot into
  the donor checker (the checker), `run_context` (the CLI's context command —
  S12's donor), and (not yet built) the LSP. The donor checker's fold step's
  `fold_env` (line 198) is the pure, total, side-effect-free folding step
  consuming a resolved snapshot.
- **Diagnostic shape**: the donor checker's directive validator `check_directive`
  (lines 66-155) emits `E-UNKNOWN-DIRECTIVE` (with an "activate plugin" fix-it
  when the tag is installed-but-inactive, plugin §11.2), `E-UNKNOWN-ATTR`,
  `E-MISSING-ATTR`, and — at line 264-267 — the enum-violation message format
  this change lifts verbatim: `` `{got}` is not a valid value for `{key}` of
  `::{tag}` (expected one of: {members}) ``.

Prerequisites from earlier waves (S10 is W4, last): S1 supplies the `Task` and
`EvidenceRecord` model + join spine this change's typed atoms compile to and the
handoff body template registry it now drives; S5 supplies the policy-derived
evidence requirements (`policy.yaml`) a typed atom's evidence attribute must
resolve against, and the evidence-gated flip (`flipTaskDone`/`scanFakeMarkers`,
the donor CLI) this change adds a typed path
alongside; S12 (`canon context`) is the consumer that turns this change's
capability snapshot into an agent-facing authoring surface.

## Goals / Non-Goals

**Goals:**

- One manifest format (`plugin.yaml` + `directives/*.yaml` + `enums.yaml`)
  declares canon's own authoring vocabulary (task atoms, handoff body templates),
  structurally identical to the donor vocabulary project's, so the donor vocabulary project's checker algorithm and diagnostic
  shape transfer with minimal adaptation.
- Exactly one capability-snapshot resolution function; the checker, `canon
  context` (S12), and a documented (not yet built) LSP extension point all
  consume its output — no second registration site where a vocabulary rule could
  be declared differently.
- A typed task atom's evidence requirement is a structured, checker-validated
  attribute resolved against S5's own `policy.yaml`-derived requirement domain —
  never a free string a human must interpret.
- A canon-vocabulary tasks file validates, compiles to the existing S1 `Task`
  model, and round-trips (compile → decompile reproduces an equivalent atom).
- Handoff body templates (S1's per-domain registry) are declared and validated
  through the same manifest/resolution/diagnostic machinery as task atoms — one
  vocabulary system, two authoring domains.
- One real consumer-repo change is authored with the typed vocabulary, proving
  the mechanism end to end.

**Non-Goals:**

- Migrating every existing openspec change's `tasks.md` to the typed format. This
  change proves the mechanism on one pilot change; a repo-wide cutover is a
  future decision, not S10's acceptance bar.
- Removing or disabling the existing freeform `tasks.md` checkbox grammar or S5's
  free-string `--verify-via` evidence path. The typed path is additive.
- Building an LSP. The capability-snapshot resolution is structured so a later
  LSP can consume it (an explicit, documented public entry point), but no LSP
  server ships in this change (design doc: "and (later) LSP").
- Lifting the donor vocabulary project's scene-script text DSL (the donor's scene-DSL parser crate's shot/track/timeline/CEL
  grammar) or CEL expression slots (the donor's CEL-binding crate). That parser is purpose-built for
  VN scene structure and has no task-atom or handoff-template analog; only the
  **manifest declaration format + resolution + validation algorithm** are lifted,
  not the scene-script surface syntax.
- Deciding the plugin-system lift mechanism (git/path dependency on the donor vocabulary project crates
  vs. importing the source). Tracked as an explicit open question below per
  design doc §10 Q5 — decided at S10 kickoff, not by this design.

## Decisions

### D1. Manifest format is lifted structurally as-is, retargeted at canon's own vocabulary domain

- **Decision:** canon defines its own core plugin (analogous to the donor's core plugin) —
  `canon/vocab/canon.core/plugin.yaml` — with `exports: {directives:
  directives/, enums: enums.yaml}`, exactly the donor vocabulary project's `plugin.yaml` shape
  (`id`, `version`, `kind`, `exports`). Its `directives/` declares two directive
  families: **task-atom directives** (one directive per task "kind" a repo wants
  to distinguish, minimally a `task` directive) and **handoff-template
  directives** (one directive per domain: `handoff-dev`, `handoff-design`,
  `handoff-content`, `handoff-test`, …). `enums.yaml` declares shared enums
  (task `status`, evidence `kind`, handoff domain names). Consumer repos extend
  the vocabulary with their own `canon/vocab/<id>/plugin.yaml` the same way a
  donor project's `plugins/` directory adds project-specific directives, resolved
  via a `canon.project.yaml` (or `.canon/canon.yaml`-embedded) analog of
  the donor's project manifest's `pluginsDir`/`profiles` shape.
- **Why:** decision 10 explicitly calls for a near-as-is lift (user-owned code,
  no clean-room constraint); reusing the identical manifest shape means the donor vocabulary project's
  resolution and validation algorithms transfer with only the vocabulary content
  (which directives/enums exist), not their structure, changing.
- **Alternatives:** design a canon-native manifest format from scratch (rejected
  — decision 10 is explicit, and there is no benefit: the donor vocabulary project's shape already
  cleanly separates "what a directive accepts" from "how a project activates
  plugins," which is exactly canon's task-atom/handoff-template problem too).

### D2. Task atoms and handoff-template bodies are authored as YAML validated against the snapshot — the donor vocabulary project's scene-script DSL is not lifted

- **Decision:** a typed task atom is one YAML record — `{id, tag, attrs}` where
  `tag` names a directive (e.g. `task`) and `attrs` supplies that directive's
  declared attributes (`desc`, `owner?`, `status`, `evidence`). A handoff body is
  likewise `{tag: "handoff-<domain>", attrs: {…}}`. Both are validated by
  resolving `tag` against the capability snapshot's directive index exactly as
  `check_directive` does (unknown tag → `E-UNKNOWN-DIRECTIVE`; unknown attr →
  `E-UNKNOWN-ATTR`; missing required attr → `E-MISSING-ATTR`; bad enum value →
  the "expected one of: …" message, lifted verbatim from
  the donor checker's enum-violation message). This is a NEW, canon-authored YAML
  parser — it does not reuse the donor's scene-DSL parser crate's line-based scene DSL (`::tag
  attr=value` inline directives, shots, tracks, timelines, CEL slots), because
  that grammar exists to express VN scene structure canon's task/handoff domain
  has no analog for.
- **Why:** the valuable, generalizable part of the donor vocabulary project's system is the manifest
  declaration format + the snapshot-resolution + the validation algorithm and
  diagnostic shape — all domain-agnostic. The donor's scene-script parser is
  domain-specific (shots/tracks/timelines) and pulling it in would import a large
  amount of irrelevant surface area for a flat list of typed records. YAML is
  also already canon's manifest format (`plugin.yaml`, `directives/*.yaml`,
  `canon.yaml`), so task atoms and handoff bodies stay in the same file family
  the rest of canon's config already uses.
- **Alternatives:** reuse the donor's scene-DSL parser crate's directive-line grammar verbatim
  (rejected — drags in shot/track/timeline/CEL concepts with no task-atom
  equivalent, and would make canon depend on the donor vocabulary project's scene-parser surface for a
  feature that needs none of it); invent a bespoke non-YAML text format
  (rejected — no reuse benefit over YAML, and diverges from canon's own manifest
  file family for no reason).

### D3. Capability-snapshot resolution is one function; checker, `canon context`, and a future LSP all consume it

- **Decision:** a new crate (e.g. `canon-vocab`) exposes one resolution entry
  point — `resolve_snapshot(project_dir, profile) -> (CapabilitySnapshot,
  Vec<Diagnostic>)` — structurally mirroring `resolve_document_snapshot` +
  `fold_env` (the donor checker's fold step, pure and total, never panics). The
  checker calls it to validate a task-atom/handoff-body file; S12's `canon
  context` calls the SAME function and serializes its directive/enum index as
  the authoring surface; the crate documents (doc comment + a `pub` boundary,
  not a trait with no implementers) the extension point a later LSP will call
  identically. No consumer computes its own partial vocabulary view.
- **Why:** this is the design doc's stated invariant for both S10 and S12: "a
  capability query, not validation… built from the SAME schema registry +
  policy resolution the validator uses, so the surface can never diverge from
  what `canon fmt`/`canon gate` enforce" (§5 S12). Splitting resolution logic
  between the checker and `canon context` would recreate exactly the drift risk
  that invariant exists to prevent.
- **Alternatives:** let `canon context` re-derive its own view from the raw
  manifests independently of the checker (rejected — the whole point of lifting
  the donor vocabulary project's architecture is the single-resolution-source guarantee; a second
  resolution path is a second place to be wrong).

### D4. Evidence requirement is a new, S5-backed attribute type — not a free string

- **Decision:** the attribute type system gains one addition beyond the donor vocabulary project's
  scalar/enum/list types: an `evidence` type, whose domain is resolved from S5's
  own parsed `policy.yaml` (the same parse S5's gate uses, not a duplicate
  parser) rather than a locally-declared enum. A task atom's `evidence` attribute
  is therefore `{kind: <S1 EvidenceRecord kind>, ref: <string, meaning depends on
  kind>}`, and an unrecognized `kind` produces the same "expected one of: …"
  diagnostic as any other enum violation, listing the kinds S5's policy actually
  recognizes for that task's tags. `canon gate task <task_id>` (S5) gains a typed
  path: given a typed atom, it checks for an `EvidenceRecord` matching the atom's
  declared `evidence.kind`/`ref` instead of only accepting an arbitrary
  `--verify-via` string.
- **Why:** this is the design doc's explicit "closing the loop with S5" — S5's
  evidence-gated flip today can only confirm *some* evidence string was
  supplied, never that it is evidence *of the kind the task actually needs*.
  Resolving the `evidence` type's domain from S5's live policy (not a copy) keeps
  one source of truth for "what evidence satisfies what," matching D3's
  single-resolution principle.
- **Alternatives:** keep evidence as an untyped free string on the typed atom too
  (rejected — this is precisely the freeform-tasks.md problem S10 exists to
  fix, just moved into a new file format); declare evidence kinds as a
  vocabulary-local enum duplicating S5's policy (rejected — two places to update
  when a policy changes, exactly what D3 rejects for the directive/enum index).

### D5. Handoff body templates are directives, not a second schema mechanism

- **Decision:** S1's per-domain handoff body template registry
  (기획/디자인/개발/테스트/…, referenced from `canon.yaml`) is implemented as one
  directive per domain in canon's core plugin (`handoff-dev`, `handoff-design`,
  …), validated and rendered through the exact same
  resolve-snapshot → validate-attrs pipeline as task atoms. A handoff's `domain`
  field selects the directive tag; its body is the directive's attrs.
- **Why:** the design doc explicitly ties this to S10 ("handoff body templates
  (S1) also become vocabulary-defined here") rather than proposing a separate
  templating mechanism; reusing directives means a new domain template is
  "add one more `directives/*.yaml` entry," not a new subsystem.
- **Alternatives:** a dedicated template-string mechanism (e.g. Handlebars/Jinja
  body templates) separate from the directive/attr system (rejected — would be
  a second, parallel vocabulary mechanism inside the same change that argues for
  exactly one).

### D6. Diagnostic codes and message format are lifted verbatim, then become canon's own stable failure classes

- **Decision:** `E-UNKNOWN-DIRECTIVE`, `E-UNKNOWN-ATTR`, `E-MISSING-ATTR`, and the
  enum "expected one of: …" message text are lifted unchanged from
  the donor checker's directive validator. From the moment this change lands, they are
  canon's own stable failure-class strings under design doc §7's rule (never
  renamed without migrating both fixtures and hooks that grep them) — not an
  ongoing sync target with the donor vocabulary project's codes if the donor vocabulary project's own error vocabulary evolves
  later.
- **Why:** identical codes/messages mean the donor vocabulary project's existing test fixtures and
  authoring-error intuition transfer directly; freezing them as canon's own the
  moment they land avoids an ongoing coupling to the donor vocabulary project's error-code choices that
  §7's stability rule would otherwise be violated by on every the donor vocabulary project upgrade.
- **Alternatives:** namespace canon's codes distinctly from day one (e.g.
  `CANON-E-…`, rejected — pure churn with no benefit for a first-class "adopted
  nearly as-is" lift); keep re-syncing to the donor vocabulary project's codes on every the donor vocabulary project version bump
  (rejected — directly conflicts with §7's stable-failure-class rule).

### D7. Conditional handoff/task-atom sections MAY use `applies_when` (S13 CEL) — not built in this change

- **Decision:** where a task-atom directive (D2) or a handoff-template
  directive (D5) later needs a conditionally-included attribute/section
  (e.g. a handoff domain's optional block that only applies under certain
  repo/task facts), the attribute-type system's extension point is S13's
  `applies_when` CEL predicate — design doc §5 S13: "S1/S10 template
  `applies_when` — conditional handoff-template and task-atom sections
  (precedent: the donor monorepo spaces-lens validates `applies_when` CEL at write
  time)" — not a bespoke boolean-flag mini-DSL invented inside
  `canon-vocab`. This change does not implement `applies_when` evaluation
  itself (S13 owns the shared `canon-policy` expression engine and its
  bindings); it only reserves the pointer so a future attribute-type
  addition is `Type::AppliesWhen` resolved through `canon-policy`, never a
  second, parallel conditional-expression mechanism.
- **Why:** design decision 12 and §5 S13 name `applies_when` as CEL's
  cross-cutting use for exactly S1/S10 template conditionals; recording
  the pointer now in D-form (beside D2/D5, the directives it would apply
  to) prevents a future S10 revision from inventing an ad hoc conditional
  syntax the moment the first real "only for this domain" case appears.
- **Alternatives:** invent a local conditional syntax scoped to
  `canon-vocab` (rejected — duplicates S13's shared expression engine for
  the exact use case §5 S13 already claims for S10).

## Risks / Trade-offs

- **[Risk] A second authoring format (typed YAML atoms) beside the existing
  freeform `tasks.md` recreates the "three uncoordinated systems" problem §1
  warns about, if adoption stalls half-migrated.** → **Mitigation:** scoped
  explicitly as additive/opt-in (Non-Goals): this change's acceptance bar is one
  pilot consumer-repo change, not a repo-wide cutover; a future change makes the
  cutover decision deliberately, with both formats coexisting and clearly
  labeled until then.
- **[Risk] The evidence-kind domain (D4) is resolved from S5's `policy.yaml` at
  authoring time; if a repo's policy changes between authoring and gating, a
  previously-valid typed atom could reference a since-removed evidence kind.** →
  **Mitigation:** `canon gate task <task_id>` re-resolves the snapshot (and
  therefore the evidence domain) at gate time, not authoring time — exactly the
  same "policy is the live source of truth" posture S5 already has for its own
  free-string evidence checks; a stale-kind atom fails the gate with a normal
  `E-UNKNOWN-EVIDENCE-KIND`-class diagnostic, not a silent pass.
- **[Risk] Lifting the donor vocabulary project code without a clean-room boundary (decision 10 permits
  this) still means canon inherits any bug in the lifted algorithm.** →
  **Mitigation:** D6's fixture/diagnostic freeze plus this change's own selftest
  fixtures (design §8) re-validate the lifted checker against canon's own corpus,
  independent of the donor vocabulary project's own test suite continuing to pass.
- **[Trade-off] Not lifting the donor's scene-DSL parser crate's scene DSL (D2) means canon cannot
  directly reuse the donor vocabulary project's LSP either, once one exists — a canon LSP will need its
  own text-position mapping for YAML, not the donor vocabulary project's line-based directive spans.** →
  Accepted: D3 keeps the resolution function reusable regardless of surface
  syntax; only the (unbuilt) LSP's syntax-to-position layer is genuinely
  YAML-specific, and that is explicitly out of scope for this change.

## Migration Plan

Net-new authoring surface; no prior canon vocabulary exists to migrate from.
Sequencing within the change: (1) manifest format + capability-snapshot
resolution (D1/D3) lands and is unit-tested against canon's own core plugin
before any consumer-facing atom format exists; (2) the checker + diagnostics
(D6) land against that resolution; (3) typed task atoms (D2/D4) and handoff
templates (D5) land last, each with its own compile-to-S1-model +
round-trip test; (4) the pilot consumer-repo change is authored only after all
three are individually green. No existing repo's `tasks.md` or handoff rows are
touched by this change.

## Open Questions

1. ~~**Plugin-system lift mechanism (design doc §10 Q5):** depend on the donor vocabulary project
   crates (git/path dependency, or a published version) vs. import the
   source into canon's own workspace — decided at S10 kickoff.~~
   **RESOLVED (2026-07-10, crate-graph audit):** the git-dep-vs-source-
   import binary was false — the crate-dependency graph (verified against
   every crate's `Cargo.toml` by 8 independent surface audits) means the
   right answer is per-crate, not uniform. **Lift** the donor's manifest crate +
   the donor's span crate only (leaf-plus-one-hop: the donor's manifest crate depends on
   the donor's span crate alone, nothing from the donor's checker crate/the donor's CEL-binding crate/
   the donor's scene-DSL parser crate — git/path-dep or verbatim source-import are equally
   cheap for these two specifically, pick whichever matches canon's
   release-cadence tolerance). **INSPIRE-only** (reimplement the
   architecture, no crate dependency at all) for the donor's checker crate (fold_env/
   check() shape, "expected one of" sourcing discipline), the donor CLI
   (`run_context`/`authoring_surface`/`context_outline` — it's a `[[bin]]`,
   not a lib, so it cannot be a crate dep regardless), and the donor's compile crate
   (port only the ~50-LOC `attr_json`+`resolve_effect` passthrough
   pattern) — git-dep'ing the donor's checker crate mandatorily drags in the donor's scene-DSL parser crate
   (a DSL parser canon doesn't want, D2) and the donor's CEL-binding crate+`cel-parser` (an
   expression engine canon doesn't need here, S13 owns CEL separately).
   **SKIP** the donor's scene-DSL parser crate/the donor's CEL-binding crate/the donor's tree-sitter grammar/the donor's LSP crate entirely
   for the initial lift. See
   the donor adoption brief §1 for the full
   crate-graph evidence and per-crate rationale.
