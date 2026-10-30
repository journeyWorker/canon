# Changelog

The full release notes before 0.32 are readable with `git show e304d13b:README.md`.

## Unreleased

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

Id options lose their `-id` suffix, briefs drop HTML comments, and `canon search` / `canon trace` are new.

### Breaking
- The 14 options that took a record or config id lose their `-id` suffix on every command: `--actor`, `--session`, `--project` (the spec root's id; the repository directory stays `--repo`), `--change`, `--run`, `--task`, `--subject`, `--scenario`, `--skill`, `--parent-run`, `--evaluator`, `--review`, `--plugin`, `--contradicting-trajectory`. Values, defaults, `CANON_ACTOR`, exit codes, stored records and report JSON are unchanged.
- The old `-id` spellings have no alias: they fail with `USAGE` (exit 2) before anything is written. `error.field` is the spelling typed, `error.expected` the new option, and `error.hint` the typed line with the old spellings replaced.
- `--input` document keys follow the option names: `scenarioId` is now `scenario`, `actorId` is `actor`, `parentRunId` is `parentRun`, and so on. Old keys fail with `INPUT_UNKNOWN_FIELD` (exit 2, nothing written); the hint names the new key. Record keys, report keys, error JSON keys, `--fields` values and dashboard HTTP request keys keep their `*_id` / `*Id` names.
- `canon status --json` `statusVersion` is `4`. A subject's `brief` object is gone; read `briefs` instead, a list with one entry per change the subject answers for: `change`, `brief` (path), `status` (`approved`, `unapproved`, `changed`), `reason` (`tagged`, `approved`, `chore`; absent for a subject with no such change) and `findings` (check counts). `pendingDrafts` holds only pending drafts, and `flow` the steps of every tagged change.
- A brief is now frontmatter, a title, `##` sections and the `Approved:` line, with no HTML comments. Sections are found by exact English heading (`Intent`, `Desired outcome`, `User flow`, `Step detail`, `Acceptance`, `Questions and assumptions`, `Out of scope`; chore: `Intent`, `Category`, `Invariants`, `Checks`, `Out of scope`). `language:` only picks the language of prose rules and guidance, never the headings.
- Waivers are entries of the frontmatter `waive` list (`rule`, `section`, `reason`, optional `match`). The approval digest covers the frontmatter, so adding a waiver after approval is a change after approval. A chore has `kind: chore` in the frontmatter.
- A 0.30 brief (`canon:` comments or Korean headings) gets one blocker, `brief.old-format`. `status`, `gate check`, `context` and `brief show` warn and point to `canon migrate --json`. `evidence add`, `subject move` to ship or reopen, `subject adopt`, `scenario new --flow` and `review change` refuse it with `OLD_BRIEF_FORMAT`.
- Exit codes: not-found and invalid input now exit 2; exit 1 stays for check results and refused verdicts.
- `canon adapter authorize` exits 2 (`ADAPTER_REFUSED`) for a response whose envelope breaks a rule and for a broken policy file (was 1); exit 1 there is only `ADAPTER_EXECUTION_NOT_AUTHORIZED`. `adapter validate` keeps exit 1 for a rule-breaking envelope and exit 2 for an unreadable file.

### Added
- `canon search "<text>"` finds records, brief sections, flow steps and scenario text (title, tags, steps, doc strings, tables, `Examples`, inherited `Background`) and lists each hit with its neighbors. `data` holds `hits[]` (`id`, `kind`, `rank`, `score`, `title`, `snippet`, `placeholder`), `neighbors[]` (`id`, `kind`, `hops`, `via`, `from`, `title`, `placeholder`, `refs`), `edges[]` (`from`, `to`, `rel`), `counts` and `index`.
- `canon search` options: `--kind` (repeatable; record kinds and `brief`) filters hits, `--expand <N>` sets the neighbor hop count (default 1), `--limit <N>` the hit count (default 20). The text is literal (no query syntax); text of three or more characters uses a trigram match (works for Korean), shorter text is a substring match. No match is exit 0 with empty `hits`; empty text is exit 2 (`SEARCH_QUERY_EMPTY`).
- Search node ids: `finding:<change>_<round>_<seq>`, `review:<project>_scenario_<id>`, `scenario:<project>_<scenario id>`, `brief:<change>#<section>`, `brief:<change>#F1` for flow steps. Edges come from each kind's reference fields plus `re-review`, `fix-reviewed`, `fix-of-fix`, `evidence-at-commit` and `flow`. References no committed source holds appear as placeholder nodes (`placeholder: true`, `refs`).
- `canon trace <finding key|review id|sha>` prints one review loop in time order: `data.chain[]` entries with `at`, `id`, `kind`, `role`, `sha`, `version`, `archived`, `edges[]`, including archived old versions. A finding appears as a `finding` entry and a `fix` entry. The operand is a 26-character review id, a commit sha or its first 7–40 hex digits, or a finding key; anything else is exit 2 (`TRACE_NOT_FOUND`, `TRACE_OPERAND_INVALID`).
- Both commands keep a derived index at `.canon/index/search.db`, rebuilt in full whenever an input changes; deleting `.canon/index` loses nothing. The first build writes `.canon/index/.gitignore`, and `canon init` adds `.canon/index/` to the repo's `.gitignore`; until then both commands warn. An unwritable directory builds the index in memory with a warning (`data.index.inMemory`). Staged records and records routed to postgres or an object store are not read (`data.index.unreadStores`). Unreadable records are exit 1 (`SEARCH_INDEX_FAILED`).
- `canon brief approve` takes one or more change ids with one name, answer and timestamp. All ids are checked first; any refusal (a blocker, no brief, an open code fence) fails the call with `BRIEF_APPROVE_REFUSED` (exit 2) naming each refused id and its code, and nothing is written. With several ids, `--json` has one entry per id (`written`, `unchanged`, `failed`, `skipped`). The `canon dashboard` briefs page can approve several briefs at once.
- `canon change new` writes empty sections; `canon brief check` reports each with its guidance in the hint, and `canon brief show` lists them (`emptySections` in JSON).
- `canon migrate` also rewrites 0.30 briefs: the preview says whether it re-binds `Approved:` lines or chore review pins, and `--commit --plan-digest` applies it. A `--commit` that re-binds approvals is human-only (`HUMAN_ONLY` for every agent role); a plan that re-binds nothing may run under an orchestrator. A brief edited after approval, or whose marker role differs, is rewritten but not re-bound and is named.
- A brief frontmatter that is not YAML or has an unknown key is a blocker (`brief.frontmatter-invalid`), and no waiver in it is read. A misspelt heading (`## Intnet`) is reported with the nearest valid heading.
- `canon context-pack show` and `verify` accept only `sha256:` plus 64 lower-case hex digits; anything else is `USAGE` (exit 2).

### Changed
- A subject answers for every adopted change that is tagged (`@flow:<change>.*`), has an approved line, or is a chore; reopen, ship and `canon status` judge all of them, regardless of adoption or approval order. A never-approved, untagged feature change is a pending draft and blocks nothing. A tagged scenario's evidence answers to the changes it tags, an untagged one to every approved-line change; an unapproved chore blocks reopen and ship, not evidence. A subject with no governing change keeps the old rule: its newest adopted change awaits its first approval.
- `canon demo init` refuses any directory that holds a `canon.yaml` (`INIT_CONFIG_EXISTS`, exit 2) and leaves its policy file untouched.
- `canon gate install-hooks` refuses a `--repo` that is not an existing directory (`HOOKS_INSTALL_FAILED`, exit 2, also under `--dry-run`). Its output names the two agent hook files and the script, says no git hook was installed, and says to copy the script into git's pre-commit hook or call it from a hook runner.
- `canon evidence approval-payload` refuses a `--scenario` missing from the scenario index (`EVIDENCE_SCENARIO_NOT_INDEXED`, exit 2; run `canon inventory sync`).
- `canon strategy rollback` looks the strategy id up first: `LEARN_STRATEGY_NOT_FOUND` (exit 2). A signature that does not verify stays `LEARN_ROLLBACK_FAILED` (exit 1).
- `canon report build`, `check` and `snapshot` fail (exit 1) naming the path when the hot path is a directory or not a SQLite database, as `canon query` does; the dashboard starts and says why it has no snapshot. A path with nothing at it still reads as no hot records.
- `canon schema`, the skills and the website use the new option names only.
- The dashboard hint for an unattributed run names `canon ingest records --run` instead of the removed `ingest artifacts`.

### Fixed
- Commands that take no `--repo` (`canon schema`, `canon skills show`, ...) no longer fail with `CONFIG_MISSING` outside a repo in release builds.
- `canon migrate --revert` refuses, naming the file, when a file the migrate left was deleted since, when the review version the migrate wrote was deleted, and after a resumed run that never sealed its backup.
- The old-spelling hint for a line mixing removed options (`subject adopt --change-id ... --subject-id ...`) runs as printed in either order.
- Skill and website pages no longer cite commands that do not exist (`canon review add`, `canon divergence promote`, `canon ingest artifacts`).

### Upgrading
1. Run `canon migrate` once on a repo with 0.30 briefs, then have the human run `canon migrate --commit --plan-digest <digest from the preview>`. Until then `canon evidence add` and the other writes listed under Breaking refuse with `OLD_BRIEF_FORMAT`. `canon migrate --revert` undoes it.
2. Rewrite old option spellings in scripts and CI using the table below (the first run also prints the corrected line in `error.hint`), and rename `--input` document keys the same way (`scenarioId` to `scenario`, etc.).

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

3. Tools that read `canon status --json` must read `briefs` instead of the subject's `brief` object (`statusVersion` 4).

## 0.30.0 — 2026-10-27

Stored records get one naming rule and `evidence_record` becomes `evidence`; `canon migrate` moves a 0.29 repo over, and review loops get linked ids.

### Breaking
- Every store (git ledger, archive, staging, plugin overlays, sqlite and Postgres keys, R2 object keys) names a record `kind=<kind>/<key components joined by _>_<digest8>.json`. The `area=` level is gone from ledger paths (spec `.feature` trees keep theirs). A kind with a `project_id` starts with it (`-` when absent); the last part is the first 8 hex digits of the content's SHA-256; a review's 40-hex pin leaves the name; free-text ids escape bytes outside `[a-z0-9.-]` as `+` and two hex digits. The same key with different content is refused in every store. Example: `kind=review/canon_scenario_gate.check.104_e7f52dd1.json`.
- The kind `evidence_record` is now `evidence` on disk, in the record's `kind` field, the schema, `canon.yaml` `routing:` and aging keys, and report views. Evidence with no task, scenario or run key is refused.
- A 0.29 repo warns on read commands and refuses write commands, naming `canon migrate` as the next step.
- Query cursors from 0.29 are rejected with a hint to start the query again.
- A new blocker or should-fix finding needs `--introduced-rev <rev>` or `--introduced-unknown`.

### Added
- `canon migrate` previews the plan and its digest and writes nothing; `canon migrate --commit --plan-digest <digest>` moves ledger, archive, staging and overlay files, rewrites evidence bodies, `canon.yaml` keys and `.gitignore` lines, re-keys sqlite rows (and configured Postgres and R2 keys), and backs up rewritten files under `.canon/migrate-backup/<stamp>/`. `canon migrate --revert [--backup DIR]` previews the undo of the last migrate; `--revert --commit --plan-digest <digest>` restores it, including Postgres and R2 keys.
- `canon finding add --review-id <id>` links a finding to its review (renamed `--review` in 0.31).
- `canon review scenario` and `canon review change` give each run one review id (a ULID in the record body; a skipped repeat shows the earlier id).
- Scenario index records carry their Given/When/Then steps; scenario digests are unchanged.
- `canon query` marks references to records no committed source holds, and `canon status --json` counts them in `danglingReferences`.
- `canon init --check` and every command that writes to the hot store print the hot store location.

### Upgrading
1. Run `canon migrate`, review the plan, then `canon migrate --commit --plan-digest <digest from the preview>`.
2. Restart any paged `canon query` from the first page.

## 0.29.0 — 2026-10-25

`canon gate check`, `canon status` and batch `canon evidence add` run much faster on large repos, with identical output.

### Changed
- `canon gate check`, `canon status` and batch `canon evidence add` are much faster on large repos; their output is unchanged (`--json` of `gate check` and `status` is identical to 0.28.0 apart from the version string).

## 0.28.0 — 2026-10-24

One grammar for the whole CLI, generated from one command spec. Old spellings fail with exit `2` and an `error.hint` naming the replacement; there are no deprecated aliases.

### Breaking
- Every group now requires a subcommand: `canon brief` is `canon brief show`, `canon report` is `canon report build`.
- A command's primary object is now its operand: `canon brief approve add-login`, `canon subject adopt demo --change add-login`, `canon task done add-login#1`, `canon prompt add reviewer@v1`.
- Option names have one meaning everywhere: `--kind` is only a record kind, `--role` is only the role a record is attributed to (default `--as`), `--actor-id` is who acts, `--approver` is the approving human, `--rev` is the commit a record is about, `--limit` pages.
- Modes that change a command's options are separate commands: `report build|check|snapshot`, `review scenario|change`, `evidence add|audit`, `strategy promote|rollback`.
- `--project-id` is required only when `canon.yaml` configures more than one spec root.
- `--json` prints exactly one envelope on stdout and nothing on stderr: `{"ok", "data", "meta"}` or `{"ok": false, "error", "meta"}`. Every report is wrapped, so read `data`; each report's `schemaVersion` starts at `"1.0"`. Warnings go to `meta.warnings`.
- Report, error and `--input` keys are camelCase. Renamed keys include `schema_version`, `candidate_strategy_id`, `evaluation_digest`, `approver_identity`, `approver_role`, `approved_at`, `payload_hex` (`strategy approval-payload`); `strategy_id`, `rolled_back_at`, `approval_digest`, `contradicting_trajectory_id`, `outcome_provenance` (`strategy rollback`); `run_id`, `injected_guidance`, `task_id`, `parent_run_id`, `tier_degraded`, `ended_at` (`dispatch begin|end`); `change_id`, `resolution_sha`, `file_ref` (`finding list`, including `--fields` names); `as_of`, `project_id`, `scenario_id`, `still_divergent`, `resolved_invalid` (`divergence list`); `source_scope`, `record_count`, `content_digests`, `sensitive_records` and `out` → `output` (`export` manifest); and the `ingest sessions` metadata keys (`client_id`, `files_scanned`, `rows_parsed`, `session_id`, `workspace_key`, `run_id`, and so on). The `policy.*` keys of `canon context` keep the snake_case spelling of `policy.yaml`.
- `canon context` keys record kinds by the name `--kind` takes (`evidence`, not `evidence_record`).
- `canon schema --all` is gone; use `--as <role>` to filter.
- `finding close` reports a repeat with `meta.unchanged: true` instead of `FindingCloseReport.unchanged`.
- `purge`, `tier age`, `ledger compact`, `artifacts move` and `migrate` preview by default: a bare run prints `planDigest` and `effects[]` and changes nothing. `--commit --plan-digest <DIGEST>` acts, and refuses with `PLAN_CHANGED` if the plan changed since the preview. Their `--dry-run`, and `purge --plan` / `purge --confirm`, are removed.
- Revision options are unified: `--rev <REV>` replaces `--pin`, `--sha`, `--artifact-sha` and `--reviewed-sha`; `--resolution-rev` and `--introduced-rev` replace `--resolution-sha` and `--introduced-by`.
- Error codes renamed: `ARTIFACTS_MIGRATE_REFUSED` → `ARTIFACTS_MOVE_REFUSED`, `EVIDENCE_VAULT_FAILED` → `ARTIFACTS_STORE_FAILED`, `FINDING_RESOLUTION_SHA_UNEXPECTED` → `FINDING_RESOLUTION_REV_UNEXPECTED`.
- Error codes removed: `PURGE_PLAN_REQUIRED`, `FINDING_RESOLUTION_SHA_REQUIRED`, `FINDING_UNRESOLVABLE_COMMIT` (an unknown revision is now `NO_COMMIT`).
- Exit codes: `brief check` with blockers exits `1` (was `2`); every `*_NOT_FOUND`, `*_UNREADABLE` and `*_INVALID_ARGUMENT` exits `2`; `init --check` of a malformed config exits `2`; `task done` on an unknown task exits `2`.
- `BRIEF_HAS_BLOCKERS` is now only for `brief check` (exit `1`); `brief approve` on a brief with blockers fails with the new `BRIEF_APPROVE_REFUSED` (exit `2`).
- `brief approve`, `policy approve` and `divergence accept` are human-only; agent roles are refused with `HUMAN_ONLY`.
- `canon skills install` writes a short `SKILL.md` stub and the retrieve hook script instead of the full guide and `reference/**`; `skills check` reports a stub from another canon version as outdated.
- A check that examined nothing reports `UNVERIFIABLE` (exit `1`), never clean.
- Removed 0.27 spellings are listed in the table under Upgrading.

### Added
- `--input` on every write command takes its arguments as one JSON document: `--input '{"operands": ["auth.login.01"], "title": "Sign in"}'`, `--input file.json`, or `--input -` (stdin; a terminal is refused). Keys are the camelCase option names plus `operands`.
- A JSON array (or NDJSON from a file or stdin) given to `--input` is a batch: all elements are validated first, then run in order. The reply is a `Batch<Name>Report` with one `ok`, `unchanged`, `failed` or `skipped` item per element in `data.items[]`; re-running is safe. Batch variants exist for 41 write leaves; destructive commands and the `dashboard` stream refuse a batch (`INPUT_BATCH_UNSUPPORTED`).
- `canon skills show <NAME> [--full]` prints the guide embedded in the binary (`canon` for the core guide, or a role or topic name); it returns `{name, version, markdown}`. `--full` prints the core guide followed by every reference.
- `canon schema <command> --json` shows each command's `effect` (`read`, `write`, `destructive`), `idempotency`, `roles`, `exits`, error codes, input and output schemas, and one runnable example. `canon schema --errors --json` lists every error code with its exit, retryability and fix.
- `meta` carries `command`, `schemaVersion`, `canonVersion`, `context` (role, actor, repo), `next` (each step with `command` and `argv`), paging, `warnings` and `coverage`; `meta.unchanged` is `true` when a repeat changed nothing. A failure also carries `data` when there is a partial report (for example `gate check` violations).
- `error` carries `code`, `message`, `field`, `expected`, `received`, `suggestions[]` and a `hint` that is a working command.
- `canon context` gains a `commands` index (filtered by `--as`); each record kind links its schema. `--fields` accepts any top-level name in camelCase or snake_case; an unknown name is `UNKNOWN_FIELD` (exit `2`).
- `--repo <DIR>` is declared once for all commands; absent, canon uses the nearest ancestor holding `canon.yaml`.
- `--session-id` beside `--actor-id` on `subject new`, `subject move`, `scenario new`, `feature new`, `dispatch begin`, and `divergence add|defer|resolve`.
- Separate reports for the split commands: `evidence audit`, `review scenario` and `review change`, `report build|check|snapshot`; `scenario new` has a single and a batch report.
- `canon subject move --to` changes a subject's state; `canon subject show` replaces `subject status`.
- `--dry-run` stays on the other write commands where a preview helps.
- `--override-reason` is the only "proceed although refused" flag. The waiver is recorded and echoed in `data.overrides[]` (`{code, reason}`, always present on `evidence add`, `evidence audit` and `subject move`), and `status` lists it for the human.
- Uncommitted code is recorded, not refused. Reviews, divergences, evidence and findings gain optional `code_tree` and `worktree_dirty` fields (findings also `resolution_code_tree` and `resolution_worktree_dirty`); revision fields are always a full sha. Old records still read. `code.exclude` in `canon.yaml` (default `docs/**`, `*.md`) says what is not code.
- A command that needs a commit outside a git work tree, or with no commit, refuses with `NO_COMMIT`.
- Destructive commands report `planDigest`, `effects[]` and `committed`; several `--dry-run` reports gain a `dryRun` key, and `export --dry-run` returns `{dryRun, output, kind, recordCount}`.
- `canon evidence add --all-scenarios --report junit:a.xml --report junit:b.xml --role implementer` is the batch command named by `status`, `meta.next` and refusal hints. With `--report`, `--class` defaults to `test-run` and `--ref` to the report paths.
- `canon migrate` rewrites canon's own hook commands, the retrieve hook script and the old full skill, and refreshes `AGENTS.md`; its report gains `backupDir`, and its effects are `rewrite`, `write` or `remove` entries with a `path`.
- New error codes: `NO_COMMIT`, `UNVERIFIABLE`, `HUMAN_ONLY`, `UNKNOWN_ERROR_CODE`, `BRIEF_APPROVE_REFUSED`, `SKILLS_INVALID_ARGUMENT`, `CONFIG_CHECK_FAILED`, `PLAN_CHANGED`, and the `INPUT_*` family (`INPUT_INVALID`, `INPUT_INVALID_TYPE`, `INPUT_UNKNOWN_FIELD`, `INPUT_CONFLICT`, `INPUT_UNREADABLE`, `INPUT_TOO_LARGE`, `INPUT_STDIN_CONFLICT`, `INPUT_STDIN_TTY`, `INPUT_BATCH_UNSUPPORTED`, `INPUT_ELEMENT_FAILED`).
- `status`, `gate check` and `context` detect a 0.27 skill lock, hook or `canon gate task` hook entry, warn, and put `canon migrate --json` first in `meta.next`.

### Changed
- An approved chore asks for new evidence only for scenarios whose latest evidence was recorded on a different code tree, so a docs-only chore needs only its change-level review. A review of a working tree also covers the commit made from it.
- A bound report decides the verdict of a single scenario: any failed case is `divergent`, skipped cases with none failed are `not-applicable`, otherwise `faithful`.
- Repeating a write with identical content, and `finding close` on a closed finding, are no-ops reported with `meta.unchanged`. A repeated `init` exits `0` as unchanged.
- `divergence accept` defaults `--role` to `reviewer`. `--role` stays an open slug: an unknown role is a warning with a suggestion, not a refusal.
- `init --check` with a failing section exits `1` with `CONFIG_CHECK_FAILED`. `skills doctor` exits `0` per diagnosis, `2` on refusal, `1` on an IO failure.
- `evidence approval-payload` and `strategy approval-payload` are declared replay-safe.
- `canon completion` and the generated command lists are rendered from the same spec as `canon schema`, so they match the binary.
- A `purge --commit` that matches nothing writes no audit line.

### Removed
- `canon gate task` (use `canon task done`), `canon fmt` alias, `scenario new --from`, `review add --scenario-ids-from`, and the other spellings in the table below.

### Upgrading
1. Preview the migration, then run it with the digest it prints (with `--json`, `meta.next[0].argv` is the second command with the real digest filled in). A copy of every rewritten file is kept in `.canon/migrate-backup/<timestamp>/`, and a repeat finds nothing to do.

   ```bash
   canon migrate
   canon migrate --commit --plan-digest <DIGEST>
   ```
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

4. Update scripts that read `--json` output: read `data` inside the envelope, use camelCase keys, and handle exit `1` (finding) versus `2` (refused).
5. Where you passed `--pin`, `--sha` or similar, use `--rev` (default `HEAD`); where you used `purge --confirm` or `--dry-run` on destructive commands, use the bare preview then `--commit --plan-digest`.

## 0.27.0 — 2026-10-22

Two safety fixes: a gate can no longer pass without canon state, and read commands no longer write.

### Breaking
- A command that needs canon state and finds none now refuses with `CONFIG_MISSING` (exit `2`, field `--repo`, hint naming `canon init` and the directory searched). Before, `canon gate check` in a directory without `canon.yaml`, or with `--repo <subdir>`, printed `clean` and exited `0`. `--repo` absent or `.` finds the nearest ancestor holding `canon.yaml`; any other value is used as given. `init`, `migrate`, `schema`, `context`, `prompt`, `retrieve` and a few others still run without config.

### Fixed
- Every command that `canon schema` marks `effect: read` leaves the tree unchanged: `status` and `gate check` grade a stale scenario index in memory instead of rewriting it, a read of a never-written hot tier creates no `hot.db`, and `report --check` no longer writes into the repo. `query` keeps its gitignored audit line, noted in its `effectNote`.
- `query` fails with an unavailable-tier error when the hot path is a directory or not a SQLite file; only a missing path reads as empty.
- `format` refuses a corpus root that is not a directory.

## 0.26.0 — 2026-10-22

`brief check` now reads a brief in the language it is written in, judges Intent and Questions, and adds six advisory prose rules. The planner is sent through a polish pass before asking the human to approve.

### Added
- `brief check` detects the brief's language from its prose (Hangul → `ko`, Latin → `en`; Kana or Han → `ja` or `zh`, which get structure rules only; a section in another script is judged in its own language). Output prints `language: ko (detected)`; JSON gains `languageSource` and `languageNote` (set when a configured language disagrees with the text).
- Six advisory rules, each waivable with a `waive` entry in the brief's frontmatter (the whole-brief length finding uses section key `brief`): `prose.code-citation`, `prose.parenthetical`, `prose.decision-history`, `prose.ai-vocabulary`, `prose.ko-ending`, `prose.length`.
- When a brief has no blockers but has advisories, `canon status` and `meta.next` give the planner a polish step first (`reference/brief-polish.md`, then `canon brief check`) before the human step. Advisories never block `brief approve`.
- The briefs page shows the advisory total, a per-brief advisory badge, its language, and findings grouped by rule.
- `--project-id` naming no spec root in `canon.yaml` is refused with `UNKNOWN_SPEC_ROOT` (exit `2`) before anything is written, with a hint listing the configured roots. This covers `evidence add` (including `--all-scenarios`), `evidence approval-payload`, `review add`, `divergence add|resolve|defer|accept`, and `scenario new --feature`.

### Changed
- `language:` in `canon.yaml` is now an override of detection rather than the only source.
- Intent and Questions sections are judged by the phrase rules and the new rules; briefs may report advisories they did not before.
- `canon init` and `canon migrate` ignore lines also cover `.canon/dashboard-snapshot/`, `.canon/learn/`, `.canon/r2/` and `.canon/archive/kind=scenario/`; `migrate` adds a missing line without duplicating its comment. `canon skills doctor` and `canon init --check-config` name a missing line.
- The skill's `brief-polish.md` is rewritten per rule, and `role-planner.md` gains a compression pass.

### Fixed
- The dashboard drains the request before closing, so a client always receives an early refusal.

### Upgrading
1. In an existing repo, run `canon migrate` once to add the new ignore lines.
2. Polish briefs that now report advisories, or waive a finding with a reason. Approval is unaffected.
3. Fix scripts that pass a `--project-id` naming no configured spec root; they now exit `2` (`UNKNOWN_SPEC_ROOT`).

## 0.25.0 — 2026-10-21

Brief approval now holds back only the change it approves, `canon dashboard` works in any repo and can approve briefs, and the spellings deprecated in 0.24.0 are removed.

### Breaking
- The flags and `--kind *_record` values deprecated in 0.24.0 are removed and are usage errors whose hint names the new spelling: replace `--change`, `--project`, `--subject`, `--actor`, `--run` and `--kind evidence_record` with `--change-id`, `--project-id`, `--subject-id`, `--actor-id`, `--run-id` and `--kind evidence`. `meta.deprecated` and `deprecatedAliases` no longer appear.
- `canon finding close --disposition rejected|deferred` now requires `--reason` (`FINDING_REASON_REQUIRED`).

### Added
- `canon dashboard` runs in any repo: the release binary embeds the app. Without the `duckdb` CLI the server still starts and the panels page explains why there is no snapshot.
- `/briefs.html` on the dashboard lists every live change whose brief awaits approval, with its intent, outcome, flow steps, acceptance, questions, assumptions and `brief check` blockers and advisories. Approving there records the same thing as `canon brief approve --change-id <id> --by <name> --note <text>`.
- Dashboard approval is refused when the brief changed after the page loaded (`BRIEF_CHANGED_SINCE_PAGE_LOAD`) or while it has blockers. The server binds 127.0.0.1 and rejects requests with a foreign `Host`, `Origin`, a non-JSON `Content-Type` or a body over 64 KiB (`DASHBOARD_ORIGIN_REFUSED`, `DASHBOARD_BAD_REQUEST`).
- New dashboard error codes: `DASHBOARD_PORT_IN_USE` (use `--port <n>` or `--port 0`) and `DASHBOARD_NOT_BUILT`. A nonexistent `--repo` is `REPO_UNREADABLE` and is not created.
- `canon subject unadopt <change> --subject-id <id>` moves a draft change off a subject. Refused with `SUBJECT_UNADOPT_REFUSED` for an approved brief or while a scenario of the subject is tagged for the change.
- `canon change new --subject-id` and `canon subject adopt` warn (stderr and `meta.warnings`) when the subject is building, verifying or shipped and the brief is not approved.
- `finding close --reason` is stored on the closing version as an optional `reason` field (absent on older records) and shown in `finding list` and JSON; optional for `fixed`.

### Changed
- Evidence for a scenario needs approval only from the changes its `@flow:<change>.<F-id>` tags name, counting only changes its subject adopted. An untagged scenario answers to the subject's newest adopted change whose brief is approved; when none ever was, a scenario evidenced before the subject adopted its first change (without a brief override) needs no brief, and any other answers to the newest adopted change. A task answers to its own change.
- A pending draft (an unapproved adopted change no scenario is tagged for) is outside the subject's scope: reopen, evidence and ship read the newest approved change, and `canon status` lists the draft as awaiting approval. Once a scenario is tagged for it, the subject cannot ship until it is approved.
- `canon skills install` deletes projected files the bundle no longer ships when they still match the install lock, keeps and reports edited ones, and keeps lock entries of unselected providers. `canon skills check` (`SKILLS_DRIFT`) and `canon skills doctor` name each orphan; malformed lock keys are ignored and reported.
- Bundled skill text covers the 0.25 surfaces (dashboard approval, `subject unadopt`, `finding close --reason`).

### Upgrading
1. Replace the removed spellings in scripts (see Breaking).
2. Add `--reason` to `canon finding close --disposition rejected|deferred`.
3. Run `canon skills install` once to prune files 0.24 shipped that 0.25 no longer does.

## 0.24.1 — 2026-10-21

A fixed blocker now re-verifies only the scenarios it names.

### Added
- `canon finding add` takes a repeatable `--scenario-id`, validated against the scenario index (`FINDING_UNKNOWN_SCENARIO` for an unknown id). The Finding record stores it as an optional `scenario_ids` list, omitted when empty, so older records read unchanged. `canon finding close` keeps the list of the version it closes.

### Changed
- After a `fixed` close of a finding with `scenario_ids`, only the named scenarios the subject owns and has evidence for need evidence recorded after the close. `canon gate check`, the ship guard and `canon status` apply the same rule. A finding naming no scenario keeps the previous rule (every evidenced scenario of the subject).
- The refusal and `meta.next` for a scoped finding name a per-scenario `canon evidence add --project-id <root> --scenario-id <id> …` for each spec root instead of the whole-suite batch.

## 0.24.0 — 2026-10-19

Each concept gets one flag name across every command, and evidence batches bind each attachment to the scenario it belongs to.

### Breaking
- `evidence add --all-scenarios` refuses a bare `--artifact <path>`, which would bind to every scenario. Use `--artifact <scenario-id>=<path>` to bind one file to one scenario, or let the report carry attachments per test.
- A report attachment that is missing or over the size limit is refused, naming the case and path; nothing is written.

### Added
- Reports can say which file belongs to which test: JUnit `[[ATTACHMENT|<path>]]` lines in a case's `<system-out>` (relative paths resolve against the report's directory) and Cucumber JSON `embeddings`. A record binds the attachments of the cases it owns, in batch and single-scenario mode.
- `canon schema <command>` lists deprecated spellings as `deprecatedAliases`.

