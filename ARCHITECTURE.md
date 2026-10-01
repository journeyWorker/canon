---
owner: canon-maintainers
review_date: 2026-10-01
review_criteria:
  - crate boundaries match the exported library contracts
  - all five invariants have a checkable evidence path
  - canonical sources, provider projections, and empirical memory remain distinct
---

# Canon architecture

This is the current-contract map for Canon. It is intentionally separate from
historical change plans in `openspec/changes/` and `docs/superpowers/`. The
machine-readable companion is [`canon/knowledge-index.json`](canon/knowledge-index.json);
run `python3 scripts/check-knowledge.py --as-of 2026-10-01` to check its paths,
primary Markdown links, projection ownership, and review dates.

Canon is a **vendor-neutral AI-native SDLC control plane**, not a general agent
runtime. External agents and sandbox providers may change. Canon owns the
intent, canonical knowledge, run/evidence joins, policy and approval boundary,
and measured learning lineage that make those providers accountable.

## Knowledge ownership

The repository has three deliberately different knowledge classes. A consumer
MUST NOT treat one class as another:

1. **Canonical source** is the reviewed source of truth. It includes this
   architecture contract, `canon.yaml`, the typed Rust model and subsystem
   contracts, the feature corpus, and the provider-neutral skill source in
   [`canon/skills/SKILL.src.md`](canon/skills/SKILL.src.md). A source can define
   policy or behavior; its owner and review criteria are recorded in the
   [knowledge index](canon/knowledge-index.json).
2. **Provider projection** is a generated/materialized view for one agent
   provider. `.claude/`, `.codex/`, `.omp/`, and `.pi/` projections are not
   alternate authorities. `canon skills install` derives them from
   `canon/skills/`; `canon skills check` detects drift. Codex is flattened,
   while Claude, OMP, and Pi use directory-shaped entrypoints. OMP and Pi's
   retrieve script is a passive sidecar, not a native hook. The website
   architecture page is a public documentation surface, not this repository's
   contract.
3. **Empirical memory** is evidence-derived experience, not product policy.
   `canon-learn` separates raw `Trajectory` evidence from distilled
   `StrategyItem` memory, retains provenance, retrieves by role/regime, and
   applies promotion/demotion gates. A strategy MAY guide a later run only as
   recorded, reviewable evidence; it MUST NOT silently rewrite canonical
   architecture or policy.

When a projection disagrees with a source, fix or regenerate the projection.
When empirical memory disagrees with a source, quarantine or demote the memory
and review the source; do not resolve the conflict by prompt text alone.

## Subsystem boundaries

The data path is adapter → model → store → gate/report/learn. The boundaries
are code contracts, not merely labels:

- [`canon-model`](crates/canon-model/src/lib.rs) owns the closed, versioned
  record families. Every record carries the shared `Envelope`; typed IDs and
  the join spine connect work, execution, evidence, and memory. JSON Schema and
  `JOIN_SPINE.md` are generated from this Rust source.
- [`canon-ingest`](crates/canon-ingest/src/lib.rs) parses external session,
  artifact, and plan dialects into canonical rows and records. It has no
  storage dependency. `canon-cli` composes ingest with writes.
- [`canon-store`](crates/canon-store/src/lib.rs) owns the `Tier` abstraction,
  `TierPolicy`, registry, atomic writes, cursors, and the single embedded
  DuckDB view layer. `canon.yaml` chooses local/hot/cold routing; a capability
  rung is distinct from its Git/Postgres/S3/SQLite backend.
- [`canon-gate`](crates/canon-gate/src/lib.rs) composes coverage, evidence
  ledger, staleness, trust, task-completion, marker, hook, and risk checks.
  It is the verification boundary, not an agent executor.
- [`canon-policy`](crates/canon-policy/src/lib.rs) provides typed, bounded CEL
  validation/evaluation with a reviewed pure-function allowlist. It is not a
  general scripting or reward-execution surface.
- [`canon-learn`](crates/canon-learn/src/lib.rs) owns trajectory/strategy
  storage, deterministic distillation, role-scoped retrieval, reward and
  promotion/demotion. Its guidance is empirical and provenance-bearing.
- [`canon-report`](crates/canon-report/src/lib.rs) reads one pinned corpus
  through `canon-store`'s view layer and renders reports/snapshots. It is a
  reporting surface, not a second source of truth or a second mart
  implementation.
- [`canon-cli`](crates/canon-cli/src/main.rs) is composition and user-facing
  command wiring. Provider-specific skill materialization lives in
  `crates/canon-cli/src/skills.rs`; it does not change the canonical source.

## Five architecture invariants

These are the public principles. Each has an existing evidence boundary and a
failure mode that must remain visible rather than being hidden in an agent
instruction.

### 1. Every work has intent.

