# canon CLI conformance corpus

This directory freezes the **external contracts** of the `canon` CLI — the
things scripts, CI jobs, git hooks, and other repositories rely on without
reading canon's source:

- **exit codes** (0 clean, 1 gate red / refusal, 2 usage error, …),
- the gate's **violation lines**,
- **refusal messages** on stderr,
- **JSON output** (`--json`).

Each case is a frozen fixture repository plus an ordered list of `canon`
invocations, each with its expected exit code, stdout, and stderr. Unit tests
pin internals; this corpus pins what a consumer actually sees. It does not
exercise network tiers (Postgres/R2).

CI enforces it through the cargo integration test
`crates/canon-cli/tests/conformance.rs`, so `cargo test --workspace` fails on
any drift.

## Layout

```
conformance/
  README.md            # this file
  regenerate.sh        # the only bless entry point
  cases/<name>/
    case.yaml          # description, env, steps
    repo/              # the fixture repository (optional), copied to a temp dir
    expected/<n>.out   # normalized stdout of step n (1-based)
    expected/<n>.err   # normalized stderr of step n (1-based)
```

`case.yaml` (unknown keys are rejected):

```yaml
description: one sentence naming the contract this case pins
env: { CANON_ACTOR: canon }       # optional; merged over the fixed base env
steps:
  - args: [gate, check]
    exit: 1
```

## How a case runs

1. Cases are **discovered, never listed**: every `cases/*/case.yaml`, in name
   order. A directory under `cases/` without a `case.yaml` is an error.
   **Zero discovered cases fails the run**, so a moved or deleted corpus
   cannot pass silently.
2. `repo/` (if present) is copied into a fresh temp dir, which is then
   `git init`ed (canon reads Git). An absent `repo/` means an empty repo.
3. Each step runs the built `canon` binary with `args`, from the temp repo,
   with a **cleared environment** plus a fixed base — `PATH` (inherited),
   `HOME` (a fresh empty temp dir), `LC_ALL=C`, `TZ=UTC` — with the case's
   `env` merged over it. No `CANON_*` variable is ever inherited.
4. The exit code must equal the step's `exit`; the normalized stdout and
   stderr must equal `expected/<n>.out` and `expected/<n>.err` byte for byte.
   Both files exist for every step, empty when the stream is empty. An
   `expected/` file with no matching step is an error.
5. On mismatch the runner prints a unified diff (`-` expected, `+` actual)
   per stream and the bless command.

## Normalization rules

Outputs carry values that are legitimately non-deterministic. Exactly these
rules are applied, in this order, to both stdout and stderr; nothing else is
rewritten, so anything a consumer could parse is compared exactly:

1. The **canonicalized** temp repo path (on macOS `/private/var/...`) → `<repo>`.
2. The temp repo path **as created** (on macOS `/var/...`) → `<repo>`.
3. **RFC3339 timestamps** — `YYYY-MM-DDTHH:MM:SS`, optional `.fraction`, then
   `Z` or `±HH:MM`, standing alone (no ASCII letter/digit on either side) → `<ts>`.
4. **ULIDs** — exactly 26 Crockford base32 characters (`0-9`, `A-Z` without
   `I L O U`), the first `0`–`7`, standing alone (no ASCII letter/digit on
   either side) → `<ulid>`.
5. **Ledger record digests** — the 12 lowercase hex characters in a ledger
   filename's `__<12 hex>.json` suffix → `__<digest12>.json`.

A plain date, a full Git SHA, a task id, or any other value is never
normalized. If a new case needs another rule, that is a change to this list
and to `conformance.rs` together, reviewed as a contract change.

## Adding a case

1. Create `cases/<name>/case.yaml` naming the one contract it pins, and the
   fixture under `cases/<name>/repo/` (hand-authored files). The root
   `.gitignore`'s unanchored patterns (`.env*`, `*.duckdb`, `target/`,
   `dist/`, `node_modules/`, …) apply inside fixtures too; confirm with
   `git status --ignored conformance/` that every fixture file is tracked.
2. Set each step's `exit` **by hand** from the contract, not from what the
   binary happens to return.
3. Run `conformance/regenerate.sh` to write `expected/`.
4. Read every written file and check it against the code that produces it.
   An expectation is derived from the real binary, never hand-typed — and
   never accepted unread.

## Blessing

```sh
conformance/regenerate.sh            # = CANON_CONFORMANCE_BLESS=1 cargo test -p canon-cli --test conformance
```

Bless mode replaces each case's `expected/*.out|*.err` with the current
normalized output instead of comparing. It does **not** touch `case.yaml`:
an exit code mismatch still fails in bless mode, and fixing it means editing
`exit:` by hand — a deliberate decision, not a side effect of re-blessing.

**A bless that changes an existing expectation is a contract change.** Review
`git diff conformance/`, and name every changed expectation in the release
notes. A bless that only adds files for a new case is not.

**Hand-built refusal fixtures are never regenerated.** Malformed records,
broken policy files, and other deliberately invalid inputs under
`cases/*/repo/` are authored by hand; no script or code path rewrites them.
Only `expected/` is ever blessed.

## The version case (live stamp)

`cases/version` pins `canon --version` to the workspace version, and the
runner separately asserts that the blessed `expected/1.out` equals
`canon <CARGO_PKG_VERSION>`. A release bump without a re-bless therefore
fails with a pointed message, so the corpus is always stamped with the
version it describes.
