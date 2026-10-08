#!/usr/bin/env bash
# The only bless entry point for the canon CLI conformance corpus
# (conformance/README.md). Replays every case and writes its normalized
# stdout/stderr to cases/<name>/expected/<n>.out|.err instead of comparing.
#
# A bless that changes an existing expectation is a contract change: review
# the resulting `git diff conformance/` and name it in the release notes.
# Exit codes live in case.yaml and are never blessed; hand-built refusal
# fixtures under cases/*/repo/ are never regenerated.
set -euo pipefail
cd "$(dirname "$0")/.."
CANON_CONFORMANCE_BLESS=1 cargo test -p canon-cli --test conformance "$@"
git status --short -- conformance/