### Changed
- Flags whose value is a record or entity id are now `--<noun>-id`: `--change` → `--change-id` (`brief approve`, `brief check`, `review add`, `scenario new`), `--project` → `--project-id` (`feature new`, `scenario new`), `--subject` → `--subject-id` (`change new`, `retrieve`, `scenario new`, `subject adopt`), `--actor` → `--actor-id` (`feature new`, `learn rollback`, `scenario new`), `--run` → `--run-id` (`dispatch end`, `ingest artifacts`).
- `--kind` values of `query`, `export` and `purge` drop the `_record` suffix (`evidence_record` → `evidence`). The record's own `kind` field, ledger paths and `canon.yaml` routing keys keep `evidence_record`.
- The old spellings still run until 0.25.0. Each use prints `canon <command>: --<old> is deprecated — use --<new> (removed in 0.25.0)` on stderr and adds `meta.deprecated: [{used, use, removedIn}]` to JSON; help hides them. `<kind>_record` is accepted for every `--kind`.
- Hints, `meta.next`, examples and help print the new names.

### Upgrading
1. Switch scripts to the new flag names and `--kind` values above; the old ones warn until 0.25.0.
2. If a batch passed screenshots as bare `--artifact`, let the report carry them per test or use `--artifact <scenario-id>=<path>`.

