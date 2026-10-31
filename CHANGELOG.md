# Changelog

The full release notes before 0.32 are readable with `git show e304d13b:README.md`.

## Unreleased

### Breaking

canon's `--json` output and error contract now follow atip/1 L0 (https://github.com/journeyWorker/atip, spec sections 4 to 8). The old names below are deleted: there are no aliases, and every consumer in this repository (the guides the binary carries, the website in both languages, the dashboard, the dogfood evals, the conformance corpus, the command spec file) moved in the same release.

**The envelope.** `meta` holds only the members the contract defines and canon's own `x-canon`; a member with no value is absent (never `null`, never `[]`).

| 0.32 | 0.33 |
|---|---|
| `meta.canonVersion` | `meta.toolVersion`; new `meta.tool` (`canon`) and `meta.contract` (`atip/1`) on every envelope. `data.canonVersion` of `canon status` is a member of that report and stays |
| `meta.command` `"canon"` when nothing resolved | `meta.command` is absent when not even the first word resolved; the path resolved so far otherwise (`canon finding nosuch` → `finding`) |
| `meta.context.role`, `meta.context.actor` (and `null`s) | gone; `meta.context` is `{repo}` (a path relative to the working directory), present for a command that takes `--repo` |
| `meta.returned`, `meta.total`, `meta.nextCursor`, `meta.hasMore` | `meta.page` = `{returned, total, nextCursor?}`; `nextCursor` present means more follow and `meta.next` holds the same call plus `--cursor <nextCursor>` |
| `meta.unchanged: true` | `data.unchanged: true` of a write's report (a report that is a stored record, `canon subject new/adopt/move/unadopt`, or not an object: `meta.x-canon.unchanged`) |
| `meta.waitingOn`, `meta.nextOmitted` | `data.waitingOn`, `data.nextOmitted` (same rule: `meta.x-canon` for a record); `canon status` has them in `data` too |
| `meta.hotStore` of `canon subject …` | `meta.x-canon.hotStore` (the data is the stored record) |
| `meta.note` of `canon scenario new` | `data.note` |
| `meta.warnings: ["text"]` | `meta.warnings: [{"code", "message", "field"?}]`; every warning situation has a registered code (listed below) |
| `meta.next[].why` | `meta.next[].reason` |
| `meta.next[].actor`, `.role` | `meta.next[].x-canon.actor`, `.x-canon.role` |
| `meta.next[]` with `argv: []` for a command holding a `<placeholder>` | not an entry: the guidance is in the error `hint` or the report text (text mode still shows it) |
| `meta.next[].argv` without `--json` | in JSON mode every entry's `argv` carries `--json` where the command takes it |
| `meta.next: []` on a write | absent |
| `error.hint: null`, `error.suggestions: []` | absent |
| `meta.output` | new: the report's name (`BriefApproveReport`, `InitCheckReport`, `BatchFindingCloseReport`) when the command declares several outputs |
| the canon-specific envelope schemas (`envelopes` of `docs/cli/canon.cli.json`) | removed; the contract's `envelope.schema.json`, `error.schema.json` and `registry-entry.schema.json` are vendored unchanged in `crates/atip/schema/` and pinned by digest; `docs/cli/canon.cli.json` has `"contract": "atip/1"` |

Every report of a command that writes, and `canon status`, gained the optional members `unchanged`, `waitingOn` and `nextOmitted`, so their `schemaVersion` is `1.1` (`BatchReport` too, which also reports `unchanged` when every element did). `canon scenario new` also gained `data.note`.

**Wrong calls.** `--json` is accepted anywhere before `--`, at the root and on groups too (`canon --json`, `canon finding --json`).

