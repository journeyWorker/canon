# Intake — addyosmani-agent-skills

**GO.**

Production-grade engineering skills packaged as a lifecycle (define → plan → build → verify → review → ship), with slash commands, hooks, agents and an eval harness. Health: 109k stars, 81 contributors, ≥100 commits in the last 90 days, pushed 2026-10-13. License: MIT.

Why canon should look: It is the closest analogue to canon's own loop (spec before code, small slices, tests as proof, review gate, ship) but expressed as skills only, with no ledger. Worth checking which gates it encodes in prose that canon either enforces mechanically or misses: `skills/spec-driven-development/SKILL.md`, `skills/doubt-driven-development/SKILL.md`, `hooks/`, `evals/`.

Posture: read-only reference, cloned shallow at `vendors/addyosmani-agent-skills` (local only; `vendors/` is gitignored in canon). Agent surfaces renamed with the `_vendor_` prefix so they are never loaded as instructions. Adoption is clean-room reimplementation from these audits.