## 0.23.0 — 2026-10-18

The working set keeps only current records: superseded versions move to `.canon/archive`, evidence no longer carries attachments forward, reviews pin the verdict and checks of the evidence instead of record ids, and artifact bytes can live off git.

### Breaking
- `evidence add --drop-attachment` and `--drop-reason` are removed (usage error, exit 2), along with the `attachment_drop` evidence field and the `EVIDENCE_CARRY_BLOB_MISSING` code. An evidence record binds exactly the files its own write names; nothing earlier is added, so name every file the evidence should bind.
- Review JSON `pins.evidence: [{key, digest}]` becomes `pins.checks: [{role, verdict, checks: [...]}]`; `scenario_digest` and `scenario_digest_scope` are unchanged. `Review.scenario_id` is now optional and `change_id` is new.
- `canon query` reads the ledger only. Add `--history` to include superseded versions from the archive.
- The pre-0.15 review freshness algorithm is removed. A review that predates 0.23 pins does not count until `canon migrate` pins it: `canon gate check`, `canon status` and `canon subject status` print an advisory naming how many scenarios wait, and a transition needing the review is refused as `unreviewed-promotion`.
- `divergence resolve|defer|accept` stage and promote their own record in the same run and print "staged and promoted in this run; nothing to promote". `canon gate promote` summary counts `divergence=N`; `canon divergence promote` commits only staged divergences. `--no-promote` still leaves a record staged.

