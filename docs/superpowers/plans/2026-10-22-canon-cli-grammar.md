# Canon CLI grammar: one predictable interface (0.27 – 0.30)

- **Status:** PLANNED. Not started; 0.26 (brief prose) is in flight and lands first.
- **Base:** `main` after 0.26.0.
- **Versions:** four minors, each shippable on its own (R1–R4 below).
- **Evidence:** `docs/research/2026-10-22-cli-audit-surface.md` (rule-by-rule, ids C/O/P/D/X), `docs/research/2026-10-22-cli-audit-output.md` (every leaf × JSON/text/error shape), `docs/research/2026-10-22-cli-audit-callers.md` (who parses what). Audited against canon 0.25.0 with the cli-interface-design checklist and its worked example (Lute `docs/design/cli-grammar.md`, `crates/lute-cli/tests/cli_grammar_048.rs`).

## Principle

An agent that has used one canon command should guess the next one right: one
name per concept, one meaning per name, one JSON envelope, one exit table, and a
schema that tells the truth about what a command does. Every rule is enforced by
a test over `canon schema`, not by review.

## Why (measured on 0.25.0)

| Area | Finding | Audit id |
| --- | --- | --- |
| Truthful schema | `status` is `effect: read` but refreshes the scenario index and writes ledger files; `query` appends an audit log; `dispatch diff` creates `hot.db`; `report --check` writes seed files | X1 |
| Gate safety | `--repo` discovers `canon.yaml` only for the literal `.`; `gate check` in a directory without `canon.yaml` or with `--repo <subdir>` prints `clean` and exits 0 | X2 |
| JSON envelope | 57 leaves print `{data, meta}`; 13 print a bare report (`status`, `brief check`, `context`, `query`, `finding list`, `dispatch diff`, `context-pack show/verify`, `prompt show`, `adapter validate/authorize`, `selftest`, `retrieve` as a bare array); no `ok`, no `schemaVersion`; errors carry no command id; 6 failure shapes | output §1–3 |
| Exit codes | `brief check`: 2 = blockers, 1 = error (reverse of `gate check`/`format`); `format /missing` exits 0; 37 leaves share a boilerplate exit table | X5, output §6 |
| Option names | one name, several shapes: `--to` (STATE/BACKEND), `--change-id` (SLUG), `--scenario-id` (single/repeatable), `--providers` (LIST/PROVIDERS); one name, several meanings: `--repo` (root / key segment / corpus base), `--kind` (record / evidence class / change kind), `--status` ×3, `--role` vs `--as` ×4, `--version` redefined on `prompt *`; one concept, many names: actor (`--actor-id`, `--agent-id`, `--reviewer`), approver (`--by`, `--principal`, `--approval-by`), sha ×5, signature ×4, manifest ×2 | O2–O16, O22 |
| Command names | same action at two scopes (`selftest`/`gate selftest`, `gate promote`/`divergence promote`); one verb, two actions (`migrate`/`artifacts migrate`); `artifacts` means three things; `context` vs `context-pack`; `brief` is a group and a runnable leaf missing from `schema --all`; nouns that act (`gate task`, `evidence vault`, `subject status --to`, `regime-key`); `format` (alias `fmt`) never rewrites and has a no-op `--check` | C1–C16, X3 |
| Safety | only `purge` is `destructive`; it previews with `--plan` and acts with `--confirm <digest>`; `artifacts migrate` deletes local blobs by default; `--dry-run` on 7 of 46 writing leaves; no `--no-input` | X4, O6, O18–O20 |
| Self-description | no `outputs` (report/artifact/stream) in schema; 64 options without help; validated enums with empty `possibleValues`; prose-only defaults; top-level help 4.3 KB; `--repo`/`--json` redeclared on 63/70 commands with 7/10 help strings; 289 of 379 value names parser-derived | D1–D8, X8, X9 |
| Callers | 30 conformance steps pin JSON bodies; ~293 test invocations parse `--json`; the shipped `canon-retrieve-pre-dispatch.sh` hook parses `retrieve --json` as a bare array and fails silently if it is wrapped; `SKILL.src.md` tells agents to read `meta.next` from `status --json`, which has a top-level `next` | callers §1–7 |

Already right: kebab-case everywhere, `--json` on every leaf, `--as` declared once, flattened paging, the `-id` rule, schema derived from clap with drift tests, registered error codes with hints, runnable `example` per leaf.