| Call | 0.32 | 0.33 |
|---|---|---|
| an unknown command | `USAGE` (clap's text in text mode) | `UNKNOWN_COMMAND`, `field` `command`, `expected` the valid words, `received`, `suggestions` |
| no command (`canon`, `canon finding`) | `USAGE` | `COMMAND_REQUIRED`, `expected` the commands of that level |
| an unknown option, a missing value, a bad `--fields` name | `USAGE`, or `UNKNOWN_FIELD` for `--fields` | `USAGE`, `field` the argument; a bad `--fields` name lists the selectable fields in `expected` |
| a bug (panic) | exit 101 | `INTERNAL`, exit 1, the message says how to report it; a closed stdout still ends the process by SIGPIPE (a signal exit, outside the registry) |

In text mode every error leaves stdout empty and opens stderr with `canon: <CODE>: <message>`, then indented `field:`, `expected:`, `suggestions:` and `hint:` lines (it was `canon <command path>: <CODE> — <message>` and `hint: …`).

**Error codes.** The old names are deleted.

| 0.32 | 0.33 | exit |
|---|---|---|
| `PLAN_CHANGED` | `PLAN_STALE` | 2 |
| `INPUT_ELEMENT_FAILED` | `BATCH_INCOMPLETE` (data shape and pointers unchanged; a crashed element's own error is `INTERNAL`) | 1 |
| `INPUT_BATCH_UNSUPPORTED`, `INPUT_INVALID_TYPE`, `INPUT_UNKNOWN_FIELD`, `INPUT_UNREADABLE`, `INPUT_STDIN_TTY` | `INPUT_INVALID` (the difference stays in `message` and `field`; `field` of a terminal stdin is `--input`) | 2 |
| `INPUT_STDIN_CONFLICT` | `USAGE`, `field` `--input` | 2 |
| `INPUT_INVALID`, `INPUT_CONFLICT`, `INPUT_TOO_LARGE` | unchanged | 2 |
| `INVALID_CURSOR` | `CURSOR_INVALID` (`field` `--cursor`) | 2 |
| `UNKNOWN_FIELD` | `USAGE`, `field` `--fields` | 2 |
| `USAGE` for an unknown command / no command | `UNKNOWN_COMMAND` / `COMMAND_REQUIRED` | 2 |
| (none) | `INTERNAL` | 1 |
| `ADAPTER_REFUSED` used with exit 1 and 2 | `ADAPTER_REFUSED` (2); `ADAPTER_RESPONSE_INVALID` (1) for `canon adapter validate` on an envelope that breaks a rule | 2 / 1 |
| `EVIDENCE_BATCH_REFUSED` used with exit 2 and 1 | `EVIDENCE_BATCH_REFUSED` only for a refused promotion (1); an unwritable directory is `WRITE_UNAVAILABLE` (2), a store problem `EVIDENCE_STORE_UNAVAILABLE` (2) or `EVIDENCE_STORE_FAILED` (1), a write failing after the first record `WRITE_FAILED` (1) | 1 / 2 |

**Exit codes** now come from the code's registry row alone (`canon schema --errors --json`); nothing sets an exit at a call site, and only 0, 1 and 2 exist (signal exits 128+n are outside the registry). Exit 1: the command ran and its answer is negative or incomplete; exit 2: it refused to run. `canon subject move` blocked by a gate violation, `canon brief approve` and `canon evidence add` batches, `canon adapter` and the batch runner no longer compute an exit. `POLICY_APPROVAL_UNREADABLE` is exit 1 (it was registered 2 but `subject move` returned 1). `BRIEF_UNREADABLE` read during `canon brief approve`'s check is exit 2 (it was 1).

`WRITE_FAILED` is exit 1 (a write that began and failed). The same failure known before the first write (a target that cannot be written) is the new `WRITE_UNAVAILABLE`, exit 2, with nothing changed; `change new`, `feature new`, `scenario new`, `demo review`, `divergence add`/`accept`, `evidence add`, `finding add`/`close`, `task done`, `gate install-hooks`, `init`, `migrate`, `artifacts move`, `brief approve`, `policy approve`, `export` and `review` check it up front.

The thirteen exit-2 codes named for a failure during the work, decided one by one (a: the same failure can happen before and during the work, so the code is split; b: only before, stays exit 2; c: only during, becomes exit 1):

| Code | Decision | Split into | Exit |
|---|---|---|---|
| `ARTIFACTS_STORE_FAILED` | a | `ARTIFACTS_STORE_UNREACHABLE` (2) before, `ARTIFACTS_STORE_FAILED` (1, retryable) during | 2 → 1 |
| `ARTIFACTS_STORE_UNREACHABLE` | a | stays for the store that cannot be attached or reached before the work; mid-upload is `ARTIFACTS_STORE_FAILED` | 2 |
| `EVIDENCE_STORE_FAILED` | a | `EVIDENCE_STORE_UNAVAILABLE` (2, retryable) before, `EVIDENCE_STORE_FAILED` (1) during | 2 → 1 |
| `HOOKS_INSTALL_FAILED` | a | `HOOKS_INSTALL_REFUSED` (2) before, `HOOKS_INSTALL_FAILED` (1) during | 2 → 1 |
| `INIT_SKILL_INSTALL_FAILED` | c | raised only after `canon.yaml` was written | 2 → 1 |
| `INVENTORY_FAILED` | a | `INVENTORY_REFUSED` (2) for configuration and lock problems, `INVENTORY_FAILED` (1) for store errors while syncing | 2 → 1 |
| `LEDGER_ARCHIVE_UNAVAILABLE` | b | an up-front check that the ledger root is named `ledger` | 2 |
| `MIGRATE_PINS_FAILED` | c | raised only after files moved and the index was rebuilt | 2 → 1 |
| `PLUGIN_SYNC_FAILED` | b | every error of the run is a resolution error; write errors are `PLUGIN_SYNC_ERRORS` (1) | 2 |
| `POLICY_APPROVAL_FAILED` | a | `POLICY_APPROVAL_REFUSED` (2) before, `POLICY_APPROVAL_FAILED` (1) when writing the approval file | 2 → 1 |
| `RETENTION_EXPORT_FAILED` | a | `RETENTION_EXPORT_REFUSED` (2) before, `RETENTION_EXPORT_FAILED` (1) during | 2 → 1 |
| `RETENTION_PURGE_FAILED` | a | `RETENTION_PURGE_REFUSED` (2) before, `RETENTION_PURGE_FAILED` (1) once `--commit` is deleting | 2 → 1 |
| `RETRIEVE_FAILED` | b | only usage and derivation errors; a store outage still degrades to empty guidance and exit 0 | 2 |
| `WRITE_FAILED` | a | `WRITE_UNAVAILABLE` (2) before, `WRITE_FAILED` (1) during | 2 → 1 |

`MIGRATE_STOPPED` and `CHANGE_ADOPTION_INCOMPLETE` (exit 2, a failure after the work began) were outside the thirteen and are unchanged.

**Warning codes.** New, registered with `exit` 1 or 2 and `retryable` false like error codes: `BRIEF_ADOPTION_UNAPPROVED`, `BRIEF_DRAFT_PENDING`, `BRIEF_OVERRIDE_ON_RECORD`, `BRIEF_UNAPPROVED_NOT_ENFORCED`, `DASHBOARD_SNAPSHOT_UNAVAILABLE`, `DISPATCH_DIFF_RECORDS_UNAVAILABLE`, `DISPATCH_RUN_COPIES_DIVERGE`, `DISPATCH_TIER_NOT_PERSISTED`, `LEDGER_KIND_ROUTED_AWAY`, `LEDGER_NO_SCENARIOS`, `LEDGER_RECORDS_UNREADABLE`, `LEDGER_ROLLBACK_NOT_RESTORED`, `LEDGER_UNREADABLE`, `LEGACY_GUIDE_STALE`, `LEGACY_STATE_FOUND`, `OLD_BRIEF_FORMAT_FOUND`, `OLD_LAYOUT_FOUND`, `OVERRIDE_REASON_UNUSED`, `OVERRIDE_RECORDED`, `OVERRIDE_REFUSAL_WAIVED`, `POLICY_MISSING`, `POLICY_NOT_APPROVED`, `POLICY_SPEC_COVERAGE_ABSENT`, `POLICY_SPEC_COVERAGE_INVALID`, `POLICY_UNREADABLE`, `QUERY_PLUGIN_DIAGNOSTIC`, `REFERENCES_UNCHECKED`, `REPORT_TIER_NOT_READ_DIRECTLY`, `RETRIEVE_REGIME_FELL_BACK`, `REVIEW_RECORDS_UNMIGRATED`, `ROLE_UNKNOWN_RECORDED`, `SCENARIO_INDEX_CHECK_FAILED`, `SCENARIO_INDEX_NOT_REFRESHED`, `SCENARIO_INDEX_REFRESHED`, `SCENARIO_SUBJECT_UNKNOWN`, `SEARCH_GITIGNORE_MISSING`, `SEARCH_INDEX_IN_MEMORY`, `SEARCH_INDEX_REBUILT`, `STRATEGY_DEMOTED`, `SUBJECT_OVERRIDE_ON_RECORD`, `SUBJECT_REOPENED`, `TASK_PROOF_IS_CANON_COMMAND`, `TASK_REIMPORT_INCOMPLETE`, `TASK_STATUS_NOT_REFRESHED`, `UNKNOWN_ROLE_IGNORED`, plus the existing `CONFIG_MISSING`, `CONFIG_UNREADABLE`, `DISPATCH_MANIFEST_MISMATCH`, `DISPATCH_MANIFEST_UNREADABLE`, `LAYOUT_MIGRATION_REQUIRED`, `POLICY_APPROVAL_UNREADABLE` and `POLICY_CHANGED_SINCE_APPROVAL` where the same situation was a warning.

**Consumers.** The npm launcher exits 128+n when the binary dies by a signal (it was 1, which canon now uses for a ran-and-found answer). The dashboard reads an absent `error.hint`. The embedded guides, the website (English and Korean), the dogfood evals and `crates/canon-cli/tests/old_names.rs` carry the new names; the conformance corpus was regenerated and differs only by the contract changes above.

### Changed
- The error value (`CliError` and its builder), `redact`, the error-code row type, JSON-mode detection, the one-line envelope output and the shell quoting and splitting of next-step commands moved into a new workspace crate, `atip`, that depends on no canon crate. Output, exit codes and the command spec file are byte-identical to 0.32.0.

## 0.32.0 — 2026-10-31

An agent can record the approval a human gave in the conversation, and release notes move from the README to this file.

### Added
- `canon brief show <change> --body` prints the brief as written, then a `digest: sha256:…` line equal to the digest `canon brief approve` records. `--json` gives `body` and `digest`.
- `--relayed-via <tool>` on `canon brief approve`, `canon policy approve`, `canon divergence accept` and `canon migrate --commit`: an agent role records the human's answer from the conversation (`--approver`, `--note` with the human's words, `--actor` or `CANON_ACTOR`). `brief approve` with `--relayed-via` also needs `--digest <change>=<sha256:…>` per brief, the digest the human was shown; a brief that changed since is refused with `RELAY_DIGEST_MISMATCH` (exit 2, nothing written).
- Relayed approvals are marked: the `Approved:` line reads `(relayed by <role> <actor> via <tool>)`, and `policy.approval.json`, divergence acceptance records and the migrate journal gain a `relay` field. `canon brief show`, `canon status` (`relay` in each brief entry) and the dashboard briefs page show it.
- `approval: { relay: allow | deny }` in `canon.yaml`, default `allow`. With `deny`, a relayed call is refused with `HUMAN_ONLY` naming the key. `canon init` writes the key as a comment.

### Changed
- `canon brief approve`, `canon policy approve` and `canon migrate` accept `--actor`; `canon divergence accept` and `canon migrate` accept `--approver` and `--note`.
- An agent role calling one of the four commands without `--relayed-via` still gets `HUMAN_ONLY`; the hint now describes showing the brief, asking, and recording with `--relayed-via`. The skill guides and `canon status` next steps describe the same procedure instead of "report the command and wait".
- Release notes live in `CHANGELOG.md`; the README keeps the introduction and quick start. Contributors run `just` to list the build, test, lint and gate recipes.

## 0.31.0 — 2026-10-28

Id options lose their `-id` suffix, briefs become plain frontmatter documents, and `canon search` / `canon trace` are new.

### Breaking
- The 14 id options lose `-id` on every command (`--actor-id` → `--actor`, `--change-id` → `--change`; `--repo` stays). Old spellings have no alias and fail with `USAGE` (exit 2).
- `--input` document keys follow the option names (`scenarioId` → `scenario`); old keys fail with `INPUT_UNKNOWN_FIELD` (exit 2). Record, report, error and `--fields` keys keep `*_id` / `*Id`.
- `canon status --json` `statusVersion` is `4`: a subject's `brief` object is replaced by `briefs`, a list with one entry per change the subject answers for.
- A brief is frontmatter, a title, `##` sections (exact English headings) and the `Approved:` line, with no HTML comments; waivers are the frontmatter `waive` list and a chore has `kind: chore`.
- A 0.30 brief gets blocker `brief.old-format`; `evidence add`, `subject move` to ship or reopen, `subject adopt` and `review change` refuse it with `OLD_BRIEF_FORMAT` until `canon migrate`.
- Not-found and invalid input now exit 2; exit 1 stays for check results and refused verdicts. `canon adapter authorize` exits 2 (`ADAPTER_REFUSED`) for a broken envelope or policy file (was 1).

### Added
- `canon search "<text>"` finds records, brief sections, flow steps and scenario text, with neighbors; options `--kind`, `--expand <N>`, `--limit <N>`. No match is exit 0; empty text is exit 2.
- `canon trace <finding key|review id|sha>` prints one review loop in time order, including archived versions. An unknown or invalid operand is exit 2.
- `canon brief approve` takes several change ids; if any is refused the call fails with `BRIEF_APPROVE_REFUSED` (exit 2) and writes nothing. The dashboard briefs page can approve several too.
- `canon migrate` also rewrites 0.30 briefs (`--commit --plan-digest`); a commit that re-binds approvals is human-only (`HUMAN_ONLY` for agent roles).
- A brief with invalid frontmatter is a blocker (`brief.frontmatter-invalid`); `canon context-pack show` / `verify` accept only `sha256:` plus 64 lower-case hex digits.

### Changed
- A subject answers for every adopted change that is tagged, approved or a chore, whatever the adoption order; an unapproved chore blocks reopen and ship. An untagged, never-approved change is only a draft.
- `canon demo init` refuses a directory holding `canon.yaml` (`INIT_CONFIG_EXISTS`); `canon gate install-hooks` refuses a `--repo` that is not a directory (`HOOKS_INSTALL_FAILED`).
- `canon search` / `canon trace` keep a derived index in `.canon/index/`, which `canon init` adds to `.gitignore`; deleting it loses nothing.

### Fixed
- Commands without `--repo` (`canon schema`, `canon skills show`) no longer fail with `CONFIG_MISSING` outside a repo in release builds.
- `canon migrate --revert` refuses when a file the migrate left was deleted since.

### Upgrading
1. Run `canon migrate` on a repo with 0.30 briefs, then have the human run `canon migrate --commit --plan-digest <digest from the preview>`; `canon migrate --revert` undoes it.
2. Rewrite old option spellings in scripts and CI using the table below, and rename `--input` document keys the same way.

<!-- canon:generated removed-031 begin -->
| 0.30 spelling | 0.31 spelling | What changed |
|---|---|---|
| `--actor-id` on `canon dispatch begin`, `canon divergence accept`, `canon divergence add`, `canon divergence defer`, `canon divergence resolve`, `canon evidence add`, `canon evidence audit`, `canon feature new`, `canon finding add`, `canon finding close`, `canon review change`, `canon review scenario`, `canon scenario new`, `canon strategy rollback`, `canon subject move`, `canon subject new` | `--actor` |  |
| `--session-id` on `canon dispatch begin`, `canon divergence accept`, `canon divergence add`, `canon divergence defer`, `canon divergence resolve`, `canon evidence add`, `canon evidence audit`, `canon feature new`, `canon finding add`, `canon finding close`, `canon review change`, `canon review scenario`, `canon scenario new`, `canon subject move`, `canon subject new` | `--session` |  |
| `--project-id` on `canon divergence accept`, `canon divergence add`, `canon divergence defer`, `canon divergence resolve`, `canon evidence add`, `canon evidence approval-payload`, `canon evidence audit`, `canon feature new`, `canon review change`, `canon review scenario`, `canon scenario new` | `--project` |  |
| `--change-id` on `canon finding add`, `canon finding list`, `canon query`, `canon scenario new`, `canon subject adopt`, `canon subject unadopt` | `--change` |  |
| `--run-id` on `canon evidence add`, `canon evidence approval-payload`, `canon evidence audit`, `canon ingest records` | `--run` |  |
| `--task-id` on `canon dispatch begin`, `canon evidence add`, `canon evidence approval-payload` | `--task` |  |
| `--subject-id` on `canon change new`, `canon retrieve`, `canon scenario new` | `--subject` |  |
| `--scenario-id` on `canon evidence add`, `canon evidence approval-payload`, `canon finding add` | `--scenario` |  |
| `--skill-id` on `canon dispatch begin` | `--skill` |  |
| `--parent-run-id` on `canon dispatch begin` | `--parent-run` |  |
| `--evaluator-id` on `canon evidence audit` | `--evaluator` |  |
| `--review-id` on `canon finding add` | `--review` |  |
| `--plugin-id` on `canon query` | `--plugin` |  |
| `--contradicting-trajectory-id` on `canon strategy rollback` | `--contradicting-trajectory` |  |
<!-- canon:generated removed-031 end -->

3. Tools that read `canon status --json` must read `briefs` instead of `brief` (`statusVersion` 4).

## 0.30.0 — 2026-10-27

Stored records get one naming rule and `evidence_record` becomes `evidence`; `canon migrate` moves a 0.29 repo over.

### Breaking
- Every store names a record `kind=<kind>/<key>_<digest8>.json`, and the `area=` level is gone from ledger paths; the same key with different content is refused.
- The kind `evidence_record` is now `evidence` on disk, in the `kind` field, the schema, `canon.yaml` `routing:` and aging keys, and report views.
- A 0.29 repo warns on read commands and refuses writes until `canon migrate`; 0.29 query cursors are rejected.
- A new blocker or should-fix finding needs `--introduced-rev <rev>` or `--introduced-unknown`.

### Added
- `canon migrate` previews the plan and digest; `--commit --plan-digest <digest>` applies it with a backup under `.canon/migrate-backup/`; `--revert` undoes the last migrate.
- `canon finding add --review-id <id>` links a finding to its review (`--review` from 0.31); each review run gets one review id.
- `canon query` marks dangling references, and `canon status --json` counts them in `danglingReferences`.

### Upgrading
1. Run `canon migrate`, review the plan, then `canon migrate --commit --plan-digest <digest from the preview>`.
2. Restart any paged `canon query` from the first page.

## 0.29.0 — 2026-10-25

`canon gate check`, `canon status` and batch `canon evidence add` run much faster on large repos, with identical output.

## 0.28.0 — 2026-10-24

One grammar for the whole CLI, generated from one command spec. Old spellings fail with exit `2` and an `error.hint` naming the replacement; no deprecated aliases.

### Breaking
- Every group needs a subcommand; the primary object is the operand: `canon brief show`, `canon task done add-login#1`, `canon prompt add reviewer@v1`; mode flags became commands.
- One meaning per option: `--kind` record kind, `--role` attributed role, `--actor-id` who acts, `--approver` approving human, `--limit` pages, `--out` → `--output`.
- `--rev <REV>` replaces `--pin`, `--sha`, `--artifact-sha`, `--reviewed-sha`; `--resolution-rev` and `--introduced-rev` replace `--resolution-sha` and `--introduced-by`.
- `--json` prints one envelope and nothing on stderr: `{"ok", "data", "meta"}` or `{"ok": false, "error", "meta"}`; warnings in `meta.warnings`; every report, error and `--input` key is camelCase (`foo_bar` → `fooBar`), except `canon context`'s `policy.*` keys (snake_case, as in `policy.yaml`); the `export` result key `out` is `output`.
- `purge`, `tier age`, `ledger compact`, `artifacts move` and `migrate` preview by default; `--commit --plan-digest <DIGEST>` acts (`PLAN_CHANGED` if the plan moved); their `--dry-run` is removed.
- `brief approve`, `policy approve`, `divergence accept` are human-only (`HUMAN_ONLY`); `brief approve` with blockers fails `BRIEF_APPROVE_REFUSED` (exit `2`); `brief check` with blockers exits `1`.
- Error codes renamed: `ARTIFACTS_MIGRATE_REFUSED` → `ARTIFACTS_MOVE_REFUSED`, `EVIDENCE_VAULT_FAILED` → `ARTIFACTS_STORE_FAILED`, `FINDING_RESOLUTION_SHA_UNEXPECTED` → `FINDING_RESOLUTION_REV_UNEXPECTED`.
- Error codes removed: `PURGE_PLAN_REQUIRED`, `FINDING_RESOLUTION_SHA_REQUIRED`, `FINDING_UNRESOLVABLE_COMMIT` (an unknown revision is `NO_COMMIT`).
- Exit changes: every `*_NOT_FOUND`, `*_UNREADABLE` and `*_INVALID_ARGUMENT` exits `2`, as do `init --check` of a malformed config and `task done` on an unknown task.
- `canon context` keys record kinds by the name `--kind` takes (`evidence`, not `evidence_record`); `finding close` reports a repeat with `meta.unchanged: true` instead of a report `unchanged` key.
- `--project-id` only with more than one spec root; `schema --all` is gone (use `--as <role>`); `skills install` writes a short `SKILL.md` stub; an empty check reports `UNVERIFIABLE` (exit `1`).

### Added
- `--input` on write commands takes arguments as JSON: `--input '{"operands": ["auth.login.01"]}'`, `--input file.json` or `--input -`; keys are camelCase option names plus `operands`.
- `canon skills show <NAME> [--full]` prints the embedded guide; `canon schema <command> --json` shows effect, roles, exits, errors; `schema --errors --json` lists every error code.
- `canon context` gains a `commands` index (filtered by `--as`); `--fields` takes camelCase or snake_case, an unknown name is `UNKNOWN_FIELD` (exit `2`).
- `--repo <DIR>` is declared once for all commands (default: nearest `canon.yaml`); `--session-id` joins `--actor-id`; `--override-reason` is the only "proceed although refused" flag.
- `canon subject move --to` changes a subject's state (`subject show` replaces `subject status`); `evidence add --all-scenarios --report junit:a.xml` is the batch form.
- Uncommitted code is recorded, not refused: records carry a `code_tree` and `worktree_dirty`, and `canon.yaml` `code.exclude` (default `docs/**`, `*.md`) says what is not code.
- Destructive commands report `planDigest`, `effects[]`, `committed`; commands needing a commit refuse with `NO_COMMIT`; `canon migrate` rewrites hooks, old skill and `AGENTS.md`, reports `backupDir`.

### Changed
- Repeating an identical write is a no-op with `meta.unchanged` (exit `0`); `divergence accept` defaults `--role` to `reviewer`; `init --check` with a failing section exits `1`.

### Removed
- `canon gate task` (use `canon task done`), the `canon fmt` alias, `scenario new --from`, `review add --scenario-ids-from`, and the other spellings in the table below.

### Upgrading
1. Run `canon migrate`, then `canon migrate --commit --plan-digest <DIGEST>` (`meta.next[0].argv` has it with `--json`); backups go to `.canon/migrate-backup/<timestamp>/`.
2. Refresh the skill stub: `canon skills install`.
3. Rewrite old spellings in your scripts using the table below.

<!-- canon:generated removed begin -->
| 0.27 spelling | 0.28 spelling | What changed |
|---|---|---|
| `canon artifacts migrate` | `canon artifacts move --tier` | the bare run previews; `--commit --plan-digest` acts |
| `canon brief` (bare) | `canon brief show` | a group is never runnable; the change is an optional operand |
| `canon context-pack new` | `canon context-pack add --manifest` |  |
| `canon demo add` | `canon demo review` |  |
| `canon divergence promote` | `canon gate promote --kind divergence` |  |
| `canon divergence status` | `canon divergence list` |  |
| `canon evidence vault` | `canon artifacts store` |  |
| `canon fmt` | `canon format` | the `fmt` alias is gone; the directory is an optional operand |
| `canon gate selftest` | `canon selftest --suite trust-spine-gate` |  |
| `canon gate task` | `canon task done` | the task id is the operand; installed hooks are rewritten by `canon migrate` |
| `canon ingest artifacts` | `canon ingest records` | "artifact" now means the blob store only |
| `canon learn promote` | `canon strategy promote` | the strategy id is the operand |
| `canon learn approve` | `canon strategy approval-payload --evaluation --approver --approval-at` | the strategy id is the operand; `approve` is reserved for commands that record a human approval |
| `canon learn rollback` | `canon strategy rollback` | the strategy id is the operand |
| `canon regime-key` | `canon regime show --role-segment --repo-name --area --hash` |  |
| `canon report` (bare) | `canon report build` | `report check` and `report snapshot` are the other two modes |
| `canon review add` | `canon review scenario` | split by what it reviews: `review change` reviews a chore change |
| `canon subject status` | `canon subject show` | `subject move --to` changes the state |
| `--check` on `canon report` | `canon report check` |  |
| `--snapshot` on `canon report` | `canon report snapshot` | the output directory is the operand |
| `--check` on `canon format` | `canon format` | `format` always validates; drop the flag |
| `--check-config` on `canon init` | `--check` |  |
| `--change-id` on `canon brief show`, `canon brief check`, `canon brief approve` | `canon brief show` | the change is the operand |
| `--change` on `canon brief show`, `canon brief check`, `canon brief approve` | `canon brief show` | the change is the operand |
| `--change-id` on `canon finding close` | `canon finding close` | the change is the operand |
| `--run-id` on `canon dispatch end` | `canon dispatch end` | the run is the operand |
| `--subject-id` on `canon subject adopt`, `canon subject unadopt` | `canon subject adopt` | the subject is the operand and the change is `--change` |
| `--name` on `canon prompt add`, `canon prompt show` | `canon prompt add` | name and version are one operand, name@version |
| `--version` on `canon prompt add`, `canon prompt show` | `canon prompt add` | name and version are one operand, name@version |
| `--scenario-id` on `canon divergence add`, `canon divergence defer`, `canon divergence resolve`, `canon divergence accept` | `canon divergence add` | the scenario is the operand |
| `--scenario-id` on `canon review scenario` | `canon review scenario` | the scenarios are operands |
| `--scenario-ids-from` on `canon review scenario` | `--input` | a batch is an `--input` array |
| `--change-id` on `canon review scenario` | `canon review change` | a chore change is reviewed with `review change` |
| `--scenario-id` on `canon review change` | `canon review scenario` | scenarios are reviewed with `review scenario` |
| `--scenario-ids-from` on `canon review change` | `canon review scenario` | scenarios are reviewed with `review scenario` |
| `--change-id` on `canon review change` | `canon review change` | the change is the operand |
| `--from` on `canon scenario new` | `--input` | a batch is an `--input` array with `operands` per element |
| `--all` on `canon schema` | `canon schema` | the listing is unfiltered; `--as` filters it by role |
| `--agent-id` on `canon dispatch begin` | `--actor` |  |
| `--reviewer` on `canon divergence add`, `canon divergence defer`, `canon divergence resolve`, `canon divergence accept`, `canon finding add`, `canon review scenario`, `canon review change` | `--actor` | it is the reviewer, and required where `--reviewer` was |
| `--by` on `canon brief approve`, `canon policy approve` | `--approver` |  |
| `--principal` on `canon strategy approval-payload` | `--approver` |  |
| `--approval-by` on `canon evidence add`, `canon evidence approval-payload` | `--approver` |  |
| `--approved-at` on `canon strategy rollback` | `--approval-at` |  |
| `--signature` on `canon strategy promote` | `--signature-file` |  |
| `--approval-signature-file` on `canon evidence add` | `--signature-file` |  |
| `--context-manifest` on `canon dispatch begin` | `--manifest` |  |
| `--sha` on `canon divergence add`, `canon divergence defer`, `canon divergence resolve` | `--rev` | the commit the record is about; default HEAD |
| `--pin` on `canon review scenario`, `canon review change` | `--rev` | default HEAD |
| `--artifact-sha` on `canon evidence add`, `canon evidence audit`, `canon evidence approval-payload` | `--rev` | default HEAD |
| `--reviewed-sha` on `canon finding add` | `--rev` | default HEAD |
| `--resolution-sha` on `canon finding add`, `canon finding close` | `--resolution-rev` | default HEAD when fixed |
| `--introduced-by` on `canon finding add` | `--introduced-rev` | no default |
| `--out` on `canon export` | `--output` | `-o` is the short form |
| `-k` on `canon retrieve` | `--limit` |  |
| `--k` on `canon retrieve` | `--limit` |  |
| `--role` on `canon retrieve` | `--for-role` | `--role` means only attribution |
| `--kind` on `canon evidence add` | `--class` | an external re-audit is `evidence audit`, not `--class audit` |
| `--kind` on `canon change new` | `--change-kind` | `--kind` is reserved for record kinds |
| `--status` on `canon dispatch end` | `--run-outcome` |  |
| `--status` on `canon divergence add` | `--state` |  |
| `--outcome` on `canon review scenario`, `canon review change` | `--review-outcome` |  |
| `--source` on `canon skills install`, `canon skills check`, `canon skills doctor` | `--skills-source` |  |
| `--source` on `canon ingest plans` | `--plan-source` |  |
| `--role` on `canon regime show` | `--role-segment` |  |
| `--repo` on `canon regime show` | `--repo-name` | the repo segment of the key; `--repo` names a directory on other commands |
| `--to` on `canon artifacts move` | `--tier` |  |
| `--target` on `canon skills install`, `canon skills check`, `canon skills doctor` | `--repo` | the repo is `--repo` on every command; omit it to use the nearest directory holding canon.yaml |
| `--canon-yaml` on `canon ingest sessions`, `canon query`, `canon tier age` | `--repo` | the repo's canon.yaml is found from `--repo`, or from the nearest directory holding one when it is omitted |
| `--dry-run` on `canon artifacts move`, `canon ledger compact`, `canon migrate`, `canon purge`, `canon tier age` | `--commit --plan-digest` | the bare run previews; `--commit --plan-digest` acts |
| `--plan` on `canon purge` | `canon purge` | the bare run previews; `--commit --plan-digest` acts |
| `--confirm` on `canon purge` | `--commit --plan-digest` | the bare run previews; `--commit --plan-digest` acts |
| `--evaluator-id` on `canon evidence add` | `canon evidence audit` | an external evaluator's re-audit is its own command |
| `--evaluator-version` on `canon evidence add` | `canon evidence audit` | an external evaluator's re-audit is its own command |
| `--evaluator-digest` on `canon evidence add` | `canon evidence audit` | an external evaluator's re-audit is its own command |
| `--audit-result` on `canon evidence add` | `canon evidence audit` | an external evaluator's re-audit is its own command |
<!-- canon:generated removed end -->

4. Scripts reading `--json`: read `data`, use camelCase keys, treat exit `1` (finding) and `2` (refused) differently.
5. Use `--rev` (default `HEAD`) instead of `--pin`/`--sha`; use the bare preview then `--commit --plan-digest` instead of `purge --confirm`/`--dry-run`.

## 0.27.0 — 2026-10-22

Two safety fixes: a gate can no longer pass without canon state, and read commands no longer write.

### Breaking
- A command that needs canon state and finds none refuses with `CONFIG_MISSING` (exit `2`); before, `canon gate check` without `canon.yaml` printed `clean` and exited `0`.

### Fixed
- Commands that `canon schema` marks `effect: read` leave the tree unchanged (`status`, `gate check`, `report --check` and `query` reads no longer write).
- `query` fails with an unavailable-tier error when the hot path is a directory or not a SQLite file; `format` refuses a corpus root that is not a directory.

## 0.26.0 — 2026-10-22

`brief check` now reads a brief in the language it is written in, judges Intent and Questions, and adds six advisory prose rules.

### Added
- `brief check` detects the brief's language from its prose (`ko`, `en`; `ja` and `zh` get structure rules only); JSON gains `languageSource` and `languageNote`.
- Six advisory prose rules (`prose.code-citation`, `.parenthetical`, `.decision-history`, `.ai-vocabulary`, `.ko-ending`, `.length`), waivable in frontmatter; they never block `brief approve`.
- A brief with advisories but no blockers gets a polish step (`reference/brief-polish.md`, then `canon brief check`) in `canon status` and `meta.next` before the human step.
- The briefs page shows advisory totals, a per-brief badge, language, and findings grouped by rule.
- A `--project-id` naming no spec root in `canon.yaml` is refused with `UNKNOWN_SPEC_ROOT` (exit `2`) before anything is written.

### Changed
- `language:` in `canon.yaml` now overrides detection instead of being the only source.
- `canon init` and `canon migrate` ignore lines also cover `.canon/dashboard-snapshot/`, `.canon/learn/`, `.canon/r2/` and `.canon/archive/kind=scenario/`.

### Fixed
- The dashboard drains the request before closing, so a client always receives an early refusal.

### Upgrading
1. In an existing repo, run `canon migrate` once to add the new ignore lines.
2. Polish briefs that now report advisories, or waive a finding with a reason.
3. Fix scripts that pass a `--project-id` naming no configured spec root; they now exit `2`.

## 0.25.0 — 2026-10-21

Brief approval holds back only the change it approves, `canon dashboard` works in any repo and can approve briefs, and 0.24.0's deprecated spellings are removed.

### Breaking
- Removed 0.24.0 spellings: `--change`, `--project`, `--subject`, `--actor`, `--run` gain `-id`; `--kind evidence_record` becomes `--kind evidence`.
- `canon finding close --disposition rejected|deferred` now requires `--reason` (`FINDING_REASON_REQUIRED`).

### Added
- `canon dashboard` runs in any repo (binary embeds the app; `--port` if taken); `/briefs.html` approves briefs like `canon brief approve --change-id <id> --by <name>`, refused if changed or blocked.
- `canon subject unadopt <change> --subject-id <id>` moves a draft change off a subject; `change new --subject-id` and `subject adopt` warn when the subject is building, verifying or shipped.
- `finding close --reason` is stored as an optional `reason` on the closing version and shown in `finding list`.

### Changed
- Evidence needs approval only from the changes a scenario's `@flow:<change>.<F-id>` tags name (untagged: the subject's newest approved change); an untagged pending draft is outside scope.
- `canon skills install` deletes unedited files the bundle no longer ships and reports edited ones; `skills check` (`SKILLS_DRIFT`) and `skills doctor` name each orphan.

