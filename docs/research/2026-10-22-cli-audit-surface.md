# canon 0.25.0 CLI surface audit (SurfaceAudit)

Scope: `canon schema --all --json` plus `canon schema <leaf> --json` for all 70 leaves (installed `canon 0.25.0`, main e347991f), `crates/canon-cli/src/main.rs`, `crates/canon-cli/tests/`, checked against the cli-interface-design skill and Lute's `docs/design/cli-grammar.md` + `crates/lute-cli/tests/cli_grammar_048.rs`.
Evidence tags: `cmd --opt` = a schema field, `file:line` = source, `$ ...` = a command I ran (scratch repos under `<scratch>/`, deleted at the end). `[INFERENCE]` = not exercised.

**Side-effect disclosure (mine).** Running `canon status` inside `<canon checkout>` refreshed the scenario index (330 ledger records written, 631 untracked files under `.canon/archive/kind=scenario/`). I removed the untracked archive files with `git clean -- .canon/archive`; `git status` was clean afterwards. The gitignored ledger/index files stay refreshed (regenerable). This is itself finding **X1** below.

---

## 0. Headline findings (ordered by what would bite a caller)

| # | finding | evidence | proposed fix | breaks callers? |
|---|---|---|---|---|
| X1 | `status` is declared `effect: read` but writes: it calls `refresh_stale_index` before reporting. `dispatch diff` (read) creates `.canon/hot.db{,-shm,-wal}`; `report --check` (effect `write`, no `effectNote`) writes parquet seed files while "checking"; `query` (read) appends `.canon/audit/access.jsonl`. Agents and sandboxes trust `effect`/`roles`. | `main.rs:1914`; `$ canon status` in canon repo; diff of file lists in r3 for `dispatch diff`, `report --check`, `query --kind scenario` | make the read paths not write, or declare it: `effectNote` on each ("refreshes the scenario index when feature files changed", ...), and add a test that runs every `read` leaf in a scratch repo and diffs the tree | No (behaviour becomes stricter / schema gains a field) |
| X2 | `--repo` default is discovery only for the literal `.`. Any other value is used as-is. `canon gate check --repo <subdir-without-canon.yaml>` and `canon gate check` in a dir with no `canon.yaml` both print `clean (0 violations)` and exit 0 (false green). `schema` reports `default: ["."]`, help prints both "default: nearest ancestor" and `[default: .]`. | `context.rs:345-369`; `$ canon gate check --repo sub` -> clean, `cd sub && canon gate check` -> 1 violation; `$ cd empty && canon gate check --json` -> `{"clean":true}` exit 0; `tests/schema.rs:81-88` pins `repo.default[0] == "."` | make `repo` `Option<PathBuf>`: absent = discover nearest `canon.yaml` ancestor; present = as-is; commands that need canon state fail `CONFIG_MISSING` (exit 2) when none; schema `default` = `null` + help "discovered" | Behaviour change only for no-canon dirs (gate check turns 0->2) |
| X3 | `canon brief` is a group **and** a runnable leaf (bare `canon brief` prints state + next steps, exit 1 `NO_BRIEF` in an empty repo). It is absent from `schema --all`, so the JSON sweep (`json_contract.rs`), role filter and examples never see it. `brief` also owns `--change-id/--repo/--json` as `global = true`, the only group doing that. | `main.rs:225-237`; `$ canon schema brief --json` lists only `brief approve`/`brief check`; all 23 other groups exit 2 when bare (`$ canon <group>`) | add `brief status` (noun view, like `git status`) and make bare `brief` print help (exit 2 like other groups); or list `brief` as a leaf in schema | Yes for bare `canon brief` callers (after_help, skills) |
| X4 | Dangerous commands do not follow "preview by default, `--commit` to act". `purge`: bare run refuses (exit 2 `PURGE_PLAN_REQUIRED`), preview is `--plan`, act is `--confirm <planDigest>`. `artifacts migrate` (removes local blobs, "moving bytes back is not supported"), `migrate` (git mv + rewrites canon.yaml), `ledger compact`, `tier age` execute by default with `--dry-run`. Only `purge` has `effect: destructive`. | `$ canon purge --kind run --before 2099-01-01T00:00:00Z` -> exit 2; `canon artifacts migrate --help`; schema `effect` histogram: read 24, write 45, destructive 1 | `purge`: bare = preview (exit 0, prints `planDigest`), act with `--commit --plan-digest <DIGEST>` (keeps the digest binding, drops `--plan`/`--confirm`); classify `artifacts migrate` (and owner decision on `migrate`, `tier age`, `ledger compact`) as dangerous -> preview default + `--commit`, `effect: destructive` | Yes (`purge` scripts; `artifacts migrate` scripts) |
| X5 | Inverted exit table on one command: `brief check` = 0 clean, **2 blockers**, **1 operational error**; everywhere else 1 = the command's finding (`gate check`, `format`, `report --check`, `skills check`, `artifacts verify`) and 2 = refused/usage. 37 leaves share the boilerplate "1: operational failure, or a blocking finding", 21 leaves declare no exit 1. | `canon schema brief check --json` exits; `main.rs:224` after_help "exit 0 clean, 2 blockers, 1 error" | flip `brief check` to 1 = blockers, 2 = refused (clean cutover; note in release) | Yes (anything branching on `brief check` exit) |
| X6 | Standard name `--version` redefined on `prompt add` / `prompt show` (`--version <VERSION>`, required) -> `canon prompt show -V` is an unexpected-argument error and the flag means a bundle version, not the tool version. | `canon schema prompt show --json`; `$ canon prompt show -V` -> exit 2 | `--bundle-version <VERSION>`, or accept `--prompt-bundle <NAME@VERSION>` (already the spelling of `dispatch begin --prompt-bundle`) on both | Yes (prompt scripts) |
| X7 | Same-action-two-scopes commands: `selftest` vs `gate selftest` (the aggregator registers the gate one as a suite), `gate promote` vs `divergence promote` (`gate promote` "drains them too"), `migrate` vs `artifacts migrate` (two unrelated actions under one verb), `learn approve` vs `brief/policy approve` (this `approve` only prints an unsigned payload). | `selftest.rs:1-40`; `divergence.rs:17-18`; `canon schema learn approve` about | see section 2.1 (C1-C5) | Yes |
| X8 | `--json`, `--repo`, `--fields`, `--dry-run` are re-declared per command (63 `repo` fields in `main.rs`, one `json: bool` per command; `--as` is the only one declared once, `global = true`, `main.rs:77`). Result: 10 different help strings for `--json`, 7 for `--repo`, 4 for `--dry-run`, 6 for `--fields`; 7 commands have no `--repo` at all (see O3). | `main.rs` `Command` enum; schema help strings | flatten `RepoArgs`, `JsonArgs`, `DryRunArgs` (like existing `PageFlags`/`PromoteArgs`, `main.rs:921`) | No (help text normalised only) |
| X9 | No vocabulary / grammar doc / mechanical test beyond the 0.24 `-id` rule. 289 of 379 valued options show the parser-derived value name (`--task <TASK>`, `--repo <REPO>`); file-valued options use 11 different placeholders. | section 3, section 5 | add `docs/cli-grammar.md` + `cli-vocabulary.tsv` + `cli-commands.tsv` + `tests/cli_grammar.rs` | No (cosmetic until test lands) |

Things that are already right: all 70 ids and 133 option names are kebab-case; `--json` is on all 70 leaves (`supportsJson` true everywhere) and is always a flag; `--as` is declared once; paging (`--limit/--cursor/--all`) is one flattened struct on 4 commands; the `-id` suffix rule holds for change/project/subject/actor/scenario/session/run; schema is derived from clap and `META` drift is a test failure (`schema.rs:1-26`); errors, examples and roles have derived/drift-checked tables.

---

## 1. Inventory

Full per-leaf table: appendix A. Option-name table: appendix B. Raw dumps: `schema --all --json` = 70 leaves in 24 groups (`divergence` 6, `gate` 5, `subject` 4, 3 each in `context-pack dispatch evidence finding ingest learn skills`, 2 each in `adapter artifacts brief demo policy prompt`, 1 each in `change feature inventory ledger plugin review scenario tier`) plus 14 top-level leaves (`context dashboard export format init migrate purge query regime-key report retrieve schema selftest status`).

Numbers: 487 option rows = 70 `--as` + 70 `--json` + 63 `--repo` + 284 command-specific. 133 distinct option names, 72 used by exactly one command. Operands: 16 commands take one operand each (`schema [PATH]...` is the only variadic). Effects: read 24, write 45, destructive 1. Roles field: 2 leaves with `roles: []` = human-only (`brief approve`, `policy approve`). Exit sets: `0/1/2` 45, `0/2` 19, `0/1` 4, `0` 2 (`context`, `dispatch diff`).

Schema shape per option: `{name, valueName, kind: flag|value|positional, required, repeatable, default: [string], possibleValues, aliases, help, short?}`; per command: `{command, about, aliases, arguments, options, effect, effectNote?, roles, supportsJson, exits[{code,meaning}], errorCodes, example, requiredOneOf?}`. No `outputs`, no `hasSubcommands`, group nodes are not entries.

---

## 2. Rule-by-rule

Legend for "breaks callers": **Y** = existing invocations/scripts/docs stop working (use `removed.rs` `FLAGS` to name the replacement, as 0.25 did for 0.24 renames); **N** = additive/cosmetic.

### 2.1 Command names

