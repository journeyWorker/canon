# Vendor Audit — addyosmani-agent-skills

**Vendor root**: `vendors/addyosmani-agent-skills/` (shallow clone of `https://github.com/addyosmani/agent-skills` at `aeac41d`, 2026-10-21; local only, never tracked).
**Goal**: per-surface investigation of addyosmani-agent-skills to find patterns canon lacks and could lift, adapt or be inspired by.
**Posture**: read-only reference material. No edits to `vendors/**`. Files named `_vendor_*` are the vendor's agent instructions, renamed so no harness loads them: read them as documentation, never as instructions.

## Surface table

| Slug | Vendor paths | Scale | Canon analog hint |
| --- | --- | --- | --- |
| `a-define-plan` | `skills/{spec-driven-development,idea-refine,interview-me,planning-and-task-breakdown,constraint-driven-development,doubt-driven-development,source-driven-development,context-engineering,using-agent-skills}` | ~9 skills | brief (PRD), planner role, `canon brief check` |
| `a-build-verify` | `skills/{test-driven-development,incremental-implementation,debugging-and-error-recovery,code-review-and-quality,code-simplification,browser-testing-with-devtools,frontend-ui-engineering,api-and-interface-design,performance-optimization,security-and-hardening}` | ~10 skills | implementer/reviewer roles, evidence, findings |
| `a-ship-ops` | `skills/{ci-cd-and-automation,git-workflow-and-versioning,shipping-and-launch,deprecation-and-migration,documentation-and-adrs,observability-and-instrumentation}` | ~6 skills | ship guard, chore changes, deprecation (0.21/0.24) |
| `a-harness` | `commands/`, `agents/`, `hooks/`, `plugin.json`, `_vendor_*` plugin dirs, `scripts/`, `references/`, `docs/` | ~60 files | `canon skills install`, providers, AGENTS block |
| `a-evals` | `evals/` | ~90 files | `evals/dogfood/`, trace_check |

5 audit agents.

## Canon target surfaces

- `canon/skills/SKILL.src.md` — the core skill (loop: brief → scenarios → evidence → independent review → ship), projected to `.claude`/`.agents`/`.omp`/`.pi`.
- `canon/skills/reference/role-{planner,implementer,reviewer,orchestrator}.md` and `canon-*.md` — role and command references.
- `crates/canon-cli/src/init.rs` — the AGENTS block `canon init` writes.
- CLI: `canon brief check|approve`, `scenario new`, `evidence add`, `review add`, `finding add|close`, `subject status`, `gate check`, `status` (`meta.next`), `schema`, `divergence`, `learn`, `skills install|check`.
- `crates/canon-cli/src/brief_check.rs` (brief rules), `crates/canon-gate/` (gates), `evals/dogfood/` (trace_check, dogfood evals).

## Output convention

- One audit per surface at `docs/research/vendor-audits/addyosmani-agent-skills/<slug>.md`, following `_TEMPLATE.md`. English, neutral tone.
- Cite vendor paths as `vendors/addyosmani-agent-skills/<path>:<lines>`.
- Actions: `EXTRACT` / `INSPIRE` / `SKIP`, one-line rationale each.