### Upgrading
1. Replace the removed spellings in scripts and add `--reason` to `finding close --disposition rejected|deferred`.
2. Run `canon skills install` once to prune files 0.24 shipped that 0.25 no longer does.

## 0.24.1 — 2026-10-21

A fixed blocker now re-verifies only the scenarios it names.

### Added
- `canon finding add` takes a repeatable `--scenario-id` (`FINDING_UNKNOWN_SCENARIO` for an unknown id), stored as an optional `scenario_ids` list on the Finding record; older records read unchanged.

### Changed
- After a `fixed` close of a finding with `scenario_ids`, only the named scenarios with evidence need newer evidence; `gate check`, ship and `status` agree.

## 0.24.0 — 2026-10-19

Each concept gets one flag name across every command, and evidence batches bind each attachment to the scenario it belongs to.

### Breaking
- `evidence add --all-scenarios` refuses a bare `--artifact <path>`; use `--artifact <scenario-id>=<path>` or let the report carry attachments per test.
- A report attachment that is missing or over the size limit is refused, naming the case and path; nothing is written.

### Added
- Reports can name which file belongs to which test: JUnit `[[ATTACHMENT|<path>]]` lines in `<system-out>` and Cucumber JSON `embeddings`.
- `canon schema <command>` lists deprecated spellings as `deprecatedAliases`.