### Added
- `.canon/archive/kind=<k>/…`: appending a version of a record moves the versions it supersedes out of `.canon/ledger/kind=<k>/…` (same layout and file names, tracked in git), so the ledger holds one file per key. Archived kinds: `change`, `subject`, `scenario`, `finding`, `evidence_record`, `review`, `divergence`.
- `canon query --kind <kind> --history` returns every version, ledger and archive, oldest first.
- `canon ledger compact [--dry-run] [--json]` moves superseded versions of an existing ledger once; idempotent, and `canon migrate` runs it. Exit 0 done, 1 a move failed (`LEDGER_COMPACT_FAILED`; run again), 2 refused (`LEDGER_ARCHIVE_UNAVAILABLE`: the tier root is not a directory named `ledger`).
- `canon.yaml` `artifact_store: { backend, prefix }`. `backend: local` (default, also when absent) keeps blobs in git-tracked `.canon/artifacts/sha256/`. `backend: <tier>` names an `s3` tier of `tiers:` (R2 or S3) and stores blobs at `<prefix>sha256/<digest>` (`prefix` default `artifacts/`); `evidence add`, `review add` and `evidence vault` upload there and keep nothing locally. The existing `artifacts:` block stays the ingest source config.
- `canon artifacts verify [--full]` checks that every file bound by the latest evidence and current reviews is stored (presence and metadata, or with `--full` download and re-hash). Exit 0 all stored, 1 a file missing or altered (`ARTIFACTS_VERIFY_FAILED`), 2 the store is unreachable (`ARTIFACTS_STORE_UNREACHABLE`). The gate asks the same store.
- `canon artifacts migrate --to <tier> [--dry-run]` uploads every local blob by digest, verifies it, writes the `artifact_store:` block, then removes local copies and adds `.canon/artifacts/` to `.gitignore`. A mismatched blob, a non-`s3` target or a bad stored copy is `ARTIFACTS_MIGRATE_REFUSED` (exit 2). An interrupted run leaves a repo the gate still passes; moving bytes back to local is not supported.
- Change-level reviews: `canon review add --change <slug> …` reviews a chore as a whole (pins the brief minus its `Approved:` line; refused for a feature change; `distinct_actor` applies). `canon status` JSON lists `choreReviews` on each subject.
- `canon migrate` re-pins reviews and accepted divergences (0.15–0.22 or pre-0.15) that were current under the rule they were written under to the 0.23 pin, keeping envelope, reviewer and artifacts. Stale ones stay stale. A pre-0.15 commit the clone cannot read pins the review as the scenario stands, and the run says how many. Reviews of scenarios no `.feature` declares are left alone. Idempotent; `MIGRATE_PINS_FAILED` means files moved but pinning did not finish (run again).

### Changed
- A review pins its scenario digest plus, per role, the latest evidence's verdict and check identity (report format and matched case names such as `junit:<case>`, or artifact digests for artifact-only evidence). Recording the suite again with the same verdicts from the same cases leaves the review current; a different verdict, an added or removed case, or a changed scenario stales it, and the message says which.
- `review add` output reads "pinned … at scenario digest … and the verdict and checks of its N role(s) of evidence".
- A chore is satisfied by one change-level review, or by every evidenced scenario having a current signed review recorded after the chore's approval and latest evidence. Re-recording the suite after a chore no longer stales unchanged scenarios.
- A fixed blocker still needs evidence newer than the close; scenario reviews stay current when verdicts and checks are unchanged.
- Evidence, finding and divergence all stage under `.canon/ledger/_staging/` and are committed by `canon gate promote`.
- The gate-check false-success hint no longer suggests `--drop-attachment`.

### Upgrading
1. Run `canon migrate` once and commit the result. It compacts the ledger into `.canon/archive` and re-pins reviews and accepted divergences that were current. Until then, old reviews do not count and the gate reports how many scenarios are waiting.
2. In CI, check out full history (`fetch-depth: 0`) for the job that runs `canon migrate`; otherwise a pre-0.15 review whose commit is missing is pinned as the scenario stands instead of being checked.
3. Remove `--drop-attachment` / `--drop-reason` from scripts and name every file evidence should bind.
4. Add `--history` to `canon query` calls that read superseded versions; update scripts reading `pins.evidence` to `pins.checks`.
5. Optional: run `canon artifacts migrate --to <tier>` to move artifact bytes off git. Without `artifact_store:` nothing changes.

## 0.22.0 — 2026-10-17

Gates now fail closed on policy drift and on unverified fixes, a tracked known gap can be accepted without a false `faithful` verdict, and every refusal carries a hint. The 0.21 deprecated command names are removed.

### Breaking
- `context-pack create`, `prompt register`, `divergence stage`, `demo attest` and the two-positional `subject status <id> <state>` are removed and are now plain usage errors. `meta.deprecated` (JSON) and the `deprecated` field in `canon schema` are gone.
- `canon gate check` exits 1 when `.canon/policy.yaml` differs from its approved digest (`unreviewed-promotion policy — policy changed since approval`) or its approval file is unreadable (`policy approval unreadable`). `--override-reason` does not waive this. A policy that was never approved stays advisory.
- `canon subject status --to shipped` and the reopen are refused with `POLICY_CHANGED_SINCE_APPROVAL` or `POLICY_APPROVAL_UNREADABLE` (exit 1) in those cases.
- A blocker finding closed `fixed` keeps blocking (`open-blocker`) until every evidenced scenario of its change's subjects has evidence newer than the time it was closed. This check is waivable with `--override-reason`.
- After an approved chore change on a subject, shipping is refused (`uncovered-cell`, not waivable) until every evidenced scenario has evidence newer than the approval. `canon gate check` reports it for a subject past `building`.
- `canon status` shows the planner's `canon brief check --change <slug>` step (`actor: agent`, `role: planner`) instead of the human stop while the brief has blockers. Agents that stop on any `human` step now stop only when the brief is clean.

### Added
- `canon divergence accept --project-id <root> --scenario-id <id> --reason <text> --tracking <url-or-ref> [--expiry YYYY-MM-DD] --reviewer <id> [--actor-id <id>] [--session-id <id>] --role reviewer` records a reviewer's acceptance of a scenario whose latest evidence is `divergent` as a known, tracked gap. `--scenario-id` repeats and the call is all-or-nothing.
- The accepted gap is a `Divergence` record with the new status `accepted` (`reason`, `tracking`, optional `expiry`, `evidence` pins); `divergence.schema.json` is regenerated.
- An acceptance covers the cell for the ship guard until new evidence for the scenario arrives or `--expiry` is reached (a date means the start of that day, UTC); after that the guard blocks again and says why.
- `canon divergence accept` is refused with `DIVERGENCE_ACCEPT_REFUSED` (exit 2, nothing written) for a non-reviewer role, for non-divergent evidence, and for the evidence's own actor or session. An empty `--reason` or `--tracking`, an unindexed scenario or an unparseable `--expiry` is `DIVERGENCE_INVALID_ARGUMENT`.
- `canon status` lists `accepted-divergence <id> — <tracking> (expires <date>|no expiry)` (JSON: `accepted_divergences` on the subject entry, omitted when empty); `canon gate check` prints the same lines as an `accepted_divergence` advisory. Neither fails the gate.
- `canon policy show [--json]` prints the policy path, current digest, approval state (`none`, `approved`, `unapproved`, `changed`, `unreadable`), approver, time, approved digest and note, and says when the gate fails closed.
- `canon change new` on a shipped subject prints the reopen command (`canon subject status <id> --to verifying --reason …`) and adds it to `meta.next`.
- The `uncovered-cell` refusal for a divergent verdict names both ways out: fix and re-attest, or `canon divergence accept`.
- The reviewer skill reference gains a "Check the checks" pass (does a test pin behavior the scenario does not require, or skip a Then step it states).

### Changed
- `canon brief approve` writes the approval time as RFC 3339 UTC to the second (`Approved: <who> 2026-10-17T05:12:33Z — "…"`). Older date-only `Approved:` lines still read, as the start of that day.
- Every refusal now has a `hint`: human stderr gains a `hint: …` line and JSON `error.hint` is a string on all refusals. Refusals for missing evidence name the scenarios and the `evidence add --all-scenarios` command.
- `meta.next` after `change new`, `subject adopt` and other writes follows the same brief-first rule as `status`.

### Upgrading
1. Switch scripts to the 0.21 names: `context-pack new`, `prompt add`, `divergence add`, `demo add`, `subject status <id> --to <state>`. Drop any branch that reads `meta.deprecated`.
2. If `canon gate check` now exits 1 on policy, have the human run `canon policy approve --by <human> --note "<words>"`, or restore the approved policy files from git.
3. After a blocker closed `fixed`, or an approved chore, re-record the suite with `canon evidence add --all-scenarios …` and re-review before `--to shipped`.

## 0.21.1 — 2026-10-16

Patch release: clearer refusals for outside-the-repo evidence, `--reason` on every status transition, and an explanation of dropped provenance lines.

### Added
- `canon subject status <id> --to <state> --reason "<text>"` is accepted on any transition and stored on the subject as the optional `status_reason: { reason, actor }` (cleared by the next status write; `subject status <id>` prints it). The reopen (`shipped → verifying`) still requires `--reason`. `subject.schema.json` is regenerated; existing records read unchanged.
- `canon scenario new` (single and `--from`) says to keep the `# canon: {...}` provenance lines when filling in the scenario (stdout line; `meta.note` in JSON).

### Changed
- `canon evidence add --artifact|--report` and `canon review add --artifact` still refuse files outside the repository, but the refusal now has a hint (copy the file under the repository and bind that path) and JSON `error.field` names the flag. Every other `EVIDENCE_BIND_REFUSED` cause also has a hint.
- `SUBJECT_REASON_REQUIRED` now has a hint with the full command.
- The `missing-provenance` violation lists the affected header lines and the line to restore; the `FORMAT_VIOLATIONS` and `INVENTORY_ABORTED` hints repeat the fix.

### Removed
- Error code `SUBJECT_REASON_UNEXPECTED`: `--reason` is accepted on every transition. Drop any branch that handled it.

## 0.21.0 — 2026-10-16

One verb grammar, a `meta.next` field on every write so agents no longer call `canon status` to ask "what now?", declared agent roles, a skill split into a core plus one reference per role, and attachments that carry forward when a scenario is re-attested.

### Breaking
- `canon evidence add --all-scenarios` and single re-attestations no longer refuse with `EVIDENCE_REFUSED` when earlier attachments are not repeated; they carry them forward (see Added). An identical repeated `evidence add` now returns unchanged (exit 0) instead of staging a duplicate.
- Write commands answer `{ data, meta }` with `meta.next`; read `.data`, not the top level. Moved shapes: `dispatch begin` has `run_id` at `.data.run_id`; `export`'s page state (`returned`, `total`, `hasMore`, `nextCursor`) is in `meta`; `brief approve`, `subject *`, `context-pack new`, `prompt add`, `dispatch end`, `learn approve|rollback`, `purge` and `ingest artifacts|plans` JSON are enveloped.
- `review add` with neither provenance flag is now a clap `USAGE` error (was `REVIEW_INVALID_ARGUMENT`). Enum flags (`--kind`, `--verdict`, …) fail as clap closed-set `USAGE` errors that name the flag in `field`.
- `CANON_ROLE` set to anything but `planner`, `implementer`, `reviewer` or `orchestrator` is `UNKNOWN_ROLE`.
- Human output of writes now ends with a `next: <command> — <why>` line (`(+N more)` when there are others); match the leading lines instead of the last one.
- `status` and `meta.next` steps gain a `role` member (`null` for a human step).

