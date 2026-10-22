# Canon 0.28.0: one CLI grammar, generated from one spec

- **Status:** PLANNED. 0.26.0 and 0.27.0 (#109, #110) have shipped; starts on owner go.
- **Base:** `main` at 0.27.0.
- **Version:** 0.28.0 (minor). One release carries the whole grammar change set as a clean cutover: no deprecated aliases (owner decision 2026-10-22).
- **Evidence:**
  - `docs/research/2026-10-22-cli-audit-surface.md`: canon 0.25.0, rule by rule; ids C*, O*, P*, D*, X*.
  - `docs/research/2026-10-22-cli-audit-output.md`: every leaf × JSON/text/error shape and exit code.
  - `docs/research/2026-10-22-cli-audit-callers.md`: who parses what.
  - `docs/research/2026-10-22-cli-bigcli-research.md`: wrangler/cf, gh, kubectl, docker, cargo/rustfmt/clippy, Biome, oxc, terraform, flyctl, AWS CLI, gws; 86 sources.
  - `docs/research/2026-10-22-cli-coverage-and-original.md`: the original agent-CLI report (`docs/research/deep-research-report.md`) against this plan (45 items), and the coverage design.
  - Checklist: the cli-interface-design skill and its worked example (Lute `docs/design/cli-grammar.md`, `crates/lute-cli/tests/cli_grammar_048.rs`).

## Principle

An agent that has used one canon command should guess the next one right. One name per concept, one meaning per name, one JSON envelope, one exit table. The schema tells the truth about what a command does.

The command surface is described once, in a generated spec that the parser, `canon schema`, `canon context`, the docs and the completions are all checked against. A command that is not in the spec cannot exist, and a document that names a command the spec lacks fails CI.

## Why (measured on 0.25.0)

| Area | Finding | Source |
| --- | --- | --- |
| Gate safety | `--repo` discovers `canon.yaml` only for the literal `.`. `gate check` without `canon.yaml`, or with `--repo <subdir>`, prints `clean` and exits 0. `format /missing` exits 0. | X2, #109 |
| Truthful schema | These commands are declared `read` but write: `status` refreshes the scenario index into the ledger/archive; `query` and `export` append an audit log; `dispatch diff` creates `hot.db`; `report --check` writes seed files. The README says `format` is `write`, while the schema says `read`. | X1, #110; coverage A3 |
| Coverage | `canon brief` is a runnable group and is missing from `schema --all`, because `leaf_paths` only walks leaves. `canon context` lists no commands and no artifact families. Context says `evidence_record` while `--kind` says `evidence`. | coverage B2, B6 |
| JSON | 57 leaves print `{data, meta}`. 13 print a bare report, and `retrieve` prints a bare array. There is no `ok` and no `schemaVersion`, errors carry no command id, and there are 6 failure shapes. About 49 of 70 payloads are built with `serde_json::json!` and have no schema. | output §1–3; coverage B4 |
| Exit codes | `brief check` is reversed: 2 = blockers, 1 = error. 37 leaves share a boilerplate exit table. | X5 |
| format vs lint | `format` never rewrites: it validates corpus shape, has a no-op `--check`, and an alias `fmt`. Out of scope for 0.28 (owner decision 2026-10-22): `format` keeps its name and behavior, except that its no-op `--check` and the `fmt` alias are dropped and `[<DIR>]` defaults to the configured spec roots (D12). | C8; bigcli §1 |
| Option names | One name with several shapes: `--to`, `--change-id`, `--scenario-id`, `--providers`. One name with several meanings: `--repo`, `--kind`, `--status`, `--role`/`--as`, and `--version` on `prompt`. One concept with many names: actor ×3, approver ×3, sha ×5, signature ×4, manifest ×2. | O2–O16, O22 |
| Command names | Same action at two scopes: `selftest`/`gate selftest`, `gate promote`/`divergence promote`. One verb with two actions: `migrate`/`artifacts migrate`. `artifacts` has 3 meanings. `context` vs `context-pack`. Nouns that act: `gate task`, `evidence vault`, `subject status --to`, `regime-key`. | C1–C16, X3 |
| Safety | Only `purge` is `destructive`, and it uses `--plan` / `--confirm <digest>`. `artifacts migrate` deletes local blobs by default. 7 of 46 writing leaves have `--dry-run`. There is no `--no-input`. | X4, O6, O18–O20 |
| Self-description | 64 options have no help. Validated enums have empty `possibleValues`. Some defaults exist only in prose. Top-level help is 4.3 KB. `--repo` and `--json` are redeclared per command with 7 and 10 help strings. 289 of 379 value names are parser-derived. | D1–D8, X8, X9 |
| Original report | Not yet planned: an operand rule; a boolean-naming rule; "refused leaves the tree unchanged"; declared idempotency; `error.received`; truthful `retryable`; `meta.context`; `meta.next[].argv`. | coverage A3 |
| Callers | 30 conformance steps pin JSON bodies. About 293 test invocations parse `--json`. The shipped retrieve hook parses a bare array and fails silently if it is wrapped. `SKILL.src.md` points at `meta.next`, but `status` has a top-level `next`. | callers §1–7 |

Already right: kebab-case everywhere; `--json` on every leaf; `--as` declared once; paging flattened; the `-id` rule; schema derived from clap with drift tests; registered error codes with hints; a runnable `example` per leaf.

## Decisions

### D1 Grammar doc

`docs/design/cli-grammar.md` is one normative page. It holds the concept table, verbs, placeholders, envelope, exit table, safety rule, and owner decisions with their sources. Its tables are machine-readable: `docs/design/cli-vocabulary.tsv` maps option → value name → concept, and `docs/design/cli-commands-0.28.md` is the **final per-leaf table** (both reviews' first blocker): every leaf with its operand (kind, arity), required options, mutually exclusive and dependent options, effect, output form (report / artifact / stream, media type), and one runnable example. The skill, the website reference and the spec examples are generated from it; it is written before S0 starts and reviewed with this plan.

- **Verbs:**
  - `new` creates an authored unit (feature, scenario, subject, change).
  - `add` records an attributed record.
  - `list` and `show` read.
  - `check` gives a verdict over repo state.
  - `verify` re-hashes stored objects.
  - `validate` checks the shape of one input file.
  - `approve` records a human approval.
  - `accept` records a reviewer taking a known gap.
  - `move` moves a thing between places or states: `artifacts move --tier`, `subject move --to` (the review round's fix: a noun such as `status` stays read-only).
  - `update`, `get`, `delete` are not used (a read-one leaf is `show`: `skills show <NAME>`); the original report's CRUD set is rejected (coverage A2).
- **Placeholders:** `<DIR> <FILE> <PATH> <ID> <SHA> <N> <TIMESTAMP> <NAME> <TEXT> <URL> <KIND> <FIELDS>` plus the concept ids (`<CHANGE_ID>`, …). `--fields <FIELDS>` is one comma list; the table names its values per leaf.
- **Booleans** name a mode (`--all`, `--check`, `--no-promote`).
- **One name, one meaning (T3, round 2):** an option name keeps one value name, arity, meaning and value set on every leaf; only `required` and the default may differ per leaf, and a leaf may declare a subset of a shared set (`purge --kind`, `gate promote --kind`). So `--kind` is only a record kind (`change new --change-kind`), `--role` is only the role a record is attributed to (default `--as`; the filter on `retrieve` is `--for-role`, the key segment on `regime show` is `--role-segment`, and `dispatch begin --role` is the role the run acts as, so it keeps the name), `--outcome` is not used (`dispatch end --run-outcome`, `review scenario|change --review-outcome`), and no leaf has a bare `--source` (`--skills-source`, `--plan-source`).
- **Operands (Q8, decided 2026-10-22; rule from review round 1):** a command's primary object is its operand, decided by three rules applied in order. (1) In a noun group, a command that acts on one instance of that noun takes it: `subject show|move|adopt|unadopt <SUBJECT_ID>` (so `subject adopt <SUBJECT_ID> --change-id <CHANGE_ID>`), `change new <CHANGE_ID>`, `feature new <SURFACE>`, `task done <TASK_ID>`, `plugin sync <PLUGIN_ID>`, `strategy promote|approval-payload|rollback <STRATEGY_ID>`, `context-pack show|verify <PACK_ID>`, `dispatch end <RUN_ID>`, `prompt add|show <NAME@VERSION>`. (2) A command named for its target kind takes that target: `brief show|check|approve <CHANGE_ID>`, `review scenario <SCENARIO_ID>...`, `review change <CHANGE_ID>`, `divergence add|defer|resolve <SCENARIO_ID>`, `divergence accept <SCENARIO_ID>...`, `evidence audit <SCENARIO_ID>`, `finding close <CHANGE_ID> --round --seq`. (3) A command that can target several kinds, or filters a collection, takes no operand: `evidence add` (`--scenario-id`, `--task-id` or `--all-scenarios`), `finding add`, `finding list`, `query`, `export`, `purge`. Every other identifier is `--<noun>-id` (`--task-id`, `--plugin-id`, `--parent-run-id` included). At most one operand kind per command; context is never an operand. The per-leaf table records the result for every leaf.
- **`--project-id` is required only when the repo configures more than one spec root**, on every command that takes it (today `evidence add` requires it even with one root, `scenario new` does not).
- **A mode that changes a command's options is its own command**, even when most options are shared (owner decision 2026-10-22). Shared options are declared once and flattened into each.

### D2 One generated command spec

`docs/cli/canon.cli.json` is checked in, deterministic, and marked DO-NOT-EDIT. It contains:

- `specVersion` and `cli.version`;
- global options and env vars;
- the exit table;
- envelope JSON Schemas;
- all error codes with meaning, exit and retryability;
- every node: leaves, pure groups and runnable groups (`nodeKind`). Each node carries `about`, `effect`, `effectNote`, `idempotency`, `roles`, operands, options (help, value name, default, `possibleValues`), `requiredOneOf`, `exits`, `errorCodes`, `outputs` (`kind: report|artifact|stream`, `schemaRef`, `schemaVersion`, and for every write leaf a `batch` variant `Batch<Name>Report`, D7), `inputs` (e.g. NDJSON batch lines with `schemaRef`) and `examples`;
- `schemas`: a JSON Schema 2020-12 for every report and input document;
- `records`: kinds and families with links to `crates/canon-model/schemas`.

It plays the role OpenAPI plays for an HTTP API (coverage B3).

- **Step 0:** move the clap tree out of `main.rs` into `canon_cli::cli`, so tests and the generator can reach `Cli::command()`.
- **Completeness at compile time:** `fn meta(&Command) -> &'static CommandMeta` is an exhaustive `match` over the clap enums with no wildcard arm, so a new command without metadata does not compile (bigcli §3.3). Every group enum is `subcommand_required(true)`, so no group is runnable.
- **Typed payloads:** a `Report` trait (`serde::Serialize + schemars::JsonSchema` + a `const SCHEMA_VERSION`). `json_out::data` is generic over it, so an output without a schema is a compile error. The ~49 `json!`-built payloads become structs; `schemars` and `jsonschema` are already in `Cargo.lock`.
- **Drift:** `tests/cli_spec.rs` regenerates the spec in memory and byte-diffs it against the checked-in file. `CANON_BLESS_CLI_SPEC=1` rewrites it.
- **Generated from the spec, never hand-written:**
  - `canon schema` and `schema --errors`;
  - a `commands` index in `canon context`, always present, filtered by `--as`;
  - the website CLI reference (en/ko tables; prose stays hand-written);
  - the skill/AGENTS command blocks, regenerated between markers, plus a lint that every command line in skill text, website docs, hints and `meta.next` parses against the spec;
  - a "Removed in 0.28" table (old spelling → new) for the README Upgrading section and the usage errors;
  - `canon completion <bash|zsh|fish>`, which reads the embedded spec at runtime;
  - man pages via `clap_mangen` at packaging.

### D3 Envelope

- **Success:** `{ok: true, data, meta}`.
- **Failure:** `{ok: false, error, meta}`, plus `data` when the command has a partial report (e.g. `gate check` violations).
- **`meta`:** `command` (the schema id; flags never change it); `schemaVersion` = `"<major>.<minor>"` per report, starting at `"1.0"` in 0.28 (a consumer accepts unknown fields within a major and rejects an unknown major, terraform's rule, bigcli §4.3; the report is named by `meta.command`, the release by `canonVersion`, so neither goes into the version string); `canonVersion`; `context` (role, actor, and the repo root as a path relative to the current directory, so conformance stays byte-stable and no host path leaks); `next` (each with `command` text and `argv` array); paging; `warnings`; `coverage` (below).
- **`error`:** `code` first, then `message`, `field` (an argv name such as `--scenario-id`, or an RFC 6901 JSON pointer into `--input`: `/items/2/scenarioId` for element 2 of a batch, `/scenarioId` for an object), `expected` (always the valid set), `received` (redacted for secrets), `suggestions[]` (edit distance over the spec's commands, options and values; a `destructive` command is suggested only when the typo is within distance 1 of it, Orca `command-suggestion.ts`), `hint` (a working invocation built from the spec, never clap's usage blob; when a value has the shape of another kind, e.g. a scenario id where a change id belongs, the hint is the corrected command, Orca `worktree-selector-recovery.ts`), `retryable` (true only where a retry can succeed, declared per code in the registry).
- **Keys are camelCase everywhere** in reports, errors and `--input` (0.25 mixes `planDigest` with `dry_run`); ledger records embedded verbatim keep their own schema and say so in the spec.
- **Streams:** `--json` puts the success or failure document on stdout and nothing on stderr; human text and logs go to stderr; without `--json`, errors go to stderr (Orca verified, research §3). T7 asserts empty stderr in JSON mode.
- **Format:** compact JSON. JSON mode is decided from argv before parsing, so pre-parse errors are JSON too.
- **Not wrapped:** artifacts and streams, each declared as such per leaf in the per-leaf table (`export` bytes, `strategy approval-payload` and `evidence approval-payload` bytes, `skills show` markdown and `completion` scripts in text mode; `report snapshot` writes files into its directory and prints an ordinary report). Every one of them still has an enveloped `--json` form or an envelope on failure.
- **Unchanged by environment:** no environment variable or TTY changes the format (oxlint #27569, bigcli §0). `CANON_ROLE` never filters or fails `canon schema`, `schema --errors`, `context` or `skills show`: an invalid value there is a warning, and role filtering needs `--as` on the command line (review round 1: `CANON_ROLE=bogus` blocks `schema --errors` today, and `CANON_ROLE=implementer` hides 47 of 70 commands).
- **Explaining a code:** `canon schema --errors --json` lists every error code with meaning, exit, `retryable` and the fix; `canon schema --error <CODE>` shows one. This closes the checklist's `--explain` gap without a new verb.
- **Coverage:** a check reports what it examined (`meta.coverage`: spec roots, scenarios, subjects examined; roots skipped and why). A check that examined nothing reports `unverifiable` with exit 1, never `clean` (Orca `hostScope`; the class of #109).

### D4 Exit table

0 ok; 1 the command's finding, failed check or `unverifiable` result; 2 refused (usage, `CONFIG_MISSING`, an id or file that does not exist, unreadable input, invalid argument). Each leaf declares its own rows; each registered error code declares its exit, and T5 checks that every `*_NOT_FOUND`, `*_UNREADABLE` and `*_INVALID_ARGUMENT` maps to 2 (six map to 1 in 0.25). `brief check` flips to 1 = blockers. Check commands that fail with exit 1 keep their report in `data` (`task done` when blocked included).

### D5 Context

`--repo <DIR>` is optional and declared once (flatten).

- Absent: the nearest ancestor with `canon.yaml`.
- Present: that directory.
- A command needing canon state that finds none refuses `CONFIG_MISSING`.
- It replaces `skills * --target`, the cwd-relative `ingest sessions --canon-yaml`, and the `regime-key --repo` key segment (renamed `--repo-name`).

### D6 Safety

- **Preview by default, act with `--commit`** (decided 2026-10-22) for commands that delete or rewrite existing data and for migrations: `purge`, `artifacts move` (deletes local blobs), `migrate` (git mv, rewrites canon.yaml), `ledger compact`, `tier age`. A bare run prints the plan (`planDigest`, `effects[]`) and exits 0; `--commit --plan-digest <DIGEST>` acts and refuses if the plan changed since. Their `--dry-run` is removed (the bare run is the dry run). Appends and state transitions (`gate promote`, `strategy rollback`, subject transitions) run directly.
- **`purge`** keeps its digest binding under the shared form: `--plan` / `--confirm <DIGEST>` become the bare run / `--commit --plan-digest <DIGEST>`; destructive acts also need the target named.
- **`--dry-run`** with one help string on every other writing leaf where a preview helps: init, skills install, gate install-hooks, inventory sync, plugin sync, artifacts store, ingest *, export, subject adopt/unadopt.
- **Refusal leaves the tree unchanged.** This is tested over every writing leaf; a repeated write is a no-op where declared idempotent.
- **`--override-reason <TEXT>`** stays the only "proceed although refused" flag; there is no `--force`. It is available to every role (D12): the waived refusal is recorded on the record, echoed in `data.overrides[]` and listed by `status` for the human.
- **No `--no-input` flag.** canon prompts in one place (`init` asking which harnesses, only when stdin and stderr are terminals; `--providers` answers it). A flag named `--no-input` beside `--input` (D7) would be one name with two meanings, so the rule is stated instead: canon never prompts without a terminal, and every prompted value has a flag (tested).

### D7 JSON input

Agents assemble arguments as data, so every writing leaf also accepts its arguments as one JSON document (decided 2026-10-22; gws takes request bodies as JSON, bigcli §2). `--json` stays output-only.

- **`--input <JSON|FILE|->`** (decided 2026-10-22): the value is inline JSON when it starts with `{` or `[` after whitespace (gws passes `--params '<JSON>'` / `--json '<JSON>'` inline, bigcli §2), `-` reads stdin, anything else is a file path. Inline is what agents use most: no temp file.
- **Framing (round 2):** the whole source is parsed as one JSON value first. Only if that fails and the source is a file or stdin, its non-empty lines are parsed as NDJSON (lines starting with `#` skipped); inline text that is not one JSON value is `INPUT_INVALID`. An object is one invocation and returns the ordinary report; an array or NDJSON is a batch and returns the batch report, even with one element.
- **Document shape:** a JSON object whose keys are the command's option names in camelCase (`scenarioId`, `projectId`, `reportFormat`…) and whose `operands` key holds the operands as an array. Repeatable options are arrays; flags are booleans (`true` emits the flag, `false` emits its `--no-` form when it has one, else nothing); `null` is refused (`INPUT_INVALID_TYPE`) because canon has no clear-to-default semantics. A key canon does not know is refused (`INPUT_UNKNOWN_FIELD`, `field` names the key, `expected` lists the valid keys), so a typo never drops silently. Option names, not record field names: the document is a command invocation, not a record.
- **Lossless argv:** every value becomes one `--<kebab>=<value>` element (never two tokens, never shell-parsed), so a value starting with `-` (`"summary": "- item"`) cannot read as a flag. Relative paths in the document resolve against the current directory, exactly as on the command line.
- **Batch:** a JSON array, or NDJSON from a file or stdin, runs the command once per element in order. Every element is validated (parse, clap, refusals that need no write) before the first write; a validation failure writes nothing, exits 2 and reports `error.field` as an RFC 6901 pointer into the batch addressed as `{"items": [...]}`: `/items/<i>/<key>` (D3). A write failure after that stops at the failing element and exits 1. Re-running the same batch is safe: elements whose record already exists with identical content are reported `unchanged` (the record writers are already content-addressed). Limits: 8 MiB per document, 10 000 elements; over the limit is refused before reading further (`INPUT_TOO_LARGE`). This replaces `scenario new --from` and `review add --scenario-ids-from`, which are removed. The five destructive leaves take one object only (a plan digest binds one plan; `INPUT_BATCH_UNSUPPORTED`, exit 2; O16 in the table).
- **Batch output:** every write leaf with a report output (all but the stream leaf `dashboard`) has a batch variant `Batch<Name>Report` declared in the spec's `outputs` next to the ordinary report: `data.items[]` holds one entry per element in order, `index` (0-based), `status` (`written`, `unchanged`, `failed` or `skipped`) and `data` (that element's ordinary report) or `error`. It has its own `schemaVersion` (`"1.0"`); `meta.command` stays the leaf id. Exit 0 when every element is `written` or `unchanged`, 1 when a write failed (later elements `skipped`, earlier ones stay written), 2 when validation refused (nothing written).
- **stdin:** `--input -` claims stdin; a command that also reads stdin for another option (`--report junit:-`) refuses the combination (`INPUT_STDIN_CONFLICT`).
- **Precedence:** a flag given on the command line and the same key in `--input` with a different value is refused (`INPUT_CONFLICT`); equal values are fine. Global options (`--repo`, `--as`, `--json`) and the safety pair (`--commit`, `--plan-digest`) are flags only: a document cannot commit a destructive plan.
- **Schema:** each writing leaf's input document has a JSON Schema in the generated spec (`inputs[].schemaRef`), derived from the same clap definition as the flags, so the two cannot drift. `canon schema <command> --json` shows it and an example document.
- **Validation is the same code path** as flags: the document is turned into an argv and parsed by clap, so every rule (required, `requiredOneOf`, value sets) and every error code applies unchanged.
- Read-only leaves do not take `--input` (their arguments are filters, short enough as flags).

### D7b Commits default to HEAD

An agent should not copy a sha it can ask git for (decided 2026-10-22).

- Every option that names "the commit this record is about" becomes optional and defaults to `HEAD`: `review scenario|change`, `divergence add|defer|resolve`, `evidence add`, `evidence approval-payload`, `finding add` (the commit the round reviewed), and `finding close --disposition fixed` (the resolution commit). canon resolves it with `git rev-parse --verify <rev>^{commit}` and records the full sha.
- Given explicitly, the value is any git revision (`HEAD~1`, a branch, a tag, a short sha), resolved the same way; today only `finding` resolves short shas.
- **Uncommitted code is recorded, never refused** (owner decision 2026-10-23: reviewing a working tree before the commit is normal). Every record that takes `--rev` also stores `codeTree`: the git tree id of the code the command saw, computed the same way for the working tree and for a commit: the files (tracked and untracked, gitignored files excluded; for a commit, `git read-tree <rev>`) are read into a temporary index, then `.canon/`, every configured `specs.roots[]` and the `code.exclude` globs are removed from it, then `git write-tree`. Editing a `.feature` file or a brief therefore never changes it, and it is never `<rev>^{tree}`. It also stores `worktreeDirty: true|false`. When the default is used, `rev` is HEAD and `codeTree` is the working tree, so a record made before a commit names exactly the code it was about.
- **A review survives its commit:** the commit made afterwards has the same code tree (computed the same way, from the commit), so canon can tell that a review of an uncommitted working tree covers that commit, and that it does not once the code changed. Anything that compares a record to "current code" compares `codeTree`, not `rev`.
- **No default** for the commit that introduced a finding: it must be sourced, never guessed.
- **Names:** `--rev <REV>` for the commit a record is about (one name on every command above); `--resolution-rev <REV>` and `--introduced-rev <REV>` on `finding`. `--commit` is only the destructive-command switch (D6), which resolves the collision both reviews raised.
- Outside a git work tree, or in a repo with no commits, a command that needs a commit and got none refuses (`NO_COMMIT`) with a hint. A working tree with no commits yet still gets a `codeTree`.

### D8 Clean cutover

- Every rename, split and shape change lands in 0.28 with no deprecated alias (owner decision 2026-10-22: agents work from the skill, and the skill, AGENTS block, docs and hooks are regenerated in the same release).
- An old command or option spelling fails as a usage error (exit 2) whose `error.hint` names the replacement and `error.expected` lists the valid spellings (`removed.rs`, extended with a command and option table generated from the spec's "Removed in 0.28" table).
- Every changed JSON report bumps its `schemaVersion` major; the README Upgrading section lists old → new command lines and report keys.
- `canon migrate` rewrites canon's own hook commands and scripts in a consumer repo (`gate install-hooks` output, the retrieve hook), and `canon skills install` refreshes the skill; `skills check` reports a skill older than the binary.
- The `deprecation.rs` machinery (`meta.deprecated`, `deprecatedAliases` in schema, stderr warnings) is removed with the 0.24 aliases it served, if none remain.

### D9 Callers move in the same release

Skill text, the AGENTS block, website en/ko, dashboard, `trace_check.py` (its read/write split comes from the spec's `effect`, not a verb list), conformance (reviewed re-bless; every JSON body validated against its schema), hook scripts and examples.

- **Installed callers recover in two runnable steps.** The retrieve hook reads `(.data // .)`, so it works before and after. `status`, `gate check` and `context` detect a 0.27-era skill lock, hook command or `canon gate task` hook entry and put `canon migrate --json` (the preview) first in `meta.next` with a warning; the preview's own `meta.next[].argv` is `canon migrate --commit --plan-digest <the actual digest> --json`, with the digest filled in. No generated command contains an ellipsis or an unfilled placeholder (T14). `canon migrate` backs up every file it rewrites under `.canon/migrate-backup/<timestamp>/`, is idempotent, and is tested on a fixture repo holding 0.27 hooks, skill and scripts.

### D10 From the Orca CLI (research `docs/research/2026-10-23-orca-cli-research.md`)

- **The guide comes from the binary** (Q9): `canon skills show <NAME> [--full]` prints the version-matched guide embedded at build time (text: markdown artifact; `--json`: enveloped). The installed `SKILL.md` becomes a short stub: what canon is, the loop in five lines, and "run `canon skills show canon` for the full guide; never guess a flag, ask `canon schema <command>`". The skill can no longer drift from the binary; `skills check` checks the stub's version; `canon migrate` replaces an old full skill with the stub.
- **Declared idempotency:** every writing leaf declares `idempotency` in the spec (`replay-safe`, `content-keyed`, `appends`); a duplicate `add` with identical content and a `close` of an already closed finding are no-ops reported `unchanged`. T12 runs every writing leaf twice.
- **Conflicts and requirements in the spec:** `conflictsWith` and `requires` per option, from clap; T3 checks each is enforced at parse time.
- **Human-only acts:** `brief approve`, `policy approve` and `divergence accept` refuse the agent roles with `retryable: false` and a hint addressed to the human that names the leaf and the options only they can supply (`--approver`, `--note`); no flag grants them. `--override-reason` is not human-only (D12); a refusal that was waived with it is echoed in `data.overrides[]`.
- **`--input -` refuses a terminal** (`INPUT_STDIN_TTY`) instead of waiting; each input declares `maxBytes` in the spec.
- **The generated guide ends each workflow with its done criterion** (which `gate check` / `subject show` result means finished) and where to read the result; every command line in it is parsed by T14.
- Not adopted: `--change-id current`-style context selectors (conflicts with "context is never an operand"; `meta.next[].argv` carries the explicit id), typed selector prefixes, `--yes`/`--force`, retry tokens, aliases, a 0/1 exit table, and reading the harness session id from the environment (`CLAUDE_CODE_SESSION_ID` and the like) when `--session-id` is omitted: a session id that canon fills in from an environment variable is not proof of a distinct session, which is #14's subject; the reviewers' round-1 N1 stays open by decision.

### D11 Re-verification follows the code (#116, #117)

Found in a consumer repo: three approved chores (one docs-only, one tests-only) made the ship guard ask for new evidence on all 104 evidenced scenarios of the subject; the agent recorded them one by one, and the first report-bound evidence staled 104 reviews.

- **A chore approval asks only where the code changed:** a scenario whose latest evidence was recorded on the current `codeTree` (D7b) needs no new evidence after the approval. A docs-only chore then needs its change-level review alone. What counts as code is `code.exclude` in `canon.yaml` (globs, default `docs/**`, `*.md`), applied to `codeTree` everywhere. Per-scenario code dependencies stay out of scope (#17).
- **When code changed, one command:** `status`, `meta.next` and the refusal hint lead with the batch over every report (`canon evidence add --all-scenarios --report junit:a.xml --report junit:b.xml --role implementer`, with the real report paths, and `--project-id <id>` when the repo has several spec roots), never a per-scenario loop. With a `--report`, `--class` defaults to `test-run` and `--ref` to the report paths (D12), so this command is runnable as written. T14 parses it.
- **A bound report decides the verdict** in single-scenario mode too, as the batch already does: any failed case `divergent`, a skipped case and none failed `not-applicable`, else `faithful`; an explicit `--verdict faithful` over a failed or skipped case stays refused.

### D12 Per-leaf table decisions

`docs/design/cli-commands-0.28.md` "Open for the owner": O1, O2, O7–O12, O15 and O16 are taken as recommended there. Decided in round 2 (the table lists them under "Decided in round 2"):

- **One name, one meaning** (O4, O5): `change new --change-kind`; `dispatch end --run-outcome`; `review scenario|change --review-outcome`; `--role` is only attribution (default `--as`), `retrieve --for-role`, `dispatch begin --role` stays because a run is attributed to the role it acts as; `skills install|check|doctor --skills-source`, `ingest plans --plan-source`; `--fields <FIELDS>`.
- **`--override-reason` stays available to agents** (O13): an implementer attesting while a brief waits on the human is the documented use; every waived refusal is recorded, echoed in `data.overrides[]`, and listed by `status`. The approvals themselves (`brief approve`, `policy approve`, `divergence accept`) refuse the agent roles (D10).
- **`prompt add|show <NAME@VERSION>`** (O3; the `--bundle-version` rename is deleted); **`format [<DIR>]`** defaults to the configured spec roots, `--check` dropped (O6); **`strategy approval-payload --approval-at`** is required, the leaf is a replay-safe read (O14).
- **`skills show <NAME>`**, because D1 bans `get`.
- **`evidence add`:** with at least one `--report`, `--class` defaults to `test-run` and `--ref` to the report paths; `--all-scenarios` needs `--project-id` only with several spec roots.
- **`init --check`** conflicts only with init's mutating options.

## Renames (decided 2026-10-22: all of the table)

| Kind | Now | 0.28 |
| --- | --- | --- |
| command | `evidence add --kind audit …` | `evidence audit` (own options: `--evaluator-id/-version/-digest`, `--audit-result`; no `--verdict`/`--task`/`--surface-ref`/`--approval-*`) |
| command | `review add --scenario-id …` / `review add --change-id …` | `review scenario` (pins scenario digest + evidence checks) and `review change` (pins the chore brief) |
| command | `selftest`, `gate selftest` | `selftest [--suite <NAME>]` |
| command | `gate promote`, `divergence promote` | `gate promote [--kind …]` only |
| command | `learn promote/approve/rollback` | `strategy promote`, `strategy approval-payload`, `strategy rollback` |
| command | `artifacts migrate --to <BACKEND>` | `artifacts move --tier <TIER>` (D6 preview + `--commit`) |
| command | `evidence vault` | `artifacts store` |
| command | `ingest artifacts` | `ingest records` |
| command | `context-pack new` | `context-pack add` |
| command | `demo add` | `demo review` |
| command | bare `brief` | `brief show` |
| command | `subject status <ID> [--to <STATE>]` | `subject show <SUBJECT_ID>` (read) and `subject move <SUBJECT_ID> --to <STATE>` (write) |
| command | `report`, `report --check`, `report --snapshot <DIR>` | `report build`, `report check`, `report snapshot <DIR>` |
| command | `canon schema` filtered by `CANON_ROLE` | unfiltered; `--as` filters |
| command | — | `skills show <NAME> [--full]` (D10; D1 bans `get`) |
| command | `divergence status` | `divergence list` |
| command | `gate task <TASK_ID>` | `task done <TASK_ID>` |
| command | `regime-key` | `regime show` |
| option | `--agent-id` / `--reviewer` (id) | `--actor-id` (`--reviewer` dropped where `--actor-id --role reviewer` says it) |
| option | `--by`, `--principal`, `--approval-by` | `--approver` |
| option | `--pin`, `--sha`, `--artifact-sha`, `--reviewed-sha` / `--resolution-sha` / `--introduced-by` | `--rev` (default HEAD) / `--resolution-rev` (default HEAD when fixed) / `--introduced-rev` (no default); see D7b |
| option | `--approved-at`, `--signature`, `--approval-signature-file`, `--context-manifest` | `--approval-at`, `--signature-file`, `--manifest` |
| option | `export --out` | `-o/--output` |
| option | `retrieve -k/--k` | `--limit` |
| option | `evidence add --kind` | `--class` |
| option | `dispatch end --status`, `divergence add --status` | `--run-outcome`, `--state` |
| option | `review add --outcome` | `--review-outcome` |
| option | `change new --kind` | `--change-kind` |
| option | `--role` (attribution) | stays, defaulting to `--as`, and means only attribution; `retrieve --role` → `--for-role`; `regime-key --role` → `--role-segment`; `dispatch begin --role` stays (the role the run acts as) |
| option | `--fields` (repeatable + comma list) | one form: `--fields <FIELDS>` (one comma list) |
| option | `--task`, `--plugin`, `--parent-run` | `--task-id`, `--plugin-id`, `--parent-run-id` |
| option | `ingest plans --source` | `--plan-source` |
| option | `skills install\|check\|doctor --source` | `--skills-source` |
| option | `init --check-config` | `init --check` |
| option | `format --check` | removed (D12) |
| kind | `evidence_record` in `canon context` | `evidence` (Q6) |

Open, recommended against: `learn`/`dispatch` as noun groups (`strategy` above covers `learn`; `dispatch` stays the toolchain verb for a run), `context` → `authoring`, `query/export/purge` under `records`.

## Tests (all in 0.28)

| Test | Proves |
| --- | --- |
| T1 parser walk | Every clap node (leaf, group) is in the spec and the reverse; no runnable group; `debug_assert()`. |
| T2 metadata | Exhaustive `meta()` compiles; no orphan META rows. |
| T3 options | Help, value name from the placeholder list, real default, `possibleValues` for closed sets; one name → one value name, arity, meaning and value set (per-command `required` and `default` may differ; declared subsets such as `purge --kind` and `gate promote --kind` are the only value-set exceptions); vocabulary membership both ways; verb denylist for valued options; short aliases only `-h -V -o -q`; no standard name redefined. |
| T4 spec drift | The regenerated spec equals the checked-in one. |
| T5 errors | Registry = spec; every listed code is emitted; every emitted code is listed; every `env::var("CANON_...")` in `crates/` is in the spec's env list or an internal allowlist; `schema --error <CODE>` works for every code. |
| T6 outputs | Every leaf declares outputs (or `[]` with a reason). |
| T7 output validation | In debug builds every emitted JSON document is validated against its schema, so the whole suite's ~293 `--json` calls become checks; `json_contract.rs` also validates each leaf. |
| T8 conformance | Every JSON `expected/N.out` validates against its command's schema. |
| T9 witness | Every declared output was emitted at least once by the suite; a shrinking allowlist for the stateful leaves. |
| T10 typing | No `json!` printing outside `json_out`; no untyped `Value` fields in any report schema. |
| T11 effect honesty | Every `read` leaf run in a scratch repo leaves the tree unchanged unless `effectNote` names the write. |
| T12 refusal | Every refused write leaves the tree unchanged; declared-idempotent writes are no-ops the second time; `--dry-run` runs full validation and refuses with the same code as the real run. |
| T13 context | `kinds` = `RecordKind::ALL`; `families` = `FamilyKind::ALL`; every `--kind` value set ⊆ `kinds`; every spec command in `context.commands` per role. |
| T14 docs | Every command line in skill text, website docs, the AGENTS block, hints and `meta.next` parses against the spec, and none contains `…`, `...` or an unfilled `<PLACEHOLDER>`; this includes D11's generated batch command and the two-step migrate recovery. |
| T15 mutation | Seeded violations (drop a META row, an option's help, add a `json!` print, a runnable group, a duplicate name) each fail the right test. |
| T16 input | For every writing leaf, the spec's example `--input` document (inline, file and stdin) and the equivalent flags produce byte-identical records; framing: an object is one invocation and returns the ordinary report, an array or NDJSON (comments skipped) returns the batch report even with one element, and text that is neither one JSON value nor NDJSON is `INPUT_INVALID`; an unknown key, `null`, a conflicting flag, a stdin conflict and an oversize document are refused; a batch with one invalid element writes nothing, exits 2 and sets `error.field` to `/items/<i>/<key>`; a batch whose third write fails reports written/failed/skipped, exits 1, and a re-run reports the first two `unchanged`; every batch response validates against `Batch<Name>Report`; a destructive leaf refuses a batch; a value starting with `-` survives. |
| T17 commits | Every `--rev` default resolves HEAD; an explicit short sha, branch and `HEAD~1` resolve to full shas; `codeTree` is identical for a dirty working tree and for the commit made from it, differs after any code change, and ignores `.canon/`, spec roots, `code.exclude` globs and gitignored files (editing a `.feature` file or a brief does not change it; the same function computes it for a working tree and for a commit); a dirty tree records `worktreeDirty: true` and refuses nothing; no git refuses `NO_COMMIT`. |
| T18 re-verification | A docs-only chore (code tree unchanged) needs no new evidence, only its change review; a chore after a code change asks for every evidenced scenario and names the generated batch command exactly (`canon evidence add --all-scenarios --report junit:a.xml --report junit:b.xml --role implementer` with the real paths), which T14 parses and which runs as written (`--class` and `--ref` defaulted from the reports); a single-scenario report with a skipped case gives `not-applicable` without `--verdict` and is refused with `--verdict faithful`. |

## Slices

| Slice | Scope | Depends on |
| --- | --- | --- |
| S0 | Extract the clap tree into `canon_cli::cli`; exhaustive `meta()`; groups `subcommand_required`; `brief show` | — |
| S1 | Spec generator, drift test, `canon completion`, man pages | S0 |
| S2 | `Report` trait, envelope D3, error D3; type the ~20 typed reports | S0 |
| S3a–e | Type the ~49 `json!` payloads by module (evidence/review/finding; gate/divergence/subject; dispatch/ingest/learn; retention/export/purge/query; init/migrate/skills/context/misc) | S2 |
| S4 | `--repo` declared once (D5; discovery and CONFIG_MISSING shipped in 0.27), `effectNote` in the spec (truthful reads shipped in 0.27), safety D6, `--dry-run`, exit table D4 | S0 |
| S5 | Mode splits: `evidence audit`, `review scenario`, `review change`; primary-object operands (Q8) | S0 |
| S6 | Renames table (D8), `removed.rs` command/option table with hints; preview + `--commit` set (D6); remove `deprecation.rs` | S0, S5 |
| S6b | `--input` inline/file/stdin JSON and NDJSON for every writing leaf (D7): document → lossless argv, input schemas in the spec, batch report with written/failed/skipped/unchanged; remove `--from`/`--scenario-ids-from`. `--rev` defaults to HEAD with the uncommitted-code rule (D7b) | S1, S5 |
| S7 | `canon context` commands/families; kind name `evidence` (Q6); capability version 6 | S1 |
| S8 | Derived docs: website reference, skill/AGENTS blocks between markers, doc lint T14, "Removed in 0.28" table; skill text and README | S1, S6 |
| S9 | Tests T1–T18, witness fixtures, conformance re-bless with review | all |
| S10 | Re-verification by code tree and derived single-scenario verdicts (D11; #116, #117) | S4 (D7b `codeTree`) |
| R | README 0.28.0 with the Upgrading list, version, dogfood eval, release | all |

Effort from coverage B7: ~24–27 engineer-days serial, ~8–9 calendar days fanned out. S3 is the bulk.

**Acceptance:**
- the workspace passes;
- canon's own gate is clean with new scenarios evidenced and independently reviewed;
- T1–T18 green with empty allowlists except the documented witness list;
- a dogfood eval (omp, roles, `language: ko`) on a 0.26 baseline shows 0 wrong-shape calls and no more `--help`/`schema` probes per scenario than eval 12, and a canon 0.25-era hook and script set migrated by `canon migrate`.

## Owner decisions

- **Q1 Preview + `--commit`:** decided 2026-10-22: destructive commands and migrations (`purge`, `artifacts move`, `migrate`, `ledger compact`, `tier age`). See D6. All five are `effect: destructive` in the spec, and `purge --kind` lists only the kinds purge accepts.
- **Q2 Renames:** decided 2026-10-22: the whole table, plus the round-1 review fixes (`subject move`, `report build|check|snapshot`, `-id` on `--task`/`--plugin`/`--parent-run`).
- **Q3 Mode splits:** decided 2026-10-22: split even when options overlap (`evidence audit`; `review scenario` / `review change`; `report`).
- **Q4 Envelope:** decided 2026-10-22: `{ok, data, meta, error}`; `command`, `schemaVersion` (`"1.0"` form), `canonVersion` in `meta`.
- **Q6 Kind name:** decided 2026-10-22: `evidence` everywhere a kind is named to a user (context, `--kind` values, JSON). Storage paths and the record's own `kind` field stay `evidence_record`.
- **Q7 Telemetry, NDJSON output:** not in 0.28. JSON/NDJSON *input* is in (D7).
- **Q8 Identifier flags and operands:** decided 2026-10-22: (c), the primary object is an operand, by the three rules in D1.
- **Q9 Guide from the binary:** proposed (D10, Orca `skills get`, named `skills show` here). Replaces the installed full skill with a stub that sends the agent to `canon skills show`; the full text lives in the binary.