### Changed
- Flags whose value is a record or entity id become `--<noun>-id`: `--change`, `--project`, `--subject`, `--actor`, `--run` → `--change-id`, `--project-id`, `--subject-id`, `--actor-id`, `--run-id`.
- `--kind` values of `query`, `export` and `purge` drop the `_record` suffix (`evidence_record` → `evidence`); the record's own `kind` field, ledger paths and `canon.yaml` routing keys keep it.
- The old spellings still run until 0.25.0, warning on stderr and adding `meta.deprecated` to JSON; `<kind>_record` is accepted for every `--kind`.

### Upgrading
1. Switch scripts to the new flag names and `--kind` values; the old ones warn until 0.25.0.
2. If a batch passed screenshots as bare `--artifact`, let the report carry them per test or use `--artifact <scenario-id>=<path>`.

## 0.23.0 — 2026-10-18

The working set keeps only current records: superseded versions move to `.canon/archive`, evidence stops carrying attachments, reviews pin verdicts and checks, and artifact bytes can live off git.

### Breaking
- `evidence add --drop-attachment` and `--drop-reason` are removed (exit 2), with the evidence `attachment_drop` field and the `EVIDENCE_CARRY_BLOB_MISSING` code; evidence binds only the files its own write names.
- Review JSON `pins.evidence: [{key, digest}]` becomes `pins.checks: [{role, verdict, checks: [...]}]`; `Review.scenario_id` is optional, `change_id` is new.
- `canon query` reads the ledger only; add `--history` to include superseded versions; `divergence resolve|defer|accept` stage and promote in one run (`--no-promote` stages only).
- Reviews predating 0.23 pins do not count until `canon migrate` pins them; a transition needing the review is refused as `unreviewed-promotion`.

