# Canon 0.28.0: one CLI grammar, generated from one spec

- **Status:** PLANNED. Starts after 0.26.0 (brief prose) ships; 0.27 is not reserved for this.
- **Base:** `main` after 0.26.0.
- **Version:** 0.28.0 (minor). One release carries the whole grammar change set; a later minor only removes the deprecated spellings.
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
| format vs lint | `format` never rewrites: it validates corpus shape, has a no-op `--check`, and an alias `fmt`. Out of scope for 0.28 (owner decision 2026-10-22); `format` keeps its name and behavior. | C8; bigcli §1 |
| Option names | One name with several shapes: `--to`, `--change-id`, `--scenario-id`, `--providers`. One name with several meanings: `--repo`, `--kind`, `--status`, `--role`/`--as`, and `--version` on `prompt`. One concept with many names: actor ×3, approver ×3, sha ×5, signature ×4, manifest ×2. | O2–O16, O22 |
| Command names | Same action at two scopes: `selftest`/`gate selftest`, `gate promote`/`divergence promote`. One verb with two actions: `migrate`/`artifacts migrate`. `artifacts` has 3 meanings. `context` vs `context-pack`. Nouns that act: `gate task`, `evidence vault`, `subject status --to`, `regime-key`. | C1–C16, X3 |
| Safety | Only `purge` is `destructive`, and it uses `--plan` / `--confirm <digest>`. `artifacts migrate` deletes local blobs by default. 7 of 46 writing leaves have `--dry-run`. There is no `--no-input`. | X4, O6, O18–O20 |
| Self-description | 64 options have no help. Validated enums have empty `possibleValues`. Some defaults exist only in prose. Top-level help is 4.3 KB. `--repo` and `--json` are redeclared per command with 7 and 10 help strings. 289 of 379 value names are parser-derived. | D1–D8, X8, X9 |
| Original report | Not yet planned: an operand rule; a boolean-naming rule; "refused leaves the tree unchanged"; declared idempotency; `error.received`; truthful `retryable`; `meta.context`; `meta.next[].argv`. | coverage A3 |
| Callers | 30 conformance steps pin JSON bodies. About 293 test invocations parse `--json`. The shipped retrieve hook parses a bare array and fails silently if it is wrapped. `SKILL.src.md` points at `meta.next`, but `status` has a top-level `next`. | callers §1–7 |

Already right: kebab-case everywhere; `--json` on every leaf; `--as` declared once; paging flattened; the `-id` rule; schema derived from clap with drift tests; registered error codes with hints; a runnable `example` per leaf.

## Decisions

### D1 Grammar doc

`docs/design/cli-grammar.md` is one normative page. It holds the concept table, verbs, placeholders, envelope, exit table, safety rule, and owner decisions with their sources. Its tables are machine-readable: `docs/design/cli-vocabulary.tsv` maps option → value name → concept.

- **Verbs:**
  - `new` creates an authored unit (feature, scenario, subject, change).
  - `add` records an attributed record.
  - `list` and `show` read.
  - `check` gives a verdict over repo state.
  - `verify` re-hashes stored objects.
  - `validate` checks the shape of one input file.
  - `approve` records a human approval.
  - `accept` records a reviewer taking a known gap.
  - `update`, `move`, `get`, `delete` are not used; the original report's CRUD set is rejected (coverage A2).
- **Placeholders:** `<DIR> <FILE> <PATH> <ID> <SHA> <N> <TIMESTAMP> <NAME> <TEXT> <URL> <KIND>` plus the concept ids (`<CHANGE_ID>`, …).
- **Booleans** name a mode (`--all`, `--check`, `--no-promote`).
- **Operands:** a command takes at most one operand kind: the group's own noun, or paths. Context is never an operand.
- **A mode that changes a command's options is its own command**, even when most options are shared (owner decision 2026-10-22). Shared options are declared once and flattened into each.

### D2 One generated command spec

`docs/cli/canon.cli.json` is checked in, deterministic, and marked DO-NOT-EDIT. It contains:

- `specVersion` and `cli.version`;
- global options and env vars;
- the exit table;
- envelope JSON Schemas;
- all error codes with meaning, exit and retryability;
- every node: leaves, pure groups and runnable groups (`nodeKind`). Each node carries `about`, `effect`, `effectNote`, `idempotency`, `roles`, operands, options (help, value name, default, `possibleValues`, `deprecated`), `requiredOneOf`, `exits`, `errorCodes`, `outputs` (`kind: report|artifact|stream`, `schemaRef`, `schemaVersion`), `inputs` (e.g. NDJSON batch lines with `schemaRef`) and `examples`;
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
  - a dated Deprecations table;
  - `canon completion <bash|zsh|fish>`, which reads the embedded spec at runtime;
  - man pages via `clap_mangen` at packaging.

### D3 Envelope

- **Success:** `{ok: true, data, meta}`.
- **Failure:** `{ok: false, error, meta}`, plus `data` when the command has a partial report (e.g. `gate check` violations).
- **`meta`:** `command` (the schema id; flags never change it); `schemaVersion` (`major.minor` per report: a consumer accepts unknown fields within a major and rejects an unknown major, terraform's rule, bigcli §4.3); `canonVersion`; `context` (resolved repo root, role, actor); `next` (each with `command` text and `argv` array); paging; `warnings`; `deprecated`.
- **`error`:** `code` first, then `message`, `field`, `expected` (always the valid set), `received`, `hint` (a working invocation, never clap's usage blob), `retryable` (true only where a retry can succeed).
- **Format:** compact JSON. JSON mode is decided from argv before parsing, so pre-parse errors are JSON too.
- **Not wrapped:** artifacts and streams.
- **Unchanged by environment:** no environment variable or TTY changes the format (oxlint #27569, bigcli §0).

### D4 Exit table

0 ok; 1 the command's finding or failed check; 2 refused (usage, `CONFIG_MISSING`, unreadable input, invalid argument). Each leaf declares its own rows. `brief check` flips to 1 = blockers.

### D5 Context

`--repo <DIR>` is optional and declared once (flatten).

- Absent: the nearest ancestor with `canon.yaml`.
- Present: that directory.
- A command needing canon state that finds none refuses `CONFIG_MISSING`.
- It replaces `skills * --target`, the cwd-relative `ingest sessions --canon-yaml`, and the `regime-key --repo` key segment (renamed `--repo-name`).

### D6 Safety

- **Preview by default, act with `--commit`** for commands that delete or rewrite data others depend on. The set is decided in Q1.
- **`purge`:** a bare run previews, prints `planDigest`, `risk` and `effects[]`, and exits 0. It acts with `--commit --plan-digest <DIGEST>`, like terraform's saved plan; destructive acts also need the target named.
- **`--dry-run`** with one help string on every other writing leaf where a preview helps: init, skills install, gate install-hooks, inventory sync, plugin sync, artifacts store, ingest *, export, subject adopt/unadopt.
- **Refusal leaves the tree unchanged.** This is tested over every writing leaf; a repeated write is a no-op where declared idempotent.
- **`--override-reason <TEXT>`** stays the only "proceed although refused" flag; there is no `--force`.
- **`--no-input`:** `--json` implies it.

### D8 Renames in one release

- Every rename in 0.28 keeps the old spelling working as a hidden alias. The alias prints the stderr line, adds `meta.deprecated {used, use, since, removedIn}`, and appears in the spec as `deprecated`.
- The next minor removes the alias; the usage error then names the replacement (`removed.rs`, extended with a command table).
- **Hard cuts in 0.28 that cannot warn:** the 13 bare reports move into `data`; `retrieve` becomes an envelope; `brief check` exits flip; and the kind name is unified (Q6).
  - Each one bumps the affected `schemaVersion` major and has an old → new key list in the README Upgrading section.
  - The retrieve hook reads both shapes.
  - `gate install-hooks` rewrites installed hook commands.
  - `canon migrate` gains a step that rewrites canon's own scripts and hooks in a consumer repo.
- A renamed command also changes shape in this one release. This is accepted because its old name keeps working.

### D9 Callers move in the same release

Skill text, the AGENTS block, website en/ko, dashboard, `trace_check.py`, conformance (reviewed re-bless; every JSON body validated against its schema), hook scripts and examples.

## Renames (proposal; owner confirms in Q2)

| Kind | Now | 0.28 |
| --- | --- | --- |
| command | `evidence add --kind audit …` | `evidence audit` (own options: `--evaluator-id/-version/-digest`, `--audit-result`; no `--verdict`/`--task`/`--surface-ref`/`--approval-*`) |
| command | `review add --scenario-id …` / `review add --change-id …` | `review scenario` (pins scenario digest + evidence checks) and `review change` (pins the chore brief) |
| command | `selftest`, `gate selftest` | `selftest [--suite <NAME>]` |
| command | `gate promote`, `divergence promote` | `gate promote [--kind …]` only |
| command | `learn promote/approve/rollback` | `strategy promote`, `strategy approval-payload`, `strategy rollback` |
| command | `artifacts migrate --to <BACKEND>` | `artifacts move --tier <TIER>` |
| command | `evidence vault` | `artifacts store` |
| command | `ingest artifacts` | `ingest records` |
| command | `context-pack new` | `context-pack add` |
| command | `demo add` | `demo review` |
| command | bare `brief` | `brief show` |
| command | `subject status <ID> [--to <STATE>]` | `subject show <ID>` and `subject status <ID> --to <STATE>` (write only) |
| command | `divergence status` | `divergence list` |
| command | `gate task <TASK_ID>` | `task done <TASK_ID>` |
| command | `regime-key` | `regime show` |
| option | `--agent-id` / `--reviewer` (id) | `--actor-id` (`--reviewer` dropped where `--actor-id --role reviewer` says it) |
| option | `--by`, `--principal`, `--approval-by` | `--approver` |
| option | `--sha`, `--artifact-sha`, `--reviewed-sha`, `--resolution-sha`, `--introduced-by` | `--commit`, `--reviewed-commit`, `--resolution-commit`, `--introduced-in` |
| option | `--approved-at`, `--signature`, `--approval-signature-file`, `--context-manifest` | `--approval-at`, `--signature-file`, `--manifest` |
| option | `prompt --version` | `--bundle-version` |
| option | `export --out` | `-o/--output` |
| option | `retrieve -k/--k` | `--limit` |
| option | `evidence add --kind` | `--class` |
| option | `dispatch end --status`, `divergence add --status` | `--outcome`, `--state` |
| option | `--role` (attribution) | stays, defaulting to `--as`; `regime-key --role` → `--role-segment` |
| option | `--fields` (repeatable + comma list) | one form: `--fields <A,B>` |
| option | `ingest plans --source`, `report --snapshot` | `--plan-source`, `--snapshot-dir` |
| option | `init --check-config` | `init --check` |
| kind | `evidence_record` in `canon context` | `evidence` (Q6) |

Open, recommended against: `learn`/`dispatch` as noun groups (`strategy` above covers `learn`; `dispatch` stays the toolchain verb for a run), `context` → `authoring`, `query/export/purge` under `records`.

## Tests (all in 0.28)

| Test | Proves |
| --- | --- |
| T1 parser walk | Every clap node (leaf, group) is in the spec and the reverse; no runnable group; `debug_assert()`. |
| T2 metadata | Exhaustive `meta()` compiles; no orphan META rows. |
| T3 options | Help, value name from the placeholder list, real default, `possibleValues` for closed sets; one name → one value name, arity and meaning; vocabulary membership both ways; verb denylist for valued options; short aliases only `-h -V -o -q`; no standard name redefined. |
| T4 spec drift | The regenerated spec equals the checked-in one. |
| T5 errors | Registry = spec; every listed code is emitted; every emitted code is listed. |
| T6 outputs | Every leaf declares outputs (or `[]` with a reason). |
| T7 output validation | In debug builds every emitted JSON document is validated against its schema, so the whole suite's ~293 `--json` calls become checks; `json_contract.rs` also validates each leaf. |
| T8 conformance | Every JSON `expected/N.out` validates against its command's schema. |
| T9 witness | Every declared output was emitted at least once by the suite; a shrinking allowlist for the stateful leaves. |
| T10 typing | No `json!` printing outside `json_out`; no untyped `Value` fields in any report schema. |
| T11 effect honesty | Every `read` leaf run in a scratch repo leaves the tree unchanged unless `effectNote` names the write. |
| T12 refusal | Every refused write leaves the tree unchanged; declared-idempotent writes are no-ops the second time. |
| T13 context | `kinds` = `RecordKind::ALL`; `families` = `FamilyKind::ALL`; every `--kind` value set ⊆ `kinds`; every spec command in `context.commands` per role. |
| T14 docs | Every command line in skill text, website docs, the AGENTS block, hints and `meta.next` parses against the spec. |
| T15 mutation | Seeded violations (drop a META row, an option's help, add a `json!` print, a runnable group, a duplicate name) each fail the right test. |

## Slices

| Slice | Scope | Depends on |
| --- | --- | --- |
| S0 | Extract the clap tree into `canon_cli::cli`; exhaustive `meta()`; groups `subcommand_required`; `brief show` | — |
| S1 | Spec generator, drift test, `canon completion`, man pages | S0 |
| S2 | `Report` trait, envelope D3, error D3; type the ~20 typed reports | S0 |
| S3a–e | Type the ~49 `json!` payloads by module (evidence/review/finding; gate/divergence/subject; dispatch/ingest/learn; retention/export/purge/query; init/migrate/skills/context/misc) | S2 |
| S4 | `--repo` D5 (#109), effect honesty (#110), safety D6, `--dry-run`/`--no-input`, exit table D4 | S0 |
| S5 | Mode splits: `evidence audit`, `review scenario`, `review change` (old forms alias with deprecation) | S0 |
| S6 | Renames table with aliases and deprecation (D8), `removed.rs` command table | S0, S5 |
| S7 | `canon context` commands/families/kind name; capability version 6 | S1 |
| S8 | Derived docs: website reference, skill/AGENTS blocks between markers, doc lint T14, Deprecations table; skill text and README | S1, S6 |
| S9 | Tests T1–T15, witness fixtures, conformance re-bless with review | all |
| R | README 0.28.0 with the Upgrading list, version, dogfood eval, release | all |

Effort from coverage B7: ~24–27 engineer-days serial, ~8–9 calendar days fanned out. S3 is the bulk.

**Acceptance:**
- the workspace passes;
- canon's own gate is clean with new scenarios evidenced and independently reviewed;
- T1–T15 green with empty allowlists except the documented witness list;
- a dogfood eval (omp, roles, `language: ko`) on a 0.26 baseline shows 0 wrong-shape calls and no more `--help`/`schema` probes per scenario than eval 12, and a canon 0.25-era hook and script set migrated by `canon migrate`.

## Owner decisions

- **Q1 Preview by default + `--commit`:** which of `artifacts move` (deletes local blobs), `migrate` (git mv + rewrites canon.yaml), `ledger compact`, `tier age`, `gate promote`, `strategy rollback`, `subject status --to retired|shipped`.
- **Q2 Renames:** confirm the table above.
- **Q3 Mode splits:** decided 2026-10-22: split even when options overlap (`evidence audit`; `review scenario` / `review change`).
- **Q4 Envelope:** `{ok, data, meta}` with `command`/`schemaVersion` in `meta` (recommended; 57 leaves and their callers already read `.data`) vs the flat form.
- **Q6 Kind name:** `evidence` everywhere (recommended; it follows the 0.24 no-`_record` rule).
- **Q7 Issues only:** telemetry and NDJSON outputs (recommended).
- **Q8 Identifier flags and operands:** every leaf is at most two words deep (14 top-level, 56 `group verb`), and 16 leaves take an operand. Options: (a) keep `--<noun>-id` everywhere (0.24 rule); (b) drop the suffix; (c) the command's primary object is an operand (`review scenario <SCENARIO_ID>...`, `evidence audit <SCENARIO_ID>`, `finding close <FINDING_KEY>`), every other identifier stays `--<noun>-id`. Recommended (c).
