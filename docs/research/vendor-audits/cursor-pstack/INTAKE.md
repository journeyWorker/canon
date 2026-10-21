# Intake — cursor-pstack

**GO.**

One engineer's Cursor plugin: a mode skill that picks playbooks, ~25 principle skills, verification-skill generators, multi-model workflows and overnight automations. Health: cursor/plugins: 13k stars, 28 contributors, ≥100 commits in 90 days (≥100 touching pstack), pushed 2026-10-20. License: MIT (pstack/LICENSE, Lauren Tan).

Why canon should look: Strongest on verification and multi-agent rigor: it generates per-repo verification skills (`skills/create-verification-skill/`), adversarial review (`skills/interrogate/`, `skills/arena/`), multi-model routing (`skills/poteto-mode/`). These map onto canon's reviewer role and evidence model.

Posture: read-only reference, cloned shallow at `vendors/cursor-plugins/pstack` (local only; `vendors/` is gitignored in canon). Agent surfaces renamed with the `_vendor_` prefix so they are never loaded as instructions. Adoption is clean-room reimplementation from these audits.
