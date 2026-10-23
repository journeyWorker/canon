# Canon 0.28: the per-leaf command table

Status: normative draft, reviewed together with the 0.28 grammar plan (`docs/superpowers/plans/2026-10-22-canon-cli-grammar.md`). This is the final table of every leaf that plan D1 promises. The spec generator (D2), the skill, the website reference and the "Removed in 0.28" hints are generated from it; an agent must be able to build any command from this table alone.

76 leaves: 29 read, 42 write, 5 destructive. A leaf is a command you can run. Every group requires a subcommand and none is runnable (D2). `help`, `-h/--help` and `-V/--version` are clap built-ins, not leaves.

Rule references: `D1`..`D12` are the plan's decisions (`D1 r1`/`r2`/`r3` are its three operand rules, `D1 Renames` its renames table), `Q1`..`Q9` its owner decisions, `O<n>` the open items at the end of this page (numbers are never reused; the decided ones are listed under "Decided in round 2").

## How to read the tables

One row per leaf. Columns: command id; operand; required options; optional options; exclusive and dependent options; effect with idempotency and roles; output; one runnable example.

**Operands** (one kind per command, context is never an operand, D1): `<X>` exactly one, required; `[<X>]` zero or one; `<X>...` one or more; `[<X>...]` zero or more. `<X>` names the kind (a concept id such as `<CHANGE_ID>`, or a placeholder); `{a,b}` after a placeholder is a closed set.

**Options**: `--name <VALUE>`; `--name <VALUE>=default`; `--name <VALUE>...` repeatable; `--name <VALUE>{a,b}` closed set; bare `--name` is a boolean that names a mode. Every option not named in a row does not exist on that leaf. "Required options" are required unconditionally; conditional requirements are in the exclusive/dependent column. T3 (D1): across leaves an option name keeps one value name, arity, meaning and value set; only `required` and the default may differ per leaf, and a leaf may declare a subset of a shared set (`purge --kind`, `gate promote --kind`).

**Shared, declared once, not repeated in the rows**

| Option | Where | Meaning |
| --- | --- | --- |
| `--repo <DIR>` | every leaf except `schema`, `selftest`, `completion`, `regime show`, `skills show` | D5: absent = nearest ancestor holding `canon.yaml`; present = that directory. A leaf that needs canon state and finds none refuses `CONFIG_MISSING` (exit 2). `init` and `demo init` take the directory to set up as-is (O1) |
| `--as <ROLE>{planner,implementer,reviewer,orchestrator}` | every leaf | The role you act as. Overrides `CANON_ROLE`. Filters `schema` and `context` listings; is the default of attribution `--role` |
| `--json` | every leaf | Output only (D7). Success `{ok: true, data, meta}`, failure `{ok: false, error, meta}`, nothing on stderr (D3). Artifact and stream leaves keep their bytes without `--json` and have an enveloped `--json` form; the output column says which |
| `--input <JSON\|FILE\|->` | every write or destructive leaf (47 leaves: the rows whose effect starts with `write` or `destructive`); no read leaf | D7: the leaf's arguments as one JSON object (keys are option names in camelCase, `operands` an array), or a JSON array / NDJSON batch of them (see Batch output). A document can never carry `--commit` or `--plan-digest`. `--input -` refuses a terminal |
| `--dry-run` | only where a row lists it | D6: one help string, and full validation: it refuses with the same code the real run would (T12). Listed on `init`, `skills install`, `gate install-hooks`, `inventory sync`, `plugin sync`, `artifacts store`, `ingest records`, `ingest plans`, `ingest sessions`, `export`, `subject adopt`, `subject unadopt` (D6) and, kept from 0.27, `gate promote` and `strategy promote` |
| `--commit`, `--plan-digest <DIGEST>` | exactly the five destructive leaves: `purge`, `artifacts move`, `migrate`, `ledger compact`, `tier age` | D6: the bare run previews (exit 0, report with `planDigest` and `effects[]`); `--commit --plan-digest <DIGEST>` acts and refuses (`PLAN_CHANGED`, exit 2) when the plan changed since. Their `--dry-run` is removed. A destructive leaf's report always carries `planDigest`, `effects[]` and `committed` |
| `[paging]` | `query`, `export`, `finding list`, `divergence list` | `--limit <N>=100`, `--cursor <CURSOR>`, `--all` (every record from the cursor on; conflicts `--limit`) |
| `[staged]` | `evidence add`, `evidence audit`, `finding add`, `finding close`, `divergence add` | `--promote` or `--no-promote` (mutually exclusive): promote the record this run staged in the same run, or keep it staged, overriding `canon.yaml` `write.promote` |
| `[bind]` | `evidence add`, `evidence audit` | `--artifact <PATH>...` (a file inside the repo, bound by sha256), `--report <FORMAT:PATH>...` (`junit`, `cucumber` or `swift-testing`), `--report-case <NAME>...`, `--max-artifact-mib <N>=25` (at least 1) |

