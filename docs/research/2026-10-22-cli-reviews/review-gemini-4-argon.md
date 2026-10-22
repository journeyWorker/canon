**Verdict: Ship with changes.** The unified spec (`canon.cli.json`), compile-time `Report` typing, and `{ok, data, meta, error}` envelope solve canon’s worst drift and parsing hazards, but the proposed renames introduce a direct flag collision (`--commit`), violate the plan’s own verb bans (`artifacts move`), and leave operand rules and `--input` edge cases underspecified. Fix the blockers below before cutting 0.28 so the single-release cutover truly lands once.

---

### Blockers

#### B1. `--commit` has two meanings: boolean safety flag vs valued Git SHA option
- **Where:** `plan.md` D6 (`--commit --plan-digest <DIGEST>`) vs `plan.md` Renames table (`--sha`, `--artifact-sha` → `--commit`).
- **What:** The plan renames `--sha` and `--artifact-sha` to `--commit <SHA>` on `divergence add/defer/resolve`, `evidence add/audit`, and `evidence approval-payload`, while simultaneously introducing `--commit` as the boolean execution flag for destructive commands (`purge`, `artifacts move`, `migrate`, `ledger compact`, `tier age`). This directly violates D1 ("one meaning per name") and Test T3 ("one name → one value name, arity and meaning"). Worse, in `--input` JSON documents, `"commit": true` and `"commit": "0123456..."` have different types.
- **Why it matters for agents:** An agent learning `--commit <SHA>` on `evidence add` will either pass a SHA to `purge --commit` or fail T3's mechanical vocabulary check immediately.
- **Concrete fix:** Keep `--commit` strictly as the boolean safety flag (matching `cli-interface-design-checklist.md` §Option names). Rename `--sha` and `--artifact-sha` to `--commit-sha <SHA>` (joining `--reviewed-commit-sha`, `--resolution-commit-sha`, and `--introduced-in-sha`, or keep `--reviewed-commit` / `--resolution-commit` if valued options ending in `-commit` are distinct from the bare `--commit` flag; `--commit-sha` is unambiguous everywhere).

#### B2. `CANON_ROLE` breaks `canon schema` and `canon context` when invalid or set
- **Where:** `plan.md` D2, D3; `2026-10-22-cli-audit-output.md` §2; `2026-10-22-cli-coverage-and-original.md` A1.10, B2.
- **What:** Today, `CANON_ROLE=bogus` causes *every* command—including `canon schema` and `canon schema --errors`—to fail with exit 2 (`UNKNOWN_ROLE`). Furthermore, a valid `CANON_ROLE` silently filters `canon schema` from 70 commands down to the role's subset unless `--all` is passed. This violates D3 ("no environment variable or TTY changes the format") and `cli-interface-design-checklist.md` §Self-description ("`schema` reads no project files...").
- **Why it matters for agents:** If an agent or harness sets an invalid `CANON_ROLE`, the agent cannot even run `canon schema --errors` or `canon schema` to discover valid roles or recover. If `CANON_ROLE=implementer` is set in the environment, an agent probing `canon schema` thinks reviewer/orchestrator commands do not exist.
- **Concrete fix:** Make `canon schema` (and `canon completion`) completely ignore `CANON_ROLE` unless `--as <ROLE>` is explicitly passed on the command line. Never fail `canon schema` or `canon schema --errors` on environment variables. In `canon context`, always emit the full `commands` list or include `meta.context.role` so filtering is explicit.