## Decisions

- **D1 Grammar doc and tables are the source of truth.** `docs/design/cli-grammar.md` (one page, normative: concept table, verb list, placeholder list, envelope, exit table, safety rule, owner decisions and their sources), `docs/design/cli-vocabulary.tsv` (option → value name → concept), `docs/design/cli-commands.tsv` (command → shared-option groups: repo, json, dry-run, staged-write, paging; effect; outputs). Placeholders: `<DIR> <FILE> <PATH> <ID> <SHA> <N> <TIMESTAMP> <NAME> <TEXT> <URL> <KIND>` plus the concept ids (`<CHANGE_ID>` …).
- **D2 Enforcement test** `crates/canon-cli/tests/cli_grammar.rs` over `schema --all --json`, with a mutation test per rule (Lute's `mutated()` pattern). Rules: R1 kebab-case; R2 one name → one value name, arity, meaning; R3 vocabulary membership both ways; R4 placeholder list parsed from the grammar doc; R5 verb denylist for valued options (`confirm`, `out`, …); R6 shared-option groups per `cli-commands.tsv`; R7 short aliases only `-h -V -o -q`; R8 every `--json` leaf has an `outputs` entry; R9 every option has help; R10 validated enums expose `possibleValues`; R11 a `read` leaf run in a scratch repo leaves the tree unchanged unless `effectNote` names the write; R12 no runnable group; R13 exit 1 = finding, 2 = refused; R14 `meta.command` equals the schema id (flags never change it); R15 no standard name redefined; R16 every command or option named in skill text, website docs, the AGENTS block, hints and `meta.next` strings exists in the clap tree. Violations that later releases remove are listed in an explicit, shrinking allowlist in the test.
- **D3 Envelope.** Success `{ok: true, data, meta}`; failure `{ok: false, error, meta}` (plus `data` when the command has a partial report, e.g. `gate check` violations). `meta` keeps `command` (schema id), adds `schemaVersion` (`<release>.<report>`, unchanged while the payload is unchanged), and keeps `next`, paging, `warnings`. Error keeps `code` first, `message`, `field`, `expected` (always the valid set), `hint` (a working invocation, not clap's usage blob), `retryable`. Compact JSON. Artifacts and streams are not wrapped. Deviation from the skill's flat envelope (`command`/`schemaVersion` at top level) is recorded in the grammar doc: 57 leaves and their consumers already read `.data`.
- **D4 Exit table** for every leaf: 0 ok, 1 the command's finding or failed check, 2 refused (usage, missing config, unreadable input, invalid argument). `brief check` flips to 1 = blockers.
- **D5 Context.** `--repo <DIR>` is `Option`: absent = nearest ancestor with `canon.yaml`; present = that directory; a command that needs canon state and finds none refuses `CONFIG_MISSING` (exit 2). Declared once (flatten), on every command that reads repo state, including `skills *` (replacing `--target`) and `ingest sessions` (replacing the cwd-relative `--canon-yaml`).
- **D6 Safety.** Commands that delete or rewrite data others depend on preview by default and act with `--commit` (no short alias). `purge`: bare run previews and prints `planDigest`; act with `--commit --plan-digest <DIGEST>` (keeps the digest binding). Candidates for the same treatment are an owner decision (Q1). `--dry-run` with one help string on every other writing leaf where a preview helps (init, skills install, gate install-hooks, inventory sync, plugin sync, evidence vault, ingest *, export, subject adopt/unadopt). `--override-reason <TEXT>` stays canon's "proceed although refused" and is registered as such; no `--force`. `--no-input`, and `--json` implies it.
- **D7 Renames.** Every rename gets one minor where the old spelling still runs, prints the 0.24-style stderr line and `meta.deprecated`, and appears as `deprecatedAliases` in schema; the next minor removes it and the usage error names the replacement (`removed.rs`, extended with a command-rename table). A command never changes envelope shape and name in the same release.
- **D8 Callers move in the same release**: skill text, AGENTS block, website en/ko, dashboard, `trace_check.py`, conformance (reviewed re-bless), hook scripts, examples. The retrieve hook accepts both shapes one release before `retrieve` is wrapped.

## Releases

| Release | Kind | Scope |
| --- | --- | --- |
| **R1 = 0.27** | non-breaking + bug fixes | D1 doc and tables; D2 test with the allowlist; flatten `RepoArgs`/`JsonArgs`/`DryRunArgs`; X1 effect honesty (stop writing on read paths or declare `effectNote`); X2 `--repo` discovery + `CONFIG_MISSING` (the gate stops being fail-open: documented as a behavior fix); additive envelope fields (`ok`, `meta.schemaVersion`, error `meta.command`) on the 57 `{data, meta}` leaves; schema `outputs`, `hasSubcommands`, real defaults, `possibleValues`, help on all 64 options; one-sentence `about` + `long_about` (top-level help ≤ 3 KB); `--dry-run` coverage; `--no-input`; one time grammar (`YYYY-MM-DD` or RFC3339) with errors naming the option; retrieve hook reads both shapes; R16 doc lint; `removed.rs` command table; small bugs from the output audit (purge `--before` message, `purge --kind` listing, manifest error as `SERIALIZE_FAILED`, `learn rollback` hint, `scenario new --from` help, `init --providers bogus` exit 0, `format /missing` exit 0) |
| **R2 = 0.28** | new names, old ones warn | option renames O2–O16, O22 per the vocabulary (e.g. `--agent-id`→`--actor-id`, `--reviewer`→`--reviewer-id` or dropped, approver = `--approver`, sha = `--commit`/`--reviewed-commit`/`--resolution-commit`, `--version`→`--bundle-version`, `--out`→`-o/--output`, `retrieve -k`→`--limit`, `evidence add --kind`→`--class`, `dispatch end --status`→`--outcome`, `artifacts migrate --to`→`--tier`); command renames C1–C16 decided in Q2 (old command ids keep working with a warning); `gate install-hooks` rewrites installed hook commands |
| **R3 = 0.29** | envelope cut | the 13 bare reports move into `data` (per-command old→new key list in the README), `retrieve` stops being a bare array, `report --check` id fixed, `brief check` exit flip, failure shapes unified; skill and docs updated; conformance re-blessed with review |
| **R4 = 0.30** | removal | R2's deprecated spellings and command ids removed; usage errors name the replacement; allowlist in `cli_grammar.rs` empty |

Each release: plan slice table, independent reviewer process, dogfood eval measuring wrong-shape calls and `--help`/`schema` probes per scenario against the previous release (0.25 baseline: 0 naming errors in eval 12, 6–8 probes per run), README release section with an Upgrading list.

## Owner decisions needed (before R2)

- **Q1 Dangerous set.** Which of `artifacts migrate` (deletes local blobs), `migrate` (git mv + rewrites canon.yaml), `ledger compact`, `tier age`, `gate promote`, `learn rollback`, `subject status --to retired|shipped` become preview-by-default + `--commit`.
- **Q2 Command renames.** Recommended: `selftest --suite` (drop `gate selftest`); one staged-drain `promote` (drop `divergence promote`), `learn promote`→`strategy promote`; `artifacts migrate`→`artifacts move --tier`; `ingest artifacts`→`ingest records`; `evidence vault`→`artifacts store`; `learn approve`→`learn approval-payload`; `context-pack new`→`context-pack add`; `demo add`→`demo review`; `format`→`lint` (drop `fmt`, no-op `--check`); bare `brief`→`brief status`; `subject status`→`subject show` + `subject move --to`; `divergence status`→`divergence list`; `finding close`→`finding update` (keep `close` as sugar?); `gate task`→`task done`. Open: `learn`/`dispatch` as verb groups vs `strategy`/`run` noun groups; `context`→`authoring`; `regime-key`→`regime show`; top-level `query/export/purge` under `records`.
- **Q3 Mode splits.** Whether `evidence add --kind audit` becomes `evidence audit` and `review add` splits into scenario and change reviews (a kind that changes options is a subcommand), or stays one command with `requiredOneOf`.
- **Q4 Envelope.** Confirm D3 (`ok` top level, `command`/`schemaVersion` in `meta`) over the skill's flat form.
- **Q5 Hard cut vs warn for R3.** JSON shapes cannot warn through argv; R3 is a documented hard cut. Alternative: emit both shapes for one release (rejected in the caller audit: `context --json` is 22 KB).

## Issues to file

- R1 bugs worth fixing earlier if 0.26 has room: X2 (`gate check` false green without `canon.yaml`), X1 (`status` writes while declared read).
- One tracking issue per release R1–R4 linking this plan.
