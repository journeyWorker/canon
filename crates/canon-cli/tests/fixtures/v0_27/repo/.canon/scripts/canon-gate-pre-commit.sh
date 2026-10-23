#!/bin/sh
# canon-gate-pre-commit.sh — generic pre-commit hook for repos with no
# donor-CLI `hook run <kind>` wiring (S5 design decision 8; spec
# `gated-task-completion` "A repo with no donor-CLI hook wiring gets a generic pre-commit
# script"). Mirrors the internal monorepo's own lefthook `pre-commit:` job shape
# (lefthook.yml's `command -v <tool> >/dev/null 2>&1 && <tool> ... ||
# true` advisory idiom) as a single portable script — install it
# directly as `.git/hooks/pre-commit`, or invoke it from a lefthook/
# husky `run:` line.
#
# Embedded into `canon-gate` as `hooks::PRE_COMMIT_SCRIPT`
# (crates/canon-gate/src/hooks.rs) — this file is that constant's
# single source; the two never drift.
#
# Fail-soft when canon itself is missing (this crate's own hooks-fail-
# soft invariant, design doc §7: "hooks fail soft, an internal hook
# error allows the action") — a repo that has not installed canon yet
# is never blocked by its absence.
#
# BLOCKING vs ADVISORY (design decision 8: "advisory vs blocking
# configurable per repo") is one env var: CANON_GATE_ADVISORY=1 (the
# default) never fails the commit even when the gate finds violations;
# set CANON_GATE_ADVISORY=0 to make a gate failure block the commit.
#
# This script ships UNWIRED by this change — no consumer repo's hook
# config is edited here (S5 migration plan step 2, a separate,
# documented follow-up: the internal monorepo's own `.claude/settings.json`/
# `.codex/hooks.json`/lefthook.yml opt-in).
set -eu

if ! command -v canon >/dev/null 2>&1; then
  exit 0
fi

if [ "${CANON_GATE_ADVISORY:-1}" = "1" ]; then
  canon gate check || true
else
  canon gate check
fi