#### B3. Clean cutover of `retrieve --json` and `gate task` silently breaks already-installed repo hooks
- **Where:** `plan.md` D8, Renames (`gate task` → `task done`); `2026-10-22-cli-audit-callers.md` §0.4, §1; `2026-10-22-cli-audit-surface.md` C13.
- **What:** Upgrading the `canon` binary on a machine immediately changes `retrieve --json` from a bare array to `{ok, data, meta}` and removes `canon gate task` and `canon regime-key`. However, consumer repos have copies of `canon-retrieve-pre-dispatch.sh` (installed by `canon skills install`) and `.claude/settings.json` / `.codex/hooks.json` (installed by `canon gate install-hooks` with default command `canon gate task`). As `audit-callers` §0.4 proved, `canon-retrieve-pre-dispatch.sh` runs `jq 'length'` (which returns `3` on `{ok, data, meta}`!), then fails on `map(.title)` under `2>/dev/null || true`, **silently dropping all guidance with exit 0**. And `.claude/settings.json` calling `canon gate task` will hard-fail on every tool use until `canon migrate` is manually run in that repo.
- **Why it matters for agents:** An agent starting a session in an existing repo after a binary upgrade will silently lose all retrieved strategy guidance before it ever sees a hint to run `canon migrate` or `canon skills install`.
- **Concrete fix:**
  1. In `canon-retrieve-pre-dispatch.sh`, update the jq expression to `(.data // .)` and update `regime-key` to `regime show`.
  2. Have `canon status` and `canon gate check` inspect `.canon/skills/.install-lock.json` and `.claude/settings.json` / `.codex/hooks.json`; if they reference `< 0.28` or `canon gate task`, emit a top-priority warning in `meta.warnings` and a first `meta.next` step: `canon migrate && canon skills install`.
  3. In `gate install-hooks`, change the default `--command` to `canon task done`.

#### B4. `--input` document-to-argv translation breaks on `--input -` conflicts, relative paths, and batch error reporting
- **Where:** `plan.md` D7 (`--input <FILE|->`), T16.
- **What:** D7 states that `--input` is validated by turning the JSON document into an argv and parsing it through clap, and supports batch arrays/NDJSON all-or-nothing. Four edge cases break this model:
  1. **Batch validation errors:** If item 7 of a 20-item NDJSON batch has an invalid flag or unknown scenario, single-argv clap parsing cannot report *which item* failed, and `data.items[]` (promised by D7 and currently used by `scenario new --from`, `canon-0.27-schema-all.json`) has no error carrier on exit 2 unless specified.
  2. **Boolean false and defaults:** In JSON, `{"promote": false}` or `{"allScenarios": false}` converted naively to argv must *omit* `--promote`, whereas a flag with a true default (if any exist later) cannot be unset unless mapped carefully. Worse, mutually exclusive flags (`--promote` vs `--no-promote`) in JSON as `{"promote": false}` vs `{"noPromote": true}` are awkward for agents.
  3. **Values starting with `-`:** If a JSON string value (e.g., `"summary": "-v"`, `"ref": "--check"`) is turned into `["--summary", "-v"]`, clap will fail with unexpected argument unless `--flag=value` or `Arg::allow_hyphen_values(true)` is used.
  4. **Path resolution:** When `--input /tmp/req.json` or `--repo sub` is used, are paths inside the JSON (`manifest`, `artifact`, `feature`, `signatureFile`) relative to cwd, `--repo`, or the input file?