### Added
- `.canon/archive/kind=<k>/…` holds superseded versions (one ledger file per key); `canon query --kind <kind> --history` returns all; `canon ledger compact [--dry-run]` moves them once.
- `canon.yaml` `artifact_store: { backend, prefix }` (`local` default, or an `s3` tier); `canon artifacts verify [--full]` checks bound files; exit 1 missing or altered, 2 store unreachable.
- `canon artifacts migrate --to <tier> [--dry-run]` uploads blobs, writes `artifact_store:`, removes local copies and git-ignores `.canon/artifacts/`; no way back.
- `canon review add --change <slug>` reviews a chore as a whole; `canon migrate` re-pins still-current reviews and accepted divergences.

### Changed
- A review pins its scenario digest plus per-role latest verdict and check identity; a chore is satisfied by one change-level review or current signed reviews of every evidenced scenario.
- Evidence, finding and divergence stage under `.canon/ledger/_staging/` and are committed by `canon gate promote`.

### Upgrading
1. Run `canon migrate` once and commit the result (CI needs `fetch-depth: 0`); until then old reviews do not count.
2. Remove `--drop-attachment`/`--drop-reason` from scripts; add `--history` to `canon query` calls reading superseded versions; read `pins.checks`.
3. Optional: `canon artifacts migrate --to <tier>` moves artifact bytes off git.