**Batch output** (D2 `outputs`, D7). Every write leaf whose output is a report (all write leaves but the stream leaf `dashboard`) has a second output variant, `Batch<Name>Report`, returned when `--input` is a JSON array or NDJSON, even with one element; an object, or no `--input`, returns the ordinary `<Name>Report`. `meta.command` is the leaf id in both cases, and the batch variant has its own `meta.schemaVersion` (`"1.0"` in 0.28). `data.items[]` has one entry per input element, in order: `index` (0-based), `status` (`written`, `unchanged`, `failed` or `skipped`), and `data` (that element's ordinary report) or `error` (the ordinary error). Exits: 0 every element `written` or `unchanged`; 1 a write failed (the failing element is `failed`, later ones `skipped`, earlier ones stay written); 2 validation refused, nothing written, `error.field` an RFC 6901 JSON pointer such as `/items/2/scenarioId`. The five destructive leaves take one object only (O16). Batch examples:

| Leaf | Example |
| --- | --- |
| `scenario new` | `canon scenario new --input '[{"operands":["auth.login.01"],"title":"Logs in","case":"happy"},{"operands":["auth.login.02"],"title":"Signs out","case":"happy"}]'` |
| `evidence add` | `canon evidence add --input '[{"scenarioId":"auth.login.01","class":"test-run","ref":"npm test","role":"implementer"},{"scenarioId":"auth.login.02","class":"test-run","ref":"npm test","role":"implementer"}]'` |
| `finding add` | `canon finding add --input '[{"changeId":"add-login","round":1,"seq":1,"severity":"note","actorId":"reviewer-1","summary":"Token is logged"},{"changeId":"add-login","round":1,"seq":2,"severity":"blocker","actorId":"reviewer-1","summary":"No rate limit"}]'` |
| `divergence add` | `canon divergence add --input '[{"operands":["auth.login.01"],"actorId":"reviewer-1","role":"reviewer"},{"operands":["auth.login.02"],"actorId":"reviewer-1","role":"reviewer"}]'` |
| `subject new` | `canon subject new --input '[{"operands":["payments"],"domain":"dev","title":"Payments"},{"operands":["billing"],"domain":"dev","title":"Billing"}]'` |

**Attribution options** (same meaning everywhere they appear): `--actor-id <ACTOR_ID>` is the recording actor; default `canon` (`canon-scaffold`, env `CANON_ACTOR`, on `feature new` and `scenario new`), required where a row lists it under required options, because it then names a reviewer or signer (O9). `--role <ROLE>` (an open slug: owner decision 2026-10-23, policies may require custom roles; a value that is none of planner, implementer, reviewer, orchestrator and no role the policy names still runs and adds a `meta.warnings` line naming the closest known role) means only "the role this record is attributed to": it defaults to the role declared with `--as` / `CANON_ROLE`, and is required when none is declared (a `--role` listed as optional is conditionally required in this sense; `finding add` and `finding close` default to `reviewer`). `dispatch begin --role` is the role the run acts as, so it is attribution and keeps the name; a run's role must equal the regime's first segment. A filter by role is `--for-role` (`retrieve`), and a role inside a regime key is `--role-segment` (`regime show`). `--session-id <SESSION_ID>` is the recording session. `--approver <NAME>` is a human approver, one name on all five leaves that take it (D1 Renames).

**Commits** (D7b): `--rev <REV>` is the commit a record is about; any git revision (`HEAD~1`, a branch, a tag, a short sha), resolved with `git rev-parse --verify <rev>^{commit}`; default `HEAD`. Every leaf that takes `--rev`, `--resolution-rev` or `--introduced-rev` records the full sha, plus `codeTree` and `worktreeDirty` for the code it saw; uncommitted code is recorded, never refused. `codeTree` is computed the same way for the working tree and for a commit: read the files into a temporary index, remove `.canon/`, every configured `specs.roots[]` and the `code.exclude` globs, then `git write-tree`; so editing a `.feature` file or a brief never changes it, and it is never `<rev>^{tree}`. Outside git, or with no commits, a leaf that needs a commit refuses `NO_COMMIT`. `--commit` never names a commit.

**Time**: `<TIMESTAMP>` is RFC3339 or `YYYY-MM-DD` (UTC midnight) on every option (O11).

**Effect** (D2, D6): `read` changes nothing in the tree, except what `effectNote` names (T11); `write` records or rewrites state directly or stages it; `destructive` deletes or rewrites existing data and is preview-by-default. A refusal leaves the tree unchanged (D6, T12).

**Idempotency** (D10, T12): `replay-safe` a repeat is a no-op, reported `unchanged`; `content-keyed` identical content is a no-op (`unchanged`), different content under the same key is refused; `appends` every call adds a record; `once` a repeat is refused with exit 2 and changes nothing (O10).

**Roles**: the 0.27 role list of the leaf the row came from (`P` planner, `I` implementer, `R` reviewer, `O` orchestrator); a split leaf inherits its parent's roles. `human only` leaves (`brief approve`, `policy approve`, `divergence accept`) refuse every agent role with `retryable: false` and a hint addressed to the human; no flag grants them (D10). `--override-reason` is not one of them (D12).

**Output**: `report <Name>` an enveloped report (`schemaVersion` `"1.0"` in 0.28, `meta.command` the id in the first column; `Name` is the PascalCase id plus `Report`, the report schema's name in the spec; keys are camelCase, D3); `artifact <media type>` raw bytes on stdout without `--json`; `stream` a long-running stdout. Every leaf's failure is the enveloped error. Exits follow D4: 0 ok; 1 the leaf's finding, a failed check or `unverifiable`; 2 refused (usage, `CONFIG_MISSING`, an id or file that does not exist, unreadable input, invalid argument). A row names the leaf's exit 1 where it has one; `data` carries the report on exit 1 (D3). A generated command (`meta.next[].argv`, hints, skill text) never contains an ellipsis or an unfilled placeholder (T14).

**Value names** come from D1's placeholder list (`<DIR> <FILE> <PATH> <ID> <SHA> <N> <TIMESTAMP> <NAME> <TEXT> <URL> <KIND> <FIELDS>`), the concept ids, and the closed sets shown. Names this page adds beyond that list (`<REV>`, `<DIGEST>`, `<LIST>`, `<TAG>`, `<LEVEL>`, `<TIER>`, `<STATE>`, `<SHELL>`, `<KEY>`, `<CURSOR>`, `<CLASS>`, `<CODE>`, `<SEVERITY>`, `<DISPOSITION>`, `<RUN_OUTCOME>`, `<REVIEW_OUTCOME>`, `<RESULT>`, `<VERDICT>`, `<ROLE>`, `<CHANGE_KIND>`, `<FLOW_ID>`, `<PATH_OR_EFFECT>`, `<NAME@VERSION>`) go into `cli-vocabulary.tsv`.

## Leaves

### Top-level leaves

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon init` | — | — | `--check`; `--no-agents-md`; `--policy <LEVEL>{light,standard,strict}=strict`; `--no-policy`; `--providers <LIST>` (claude,codex,omp,pi comma list, or `none`); `--language <TAG>`; `--dry-run` | `--check` conflicts only with init's mutating options (`--no-agents-md`, `--policy`, `--no-policy`, `--providers`, `--language`, `--dry-run`); `--json`, `--as` and `--repo` stay allowed; `--policy` conflicts `--no-policy`. `--repo` here is the directory to set up: used as-is, default the current directory, no ancestor walk (O1). Prompts for providers only when stdin and stderr are terminals and `--providers` is absent (D6) | write; replay-safe (a rerun refreshes the AGENTS.md block and missing `.gitignore` lines only); `--check` writes nothing (effectNote); roles O | report `InitReport` (`data.steps[]`); with `--check` `InitCheckReport` (`data.sections[]`), exit 1 = a failed section | `canon init --check --json` |
| `canon migrate` | — | — | — | Preview by default; acts only with `--commit --plan-digest` (D6). Moves spec roots (`git mv`), rewrites `canon.yaml`, canon-owned hooks and scripts, the ledger layout and pins; backs up every rewritten file under `.canon/migrate-backup/<timestamp>/` (D9). Refused on uncommitted changes under a path it moves. Recovery from a 0.27 repo is two runnable steps (D9): `status`, `gate check` and `context` put `canon migrate --json` first in `meta.next`; the preview's own `meta.next[].argv` is `canon migrate --commit --plan-digest <the actual digest> --json`, with the digest filled in | destructive; replay-safe (a second run finds nothing to move; a stale digest is `PLAN_CHANGED`, exit 2); roles O | report `MigrateReport` (`planDigest`, `effects[]`, `committed`); exit 1 = the index rebuild finds the corpus unclean | `canon migrate` |
| `canon status` | — | — | `--fields <FIELDS>` (comma list of top-level report fields) | Unknown field is `UNKNOWN_FIELD`, exit 2. `--as` filters `meta.next` and fills `waitingOn` | read; the stale scenario index is graded in memory, nothing is written; roles P I R O | report `StatusReport`; the 0.27 top-level `next` becomes `meta.next` (D3, D9). Exits 0 whatever it finds; `unverifiable` (exit 1) only when nothing was examined (D3 coverage) | `canon status` |
| `canon context` | — | — | `--fields <FIELDS>` (comma list of `capabilityVersion`, `kinds`, `families`, `commands`, `enums`, `joinKeys`, `policy`, `vocab`, `review`, `cel`) | Unknown field is `UNKNOWN_FIELD`, exit 2, in text mode too. Needs no `canon.yaml`. Never filtered or failed by `CANON_ROLE` (D3); `commands` is filtered only by `--as` (D2) | read; roles P I R O | report `ContextReport` (the authoring surface; kind name is `evidence`, Q6) | `canon context --fields kinds,commands` |
| `canon format` | `[<DIR>]` — 0 or 1; default: every configured `specs.roots[]` entry (D12) | — | — | A relative `<DIR>` resolves under the repo root when `--repo` is given, else the current directory. The no-op `--check` and the `fmt` alias are gone (D8, D12); format keeps its name and behavior otherwise | read; roles P I R O | report `FormatReport`; exit 1 = violations, or a corpus with 0 files (`unverifiable`, D3 coverage); exit 2 = `<DIR>` is not a directory | `canon format` |
| `canon retrieve` | — | `--for-role <ROLE>` | `--regime <KEY>`; `--domain <NAME>`; `--subject-id <SUBJECT_ID>`; `--limit <N>=5` | Exactly one scope: `--regime`, or `--domain` with optional `--subject-id` (`--subject-id` requires `--domain`; `--regime` conflicts both). With `--regime`, `--for-role` must equal the key's first segment. `--role` became `--for-role` because `--role` means only attribution (D12). `--limit` replaces `-k/--k` (D1 Renames) | read; roles P I R O | report `RetrieveReport`; `data` is the array of guidance items, so the installed hook reads `(.data // .)` (D3, D9); `[]` when none | `canon retrieve --for-role reviewer --domain dev --limit 20` |
| `canon query` | — | `--kind <KIND>{change,task,scenario,session,run,event,handoff,review,divergence,trajectory,strategy_item,evidence,subject,finding}` | `--since <TIMESTAMP>`; `--plugin-id <PLUGIN_ID>`; `--history`; `--change-id <CHANGE_ID>`; `--status <STATUS>`; `--domain <NAME>`; `--include-sensitive`; `[paging]`; `--fields <FIELDS>` | `--history` conflicts `--plugin-id`; `--change-id` only with kind `change` or `task`; `--status` only with `change`, `task` or `subject` (value must be in that kind's domain); `--domain` only with `subject`; each violation exit 2. `--canon-yaml` is gone (O2) | read; effectNote: appends one line to `.canon/audit/access.jsonl` (O7); roles P I R O | report `QueryReport` (records verbatim, paging in `meta`) | `canon query --kind evidence --since 2026-10-01 --limit 20` |
| `canon export` | — | `--kind <KIND>` (same 14 values as `query`) | `--before <TIMESTAMP>` (strictly before); `--after <TIMESTAMP>` (at or after); `-o, --output <FILE>`; `--include-sensitive`; `[paging]`; `--fields <FIELDS>`; `--dry-run` | `--fields` takes a comma list of `kind`, `at`, `id`, `digest`. `--dry-run` writes no `--output` file. `--out` is now `-o/--output` (D1 Renames) | write (the `--output` file, mode 0600, and an audit line, effectNote); replay-safe; roles O | artifact `application/json` (the `canon.retention.export` manifest) on stdout, or the `--output` file with empty stdout; `--json` envelopes it: `ExportReport` (`data` = manifest, or `{output, kind, recordCount}` with `--output`) (D3) | `canon export --kind evidence --output evidence.json` |
| `canon purge` | — | `--kind <KIND>{session,run,event,trajectory}`; `--before <TIMESTAMP>` | — | Preview by default: the bare run prints `planDigest` and `effects[]`, exit 0. Acts only with `--commit --plan-digest <DIGEST>`, and only when the plan recomputed now has that digest (`PLAN_CHANGED`, exit 2). The target is named by `--kind` and `--before`, both required (D6). `--plan`/`--confirm` are gone. `--kind` lists only the four kinds purge accepts (Q1). `trajectory` needs the configured learn root to exist | destructive; replay-safe; effectNote: audit line; roles O | report `PurgeReport` (`planDigest`, `effects[]`, `committed`; per-rung `matched`/`deleted`) | `canon purge --kind session --before 2026-01-01` |
| `canon dashboard` | — | — | `--snapshot <DIR>`; `--port <N>=4173` (0 = OS-assigned) | Binds 127.0.0.1 only. `--snapshot` is now only this command's option (`report --snapshot` became `report snapshot <DIR>`) | write (regenerates `.canon/dashboard-snapshot`; `POST /api/briefs/approve` rewrites a brief, effectNote); long-running; replay-safe; roles O | stream: with `--json`, one `DashboardReport` envelope (`url`, `briefsUrl`, `app`, `snapshot`, `snapshotAvailable`), then the server runs until a signal | `canon dashboard --port 0` |
| `canon selftest` | — | — | `--suite <NAME>{trust-spine-gate,tiered-storage,typed-authoring-vocabulary,policy-expressions,format-authority,session-ingest,canon-context,artifact-ingest,spec-ledger-selftest,plugin-overlays,plan-import}` | Default: every suite. `gate selftest` is `selftest --suite trust-spine-gate` (D1 Renames). Needs no repo | read; effectNote: scratch directories in the system temp dir, removed on exit; roles O | report `SelftestReport` (`clean`, `suites[]`); exit 1 = a failed suite | `canon selftest --suite trust-spine-gate` |
| `canon schema` | `[<PATH>...]` — zero or more command-path words; none lists every command | — | `--errors`; `--error <CODE>` | `--errors`, `--error` and the operand conflict with each other. `--errors` lists every error code with meaning, exit, `retryable` and the fix; `--error <CODE>` shows one (D3). Unfiltered; `--as <ROLE>` filters (D1 Renames, D3). `--all` is gone: the bare listing is already complete. Never fails or filters on `CANON_ROLE`. Needs no repo or network. An unknown path is `UNKNOWN_COMMAND`, exit 2, with the nearest matches | read; roles P I R O | report `SchemaReport` (commands, or one command's node from the generated spec incl. `inputs[]` schemas and examples, or `errors` with `--errors`, or one error code with `--error`) | `canon schema --error CONFIG_MISSING` |
| `canon completion` | `<SHELL>{bash,zsh,fish}` — 1, required | — | — | Reads the embedded spec; no repo (D2) | read; roles P I R O | artifact `text/x-shellscript` on stdout; `--json` envelopes `{shell, script}` | `canon completion zsh` |

### `policy`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon policy approve` | — | `--approver <NAME>` | `--note <TEXT>` | Human-only (D10): refuses the agent roles, `retryable: false`, hint addressed to the human. Writes `.canon/policy.approval.json` (the policy sha256, approver, time, note) | write; replay-safe (re-approving rewrites the one approval file); human only (D10) | report `PolicyApproveReport` | `canon policy approve --approver alice --note "reviewed the starter policy"` |
| `canon policy show` | — | — | — | — | read; roles O | report `PolicyShowReport` (digest and approval: who, when, which bytes) | `canon policy show` |

### `demo`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon demo init` | — | — | — | `--repo` is the directory to scaffold: used as-is, default the current directory (O1). Refused with `INIT_CONFIG_EXISTS` (exit 2) when `canon.yaml` exists | write; once; scaffolds a repo whose gate starts RED; roles O | report `DemoInitReport` (`repo`, `scenario`) | `canon demo init --repo canon-demo` |
| `canon demo review` | — | — | — | Needs a repo made by `demo init` (`DEMO_NOT_INITIALIZED`, exit 2). Renamed from `demo add` (D1 Renames): it records the reviewer's review, the gate turns GREEN | write; appends (a new record every run); roles O | report `DemoReviewReport` | `canon demo review` |

### `context-pack`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon context-pack add` | — | `--manifest <FILE>` (repository-relative JSON, `manifestVersion: 1`) | — | Renamed from `context-pack new` (D1 Renames; `new` is for authored units). The manifest is an input file, not an instance of the noun, so it stays an option (D1 r1). Needs git (`CONTEXT_GIT_UNAVAILABLE`) | write; content-keyed (same content is a no-op; different bytes under one digest are `CONTEXT_OBJECT_CONFLICT`); roles O I | report `ContextPackAddReport` (the complete pack manifest); text mode prints the pack id `sha256:<hex>` | `canon context-pack add --manifest .canon/context-manifest.json` |
| `canon context-pack show` | `<PACK_ID>` — 1, required, `sha256:<64 hex>` (D1 r1) | — | — | Verifies the manifest and objects first; a tampered pack is `CONTEXT_PACK_TAMPERED`, exit 1 | read; roles O I | report `ContextPackShowReport` (the pack manifest) | `canon context-pack show sha256:0000000000000000000000000000000000000000000000000000000000000000` |
| `canon context-pack verify` | `<PACK_ID>` — 1, required (D1 r1) | — | — | — | read; roles O I | report `ContextPackVerifyReport` (`id`, `verified`); exit 1 = tampered | `canon context-pack verify sha256:0000000000000000000000000000000000000000000000000000000000000000` |

### `adapter`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon adapter authorize` | — | `--response <FILE>` | — | `--repo` is now optional with discovery (0.27: required, literal; D5). Reads `.canon/policy.yaml`; an absent policy means default denial. Never executes anything | read; roles O | report `AdapterAuthorizeReport` (`executionAuthorized`, `executionPerformed`, `sandboxEnforced`, `reasons[]`, `capabilities`); exit 1 = not authorized, report kept in `data` (D3) | `canon adapter authorize --response adapter-response.json` |
| `canon adapter validate` | — | `--response <FILE>` | — | Context-pack join verification runs when a `canon.yaml` is found (given or discovered), and is skipped otherwise; no `CONFIG_MISSING` (D5: this leaf needs no canon state) | read; roles O | report `AdapterValidateReport` (the normalized summary); exit 1 = invalid envelope; exit 2 = file missing or unreadable (D4) | `canon adapter validate --response adapter-response.json` |

### `prompt`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon prompt add` | `<NAME@VERSION>` — 1, required (D1 r1) | `--manifest <FILE>` (repository-relative JSON) | — | `--name`/`--version` are gone: the operand carries both, matching `dispatch begin --prompt-bundle` (D12). Labels may not be empty, `.`/`..`, or contain `/`, `\` or NUL | write; content-keyed (same content is a no-op; different content under one `name@version` is `CONTEXT_OBJECT_CONFLICT`, exit 1); roles O | report `PromptAddReport` (`name`, `version`, `digest`, `prompts`) | `canon prompt add reviewer@v1 --manifest .canon/prompt-manifest.json` |
| `canon prompt show` | `<NAME@VERSION>` — 1, required (D1 r1) | — | — | Re-verifies the digest and file snapshots; unregistered is `PROMPT_BUNDLE_INVALID`, exit 2 | read; roles O | report `PromptShowReport` (the bundle) | `canon prompt show reviewer@v1` |

### `feature`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon feature new` | `<SURFACE>` — 1, required, `<area>.<surface>` (D1 r1) | `--title <TEXT>` | `--actor-id <ACTOR_ID>=canon-scaffold` (env `CANON_ACTOR`); `--project-id <PROJECT_ID>` | `--project-id` is needed only when the repo configures more than one spec root (D1). Creates `<root>/kind=feature/area=<area>/<surface>.feature` with zero scenarios | write; once (`FEATURE_EXISTS`, exit 2); roles P I | report `FeatureNewReport` (`feature`) | `canon feature new billing.invoice --title Invoices` |

### `scenario`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon scenario new` | `<SCENARIO_ID>` — 1, required, `<area>.<surface>.<nn>` (D1 r1; was `<TAG>`, one leading `@` still accepted) | `--title <TEXT>` | `--subject-id <SUBJECT_ID>`; `--lane <NAME>`; `--case <NAME>`; `--flow <FLOW_ID>`; `--change-id <CHANGE_ID>`; `--actor-id <ACTOR_ID>=canon-scaffold` (env `CANON_ACTOR`); `--feature <FILE>`; `--project-id <PROJECT_ID>` | `--change-id` requires `--flow`. `--flow` writes `@flow:<change>.<id>`; the change is `--change-id`, else the newest change of `--subject-id`, else the only live change. `--lane`/`--case` must be in the repo's activated vocabulary. `--feature` must resolve under a spec root. `--from` (NDJSON) is gone: a batch is `--input` with `operands` per element (D7) | write; once (`SCENARIO_TAG_EXISTS`, exit 2). Writes only the `.feature` file; roles P I | report `ScenarioNewReport` (`scenarioId`, `feature`) | `canon scenario new auth.login.02 --title "Sign out" --case happy` |

### `subject`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon subject new` | `<SUBJECT_ID>` — 1, required, kebab slug (D1 r1) | `--domain <NAME>`; `--title <TEXT>` | `--summary <TEXT>`; `--owner-role <ROLE>=implementer`; `--actor-id <ACTOR_ID>=canon` | `--domain` must be in the repo's activated `domain` enum when one is declared. Creates the subject at status `proposed` | write; once (`SUBJECT_EXISTS`, exit 2); roles P O | report `SubjectNewReport` (embeds the Subject record verbatim) | `canon subject new payments --domain dev --title Payments` |
| `canon subject show` | `<SUBJECT_ID>` — 1, required (D1 r1) | — | — | Read half of the 0.27 `subject status` (D1 Renames: a noun such as `status` stays read-only). `--override-reason`, `--reason`, `--actor-id` do not exist here | read; roles O | report `SubjectShowReport` (embeds the Subject record verbatim); `SUBJECT_NOT_FOUND` exit 2 | `canon subject show payments` |
| `canon subject move` | `<SUBJECT_ID>` — 1, required (D1 r1) | `--to <STATE>{proposed,specced,building,verifying,shipped,retired}` | `--reason <TEXT>`; `--override-reason <TEXT>`; `--actor-id <ACTOR_ID>=canon` | Valid steps: forward `proposed→specced→building→verifying→shipped`, `shipped→verifying` (needs `--reason`), any live state `→retired`; anything else `SUBJECT_INVALID_TRANSITION`, exit 2. Shipping and reopening are gated: policy changed since approval, evidence, brief and chore-review gaps refuse with exit 1 (`SUBJECT_GATE_VIOLATION`); only the `require_review` gaps (`unreviewed-promotion`, `open-blocker`) and the brief gap can be waived. `--override-reason` is available to every role (D12): it is recorded on the subject, echoed in `data.overrides[]` and listed by `status` | write; once (a move to the current state is refused, exit 2); effectNote: refreshes stale scenario-index records first; roles O | report `SubjectMoveReport` (embeds the Subject record verbatim, `meta.next`) | `canon subject move payments --to specced` |
| `canon subject adopt` | `<SUBJECT_ID>` — 1, required (D1 r1: the group noun wins) | `--change-id <CHANGE_ID>` | `--dry-run` | The change must already be imported (`SUBJECT_ADOPT_REFUSED`, exit 2); a change may belong to several subjects | write; replay-safe (an already-linked pair writes nothing, `unchanged`); roles P O | report `SubjectAdoptReport` | `canon subject adopt payments --change-id add-search` |
| `canon subject unadopt` | `<SUBJECT_ID>` — 1, required (D1 r1) | `--change-id <CHANGE_ID>` | `--dry-run` | Refused (`SUBJECT_UNADOPT_REFUSED`, exit 2) when the change's brief carries an `Approved:` line or the subject owns a scenario tagged `@flow:<change>.*` | write; replay-safe (a pair that is not linked writes nothing); roles P O | report `SubjectUnadoptReport` | `canon subject unadopt payments --change-id add-search` |

### `change`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon change new` | `<CHANGE_ID>` — 1, required, slug (D1 r1: it creates the change) | `--subject-id <SUBJECT_ID>`; `--title <TEXT>` | `--change-kind <CHANGE_KIND>{feature,chore}=feature` | Writes `.canon/changes/<id>/brief.md`, records the change and adopts it into the subject. `--change-kind` is the brief template; `--kind` is reserved for record kinds (D12). On a shipped subject it adds a reopen step | write; once (`CHANGE_EXISTS`, exit 2); roles P | report `ChangeNewReport` (`changeId`, `subjectId`, `brief`) | `canon change new add-search --subject-id payments --title "Add search"` |

### `brief`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon brief show` | `[<CHANGE_ID>]` — 0 or 1; default: the repo's only live change (D1 r2) | — | — | Replaces bare `canon brief` (D1 Renames; a group is never runnable, D2). No live change and none named is `NO_BRIEF`, exit 2 | read; roles P R | report `BriefShowReport` (state and the next one to three steps in `meta.next`) | `canon brief show add-search` |
| `canon brief check` | `[<CHANGE_ID>]` — 0 or 1; default: the only live change (D1 r2) | — | — | Brief rules: blockers (structure) and advisories | read; roles P R | report `BriefCheckReport`; exit 1 = blockers (flipped from 0.27's 2, D4), exit 2 = refused (no such change) | `canon brief check add-search` |
| `canon brief approve` | `[<CHANGE_ID>]` — 0 or 1; default: the only live change (D1 r2) | `--approver <NAME>`; `--note <TEXT>` (the human's own words) | — | Human-only (D10). Refused while `brief check` has blockers. Writes `Approved: <who> <when> — "<words>" (brief sha256:<digest>)` into the brief and records the policy approval | write; replay-safe (re-approving replaces the one `Approved:` line); human only (D10) | report `BriefApproveReport` | `canon brief approve add-search --approver alice --note "approved as written"` |

### `inventory`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon inventory sync` | — | — | `--spec-root <DIR>`; `--dry-run` | `--spec-root` overrides `specs.roots[]` entirely: one ad hoc root with the id `root`; a relative path resolves under the repo root | write; replay-safe (an unchanged corpus writes nothing); roles I O | report `InventorySyncReport`; exit 1 = violations or duplicates in a root (`INVENTORY_ABORTED`, nothing written for it) | `canon inventory sync` |

### `gate`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon gate check` | — | — | `--release` | `--release` adds the release-scoped trust check, never drops one | read; roles P I R O | report `GateCheckReport` (`clean`, `violations[]`, `advisories[]`); exit 1 = violations or `unverifiable` (nothing examined, `meta.coverage`, D3); exit 2 = `CONFIG_MISSING`/unreadable | `canon gate check --release` |
| `canon gate install-hooks` | — | — | `--event <NAME>=PreToolUse`; `--matcher <TEXT>`; `--command <TEXT>=canon task done`; `--timeout <N>=30`; `--dry-run` | The default hook command follows the rename (`gate task` → `task done`, D9). Merges one hook into both `.claude/settings.json` and `.codex/hooks.json`, and writes `.canon/scripts/canon-gate-pre-commit.sh` when no canon hook existed | write; replay-safe (`no diff, nothing written`); roles O | report `GateInstallHooksReport` (`installed`, `preCommitScriptWritten`) | `canon gate install-hooks --event PreToolUse` |
| `canon gate promote` | — | — | `--kind <KIND>{evidence,finding,divergence}`; `--dry-run` | Default: drains every staged kind. `--kind divergence` is the old `divergence promote` (D1 Renames; the three staged kinds are the ones `evidence add`, `finding add` and `finding close` and `divergence add` stage; `review` never stages). `--kind` here is a closed three-value subset of the record kinds | write; replay-safe (an already-committed staging id is `recovered`); roles I | report `GatePromoteReport` (`dryRun`, `promoted[]`, `recovered[]`, `refused[]`); exit 1 = `PROMOTE_REFUSED`, report kept in `data` | `canon gate promote --kind evidence` |

### `task`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon task done` | `<TASK_ID>` — 1, required, `<change_id>#<n>` (D1 r1) | — | — | Flips the checkbox only when committed evidence supports it (fails closed); staged evidence does not count. Replaces `gate task` (D1 Renames) | write; replay-safe (an already `[x]` row is an exit-0 no-op, `unchanged`); roles I | report `TaskDoneReport` (`taskId`, `flipped`, `document`); exit 1 = `GATE_TASK_BLOCKED`, the report stays in `data` (D4) | `canon task done add-search#1` |

### `evidence`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon evidence add` | —  (D1 r3: it can target a task, a scenario, or every scenario) | `--class <CLASS>` and `--ref <TEXT>`, each unless a `--report` is given (D12) | `--class <CLASS>` (free text: `test-run`, `review`; `audit` is refused with the hint `evidence audit`; default `test-run` when a `--report` is given); `--ref <TEXT>` (default the `--report` paths, comma-separated, when one is given); `--task-id <TASK_ID>`; `--scenario-id <SCENARIO_ID>`; `--all-scenarios`; `--project-id <PROJECT_ID>`; `--verdict <VERDICT>{faithful,not-applicable,divergent}`; `--summary <TEXT>`; `--command-result <TEXT>`; `--run-id <RUN_ID>`; `--surface-ref <PATH_OR_EFFECT>...`; `--actor-id <ACTOR_ID>=canon`; `--role <ROLE>`; `--session-id <SESSION_ID>`; `--rev <REV>=HEAD`; `--approver <NAME>`; `--approval-role <ROLE>{human}`; `--signature-file <FILE>`; `--approval-at <TIMESTAMP>`; `[bind]`; `--override-reason <TEXT>`; `[staged]` | requiredOneOf: `--task-id`, `--scenario-id`, `--all-scenarios` (the first two may be given together). `--all-scenarios` conflicts `--scenario-id`, `--task-id`, `--report-case`, and the four approval options; it requires at least one `--report` and `--project-id` (only with several spec roots, D1); `--artifact` is then only `<scenario-id>=<path>`. `--project-id` is needed with `--scenario-id` only when several spec roots exist (D1). A bound report decides the verdict (D11). One scenario: `canon evidence add --scenario-id auth.login.01 --class test-run --ref "npm test" --role implementer`. `--command-result` requires `--summary`. Approval set: `--approver`, `--approval-role`, `--signature-file`, `--approval-at` all or none (`--rev` is then always set, default HEAD). `--override-reason <TEXT>` stays available to every role (D12): a waived refusal is recorded on the record, echoed in `data.overrides[]` and listed by `status` | write; content-keyed (an identical repeat is `unchanged`; `--all-scenarios` reports it per scenario); effectNote: refreshes stale scenario-index records first; roles I | report `EvidenceAddReport` (`records[]`: staged/committed location, subject, outcome, `unchanged`); exit 1 = refused by the gate (fabrication marker, invalid atom, contradicting report, unapproved brief) or `EVIDENCE_BATCH_REFUSED`; nothing is staged | `canon evidence add --all-scenarios --report junit:a.xml --report junit:b.xml --role implementer` |
| `canon evidence audit` | `<SCENARIO_ID>` — 1, required (D1 r2) | `--ref <TEXT>`; `--evaluator-id <ID>`; `--evaluator-version <TEXT>`; `--evaluator-digest <DIGEST>` (64 lowercase hex); `--audit-result <RESULT>{confirmed,refuted,indeterminate}` | `--project-id <PROJECT_ID>`; `--summary <TEXT>`; `--command-result <TEXT>`; `--run-id <RUN_ID>`; `--actor-id <ACTOR_ID>=canon`; `--role <ROLE>`; `--session-id <SESSION_ID>`; `--rev <REV>=HEAD`; `[bind]`; `--override-reason <TEXT>`; `[staged]` | No `--class` (the class is `audit`), no `--verdict` (derived: confirmed→faithful, refuted→divergent, indeterminate→not-applicable), no `--task-id`, `--surface-ref`, approval options or `--all-scenarios` (D1 Renames, Q3). `--command-result` requires `--summary`. `--override-reason` is available to every role and echoed in `data.overrides[]` (D12) | write; content-keyed; effectNote: refreshes stale scenario-index records first; roles I | report `EvidenceAuditReport` (same shape as `EvidenceAddReport`) | `canon evidence audit auth.login.01 --ref audit-report.json --evaluator-id eval-1 --evaluator-version 2.1 --evaluator-digest 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef --audit-result confirmed` |
| `canon evidence approval-payload` | —  (D1 r3: task or scenario) | `--approver <NAME>`; `--approval-at <TIMESTAMP>` | `--task-id <TASK_ID>`; `--scenario-id <SCENARIO_ID>`; `--project-id <PROJECT_ID>`; `--run-id <RUN_ID>`; `--surface-ref <PATH_OR_EFFECT>...`; `--rev <REV>=HEAD` | requiredOneOf: `--task-id`, `--scenario-id`; with both, the task id names the payload subject. `--project-id` is needed with `--scenario-id` only when several spec roots exist. The bytes are what an external `ssh-keygen -Y sign -n canon-approval-v1` signs | read; replay-safe (identical arguments give identical bytes); roles I O | artifact `application/octet-stream` (the payload bytes, no trailing newline) on stdout; `--json` envelopes `EvidenceApprovalPayloadReport` (`payloadHex`, `payload`, `surface`, `effects`) (D3) | `canon evidence approval-payload --scenario-id auth.login.01 --approver alice --approval-at 2030-01-01T00:00:00Z` |

### `artifacts`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon artifacts verify` | — | — | `--full` | `--full` downloads and re-hashes every blob. An unusable or unreachable store is refused (exit 2), never a pass | read; roles O | report `ArtifactsVerifyReport` (`backend`, `bound`, `stored`, `workingTreeOnly`, `problems[]`); exit 1 = a bound file is missing or altered | `canon artifacts verify --full` |
| `canon artifacts store` | — | — | `--max-artifact-mib <N>=25` (at least 1); `--dry-run` | Copies working-tree files whose hash still matches into `.canon/artifacts/sha256/` (or uploads to the object store); files over the limit, outside the repo or missing are listed in `notStored`. Renamed from `evidence vault` (D1 Renames). Runs `refresh_stale_index` first, so its effectNote names the index | write; replay-safe (everything counts as `alreadyStored`); effectNote: refreshes stale scenario-index records first; roles O | report `ArtifactsStoreReport` (`boundFiles`, `stored[]`, `alreadyStored`, `notStored[]`); exit 0 whenever the pass ran | `canon artifacts store --dry-run` |
| `canon artifacts move` | — | `--tier <TIER>` (the name of an `s3` tier in `canon.yaml`, e.g. `cold`) | — | Preview by default; acts only with `--commit --plan-digest` (D6). Uploads every local blob, reads each back, then removes the local copies, adds `artifact_store:` to `canon.yaml` and `.canon/artifacts/` to `.gitignore`. Moving back to local is refused (exit 2); `--tier local` on a local repo is a no-op. `--dry-run` is gone: the bare run is the preview (D6). Renamed from `artifacts migrate --to` (D1 Renames) | destructive; replay-safe (resumable); roles O | report `ArtifactsMoveReport` (`planDigest`, `effects[]`, `committed`, `blobs`, `uploaded`, `alreadyInStore`, `removedLocal`) | `canon artifacts move --tier cold` |

### `finding`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon finding add` | —  (D1 r3: findings are filtered or created by change, round, seq) | `--change-id <CHANGE_ID>`; `--round <N>` (at least 1); `--seq <N>` (at least 1); `--severity <SEVERITY>{blocker,should-fix,note}`; `--actor-id <ACTOR_ID>` (the reviewer who raised it); `--summary <TEXT>` | `--disposition <DISPOSITION>{open,fixed,rejected,deferred}=open`; `--rev <REV>=HEAD` (the commit the round reviewed); `--resolution-rev <REV>` (default HEAD when fixed); `--introduced-rev <REV>` (no default); `--file-ref <TEXT>` (`path:120-134`); `--scenario-id <SCENARIO_ID>...`; `--role <ROLE>=reviewer`; `--session-id <SESSION_ID>`; `[staged]` | `--disposition fixed` requires `--resolution-rev` (default HEAD satisfies it); only `fixed` may carry one. `--introduced-rev` has no default: it must be sourced, never guessed (D7b). `--reviewer` is gone: `--actor-id` is the reviewer (D1 Renames). Each `--scenario-id` must be indexed. Line breaks are refused in text options | write; content-keyed (an identical repeat is `unchanged`; a different finding under an occupied key is `FINDING_SEQ_OCCUPIED`, exit 2; D10); roles R | report `FindingAddReport` (`staged`, `committed`, `changeId`, `round`, `seq`, `severity`, `disposition`, `unsourced`) | `canon finding add --change-id add-login --round 1 --seq 2 --severity should-fix --actor-id reviewer-1 --summary "Token is logged"` |
| `canon finding close` | `<CHANGE_ID>` — 1, required (D1 r2; with `--round` and `--seq` it names the finding) | `--round <N>`; `--seq <N>`; `--disposition <DISPOSITION>{open,fixed,rejected,deferred}` | `--resolution-rev <REV>` (default HEAD when fixed); `--reason <TEXT>`; `--actor-id <ACTOR_ID>=canon`; `--role <ROLE>=reviewer`; `--session-id <SESSION_ID>`; `[staged]` | `fixed` requires `--resolution-rev`, only `fixed` may carry one; `rejected` and `deferred` require `--reason` (`FINDING_REASON_REQUIRED`). `--disposition open` reopens. Changes only disposition, resolution commit and reason. The finding must be committed (`FINDING_NOT_FOUND`, exit 2; still staged is `FINDING_STAGED`) | write; replay-safe (the same disposition again is `unchanged`; D10); roles R | report `FindingCloseReport` (`staged`, `committed`, `changeId`, `round`, `seq`, `from`, `disposition`, `unchanged`) | `canon finding close add-login --round 1 --seq 1 --disposition fixed` |
| `canon finding list` | —  (D1 r3: it filters a collection) | `--change-id <CHANGE_ID>` | `--open`; `[paging]`; `--fields <FIELDS>` (comma list of finding field names, camelCase; `canon schema finding list` lists them) | Unknown field is `UNKNOWN_FIELD`, exit 2 | read; roles P I R O | report `FindingListReport` (`changeId`, `findings[]`, paging in `meta`) | `canon finding list --change-id add-login --open` |

### `review`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon review scenario` | `<SCENARIO_ID>...` — 1 or more, all or nothing (D1 r2) | `--actor-id <ACTOR_ID>` (the reviewer); `--upstream-ref <TEXT>` or `--original-spec-ref <TEXT>` (exactly one) | `--project-id <PROJECT_ID>`; `--rev <REV>=HEAD`; `--role <ROLE>`; `--session-id <SESSION_ID>`; `--review-outcome <REVIEW_OUTCOME>{signed,indeterminate,unavailable}=signed`; `--reviewer-provider <NAME>`; `--reviewer-model <NAME>`; `--reviewer-harness <NAME>`; `--artifact <PATH>...`; `--max-artifact-mib <N>=25` | The two provenance options are mutually exclusive and one is required. Each scenario must be indexed (`REVIEW_SCENARIO_NOT_INDEXED` refuses the whole batch). Pins the scenario digest and what its latest evidence says. `--reviewer` is gone (`--actor-id` is the reviewer) and `--pin` became `--rev` (D1 Renames, D7b); `--scenario-ids-from` is gone (`--input` batch, D7). `--review-outcome` is the 0.27 `--outcome` (D12). Only `signed` counts; the others are recorded as review gaps. `--project-id` only with several spec roots (D1) | write; content-keyed (an identical repeat is `unchanged`); effectNote: refreshes stale scenario-index records first. Written directly, nothing to promote; roles R | report `ReviewScenarioReport` (`reviews[]`: location, `scenarioId`, `unchanged`) | `canon review scenario auth.login.01 auth.login.02 --actor-id reviewer-1 --original-spec-ref docs/prd.md` |
| `canon review change` | `<CHANGE_ID>` — 1, required (D1 r2) | `--actor-id <ACTOR_ID>` (the reviewer); `--upstream-ref <TEXT>` or `--original-spec-ref <TEXT>` (exactly one) | `--project-id <PROJECT_ID>`; `--rev <REV>=HEAD`; `--role <ROLE>`; `--session-id <SESSION_ID>`; `--review-outcome <REVIEW_OUTCOME>{signed,indeterminate,unavailable}=signed`; `--reviewer-provider <NAME>`; `--reviewer-model <NAME>`; `--reviewer-harness <NAME>`; `--artifact <PATH>...`; `--max-artifact-mib <N>=25` | Reviews a chore change (`REVIEW_INVALID_ARGUMENT` otherwise): pins the chore's brief, so an edit to the brief stales it. A chore needs exactly this one review before `subject move --to shipped`. `--project-id` stays (part of the record key) and is needed only with several spec roots (D1) | write; content-keyed; roles R | report `ReviewChangeReport` (same shape as `ReviewScenarioReport`) | `canon review change add-search --actor-id reviewer-1 --upstream-ref https://example.com/issues/42` |

### `divergence`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon divergence add` | `<SCENARIO_ID>` — 1, required (D1 r2) | `--actor-id <ACTOR_ID>` (the reviewer) | `--project-id <PROJECT_ID>`; `--rev <REV>=HEAD`; `--state <STATE>{open,still-divergent,resolved,deferred}=open`; `--reason <TEXT>`; `--expiry <TIMESTAMP>`; `--round <N>=1`; `--detail <TEXT>`; `--role <ROLE>`; `[staged]` | `--state deferred` requires `--reason` and `--expiry`; with any other state they are ignored (as in 0.27). `--reviewer` is gone (`--actor-id`), `--sha` became `--rev` (D1 Renames, D7b), `--status` became `--state`. `--role` is now defaulted from `--as` like every attribution role (was required). `--project-id` only with several spec roots (D1) | write (staged, `run_seq` assigned by `gate promote --kind divergence`, or in the same run under `write: { promote: auto }`); content-keyed (identical repeat `unchanged`, D10); roles I R | report `DivergenceAddReport` | `canon divergence add auth.login.01 --actor-id reviewer-1 --role reviewer` |
| `canon divergence defer` | `<SCENARIO_ID>` — 1, required (D1 r2) | `--actor-id <ACTOR_ID>` (the reviewer); `--reason <TEXT>`; `--expiry <TIMESTAMP>` | `--project-id <PROJECT_ID>`; `--rev <REV>=HEAD`; `--round <N>=1`; `--role <ROLE>` | Records a deferred divergence directly (staged and promoted in the same run); no staged-only mode | write; appends; roles I R | report `DivergenceDeferReport` | `canon divergence defer auth.login.01 --actor-id reviewer-1 --reason "waiting on the vendor" --expiry 2030-01-01 --role reviewer` |
| `canon divergence resolve` | `<SCENARIO_ID>` — 1, required (D1 r2) | `--actor-id <ACTOR_ID>` (the reviewer) | `--project-id <PROJECT_ID>`; `--rev <REV>=HEAD`; `--round <N>=1`; `--detail <TEXT>`; `--role <ROLE>` | Records a resolved divergence directly (staged and promoted in the same run) | write; appends; roles I R | report `DivergenceResolveReport` | `canon divergence resolve auth.login.01 --actor-id reviewer-1 --role reviewer` |
| `canon divergence accept` | `<SCENARIO_ID>...` — 1 or more, all or nothing (D1 r2) | `--actor-id <ACTOR_ID>` (the accepting reviewer); `--reason <TEXT>`; `--tracking <TEXT>` (an issue URL or reference) | `--project-id <PROJECT_ID>`; `--expiry <TIMESTAMP>`; `--role <ROLE>`; `--session-id <SESSION_ID>` | Human-only (D10): refuses the agent roles. `--role` must resolve to `reviewer` (`DIVERGENCE_ACCEPT_REFUSED`). Each scenario's latest evidence must be divergent, and neither `--actor-id` nor `--session-id` may match any evidence actor or session. The acceptance pins that evidence: new evidence ends it, and it lapses at `--expiry`. Not in D7b's `--rev` list: it pins evidence, not a commit. `--reviewer` is gone (`--actor-id`) | write; appends; effectNote: refreshes stale scenario-index records first; human only (D10) | report `DivergenceAcceptReport` | `canon divergence accept auth.login.01 --actor-id reviewer-1 --reason "the engine drops the event" --tracking https://example.com/engine/issues/118 --expiry 2030-01-01 --role reviewer` |
| `canon divergence list` | —  (D1 r3: it filters a collection) | — | `--as-of <TIMESTAMP>`; `[paging]`; `--fields <FIELDS>` (comma list of scenario field names, camelCase: `projectId`, `scenarioId`, `state`, `reason`, `expiry`, `tracking`) | `--as-of` (default now) governs only deferred expiry. Reads committed records only; staged ones are invisible. Renamed from `divergence status` (D1 Renames) | read; roles P I R O | report `DivergenceListReport` (`asOf`, `summary`, `scenarios[]`); always exit 0 | `canon divergence list --as-of 2030-01-01` |

### `ingest`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon ingest records` | — | — | `--watch`; `--interval-secs <N>=30`; `--run-id <RUN_ID>`; `--dry-run` | `--interval-secs` needs `--watch`. `--run-id` must be a dispatched, non-pending run (else exit 2, D4). Renamed from `ingest artifacts` (D1 Renames; "artifact" now means the blob store only) | write; replay-safe (derivation-keyed); roles O | report `IngestRecordsReport`; with `--watch` one document per pass | `canon ingest records --run-id 01ARZ3NDEKTSV4RRFFQ69G5FAV` |
| `canon ingest plans` | — | — | `--dialect <NAME>`; `--plan-source <DIR>`; `--dry-run` | `--dialect` and `--plan-source` both or neither (`requires` each way); the pair replaces the `plans:` scan and canon's native `.canon/changes/`. `--source` became `--plan-source` (D1 Renames, D12): no leaf has a bare `--source`. A pair given alone is exit 2 (was 1, D4). All unusable sources: exit 1 `INGEST_SOURCE_UNUSABLE` | write; replay-safe (unchanged sources skipped); roles O | report `IngestPlansReport` | `canon ingest plans` |
| `canon ingest sessions` | — | — | `--watch`; `--interval-secs <N>=30`; `--home <DIR>` (default `$HOME`); `--full`; `--all-workspaces`; `--dry-run` | Takes `--repo` (D5); `--canon-yaml` is gone. `--interval-secs` needs `--watch`. A missing `canon.yaml` is `CONFIG_MISSING` (0.27 degraded silently to "unwritten") | write; replay-safe (per-file digest); roles O | report `IngestSessionsReport` (`scope`, `adapters[]`, `sessionsNormalized`, `runsWritten`, `eventsWritten`, `unwritten`); with `--watch` one document per poll | `canon ingest sessions --watch --interval-secs 60` |

### `strategy`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon strategy promote` | `<STRATEGY_ID>` — 1, required, ULID (D1 r1) | — | `--evaluation <FILE>`; `--approval <FILE>`; `--signature-file <FILE>`; `--dry-run` | `--evaluation` and `--approval` both or neither; `--signature-file` requires `--approval`. By lifecycle: a quarantined strategy needs both, an active one neither (it re-renders its git-tier file); policy-pinned `approval.allowed_signers` are required on the approved path. Renamed from `learn promote`; `--signature` became `--signature-file` (D1 Renames). A blocked run on an active strategy demotes it (effectNote) | write; replay-safe (re-rendering an active strategy is idempotent); roles O | report `StrategyPromoteReport` (`strategyId`, `path`, `dryRun`, `advisories[]`); exit 1 = `LEARN_PROMOTION_BLOCKED`; argument errors are exit 2 (D4) | `canon strategy promote 01ARZ3NDEKTSV4RRFFQ69G5FAV --dry-run` |
| `canon strategy approval-payload` | `<STRATEGY_ID>` — 1, required (D1 r1) | `--evaluation <FILE>`; `--approver <NAME>`; `--approval-at <TIMESTAMP>` | — | The bytes an external signer signs (`canon-learning-approval-v1`, subject `strategy:<id>`). Renamed from `learn approve`: `approve` is reserved for commands that record a human approval (D1 Renames, audit C5). `--approval-at` is signed into the payload, so identical arguments give identical bytes (D12) | read; replay-safe (0.27 declared `write` and embedded the current time); roles O | artifact `application/octet-stream` (payload bytes) on stdout; `--json` envelopes `StrategyApprovalPayloadReport` (`payloadHex`, `approver`, `approvalAt`, `subject`, `instructions`) (D3). 0.27 printed JSON in text mode; this is a shape change | `canon strategy approval-payload 01ARZ3NDEKTSV4RRFFQ69G5FAV --evaluation evaluation.json --approver alice --approval-at 2030-01-01T00:00:00Z` |
| `canon strategy rollback` | `<STRATEGY_ID>` — 1, required (D1 r1) | `--reason <TEXT>`; `--actor-id <ACTOR_ID>` (must be a policy-pinned approver); `--signature-file <FILE>`; `--approval-at <TIMESTAMP>` | `--contradicting-trajectory-id <ID>` (must exist in the trajectory store) | The signature must verify over `strategy:<id>:rollback:<sha256(reason)>`. `--approved-at` became `--approval-at` (D1 Renames). `--actor-id` is required here, as in 0.27 | write; replay-safe (an already rolled-back strategy returns the record, "already rolled back; no additional write"); roles O | report `StrategyRollbackReport` (the rollback record, `meta.next`) | `canon strategy rollback 01ARZ3NDEKTSV4RRFFQ69G5FAV --reason "regressed in production" --actor-id alice --signature-file approval.sig --approval-at 2030-01-01T00:00:00Z` |

### `dispatch`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon dispatch begin` | — | `--regime <KEY>` (`<role>/<repo>/<area>/<hash>`, build one with `regime show`) | `--role <ROLE>` (the role the run acts as; default the role declared with `--as`); `--actor-id <ACTOR_ID>=canon`; `--provider <NAME>`; `--model <NAME>`; `--skill-id <ID>`; `--skill-digest <DIGEST>`; `--manifest <FILE>`; `--prompt-bundle <NAME@VERSION>`; `--task-id <TASK_ID>`; `--parent-run-id <RUN_ID>` | The run record is attributed to the role it acts as, so `--role` is the attribution `--role` (D12); it must equal the regime's first segment (`DISPATCH_ROLE_REGIME_MISMATCH`, exit 2). `--skill-digest` requires `--skill-id`. `--task-id` is `<change>#<n>` and must exist in the plan corpus. `--manifest` is a repository-relative ContextPack input manifest (was `--context-manifest`). `--agent-id` became `--actor-id`, `--task` `--task-id`, `--parent-run` `--parent-run-id` (D1 Renames). | write; appends (each call mints a new run id); roles O | report `DispatchBeginReport` (`runId`, `manifest`, `injectedGuidance[]`, `lineage`, `taskId`, `parentRunId`) | `canon dispatch begin --role implementer --regime implementer/my-repo/api/abcdef012345 --task-id add-login#1` |
| `canon dispatch diff` | — | — | — | Diffs the declared plan DAG against the observed execution graph | read; roles O | report `DispatchDiffReport` (`counts`, `declared`, `observed`, `edges[]`, `notes[]`); always exit 0 | `canon dispatch diff` |
| `canon dispatch end` | `<RUN_ID>` — 1, required, ULID (D1 r1) | `--run-outcome <RUN_OUTCOME>{succeeded,failed}` | — | `--status` became `--run-outcome` (D1 Renames, D12): `--outcome` is not used, so one name never carries two value sets. | write; once (a second end is `DISPATCH_ALREADY_ENDED`, exit 2); roles O | report `DispatchEndReport` (`runId`, `manifest`, `outcome`, `endedAt`) | `canon dispatch end 01ARZ3NDEKTSV4RRFFQ69G5FAV --run-outcome succeeded` |

### `regime`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon regime show` | — | `--role-segment <NAME>`; `--repo-name <NAME>`; `--area <NAME>`; `--hash <HASH>` (6 to 64 lowercase hex, passed through) | — | Builds the canonical regime key `<role>/<repo>/<area>/<hash>` (role, repo, area canonicalized to lowercase kebab). Renamed from `regime-key`; `--role` → `--role-segment`, `--repo` → `--repo-name` (D1 Renames, D5). Takes no `--repo`: it reads no repo | read; roles O | report `RegimeShowReport` (`regimeKey`); invalid key `REGIME_KEY_INVALID`, exit 2; text mode prints the key line | `canon regime show --role-segment reviewer --repo-name my-repo --area api --hash abcdef012345` |

### `plugin`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon plugin sync` | `<PLUGIN_ID>` — 1, required, a `.canon/plugins/<id>/plugin.yaml` id (D1 r1) | — | `--spec-root <DIR>`; `--dry-run` | The plugin must resolve, have a registered overlay source and own at least one overlay. `--spec-root` as in `inventory sync` | write; replay-safe (content-digest dedup reports `deduped`); roles O | report `PluginSyncReport` (`roots[]`); exit 1 = per-candidate write errors (`PLUGIN_SYNC_ERRORS`) | `canon plugin sync my-plugin` |

### `skills`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon skills install` | — | — | `--skills-source <DIR>`; `--providers <LIST>` (claude,codex,omp,pi comma list, or `none`); `--dry-run` | `--target` is gone: the consumer repo is `--repo` (D5). `--skills-source` was `--source` (D12). Source default `CANON_SKILLS_SOURCE`, else `canon/skills`. An explicit `--providers` also rewrites `skills.providers` in `canon.yaml`. Writes the short stub and lock (D10); prunes orphans whose bytes still match the lock and keeps modified ones | write; replay-safe (`unchanged`); roles O | report `SkillsInstallReport` (`providers`, `selection`, `changed`, `configWritten`, `pruned[]`, `kept[]`, `ownedPaths[]`) | `canon skills install --providers claude,codex` |
| `canon skills check` | — | — | `--skills-source <DIR>`; `--providers <LIST>` | Also reports a skill or stub older than the binary (D10). `--target` is gone (D5) | read; roles O | report `SkillsCheckReport` (`clean`, `statuses[]`, `orphans[]`, `remnants[]`, `outdated`); exit 1 = drift (`SKILLS_DRIFT`) | `canon skills check` |
| `canon skills doctor` | — | — | `--skills-source <DIR>`; `--providers <LIST>` | Diagnostics only; never edits. `--target` is gone (D5) | read; roles O | report `SkillsDoctorReport` (`lines[]`); always exit 0 | `canon skills doctor` |
| `canon skills show` | `<NAME>` — 1, required: a guide embedded in the binary (D10; `canon` is the only name the plan fixes, O8) | — | `--full` | Prints the version-matched guide built into the binary; no repo. The installed `SKILL.md` is a stub that points here (D10, Q9) | read; roles P I R O | artifact `text/markdown` on stdout; `--json` envelopes `SkillsShowReport` (`name`, `version`, `markdown`) (D3, D10) | `canon skills show canon --full` |

### `tier`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon tier age` | — | — | — | Applies `canon.yaml` aging rules, moving old records to their destination tier (write, confirm, delete). Preview by default; `--commit --plan-digest` acts (D6). `--dry-run` and `--canon-yaml` are gone (D6, O2). No aging rules configured: "nothing to do", exit 0 | destructive; replay-safe (already aged records are counted, not moved); roles O | report `TierAgeReport` (`planDigest`, `effects[]`, `committed`, `rules[]`) | `canon tier age` |

### `ledger`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon ledger compact` | — | — | — | Moves superseded versions of every record from `.canon/ledger` to `.canon/archive`; a record carrying a standing claim stays. Preview by default (D6); `--dry-run` is gone | destructive; replay-safe (a repeat finds nothing to move); roles O | report `LedgerCompactReport` (`planDigest`, `effects[]`, `committed`, `kinds[]`) | `canon ledger compact` |

### `report`

| Command | Operand | Required options | Optional options | Exclusive, dependent, notes | Effect; idempotency; roles | Output | Example |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `canon report build` | — | — | — | Writes `.canon/REPORT.md`; shells out to `duckdb` (missing: `REPORT_FAILED`, exit 1); scratch files under the system temp dir (effectNote) | write; replay-safe (byte-identical for unchanged inputs); roles O | report `ReportBuildReport` (`path`) | `canon report build` |
| `canon report check` | — | — | — | Byte-diffs `.canon/REPORT.md` against a fresh build; writes no repo file; `--check` was a mode that changed the effect and `meta.command` (D1 modes rule, Q3) | read; effectNote: scratch files under the system temp dir; roles O | report `ReportCheckReport` (`state`: fresh, missing, drift); exit 1 = missing or drift (`REPORT_STALE`) | `canon report check` |
| `canon report snapshot` | `<DIR>` — 1, required: the output directory (created if absent) (D1 Renames) | — | — | Writes 10 `<mart>.parquet` files and `manifest.json` (carries `generated_at`) into `<DIR>` and prints a report on stdout; it is not an artifact on stdout (D3). `--snapshot <DIR>` is the old spelling | write; replay-safe (tables identical, `manifest.json` differs by `generated_at`); roles O | report `ReportSnapshotReport` (`directory`, `tables[]`); the files are written into `<DIR>` | `canon report snapshot snapshot-out` |

## 0.27 → 0.28

Every 0.27 leaf (the 70 in `canon schema --all`, plus the runnable group `canon brief`) and every renamed or removed option. Generated into the README Upgrading section, the "Removed in 0.28" table and the usage-error hints (D8): an old spelling is a usage error, exit 2, whose `error.hint` names the replacement and `error.expected` lists the valid spellings. Operands are written bare here (`C` = a change id, `S` = a scenario id).

### Commands

| 0.27 | 0.28 | Rule |
| --- | --- | --- |
| `adapter authorize --response F --repo D` | `adapter authorize --response F` | `--repo` optional, discovered (D5) |
| `adapter validate` | `adapter validate` | unchanged |
| `artifacts migrate --to B [--dry-run]` | `artifacts move --tier B` (+ `--commit --plan-digest`) | D6; bare run previews |
| `artifacts verify [--full]` | `artifacts verify [--full]` | unchanged |
| `brief` (bare) | `brief show [CHANGE_ID]` | a group is never runnable (D2) |
| `brief approve [--change-id C] --by W --note N` | `brief approve [C] --approver W --note N` | operand (D1 r2); `--by` → `--approver` |
| `brief check [--change-id C]` | `brief check [C]` | operand; exit 1 = blockers (D4) |
| `change new SLUG … [--kind K]` | `change new CHANGE_ID … [--change-kind K]` | `--kind` is reserved for record kinds (D12) |
| `context` | `context` | `evidence_record` kind name → `evidence` (Q6); gains `commands`, `families` |
| `context-pack new --manifest F` | `context-pack add --manifest F` | D1 Renames |
| `context-pack show\|verify ID` | `context-pack show\|verify PACK_ID` | placeholder rename only |
| `dashboard` | `dashboard` | unchanged |
| `demo add` | `demo review` | D1 Renames |
| `demo init` | `demo init` | unchanged |
| `dispatch begin --agent-id --context-manifest --task --parent-run` | `dispatch begin --actor-id --manifest --task-id --parent-run-id` | D1 Renames |
| `dispatch diff` | `dispatch diff` | unchanged |
| `dispatch end --run-id R --status S` | `dispatch end R --run-outcome S` | operand (D1 r1); `--status` → `--run-outcome` (D12) |
| `divergence accept --project-id P --scenario-id S… --reviewer W --reason … --tracking …` | `divergence accept S… --actor-id W --reason … --tracking …` | operands (D1 r2); `--project-id` only with several roots; human-only (D10) |
| `divergence add --scenario-id S --sha H --status X --reviewer W --role R` | `divergence add S --rev H --state X --actor-id W [--role R]` | D1 r2, D7b, Renames |
| `divergence defer … --sha H --reviewer W` | `divergence defer S --rev H --actor-id W …` | as above |
| `divergence promote [--dry-run]` | `gate promote --kind divergence [--dry-run]` | D1 Renames |
| `divergence resolve … --sha H --reviewer W` | `divergence resolve S --rev H --actor-id W` | as above |
| `divergence status` | `divergence list` | D1 Renames |
| `evidence add --kind audit …` | `evidence audit SCENARIO_ID …` | D1 Renames, Q3 |
| `evidence add --kind C …` | `evidence add --class C …` | D1 Renames |
| `evidence approval-payload --task T --artifact-sha H --approval-by W` | `evidence approval-payload --task-id T --rev H --approver W` | D1 Renames, D7b; `--rev` optional |
| `evidence vault` | `artifacts store` | D1 Renames |
| `export --out F` | `export -o F` / `--output F` | D1 Renames |
| `feature new SURFACE` | `feature new SURFACE` | unchanged |
| `finding add --reviewer W --reviewed-sha H --resolution-sha H --introduced-by H` | `finding add --actor-id W --rev H --resolution-rev H --introduced-rev H` | D7b; `--rev` and `--resolution-rev` optional |
| `finding close --change-id C --round N --seq M …` | `finding close C --round N --seq M …` | operand (D1 r2); `--resolution-sha` → `--resolution-rev` |
| `finding list --change-id C` | `finding list --change-id C` | unchanged (D1 r3) |
| `format ROOT [--check]` / `fmt ROOT` | `format [DIR]` | `fmt` alias and no-op `--check` removed; no operand means the configured spec roots (D8, D12) |
| `gate check` | `gate check` | unchanged; `unverifiable` result (D3) |
| `gate install-hooks` | `gate install-hooks` | default `--command` becomes `canon task done`; gains `--dry-run` |
| `gate promote` | `gate promote [--kind K]` | gains `--kind` |
| `gate selftest` | `selftest --suite trust-spine-gate` | D1 Renames |
| `gate task TASK_ID` | `task done TASK_ID` | D1 Renames; installed hooks are rewritten by `migrate` (D9) |
| `ingest artifacts` | `ingest records` | D1 Renames |
| `ingest plans --source D` | `ingest plans --plan-source D` | D1 Renames |
| `ingest sessions --canon-yaml F` | `ingest sessions --repo D` | D5 |
| `init --check-config` | `init --check` | D1 Renames |
| `inventory sync` | `inventory sync [--dry-run]` | gains `--dry-run` (D6) |
| `learn approve ID --principal W --evaluation F` | `strategy approval-payload ID --approver W --evaluation F --approval-at T` | D1 Renames; `--approval-at` is signed into the bytes, which are an artifact (D3, D12) |
| `learn promote ID --signature F` | `strategy promote ID --signature-file F` | D1 Renames |
| `learn rollback ID --approved-at T …` | `strategy rollback ID --approval-at T …` | D1 Renames |
| `ledger compact [--dry-run]` | `ledger compact` (+ `--commit --plan-digest`) | D6; bare run previews |
| `migrate [--dry-run]` | `migrate` (+ `--commit --plan-digest`) | D6; bare run previews |
| `plugin sync PLUGIN_ID` | `plugin sync PLUGIN_ID [--dry-run]` | gains `--dry-run` (D6) |
| `policy approve --by W` | `policy approve --approver W` | D1 Renames; human-only (D10) |
| `policy show` | `policy show` | unchanged |
| `prompt add --name N --version V --manifest F` | `prompt add N@V --manifest F` | operand (D1 r1); the Renames row `--version` → `--bundle-version` is deleted (D12) |
| `prompt show --name N --version V` | `prompt show N@V` | as above |
| `purge --kind K --before T --plan` | `purge --kind K --before T` | bare run previews (D6) |
| `purge --kind K --before T --confirm D` | `purge --kind K --before T --commit --plan-digest D` | D6 |
| `query --plugin P --canon-yaml F` | `query --plugin-id P --repo D` | D1 Renames; `--canon-yaml` removal is O2 |
| `regime-key --role R --repo N --area A --hash H` | `regime show --role-segment R --repo-name N --area A --hash H` | D1 Renames, D5 |
| `report` | `report build` | D1 Renames |
| `report --check` | `report check` | D1 Renames |
| `report --snapshot D` | `report snapshot D` | D1 Renames |
| `retrieve -k N` / `--k N` | `retrieve --limit N` | D1 Renames |
| `retrieve --role R` | `retrieve --for-role R` | `--role` means only attribution (D12) |
| `review add --scenario-id S… --reviewer W --pin P [--outcome O]` | `review scenario S… --actor-id W [--rev R] [--review-outcome O]` | D1 Renames, D7b, D12 |
| `review add --change-id C --reviewer W --pin P [--outcome O]` | `review change C --actor-id W [--rev R] [--review-outcome O]` | D1 Renames, D7b, D12 |
| `scenario new TAG [--from F]` | `scenario new SCENARIO_ID` / `--input` batch | `--from` removed (D7) |
| `schema [--all]` | `schema` (unfiltered; `--as R` filters) | `--all` removed; `CANON_ROLE` no longer filters (D3) |
| `selftest` | `selftest [--suite N]` | D1 Renames |
| `skills check\|doctor\|install --target D --source S` | `skills check\|doctor\|install --repo D --skills-source S` | D5, D12 |
| — | `skills show NAME [--full]` | new (D10; `show` because D1 bans `get`) |
| — | `completion SHELL` | new (D2) |
| `status` | `status` | top-level `next` moves to `meta.next` (D3) |
| `subject adopt CHANGE_ID --subject-id S` | `subject adopt SUBJECT_ID --change-id C [--dry-run]` | operand (D1 r1); gains `--dry-run` |
| `subject new ID` | `subject new SUBJECT_ID` | placeholder rename only |
| `subject status ID` | `subject show SUBJECT_ID` | D1 Renames |
| `subject status ID --to S [--reason --override-reason --actor-id]` | `subject move SUBJECT_ID --to S [--reason --override-reason --actor-id]` | D1 Renames |
| `subject unadopt CHANGE_ID --subject-id S` | `subject unadopt SUBJECT_ID --change-id C [--dry-run]` | operand (D1 r1); gains `--dry-run` |
| `tier age [--dry-run] [--canon-yaml F]` | `tier age` (+ `--commit --plan-digest`) | D6, D5 (O2) |

### Options and behaviours that changed on several leaves

| 0.27 | 0.28 | Rule |
| --- | --- | --- |
| `--agent-id`, `--reviewer` (as an id) | `--actor-id` | D1 Renames; `--actor-id` is required where `--reviewer` was (O9) |
| `--by`, `--principal`, `--approval-by` | `--approver` | D1 Renames |
| `--pin`, `--sha`, `--artifact-sha`, `--reviewed-sha` | `--rev` (default `HEAD`) | D1 Renames, D7b |
| `--resolution-sha` | `--resolution-rev` (default `HEAD` when fixed) | D7b |
| `--introduced-by` | `--introduced-rev` (no default) | D7b |
| `--approved-at`, `--signature`, `--approval-signature-file`, `--context-manifest` | `--approval-at`, `--signature-file`, `--manifest` | D1 Renames |
| `--version` on `prompt` | operand `NAME@VERSION` | D1 r1, D12 |
| `--out` | `-o, --output` | D1 Renames |
| `-k`, `--k` | `--limit` | D1 Renames |
| `evidence add --kind` | `--class` | D1 Renames |
| `dispatch end --status`, `divergence add --status` | `--run-outcome`, `--state` | D1 Renames, D12 |
| `change new --kind` | `--change-kind` | D12 |
| `review add --outcome` | `--review-outcome` | D12 |
| `retrieve --role` | `--for-role` | D12 |
| `skills install\|check\|doctor --source` | `--skills-source` | D12 |
| `format --check` | removed | D12 |
| `regime-key --role`, `--repo` | `--role-segment`, `--repo-name` | D1 Renames, D5 |
| `--fields` repeatable and comma list | `--fields <FIELDS>` (one comma list) only | D1 Renames |
| `--task`, `--plugin`, `--parent-run` | `--task-id`, `--plugin-id`, `--parent-run-id` | D1 Renames |
| `ingest plans --source` | `--plan-source` | D1 Renames, D12 |
| `init --check-config` | `init --check` | D1 Renames |
| `--target`, `--canon-yaml` (on `skills *`, `ingest sessions`) | `--repo` | D5 |
| `--dry-run` on `artifacts migrate`, `ledger compact`, `migrate`, `tier age` | the bare run (preview) and `--commit --plan-digest` | D6 |
| `purge --plan`, `purge --confirm` | the bare run and `--commit --plan-digest` | D6 |
| `scenario new --from`, `review add --scenario-ids-from` | `--input <JSON\|FILE\|->` (batch) | D7 |
| `artifacts migrate --to` | `artifacts move --tier` | D1 Renames |
| `gate install-hooks --command` default `canon gate task` | default `canon task done` | D9 |
| `schema --all` | removed | D3: the listing is unfiltered |
| `fmt` (alias of `format`) | removed | D8: no aliases |
| `CANON_ROLE` filters `schema`, `context`, `skills show` | `--as` filters; the variable is only a default for attribution roles | D3 |

Kind values: `<kind>_record` stays a removed 0.24 spelling; the `evidence_record` name in `canon context` becomes `evidence` (Q6); storage paths and the record's own `kind` field keep `evidence_record`.

## Resolved questions

Every `?` in the reviewer's Appendix A and every `[GUESS]` in its flows, with the answer and the rule that decides it. "Appendix" is Appendix A of the reviewer's report, "flow" its worked flows.

| # | Question (source) | Answer | Decided by |
| --- | --- | --- | --- |
| 1 | `brief show` operand: `--change-id` or `<CHANGE_ID>`? (Appendix, flow 1 step 4) | `canon brief show [<CHANGE_ID>]`, and the same for `brief check` and `brief approve`; the default is the repo's only live change, as in 0.27 | D1 r2 |
| 2 | Operand of `divergence add`, `defer`, `resolve`, `accept` (Appendix) | `<SCENARIO_ID>`; `accept` takes `<SCENARIO_ID>...` (all or nothing) | D1 r2 |
| 3 | Required options of the four divergence leaves; Appendix lists `--commit` and `--reviewer` (Appendix) | `add` and `resolve`: `--actor-id`; `defer`: `--actor-id --reason --expiry`; `accept`: `--actor-id --reason --tracking`. `--project-id` is needed only with several spec roots. The Appendix's `--commit` is `--rev`, optional, default `HEAD`; `--commit` only switches destructive leaves. `--reviewer` is dropped (`--actor-id` is the reviewer). `--role` is defaulted from `--as` like every attribution role | D1 (project-id, r2), D6, D7b, D1 Renames |
| 4 | Operand of ordinary `evidence add`; may it repeat operands? (Appendix, flow 2 steps 3-4) | No operand. One of `--task-id`, `--scenario-id`, `--all-scenarios` selects the subject. Several scenarios: one call per scenario, `--all-scenarios` with `--report`, or an `--input` array/NDJSON batch | D1 r3, D7 |
| 5 | `evidence add` required fields (Appendix, flow 2 step 3) | `--class` and `--ref`, each unless a `--report` is given (then `--class` defaults to `test-run` and `--ref` to the report paths), plus exactly the subject selection above. `--role` is conditionally required (header). `--class audit` is refused with the hint `evidence audit`. The generated D11 batch command is `canon evidence add --all-scenarios --report junit:a.xml --report junit:b.xml --role implementer`, runnable as written | D1 Renames, D1 modes rule, D11, D12 |
| 6 | `evidence audit`: does it keep `--ref`? Exact requiredness (flow 2 step 5) | Operand `<SCENARIO_ID>`; required `--ref --evaluator-id --evaluator-version --evaluator-digest --audit-result`; `--project-id` only with several roots; no `--class`, `--verdict`, `--task-id`, `--surface-ref`, approval options | D1 Renames, Q3 |
| 7 | `evidence approval-payload` operand and options (Appendix lists only `--commit --approver --approval-at`) | No operand; one of `--task-id` or `--scenario-id`; required `--approver --approval-at`; `--rev` optional, default `HEAD` | D1 r3, D7b |
| 8 | `finding add` operand (Appendix) and whether `--reviewer` remains (flow 3 step 3) | No operand; required `--change-id --round --seq --severity --actor-id --summary`; `--reviewer` is `--actor-id`; `--role` defaults to `reviewer`; `--rev` defaults to `HEAD` | D1 r3, D1 Renames, D7b |
| 9 | `finding close` key syntax (Appendix `<FINDING_KEY>`, flow 3 step 4: `login#1#1`) | There is no key string: `canon finding close <CHANGE_ID> --round <N> --seq <N> --disposition <D>`. The composite key string is internal storage and is never printed. `finding add` reports `changeId`, `round`, `seq` | D1 r2 |
| 10 | `gate promote --kind`: required? values after merging divergence promotion? (Appendix, flow 2 step 6) | Optional. Values `evidence`, `finding`, `divergence`, the three kinds that are staged; absent drains all three. `divergence promote` is `gate promote --kind divergence` | D1 Renames; the staged kinds are the ones `evidence add`, `finding add` and `finding close` and `divergence add` stage |
| 11 | `strategy approval-payload` required options (Appendix) | Operand `<STRATEGY_ID>`; required `--evaluation --approver --approval-at` (the time is signed into the bytes, so the leaf is a pure read) | D1 r1, D1 Renames, D12 |
| 12 | `purge` target (Appendix, review blocker) | No operand; the target is `--kind` (only `session`, `run`, `event`, `trajectory`) and `--before`, both required; acting needs `--commit --plan-digest` | D1 r3, D6, Q1 |
| 13 | `review scenario`: `--reviewer`, `--pin`, project-id inference, `--pin` arity (Appendix, flow 3 step 2) | `<SCENARIO_ID>...`; required `--actor-id` and exactly one of `--upstream-ref` or `--original-spec-ref`; `--pin` is `--rev` (one value for the whole batch, optional, default `HEAD`); `--reviewer` is `--actor-id`; `--project-id` only with several roots; `--role` conditionally required (header) | D1 r2, D1 Renames, D7b |
| 14 | `review change` operand (Appendix) | `<CHANGE_ID>`; same options as `review scenario` | D1 r2 |
| 15 | `subject status` final shape and requiredness (flow 4 step 3) | `subject move <SUBJECT_ID> --to <STATE>`; `--to` is the only required option; `--reason` is required for `shipped → verifying`; `--actor-id` default `canon`. The read half is `subject show <SUBJECT_ID>` | D1 Renames, D1 verbs (`move`) |
| 16 | Does the verb `move` contradict D1's denylist? (reviewers' blocker) | No: the plan's D1 now lists `move` as a verb (`artifacts move --tier`, `subject move --to`); `update`, `get`, `delete` are the unused ones | D1 verbs |
| 17 | `init`: provider values, root normalization, dry-run default (flow 1 step 1) | `--providers` is a comma list of `claude`, `codex`, `omp`, `pi`, or `none`; an unknown name is `INIT_INVALID_ARGUMENT`, exit 2, before anything is written. `--repo` is taken as given (no ancestor walk, default current directory). `init` is not preview-by-default: only `purge`, `artifacts move`, `migrate`, `ledger compact`, `tier age` are; `init --dry-run` is optional | D6 |
| 18 | `subject new` requiredness (flow 1 step 2) | Operand `<SUBJECT_ID>`; required `--domain --title`; unchanged from 0.27 | no rule changes it |
| 19 | `change new` operand and valid kind (flow 1 step 3) | `<CHANGE_ID>`; required `--subject-id --title`; `--kind` is `feature` (default) or `chore` | D1 r1 |
| 20 | `brief approve`: is `--note` required, operand shape (flow 1 step 6) | Required `--approver` and `--note` (the human's own words); operand `[<CHANGE_ID>]`; human only | D1 r2, D1 Renames, D10 |
| 21 | `scenario new`: repeatability and the replacement of `--from` (flow 1 step 7) | One `<SCENARIO_ID>` operand and `--title`; `--case`, `--flow` are single-valued. Many scenarios: `--input` with an array or NDJSON, each element carrying `operands` | D7 |
| 22 | `dispatch begin`: valid regime and task grammar (flow 2 step 1) | `--regime` is `<role>/<repo>/<area>/<hash>` (`canon regime show` builds one; `--role` must equal its first segment); `--task-id` is `<change_id>#<n>` and must exist in the plan corpus | D1 Renames; 0.27 rules unchanged |
| 23 | `dispatch end`: closed outcome values (flow 2 step 2) | `--run-outcome {succeeded,failed}`, with the run id as operand | D1 r1, D1 Renames, D12 |
| 24 | `retrieve`: envelope or bare array (flow 3 step 1) | Enveloped; `data` is the array of guidance items. The installed hook reads `(.data // .)` and works before and after | D3, D9 |
| 25 | `divergence accept ... --override-reason`? (flow 4 step 5) | The leaf has no `--override-reason`. `accept` is human only: an agent role is refused and the hint tells it to ask the user. `--override-reason` itself stays available to every role on `evidence add`, `evidence audit` and `subject move` | D10, D12 |
| 26 | Ship flow: is the second `gate check` required, and is `meta.next` authoritative? (flow 4 step 4) | `gate check` exit 0 with `clean` is the done criterion before and after `subject move --to shipped`; `meta.next[].argv` is the one place for next steps (the 0.27 top-level `next` of `status` moves there) | D3, D4, D10 |
| 27 | `--project-id`: one stable inference rule (review finding) | With one configured spec root it is inferred; with several it is required, on every leaf that takes it: `evidence add`, `evidence audit`, `evidence approval-payload`, `review scenario`, `review change`, `divergence add`, `defer`, `resolve`, `accept`, `feature new`, `scenario new`. `--all-scenarios` follows the same rule | D1 |
| 28 | `schemaVersion` grammar (review blocker) | `"<major>.<minor>"` per report, `"1.0"` in 0.28; the report is named by `meta.command`, the release by `canonVersion` | D3 |
| 29 | Output form per leaf (review blocker) | The Output column. Artifacts: `export`, `evidence approval-payload`, `strategy approval-payload`, `skills show`, `completion`; stream: `dashboard`, and the `--watch` forms of `ingest`; all others enveloped reports. `report snapshot` writes files into its directory and prints a report. Every write leaf also has the `Batch<Name>Report` variant (header) | D3, D7 |
| 30 | `finding close` of an already closed finding, repeated `add` (review) | `close` to the current disposition is `unchanged`; an identical `finding add` is `unchanged`, a different one under an occupied key stays `FINDING_SEQ_OCCUPIED` | D10 |
| 31 | Batch output shape (round 2) | `Batch<Name>Report`: `data.items[]` with `index`, `status` (`written`, `unchanged`, `failed`, `skipped`), `data` or `error`; exit 0, 1 or 2 as in the header | D2, D7 |
| 32 | `--input` framing (round 2) | The whole source is parsed as one JSON value first; only if that fails and the source is a file or stdin are non-empty lines parsed as NDJSON (`#` lines skipped). An object is one invocation (ordinary report), an array or NDJSON is a batch (Batch report), even with one element | D7 |
| 33 | `error.field` for `--input` (round 2) | An RFC 6901 JSON pointer, `/items/<i>/<key>` for a batch element and `/<key>` for an object; an argv name for flags | D3, D7 |
| 34 | Is `init --check` a mode that conflicts with everything? (round 2) | No: only with the mutating options; `--json`, `--as`, `--repo` stay. Example `canon init --check --json` | D1 modes rule |
| 35 | Migration recovery step (round 2) | Two runnable commands: `canon migrate --json`, then the preview's `canon migrate --commit --plan-digest <actual digest> --json` | D9 |
| 36 | Explaining an error code (round 2) | `canon schema --errors --json` (meaning, exit, retryable, fix per code) and `canon schema --error <CODE>` | D3 |

## Decided in round 2

Taken from the second review round and applied to the plan (D12) and to every row above. The numbers stay reserved.

| # | Was | Decision | Rows |
| --- | --- | --- | --- |
| O3 | `prompt` operand against the `--bundle-version` rename | The operand `<NAME@VERSION>` wins; the `--version` → `--bundle-version` Renames row is deleted | `prompt add`, `prompt show` |
| O4 | `--kind` meant a record kind and a brief template | `change new --change-kind {feature,chore}`; `--kind` is only a record kind (`query`, `export`, `purge`, and the declared subset on `gate promote`) | `change new` |
| O5 | `--outcome` had two value sets | `dispatch end --run-outcome {succeeded,failed}`; `review scenario` and `review change --review-outcome {signed,indeterminate,unavailable}` | `dispatch end`, `review scenario`, `review change` |
| O6 | `format` kept `--check`, a required operand and a scope exception | `format [<DIR>]` defaults to the configured spec roots; `--check` and `fmt` are gone | `format` |
| O13 | D10 made `--override-reason` human-only | It stays available to every role; every waived refusal is recorded, echoed in `data.overrides[]` and listed by `status`. Human only: `brief approve`, `policy approve`, `divergence accept` | `evidence add`, `evidence audit`, `subject move` |
| O14 | `strategy approval-payload` embedded the clock | `--approval-at <TIMESTAMP>` is required and signed into the bytes; the leaf is a replay-safe read | `strategy approval-payload` |
| — | `--role` also meant a filter or a key segment | `--role` is only attribution (default `--as`); `retrieve --for-role`; `regime show --role-segment`; `dispatch begin --role` is the role the run acts as, so it keeps `--role` | `retrieve`, `dispatch begin`, `regime show` |
| — | bare `--source` meant two things | `--skills-source` (`skills install`, `check`, `doctor`) and `--plan-source` (`ingest plans`) | those rows |
| — | `skills get` against D1's verb list | `skills show <NAME>` | `skills show` |
| — | `--fields <A,B>` | `--fields <FIELDS>`, one comma list | `status`, `context`, `query`, `export`, `finding list`, `divergence list` |

## Decided by the owner (2026-10-23)

Round 2 decided O3, O4, O5, O6, O13 and O14 (next section). The owner took every remaining recommendation below as decided (O1, O2, O7–O12, O15, O16); the table already applies them.

| # | Question | Recommendation | Rows that change |
| --- | --- | --- | --- |
| O1 | D5 says an absent `--repo` means the nearest ancestor with `canon.yaml`, but `init` and `demo init` create that file. | They take the directory as given, default the current directory, with no ancestor walk (0.27 behaviour). | `init`, `demo init` |
| O2 | `query` and `tier age` also accept `--canon-yaml <FILE>`, a second way to name the context that D5 does not list among the options it replaces. | Remove it; `--repo <DIR>` is the one context option (D5's own principle). | `query`, `tier age` |
| O7 | `query`, `export` and `purge` append an access-audit line, and 0.27 also appends one on refused runs, which contradicts "a refusal leaves the tree unchanged" (D6, T12). | A refused run writes no audit line; a run that read or deleted records still does and says so in `effectNote`. `export --dry-run` writes neither the output file nor an audit line. | `query`, `export`, `purge` |
| O8 | `skills show <NAME>`: D10 fixes `canon` only. | The names are the guides embedded in the binary; `canon schema skills show` lists them as `possibleValues`. | `skills get` |
| O9 | Dropping `--reviewer` leaves one identity (`--actor-id`), whose 0.27 default is `canon`. | `--actor-id` is required wherever 0.27 required `--reviewer` or `--actor-id` (`review scenario`, `review change`, `finding add`, the four `divergence` writers, `strategy rollback`), and defaults to `canon` elsewhere. | those rows |
| O10 | D10 names three idempotency values, but `subject new`, `change new`, `feature new`, `scenario new`, `demo init`, `subject move`, `dispatch end` refuse a repeat (exit 2) and `dispatch begin`, `demo review` always append. | Add a fourth value `once` (repeat refused, tree unchanged) for the refusing ones; `appends` covers the others. | those rows |
| O11 | `--expiry` is RFC3339-only on `divergence add` and `defer` but also takes `YYYY-MM-DD` on `accept`; `--before`, `--after` and `--since` are RFC3339-only. | One `<TIMESTAMP>` grammar everywhere: RFC3339 or `YYYY-MM-DD` as UTC midnight (loosening, so no break). | every timestamp option |
| O12 | D10 makes a duplicate `add` and a repeated `close` no-ops, but is silent on `divergence defer`, `resolve`, `accept` (0.27: each appends a record). | Declare them `appends` now; make them `content-keyed` like `add` when S3 touches them. | `divergence defer\|resolve\|accept` |
| O15 | `skills install`, `skills check`, `skills doctor` `--skills-source` and the 0.27 `skills install` source default (`CANON_SKILLS_SOURCE`, else `canon/skills`) predate the embedded guide (D10). | Keep `--skills-source` as a developer override only; the default install writes the stub from the embedded text. | `skills install`, `skills check`, `skills doctor` |
| O16 | `--input` batches (D7) on the five destructive leaves: a plan digest binds one plan, and a document cannot carry `--commit`. | The destructive leaves take one object only; an array or NDJSON is refused (`INPUT_BATCH_UNSUPPORTED`, exit 2, nothing written). | `purge`, `artifacts move`, `migrate`, `ledger compact`, `tier age` |