- **Why it matters for agents:** Agents construct JSON payloads programmatically (`summary` or `note` often starts with `- ` for bullets; batch runs are the main reason to use NDJSON).
- **Concrete fix:** Specify in D7 and test in T16:
  1. Serialize option values to argv as `--<kebab-key>=<value>` (never two tokens) so leading hyphens in values never parse as flags.
  2. For booleans: `true` emits `--<kebab-key>`, `false` emits nothing (and for `--promote`/`--no-promote`, allow `"promote": false` to emit `--no-promote`). Reject `null` values with `INPUT_INVALID_TYPE` (or treat `null` as omitted—pick one and document it in the input JSON Schema).
  3. All relative paths inside `--input` resolve relative to the current working directory (or `--repo` where the flag's own contract says "repository-relative", like `--manifest`), never relative to the `--input` file's parent directory.
  4. For batch `--input`, validate all items before writing; if any item fails, exit 2 with `error.field` set to `"items[<i>].<field>"` (e.g. `"items[3].scenarioId"`) and `data.items` listing per-line `{index, ok, error?}`. Cap `--input` size (e.g., 10 MiB) to prevent OOM on accidental stream piping.

---

### Should-Fix

#### S1. Grammar & Renames: `artifacts move`, `subject status`, and `report` modes violate D1
- **Where:** `plan.md` D1, Renames table; `2026-10-22-cli-audit-surface.md` C3, C8, C10, C11, C16, O15.
- **What:** Walking the final 0.28 command list (71 leaves across 24 groups + top-level) reveals five remaining grammar inconsistencies:
  1. **`artifacts move` violates D1:** D1 explicitly states: *"`update`, `move`, `get`, `delete` are not used."* Yet the Renames table renames `artifacts migrate` → `artifacts move --tier <TIER>`.
  2. **`subject status <ID> --to <STATE>` violates D1 & Checklist:** Checklist §Command names: *"Actions are verbs... Read-only views may be nouns when the noun is the thing shown; keep them clearly read-only."* Splitting `subject status` into `subject show <ID>` (read) and `subject status <ID> --to <STATE>` (write) leaves a **noun** (`status`) as a **write-only** command! And `finding close` (which can reopen via `--disposition open`, C11) is a verb, whereas `subject status` is a noun.
  3. **`report` has three modes that change its effect and output:** `canon report` (writes `.canon/REPORT.md`), `canon report --check` (read-only byte-diff, exit 1 on drift), and `canon report --snapshot <DIR>` (writes 10 parquet files + manifest). D1 states: *"A mode that changes a command's options is its own command"*, and `audit-surface` §2.1 notes `report --check` even corrupted `meta.command`.
  4. **Unrenamed non-`-id` identifier options (O15):** `one_name.rs` enforces `-id` on identifier flags, and `gate task` takes `<TASK_ID>`, `plugin sync` takes `<PLUGIN_ID>`, `dispatch begin` takes `--run-id`. Yet `--task` (on `dispatch begin`, `evidence add`, `evidence approval-payload`), `--parent-run` (on `dispatch begin`), and `--plugin` (on `query`) remain bare without `-id`.
  5. **`finding close <FINDING_KEY>` format is undefined:** Today `finding close` takes `--change-id <CHANGE_ID> --round <ROUND> --seq <SEQ>` (three required flags, no operand). D1 changes this to `finding close <FINDING_KEY>`, which is a brand-new composite syntax not used anywhere else in `finding` (while `finding add` still takes `--change-id`, `--round`, `--seq`).
- **Why it matters for agents:** Agents guess commands by analogy. If `status` is a read-only view at top-level (`canon status`), making `canon subject status` a write-only mutation is a trap. If `finding add` outputs `{changeId, round, seq}` without a `findingKey`, an agent cannot call `finding close <FINDING_KEY>` without guessing the delimiter (`change#1.2`? `change/1/2`? `change:1:2`?).
- **Concrete fix:**
  1. Remove `move` from D1's banned list (since `artifacts move` is genuinely moving blobs across tiers), **or** name it `artifacts tier --tier <TIER>` / `artifacts archive --tier <TIER>`.
  2. Rename `subject status <ID> --to <STATE>` → `subject transition <SUBJECT_ID> --to <STATE>` (or `subject set-status`), keeping `status` 100% read-only across the CLI.
  3. Split `report --snapshot <DIR>` into `report snapshot <DIR>` (or keep `report` as a group: `report build`, `report check`, `report snapshot <DIR>`).
  4. Rename `--task` → `--task-id`, `--parent-run` → `--parent-run-id`, and `--plugin` → `--plugin-id`.
  5. If `finding close` takes `<FINDING_KEY>`, define the exact grammar (e.g. `<change_id>#<round>.<seq>`), **emit `findingKey` in `finding add` and `finding list` outputs**, or keep `--change-id --round --seq` and make `<CHANGE_ID>` the operand (`finding close <CHANGE_ID> --round <N> --seq <N>`).

#### S2. Operands vs `--<noun>-id`: 12 commands have an ambiguous "primary object" under D1's rule
- **Where:** `plan.md` D1 (Operands / Q8), Renames table; `canon-0.27-schema-all.json`.
- **What:** D1 states: *"a command's primary object is its operand, repeatable when the command acts on several of one kind (`review scenario <SCENARIO_ID>...`, `evidence audit <SCENARIO_ID>`, `brief approve <CHANGE_ID>`, `finding close <FINDING_KEY>`, `subject adopt <CHANGE_ID>`). Every other identifier is `--<noun>-id`."*
  Applying this rule across all 71 commands exposes 12 commands where the primary object is either inverted, ambiguous between two identifiers, or left as a `--<noun>-id` flag despite being the group's own noun:
  1. `subject adopt <CHANGE_ID> --subject-id <SUBJECT_ID>` and `subject unadopt <CHANGE_ID> --subject-id <SUBJECT_ID>`: The noun group is `subject`, and `subject new/show/status` take `<SUBJECT_ID>` as the operand. Having `subject adopt` take `<CHANGE_ID>` as the operand and `--subject-id` as a flag (`audit-surface` P2) means `canon subject <verb> <ARG>` changes the type of `<ARG>` depending on `<verb>`!
  2. `evidence add`: `evidence audit <SCENARIO_ID>` makes `<SCENARIO_ID>` an operand, but `evidence add` attests to either `--task <TASK>` or `--scenario-id <SCENARIO_ID>` or `--all-scenarios` (3 modes!). What is `evidence add`'s operand?
  3. `evidence approval-payload`: Joins `--task` or `--scenario-id` + `--artifact-sha`. Does it take `<SCENARIO_ID>` like `evidence audit`, or no operand?
  4. `divergence add`, `divergence defer`, `divergence resolve`, `divergence accept`: All four act on a scenario (and `accept` acts on `--scenario-id <SCENARIO_IDS>...`), yet in 0.27 all four take `--scenario-id` as a flag and no operand, whereas `review scenario <SCENARIO_ID>...` makes `<SCENARIO_ID>...` an operand.
  5. `review change`: `review scenario` takes `<SCENARIO_ID>...`; does `review change` take `<CHANGE_ID>` as its operand? (Not listed in D1's parenthetical).
  6. `brief check` and `brief show`: `brief approve [<CHANGE_ID>]` takes an optional `<CHANGE_ID>` operand (defaulting to the repo's only live change). Do `brief check [<CHANGE_ID>]` and `brief show [<CHANGE_ID>]` also take `<CHANGE_ID>` as an optional operand instead of `--change-id`?
  7. `finding list`: Takes `--change-id <CHANGE_ID>` (required). Is `<CHANGE_ID>` an operand or `--change-id`?
  8. `dispatch end`: Acts on one run (`--run-id <RUN_ID>`, required). Is `<RUN_ID>` the operand (`dispatch end <RUN_ID> --outcome succeeded`)?
  9. `prompt add` and `prompt show`: Act on a prompt bundle (`--name <NAME> --bundle-version <VERSION>`). Meanwhile `dispatch begin` uses `--prompt-bundle <NAME@VERSION>`.
- **Why it matters for agents:** If `review scenario` takes `<SCENARIO_ID>...` as an operand, an agent will naturally type `divergence accept <SCENARIO_ID>...` and `evidence add <SCENARIO_ID>`. If `subject show` takes `<SUBJECT_ID>`, an agent will type `subject adopt <SUBJECT_ID> --change-id <CHANGE_ID>`. Half-applying Q8 makes operands *less* guessable than 0.27's all-flags rule.
- **Concrete fix:** Adopt and enforce a deterministic **3-part Operand Rule** in D1:
  - **Rule 1 (Group noun wins):** A command in a `<noun>` group whose action targets a single instance of `<noun>` takes `<NOUN_ID>` as its operand:
    - `subject new|show|status|adopt|unadopt <SUBJECT_ID>` (so `subject adopt <SUBJECT_ID> --change-id <CHANGE_ID>`).
    - `change new <CHANGE_ID>`
    - `feature new <SURFACE>`
    - `scenario new [<SCENARIO_ID>]`
    - `context-pack show|verify <PACK_ID>` (and `context-pack add <MANIFEST>` or `--manifest`)
    - `prompt add|show <NAME@VERSION>` (unifying with `--prompt-bundle <NAME@VERSION>`)
    - `plugin sync <PLUGIN_ID>`
    - `task done <TASK_ID>`
    - `strategy promote|approval-payload|rollback <STRATEGY_ID>`
    - `finding close <FINDING_KEY>`
    - `dispatch end <RUN_ID> --outcome <OUTCOME>`
  - **Rule 2 (Target-noun subcommands):** When a subcommand's verb or name *is* the target noun (or acts on one target kind across the whole group), that target is the operand:
    - `brief show|check|approve [<CHANGE_ID>]` (optional operand, defaults to single live change)
    - `review scenario <SCENARIO_ID>...` and `review change [<CHANGE_ID>]`
    - `divergence add|defer|resolve <SCENARIO_ID>` and `divergence accept <SCENARIO_ID>...`
    - `evidence audit <SCENARIO_ID>`
  - **Rule 3 (Multi-target / filter commands use flags):** When a command can target different kinds (`evidence add` / `evidence approval-payload` which take `--task-id`, `--scenario-id`, or `--all-scenarios`) or filters a collection (`finding list --change-id`, `query --kind`, `export --kind`, `purge --kind`), **all** identifiers are `--<noun>-id` flags and there is no operand.

#### S3. Envelope & Exit Table gaps: `schemaVersion` format, `gate task` partial data, and `meta.context` portability
- **Where:** `plan.md` D3, D4; `2026-10-22-cli-audit-output.md` §1, §4, §7; `2026-10-22-cli-coverage-and-original.md` A1.9, A1.27.
- **What:**
  1. **`schemaVersion` format conflict:** `plan.md` D3 says `schemaVersion` is `"major.minor per report: a consumer accepts unknown fields within a major and rejects an unknown major, terraform's rule"`. However, `coverage-and-original.md` B3/B4 and `audit-output.md` §7 write `"schemaVersion": "0.28.brief-check"`. `"0.28.brief-check"` is `<release>.<report>`, *not* a `major.minor` semver integer pair (`"1.0"`), which `bigcli-research` §0.4 warned against because bumping canon to `0.29` looks like a minor bump even if the report didn't change, or breaks equality checks.
  2. **Lost partial reports on exit 1 (`gate task` / `task done`):** `audit-output` §0 & §1 found that when `gate task` is blocked (exit 1 `GATE_TASK_BLOCKED`), it emits `{error}` only and discards the report (`{document, flipped, taskId}`), unlike `gate check` and `brief check` which emit `{ok: false, data, error, meta}`.
  3. **`meta.context.repo` absolute paths break conformance determinism:** Adding `meta.context: {repo, role, actor}` (D3) puts the resolved repo root path into *every* JSON output. Conformance tests (`conformance/cases/*`, 30 JSON pins) run in temporary scratch directories; an absolute path in `meta.context.repo` will fail byte-exact conformance diffs and leak host paths.
  4. **Exit 1 vs Exit 2 misclassifications from 0.25 are not listed in D4:** `audit-output` §4 found that `BRIEF_NOT_FOUND`, `LEARN_STRATEGY_NOT_FOUND`, `LEARN_SIGNATURE_UNREADABLE`, `LEARN_INVALID_ARGUMENT`, `ADAPTER_REFUSED` (on unreadable file), and `SKILLS_FAILED` (on invalid `--providers`) all exit `1` instead of `2`. D4 only mentions flipping `brief check`.
- **Why it matters for agents:** Agents branch on exit `2` (fix my flags/inputs) vs `1` (domain gate/check failed). If `STRATEGY_NOT_FOUND` or `BRIEF_NOT_FOUND` exits 1, the agent thinks a check failed rather than a bad ID/missing file refusal.
- **Concrete fix:**
  1. Make `meta.schemaVersion` `"1.0"` (independent `major.minor` per report, starting at `"1.0"` in 0.28), since `meta.canonVersion` already carries `"0.28.0"` and `meta.command` already names the report.
  2. Emit `data` alongside `error` on exit 1 for all check/gate commands: `gate check`, `brief check`, `format`, `report --check`, `skills check`, `artifacts verify`, `adapter authorize`, `selftest`, and `task done` (`GATE_TASK_BLOCKED`).
  3. Make `meta.context` omit the host filesystem path (or report `repo` as the canonical repo slug / relative to cwd, or normalize in conformance), and ensure `error.received` is populated on invalid enum/value errors.
  4. Explicitly audit all 151 error codes in T5 so every `*_NOT_FOUND`, `*_INVALID_ARGUMENT`, `*_UNREADABLE`, and bad-input refusal maps to exit `2`.

#### S4. Safety set (D6): `effect: destructive` mismatches and missing target guard on `purge`
- **Where:** `plan.md` D6, Q1; `2026-10-22-cli-bigcli-research.md` §5.3; `2026-10-22-cli-audit-output.md` §3, §4.
- **What:**
  1. D6 puts 5 commands in the preview-by-default + `--commit --plan-digest <DIGEST>` set: `purge`, `artifacts move`, `migrate`, `ledger compact`, `tier age`. However, in 0.27 (`canon-0.27-schema-all.json`), only `purge` has `effect: "destructive"`; the other 4 have `effect: "write"`. If `effect` is not updated to `"destructive"` (or a distinct `"migration"`/`"destructive"` class), an agent reading `canon schema` cannot tell from `effect` which commands require `--commit --plan-digest` vs which `write` commands execute immediately!
  2. On `purge --kind <KIND>`, `possibleValues` in `canon-0.27-schema-all.json` lists all 14 record kinds (`change`, `task`, `scenario`, ..., `subject`, `finding`), but `audit-output` §3 found that `purge` actually only allows 4 kinds at runtime and fails with `RETENTION_PURGE_FAILED` on the other 10 (`purge --kind subject`).
  3. Does anything else belong in the `--commit` set? No—`gate promote`, `strategy promote/rollback`, and `subject status` are append-only ledger writes and should execute directly. However, `ledger compact` and `tier age` are routine maintenance commands that only move superseded/old records to archive tiers without losing data; requiring a two-step `--plan-digest` dance on `ledger compact` and `tier age` adds friction if nothing is destroyed, *unless* their `planDigest` can also be bypassed or they are genuinely treated as destructive. Keeping the 5-command Q1 set is fine, provided all 5 share `effect: "destructive"` (or a `requiresCommit: true` schema property) and identical plan report fields (`planDigest`, `effects[]`, `risk`).
- **Why it matters for agents:** Agents inspect `effect` and `possibleValues` in `canon schema` to build valid calls. Advertising 14 `--kind` values on `purge` when 10 are refused at runtime violates T3 (`possibleValues` must be the true closed set).
- **Concrete fix:**
  1. Mark all 5 commands in the Q1 set as `effect: "destructive"` (or add `requiresCommit: true` to `CommandMeta` and `canon.cli.json`) and test in T3/T11 that `effect == "destructive" <=> has --commit && has --plan-digest`.
  2. Restrict `purge --kind` `possibleValues` in clap to the exact purgeable `RecordKind` subset so clap rejects non-purgeable kinds at parse time with `expected` listing only the valid purge kinds.

#### S5. Spec & Tests T1–T16: 5 gaps that T1–T16 will not catch
- **Where:** `plan.md` D2, Tests T1–T16; `2026-10-22-cli-coverage-and-original.md` B2, B5, B7.
- **What:**
  1. **Undeclared `CANON_*` environment variables:** `coverage-and-original.md` B2/B7 noted that `CANON_ROLE`, `CANON_ACTOR`, `CANON_S3_*`, `CANON_SKILLS_SOURCE`, `CANON_GATE_ADVISORY`, `CANON_DASHBOARD_DIST`, etc., are read via scattered `std::env::var` calls, not clap `env =`. None of T1–T16 checks that `spec.global.env` matches the actual `std::env::var("CANON_...")` calls in the codebase.
  2. **Per-command `errorCodes` accuracy vs module superset:** `coverage-and-original.md` B1/B2 notes `schema_codes.rs` is a module-granularity superset (e.g., `context-pack show` lists `CONTEXT_OBJECT_CONFLICT` and `PROMPT_BUNDLE_INVALID`; `dispatch end` lists `DISPATCH_ROLE_REGIME_MISMATCH`). T5 only checks `Registry = spec; every listed code is emitted; every emitted code is listed`, not that a command's `errorCodes` can actually be emitted by *that* command.
  3. **Casing inconsistency in `Report` structs:** `audit-output` §1 and `coverage-and-original` B7 Risk 2 point out that ~49 `json!` payloads mix `camelCase` and `snake_case` in the same object (e.g., `purge` has `planDigest` next to `dry_run` and `source_scope`; `export` has `record_count` next to `content_digests`; `status` has `statusVersion` next to `policy.spec_coverage`). T10 types the structs as-is, cementing the mixed casing into `1.0` JSON Schemas forever if not normalized during the 0.28 cutover.
  4. **Special characters / injection / path traversal in IDs (A1.42, E5):** `audit-output` §6 E5 notes `TaskId` (`<change>#<n>`) puts `#` into ledger filenames, and `coverage-and-original` A1.42 notes there is no test for special characters, whitespace, `--`, or path traversal in IDs/arguments.
  5. **`--dry-run` parity with real execution (A1.32):** T12 tests refusal and idempotency, but no test verifies that `--dry-run` runs the exact same validation path and refuses with the same error code as the non-dry-run call.
- **Why it matters for agents:** If `Report` schemas freeze a random mix of `snake_case` and `camelCase` in 0.28 (while `--input` is strictly `camelCase` per D7!), agents will constantly guess wrong field names when reading `data` vs writing `--input`.
- **Concrete fix:**
  1. Add an env-var scan assertion to **T5** (or **T2**): every `env::var("CANON_...")` in `crates/` must be declared in `spec.global.env`.
  2. In **T10**, enforce `#[serde(rename_all = "camelCase")]` on all CLI `Report` structs (except raw embedded ledger records if those are intentionally verbatim `canon-model` schemas, which should be documented). Since 0.28 is already a clean cutover of JSON shapes (D8), normalize report keys to `camelCase` now rather than freezing `dry_run` vs `dryRun`.
  3. Add `--dry-run` validation parity and hostile-identifier (`../`, `\n`, `--foo`) checks to **T12**.

---

### Notes

#### N1. Harness session-ID and actor capture from environment (BigCLI §0.10, §7)
- **Where:** `2026-10-22-cli-bigcli-research.md` §0.10, §7 (`CLAUDE_CODE_SESSION_ID`, `CODEX_THREAD_ID`, `CURSOR_CONVERSATION_ID`, `AMP_CURRENT_THREAD_ID`, `PI_SESSION_ID`).
- **What:** `canon` relies on `--session-id` for `require_review.distinct_actor` (checking that the reviewer session differs from the implementer session), yet agents frequently omit `--session-id` or make one up because they don't know their harness session ID.
- **Why it matters for agents:** Recording unsigned harness session env vars automatically (as fallback or alongside `--session-id` when present, without changing output format) prevents accidental self-review or missing session provenance.
- **Concrete fix:** If `--session-id` is omitted on `evidence add/audit`, `review scenario/change`, `finding add/close`, or `divergence accept`, optionally read known harness session ID env vars (or record them in `actor.env_session_id`) and list those env vars in `canon.cli.json`.

#### N2. `trace_check.py` coupling to `removed.rs` wording and new verbs
- **Where:** `plan.md` D8, D9; `2026-10-22-cli-audit-callers.md` §1 (`evals/dogfood/trace_check.py`).
- **What:** `trace_check.py` uses a hard-coded regex `RENAMED` (`--x was renamed --y in N; use `) and hard-coded `READ_ONLY` / `READ_ACTIONS` sets (`{check,show,list,doctor,verify,diff,explain}`).
- **Why it matters for agents:** Dogfood eval grading (part of 0.28 acceptance criteria) will misclassify 0.28 commands (`regime show`, `divergence list`, `brief show`, `evidence audit`, `review scenario`) and removed-command hints if the hint phrasing changes.
- **Concrete fix:** Keep the exact `was renamed <new> in 0.28.0; use <new>` format in `removed.rs` for both commands and options, and derive `trace_check.py`'s read/write command classification from `docs/cli/canon.cli.json` (`effect == "read"`) instead of hard-coding verb sets.

#### N3. `format` in 0.28 still has a no-op `--check` and alias `fmt`
- **Where:** `plan.md` Why table ("format vs lint: Out of scope for 0.28... `format` keeps its name and behavior"); `2026-10-22-cli-audit-surface.md` C8, O7, P1.
- **What:** Even keeping `format`'s name and validation behavior for 0.28, `format` currently has a required positional `<ROOT>` that duplicates context (`--repo`), a no-op `--check` flag ("kept for compatibility"), and an alias `fmt` (violating one-name-per-concept).
- **Why it matters for agents:** `format` is the only command with a command alias (`fmt`) and a no-op `--check`, which will require carve-outs in T1/T3 if left untouched.
- **Concrete fix:** Even while deferring the `format` vs `lint` split, drop the `fmt` alias (move to `removed.rs`) and make `[<ROOT>]` optional (defaulting to the configured `specs.roots[]` in `canon.yaml`) so `canon format` works with zero arguments just like `canon gate check`.

---

### What the Plan Gets Right

1. **Compile-time completeness (`meta()` + `Report` trait + `subcommand_required(true)`):** Making missing metadata and untyped `json!` outputs fail `cargo build` (D2, inspired by `cf` and `cargo`) eliminates the exact mechanism that let `canon brief` and 49 ad-hoc payloads drift in 0.25.
2. **In-process debug schema validation (T7):** Validating every emitted JSON document against its `schemars` JSON Schema inside `json_out::envelope` during `cargo test` turns all ~293 existing integration test calls into contract checks for free.
3. **Mode splits (`evidence audit`, `review scenario`, `review change`, `subject show`):** Splitting commands whose `--kind` or target flag radically changed the valid option set (D1, Renames) eliminates the 31-option `evidence add` mega-command (`9.6 KB` of `--help`) and makes `schema` truthful without needing complex conditional `requiredWhen` logic.
4. **Preview-by-default with `--plan-digest` on destructive ops (D6):** Making the bare run of `purge` and `artifacts move` safe (exit 0 + plan + digest) and binding `--commit` to `--plan-digest <DIGEST>` prevents both accidental data loss and TOCTOU races.
5. **Doc and `meta.next` lint against the spec (T14):** Parsing every `canon ...` command line in skills, `AGENTS.md`, website docs, error `hint`s, and `meta.next` against the spec ensures canon never tells an agent to run a flag or command that was renamed.
6. **Dropping `--no-input` in favor of a tested non-TTY guarantee (D6):** Avoiding the `--input` vs `--no-input` naming collision while testing that non-TTY / `--json` invocations never prompt is cleaner and safer than adding another flag.