## 0.22.0 — 2026-10-17

Gates fail closed on policy drift and unverified fixes, a tracked known gap can be accepted without a false `faithful` verdict, and every refusal has a hint. 0.21 deprecated names are removed.

### Breaking
- Removed (usage errors now): `context-pack create`, `prompt register`, `divergence stage`, `demo attest`, two-positional `subject status <id> <state>`; `meta.deprecated` is gone.
- `canon gate check` exits 1 and `subject status --to shipped`/reopen is refused (`POLICY_CHANGED_SINCE_APPROVAL`, `POLICY_APPROVAL_UNREADABLE`) when the policy differs from its approved digest.
- A blocker closed `fixed` keeps blocking (`open-blocker`, waivable with `--override-reason`); an approved chore refuses shipping (`uncovered-cell`) until every evidenced scenario has newer evidence.
- While the brief has blockers, `canon status` shows `canon brief check --change <slug>`, not the human stop.

### Added
- `canon divergence accept --project-id <root> --scenario-id <id> --reason <text> --tracking <ref> [--expiry YYYY-MM-DD] --reviewer <id> --role reviewer` accepts a `divergent` scenario as a gap.
- `canon policy show [--json]` prints the policy digest and approval state (`none`, `approved`, `unapproved`, `changed`, `unreadable`).
- `canon change new` on a shipped subject prints the reopen command.

### Changed
- `canon brief approve` writes RFC 3339 UTC time (date-only `Approved:` lines still read); every refusal has a `hint` (stderr `hint: …`; JSON `error.hint` string).
- `meta.next` after writes follows the same brief-first rule as `status`.

### Upgrading
1. Switch scripts to the 0.21 names (`context-pack new`, `prompt add`, `divergence add`, `demo add`, `subject status <id> --to <state>`); drop any branch reading `meta.deprecated`.
2. If `canon gate check` now exits 1 on policy, have the human run `canon policy approve --by <human> --note "<words>"`, or restore the approved policy files from git.
3. After a blocker closed `fixed`, or an approved chore, re-record with `canon evidence add --all-scenarios …` and re-review before `--to shipped`.

## 0.21.1 — 2026-10-16

Patch release: clearer refusals for outside-the-repo evidence, and `--reason` on every status transition.

### Added
- `canon subject status <id> --to <state> --reason "<text>"` is accepted on any transition, stored as optional `status_reason`. Reopen still requires `--reason`.

### Changed
- Refusals for evidence/review files outside the repository now carry a hint (copy it under the repository) and JSON `error.field` names the flag.

### Removed
- Error code `SUBJECT_REASON_UNEXPECTED`; drop any branch that handled it.

## 0.21.0 — 2026-10-16

One verb grammar, `meta.next` on every write so agents need not call `canon status`, declared agent roles, a core-plus-role skill, and attachments that carry forward on re-attestation.

### Breaking
- `canon evidence add` no longer refuses with `EVIDENCE_REFUSED` when earlier attachments are not repeated; they carry forward. An identical repeat returns unchanged (exit 0).
- Write commands answer `{ data, meta }`; read `.data` (e.g. `dispatch begin` `run_id` is `.data.run_id`). Write output ends with `next: <command> — <why>`; `status`/`meta.next` steps gain `role`.
- `review add` without a provenance flag and invalid enum values are `USAGE` errors; `CANON_ROLE` outside `planner`, `implementer`, `reviewer`, `orchestrator` is `UNKNOWN_ROLE`.

### Added
- Verb grammar: `canon context-pack new`, `prompt add`, `divergence add`, `demo add`, `subject status <id> --to <state>` (without `--to` it shows).
- `meta.next: [{ command, why, actor, role }]` on every successful write; roles via `CANON_ROLE` or global `--as <role>` (flag wins); other roles' steps become `meta.waitingOn`.
- Attachments carry forward (`carried`); a named file replaces its binding; `--drop-attachment <path> --drop-reason <text>` drops one; `EVIDENCE_CARRY_BLOB_MISSING` (exit 2) if a blob is lost.
- `canon schema` gives each command `roles`, `example`, `requiredOneOf`; bare `canon schema` lists the declared role's commands, `--all` all; `--fields a,b` on `status`, `export`, `context`.
- The skill is a shared core plus `reference/role-*.md` per role; `--role` on `evidence add`/`review add` defaults to the declared role.

### Changed
- Deprecated until 0.22.0 (stderr warning): `context-pack create`, `prompt register`, `divergence stage`, `demo attest`, `subject status <id> <state>`; `subject status`, `format` are `write`.

### Upgrading
1. Run `canon skills install` to get the 0.21 skill.
2. Switch to the new spellings before 0.22.0 (`context-pack new`, `prompt add`, `divergence add`, `demo add`, `subject status <id> --to <state>`).
3. Read `.data` and `.meta` of write JSON; match the leading lines of write output, since `next:` is now last.

## 0.20.0 — 2026-10-15

Agents can drive the CLI without `--help`: `canon schema` describes every command, all take `--json`, every refusal has a stable code, lists are paged, and `purge` deletes only a reviewed plan.

### Breaking
- `canon purge --dry-run` is removed; use `--plan`, then `--confirm <planDigest>`. A bare `purge` is refused with `PURGE_PLAN_REQUIRED`. `canon learn request` is removed; use `canon learn approve`.
- Stderr errors are `canon <command path>: <CODE> — <message>` plus optional `hint:`; match the code, not old `refused —` forms. A multi-violation refusal is one error.
- `query`, `finding list`, `export`, `divergence status` return at most 100 records by default; pass `--all` or follow `nextCursor`.
- A closed stdout (`canon … | head`) ends canon by SIGPIPE (exit 141) instead of panicking with exit 101.

### Added
- `canon schema [<command path>…] [--json] [--errors]` describes the command tree; unknown path is `UNKNOWN_COMMAND`. Exit codes keep meaning (0 ok, 1 gate/operational, 2 refusal/usage).
- Every command accepts `--json`: `{ "data", "meta" }` or `{ "error": { "code", "message", "hint", "retryable" } }`; `statusVersion` stays `3`. Errors have stable codes; clap usage errors are `USAGE`.
- `query`, `finding list`, `divergence status`, `export` take `--limit <n>` (default 100), `--cursor`, `--all`; `meta` has `hasMore`, `nextCursor`; `--fields a,b,c` on `query`, `finding list`.
- `canon purge --kind <kind> --before <time> --plan` prints a `planDigest`; `--confirm <digest>` deletes only if the plan is unchanged, else `PLAN_CHANGED`.

### Fixed
- `canon … | head` no longer panics; `canon schema "evidence add"` (quoted path) is accepted.

### Upgrading
1. Run `canon skills install` to get the 0.20 skill.
2. Replace `canon purge --dry-run` with `--kind <kind> --before <time> --plan`, then rerun with `--confirm <planDigest>`; replace `canon learn request` with `canon learn approve`.
3. Scripts reading all records from `query`, `finding list`, `export`, `divergence status` must pass `--all` or follow `meta.nextCursor`; match error codes, not `refused —`.

## 0.19.0 — 2026-10-15

The brief becomes a PRD that canon can check: a fixed skeleton with numbered flow steps, `canon brief check`, a digest-bound approval, flow-step coverage, a chore brief, and a configurable language.