| id | command(s) | what | rule | fix | breaks |
|---|---|---|---|---|---|
| C1 | `selftest`, `gate selftest` | same action at two scopes; `selftest` registers the gate suite (`selftest.rs`); both are developer fixtures in the public surface | "no two commands for one action at different scope" | keep `selftest [--suite <NAME>]`, remove `gate selftest`; consider hiding both from the default listing | Y (CI that calls `canon gate selftest`) |
| C2 | `gate promote`, `divergence promote` | one verb, one meaning (staged -> committed ledger), two commands; `divergence.rs:17-18`: "`canon gate promote` drains them too". `learn promote` is a third, different action (strategy -> git tier, signed) | one action, one command; one word one meaning | `promote [--kind evidence\|divergence]` (or keep `gate promote` as the only staged-drain and delete `divergence promote`); rename `learn promote` -> `strategy promote` | Y |
| C3 | `migrate`, `artifacts migrate` | same verb, unrelated actions (repo layout upgrade vs move blobs to an s3 tier). Also `--to <BACKEND>` vs `subject status --to <STATE>` | a name never carries two meanings | `artifacts migrate` -> `artifacts move --tier <TIER>` (it moves and deletes local copies); keep top-level `migrate` (repo upgrade) | Y |
| C4 | `artifacts` (group), `ingest artifacts`, `--artifact`, `--artifact-sha`, `--max-artifact-mib`, `evidence vault` | the word means three things: (1) blob store (`artifacts verify/migrate`, `--artifact`, `--max-artifact-mib`, `evidence vault`), (2) agent-run files to import (`ingest artifacts`: "review/divergence/task/handoff artifacts"), (3) `--artifact-sha` = a **git commit sha** "bound by the evidence", not a blob digest | one concept per name | (2) needs another noun (`ingest records`/`ingest runs`) [INFERENCE on the target noun; `--run-id` there says "trajectories"], (3) -> `--commit <SHA>`/`--sha` (already `divergence add --sha`); `evidence vault` -> `artifacts store` (it belongs to the store group) | Y |
| C5 | `brief approve`, `policy approve`, `learn approve`, `divergence accept`, `evidence approval-payload` | `approve` records a human approval in two commands, but `learn approve` only emits an unsigned payload for external signing (same job as `evidence approval-payload`). `accept` is a near-synonym used for a reviewer decision | near-synonyms side by side / one word one meaning | `learn approve` -> `learn approval-payload`; keep `approve` = records a human approval; document `accept` (reviewer takes a known gap) vs `approve` | Y (`learn approve` only) |
| C6 | `new` vs `add` | `feature/scenario/subject/change new` (authored units) vs `evidence/finding/review/divergence add` (attributed records) is a workable rule, but `context-pack new` (store object from a manifest), `prompt add` (registers from a manifest) and `demo add` ("record the missing reviewer evidence") break it | near-synonyms | write the rule in the grammar doc; `context-pack new` -> `context-pack add` (same shape as `prompt add`); `demo add` -> `demo review` | Y (low traffic) |
| C7 | `check`/`verify`/`validate`/`doctor` | `brief check`, `gate check`, `skills check` (findings, exit 1); `artifacts verify`, `context-pack verify` (integrity of stored bytes); `adapter validate`, `format` (input shape); `skills doctor` (diagnostics) | near-synonyms | adopt: `check` = verdict over repo state, `verify` = re-hash stored objects, `validate` = shape of one input file; `format` -> see C8; no renames needed beyond C8 if written down | N |
| C8 | `format` (+ visible alias `fmt`) | never rewrites; the skill reserves `fmt` for canonical rewriters with `--check`. Takes `--check` that does nothing ("kept for compatibility", `main.rs:190`, `main.rs:1911` `check: _`). Second spelling `fmt` (schema resolves it). Operand `<ROOT>` plus `--repo` (context twice) | verbs; one name per concept; operands vs context | rename to `lint [<ROOT>]` (or `spec check`), drop alias `fmt` and the no-op `--check`; `ROOT` defaults to the configured spec roots | Y |
| C9 | `brief` | group + executable leaf (X3) | "a group only groups its subcommands" | `brief status` leaf; bare = help | Y |
| C10 | `subject status` | noun used as verb: shows when no `--to`, transitions with `--to` (effect differs: schema carries `effectNote`); `divergence status` is a list; top-level `status` a report | verbs; read-only views may be nouns but must stay read-only | split: `subject show <ID>` (read) + `subject move <ID> --to <STATE>` (write); `divergence status` -> `divergence list` | Y |
| C11 | `finding close` | also reopens: `--disposition open` ("reopening is a real transition"); `finding add --disposition fixed` can create an already-closed finding | name vs behaviour | `finding update <KEY> --disposition ...` (keep `close` as sugar only if wanted) | Y |
| C12 | `learn`, `dispatch`, `ingest` | verb groups; `learn promote`/`dispatch begin` read verb+verb, `ingest sessions` reads verb+noun whereas the rest are noun+verb | one ordering: noun group + verb | `learn *` -> `strategy *`, `dispatch begin/end/diff` -> `run begin/end/diff` (the id is `--run-id`), `ingest` keep (it is the toolchain verb for import; record as owner decision) | Y |
| C13 | `gate task` | noun-named verb ("flip one task checkbox"); also the default command of `gate install-hooks --command` (`canon gate task`), so a rename invalidates installed hooks in users' `.claude/settings.json` / `.codex/hooks.json` | actions are verbs | `task done <ID>`/`gate flip`; migrate hook installer and ship `gate install-hooks` rewrite | Y (and installed hooks) |
| C14 | single-verb groups: `change new`, `feature new`, `scenario new`, `inventory sync`, `plugin sync`, `ledger compact`, `tier age`, `review add` | 8 groups with one child | none; a group of one is allowed | [INFERENCE: low value to flatten; keep for namespace reserve] | N |
| C15 | `query`, `export`, `purge`, `report`, `retrieve`, `regime-key`, `dashboard` | top-level record verbs sit beside groups; `regime-key` is a hyphenated noun that builds a string for hooks | group + verb ordering | optional `records query/export/purge`; `regime-key` -> `regime show` [INFERENCE on names] | Y (optional) |
| C16 | `context` vs `context-pack` (and `--context-manifest`) | `context` = what you can author here; `context-pack` = immutable pack of selected docs; one word, two concepts | words with two meanings | `context` -> `authoring` [INFERENCE on name]; leave `context-pack` | Y |

Id stability ("flags never change the id"): `report --check` and `report --snapshot` appear as ids in `meta.command` and error text (`$ canon report --check --json` -> `"meta":{"command":"report --check"}`; `main.rs:3040` emits `"canon report --snapshot"`). `status`, `context`, `dispatch diff`, `query` print bare objects with no `meta.command` (`$ canon status --json | jq keys`). `schema <path>` reports `meta.command: "schema"`. Cross-ref OutputAudit.

### 2.2 Option names

| id | option(s) | what | rule | fix | breaks |
|---|---|---|---|---|---|
| O1 | `--repo` ×63, `--json` ×70, `--fields` ×6, `--dry-run` ×7 | declared per command, 7/10/6/4 different help strings | declare shared options once | flatten structs; `--json` help one string (`Output JSON instead of the human-readable form`) | N |
| O2 | `--repo` | three meanings: repo root (61), **`regime-key --repo <REPO>` = a string segment of the regime key** (`The <repo> segment (canonicalized)`), `format --repo` = base the corpus `<ROOT>` resolves under; semantics: `.` discovers, others as-is (X2); `init`/`demo init`: "Directory to set up (used as-is)" | a name never carries two meanings | `regime-key --repo` -> `--repo-name <NAME>`; `format` context per C8 | Y (regime-key) |
| O3 | context spelled five ways | `--repo` (63), `skills * --target`, `ingest sessions --canon-yaml canon.yaml` (no `--repo`; cwd-relative default), `query/tier age --canon-yaml` (second option for the same context), `adapter authorize --repo*` **required** with help "Repository root containing `.canon/policy.yaml`", `adapter validate --repo` optional with no default ("Optional repo root for context-pack join verification"). Commands with no `--repo`: `schema`, `selftest`, `gate selftest` (legitimate), `skills check/doctor/install` (`--target`), `ingest sessions`, `regime-key` (different meaning) | context is one option with a discovered default | `skills --target` -> `--repo`; `ingest sessions` gets `--repo` and derives canon.yaml; `--canon-yaml` only as override on `query`/`tier age` (document), `adapter authorize --repo` gets default | Y (`skills --target`, `ingest sessions` callers pass nothing today) |
| O4 | `--version` on `prompt add/show` | collides with standard `--version` (X6) | standard names | `--bundle-version` | Y |
| O5 | `export --out` | non-standard output spelling, no help, no short | standard `-o/--output` | `-o/--output <PATH>` | Y |
| O6 | `purge --plan`, `purge --confirm <PLAN_DIGEST>` | `--confirm` is a **valued verb**; `--plan` means preview, but "plan" elsewhere = the task plan (`--task`, `ingest plans`, `dispatch diff` "declared plan DAG", "plan corpus") | valued options are nouns; one word one meaning; `--dry-run` is the standard preview | see X4 (`--commit`, `--plan-digest`) | Y |
| O7 | `--check` | `report --check` (byte-diff, exit 1 on drift: matches the rule), `format --check` (no-op), `init --check-config` (validate canon.yaml) | a name never carries two meanings | drop `format --check`; `init --check-config` -> `init --check` [INFERENCE: acceptable since init "writes canonical files"] | Y |
| O8 | `retrieve --k <K>` (`-k`) | the only short alias in the tree besides clap's `-h`/`-V`; it is the result cap = `--limit` elsewhere (`query`, `export`, `finding list`, `divergence status`: `--limit <LIMIT>`) | short only for very common flags; same concept same name | `retrieve --limit <N>`, drop `-k` | Y |
| O9 | `--status` | 3 meanings: `dispatch end --status {succeeded,failed}` (run outcome), `divergence add --status` (open/still-divergent/resolved/deferred, **no possibleValues**), `query --status` (a change/task status field); neighbours: `--disposition`, `--verdict`, `--outcome`, `--audit-result`, `--to <STATE>` | one concept per name | `dispatch end --outcome`, `divergence add --state`, `query --status` keep; list the vocabulary row per record kind | Y |
| O10 | `--kind` | 3 meanings: record kind (`query/export/purge`, 14 values), evidence class (`evidence add`, free text, `audit` special-cased), change kind (`change new`, `feature`/`chore`) | a name never carries two meanings | `evidence add --class <CLASS>` or `--evidence-kind`; keep `--kind` for record kind; `change new --kind` ok (template) | Y |
| O11 | `--role` vs `--as` | `--as` = the role I act as (global, 70 commands); `--role` = attribution role (11 commands) whose default is `--as`; `retrieve --role` = scope; `regime-key --role` = a key segment; `dispatch begin --role` "must equal --regime's leading segment". Same value name `ROLE`, four semantics | one name per concept, one concept per name | keep `--as`; rename `--role` to `--attest-as`/`--reviewer-role` per use; `regime-key --role` -> `--role-segment` or accept `--regime` parts via flags | Y |
| O12 | identity spelled many ways | actor: `--actor-id` (13), `--agent-id` (`dispatch begin`: "The dispatching agent's id (recorded as the run's actor)"), `--reviewer` (6: `divergence add/accept/defer/resolve`, `finding add`, `review add`, alongside `--actor-id`), human approver: `--by` (brief/policy approve), `--principal` (`learn approve`), `--approval-by` (`evidence add`); `--actor-id` default is `canon` (10 cmds) or `canon-scaffold` (`feature/scenario new`) or required (`learn rollback`) | one name per concept | `--agent-id` -> `--actor-id`; `--reviewer` -> `--reviewer-id` (id) and drop when `--actor-id` + `--role reviewer` already say it; approver = `--approver` on all four; one default | Y |
| O13 | sha spelled five ways | `--sha` (`divergence add/defer/resolve`), `--artifact-sha`, `--reviewed-sha`, `--resolution-sha`, `--introduced-by` | one name per concept | `--commit <SHA>` for "the commit", keep role-specific `--reviewed-commit`, `--resolution-commit`, `--introduced-by` -> `--introduced-in` [INFERENCE] | Y |
| O14 | other split concepts | approval time `--approval-at` (evidence) vs `--approved-at` (`learn rollback`); signature `--approval-signature-file`, `--signature-file`, `--signature`, `--approval`; manifest `--manifest` (`context-pack new`, `prompt add`) vs `--context-manifest` (`dispatch begin`); prompt bundle `--name --version` vs `--prompt-bundle name@version`; spec root `--project-id` (id) vs `--spec-root` (dir) vs `format <ROOT>`; reviewer-runtime `--reviewer-provider/-model/-harness` vs `dispatch begin --provider/--model` | one name per concept | unify on `--approval-at`, `--signature-file`, `--manifest`; see section 3 | Y |
| O15 | `-id` rule coverage | `one_name.rs:29` CONCEPTS = change, project, subject, actor, scenario, session, run. Still bare: `--task` (id `<change>#<n>`, while `gate task <TASK_ID>`), `--parent-run`, `--plugin`, `--regime` (a key), `--reviewer`, `--domain` | extend the checked list | add `task`, `plugin`, `parent-run`, `reviewer`, `agent` | Y |
| O16 | repeatable/plural | `--fields` is plural **and** repeatable **and** comma-separated (`value_delimiter = ','`); `--providers`/`--skills`-style plurals are one comma list; `--scenario-id` (singular, repeatable) + `--scenario-ids-from` (plural file) | repeatable options singular; plural = one collection | `--field <NAME>` repeatable or `--fields <A,B>` single; pick one; same for `--providers` | Y |
| O17 | `--no-promote`/`--promote` | verbs-as-flags, tri-state with `canon.yaml write.promote`; only on 4 of the staged writes | flags name a mode | keep; list in `cli-commands.tsv` as group `staged-write` | N |
| O18 | `--dry-run` coverage | on 7 of 46 write/destructive leaves; absent where a preview helps: `init`, `skills install`, `gate install-hooks` (writes `.claude/settings.json`), `inventory sync`, `plugin sync`, `evidence vault`, `ingest *`, `export`, `subject adopt/unadopt`, `demo init` | "`--dry-run` where a preview helps" | add to the listed; same help string | N |
| O19 | missing standard names | no `--format`, `-o/--output` (only `--out`), `--quiet`, `--verbose`, `--yes`, `--force`, `--commit`, `--no-input` anywhere. `--override-reason <REASON>` (2 commands: `evidence add`, `subject status`) is canon's "proceed although refused" with a mandatory reason: keep, and register it in the vocabulary as the sanctioned `--force` form | standard names | add `--commit` (X4), `--no-input` (O20) | N/Y |
| O20 | interactivity | only `init` prompts (`init.rs:411-421`: `stdin` and `stderr` TTY -> ask for harnesses, otherwise take detected). Value has a flag (`--providers`) and non-TTY never prompts, but there is no `--no-input`, and `--json` on a TTY still prompts [INFERENCE: not exercised: no PTY run] | prompts: every value has a flag; `--no-input` fails fast | `init --no-input`, and treat `--json` as no-input | N |
| O21 | time grammars | `--before/--after/--since` accept RFC3339 only (`$ canon query --kind run --since 2026-01-01` -> USAGE `premature end of input`; `export --before 2026-01-01` error text says "invalid **--since** timestamp" for `--before`, `query.rs:97`); `--expiry`: `divergence accept` takes a date (`YYYY-MM-DD`), `divergence add/defer` demand RFC3339 (`divergence.rs:42-47`; `$ canon divergence defer ... --expiry 2026-12-31` -> error) | same option same value grammar; errors name the valid set and an example | accept date and RFC3339 everywhere (`<TIMESTAMP>`: `YYYY-MM-DD` or RFC3339), error names the option | N (loosening) |
| O22 | `--source`, `--snapshot`, `--all`, `--full`, `--to`, `--domain`, `--name`, `--task` | `--source` skills source dir vs plan-ingest source root (paired with `--dialect`); `--snapshot` dashboard (dir to serve) vs report (dir to export parquet to); `--all` schema (every command) vs paging (every page); `--full` rehash vs rescan; `--to` `<BACKEND>` vs `<STATE>` | a name never carries two meanings | `ingest plans --source` -> `--plan-source`; `report --snapshot <DIR>` -> `--snapshot-dir`/`-o`; others document | Y (low) |