### Added
- Verb grammar: `new` creates, `add` appends a ledger record, `list` lists, `show` shows one. New spellings: `canon context-pack new`, `canon prompt add`, `canon divergence add`, `canon demo add`, `canon subject status <id> --to <state>` (`subject status <id>` alone shows the subject).
- `meta.next: [{ command, why, actor, role }]` (plus `nextOmitted`) on every successful write, in the same shape as `status`'s `next[]`. `actor: human` marks steps only a human takes (e.g. brief approval, `role: null`). Covers `dispatch begin|end`, `learn approve|rollback`, `purge`, `export` and `ingest artifacts|plans`. Reads do not carry it.
- Roles: `CANON_ROLE` or the global `--as <role>` (flag wins) declares `planner`, `implementer`, `reviewer` or `orchestrator`; any other value is `UNKNOWN_ROLE` (exit 2, with `field` and `expected`). Unset behaves as before. When set, `status` and `meta.next` keep that role's steps and summarize the rest as `meta.waitingOn` (e.g. `{ "reviewer": 2 }`) and a "waiting on reviewer: 2" line.
- `canon schema` gives every command `roles: [...]`; bare `canon schema` lists the declared role's commands and `--all` lists every command. `--role` on `evidence add` and `review add` defaults to the declared role when it is `implementer` or `reviewer`.
- `canon schema <command>` returns `example` (one minimal invocation) and `requiredOneOf` for flag groups where one of several is required (e.g. `review add` provenance).
- `--fields a,b` on `status`, `export`, `divergence status` and `context`, with `UNKNOWN_FIELD` for unknown names.
- Errors gain optional `field` and `expected` next to `code`, `message`, `hint` (e.g. `subject status --to bad`: `field: "--to"`, `expected: ["specced", …]`).
- Repeating `evidence add` or `review add` with identical input writes nothing, names the existing record and returns `meta.unchanged: true` with exit 0; batches report each unchanged scenario.
- Attachments carry forward: files bound by the latest record of the same fold that a new `evidence add` does not name again are copied into the new record unchanged and listed as `carried` (human: `carried forward N earlier attachment(s) unchanged: …`). A named file replaces its binding. `--drop-attachment <path> --drop-reason <text>` remains the only way to drop one. The audit fold never inherits bindings.
- `EVIDENCE_CARRY_BLOB_MISSING` (exit 2, nothing written) when a carried file's stored blob is missing or damaged; re-bind it, run `canon evidence vault`, or drop it.
- The skill is a shared core plus `reference/role-planner.md`, `role-implementer.md`, `role-reviewer.md` and `role-orchestrator.md`, each ending with a Commands block. The AGENTS block gains a roles line.

### Changed
- `canon schema` marks `subject status` and `format` as `write` (`subject status` only reads without `--to`) and documents per-command exit exceptions in `exits`: `context`, `dispatch diff` and `divergence status` always exit 0 on a successful read; `evidence vault` exits 0 on a partial pass with `notStored` listed; `subject status` exits 0 when it only shows.
- `canon learn promote`: when a blocked promotion's follow-up demotion also fails, the error is `LEARN_DEMOTION_FAILED` (strategy still active) instead of `LEARN_PROMOTION_BLOCKED`.
- Deprecated, still running until 0.22.0: `canon context-pack create`, `prompt register`, `divergence stage`, `demo attest` and `subject status <id> <state>` print `canon <old>: deprecated — use canon <new>` on stderr and add `meta.deprecated: { "use": "<new>", "removedIn": "0.22.0" }` in JSON; `canon schema` marks them `deprecated` and lists them only under `--all`.

### Upgrading
1. Run `canon skills install` to get the 0.21 skill (core plus role references).
2. Switch to the new spellings before 0.22.0: `context-pack new`, `prompt add`, `divergence add`, `demo add`, `subject status <id> --to <state>`.
3. Read `.data` and `.meta` of write commands' JSON (see Breaking for moved fields).
4. Scripts that match the last line of a write's human output should match the leading lines, since `next:` is now last.

## 0.20.0 — 2026-10-15

Agents can drive the CLI without reading `--help`: `canon schema` describes every command, every command takes `--json`, every refusal has a stable error code, lists are paged, and `purge` deletes only a reviewed plan.

### Breaking
- `canon purge --dry-run` is removed. Use `--plan`, then `--confirm <planDigest>`. A bare `purge` is refused with `PURGE_PLAN_REQUIRED`.
- `canon learn request` is removed. Use `canon learn approve`.
- Error lines on stderr are now `canon <command path>: <CODE> — <message>`, optionally followed by `hint: <next step>`. The old `canon <cmd>: refused — …` and `canon <cmd>: <rule> <subject> — …` forms are gone; match on the code instead.
- A refusal that used to print one line per violation (subject, ship and reopen guards) is now one error whose message lists every violation, with the next step in `hint`.
- `query`, `finding list` and `export` return at most 100 records by default, and `divergence status` is bounded the same way. Pass `--all` or follow `nextCursor` to read everything.
- A closed stdout (`canon … | head`) now ends canon by SIGPIPE (exit 141 in a shell) instead of panicking with exit 101.

### Added
- `canon schema [<command path>…] [--json] [--errors]` describes the command tree: bare `canon schema` or a group path lists the leaf commands, a leaf path returns one command with its `effect` (`read`, `write` or `destructive`; `purge` is the destructive one), `supportsJson`, arguments, options, `exits` and `errorCodes`. `--errors` lists every error code. An unresolved path is `UNKNOWN_COMMAND`.
- Every command accepts `--json`. In JSON mode stdout is exactly one JSON document, `{ "data", "meta": { "command" } }` on success; narration and advisories go to stderr. Existing JSON shapes keep their fields (`statusVersion` stays `3`).
- In JSON mode a failure is `{ "error": { "code", "message", "hint", "retryable" } }` on stdout (`hint` is `null` when absent). `gate check` and `format` failures answer one document with `error`, `data` and `meta`.
- Every error has a stable string `code`, for example `SUBJECT_GATE_VIOLATION`, `BRIEF_UNAPPROVED`, `BRIEF_CHANGED_SINCE_APPROVAL`; clap usage errors are `USAGE`.
- `query`, `finding list`, `divergence status` and `export` take `--limit <n>` (default 100, minimum 1), `--cursor <opaque>` and `--all` (conflicts with `--limit`). JSON results gain `meta` with `returned`, `total` (all matched records, across pages), `hasMore` and `nextCursor` (`null` on the last page); human output ends with `N of M shown; next cursor: …` when more remain. A cursor from another listing, or whose last record is gone, is `INVALID_CURSOR`.
- `--fields a,b,c` on `query` and `finding list` keeps only those top-level fields of each record. An unknown name is `UNKNOWN_FIELD`, and the hint lists the known fields.
- `canon purge --kind <kind> --before <time> --plan` prints the records it would remove and a `planDigest`, and deletes nothing. `--confirm <digest>` deletes only if the recomputed plan (kind, cutoff, per-rung counts and the identity of every record, including the learn store) still matches; otherwise `PLAN_CHANGED` and nothing is deleted.
- The skill and the AGENTS block tell agents to start from `canon schema <command> --json`, pass `--json`, read `error.code`, and page with `--cursor`.

### Changed
- Exit codes keep their meaning: 0 ok, 1 gate failure or operational error, 2 refusal or usage error. Per-command exceptions (`context` always 0, `dispatch diff` 0, `evidence vault` 0 on a partial pass) are now listed under `exits` in `canon schema`.
- `fmt` remains an alias of `format`; `canon schema` and error output name `format`.

### Fixed
- `canon … | head` no longer panics. `canon dashboard` still ignores SIGPIPE so a disconnecting client cannot kill the server.
- `canon schema "evidence add"` (the path quoted as one word) is accepted.

### Upgrading
1. Run `canon skills install` to get the 0.20 skill.
2. Replace `canon purge --dry-run` with `canon purge --kind <kind> --before <time> --plan`, then run it again with `--confirm <planDigest>`.
3. Replace `canon learn request` with `canon learn approve`.
4. Scripts that read all records from `query`, `finding list`, `export` or `divergence status` must pass `--all` or follow `meta.nextCursor`.
5. Scripts that matched `refused —` or a rule name on stderr must match the error code instead. Exit codes did not change.

## 0.19.0 — 2026-10-15

The brief becomes a PRD that canon can check: a fixed skeleton with numbered flow steps, `canon brief check`, an approval bound to the brief's bytes, flow-step coverage instead of the task checklist, a short brief for chores, and a configurable language.

### Breaking
- `canon change new` no longer writes `tasks.md`. An existing `tasks.md` is still read; `canon gate task` and `evidence add --task` are unchanged for imported plans.

### Added
- `canon change new` writes the sections Intent, Desired outcome, User flow, Step detail, Acceptance, Questions and assumptions, Out of scope. Each heading follows a `<!-- canon:section <key> role=<normative|context|question> -->` marker, so headings can be in any language. Flow steps carry ids `F1`, `F2`, … and read "sees → does → becomes"; step-detail subheadings follow the same ids in order.
- `canon brief check [--change <slug>] [--json]` (read-only) reports blockers (`brief.*`: missing, duplicate or out-of-order sections, leftover template hints, duplicate flow ids, mismatched detail headings, and, for an approved brief whose subject is building, a flow step with no scenario or no failure case) and advisories (`prose.*` on normative sections: evaluative phrases, essay connectives, contrast rhetoric, colloquial endings in `ko`, degree adverbs, sentence and paragraph length, missing `==**key sentence**==`). Exit 0 clean, 2 on blockers, 1 on an operational error with a string `error.code`.
- `<!-- canon:waive <rule-id> <section-key> "<reason>" -->` waives one rule in one section; the reason is required and the waiver is listed in the output.
- Bare `canon brief` prints the brief's state and the next one to three steps, and never edits.
- `canon brief approve --change <slug> --by <human> --note "<their words>"` writes `Approved: <who> <when> — "<words>" (brief sha256:<digest>)` and records the policy approval in the same call. The digest covers the brief without its `Approved:` lines. It refuses while the brief has structure blockers.
- `canon scenario new --flow <F-id> [--change <slug>]` tags the scenario `@flow:<change>.<F-id>` and refuses a step the brief does not define.
- `canon status` lists each flow step of the subject's current change with its scenarios and their evidence and review state, flags a step with no scenario or no failure case, and shows the brief finding counts. `canon status --json` adds per subject `brief.findings {blockers, advisories}`, `brief.changed`, `brief.legacyApproval` and `flow[]` (`statusVersion` stays `3`).
- `canon change new --kind chore` writes a short brief (Intent, Category `refactor|dependency|build|ci|infra`, Invariants, Checks, Out of scope) marked `<!-- canon:kind chore -->`. A chore adds no scenarios; its evidence is the suite rerun with `evidence add --report … --all-scenarios`, which stales pinned reviews and forces re-review.
- `canon init --language <tag>` writes `language:` to `canon.yaml`. Templates and prose rules ship for `en` and `ko`; other tags get `en` headings and structure rules only. The skill writes the brief and the report in that language.
- The skill gains five references: `brief-shape`, `brief-clarify`, `brief-harden`, `brief-align`, `brief-polish`.

### Changed
- Once the brief changes after approval, `canon status`, `canon evidence add`, the ship guard and the reopen guard read it as unapproved (`BRIEF_CHANGED_SINCE_APPROVAL`) and name the re-approval.
- The ship guard refuses on the brief's blockers.
- An `Approved:` line with no digest stays approved, with a `canon status` advisory to re-approve.
- A brief without section markers (every brief before 0.19) checks as a single `brief.legacy` advisory and gates as before.
- `canon policy approve` still exists; `canon brief approve` also records the policy approval.

### Fixed
- Coverage rules no longer fire on a new scope before it is approved.
- Flow steps no longer draw sentence-length advisories for their three clauses.

### Upgrading
1. Run `canon skills install` for the new skill and references.
2. Re-approve briefs whose `Approved:` line has no digest with `canon brief approve`, as `canon status` advises.
3. Make scripts that read `tasks.md` treat it as optional.

## 0.18.0 — 2026-10-13

One `evidence add` now attests every scenario a test report covers, one `review add` reviews a batch, a shipped subject can be reopened inside its approved scope, and a change can belong to several subjects.