### Breaking
- `canon change new` no longer writes `tasks.md`; an existing one is still read, and `gate task` and `evidence add --task` are unchanged for imported plans.

### Added
- `canon change new` writes a fixed brief skeleton (Intent … Out of scope) with flow ids `F1`, `F2`, …; `--kind chore` writes a short brief.
- `canon brief check [--change <slug>] [--json]` reports blockers (`brief.*`) and advisories (`prose.*`); exit 2 on blockers; `<!-- canon:waive <rule-id> <section-key> "<reason>" -->` waives one rule.
- `canon brief` prints the brief's state; `canon brief approve --change <slug> --by <human> --note "<words>"` binds `Approved:` to the brief's sha256 and refuses while blockers remain.
- `canon scenario new --flow <F-id>` tags it `@flow:<change>.<F-id>`; `canon status` lists each flow step with its scenarios and review state (`--json` adds `flow[]`).
- `canon init --language <tag>` writes `language:` to `canon.yaml`; templates and prose rules ship for `en` and `ko`.
- The skill gains five references: `brief-shape`, `brief-clarify`, `brief-harden`, `brief-align`, `brief-polish`.

### Changed
- A brief changed after approval reads as unapproved (`BRIEF_CHANGED_SINCE_APPROVAL`); the ship guard also refuses on brief blockers.
- An `Approved:` line without a digest stays approved with an advisory; coverage rules no longer fire on a new scope before it is approved.

### Upgrading
1. Run `canon skills install`, and re-approve briefs whose `Approved:` line has no digest with `canon brief approve`.
2. Make scripts that read `tasks.md` treat it as optional.

## 0.18.0 — 2026-10-13

One `evidence add` now attests every scenario a test report covers, one `review add` reviews a batch, a shipped subject can be reopened in its approved scope, and a change can have several subjects.

### Breaking
- `Change.subject_id` is now `subject_ids`, a list (`Change` schema generation `2`); generation-1 records read as a one-element list.
- `canon query --kind change` returns only each change's current version; it used to return every version.

### Added
- `canon evidence add --project-id <root> --all-scenarios --report <fmt>:<path> …` writes one record per indexed scenario with a case in the report; all or nothing.
- `canon review add` takes repeated `--scenario-id` and `--scenario-ids-from <file|->` for an all-or-nothing batch with one reviewer and pin.
- `evidence add --task --report` with no scenario id binds every case in the report; `--report swift-testing:<path>` reads `swift test --experimental-event-stream-output`.
- `canon subject status <id> --to verifying --reason "<why>"` reopens a `shipped` subject; outside the approved scope it exits 1 naming the brief to approve.
- `canon subject adopt <change> --subject <other>` adds a second subject to a change; `canon status` flags a shipped subject whose ship guard would now fail (`statusVersion` stays `3`).
- `canon finding list --change-id <id> [--open] [--json]` prints a change's committed findings; `canon skills install` and `doctor` end with an `owned-paths:` list of globs for ignore lists.

### Changed
- `--reason` is required for a reopen and refused on any other transition (exit 2); `canon skills check` reports a formatter-only rewrite as `formatter-rewritten`, not `stale` (still exit 1).
- Fabrication markers `"would pass"`, `"TBD"`, `"n/a"` match only as whole words; `canon report` fails (exit 1) on a non-sqlite `hot` db file.

### Upgrading
1. Run `canon skills install` to refresh the skill and see the `owned-paths:` ignore list.
2. Make scripts expect one row per change from `canon query --kind change`, and match `formatter-rewritten` as well as `stale` from `canon skills check`.

## 0.17.0 — 2026-10-12

canon keeps its specs, its changes and every approval under `.canon/`, reads external plans without writing into them, asks which harnesses get the skill, and `canon migrate` moves a 0.16 repo over.

### Breaking
- The spec layout drops the `features/` level and the default spec root is `.canon/specs` (was `specs`); a root still holding `features/` is refused, naming `canon migrate`.
- `Approved:` is read only from `.canon/changes/<change>/brief.md`; an external plan gets a pointer `brief.md` (`Source: <path>`).
- `canon status --json` is `statusVersion` 3: the `brief` object is `{approved, enforced, brief?, briefExists, source?, …}` and `proposal` is gone.
- `canon change new` writes `.canon/changes/<slug>/brief.md` and `tasks.md` instead of `openspec/changes/<slug>/`; `canon init` no longer creates `openspec/changes/`, and `plans:` is `sources: []`.
- Skill providers are no longer inferred from `.agents` or a Claude+Codex fallback; with no install lock and only `.agents`, pass `--providers`.

### Added
- `canon migrate [--dry-run] [--repo <dir>]` moves a 0.16 or earlier repo to the 0.17 layout, idempotently; it exits 2 and writes nothing on uncommitted changes under a path it moves.
- `.canon/changes/` is a native plan source (dialect `canon`), read first by `ingest plans`, `gate task`, `evidence add --task`, `dispatch --task` and `status`.
- `canon.yaml` gains `skills: { providers: [claude, codex, omp, pi] }`; `canon init` asks which harnesses get the skill, or takes `--providers a,b|none`; `skills doctor` flags `stray:`.

### Changed
- `canon skills install|check|doctor` follow `skills.providers`, else the install lock, then detected harnesses; `--providers` on install rewrites `canon.yaml`.
- `canon init` on an existing repo also adds missing `.gitignore` lines.

### Upgrading
1. Install 0.17.0, run `canon migrate --dry-run`, commit anything uncommitted under a path it moves, run `canon migrate`, and commit the result.
2. Run `canon skills check` and `canon skills doctor`; update `canon status --json` readers from `brief.proposal` to `brief.brief`, `brief.briefExists`, `brief.source`.
3. Record approvals in `.canon/changes/<change>/brief.md`; repos with `{ dialect: openspec, root: . }` keep it as an external source.

## 0.16.0 — 2026-10-12

After one human approval the agent can drive on its own: one approval per scope, promote on write, a self-refreshing scenario index, policy levels, review artifacts, and checked legacy reviews.

### Breaking
- `evidence add` under an unapproved brief and `subject status --to shipped` without `Approved:` refuse with `unreviewed-promotion <subject>` (exit 1); `--override-reason` waives.
- `gate check` and shipping exit 1 with `uncovered-cell <root>` when a changed root's index refresh is refused.
- `canon status --json` is `statusVersion: 2` (`next[].actor`, `subjects[].brief`, `policy.level`); the human step text changed.
- The `strict` starter policy now begins `# canon policy level: strict` (new `policy_digest`); `canon skills check` exits 1 when the lock's `canon_version` is missing or not this canon's.

### Added
- One approval per scope: only a new subject or scope change needs the human; `Approved:` line plus `canon policy approve --by <human> --note "<words>"` approves both.
- `canon evidence add --override-reason <line>` attests despite a brief refusal; the record gains `brief_override`, listed by `status` and `gate check` without failing.
- `--promote`/`--no-promote` on write commands; `write: { promote: auto }` in `canon.yaml` promotes on write (`canon init` sets it for new repos).
- A stale scenario index is rebuilt from `.feature` files by `gate check`, `status`, `subject status`, `review add`, `evidence add`; removed scenarios get a `removed: true` generation.
- `canon init --policy light|standard|strict` (default `strict`); `review add --artifact <path>` stores files under `.canon/artifacts/sha256/`; the skills lock records `canon_version`.

### Changed
- Pre-0.15 reviews are checked against the scenario at the pinned commit (undecidable is never a violation); `evidence add --task` and `gate task` read `tasks.md` as is.

### Upgrading
1. Re-run `canon skills install` (with your `--providers`) and `canon init` (refreshes the `AGENTS.md` block); until then `skills check` exits 1.
2. Add `.canon/audit/`, `.canon/scenario-index.lock`, `.canon/artifact-store.lock` to `.gitignore`; set `fetch-depth: 0` on the CI checkout running `gate check`.
3. Record each brief approval as an `Approved:` line or pass `--override-reason`; update scripts matching `status` step text or parsing `--json`; optionally add `write: { promote: auto }`.

## 0.15.0 — 2026-10-11

Reviews pin what they saw, re-attesting cannot silently drop bound files, external audits sit beside declared evidence, and the policy gets an approver.

### Breaking
- A new latest `canon evidence add` record omitting files the previous one bound is refused (exit 2); re-bind them or pass `--drop-attachment <path>` with `--drop-reason <text>`.
- The failure-class list has ten classes: `false-success` is added; a latest `refuted` audit is `false-success`, refused by the ship guard even with `--override-reason`.
- Under `require_review`, only a `signed` review qualifies, and a pinned one only while its pins match; otherwise `unreviewed-promotion` ("stale review").