### 2.3 Operands

| id | command | what | rule | fix | breaks |
|---|---|---|---|---|---|
| P1 | `format <ROOT> --repo` | operand and context option both name a root | context is an option with a discovered default | `[<ROOT>]...` + `--repo` only for context (see C8) | Y |
| P2 | `subject adopt <CHANGE_ID> --subject-id`, `subject unadopt <CHANGE_ID> --subject-id`, `change new <SLUG> --subject-id` | the group is `subject`, the operand is a *change* and the option the *subject*; elsewhere `--change-id` is always an option (8 commands) and a change is an operand in 3 | one operand kind, operand = the thing acted on | `subject adopt <ID> --change-id <ID>` (operand = the subject); `change new --slug`? [INFERENCE: keep `change new <SLUG>`: it creates the change] | Y |
| P3 | same concept as operand in some commands, option in others | subject: operand `subject new/status <ID>`, option `--subject-id` ×5; task: operand `gate task <TASK_ID>`, option `--task` ×3 (`TASK`); plugin: operand `plugin sync <PLUGIN_ID>`, option `query --plugin`; value name `ID` is used for a subject (`subject new/status`) and for a context-pack digest (`context-pack show/verify`) | one name per concept | operand names -> `<SUBJECT_ID>`, `<PACK_ID>`; rule: the operand is the group's own noun | N/Y |
| P4 | kind that changes options is an option | `evidence add --kind audit` makes `--evaluator-id --evaluator-version --evaluator-digest --audit-result` required and **refuses** `--verdict` (help text); `review add` has two modes (`--scenario-id...` vs `--change-id` chore review; one of `--upstream-ref/--original-spec-ref` required, `requiredOneOf`); `scenario new <TAG>` vs `--from FILE\|-` batch (`requiredOneOf [["--from","tag"]]`); `evidence add` task vs scenario vs `--all-scenarios`; `query/export/purge --kind` scopes `--change-id/--status/--domain` ("other kinds exit 2") | "a kind that changes the command's own options is a subcommand" | `evidence audit`; `review scenario`/`review change` [INFERENCE: owner may prefer one `review add`]; `scenario new` vs `scenario import` for the batch; document `--kind` scoping in `schema` (`requiredWhen`) | Y |
| P5 | `scenario new [TAG]` / `feature new <SURFACE>` | operands are ids (`<area>.<surface>[.<nn>]`), not paths: fine as the one thing acted on | operands | none | N |

### 2.4 Safety and interactivity

See X4, O18-O20. Additional facts:
- Effect classes: only `purge` is `destructive`; `artifacts migrate` deletes local blobs after a verified upload and is `write`.
- `--dry-run` help differs by command (4 strings: "Preview what would be uploaded and removed...", "Preview without writing anything", "Report what would move and write nothing", "Print the plan and write nothing").
- Idempotence documented for `migrate` and `artifacts migrate`; `change new` fails `CHANGE_EXISTS` rather than no-op (acceptable).
- `status` writes (X1): "read" commands cannot be assumed side-effect free.

### 2.5 Self-description