A durable unit of work starts as a `Subject` (title, summary, domain,
accountable role, status) and connects to `Change`, `Scenario`, and `Task`
through typed join IDs. A task's declared scenario/dependency references are
intent declarations; they are not an implicit scheduler. Imported plans remain
plan state until a canonical lifecycle adopts them. Evidence:
[`canon-model/src/records.rs`](crates/canon-model/src/records.rs),
[`specs/`](specs), and the `subject`/plan commands in `canon-cli`.

**Invariant:** a new execution or completion MUST name the work it serves, or
be represented as an explicitly unscoped/root run rather than acquiring an
invented work reference.

### 2. Every run has reproducible context.

`Run` is the run-scoped join point for events and manifests. Its additive
lineage captures provider/model plus `SkillSnapshot`, `ContextSnapshot`, and
`PolicySnapshot`; `injected_guidance` records the exact strategy references
provided at dispatch. Session adapters normalize external transcripts into
this model. Missing optional lineage is preserved as missing data, not guessed
from a provider projection. Evidence:
[`Run` and `RunLineage`](crates/canon-model/src/records.rs) and
[`canon-ingest`](crates/canon-ingest/src/lib.rs).

**Invariant:** replay and evaluation MUST use the recorded run inputs and
injected guidance. A current retrieval result is not a substitute for the
run's historical context.

### 3. Every completion has evidence.

`Task` completion carries an evidence note, while `EvidenceRecord` joins the
claim to task/scenario/run where available and carries verdict, lifecycle,
flag, digest, ordering, surface references, and optional approval. `canon-gate`
checks coverage, ledger state, staleness, trust, and fabrication markers;
malformed evidence is no evidence. A green test command alone is not a
canonical completion claim.

Evidence: [`EvidenceRecord`](crates/canon-model/src/records.rs) and the
[`canon-gate` trust spine](crates/canon-gate/src/lib.rs).

**Invariant:** a completion MUST be backed by the evidence set required by its
policy and risk tier; absent, malformed, stale, or fabricated evidence MUST
remain a visible gate failure.

### 4. Every risky action has accountable approval.

Approval is a typed `EvidenceApproval` attached to evidence; it records
approver, role, and timestamp. The risk gate evaluates effect-aware risk rules
and reuses the closed gate failure vocabulary. This records an attestation,
not a claim that an external identity system has verified a person. Production
or sensitive-data deployments still require the deployment's own identity and
capability controls; a prompt prohibition is not a sandbox.

Evidence: [`EvidenceApproval` and `EvidenceRecord`](crates/canon-model/src/records.rs),
[`canon-gate/src/risk.rs`](crates/canon-gate/src/risk.rs), and the repository
policy sources.

**Invariant:** a risky effect MUST have the policy-required accountable
approval before it is treated as complete; a field declaration alone MUST NOT
be described as enforcement.

### 5. Every learned rule has measured provenance.

Raw `Trajectory` evidence and distilled `StrategyItem` memory are different
layers. Distillation is deterministic and content/time-derived; strategy
entries retain source provenance and role/regime scope. Retrieval excludes
 demoted entries and caps guidance. Promotion uses explicit gates (including
occurrence or paired comparison where configured), and replay uses the
recorded guidance snapshot rather than a fresh lookup.

Evidence: [`canon-learn`](crates/canon-learn/src/lib.rs),
[`strategy`](crates/canon-learn/src/strategy.rs), and
[`trajectory`](crates/canon-learn/src/trajectory.rs).

**Invariant:** no learned rule becomes canonical guidance merely because it is
frequent or recent. It MUST retain source evidence and pass the configured
measurement/promotion boundary; contradiction or regression MUST support
demotion or rollback.

## Storage and lifecycle

`canon.yaml` is the repository configuration source. Durable authored specs,
reviews, divergences, evidence, findings, and strategies route to the local
Git tier; live task/handoff/session/run/event state routes to the hot tier; raw
trajectories route to the cold tier. The exact routing is resolved by
`canon-store`'s `TierPolicy`, not by callers branching on backend names.

Ingest normalizes external dialects before storage. Gates judge canonical
evidence. Learn consumes retained trajectory/verdict data and returns
provenance-bearing guidance. Reports consume a pinned read of the same view
layer. None of these layers may promote a provider projection or empirical
memory into canonical source implicitly.

## Change discipline

Before changing a boundary, update the canonical source and its acceptance
surface, then regenerate provider projections with `canon skills install` as
appropriate. Update the knowledge index when a source, projection, or memory
path changes. Keep historical change documents as historical records; do not
rewrite them to make this map appear current.

The optional execution-provider and multi-agent directions from the audit are
not current Canon runtime contracts. They remain disabled unless paired
behavioral evidence demonstrates a measurable, risk-adjusted improvement and
the corresponding approval/security boundary is implemented.