### Breaking
- `Change.subject_id` is now `subject_ids`, a list (`Change` schema generation `2`). Generation-1 records read as a one-element list; new records are written at generation `2`.
- `canon query --kind change` returns only each change's current version; it used to return every version.

### Added
- `canon evidence add --project-id <root> --all-scenarios --report <fmt>:<path> …` writes one record per indexed scenario that owns a case in the report. Each record binds only its own cases and gets their verdict (`faithful` all passed, `divergent` one failed, `not-applicable` one skipped and none failed) unless `--verdict` claims one for all. Unindexed ids are named and skipped. It is all or nothing: a claimed `faithful` over a failed case, a fabrication marker, an unapproved brief or a dropped binding writes no record.
- `canon review add` takes repeated `--scenario-id` and `--scenario-ids-from <file|->` (one id per line, `#` comments) for a batch with one reviewer, pin and artifact set, all or nothing. Single-scenario output is unchanged.
- `canon evidence add --task --report` with no scenario id binds every case in the report; `faithful` requires all to pass.
- `--report swift-testing:<path>` reads the JSON-lines stream of `swift test --experimental-event-stream-output`: the display name, else the function name, carries the scenario id; issues and `testSkipped` decide the outcome. `ReportFormat` gains `swift-testing`.
- When `evidence add` supersedes evidence that a current pinned review saw, it prints one line per newly stale review with the reviewer, the scenario and the `canon review add … --pin <sha>` that re-pins it.
- `canon subject status <id> --to verifying --reason "<why>"` reopens a `shipped` subject. Inside the approved scope it needs no human (the newest adopted change's brief is approved, or there is no brief); otherwise it exits 1 and names the brief to approve, and `--override-reason` does not waive that. The new Subject record carries `reopen: { reason, change, actor }`, cleared by the next status write. The review guard does not run on reopen; shipping again runs every guard.
- `canon status` flags a shipped subject whose ship guard would now fail (`gate: red — <n> <class> (first: <subject>)`; `red` in `--json`, `statusVersion` stays `3`).
- A second `canon subject adopt <change> --subject <other>` adds the subject to the change instead of re-pointing it. An open blocker on a shared change gates every subject it belongs to.
- `canon finding list --change-id <id> [--open] [--json]` prints a change's committed findings at their latest version, ordered by round and seq; `--open` keeps findings whose current disposition is `open`. JSON: `{"change_id", "findings": [{round, seq, severity, disposition, reviewer, summary, resolution_sha, file_ref}]}`. Staged findings are counted and `canon gate promote` is named.
- `canon skills install` and `canon skills doctor` end with an `owned-paths:` list of globs canon writes (each selected provider's projection, then machine-written paths under `.canon/`), for formatter and linter ignore lists. `.canon/specs/`, `.canon/policy.yaml` and `.canon/changes/` are left out.
- `canon skills doctor` reports `formatter-rewritten` and names the ignore entry and the `canon skills install --providers=<p>` command that restores the file.

### Changed
- `--reason` is required for a reopen and refused on any other transition (exit 2).
- `canon evidence vault` also stores the files of current reviews, and first refreshes a stale scenario index as `gate check` does.
- `canon gate check` re-checks `canon review add --artifact` files only for current reviews; a superseded review's files are neither checked nor vaulted. A missing blob whose working-tree file still matches is an advisory, not a `stale-evidence` violation; a changed or missing file or an altered blob is still `stale-evidence`.
- The fabrication-marker refusal in `evidence add` is one line per finding like other gate-red refusals (exit 1, nothing written).
- `canon report` now reads the sqlite hot tier, so a fresh repo's task columns fill in and `Backend::Sqlite` kinds no longer appear under `## Kinds not read directly`. A `hot` db file that is not sqlite fails the report (exit 1) and names the file. A missing db file is treated as no rows and not created. A record present in both a hot row and the git ledger or `.canon/r2` counts once.
- `canon skills check` reports a formatter-only rewrite (CRLF, trailing whitespace, blank-line runs, final newline, markdown list markers, `_` emphasis, table padding; JSON compared by value) as `formatter-rewritten` instead of `stale`; it still exits 1. Whitespace between tokens, backslashes and fenced code are never normalized, so those changes stay `stale`.
- The skill says a review runs in a session that can run the checks it signs off (a reviewer that cannot run the tests says so and does not sign off), and that a report bound with `--report` is kept in `.canon/artifacts`, so its working-tree copy may be gitignored.

### Fixed
- Fabrication markers `"would pass"`, `"TBD"` and `"n/a"` match only as whole words, so `pack.json/atlas JSON` and similar paths or file names are no longer refused. A `.` or `-` joins a marker into a longer token only when a letter, digit, `_`, `/` or `\` follows (`n/a.json`, `x-tbd`); sentence punctuation does not.
- `gate check` no longer fails on a lost blob belonging to a superseded review.

### Upgrading
1. Run `canon skills install` to refresh the projected skill and to see the `owned-paths:` list for formatter and linter ignores.
2. Make scripts that read `canon query --kind change` expect one row per change.
3. Make scripts that match `stale` from `canon skills check` match `formatter-rewritten` too.

## 0.17.0 — 2026-10-12

canon keeps its spec corpus, its own changes and every approval under `.canon/`, reads external plans without writing into them, asks which harnesses get the skill, and `canon migrate` moves a 0.16 repo over in one command.

### Breaking
- The spec layout drops the `features/` level: a spec root now holds `kind=feature/area=<area>/<surface>.feature` directly. The default spec root of a new repo, and of an absent `specs:` key or an entry without `root:`, is `.canon/specs` (was repo-level `specs`).
- canon does not read both layouts. A spec root with any `.feature` file under `<root>/features/` is refused, and every refusal names `canon migrate`:
  - `canon format` reports one `layout-grammar` violation at path `features`.
  - `canon inventory sync` aborts the root with exit 1 and 0 writes.
  - `canon status`, `canon gate check`, `canon evidence add`, `canon review add` and `canon subject status` name the root on stderr and do not refresh its scenario index; `canon status` warns and lists only `canon migrate` as its next step.
  - `canon gate check` exits 1 with `uncovered-cell <root>`, and the ship guard of `canon subject status … shipped` refuses with the same violation.
  - `canon scenario new` and `canon feature new` exit 2 and write nothing.
- The `Approved:` line is read only from `.canon/changes/<change>/brief.md` (outside HTML comments and fenced code). An `Approved:` line in an external `proposal.md` is no longer read. A change from an external plan gets a pointer `brief.md` holding a `Source: <path>` line and, once the human answers, the `Approved:` line.
- `canon status --json` is `statusVersion` 3: `brief` is `{approved, enforced, change?, brief?, briefExists, source?, approval?}`; the `proposal` field is gone (read `brief.brief`, `brief.briefExists`, `brief.source`). Human-readable output prints `brief: approved in <brief> — <words>` or `brief: not approved (…)`.
- `canon change new <slug> --subject <id> --title "…"` writes `.canon/changes/<slug>/brief.md` and `tasks.md` instead of scaffolding `openspec/changes/<slug>/`, and its output names those paths. It no longer needs a plans source, so the refusal for a repo without an `openspec` source is gone.
- `canon init` no longer creates `openspec/changes/`; its `plans:` section is `sources: []` with commented `openspec` and `superpowers` examples. A `plans.sources` entry naming `canon` is refused.
- `canon init` no longer ends with `next: canon skills install`; its last line is the skill install result (or the no-harness hint).
- Skill providers are no longer picked from `.agents` alone or by a Claude+Codex fallback. A repo with no install lock and only an `.agents` directory no longer gets Codex by default; pass `--providers`.

### Added
- `canon migrate [--dry-run] [--repo <dir>]` moves a repo set up by canon 0.16 or earlier to the 0.17 layout in one idempotent command; `--dry-run` prints the plan only.
  - Moves `<root>/features/kind=feature/` to `<root>/kind=feature/` (`git mv` for tracked files); a root at repo-level `specs` moves to `.canon/specs`; other roots only drop `features/`.
  - Records `specs.roots[].former_roots: [specs]` for a moved root, so reviews written before 0.15 are still checked against the commit they pinned.
  - Moves changes that `canon change new` wrote under openspec to `.canon/changes/<slug>/` (`proposal.md` renamed `brief.md`) and re-imports them under the same change id, so task ids (`<slug>#<n>`) keep joining. Other changes stay as external plans; ambiguous ones are named in the output.
  - Writes a pointer `brief.md` (`Source: <proposal>` plus the `Approved:` line) for each external `proposal.md` that holds an approval; the proposal is not edited.
  - Writes `skills.providers` from the install lock when `canon.yaml` has no `skills:` key, adds missing canon `.gitignore` lines, names a scenario index still tracked in git, and rebuilds the index with a full `canon inventory sync`.
  - Edits `canon.yaml` textually, keeping comments and CRLF, and refuses an edit it cannot make safely.
  - Refuses with exit 2 and writes nothing for uncommitted changes under a path it moves, an existing destination, a change directory that could stop halfway, or an unsafe `canon.yaml` edit. An interrupted run is finished by the next one.
- `.canon/changes/` is a native plan source (dialect id `canon`, never listed in `canon.yaml`) read first by `canon ingest plans`, `canon gate task`, `canon evidence add --task`, `canon dispatch --task`, `canon status` and the brief check. Its records carry the actor `canon-plan-import-canon`; external openspec changes carry `canon-plan-import-openspec`.
- `canon.yaml` gains `skills: { providers: [claude, codex, omp, pi] }` (any subset, `[]` for none). `canon init --check-config` validates it; a malformed section or unknown name fails `canon skills install|check|doctor`.
- `canon init` asks on a terminal which harnesses get the canon skill, with detected ones (`.claude`, `.codex`, `.omp`, `.pi`) checked. `--providers a,b` answers without a prompt and `--providers none` records `[]`. With no terminal and no flag it uses the detected ones, or installs nothing and names `canon skills install --providers=<list>`. It records the answer in `canon.yaml` and installs the skill; if the default source is missing, the choice stays recorded and init exits 2 naming the install to run.
- `canon skills doctor` adds a `stray:` advisory for a canon projection (`.claude`, `.agents`, `.omp`, `.pi` `skills/canon`) whose provider is not selected, with the `rm -r` and install commands. `check` does not fail on it. `check` and `doctor` name where the providers came from.

### Changed
- `canon skills install|check|doctor` follow `skills.providers`; without the key they use the install lock's providers, then the detected ones. An explicit `--providers` on install rewrites the list in an existing `canon.yaml` (other bytes kept) and never creates one.
- `canon init` on an existing repo also adds missing canon `.gitignore` lines (hot db, audit log, scenario index, index lock, artifact-store lock), printing `canon init: added to .gitignore: …`. Its first line now reads `… left unchanged; the AGENTS.md block and the canon .gitignore lines are refreshed`. With `--no-agents-md` it adds only the lines. It still exits 2 when nothing is missing.
- `canon gate task` still flips the task's checkbox in an external plan's `tasks.md`; this is the only write into an external plan.
- With no `.canon/changes/` and no `plans.sources`, `canon gate task` and `canon dispatch --task` say the repo configures no plan sources and point at `canon change new` or `plans.sources`.
- `canon review add` on a scenario with an index written before 0.15 now pins `scenario digest` at `scenario_digest_scope: scenario`, as on a fresh index.

### Fixed
- A scenario index written before 0.15 (no `scenario_digest`) no longer makes legacy reviews undecidable: the readers' stale-index check now also compares `scenario_digest`, so the first reader rewrites those generations.

### Upgrading
1. Install canon 0.17.0.
2. Run `canon migrate --dry-run` and read the plan. Commit anything uncommitted under a path it moves first; it refuses otherwise.
3. Run `canon migrate`. A second run finds nothing to move.
4. Commit the result.
5. Run `canon skills check` (and `canon skills doctor` for `stray:` projections of unselected providers).
6. Update scripts: `canon status --json` readers use `brief.brief`, `brief.briefExists` and `brief.source` instead of `brief.proposal`; matchers of `canon change new` and `canon init` output use the new text.
7. An `Approved:` line in an external `proposal.md` is no longer read; `canon migrate` writes the pointer `brief.md` for each approved proposal it finds. Until an approval is recorded in `.canon/changes/<change>/brief.md`, `canon status` lists the human step for that subject.
8. Repos that configured `{ dialect: openspec, root: . }` keep it as an external source; new changes go to `.canon/changes/`.

## 0.16.0 — 2026-10-12

After one human approval the agent can drive on its own: one approval per scope, promote on write, a self-refreshing scenario index, starter policy levels, review artifacts, and legacy reviews checked against the commit they pinned.

### Breaking
- `canon evidence add` refuses evidence under an unapproved brief. A `--scenario-id` resolves to the scenario's `@subject` owner; a `--task <change>#<n>` resolves to every subject whose adopted changes include that change. For each such subject with an enforced, unapproved brief it prints `unreviewed-promotion <subject> — evidence for <scenario or task>: brief not approved — …` plus a `refused` line, exits 1 and stages nothing. Subjects with no brief to answer, `canon review add` and `canon finding add` are not refused. (The brief moved from `proposal.md` to `.canon/changes/<change>/brief.md` in 0.17.0.)
- `canon subject status <id> --to shipped` refuses with `unreviewed-promotion <subject>` when the newest adopted change has a `proposal.md` without an `Approved:` line (outside HTML comments and fenced code). `--override-reason` waives it and records it in the waiver. With no brief to answer it prints an advisory instead.
- `canon gate check` exits 1 with `uncovered-cell <root>` when a root's `.feature` files changed but its index refresh is refused (e.g. a format violation); `canon subject status <id> --to shipped` refuses with the same violation, and `canon status` lists `canon inventory sync` as its only step. Before, the gate graded the old index and could pass.
- `canon status --json` is `statusVersion: 2`: `next[].actor` (`agent` or `human`), `subjects[].brief` (`approved`, `enforced`, `change`, `proposal`, `approval`) and `policy.level` (`null` without a policy) are new.
- The `canon status` human step text changed to `human: stop and report the brief in <proposal.md>; build and attest nothing for <id> until the human answers (one answer approves brief and policy)`. While a brief is unapproved this is the subject's only step, and its reason notes that `canon evidence add` refuses the subject until the line is recorded.
- The default `strict` starter policy now begins with `# canon policy level: strict`, so its `policy_digest` differs from a 0.15 starter's.
- `canon skills check` exits 1 when the install lock's `canon_version` is not this canon's, or the lock predates 0.16 and records none.

### Added
- One approval per scope: only a new subject, or a change to an approved subject's scope, needs the human, and one answer approves the brief and the policy. The agent records an `Approved: <who> <when> — "<their words>"` line in the change's `proposal.md` and runs `canon policy approve --by <human> --note "<words>"`. The skill, the `AGENTS.md` block and `canon status` carry this rule; fixes and follow-ups stay in the approved change as tasks.
- `canon status` shows, per building, verifying, shipped and proposed subject with an adopted change that has a `proposal.md`, whether its brief is approved (`brief:` line). With the brief approved and the ship guard passing, the ship step says to ship it yourself unless the brief says the human ships.
- `canon evidence add --override-reason <one line>` attests despite a brief refusal. It prints `waived unreviewed-promotion …` and ``override recorded by `<actor>` for the N brief refusal(s) above``, and the evidence record gains an optional `brief_override: {reason, waived: [{class, subject}], actor}`. A blank or multi-line reason exits 2. `canon status` warns for each overridden record and `canon gate check` lists them under a `brief overrides:` advisory that does not fail the gate.
- `--promote` and `--no-promote` on `canon evidence add`, `canon finding add`, `canon finding close` and `canon divergence add`. `--promote`, or `write: { promote: auto }` in `canon.yaml`, promotes the record just staged in the same run and prints one line ending `— staged and promoted in this run; nothing to promote`. A refused promotion leaves the record staged and exits 1. `canon init` writes `promote: auto` for new repos; without a `write:` key the behavior stays `manual`. A malformed `write:` section exits 2 before anything is staged, and `canon init --check-config` reports `[FAIL] write`.
- `canon gate check`, `canon status`, `canon subject status`, `canon review add` and `canon evidence add` rebuild a stale scenario index from the `.feature` files before reading it, with one stderr line naming the refreshed files. `canon inventory sync` stays; concurrent refreshes are serialized by `.canon/scenario-index.lock`.
- A scenario whose id is gone from every `.feature` file of its root gets a `removed: true` generation on sync or reader refresh. Readers, report marts and artifact ingest treat it as absent; re-adding it supersedes the tombstone.
- `canon init --policy light|standard|strict`: `light` requires evidence, `standard` adds a failure case per feature, `strict` (default) adds independent review. Lower levels carry the omitted requirements as commented lines. An unknown level, or `--policy` with `--no-policy`, exits 2 and writes nothing. `canon status` names the level (`level:` line; `custom` if none matches) and, for `light` and `standard`, how to raise it.
- `canon review add --artifact <path>` (repeatable, with `--max-artifact-mib`) stores what the reviewer looked at under `.canon/artifacts/sha256/`, recorded as `artifacts: [{path, sha256}]` on the review. `canon gate check` re-hashes the stored blobs of every in-effect review; a missing or altered blob is `stale-evidence` naming the review. A file outside the repository exits 2 and writes no review. Artifact-store writes are serialized by `.canon/artifact-store.lock`.
- The skills install lock records `canon_version` and `source_commit`. `canon skills check` and `canon skills doctor` name the install command when the lock is outdated (`outdated:` line in `doctor`).
- `canon skills doctor` lists co-installed skills in `.claude/skills`, `.agents/skills`, `.omp/skills` and `.pi/skills` whose description claims build work as an advisory `competing-skill:` line; it never edits them and still exits 0.

### Changed
- Reviews written before 0.15 (no pins, only `pin`) are checked by `canon gate check`, `canon status` and `canon subject status` against the scenario at the pinned commit: current if its scenario digest equals today's and no declared evidence is newer; stale (`unreviewed-promotion`) if the spec changed or evidence is newer; undecidable (commit or scenario not in this clone) still counts and is never a violation. A scenario that moved between feature files is found at its old location.
- The legacy-review advisory counts current, stale and undecidable reviews, counts a stale one as superseded when its scenario has a qualifying review, names only scenarios still owed a review, and suggests `fetch-depth: 0`.
- The skill's loop has no `canon format` or `canon inventory sync` step; `canon inventory sync` is named only after a command printed `NOT refreshed`. `canon gate promote` is named only for `write.promote: manual` (skill, `AGENTS.md` block, `canon evidence --help`, `canon finding --help`, `canon finding add --help`). `canon status` next-step wording changed accordingly (no "then `canon inventory sync`").
- `canon evidence add --task` and `canon gate task` read `tasks.md` as it is, so a new task row needs no `canon ingest plans`.
- `canon init`'s `.gitignore` also ignores `.canon/audit/`, `.canon/ledger/kind=scenario/`, `.canon/scenario-index.lock` and `.canon/artifact-store.lock`.

### Upgrading
1. Re-run `canon skills install` (with your `--providers`); until then `canon skills check` exits 1 because the lock records no `canon_version`.
2. Re-run `canon init` in an existing repo to refresh the `AGENTS.md` block with the one-approval loop; it leaves `canon.yaml` and the policy untouched.
3. Existing policies have no level line; `canon status` shows the level they match, or `custom`. Optionally add `write: { promote: auto }` to `canon.yaml` to promote on write.
4. Add `.canon/audit/`, `.canon/scenario-index.lock` and `.canon/artifact-store.lock` to `.gitignore` (`canon init` does this only for new repos; new repos also ignore `.canon/ledger/kind=scenario/`, which an existing repo may keep tracked).
5. Set `fetch-depth: 0` on the CI checkout that runs `canon gate check`; otherwise legacy reviews stay undecidable. With full history, a legacy review whose scenario drifted since its pin turns the gate red until a new `canon review add` pins it.
6. Record each subject's brief approval as an `Approved:` line in its newest adopted change's `proposal.md`, or pass `--override-reason <text>` to `canon evidence add`; scripts that attest before the line exists now get exit 1. Subjects with no adopted change or no `proposal.md` are unaffected.
7. Update scripts that match the `canon status` human step text or parse `canon status --json` (`statusVersion: 2`).

## 0.15.0 — 2026-10-11

Reviews pin what they saw, re-attesting cannot silently drop bound files, external audits sit beside declared evidence, and the policy gets an approver.

### Breaking
- A new latest `canon evidence add` record for a scenario that omits files the previous latest record bound is refused (exit 2) and names the missing paths. Re-bind them, or pass `--drop-attachment <path>` (repeatable) with `--drop-reason <text>`; the drop is recorded on the record and listed by `canon status`. Naming a never-bound path is refused.
- The failure-class list now has ten classes: `false-success` is added. A latest `refuted` audit is `false-success`; the ship guard refuses it even with `--override-reason`, and a later `faithful` declared record does not clear it.
- Under `require_review`, only a `signed` review qualifies, and a pinned review qualifies only while its pins match; otherwise the scenario is `unreviewed-promotion` with a "stale review" detail. The guard line now reads "a signed review, current with the spec and evidence it pinned, by an actor, and from a session, other than its evidence's".

### Added
- `canon review add` pins the reviewed scenario's digest and the ids and digests of its latest evidence records. The scenario digest covers the scenario's own block and tags plus inherited Feature tags, Background, and Rule; editing a sibling scenario keeps the review current. Reviews fold to the latest per reviewer.
- `canon review add --outcome signed|indeterminate|unavailable` (default `signed`), plus `--reviewer-provider`, `--reviewer-model`, `--reviewer-harness`. A gap from an unsigned latest review says "no signed review" and names each reviewer's outcome.
- `canon evidence add --kind audit` records an external evaluator's re-check with `--evaluator-id`, `--evaluator-version`, `--evaluator-digest`, `--audit-result confirmed|refuted|indeterminate`, and counterexample `--report`s. It sits beside the declared record and never replaces it.
- `spec_coverage.require_audit: { rate, seed, scope }` samples scenarios deterministically (`scope` defaults to `[verifying, shipped]`). A sampled scenario whose latest audit is missing or indeterminate is `uncovered-cell`.
- `canon policy approve --by <who> [--note]` writes the tracked `.canon/policy.approval.json` (policy digest, approver, time, note). `gate check` and `canon status` print an advisory while the policy is unapproved or changed since approval; exit codes are unchanged. `canon init`'s starter policy is unapproved until a human runs the command.
- `canon status` counts stale and unsigned reviews and lists audits sampled, confirmed, owed, and refuted; `--json` adds `staleReviews` and `unsignedReviews`.
- Record schemas gain `pins`, `scenario_digest`, `outcome`, `reviewer_provider`, `reviewer_model`, `reviewer_harness`, `audit`, and `attachment_drop`, plus a `ReviewOutcome` enum.
- `canon gate task` warns when a task's evidence has no attachment and its ref is a canon command.

### Changed
- Reviews written before 0.15 carry no pins and keep qualifying; `gate check`, `status`, and the ship guard print one advisory with their count (`N review(s) predate review pinning (0.15) …`).
- `canon finding add` and `canon finding close` resolve short SHAs through git and store the full SHA; a short SHA that names no commit is refused.
- The access audit (`.canon/audit/access.jsonl`, written by `query`, `export`, `purge`) is written only beside the `canon.yaml` the command read, never into another repo or one without `canon.yaml`.
- The skill tells authors to record defects in their own work before review, and that task evidence must bind the artifact or report proving the task.

### Upgrading
1. Re-review (`canon review add` by the same reviewer) to pin a scenario; until then its spec and evidence can change without the review going stale.
2. Have a human run `canon policy approve --by <who>` and commit `.canon/policy.approval.json`.
3. Update scripts that parse the failure-class list (ten classes now) and any re-attest that binds fewer files (add `--drop-attachment <path> --drop-reason <text>`).
4. Re-run `canon skills install` to project the updated skill.

## 0.14.0 — 2026-10-11

The skill becomes a working loop for agents, `canon init` sets that loop up, bound evidence stays provable, units and reviews carry a session, and `canon status` says what to run next.

### Breaking
- Every write command's success line now ends with its write mode: `evidence add`, `finding add`, `finding close` end with ``— run `canon gate promote` to commit it``; `divergence add` names `canon divergence promote`; `review add`, `divergence resolve|defer`, `subject new|adopt|status`, `change new`, `scenario new`, `feature new` end with `— written directly; nothing to promote`.
- `canon finding add --introduced-by self` is refused (`introduced_by` is a commit SHA); an author records a defect in its own work with `--reviewer <me>`, and its open blocker still blocks.
- `canon evidence add --report` refuses a `faithful` verdict when any bound case failed.
- `require_review.distinct_actor` now also rejects a review whose session equals an evidence session on that scenario, when both are set.

### Added
- `canon status [--json]` lists subjects by status with scenario, evidence, and review counts, gaps, and policy warnings, and ends with a `next:` list of commands.
- `canon init` writes a starter `.canon/policy.yaml` (`spec_coverage` with `require_evidence: true`, `require_cases: [failure]`, `require_review: {}`), a canon block in `AGENTS.md` between `<!-- canon:begin -->` / `<!-- canon:end -->` (created or merged), and an `openspec` plans source with `openspec/changes/`. `--no-agents-md` and `--no-policy` opt out.
- `canon change new <slug> --subject <id> --title <t>` scaffolds `openspec/changes/<slug>/{proposal,tasks}.md`, imports the change, and adopts it into the subject. It refuses an existing slug or unknown subject.
- `canon evidence add --artifact/--report` copies each bound file into the git-tracked `.canon/artifacts/sha256/<hex>`. `canon evidence vault` stores the bytes for pre-0.14 records whose files still match. `--max-artifact-mib <N>` on `evidence add` and `evidence vault` raises the 25 MiB limit.
- `--session-id` on `evidence add`, `review add`, `finding add`, and `finding close` fills `actor.session_id`.
- `--report-case` is repeatable on `evidence add`; `--summary` is allowed on scenario-only evidence.
- `canon context` opens with a header pointing to `canon status` and a `warning:` line when no `spec_coverage` is in force; `--json` is unchanged.
- `canon gate check` prints `advisory: spec_coverage is off — N scenario(s) are not checked for evidence; see .canon/policy.yaml` when scenarios exist and no `spec_coverage` is set. Exit code is unchanged.

### Changed
- `canon gate check` re-checks every file bound to a latest evidence record: an intact stored blob is clean even if the working file is rewritten; a missing blob with a matching working file is clean with an advisory naming `canon evidence vault`; anything else is `stale-evidence` naming the path and both digests (an altered blob says its bytes were altered).
- A report case binds to a scenario only when the scenario id is its own: the first scenario-id-shaped token (`<area>.<surface>.<nn>`) of its name, or an equal Cucumber tag. `--report-case` still binds any case by name.
- A flipped task row's note aggregates every record bound to the task; `canon gate task` re-imports the flipped change so `canon query --kind task` agrees with `tasks.md`. A record keyed by both `--task` and `--scenario-id` counts for both in `gate check`, the shipped gate, and `status`.
- Re-running `canon init` on an existing repo leaves `canon.yaml` alone, refreshes only the `AGENTS.md` block, and exits 0 (it used to exit 2); a rerun with nothing to do still refuses.
- With subjects but no `.feature` scenarios, `canon status` suggests `canon feature new` and `canon scenario new … --subject <id>` instead of `canon inventory sync`.
- The canon skill opens with an 8-step working loop (brief, subject, scenarios with failure paths, units, implement, evidence, independent review in a different session, status transition) and names the human stop points.

### Fixed
- `canon gate task` is no longer slow in repos with large directories under the plans root; the plan import reads only the files the dialect needs.
- A plan import keeps the change's `subject_id` when the plan names none, so a `gate task` flip or ticked checkbox no longer unlinks a change from its subject.

### Upgrading
1. Repos initialized before 0.14 keep `plans.sources: []`; `canon change new` refuses and names the entry to add: `{ dialect: openspec, root: . }` under `plans.sources`.
2. Commit `.canon/artifacts/` with the ledger.
3. Run `canon evidence vault` once to store bytes for bound records written before 0.14, before their working files change.
4. Update scripts that parse write-command output for the new suffixes.
5. If a change adopted before this release lost its subject link, run `canon subject adopt <change> --subject <id>`.
6. Re-run `canon skills install` to project the new skill.

## 0.13.1 — 2026-10-10

Codex agents can now discover canon: the Codex skill is installed where Codex reads it.

### Fixed
- `canon skills install` projects Codex like other providers, to `.agents/skills/canon/SKILL.md` with its `reference/**` and `scripts/**` sidecars (0.13.0 and earlier wrote `.codex/skills/canon.md`, which Codex never reads). An existing `.agents` or `.codex` directory selects Codex when `--providers` is omitted.
- `canon skills install` migrates the legacy `.codex/skills/canon.md` and `.codex/skills/canon/**` projection, removing only files recorded in `.canon/skills/.install-lock.json` whose bytes still match; edited, replaced, symlinked, and user-authored files are kept, and `.codex/skills` is removed only if empty.
- `canon skills check` (exit 1) and `canon skills doctor` report any remaining legacy projection with the fix command.

### Upgrading
1. Re-run `canon skills install`; Codex does not see canon until you do.
2. If you use the developer-only `canon/skills-dev` materializer (now writes `.agents/skills/<name>/SKILL.md`), delete old `.codex/skills/<name>.md` files by hand.

## 0.13.0 — 2026-10-09

`canon context --json` carries a typed policy summary, and unknown `policy.yaml` keys fail the gate.

### Breaking
- In `canon context --json`, `spec_coverage`, `evidence_binding`, and `review.requireReview` are JSON objects instead of summary strings. `spec_coverage` and `evidence_binding` are keyed like `policy.yaml` (booleans as booleans, lists as arrays); `review.requireReview` is `{"scope": [...], "distinctActor": true, "blockOnFindings": true}` or `null`. A present but unusable section is `{"invalid": "<detail>"}` instead of an `INVALID — …` string. `capabilityVersion` is 5. The text outline is unchanged.
- `canon gate check` reports an unknown top-level `policy.yaml` key (for example the typo `spec_coverag`) as `uncovered-cell <key>`, naming the known keys: `adapter_capabilities`, `allowed_signers`, `approval`, `experimental`, `query`, `risk_approvals`, `risk_routing`, `risk_tiers`, `schema`, `spec_coverage`, `staleness`, `trust_required`, `trust_sample`. Exit code is 1.

### Upgrading
1. Update consumers that parsed the old policy strings (`require_evidence=true scope=verifying, shipped …`) to read the fields directly, and to check for an `invalid` key instead of an `INVALID` prefix.
2. Fix or remove any unknown top-level key in `.canon/policy.yaml`.

## 0.12.0 — 2026-10-09

Ledger commands fail loudly on an invalid `canon.yaml`, and an opt-in independent-review gate is added.

### Breaking
- Commands that read or write the ledger (`gate`, `inventory sync`, `evidence`, `review`, `finding`, `divergence`, `subject`, `report`, `dashboard`, and others) exit 2 naming `<repo>/canon.yaml` and the parse error, instead of silently falling back to the default ledger. A legacy `tiers: git:` layout now refuses. `canon dispatch begin`/`end` still exit 0 and print the parse error as a stderr warning; `canon retrieve` still falls back to defaults.

### Added
- Opt-in `spec_coverage.require_review` policy (`scope`, default `[verifying, shipped]`; `distinct_actor`, default `true`; `block_on_findings`, default `true`). `canon gate check` reports each in-scope scenario with no `canon review add` record by someone other than its evidence actor as `unreviewed-promotion`.
- Ninth failure class `open-blocker`: an open `blocker` finding on a change the subject adopted.
- `canon subject status` runs the same checks when moving into a scoped status, prints which it ran or skipped, and refuses on gaps. `--override-reason "<one line>"` (with `--actor-id`) waives only `unreviewed-promotion` and `open-blocker` violations present at that time, records them on the subject, and keeps them listed as advisories by `gate check` until the next status change; a gap that appears later is not covered.
- With `block_on_findings`, an unreadable finding record is a `malformed-evidence` violation naming its ledger file, in `gate check` and `subject status`; no override waives it.
- `canon context` lists finding severities, dispositions, Review fields, and the active `require_review` setting; `--json` adds a `review` object, the Subject kind's optional `status_override` field, and `capabilityVersion` 4.

### Changed
- A `malformed-evidence` violation names the ledger file for schema-invalid records too (for example `verdict: maybe`).
- Under `experimental.evidence_binding` with `strength: report`, the remediation hint names `--report`, never `--artifact`.

### Upgrading
1. Fix any `canon.yaml` that only worked through the old fallback; rewrite a legacy `tiers: git:` layout to named tiers: `tiers: { local: { backend: git, root: .canon/ledger } }`.

## 0.11.0 — 2026-10-09

Experimental, off-by-default binding of evidence to files your own runner or agent produced; canon still never runs a test.

### Added
- `canon evidence add --artifact <path>` binds any repository file by sha256.
- `canon evidence add --report junit:<path>` or `--report cucumber:<path>` also parses the report, records the case matched to the scenario and its outcome, and refuses a `faithful` verdict over a failed case.
- Policy `experimental.evidence_binding` with `mode: off | warn | require`, minimum `strength` (`artifact` or `report`), and optional `case`/`lane`/`scope` filters; `warn` only lists advisories after `canon gate check`. Nothing changes for repos that do not set it, and the shape may change while experimental.

## 0.10.0 — 2026-10-08

Golden-path-only specs become visible through scenario case tags.

### Added
- `@case:<value>` scenario tag (`happy`, `failure`, `edge` in the base vocabulary), set by `canon scenario new --case` and indexed onto `Scenario.case` by `canon inventory sync`.
- Opt-in `spec_coverage.require_cases: [failure]` reports every feature surface (`<area>.<surface>`) whose in-scope scenarios specify no failure path; `canon subject status <id> --to shipped` refuses on the same rule.
- `canon context` prints `exclude_lanes` and `require_cases`.

### Upgrading
1. Before setting `require_cases`, tag existing failure-path scenarios; untagged scenarios satisfy no case.

## 0.9.1 — 2026-10-07

Fixes the `@journeykit/canon` launcher's exit status.

### Fixed
- The launcher in 0.8.0–0.9.0 exited 0 regardless of the native binary's result, hiding refusals and a red `canon gate check` from scripts, CI, and hooks. It passes the binary's exit code through again; a spawn failure or signal death exits 1.

## 0.9.0 — 2026-10-07

The Gherkin `@subject:<id>` tag becomes the only subject-to-scenario link.

### Breaking
- `canon report`'s Subjects panel and the `verifying → shipped` gate count scenarios whose latest `canon inventory sync` generation carries the `@subject:<id>` tag. Shipping is refused when a subject owns no tagged scenario.

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