### Added
- `canon review add` pins the scenario digest and latest evidence records; `--outcome signed|indeterminate|unavailable` (default `signed`), `--reviewer-provider|-model|-harness`.
- `canon evidence add --kind audit` records an external re-check (`--evaluator-id|-version|-digest`, `--audit-result confirmed|refuted|indeterminate`).
- `spec_coverage.require_audit: { rate, seed, scope }` samples scenarios deterministically; a sampled one without a confirmed audit is `uncovered-cell`.
- `canon policy approve --by <who> [--note]` writes `.canon/policy.approval.json`; `gate check` and `status` advise while the policy is unapproved or changed.
- `canon status` counts stale/unsigned reviews and audits (`--json`: `staleReviews`, `unsignedReviews`); `gate task` warns when evidence has no attachment and its ref is a canon command.

### Changed
- Pre-0.15 reviews carry no pins and keep qualifying; `finding add|close` resolve short SHAs (unknown refused); the access audit is written beside the `canon.yaml` read.

### Upgrading
1. Re-review (`canon review add`, same reviewer) to pin a scenario; until then it can change without the review going stale.
2. Have a human run `canon policy approve --by <who>` and commit `.canon/policy.approval.json`.
3. Update scripts that parse the failure-class list or re-attest with fewer files; re-run `canon skills install`.

## 0.14.0 — 2026-10-11

The skill becomes a working loop for agents, `canon init` sets that loop up, bound evidence stays provable, units and reviews carry a session, and `canon status` says what to run next.

### Breaking
- Write-command success lines now end with the write mode (e.g. ``— written directly; nothing to promote``); `finding add --introduced-by self` is refused (use `--reviewer <me>`).
- `evidence add --report` refuses a `faithful` verdict when a bound case failed; `require_review.distinct_actor` also rejects a review whose session equals an evidence session.

### Added
- `canon status [--json]` lists subjects, gaps, policy warnings, ends with `next:`; `canon change new <slug> --subject <id> --title <t>` scaffolds, imports, adopts a change.
- `canon init` writes a starter `.canon/policy.yaml`, an `AGENTS.md` block and an `openspec` plans source (`--no-agents-md`, `--no-policy` opt out); `--session-id` on evidence/review/finding.
- `evidence add --artifact/--report` copies bound files into git-tracked `.canon/artifacts/`; `canon evidence vault` backfills; `--max-artifact-mib <N>` raises the 25 MiB limit.

### Changed
- `gate check` re-checks bound files against stored blobs (`stale-evidence`); a report case binds a scenario only by its own id; `gate task` re-imports the flipped change.
- Re-running `canon init` leaves `canon.yaml` alone, refreshes only the `AGENTS.md` block, exits 0; the skill opens with an 8-step loop.

### Fixed
- `gate task` is no longer slow with large plans-root directories; a plan import keeps the change's `subject_id` when the plan names none.

### Upgrading
1. Add `{ dialect: openspec, root: . }` to `plans.sources` (pre-0.14 repos keep `[]`) for `change new`; commit `.canon/artifacts/`; run `canon evidence vault` once.
2. Re-run `canon skills install`; update scripts parsing write-command output; if an adopted change lost its subject, run `canon subject adopt <change> --subject <id>`.

## 0.13.1 — 2026-10-10

Codex agents can now discover canon: the Codex skill is installed where Codex reads it.

### Fixed
- `canon skills install` projects Codex to `.agents/skills/canon/SKILL.md` (the old `.codex/skills/canon.md` was never read) and migrates the legacy projection, keeping edited files.
- `canon skills check` (exit 1) and `canon skills doctor` report any remaining legacy projection with the fix command.

### Upgrading
1. Re-run `canon skills install`; Codex does not see canon until you do.
2. If you use the developer-only `canon/skills-dev` materializer (now writes `.agents/skills/<name>/SKILL.md`), delete old `.codex/skills/<name>.md` files by hand.

## 0.13.0 — 2026-10-09

`canon context --json` carries a typed policy summary, and unknown `policy.yaml` keys fail the gate.

### Breaking
- In `canon context --json`, `spec_coverage`, `evidence_binding`, `review.requireReview` are JSON objects, not strings; unusable is `{"invalid": "<detail>"}`. `capabilityVersion` is 5.
- `canon gate check` reports an unknown top-level `policy.yaml` key (e.g. the typo `spec_coverag`) as `uncovered-cell <key>`, exit 1.

### Upgrading
1. Update consumers that parsed the old policy strings to read the fields directly and check for an `invalid` key instead of an `INVALID` prefix.
2. Fix or remove any unknown top-level key in `.canon/policy.yaml`.

## 0.12.0 — 2026-10-09

Ledger commands fail loudly on an invalid `canon.yaml`, and an opt-in independent-review gate is added.

### Breaking
- Commands that read or write the ledger exit 2 naming `canon.yaml` and the parse error instead of silently using the default ledger; a legacy `tiers: git:` layout now refuses.

### Added
- Opt-in `spec_coverage.require_review` (`scope`, `distinct_actor`, `block_on_findings`): `gate check` reports in-scope scenarios without another actor's review as `unreviewed-promotion`.
- Ninth failure class `open-blocker`: an open `blocker` finding on a change the subject adopted.
- `canon subject status` runs the same checks on entering a scoped status and refuses on gaps; `--override-reason "<one line>"` (with `--actor-id`) waives only `unreviewed-promotion`/`open-blocker`.
- With `block_on_findings`, an unreadable finding record is a `malformed-evidence` violation that no override waives.
- `canon context` lists finding and review fields and the `require_review` setting; `--json` adds a `review` object and `capabilityVersion` 4.

### Upgrading
1. Fix any `canon.yaml` that only worked through the old fallback; rewrite a legacy `tiers: git:` layout to `tiers: { local: { backend: git, root: .canon/ledger } }`.

## 0.11.0 — 2026-10-09

Experimental, off-by-default binding of evidence to files your own runner or agent produced; canon still never runs a test.

### Added
- `canon evidence add --artifact <path>` binds any repository file by sha256.
- `canon evidence add --report junit:<path>` or `cucumber:<path>` parses the report, records the matched case and outcome, and refuses a `faithful` verdict over a failed case.
- Policy `experimental.evidence_binding` with `mode: off | warn | require`, minimum `strength`, and optional filters; repos that do not set it are unaffected, and the shape may change.

## 0.10.0 — 2026-10-08

Golden-path-only specs become visible through scenario case tags.

### Added
- `@case:<value>` scenario tag (`happy`, `failure`, `edge`), set by `canon scenario new --case` and indexed by `canon inventory sync`.
- Opt-in `spec_coverage.require_cases: [failure]` reports feature surfaces with no failure-path scenario; `canon subject status <id> --to shipped` refuses on the same rule.
- `canon context` prints `exclude_lanes` and `require_cases`.

### Upgrading
1. Before setting `require_cases`, tag existing failure-path scenarios; untagged scenarios satisfy no case.

## 0.9.1 — 2026-10-07

Fixes the `@journeykit/canon` launcher's exit status.

### Fixed
- The launcher in 0.8.0–0.9.0 always exited 0, hiding refusals and a red `canon gate check`; it now passes the binary's exit code through.

## 0.9.0 — 2026-10-07

The Gherkin `@subject:<id>` tag becomes the only subject-to-scenario link.

### Breaking
- `canon report`'s Subjects panel and the `verifying → shipped` gate count scenarios carrying `@subject:<id>` in the latest `canon inventory sync`; shipping is refused when a subject owns none.

### Removed
- The never-populated `Subject.scenario_ids` list (older records still read).

### Upgrading
1. Tag each subject's scenarios with `@subject:<id>`.
2. Re-run `canon inventory sync` before moving a subject to `shipped`.

## 0.8.0 — 2026-10-03

Provider projections consolidate into a single `canon` skill, with stricter signed-approval and quarantine boundaries.

### Breaking
- Ingest is metadata-only: source bytes stay external while canon records metadata and provenance.
- Signed approval and quarantine workflows have stricter requirements.

### Changed
- Provider projections are consolidated into a single `canon` skill.

### Upgrading
1. Update integrations to the current parser generations before migrating.
2. Review existing approval and quarantine workflows against the stricter requirements.