| id | what | evidence | fix | breaks |
|---|---|---|---|---|
| D1 | no `outputs` (`{flag, kind: report/artifact/stream, schemaVersion}`): `supportsJson` only. Streams/artifacts unclassified: `dashboard --json` (serves), `ingest * --watch` (one doc per poll), `export` (artifact), `context-pack new/show` (manifest), `report --snapshot` (parquet) | schema key set | add `outputs` to `META` rows | N |
| D2 | group nodes are not described; `canon schema brief` hides the bare leaf (X3); no `hasSubcommands`/`summary` on listing; `short` key exists on 1 of 487 options (shape not stable) | `top.json`; `canon schema brief --json` | listing entries `{id, summary, hasSubcommands}`; always emit `short` | N |
| D3 | `default` is `[]` for defaults that exist in prose (`--limit` "default 100", `--policy` strict, `--k`, `--agent-id`); `default: ["false"]` strings on flags; `repo` default `"."` lies (X2) | schema | defaults from clap `default_value`; prose-only defaults moved to clap; flags `default: false` | N |
| D4 | `possibleValues` empty for validated enums: `review add --outcome` (signed/indeterminate/unavailable, default `signed`), `divergence add --status` (open/still-divergent/resolved/deferred), `evidence add --audit-result` (confirmed/refuted/indeterminate, per help), `evidence add --kind` (free text with a special `audit`) | `$ canon review add ... --outcome bogus` -> "expected one of: signed, indeterminate, unavailable" (but not in schema) | move to clap `value_parser`/`PossibleValuesParser` | N |
| D5 | 64 of 487 options have no help (13%): `divergence add` 10, `divergence defer` 9, `evidence approval-payload` 9, `divergence resolve` 8, `export` 7, `learn rollback` 6, `learn approve` 4, `purge` 4 (`--kind --before --repo --json`), `review add` 3, `divergence accept` 2, `skills check/doctor --source` | `empty` list | write help; add a test: no option without help | N |
| D6 | `about` length: `canon --help` is 4.3 KB; `schema` `about` of `review add` is 357 chars, `divergence accept` 353, `skills doctor` 230, `migrate` 200 (mean 91); the skill wants one sentence per command at top level and detail per command (Lute 2.8 KB) | `canon --help` is 4307 bytes (`wc -c`) | one-sentence `about` + `long_about` | N |
| D7 | exit table: see X5; generic boilerplate on 37 leaves | exits | per-command exits | Y (X5 only) |
| D8 | `example` field exists and is run by `schema_examples.rs` (good); `schema --errors` gives codes (good). Usage errors from clap are not in the registered error shape (`error.code USAGE` with `field`/`hint`; Lute's `argument`/`hint`); hint is the generic "For more information, try '--help'." | `$ canon purge ... --before 2099-01-01 --json` | hint = valid set/example | N |

---

## 3. Concept vocabulary canon actually uses

Columns: concept, option, value name(s) (`…` = repeatable), `n` commands. Concepts that appear under several option names are flagged in section 2 (O2, O9-O16, O22).

**Context location**

| concept | option | value name(s) | n | commands |
|---|---|---|---|---|
| repo root | `--repo` | `REPO` | 63 | `adapter authorize`, `adapter validate`, `artifacts migrate`, `artifacts verify`, `brief approve`, `brief check`, `change new` +56 |
| repo root (skills spelling) | `--target` | `TARGET` | 3 | `skills check`, `skills doctor`, `skills install` |
| canon.yaml file | `--canon-yaml` | `CANON_YAML` | 3 | `ingest sessions`, `query`, `tier age` |
| home dir | `--home` | `HOME` | 1 | `ingest sessions` |
| ad hoc spec root dir | `--spec-root` | `SPEC_ROOT` | 2 | `inventory sync`, `plugin sync` |
| spec root by id | `--project-id` | `PROJECT_ID` | 9 | `divergence accept`, `divergence add`, `divergence defer`, `divergence resolve`, `evidence add`, `evidence approval-payload`, `feature new` +2 |
| skills source dir / plan source root | `--source` | `SOURCE` | 4 | `ingest plans`, `skills check`, `skills doctor`, `skills install` |
| plan dialect | `--dialect` | `DIALECT` | 1 | `ingest plans` |
| snapshot dir (serve vs export) | `--snapshot` | `SNAPSHOT` | 2 | `dashboard`, `report` |
| output file | `--out` | `OUT` | 1 | `export` |

**Identity of a record or actor**

| concept | option | value name(s) | n | commands |
|---|---|---|---|---|
| change | `--change-id` | `CHANGE_ID` / `SLUG` | 8 | `brief approve`, `brief check`, `finding add`, `finding close`, `finding list`, `query`, `review add` +1 |
| subject | `--subject-id` | `SUBJECT_ID` | 5 | `change new`, `retrieve`, `scenario new`, `subject adopt`, `subject unadopt` |
| scenario | `--scenario-id` | `SCENARIO_ID` / `SCENARIO_IDS…` | 8 | `divergence accept`, `divergence add`, `divergence defer`, `divergence resolve`, `evidence add`, `evidence approval-payload`, `finding add` +1 |
| plan task | `--task` | `TASK` | 3 | `dispatch begin`, `evidence add`, `evidence approval-payload` |
| run | `--run-id` | `RUN_ID` | 4 | `dispatch end`, `evidence add`, `evidence approval-payload`, `ingest artifacts` |
| parent run | `--parent-run` | `PARENT_RUN` | 1 | `dispatch begin` |
| session | `--session-id` | `SESSION_ID` | 5 | `divergence accept`, `evidence add`, `finding add`, `finding close`, `review add` |
| actor | `--actor-id` | `ACTOR_ID` | 13 | `divergence accept`, `divergence add`, `divergence defer`, `divergence resolve`, `evidence add`, `feature new`, `finding add` +6 |
| actor (dispatch spelling) | `--agent-id` | `AGENT_ID` | 1 | `dispatch begin` |
| reviewer identity | `--reviewer` | `REVIEWER` | 6 | `divergence accept`, `divergence add`, `divergence defer`, `divergence resolve`, `finding add`, `review add` |
| acting role (record attribution) | `--role` | `ROLE` | 11 | `dispatch begin`, `divergence accept`, `divergence add`, `divergence defer`, `divergence resolve`, `evidence add`, `finding add` +4 |
| acting role (caller declaration) | `--as` | `ROLE` | 70 | `adapter authorize`, `adapter validate`, `artifacts migrate`, `artifacts verify`, `brief approve`, `brief check`, `change new` +63 |
| role that owns a subject | `--owner-role` | `OWNER_ROLE` | 1 | `subject new` |
| human approver | `--by` | `BY` | 2 | `brief approve`, `policy approve` |
| human approver (evidence) | `--approval-by` | `APPROVAL_BY` | 2 | `evidence add`, `evidence approval-payload` |
| human approver (learn) | `--principal` | `PRINCIPAL` | 1 | `learn approve` |
| approver role | `--approval-role` | `APPROVAL_ROLE` | 1 | `evidence add` |
| plugin | `--plugin` | `PLUGIN` | 1 | `query` |
| skill | `--skill-id` | `SKILL_ID` | 1 | `dispatch begin` |
| evaluator | `--evaluator-id` | `EVALUATOR_ID` | 1 | `evidence add` |
| regime key | `--regime` | `REGIME` | 2 | `dispatch begin`, `retrieve` |
| regime key parts | `--area` | `AREA` | 1 | `regime-key` |
| regime key parts | `--hash` | `HASH` | 1 | `regime-key` |
| domain slug | `--domain` | `DOMAIN` | 3 | `query`, `retrieve`, `subject new` |
| prompt bundle name | `--name` | `NAME` | 2 | `prompt add`, `prompt show` |
| prompt bundle version | `--version` | `VERSION` | 2 | `prompt add`, `prompt show` |
| prompt bundle (name@version) | `--prompt-bundle` | `PROMPT_BUNDLE` | 1 | `dispatch begin` |
| model/provider | `--model` | `MODEL` | 1 | `dispatch begin` |
| model/provider | `--provider` | `PROVIDER` | 1 | `dispatch begin` |
| providers (harness list) | `--providers` | `LIST` / `PROVIDERS` | 4 | `init`, `skills check`, `skills doctor`, `skills install` |
| reviewer runtime | `--reviewer-provider` | `REVIEWER_PROVIDER` | 1 | `review add` |
| reviewer runtime | `--reviewer-model` | `REVIEWER_MODEL` | 1 | `review add` |
| reviewer runtime | `--reviewer-harness` | `REVIEWER_HARNESS` | 1 | `review add` |
| trajectory | `--contradicting-trajectory-id` | `CONTRADICTING_TRAJECTORY_ID` | 1 | `learn rollback` |

**Time and ordering**

| concept | option | value name(s) | n | commands |
|---|---|---|---|---|
| upper time bound | `--before` | `BEFORE` | 2 | `export`, `purge` |
| lower time bound | `--after` | `AFTER` | 1 | `export` |
| lower time bound | `--since` | `SINCE` | 1 | `query` |
| clock override | `--as-of` | `AS_OF` | 1 | `divergence status` |
| expiry | `--expiry` | `EXPIRY` | 3 | `divergence accept`, `divergence add`, `divergence defer` |
| approval time | `--approval-at` | `APPROVAL_AT` | 2 | `evidence add`, `evidence approval-payload` |
| approval time (rollback) | `--approved-at` | `APPROVED_AT` | 1 | `learn rollback` |
| poll interval | `--interval-secs` | `INTERVAL_SECS` | 2 | `ingest artifacts`, `ingest sessions` |
| hook timeout | `--timeout` | `TIMEOUT` | 1 | `gate install-hooks` |
| review round | `--round` | `ROUND` | 5 | `divergence add`, `divergence defer`, `divergence resolve`, `finding add`, `finding close` |
| index in round | `--seq` | `SEQ` | 2 | `finding add`, `finding close` |

**Paging, projection, scope**

| concept | option | value name(s) | n | commands |
|---|---|---|---|---|
| page size | `--limit` | `LIMIT` | 4 | `divergence status`, `export`, `finding list`, `query` |
| page cursor | `--cursor` | `CURSOR` | 4 | `divergence status`, `export`, `finding list`, `query` |
| all pages / all commands | `--all` | `(flag)` | 5 | `divergence status`, `export`, `finding list`, `query`, `schema` |
| field projection | `--fields` | `FIELDS…` | 6 | `context`, `divergence status`, `export`, `finding list`, `query`, `status` |
| top-k | `--k` | `K` | 1 | `retrieve` |
| include history | `--history` | `(flag)` | 1 | `query` |
| full rescan / full rehash | `--full` | `(flag)` | 2 | `artifacts verify`, `ingest sessions` |
| all workspaces | `--all-workspaces` | `(flag)` | 1 | `ingest sessions` |
| only open | `--open` | `(flag)` | 1 | `finding list` |
| batch mode | `--all-scenarios` | `(flag)` | 1 | `evidence add` |

**Safety and mode**

| concept | option | value name(s) | n | commands |
|---|---|---|---|---|
| preview | `--dry-run` | `(flag)` | 7 | `artifacts migrate`, `divergence promote`, `gate promote`, `learn promote`, `ledger compact`, `migrate`, `tier age` |
| preview (purge) | `--plan` | `(flag)` | 1 | `purge` |
| confirm purge | `--confirm` | `PLAN_DIGEST` | 1 | `purge` |
| proceed anyway (with reason) | `--override-reason` | `OVERRIDE_REASON` | 2 | `evidence add`, `subject status` |
| stage→promote | `--promote` | `(flag)` | 4 | `divergence add`, `evidence add`, `finding add`, `finding close` |
| stage→promote | `--no-promote` | `(flag)` | 4 | `divergence add`, `evidence add`, `finding add`, `finding close` |
| sensitive output | `--include-sensitive` | `(flag)` | 2 | `export`, `query` |
| verify, no rewrite | `--check` | `(flag)` | 2 | `format`, `report` |
| validate config only | `--check-config` | `(flag)` | 1 | `init` |
| stay running | `--watch` | `(flag)` | 2 | `ingest artifacts`, `ingest sessions` |
| release-scoped check | `--release` | `(flag)` | 1 | `gate check` |

**Content of a record**

| concept | option | value name(s) | n | commands |
|---|---|---|---|---|
| record kind / evidence class / change kind | `--kind` | `KIND` | 5 | `change new`, `evidence add`, `export`, `purge`, `query` |
| verdict | `--verdict` | `VERDICT` | 1 | `evidence add` |
| status/state | `--status` | `STATUS` | 3 | `dispatch end`, `divergence add`, `query` |
| state transition target | `--to` | `BACKEND` / `STATE` | 2 | `artifacts migrate`, `subject status` |
| finding disposition | `--disposition` | `DISPOSITION` | 2 | `finding add`, `finding close` |
| severity | `--severity` | `SEVERITY` | 1 | `finding add` |
| review outcome | `--outcome` | `OUTCOME` | 1 | `review add` |
| audit result | `--audit-result` | `AUDIT_RESULT` | 1 | `evidence add` |
| reason | `--reason` | `REASON` | 6 | `divergence accept`, `divergence add`, `divergence defer`, `finding close`, `learn rollback`, `subject status` |
| note | `--note` | `NOTE` | 2 | `brief approve`, `policy approve` |
| detail | `--detail` | `DETAIL` | 2 | `divergence add`, `divergence resolve` |
| summary | `--summary` | `SUMMARY` | 3 | `evidence add`, `finding add`, `subject new` |
| title | `--title` | `TITLE` | 4 | `change new`, `feature new`, `scenario new`, `subject new` |
| command result | `--command-result` | `COMMAND_RESULT` | 1 | `evidence add` |
| reference | `--ref` | `REF` | 1 | `evidence add` |
| tracking | `--tracking` | `TRACKING` | 1 | `divergence accept` |
| lane | `--lane` | `LANE` | 1 | `scenario new` |
| case | `--case` | `CASE` | 1 | `scenario new` |
| flow | `--flow` | `F-ID` | 1 | `scenario new` |
| feature file | `--feature` | `FEATURE` | 1 | `scenario new` |
| git commit sha | `--sha` | `SHA` | 3 | `divergence add`, `divergence defer`, `divergence resolve` |
| git commit sha (artifact) | `--artifact-sha` | `ARTIFACT_SHA` | 2 | `evidence add`, `evidence approval-payload` |
| git commit sha (reviewed/resolution/introduced) | `--reviewed-sha` | `REVIEWED_SHA` | 1 | `finding add` |
| git commit sha | `--resolution-sha` | `RESOLUTION_SHA` | 2 | `finding add`, `finding close` |
| git commit sha | `--introduced-by` | `INTRODUCED_BY` | 1 | `finding add` |
| file:line ref | `--file-ref` | `FILE_REF` | 1 | `finding add` |
| review pin | `--pin` | `PIN` | 1 | `review add` |
| provenance ref | `--upstream-ref` | `UPSTREAM_REF` | 1 | `review add` |
| provenance ref | `--original-spec-ref` | `ORIGINAL_SPEC_REF` | 1 | `review add` |
| surface binding | `--surface-ref` | `SURFACE_REF…` | 2 | `evidence add`, `evidence approval-payload` |
| bound file | `--artifact` | `PATH…` | 2 | `evidence add`, `review add` |
| test report | `--report` | `FORMAT:PATH…` | 1 | `evidence add` |
| report case | `--report-case` | `NAME…` | 1 | `evidence add` |
| artifact size limit | `--max-artifact-mib` | `MIB` | 3 | `evidence add`, `evidence vault`, `review add` |

**File inputs**

| concept | option | value name(s) | n | commands |
|---|---|---|---|---|
| manifest file | `--manifest` | `MANIFEST` | 2 | `context-pack new`, `prompt add` |
| manifest file | `--context-manifest` | `CONTEXT_MANIFEST` | 1 | `dispatch begin` |
| response file | `--response` | `RESPONSE` | 2 | `adapter authorize`, `adapter validate` |
| evaluation file | `--evaluation` | `EVALUATION` | 2 | `learn approve`, `learn promote` |
| approval file | `--approval` | `APPROVAL` | 1 | `learn promote` |
| signature | `--signature` | `SIGNATURE` | 1 | `learn promote` |
| signature file | `--signature-file` | `SIGNATURE_FILE` | 1 | `learn rollback` |
| signature file | `--approval-signature-file` | `APPROVAL_SIGNATURE_FILE` | 1 | `evidence add` |
| scenario ids from file | `--scenario-ids-from` | `FILE\|-` | 1 | `review add` |
| NDJSON batch | `--from` | `FILE\|-` | 1 | `scenario new` |

**Misc**

| concept | option | value name(s) | n | commands |
|---|---|---|---|---|
| machine output | `--json` | `(flag)` | 70 | `adapter authorize`, `adapter validate`, `artifacts migrate`, `artifacts verify` +66 |
| dashboard port | `--port` | `PORT` | 1 | `dashboard` |
| skill digest | `--skill-digest` | `SKILL_DIGEST` | 1 | `dispatch begin` |
| evaluator version | `--evaluator-version` | `EVALUATOR_VERSION` | 1 | `evidence add` |
| evaluator digest | `--evaluator-digest` | `EVALUATOR_DIGEST` | 1 | `evidence add` |
| hook event | `--event` | `EVENT` | 1 | `gate install-hooks` |
| hook matcher | `--matcher` | `MATCHER` | 1 | `gate install-hooks` |
| hook command | `--command` | `COMMAND` | 1 | `gate install-hooks` |
| skip AGENTS.md | `--no-agents-md` | `(flag)` | 1 | `init` |
| policy level | `--policy` | `LEVEL` | 1 | `init` |
| skip policy | `--no-policy` | `(flag)` | 1 | `init` |
| brief language | `--language` | `TAG` | 1 | `init` |
| list error codes | `--errors` | `(flag)` | 1 | `schema` |

### 3.1 Value-name vocabulary today

| value name | n | options |
|---|---|---|
| `ACTOR_ID` | 13 | --actor-id |
| `PROJECT_ID` | 9 | --project-id |
| `CHANGE_ID` | 7 | --change-id |
| `FIELDS` | 6 | --fields |
| `REASON` | 6 | --reason |
| `REVIEWER` | 6 | --reviewer |
| `KIND` | 5 | --kind |
| `ROUND` | 5 | --round |
| `SCENARIO_ID` | 5 | --scenario-id |
| `SESSION_ID` | 5 | --session-id |
| `SUBJECT_ID` | 5 | --subject-id |
| `CURSOR` | 4 | --cursor |
| `LIMIT` | 4 | --limit |
| `RUN_ID` | 4 | --run-id |
| `SOURCE` | 4 | --source |
| `TITLE` | 4 | --title |
| `CANON_YAML` | 3 | --canon-yaml |
| `DOMAIN` | 3 | --domain |
| `EXPIRY` | 3 | --expiry |
| `MIB` | 3 | --max-artifact-mib |
| `NAME` | 3 | --name, --report-case |
| `PROVIDERS` | 3 | --providers |
| `SCENARIO_IDS` | 3 | --scenario-id |
| `SHA` | 3 | --sha |
| `STATUS` | 3 | --status |
| `SUMMARY` | 3 | --summary |
| `TARGET` | 3 | --target |
| `TASK` | 3 | --task |
| `APPROVAL_AT` | 2 | --approval-at |
| `APPROVAL_BY` | 2 | --approval-by |
| `ARTIFACT_SHA` | 2 | --artifact-sha |
| `BEFORE` | 2 | --before |
| `BY` | 2 | --by |
| `DETAIL` | 2 | --detail |
| `DISPOSITION` | 2 | --disposition |
| `EVALUATION` | 2 | --evaluation |
| `FILE\|-` | 2 | --from, --scenario-ids-from |
| `INTERVAL_SECS` | 2 | --interval-secs |
| `MANIFEST` | 2 | --manifest |
| `NOTE` | 2 | --note |
| `OVERRIDE_REASON` | 2 | --override-reason |
| `PATH` | 2 | --artifact |
| `REGIME` | 2 | --regime |
| `RESOLUTION_SHA` | 2 | --resolution-sha |
| `RESPONSE` | 2 | --response |
| `SEQ` | 2 | --seq |
| `SNAPSHOT` | 2 | --snapshot |
| `SPEC_ROOT` | 2 | --spec-root |
| `SURFACE_REF` | 2 | --surface-ref |
| `VERSION` | 2 | --version |
| `AFTER` | 1 | --after |
| `AGENT_ID` | 1 | --agent-id |
| `APPROVAL` | 1 | --approval |
| `APPROVAL_ROLE` | 1 | --approval-role |
| `APPROVAL_SIGNATURE_FILE` | 1 | --approval-signature-file |
| `APPROVED_AT` | 1 | --approved-at |
| `AREA` | 1 | --area |
| `AS_OF` | 1 | --as-of |
| `AUDIT_RESULT` | 1 | --audit-result |
| `BACKEND` | 1 | --to |
| `CASE` | 1 | --case |
| `COMMAND` | 1 | --command |
| `COMMAND_RESULT` | 1 | --command-result |
| `CONTEXT_MANIFEST` | 1 | --context-manifest |
| `CONTRADICTING_TRAJECTORY_ID` | 1 | --contradicting-trajectory-id |
| `DIALECT` | 1 | --dialect |
| `EVALUATOR_DIGEST` | 1 | --evaluator-digest |
| `EVALUATOR_ID` | 1 | --evaluator-id |
| `EVALUATOR_VERSION` | 1 | --evaluator-version |
| `EVENT` | 1 | --event |
| `F-ID` | 1 | --flow |
| `FEATURE` | 1 | --feature |
| `FILE_REF` | 1 | --file-ref |
| `FORMAT:PATH` | 1 | --report |
| `HASH` | 1 | --hash |
| `HOME` | 1 | --home |
| `INTRODUCED_BY` | 1 | --introduced-by |
| `K` | 1 | --k |
| `LANE` | 1 | --lane |
| `LEVEL` | 1 | --policy |
| `LIST` | 1 | --providers |
| `MATCHER` | 1 | --matcher |
| `MODEL` | 1 | --model |
| `ORIGINAL_SPEC_REF` | 1 | --original-spec-ref |
| `OUT` | 1 | --out |
| `OUTCOME` | 1 | --outcome |
| `OWNER_ROLE` | 1 | --owner-role |
| `PARENT_RUN` | 1 | --parent-run |
| `PIN` | 1 | --pin |
| `PLAN_DIGEST` | 1 | --confirm |
| `PLUGIN` | 1 | --plugin |
| `PORT` | 1 | --port |
| `PRINCIPAL` | 1 | --principal |
| `PROMPT_BUNDLE` | 1 | --prompt-bundle |
| `PROVIDER` | 1 | --provider |
| `REF` | 1 | --ref |
| `REVIEWED_SHA` | 1 | --reviewed-sha |
| `REVIEWER_HARNESS` | 1 | --reviewer-harness |
| `REVIEWER_MODEL` | 1 | --reviewer-model |
| `REVIEWER_PROVIDER` | 1 | --reviewer-provider |
| `SEVERITY` | 1 | --severity |
| `SIGNATURE` | 1 | --signature |
| `SIGNATURE_FILE` | 1 | --signature-file |
| `SINCE` | 1 | --since |
| `SKILL_DIGEST` | 1 | --skill-digest |
| `SKILL_ID` | 1 | --skill-id |
| `SLUG` | 1 | --change-id |
| `STATE` | 1 | --to |
| `TAG` | 1 | --language |
| `TIMEOUT` | 1 | --timeout |
| `TRACKING` | 1 | --tracking |
| `UPSTREAM_REF` | 1 | --upstream-ref |
| `VERDICT` | 1 | --verdict |

Observations:
- Parser-derived default shown: 289 of 379 valued options (`--task <TASK>`, `--reviewer <REVIEWER>`, `--round <ROUND>`). Skill: "never the parser's derived defaults".
- Explicit names that are not derived: `ROLE` (`--as`), `PATH`, `MIB`, `LEVEL`, `LIST`, `NAME`, `TAG`, `SLUG`, `STATE`, `BACKEND`, `PLAN_DIGEST`, `F-ID`, `FILE\|-`, `FORMAT:PATH`, `SCENARIO_IDS`.
- File-valued options use 11 different placeholders for one kind: `MANIFEST`, `CONTEXT_MANIFEST`, `RESPONSE`, `EVALUATION`, `APPROVAL`, `SIGNATURE`, `SIGNATURE_FILE`, `APPROVAL_SIGNATURE_FILE`, `CANON_YAML`, `OUT`, `PATH` (`--artifact`), `FILE\|-`; directory kinds use 6 more: `SNAPSHOT`, `HOME`, `SOURCE`, `TARGET`, `REPO`, `SPEC_ROOT`.
- Counts: `LIMIT`, `K`, `MIB`, `PORT`, `TIMEOUT`, `INTERVAL_SECS`, `ROUND`, `SEQ` are all `<N>`. Times: `BEFORE`, `AFTER`, `SINCE`, `AS_OF`, `EXPIRY`, `APPROVAL_AT`, `APPROVED_AT` are all `<TIMESTAMP>`. Ids: `<ID>` kinds already carry the noun (`CHANGE_ID`, `RUN_ID`), which is fine.
- Same name, two value names or arities: `--to` (`BACKEND` / `STATE`), `--change-id` (`CHANGE_ID` / `SLUG` on `review add`), `--scenario-id` (`SCENARIO_ID` single / `SCENARIO_IDS` repeatable), `--providers` (`LIST` on `init` / `PROVIDERS` on `skills *`).
- Proposed vocabulary (seed for `docs/cli-grammar.md` section 5): `<DIR>`, `<FILE>`, `<PATH>`, `<FILE\|->`, `<N>`, `<TIMESTAMP>`, `<ID>` + the `*_ID` family, `<NAME>`, `<KIND>`, `<ROLE>`, `<STATE>`, `<SHA>`, `<DIGEST>`, `<REASON>`, `<TEXT>`, `<FIELD>`, `<CURSOR>`, `<TIER>`, `<LEVEL>`, `<PROVIDER>`; composite forms `<FORMAT>:<PATH>` (`--report`), `<NAME>@<VERSION>` (`--prompt-bundle`), `<CHANGE>#<N>` (task id).

---

## 4. Command-name review (summary of 2.1)

Current shape: 14 top-level leaves + 24 groups / 56 leaves.

| axis | verdict |
|---|---|
| verbs vs nouns | actions are verbs except `gate task`, `evidence vault`, `regime-key`, `dashboard`, `subject status`, `divergence status`; read views as nouns are fine (`status`, `context`, `schema`, `policy show`) but `subject status` and `report` mutate / are mode-bearing |
| group + verb ordering | mostly noun group + verb (`evidence add`). Inverted: `learn *`, `dispatch *`, `ingest *` (verb groups). Top-level orphans on records: `query`, `export`, `purge`, `report`, `retrieve` |
| near-synonyms | `new`/`add` (rule in C6), `check`/`verify`/`validate`/`doctor` (C7), `approve`/`accept` (C5), `status`/`show`/`list`/`query` for reads (C10), `close`/`resolve` (finding vs divergence), `sync` ×2 (`inventory sync` indexes feature files, `plugin sync` writes overlay records: acceptable, same "reconcile derived state") |
| same action, two scopes | `selftest` / `gate selftest`; `gate promote` / `divergence promote`; `migrate` / `artifacts migrate` (one verb, two actions) |
| words with two meanings | `artifacts` (C4), `context` (C16), `plan` (`--plan`, `ingest plans`, `--task` plan corpus), `status` (O9), `approve` (C5), `promote` (C2), `kind` (O10), `--role`/`--as` (O11), `snapshot` (O22), `source` (O22), `--version` (O4) |
| `--repo` default discovery | `default_value = "."` (not `Option`); discovery lives in `context::resolve_repo_root` (`context.rs:363`), called from 28 source files; only the literal `.` walks (X2). `init`/`demo init` take "as-is". `ingest sessions` uses `--canon-yaml canon.yaml` (cwd-relative, no walk) |
| shared options declared once | `--as`: yes (`global`, `main.rs:77`). `PageFlags` (`paging.rs`, 4 commands), `PromoteArgs` (`main.rs:921`, 4 commands) flattened. Repeated per command: `--repo` (63), `--json` (70), `--dry-run` (7), `--fields` (6), `--actor-id` (13), `--project-id` (9), `--reviewer` (6), `--round/--seq` (5/2), `--max-artifact-mib` (3), `--approval-*`. `brief` is the only group with `global = true` args beside `--as` (`main.rs:229-236`) |

---

## 5. Comparison with Lute

### 5.1 Lute's mechanical checks vs canon

| Lute check (`cli_grammar_048.rs`) | canon today | gap size on the current surface |
|---|---|---|
| R1 option/command name is kebab-case (`is_kebab`) | not tested | 0 violations (verified: all 70 ids, 133 names) |
| R2 one option name -> one value name and one arity across commands | not tested | 4 names: `--to`, `--change-id`, `--scenario-id`, `--providers` |
| R3 option missing from checked-in `cli-vocabulary.tsv` (name, value, concept) or a row unused; vocabulary row value must match the surface | no vocabulary file; only `one_name.rs` CONCEPTS (7 nouns) | 133 option names to seed |
| R4 placeholder outside the grammar's §5 list (parsed from the doc itself, test `the_placeholder_vocabulary_is_parsed_from_the_grammar`) | no placeholder list | 115 distinct value names, 289 derived |
| R5 valued option uses a denylisted verb (`choose accept run explain dump-* out`) | not tested | `--out` (`export`), `--confirm` (`purge`) |
| R6 command's options differ from its row in `cli-commands.tsv` (shared-option groups per command); command/row set equality (`the_binary_lists_every_command_the_files_describe`) | no table | `--repo` absent on 7 leaves, `--dry-run` 7/46, staged-write group on 4 |
| R7 short alias other than `-o` | not tested | `retrieve -k` |
| R8 every `--json` command has a versioned `outputs` report entry (`every_json_command_has_a_versioned_report_or_artifact_entry`) | schema has no `outputs`; JSON sweep checks only "one JSON document + registered error code" | 70 leaves |
| mutation tests proving each rule catches a seeded violation (`rule_N_catches_*`, `mutated()`) | `one_name.rs:context_capability_14` feeds a synthetic split into the checker, the only one | rest missing |

### 5.2 What canon's tests already enforce (`crates/canon-cli/tests/`)

| test | enforces |
|---|---|
| `json_contract.rs` (`every_leaf_command_answers_json_with_one_document_and_a_coded_error`) | over all `schema --json` leaves: `--json` accepted, stdout is exactly one JSON document (a refusal counts), non-zero exit has a registered `error.code`, no hidden `USAGE` from a wrong sweep line. `SKIPPED` = `dashboard` only. Does not see group-leaves (`brief`) |
| `schema.rs` | listing sorted and stable, `purge` is `destructive`, option shape (`kind`, `repeatable`), `supportsJson == has --json`, errorCodes, exits per command, `--errors` equals `cli_error::all_codes()`, enum values and `repo.default[0] == "."` (pins X2's wrong default), alias `fmt` resolves, quoted path, unknown path suggests nearest |
| `schema_codes.rs` | per-command `errorCodes` table is derived from sources and diffed; every registered code is emitted by some command |
| `schema_examples.rs` | each leaf has one `example` and it runs (or is on the named exception list) |
| `one_name.rs` | no two flags differing only by `-id`; change/project/subject/actor/scenario/session/run always `-id`; no `--kind` value ends in `_record`; removed spellings (flags, kind values, `scenario new --from` keys) are usage errors with the replacement named; only on commands that renamed them |
| `grammar.rs` | 0.21 deprecated forms gone; removed command names are plain clap "unrecognized subcommand" (`grammar.rs:77-94`, no replacement hint, unlike flag renames); `schema` carries no deprecation; `subject status <id> --to` is the only transition form; every command declares `roles`; `CANON_ROLE`/`--as` narrow `schema` and default `--role` |
| `src/schema.rs::check_metadata` (+ test) | `META` row for every clap leaf and vice versa; fails on drift |
| `removed.rs` | `FLAGS`, `_record` kind values, batch keys; **no table for renamed commands** |

Not enforced anywhere: kebab-case, value-name consistency, placeholders, vocabulary, verb denylist, short aliases, shared-option membership, `outputs`, non-empty help, `possibleValues` for validated enums, effect honesty (X1), group/leaf collision (X3), exit-table consistency (X5), id stability (`report --check`), standard-name redefinition (`--version`).

### 5.3 Proposed test (shape, not code)

`crates/canon-cli/tests/cli_grammar.rs` walking `canon schema --all --json` + each leaf (as `one_name.rs::command_tree` does), with `docs/design/cli-vocabulary.tsv` (option, value, concept) and `docs/design/cli-commands.tsv` (command, repo, json, dry-run, staged-write, paging, effect, outputs). Rules to fail on: R1-R8 from Lute, plus
- R9 every option has help; R10 an option whose parser validates a closed set exposes `possibleValues`;
- R11 `effect: read` leaf run in a scratch repo leaves the file tree unchanged unless `effectNote` names the write (X1);
- R12 no group is runnable (a clap subcommand with `Option<Subcommand>`/`subcommand_required = false`, X3);
- R13 exit table: code 1 is "finding", 2 is "refused" for every leaf that declares both (X5);
- R14 `meta.command` of a success/error envelope equals the schema id (no `report --check`);
- R15 no option redefines a standard name with another meaning (`--version`, `--help`, `--json` non-flag, `--output` non-path).
Plus `rule_N_catches_*` mutation tests like Lute's `mutated()`.
Also extend `removed.rs` with a command-rename table so a removed command (`gate selftest`, `learn approve`, ...) says what replaced it (today `grammar.rs:77-94` accepts the bare clap error).

---

## 6. Release plan by breakage

1. **Non-breaking (ship first):** X1 (`effectNote`/purity + test), X2 (discovery, `CONFIG_MISSING`, schema default), X8/O1 (flatten `RepoArgs/JsonArgs/DryRunArgs`), D1-D6, O18, O20, O21, vocabulary/commands TSV + `cli_grammar.rs` + grammar doc, `removed.rs` command table.
2. **Option renames with `removed.rs` hints (one minor, then plain unknown-option):** O4 `--version`, O5 `--out`, O6/X4 purge, O8 `--k`, O9, O10, O11, O12, O13, O14, O15, O22, C3's `--to`.
3. **Command moves (clean cutover, hint table):** C1, C2, C3, C4, C5, C6 (`context-pack new`, `demo add`), C8 (`format`/`fmt`), C9/X3 (`brief`), C10, C11, C12, C13 (mind installed hooks), X5 (exit flip).
4. **Owner decisions needed:** which of `migrate`, `ledger compact`, `tier age`, `gate promote`, `learn rollback`, `subject status --to retired|shipped` are "dangerous" (preview default + `--commit`); whether verb groups `learn`/`dispatch` become noun groups (C12); whether `review add` and `evidence add --kind audit` split into subcommands (P4); `--override-reason` stays as the `--force` form (O19).

---

## Appendix A. Per-leaf table

| id | effect | roles | json | operands | exits | #err | options (excl. `--as`,`--json`; `*` required, `…` repeatable, `=` default, `{}` enum) |
|---|---|---|---|---|---|---|---|
| `adapter authorize` | read | O | y | – | 0/1/2 | 4 | --response <RESPONSE>*; --repo <REPO>* |
| `adapter validate` | read | O | y | – | 0/1/2 | 4 | --response <RESPONSE>*; --repo <REPO> |
| `artifacts migrate` | write | O | y | – | 0/2 | 7 | --to <BACKEND>*; --dry-run; --repo <REPO>=. |
| `artifacts verify` | read | O | y | – | 0/1/2 | 5 | --full; --repo <REPO>=. |
| `brief approve` | write | human | y | – | 0/1/2 | 13 | --by <BY>*; --note <NOTE>*; --change-id <CHANGE_ID>; --repo <REPO>=. |
| `brief check` | read | PR | y | – | 0/1/2 | 10 | --change-id <CHANGE_ID>; --repo <REPO>=. |
| `change new` | write | P | y | <SLUG> | 0/2 | 9 | --subject-id <SUBJECT_ID>*; --kind <KIND>=feature{feature/chore}; --title <TITLE>*; --repo <REPO>=. |
| `context-pack new` | write | OI | y | – | 0/1/2 | 13 | --manifest <MANIFEST>*; --repo <REPO>=. |
| `context-pack show` | read | OI | y | <ID> | 0/1/2 | 13 | --repo <REPO>=. |
| `context-pack verify` | read | OI | y | <ID> | 0/1/2 | 13 | --repo <REPO>=. |
| `context` | read | PIRO | y | – | 0 | 2 | --repo <REPO>=.; --fields <FIELDS>… |
| `dashboard` | write | O | y | – | 0/1/2 | 14 | --repo <REPO>=.; --snapshot <SNAPSHOT>; --port <PORT>=4173 |
| `demo add` | write | O | y | – | 0/2 | 4 | --repo <REPO>=. |
| `demo init` | write | O | y | – | 0/2 | 3 | --repo <REPO>=. |
| `dispatch begin` | write | O | y | – | 0/1/2 | 15 | --role <ROLE>*; --regime <REGIME>*; --agent-id <AGENT_ID>=canon; --provider <PROVIDER>; --model <MODEL>; --skill-id <SKILL_ID>; --skill-digest <SKILL_DIGEST>; --context-manifest <CONTEXT_MANIFEST>; --prompt-bundle <PROMPT_BUNDLE>; --task <TASK>; --parent-run <PARENT_RUN>; --repo <REPO>=. |
| `dispatch diff` | read | O | y | – | 0 | 2 | --repo <REPO>=. |
| `dispatch end` | write | O | y | – | 0/1/2 | 15 | --run-id <RUN_ID>*; --status <STATUS>*{succeeded/failed}; --repo <REPO>=. |
| `divergence accept` | write | R | y | – | 0/2 | 6 | --project-id <PROJECT_ID>*; --scenario-id <SCENARIO_IDS>*…; --reason <REASON>*; --tracking <TRACKING>*; --expiry <EXPIRY>; --reviewer <REVIEWER>*; --actor-id <ACTOR_ID>=canon; --role <ROLE>; --session-id <SESSION_ID>; --repo <REPO>=. |
| `divergence add` | write | IR | y | – | 0/1/2 | 7 | --project-id <PROJECT_ID>*; --scenario-id <SCENARIO_ID>*; --sha <SHA>*; --status <STATUS>=open; --reason <REASON>; --expiry <EXPIRY>; --round <ROUND>=1; --reviewer <REVIEWER>*; --detail <DETAIL>=; --actor-id <ACTOR_ID>=canon; --role <ROLE>*; --promote; --no-promote; --repo <REPO>=. |
| `divergence defer` | write | IR | y | – | 0/1/2 | 7 | --project-id <PROJECT_ID>*; --scenario-id <SCENARIO_ID>*; --sha <SHA>*; --round <ROUND>=1; --reviewer <REVIEWER>*; --reason <REASON>*; --expiry <EXPIRY>*; --actor-id <ACTOR_ID>=canon; --role <ROLE>*; --repo <REPO>=. |
| `divergence promote` | write | IO | y | – | 0/1/2 | 7 | --repo <REPO>=.; --dry-run |
| `divergence resolve` | write | IR | y | – | 0/1/2 | 7 | --project-id <PROJECT_ID>*; --scenario-id <SCENARIO_ID>*; --sha <SHA>*; --round <ROUND>=1; --reviewer <REVIEWER>*; --detail <DETAIL>=; --actor-id <ACTOR_ID>=canon; --role <ROLE>*; --repo <REPO>=. |
| `divergence status` | read | PIRO | y | – | 0/2 | 8 | --repo <REPO>=.; --as-of <AS_OF>; --limit <LIMIT>; --cursor <CURSOR>; --all; --fields <FIELDS>… |
| `evidence add` | write | I | y | – | 0/1/2 | 20 | --task <TASK>; --project-id <PROJECT_ID>; --kind <KIND>*; --ref <REF>*; --verdict <VERDICT>{faithful/not-applicable/divergent}; --summary <SUMMARY>; --command-result <COMMAND_RESULT>; --scenario-id <SCENARIO_ID>; --run-id <RUN_ID>; --surface-ref <SURFACE_REF>…; --actor-id <ACTOR_ID>=canon; --role <ROLE>; --session-id <SESSION_ID>; --approval-by <APPROVAL_BY>; --approval-role <APPROVAL_ROLE>; --approval-signature-file <APPROVAL_SIGNATURE_FILE>; --approval-at <APPROVAL_AT>; --artifact-sha <ARTIFACT_SHA>; --artifact <PATH>…; --report <FORMAT:PATH>…; --report-case <NAME>…; --all-scenarios; --max-artifact-mib <MIB>=25; --evaluator-id <EVALUATOR_ID>; --evaluator-version <EVALUATOR_VERSION>; --evaluator-digest <EVALUATOR_DIGEST>; --audit-result <AUDIT_RESULT>; --override-reason <OVERRIDE_REASON>; --promote; --no-promote; --repo <REPO>=. |
| `evidence approval-payload` | read | IO | y | – | 0/1/2 | 4 | --task <TASK>; --project-id <PROJECT_ID>; --scenario-id <SCENARIO_ID>; --run-id <RUN_ID>; --artifact-sha <ARTIFACT_SHA>*; --surface-ref <SURFACE_REF>…; --approval-by <APPROVAL_BY>*; --approval-at <APPROVAL_AT>*; --repo <REPO>=. |
| `evidence vault` | write | O | y | – | 0/2 | 5 | --max-artifact-mib <MIB>=25; --repo <REPO>=. |
| `export` | write | O | y | – | 0/1/2 | 6 | --kind <KIND>*{change/task/scenario/session/run/event/handoff/review/divergence/trajectory/strategy_item/evidence/subject/finding}; --before <BEFORE>; --after <AFTER>; --out <OUT>; --repo <REPO>=.; --include-sensitive; --limit <LIMIT>; --cursor <CURSOR>; --all; --fields <FIELDS>… |
| `feature new` | write | PI | y | <SURFACE> | 0/2 | 7 | --title <TITLE>*; --actor-id <ACTOR_ID>=canon-scaffold; --project-id <PROJECT_ID>; --repo <REPO>=. |
| `finding add` | write | R | y | – | 0/2 | 12 | --change-id <CHANGE_ID>*; --round <ROUND>*; --seq <SEQ>*; --severity <SEVERITY>*{blocker/should-fix/note}; --disposition <DISPOSITION>=open{open/fixed/rejected/deferred}; --reviewer <REVIEWER>*; --summary <SUMMARY>*; --reviewed-sha <REVIEWED_SHA>; --resolution-sha <RESOLUTION_SHA>; --introduced-by <INTRODUCED_BY>; --file-ref <FILE_REF>; --scenario-id <SCENARIO_IDS>…; --actor-id <ACTOR_ID>=canon; --role <ROLE>=reviewer; --session-id <SESSION_ID>; --promote; --no-promote; --repo <REPO>=. |
| `finding close` | write | R | y | – | 0/2 | 13 | --change-id <CHANGE_ID>*; --round <ROUND>*; --seq <SEQ>*; --disposition <DISPOSITION>*{open/fixed/rejected/deferred}; --resolution-sha <RESOLUTION_SHA>; --reason <REASON>; --actor-id <ACTOR_ID>=canon; --role <ROLE>=reviewer; --session-id <SESSION_ID>; --promote; --no-promote; --repo <REPO>=. |
| `finding list` | read | PIRO | y | – | 0/2 | 6 | --change-id <CHANGE_ID>*; --open; --repo <REPO>=.; --limit <LIMIT>; --cursor <CURSOR>; --all; --fields <FIELDS>… |
| `format` | read | PIRO | y | <ROOT> | 0/1 | 3 | --check; --repo <REPO> |
| `gate check` | read | PIRO | y | – | 0/1/2 | 5 | --repo <REPO>=.; --release |
| `gate install-hooks` | write | O | y | – | 0/1/2 | 4 | --repo <REPO>=.; --event <EVENT>=PreToolUse; --matcher <MATCHER>; --command <COMMAND>=canon gate task; --timeout <TIMEOUT>=30 |
| `gate promote` | write | I | y | – | 0/1/2 | 5 | --repo <REPO>=.; --dry-run |
| `gate selftest` | read | O | y | – | 0/1 | 3 |  |
| `gate task` | write | I | y | <TASK_ID> | 0/1/2 | 14 | --repo <REPO>=. |
| `ingest artifacts` | write | O | y | – | 0/1/2 | 3 | --watch; --interval-secs <INTERVAL_SECS>=30; --repo <REPO>=.; --run-id <RUN_ID> |
| `ingest plans` | write | O | y | – | 0/1/2 | 4 | --dialect <DIALECT>; --source <SOURCE>; --repo <REPO>=. |
| `ingest sessions` | write | O | y | – | 0/1/2 | 3 | --watch; --interval-secs <INTERVAL_SECS>=30; --home <HOME>; --canon-yaml <CANON_YAML>=canon.yaml; --full; --all-workspaces |
| `init` | write | O | y | – | 0/2 | 10 | --repo <REPO>=.; --check-config; --no-agents-md; --policy <LEVEL>{light/standard/strict}; --no-policy; --providers <LIST>; --language <TAG> |
| `inventory sync` | write | IO | y | – | 0/1/2 | 4 | --repo <REPO>=.; --spec-root <SPEC_ROOT> |
| `learn approve` | write | O | y | <STRATEGY_ID> | 0/1/2 | 8 | --evaluation <EVALUATION>*; --principal <PRINCIPAL>*; --repo <REPO>=. |
| `learn promote` | write | O | y | <STRATEGY_ID> | 0/1/2 | 9 | --repo <REPO>=.; --evaluation <EVALUATION>; --approval <APPROVAL>; --signature <SIGNATURE>; --dry-run |
| `learn rollback` | write | O | y | <STRATEGY_ID> | 0/1/2 | 9 | --reason <REASON>*; --actor-id <ACTOR_ID>*; --signature-file <SIGNATURE_FILE>*; --approved-at <APPROVED_AT>*; --contradicting-trajectory-id <CONTRADICTING_TRAJECTORY_ID>; --repo <REPO>=. |
| `ledger compact` | write | O | y | – | 0/1/2 | 5 | --dry-run; --repo <REPO>=. |
| `migrate` | write | O | y | – | 0/1/2 | 9 | --repo <REPO>=.; --dry-run |
| `plugin sync` | write | O | y | <PLUGIN_ID> | 0/1/2 | 4 | --repo <REPO>=.; --spec-root <SPEC_ROOT> |
| `policy approve` | write | human | y | – | 0/2 | 3 | --by <BY>*; --note <NOTE>; --repo <REPO>=. |
| `policy show` | read | O | y | – | 0/1/2 | 2 | --repo <REPO>=. |
| `prompt add` | write | O | y | – | 0/1/2 | 13 | --name <NAME>*; --version <VERSION>*; --manifest <MANIFEST>*; --repo <REPO>=. |
| `prompt show` | read | O | y | – | 0/1/2 | 13 | --name <NAME>*; --version <VERSION>*; --repo <REPO>=. |
| `purge` | destructive | O | y | – | 0/1/2 | 5 | --kind <KIND>*{change/task/scenario/session/run/event/handoff/review/divergence/trajectory/strategy_item/evidence/subject/finding}; --before <BEFORE>*; --repo <REPO>=.; --plan; --confirm <PLAN_DIGEST> |
| `query` | read | PIRO | y | – | 0/1/2 | 6 | --kind <KIND>*{change/task/scenario/session/run/event/handoff/review/divergence/trajectory/strategy_item/evidence/subject/finding}; --since <SINCE>; --repo <REPO>=.; --canon-yaml <CANON_YAML>; --plugin <PLUGIN>; --history; --change-id <CHANGE_ID>; --status <STATUS>; --domain <DOMAIN>; --include-sensitive; --limit <LIMIT>; --cursor <CURSOR>; --all; --fields <FIELDS>… |
| `regime-key` | read | O | y | – | 0/1/2 | 3 | --role <ROLE>*; --repo <REPO>*; --area <AREA>*; --hash <HASH>* |
| `report` | write | O | y | – | 0/1/2 | 5 | --repo <REPO>=.; --check; --snapshot <SNAPSHOT> |
| `retrieve` | read | PIRO | y | – | 0/1/2 | 3 | --role <ROLE>*; --regime <REGIME>; --domain <DOMAIN>; --subject-id <SUBJECT_ID>; -k/--k <K>; --repo <REPO>=. |
| `review add` | write | R | y | – | 0/2 | 7 | --project-id <PROJECT_ID>*; --scenario-id <SCENARIO_IDS>…; --scenario-ids-from <FILE\|->; --change-id <SLUG>; --reviewer <REVIEWER>*; --pin <PIN>*; --upstream-ref <UPSTREAM_REF>; --original-spec-ref <ORIGINAL_SPEC_REF>; --actor-id <ACTOR_ID>=canon; --role <ROLE>; --session-id <SESSION_ID>; --outcome <OUTCOME>=signed; --reviewer-provider <REVIEWER_PROVIDER>; --reviewer-model <REVIEWER_MODEL>; --reviewer-harness <REVIEWER_HARNESS>; --artifact <PATH>…; --max-artifact-mib <MIB>=25; --repo <REPO>=. |
| `scenario new` | write | PI | y | [TAG] | 0/2 | 10 | --from <FILE\|->; --title <TITLE>; --subject-id <SUBJECT_ID>; --lane <LANE>; --case <CASE>; --flow <F-ID>; --change-id <CHANGE_ID>; --actor-id <ACTOR_ID>=canon-scaffold; --feature <FEATURE>; --project-id <PROJECT_ID>; --repo <REPO>=. |
| `schema` | read | PIRO | y | [PATH]… | 0/2 | 3 | --errors; --all |
| `selftest` | read | O | y | – | 0/1 | 3 |  |
| `skills check` | read | O | y | – | 0/1/2 | 4 | --source <SOURCE>; --target <TARGET>=.; --providers <PROVIDERS> |
| `skills doctor` | read | O | y | – | 0/1/2 | 3 | --source <SOURCE>; --target <TARGET>=.; --providers <PROVIDERS> |
| `skills install` | write | O | y | – | 0/1/2 | 3 | --source <SOURCE>; --target <TARGET>=.; --providers <PROVIDERS> |
| `status` | read | PIRO | y | – | 0/1 | 3 | --repo <REPO>=.; --fields <FIELDS>… |
| `subject adopt` | write | PO | y | <CHANGE_ID> | 0/2 | 4 | --subject-id <SUBJECT_ID>*; --repo <REPO>=. |
| `subject new` | write | PO | y | <ID> | 0/2 | 7 | --domain <DOMAIN>*; --title <TITLE>*; --summary <SUMMARY>=; --owner-role <OWNER_ROLE>=implementer; --actor-id <ACTOR_ID>=canon; --repo <REPO>=. |
| `subject status` | write* | O | y | <ID> | 0/1/2 | 11 | --to <STATE>{proposed/specced/building/verifying/shipped/retired}; --override-reason <OVERRIDE_REASON>; --reason <REASON>; --actor-id <ACTOR_ID>=canon; --repo <REPO>=. |
| `subject unadopt` | write | PO | y | <CHANGE_ID> | 0/2 | 5 | --subject-id <SUBJECT_ID>*; --repo <REPO>=. |
| `tier age` | write | O | y | – | 0/1/2 | 3 | --dry-run; --repo <REPO>=.; --canon-yaml <CANON_YAML> |

## Appendix B. Option names (133)

| option | value name(s) | arity | required in | commands |
|---|---|---|---|---|
| `--actor-id` | `ACTOR_ID` |  | 1/13 | `divergence accept`, `divergence add`, `divergence defer`, `divergence resolve`, `evidence add`, `feature new` +7 |
| `--after` | `AFTER` |  | 0/1 | `export` |
| `--agent-id` | `AGENT_ID` |  | 0/1 | `dispatch begin` |
| `--all` | `-` |  | 0/5 | `divergence status`, `export`, `finding list`, `query`, `schema` |
| `--all-scenarios` | `-` |  | 0/1 | `evidence add` |
| `--all-workspaces` | `-` |  | 0/1 | `ingest sessions` |
| `--approval` | `APPROVAL` |  | 0/1 | `learn promote` |
| `--approval-at` | `APPROVAL_AT` |  | 1/2 | `evidence add`, `evidence approval-payload` |
| `--approval-by` | `APPROVAL_BY` |  | 1/2 | `evidence add`, `evidence approval-payload` |
| `--approval-role` | `APPROVAL_ROLE` |  | 0/1 | `evidence add` |
| `--approval-signature-file` | `APPROVAL_SIGNATURE_FILE` |  | 0/1 | `evidence add` |
| `--approved-at` | `APPROVED_AT` |  | 1/1 | `learn rollback` |
| `--area` | `AREA` |  | 1/1 | `regime-key` |
| `--artifact` | `PATH` | repeatable | 0/2 | `evidence add`, `review add` |
| `--artifact-sha` | `ARTIFACT_SHA` |  | 1/2 | `evidence add`, `evidence approval-payload` |
| `--as` | `ROLE` |  | 0/70 | `adapter authorize`, `adapter validate`, `artifacts migrate`, `artifacts verify`, `brief approve`, `brief check` +64 |
| `--as-of` | `AS_OF` |  | 0/1 | `divergence status` |
| `--audit-result` | `AUDIT_RESULT` |  | 0/1 | `evidence add` |
| `--before` | `BEFORE` |  | 1/2 | `export`, `purge` |
| `--by` | `BY` |  | 2/2 | `brief approve`, `policy approve` |
| `--canon-yaml` | `CANON_YAML` |  | 0/3 | `ingest sessions`, `query`, `tier age` |
| `--case` | `CASE` |  | 0/1 | `scenario new` |
| `--change-id` | `CHANGE_ID` / `SLUG` |  | 3/8 | `brief approve`, `brief check`, `finding add`, `finding close`, `finding list`, `query` +2 |
| `--check` | `-` |  | 0/2 | `format`, `report` |
| `--check-config` | `-` |  | 0/1 | `init` |
| `--command` | `COMMAND` |  | 0/1 | `gate install-hooks` |
| `--command-result` | `COMMAND_RESULT` |  | 0/1 | `evidence add` |
| `--confirm` | `PLAN_DIGEST` |  | 0/1 | `purge` |
| `--context-manifest` | `CONTEXT_MANIFEST` |  | 0/1 | `dispatch begin` |
| `--contradicting-trajectory-id` | `CONTRADICTING_TRAJECTORY_ID` |  | 0/1 | `learn rollback` |
| `--cursor` | `CURSOR` |  | 0/4 | `divergence status`, `export`, `finding list`, `query` |
| `--detail` | `DETAIL` |  | 0/2 | `divergence add`, `divergence resolve` |
| `--dialect` | `DIALECT` |  | 0/1 | `ingest plans` |
| `--disposition` | `DISPOSITION` |  | 1/2 | `finding add`, `finding close` |
| `--domain` | `DOMAIN` |  | 1/3 | `query`, `retrieve`, `subject new` |
| `--dry-run` | `-` |  | 0/7 | `artifacts migrate`, `divergence promote`, `gate promote`, `learn promote`, `ledger compact`, `migrate` +1 |
| `--errors` | `-` |  | 0/1 | `schema` |
| `--evaluation` | `EVALUATION` |  | 1/2 | `learn approve`, `learn promote` |
| `--evaluator-digest` | `EVALUATOR_DIGEST` |  | 0/1 | `evidence add` |
| `--evaluator-id` | `EVALUATOR_ID` |  | 0/1 | `evidence add` |
| `--evaluator-version` | `EVALUATOR_VERSION` |  | 0/1 | `evidence add` |
| `--event` | `EVENT` |  | 0/1 | `gate install-hooks` |
| `--expiry` | `EXPIRY` |  | 1/3 | `divergence accept`, `divergence add`, `divergence defer` |
| `--feature` | `FEATURE` |  | 0/1 | `scenario new` |
| `--fields` | `FIELDS` | repeatable | 0/6 | `context`, `divergence status`, `export`, `finding list`, `query`, `status` |
| `--file-ref` | `FILE_REF` |  | 0/1 | `finding add` |
| `--flow` | `F-ID` |  | 0/1 | `scenario new` |
| `--from` | `FILE\|-` |  | 0/1 | `scenario new` |
| `--full` | `-` |  | 0/2 | `artifacts verify`, `ingest sessions` |
| `--hash` | `HASH` |  | 1/1 | `regime-key` |
| `--history` | `-` |  | 0/1 | `query` |
| `--home` | `HOME` |  | 0/1 | `ingest sessions` |
| `--include-sensitive` | `-` |  | 0/2 | `export`, `query` |
| `--interval-secs` | `INTERVAL_SECS` |  | 0/2 | `ingest artifacts`, `ingest sessions` |
| `--introduced-by` | `INTRODUCED_BY` |  | 0/1 | `finding add` |
| `--json` | `-` |  | 0/70 | `adapter authorize`, `adapter validate`, `artifacts migrate`, `artifacts verify`, `brief approve`, `brief check` +64 |
| `--k` | `K` |  | 0/1 | `retrieve` |
| `--kind` | `KIND` |  | 4/5 | `change new`, `evidence add`, `export`, `purge`, `query` |
| `--lane` | `LANE` |  | 0/1 | `scenario new` |
| `--language` | `TAG` |  | 0/1 | `init` |
| `--limit` | `LIMIT` |  | 0/4 | `divergence status`, `export`, `finding list`, `query` |
| `--manifest` | `MANIFEST` |  | 2/2 | `context-pack new`, `prompt add` |
| `--matcher` | `MATCHER` |  | 0/1 | `gate install-hooks` |
| `--max-artifact-mib` | `MIB` |  | 0/3 | `evidence add`, `evidence vault`, `review add` |
| `--model` | `MODEL` |  | 0/1 | `dispatch begin` |
| `--name` | `NAME` |  | 2/2 | `prompt add`, `prompt show` |
| `--no-agents-md` | `-` |  | 0/1 | `init` |
| `--no-policy` | `-` |  | 0/1 | `init` |
| `--no-promote` | `-` |  | 0/4 | `divergence add`, `evidence add`, `finding add`, `finding close` |
| `--note` | `NOTE` |  | 1/2 | `brief approve`, `policy approve` |
| `--open` | `-` |  | 0/1 | `finding list` |
| `--original-spec-ref` | `ORIGINAL_SPEC_REF` |  | 0/1 | `review add` |
| `--out` | `OUT` |  | 0/1 | `export` |
| `--outcome` | `OUTCOME` |  | 0/1 | `review add` |
| `--override-reason` | `OVERRIDE_REASON` |  | 0/2 | `evidence add`, `subject status` |
| `--owner-role` | `OWNER_ROLE` |  | 0/1 | `subject new` |
| `--parent-run` | `PARENT_RUN` |  | 0/1 | `dispatch begin` |
| `--pin` | `PIN` |  | 1/1 | `review add` |
| `--plan` | `-` |  | 0/1 | `purge` |
| `--plugin` | `PLUGIN` |  | 0/1 | `query` |
| `--policy` | `LEVEL` |  | 0/1 | `init` |
| `--port` | `PORT` |  | 0/1 | `dashboard` |
| `--principal` | `PRINCIPAL` |  | 1/1 | `learn approve` |
| `--project-id` | `PROJECT_ID` |  | 5/9 | `divergence accept`, `divergence add`, `divergence defer`, `divergence resolve`, `evidence add`, `evidence approval-payload` +3 |
| `--promote` | `-` |  | 0/4 | `divergence add`, `evidence add`, `finding add`, `finding close` |
| `--prompt-bundle` | `PROMPT_BUNDLE` |  | 0/1 | `dispatch begin` |
| `--provider` | `PROVIDER` |  | 0/1 | `dispatch begin` |
| `--providers` | `LIST` / `PROVIDERS` |  | 0/4 | `init`, `skills check`, `skills doctor`, `skills install` |
| `--reason` | `REASON` |  | 3/6 | `divergence accept`, `divergence add`, `divergence defer`, `finding close`, `learn rollback`, `subject status` |
| `--ref` | `REF` |  | 1/1 | `evidence add` |
| `--regime` | `REGIME` |  | 1/2 | `dispatch begin`, `retrieve` |
| `--release` | `-` |  | 0/1 | `gate check` |
| `--repo` | `REPO` |  | 2/63 | `adapter authorize`, `adapter validate`, `artifacts migrate`, `artifacts verify`, `brief approve`, `brief check` +57 |
| `--report` | `FORMAT:PATH` | repeatable | 0/1 | `evidence add` |
| `--report-case` | `NAME` | repeatable | 0/1 | `evidence add` |
| `--resolution-sha` | `RESOLUTION_SHA` |  | 0/2 | `finding add`, `finding close` |
| `--response` | `RESPONSE` |  | 2/2 | `adapter authorize`, `adapter validate` |
| `--reviewed-sha` | `REVIEWED_SHA` |  | 0/1 | `finding add` |
| `--reviewer` | `REVIEWER` |  | 6/6 | `divergence accept`, `divergence add`, `divergence defer`, `divergence resolve`, `finding add`, `review add` |
| `--reviewer-harness` | `REVIEWER_HARNESS` |  | 0/1 | `review add` |
| `--reviewer-model` | `REVIEWER_MODEL` |  | 0/1 | `review add` |
| `--reviewer-provider` | `REVIEWER_PROVIDER` |  | 0/1 | `review add` |
| `--role` | `ROLE` |  | 6/11 | `dispatch begin`, `divergence accept`, `divergence add`, `divergence defer`, `divergence resolve`, `evidence add` +5 |
| `--round` | `ROUND` |  | 2/5 | `divergence add`, `divergence defer`, `divergence resolve`, `finding add`, `finding close` |
| `--run-id` | `RUN_ID` |  | 1/4 | `dispatch end`, `evidence add`, `evidence approval-payload`, `ingest artifacts` |
| `--scenario-id` | `SCENARIO_ID` / `SCENARIO_IDS` | repeatable | 4/8 | `divergence accept`, `divergence add`, `divergence defer`, `divergence resolve`, `evidence add`, `evidence approval-payload` +2 |
| `--scenario-ids-from` | `FILE\|-` |  | 0/1 | `review add` |
| `--seq` | `SEQ` |  | 2/2 | `finding add`, `finding close` |
| `--session-id` | `SESSION_ID` |  | 0/5 | `divergence accept`, `evidence add`, `finding add`, `finding close`, `review add` |
| `--severity` | `SEVERITY` |  | 1/1 | `finding add` |
| `--sha` | `SHA` |  | 3/3 | `divergence add`, `divergence defer`, `divergence resolve` |
| `--signature` | `SIGNATURE` |  | 0/1 | `learn promote` |
| `--signature-file` | `SIGNATURE_FILE` |  | 1/1 | `learn rollback` |
| `--since` | `SINCE` |  | 0/1 | `query` |
| `--skill-digest` | `SKILL_DIGEST` |  | 0/1 | `dispatch begin` |
| `--skill-id` | `SKILL_ID` |  | 0/1 | `dispatch begin` |
| `--snapshot` | `SNAPSHOT` |  | 0/2 | `dashboard`, `report` |
| `--source` | `SOURCE` |  | 0/4 | `ingest plans`, `skills check`, `skills doctor`, `skills install` |
| `--spec-root` | `SPEC_ROOT` |  | 0/2 | `inventory sync`, `plugin sync` |
| `--status` | `STATUS` |  | 1/3 | `dispatch end`, `divergence add`, `query` |
| `--subject-id` | `SUBJECT_ID` |  | 3/5 | `change new`, `retrieve`, `scenario new`, `subject adopt`, `subject unadopt` |
| `--summary` | `SUMMARY` |  | 1/3 | `evidence add`, `finding add`, `subject new` |
| `--surface-ref` | `SURFACE_REF` | repeatable | 0/2 | `evidence add`, `evidence approval-payload` |
| `--target` | `TARGET` |  | 0/3 | `skills check`, `skills doctor`, `skills install` |
| `--task` | `TASK` |  | 0/3 | `dispatch begin`, `evidence add`, `evidence approval-payload` |
| `--timeout` | `TIMEOUT` |  | 0/1 | `gate install-hooks` |
| `--title` | `TITLE` |  | 3/4 | `change new`, `feature new`, `scenario new`, `subject new` |
| `--to` | `BACKEND` / `STATE` |  | 1/2 | `artifacts migrate`, `subject status` |
| `--tracking` | `TRACKING` |  | 1/1 | `divergence accept` |
| `--upstream-ref` | `UPSTREAM_REF` |  | 0/1 | `review add` |
| `--verdict` | `VERDICT` |  | 0/1 | `evidence add` |
| `--version` | `VERSION` |  | 2/2 | `prompt add`, `prompt show` |
| `--watch` | `-` |  | 0/2 | `ingest artifacts`, `ingest sessions` |
