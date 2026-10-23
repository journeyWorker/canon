# How large agent-used CLIs solve canon's CLI-grammar axes (primary-source research)

Date: 2026-10-22. Author: BigCliResearch. Scope: gh, kubectl, docker, cargo (rustfmt/clippy), Biome, oxc (oxfmt/oxlint), Terraform, flyctl, AWS CLI v2, Cloudflare wrangler + the new `cf` CLI, Google `gws`, plus prettier / ESLint / ruff / deno where they fill a gap.
Every claim cites a source key `[Sn]` (URL list at the end). `[INFERENCE]` = my conclusion, not stated by a source. Where a doc page was cut by the fetch tool, I paged through it; where I could not verify, I say so.

Canon context used: draft plan `docs/superpowers/plans/2026-10-22-canon-cli-grammar.md` (origin/docs/cli-grammar-plan), audit-surface (ids X1–X9, C1–C16, O1–O22, D1–D8), checklist `cli-interface-design/SKILL.md`, installed `canon 0.25.0` (`canon format --help`, `canon schema format --json`), `crates/canon-fmt/src/lib.rs:1-12`.

---

## 0. Ten findings that should change or confirm the plan

1. **format ≠ lint ≠ validate is the industry shape, and canon's `format` is a *validator*.** `canon schema format --json` says "Validate a spec/artifact corpus against canon's format", `effect: read`, `--check` is a no-op; `canon-fmt` "only ever validates" (`crates/canon-fmt/src/lib.rs:1-12`). Terraform has `fmt` (rewrite) + `validate` (structure, offline); Cargo has `fmt` + `check` + `clippy`; Biome has `format` + `lint` + `check` + `ci`; oxc has `oxfmt` + `oxlint`. §1 gives a concrete 2-command design (`format` rewrites, `lint` reports with rule ids and `--fix`) and the one trap (silent semantic flip of the name `format`).
2. **`--check` is the most guessable flag for a formatter** (rustfmt, cargo fmt, prettier, oxfmt, ruff, deno, terraform `-check`); Biome is the outlier (`format` without `--write` reports, no `--check`). Exit: 0 ok / 1 would change / 2 tool error is documented by prettier and ESLint/ruff; rustfmt uses 1 for both; oxlint exits 1 for *everything* (source). Canon's D4 (0/1/2) matches the best-documented tools.
3. **No surveyed CLI wraps results in `{ok,data,meta}`.** gh, aws, kubectl, terraform, docker, cargo print the report directly; cf *unwraps* the Cloudflare `{success, result, result_info}` envelope. Canon's envelope is a deliberate local choice (keep, but know the token-cost trade, §4).
4. **Version fields in JSON are small decoupled integers / major.minor, not `<release>.<report>`:** terraform `format_version "1.0"` (minor = additive, ignore unknown properties; major = break, reject), cargo metadata `"version": 1`, wrangler output entries `version: 1` per `type`. The plan's `schemaVersion = <release>.<report>` does not encode compatibility on its face. See §4.
5. **"No command missing from `schema`/`context`" is solved by three layered mechanisms in the big CLIs**: (a) one registry that both parser and metadata read, with *compile-time* completeness (cf: types "require exactly one typed sidecar per registered command"); (b) generated artifacts checked in + CI `git diff` (docker, cargo, cf); (c) a test over the real parser tree (cargo `verify_cli` → clap `debug_assert`). Wrangler is the cautionary tale: `registerLegacyCommand` lets commands bypass the registry. §3 has a Rust design that makes a missing row a compile error.
6. **Env-dependent output is a proven footgun.** oxlint auto-switches to `--format agent` when it sees `CLAUDECODE=1` etc., and `oxlint --rules` then silently prints nothing and exits 0 (oxc issue #27569). Supports the owner rule "same command line → same output". cf also rewrites its own `--help` when it detects an agent harness (outlier).
7. **`--commit`-style preview-by-default has no precedent in the 11 CLIs surveyed**; the closest are terraform `plan -out` → `apply <planfile>` (passing the plan file *is* the approval; `-auto-approve` is ignored for a saved plan) and `--dry-run` everywhere. Canon's `--plan-digest` binding is the same idea as the plan file. Keep it (owner decision); do not expect agents to guess `--commit` from other tools — `schema`/error `hint` must carry it.
8. **Non-interactive refusals must exit non-zero.** cf shipped destructive commands that "report success after aborting" without a terminal, then fixed it to exit 1 (cf CHANGELOG, beta). gh: `--yes required when not running interactively`. Same class as canon X2 (fail-open `gate check`).
9. **Deprecation windows:** kubectl ≥ 12 months or 2 releases for GA CLI elements + mandatory warning (Rule 5a/6); docker ≥ 1 stable release; wrangler deprecated-but-active for a whole major; Biome introduced `--write/--fix` and deprecated `--apply` in 1.x; cf (beta) renamed all camelCase commands to kebab-case with **no alias**. "Everything in 0.28" is compatible with these if 0.28 adds new names + warns and a single later minor removes; a pure hard cut has precedent only in pre-1.0/beta (cf) and for JSON shapes (which cannot warn). §6.
10. **Bonus for the earlier "real session id is only known to the harness" point:** cf's `agent-context.ts` reads documented env vars (`CLAUDECODE`, `CLAUDE_CODE_SESSION_ID`, `CODEX_THREAD_ID`, `CURSOR_CONVERSATION_ID`, `AMP_CURRENT_THREAD_ID`, `PI_SESSION_ID`, `QWEN_CODE_SESSION_ID`) with per-harness evidence links. Canon can *record* what these vars carry (never change behavior on them) instead of asking the harness. See §7.

---

## 1. format vs lint vs check

### 1.1 What each tool does (verified)

| Tool | Rewrites by default? | Verify flag | Fix flag | Exit codes | Notes |
|---|---|---|---|---|---|
| rustfmt / cargo fmt | yes (in place) `[S1]` | `--check` | – | `--check`: 0 clean / 1 would change; other modes: 1 on error `[S1]` | `cargo fmt` is an external component, not part of cargo `[S2]` |
| cargo check | n/a (compile validity) | – | – | non-zero on errors | `--message-format human|short|json` `[S3]` |
| cargo clippy | no | `-D warnings` / `CARGO_BUILD_WARNINGS=deny` fails the build `[S4]` | `cargo clippy --fix` (implies `--all-targets`) `[S4]` | non-zero when denied | warnings are exit 0 unless elevated |
| cargo fix | applies | – | – | – | refuses a dirty tree unless `--allow-dirty` / `--allow-staged` / `--allow-no-vcs` `[S5]` |
| Biome `format` | **no** – `--write` (alias `--fix`) "Applies formatting changes" `[S6]` | (report is the default) | `--write` | not documented on the reference page | `lint --write` "safe lint fixes", `--unsafe`, `--suppress` `[S6]` |
| Biome `check` | no | – | `--write`/`--fix` + `--unsafe` | – | "formatting, linting, and assist actions" in one pass `[S6]` |
| Biome `ci` | **never** ("without modifying files") `[S6]` | – | – | – | separate command so effect never depends on a flag `[S6]` |
| oxfmt | **yes** `--write` "(default)"; `--check` "also show statistics"; `--list-different` `[S7]` | `--check` | – | not documented | `--init`, `--migrate=prettier|biome` `[S7]` |
| oxlint | no (report) | – | `--fix`, `--fix-suggestions`, `--fix-dangerously` `[S8]` | **0 or 1 only**: every error/invalid-option variant maps to `ExitCode::FAILURE` `[S9]` | `--deny-warnings`, `--max-warnings N`, `--rules`, `--print-config`, `-f agent|json|github|sarif|…` `[S8]` |
| prettier | **no** – default prints; `--write` "This overwrites your files!" `[S10]` | `--check`/`-c` | – | 0 formatted / 1 not formatted / 2 prettier error `[S10]` | |
| ESLint | no | – | `--fix` | 0 / 1 (errors or > `--max-warnings`) / 2 (config or internal) `[S11]` | |
| ruff format | yes ("in-place") `[S12]` | `--check` "exit with a non-zero status" | – | – | |
| ruff check | no | – | `--fix` | 0 none-or-all-fixed / 1 violations / 2 abnormal; `--exit-zero`, `--exit-non-zero-on-fix` `[S13]` | "mirrors ESLint, Prettier, RuboCop" |
| deno fmt | yes | `--check` (+ `--fail-fast`) `[S14]` | – | non-zero | |
| terraform fmt | yes | `-check` (implies `-write=false`; "exit status 0 if formatted, otherwise non-zero and lists file names"), `-diff`, `-list=false` `[S15]` | – | – | "intentionally opinionated and has no customization" `[S15]` |
| terraform validate | no | – | – | – | structural/semantic validity of config; `-json` with `valid`, `error_count`, `warning_count`, `diagnostics[{severity,summary,detail,range}]` `[S16]` |

**What separates the roles (from the sources):**
- *Formatter* = pure function of the input, no rule set, no severities ("no customization", terraform; "unified toolchain", ruff). Verification mode answers "would you change anything?" (exit 1).
- *Linter* = rule set with ids/categories/severities (`-D/-W/-A`, `--rules`, `explain`), report-only by default, **tiered fixes** (safe → suggestions → dangerous: oxlint; safe → `--unsafe`: Biome) and a warnings gate (`--deny-warnings`, `--max-warnings`, `-D warnings`). Parse/structure errors are reported *as* errors by the same command in JS tools (Biome/oxlint) but by a separate `check`/`validate` in cargo/terraform.
- *Aggregator* = a read-only run for CI. Biome made it a separate command (`ci`) instead of a flag; terraform has no aggregator; cargo CI recipe is three commands.

### 1.2 Majority / outliers
- Verify flag name: `--check` in 7 of 8 formatters (Biome none). **Majority.**
- Rewrite default: 5 write (rustfmt, oxfmt, ruff, deno, terraform), 2 do not (prettier, Biome). **No majority** — both camps are large.
- Exit table for check-style commands: 0/1/2 with 2 = tool/usage error (prettier, ESLint, ruff, AWS parse errors = 2, kubectl diff ">1"). Outliers: oxlint 0/1 only `[S9]`; terraform `plan -detailed-exitcode` has 2 = "succeeded with changes" `[S17]`; gh 2 = cancelled, 4 = auth `[S18]`; gws 0–5 typed `[S19]`; AWS 0/1/2/130/252–255 with dual meanings `[S20]`.

### 1.3 Recommendation for canon (owner: "format and lint are different tools; do not just rename")

**Two commands, three explicit roles. Name the roles by what they do.**

| Command | Role | Effect | Modes | Exit | Output |
|---|---|---|---|---|---|
| `canon format [<PATH>]...` | canonical rewrite of the corpus families canon owns; deterministic, idempotent, no rule config | `write` (with `effectNote`: "`--check` never writes") | **exactly one of** `--check` (verify: exit 1 if any file would change, list files; optional `--diff`) or `--write` | 0 canonical / 1 would change / 2 refused | report `{checked, changed:[path…]}` |
| `canon lint [<PATH>]...` | rule engine over the corpus: every finding has a stable `code`, `severity` (`error`\|`warn`), `path`, `line`, `message`, `fix?` | `read`; `--fix` makes it `write` | `--fix` applies only fixes marked safe (today: the `PROVENANCE_FIX` class in `canon-fmt/report.rs`); later `--fix-unsafe` if ever needed | 0 clean (or all fixed, ruff semantics) / 1 any `error` finding (or `warn` with `--deny-warnings`) / 2 refused | diagnostics array, shaped like terraform's `diagnostics` `[S16]` |
| `canon lint --rules` / `canon explain <CODE>` | rule catalogue (oxlint `--rules` `[S8]`, `biome explain` `[S6]`) | `read` | | | derived from the same table as `errorCodes` |

Why this design:
1. **Today's `canon format` is `lint`'s hard-error half**: it checks shape against family schemas (`canon-fmt` lib.rs:1-12; `FORMAT_VIOLATIONS`). In JS toolchains that is lint with `error` severity (oxlint/Biome report parse/structure errors from the lint pass); in cargo/terraform it would be `check`/`validate`. Either way it is **not** a formatter. Plan Q2's `format→lint` is therefore right *as a rename of the old command*; what the plan misses is that a *new* `format` is also wanted. [INFERENCE: the alternative name `validate` is consistent with canon's own C7 rule "validate = shape of one input file", and with `terraform validate`; but the owner explicitly wants oxfmt/oxlint-style roles, and lint can also carry advisory rules later, so I recommend `lint`.]
2. **The silent-flip trap.** In 0.25 `canon format <root>` is read-only and exits 1 on violations; callers (CI, hooks) rely on it. If `format` is re-introduced as a rewriter in the same release, an unchanged call site would start rewriting files and exit 0. Mitigation that costs one clap `ArgGroup`: **no default mode** — bare `canon format` is exit 2 `USAGE` with `expected: ["--check","--write"]`. This also makes `effect` derivable from argv. (Prettier needs `--write`/`--check` to be useful at all `[S10]`; Biome uses `ci` vs `check --write` for the same reason `[S6]`.) If the owner prefers a default later, "write" matches rustfmt/oxfmt/ruff/terraform and "report" matches prettier/Biome `[S1,S6,S7,S10,S12,S15]`.
3. **Which files `format` may rewrite is a canon-specific question I could not answer from the repo**: the corpus contains ledger/divergences/inventory/policy.yaml (`canon-fmt` lib.rs:1-12). Rewriting append-only or digest-referenced files could invalidate signatures/digests [INFERENCE]. Recommend `format` operate only on families declared `canonical-rewritable` in the family descriptors; `lint` reports (never rewrites) the rest.
4. **Exit table identical for both** (0/1/2), documented per leaf, `--json` report enveloped. Do not copy oxlint (everything is 1) or gh (2 = cancelled).
5. **Fix safety gate**: cargo refuses to fix a dirty tree unless told `[S5]`; Biome/oxlint tier fixes `[S6,S8]`. For `lint --fix` on consumer repos, preview first (default `lint` prints `fixable: N`), then `--fix`. If the owner wants the `--commit` vocabulary (D6) instead of `--fix`, note that every surveyed linter uses `--fix`/`--write`; `--commit` is canon-only.
6. Optional aggregator: Biome `ci` shows the value of a read-only "everything CI runs" command `[S6]`. Canon already has `gate check` (verdict) — do **not** add a second top-level `check` (collides with the C7 rule: `check` = verdict over repo state). [INFERENCE]

Do not: let `--format`/env/agent detection change `lint` output (oxlint #27569, §0.6); use `--fmt`/`fmt` alias (one name per concept; plan already drops it).

---

## 2. Command grammar, global flags, standard flag names

### 2.1 Shapes (verified)

| CLI | Shape | Evidence |
|---|---|---|
| gh | `gh <noun> <verb>` (`gh pr list`, `gh repo delete`); verbs `list/view/create/edit/close/delete`; placeholders `<dash-case>` documented | `[S18, S21]` |
| kubectl | `kubectl <verb> <TYPE> [NAME] [flags]` (`get/describe/create/delete/apply`) | `[S22]` |
| docker | `docker <noun> <verb>` with old verb-first forms kept as aliases: `docker container ls` — aliases `docker container list`, `docker container ps`, `docker ps` | `[S23]` |
| cargo | `cargo <verb>` | `[S3]` |
| terraform | `terraform <verb>` plus noun groups (`state`, `workspace`); context = global `-chdir=DIR` before the subcommand | `[S24]` |
| wrangler | `wrangler <noun> [<noun>] <verb>` plus root verbs (`deploy`, `dev`, `login`); `d1 info`, `d1 list` | `[S25, S26]` |
| cf | `cf <product> <resource> <verb>` generated from OpenAPI; verb category `read|create|update|delete|action` | `[S27, S28]` |
| flyctl | `fly <noun> <verb>` plus root verbs (`fly deploy`, `fly status`) | `[S29]` |
| aws | `aws <service> <operation>`; operations are `verb-noun` in kebab-case (`terminate-instances`), model-derived | `[S30]` |
| gws | `gws <service> <resource> <method>` built at runtime from Google Discovery docs | `[S31]` |

**Verb vocabulary is not shared even among the big ones:** gh `view`, docker/aws `inspect|describe`, wrangler `d1 info`, cf "always `get`, never `info`" `[S32]`. Majority rule: *whatever it is, one verb per concept, enforced mechanically*. Cloudflare says so outright: "manually enforcing consistency through reviews is Swiss cheese … It's always `get`, never `info`. Always `--force`, never `--skip-confirmations`. Always `--json`, never `--format`" — enforced at the schema layer `[S32]`.

### 2.2 Context and global flags
- Context as option with discovered default: terraform `-chdir=DIR` `[S24]`; wrangler `--cwd`, `--config`, `--env`, `--profile` "work on every command" `[S25]`; gh `GH_REPO` env `[S33]`; kubectl: "Don't rely on context, preferences, or other implicit states" for scripts `[S34]`. Canon D5 (`--repo` optional, nearest `canon.yaml`, else `CONFIG_MISSING` exit 2) matches; cf also refuses rather than guesses (`--temporary-account` rejected when credentials exist) `[S27]`.
- Global flags small: cf has 10 (`--help -h`, `--version -v`, `--quiet -q`, `--zone -z`, `--profile`, `--mode -m`, `--local`, `--persist-to`, …) and *reserves* names for future cross-cutting features (`--json`, `--ndjson`, `--pretty`, `--config`, `--cwd`, `--remote`, `--experimental-*`) so generated API flags cannot collide `[S27]`. **Canon should adopt the reservation list**: put `--json`, `--output/-o`, `--format`, `--yes`, `--force`, `--commit`, `--no-input`, `--quiet`, `--verbose` in the vocabulary file as reserved-with-meaning (R3/R5 already do this by denylist).

### 2.3 Output-selection flags
| CLI | Machine output | Default | Notes |
|---|---|---|---|
| gh | `--json <fields>` (comma list; omit the list to print the available fields), `--jq <expr>` (built-in jq), `--template <go-tmpl>`; `--jq/--template` require `--json` | text | `[S35]` — field discovery is an *output schema* feature |
| kubectl | `-o json|yaml|name|go-template|jsonpath|…`; docs tell scripts to request a machine form explicitly | text | `[S22, S34]` |
| docker | `--format json|table|<go-tmpl>` | table | `[S23]` |
| aws | `--output json|yaml|yaml-stream|text|table`, `--query` (JMESPath); **default JSON and overridable by `AWS_DEFAULT_OUTPUT` / config** | JSON | `[S36]` — output depends on env, the opposite of the owner rule |
| terraform | `-json` (validate/plan/apply: one JSON object or NDJSON stream) | text | `[S16, S37]` |
| fly | `-j, --json` per command, not global | text | `[S29]` |
| wrangler | `--json` per command (boolean); help string differs per command ("Return output as JSON" vs "Return machine-readable output."); plus side-channel ND-JSON file via `WRANGLER_OUTPUT_FILE_PATH` | text | `[S26, S38]` |
| cf | **JSON is the default and only format**; no `--json/--ndjson/--pretty/--fields` today (names reserved); pretty on TTY, condensed otherwise | JSON | `[S27, S39]` |
| gws | all output JSON; `--json` means **request body input**; `--page-all` streams NDJSON | JSON | `[S31]` |
| oxlint | `-f/--format` includes `json`, `github`, `sarif`, `agent`; **auto-selects `github` on Actions and `agent` when an agent env var is present** | default | `[S8, S40]` |
| Biome | `--reporter=json|json-pretty|github|gitlab|junit|checkstyle|rdjson|…` | default | `[S6]` |

Majority: classic tools = text default + explicit machine flag; API-wrapper/agent-era tools (cf, gws, aws) = JSON default. Outliers that bite agents: gws `--json` = input (collides with canon's rule "`--json` never means input"), aws/oxlint env-dependent output.

### 2.4 Recommendation
- Keep owner decision: text default, `--json` flag on every reporting leaf, no TTY/env switching. Support: terraform "Natural language command output … is not a stable interface" and "supported ways for external software to interact … JSON output modes and exit status codes" `[S41]`; kubectl conventions `[S34]`; oxlint #27569 `[S40]`.
- Declare `--json`, `--repo`, `--dry-run`, paging **once** (flatten). Wrangler shows what happens otherwise: same `--json` flag with different help strings across commands `[S26]` — exactly audit X8.
- Add gh-style **field/shape discovery** to `schema`: `outputs[].fields` listing report keys, so agents need not run a command to learn the JSON shape (gh: omit `--json` field list to print fields `[S35]`). This is the `outputs` entry the plan already adds (D1).
- Verb table in the grammar doc, enforced by test (R5-style): pick `get|list|create|update|delete|show` once; cf's rule list is the model `[S32]`.
- Placeholders: gh documents `<dash-case>` placeholders and `[ ]`, `{ | }`, `...` syntax in a repo doc `[S21]` — same approach as plan D1.

---

## 3. Machine-readable command spec and anti-drift ("no command missing")

### 3.1 How each CLI exports its own definition
| CLI | Spec source | Exported as | Drift control |
|---|---|---|---|
| gh (cobra) | the live cobra tree | `cmd/gen-docs`: builds `root.NewCmdRoot(...)` in-process with mock config and calls `docs.GenMarkdownTreeCustom` / `GenManTree`; `gh help reference` is the single-page dump | docs are a pure function of the tree `[S42, S43]` |
| docker (cobra) | live tree via `clidocstool` | `GenMarkdownTree`, `GenYamlTree` into `docs/reference/commandline/` (checked in) | CI job: run `make -f docker.Makefile mddocs` then `git diff --stat` must be empty ("generated files do not match checked-in"); separate matrix jobs run `yamldocs`, `manpages` `[S44, S45]` |
| cargo (clap) | clap tree + `mdman` markdown sources | man pages/`doc/` generated by `cargo build-man`; checked in | `ci/validate-man.sh`: require clean tree, rebuild, fail with "Please run `cargo build-man`" `[S46]`; unit test `verify_cli` = `cli(&gctx).debug_assert()` `[S47]` |
| clap ecosystem | `clap::Command` | `clap_mangen` (ROFF), `clap_complete` (static + dynamic `CompletionCandidate`) | docs recommend `cargo xtask` over `build.rs` `[S48]` |
| oclif | command classes | `oclif manifest` → `oclif.manifest.json`; `oclif readme` rewrites the `<!-- commands -->…<!-- commandsstop -->` block of README | marker-delimited generated section inside a hand-written file `[S49]` |
| kubectl | server OpenAPI | `kubectl explain TYPE[.field]` (`-o plaintext|plaintext-openapiv2`), `kubectl api-resources` | describes the *API*, not the CLI `[S50]` |
| AWS CLI | botocore service models (`botocore/data/<service>/…json`) | `aws <svc> <op>` and `aws help` generated; `--cli-input-json` + `--generate-cli-skeleton` give a per-operation input skeleton | hand-written exceptions (`aws s3`, `ddb`, `configure`) diverge: `s3 rm --dryrun` vs `ec2 terminate-instances --dry-run` `[S51, S52, S53]` |
| wrangler | `CommandRegistry.define([{command, definition}])`; `Metadata{description,status:"experimental|alpha|private beta|open beta|stable",deprecated,deprecatedMessage,hidden,owner,examples,category}`; `behaviour{…}` shared flags | docs component `<WranglerCommand command="d1 execute"/>` defaults descriptions to "the value specified in the Wrangler help API"; can flag `cfCommand` equivalents | **gap**: `registerLegacyCommand` ("TODO: Remove this once all commands use the command registry") — commands outside the registry have no metadata `[S54, S55, S56]` |
| cf | OpenAPI → generator plugin → `_generated/` yargs modules + `_meta/commands.json` (`CommandMeta{command, fullPath, description, usage, arguments, options[{name,type,required,default,enum,alias}], category, httpMethod, apiPath, operationId, hasRequestBody, hideCommand, aliases}`) + hand-written `meta.ts` sidecars | `cf schema` (`--list`), `cf agent-context [--list] <product>`, `cf cli search` (MiniSearch), hidden `cf tools` (MCP defs); **shell completion reads `_meta/commands.json` at runtime**, no generated scripts | `_generated/` tracked, "DO NOT EDIT", `linguist-generated`; `pnpm check` runs generation; `hand-written-meta.ts` "whose types require exactly one typed sidecar per registered command"; each hand-written override carries a drift-guard test; Renovate/OpenAPI-update PRs regenerate and add changesets `[S27, S28, S57]` |
| gws | Google Discovery documents (cached 24 h) | clap tree built at runtime from the document; `gws schema drive.files.list` prints request/response schema | the tree *is* the document, so it cannot drift `[S31]` |
| terraform | n/a for CLI; providers: `terraform providers schema -json`, `terraform metadata functions -json` | versioned (`format_version`) | `[S24, S16]` |

Cloudflare's own framing of why: "To do this, we needed more than could be expressed in an OpenAPI schema. … a new TypeScript schema that can define the full scope of APIs, CLI commands and arguments, and context needed to generate any interface … while still also being able to generate an OpenAPI schema" and "enforced at the schema layer" `[S32]`.

### 3.2 What the majority does
1. One definition object feeds parser + help + docs (cobra tree for gh/docker, clap for cargo, registry for wrangler/cf).
2. Generated docs are **checked in** and CI regenerates and diffs (docker, cargo; cf for `_generated/`).
3. Completions are either generated from the tree or read from the metadata at runtime (cf; clap_complete dynamic).
4. A structural self-test over the parser (`debug_assert`) exists in the clap world.
5. Where a **side door** exists (wrangler legacy commands, AWS hand-written s3), metadata/flag consistency is lost. This is the failure mode canon's audit X3 found (`brief` runnable group invisible in `schema --all`).

### 3.3 Recommendation: make "missing command" unrepresentable, then test the rest
Canon already derives schema from clap with `META` drift tests (`schema.rs:1-26`, audit) — the gap is *enumeration*, not derivation. Concrete, layered (strongest first):

1. **Compile-time completeness [INFERENCE: Rust analogue of cf's "exactly one typed sidecar per registered command"]**: `fn meta(cmd: &Command) -> &'static CommandMeta` implemented as an exhaustive `match` over the clap-derive `Command`/subcommand enums **with no `_ =>` arm**. Adding a variant without a META row fails to compile. Forbid runnable groups (plan R12) via `subcommand_required(true)` on every group enum so the only nodes with an action are enum leaves. (Rust's exhaustive match gives what cf gets from TS types.)
2. **Parser-tree walk test** (cargo `verify_cli` pattern `[S47]`): `CommandFactory::command().debug_assert()` plus a recursive walk of `get_subcommands()` asserting (a) every node is either a leaf with a META row or a group with `subcommand_required`; (b) every META id exists in the tree (no orphan rows); (c) `schema --all` ids == walk ids; (d) every id's `--help` exits 0 and every `example` parses (canon already runs examples: `schema_examples.rs`).
3. **`context` coverage derived, not authored**: build `context --json` `commands[]` from the same META table filtered by `--as <role>` (`roles`, `effect`, `summary`, `example`), as cf builds `agent-context`/`schema --list`/`cli search` from `_meta/commands.json` `[S27, S28, S57]`. Test: for every META row, `context --as r` lists it iff `r ∈ roles`; and the skill/AGENTS text blocks that enumerate commands are *generated between markers* (oclif readme pattern `[S49]`).
4. **Generated artifacts checked in + CI diff**: `docs/reference/commands.json` (= `schema --all --json`), `docs/reference/cli.md`, man pages/completions via `clap_mangen`/`clap_complete` from an `xtask` `[S48]`; CI "regenerate, `git diff --exit-code`, tell the contributor the exact regenerate command" (cargo `validate-man.sh` `[S46]`, docker validate workflow `[S45]`). Mark generated paths `linguist-generated` and say DO-NOT-EDIT in a header (cf `[S27]`).
5. **Runtime completions** read from the embedded schema (cf: "Shell-completion scripts are no longer generated — `cf complete` reads `_meta/commands.json` at runtime" `[S27]`) → completions cannot drift because they are never a second artifact.
6. **Versioned schema**: `schema` carries its own `schemaVersion` with the terraform major.minor rule (`[S16]`), and per-command `deprecated: {since, replacement}` / `hidden` / `status` as wrangler `Metadata` does `[S54]`.
7. **No side doors rule** (wrangler lesson `[S55]`): grammar doc states "a command that is not in the enum is not a command"; hand-written escape hatches (like cf's three shadowing leaves) each need a documented exception + drift-guard test `[S27]`.

---

## 4. JSON output contracts, versioning, error shapes, exit codes

### 4.1 Evidence
- **Versioning**
  - terraform: `format_version` `"1.0"` since 1.1.0; "increment the minor version … for backward-compatible changes or additions. Ignore any object properties with unrecognized names … increment the major version … for changes that are not backward-compatible. Reject any input which reports an unsupported major version." The UI stream's first message is `type: version`. `[S16, S37, S58]`
  - terraform compat promise: "The supported ways for external software to interact with Terraform are via the JSON output modes … and via exit status codes. We may extend certain JSON formats with new object properties but we will not remove or make breaking changes …" `[S41]`
  - cargo metadata: `"version": 1` — "changed if incompatible changes are ever made"; also `--format-version` selects it `[S59]`
  - wrangler output file: every ND-JSON entry has `type` and `version: 1` per type (`wrangler-session`, `deploy`, `version-upload`, `command-failed`, …) `[S60]`
  - cf: no envelope or version on output; pure payload (`result` unwrapped, `result_info` dropped for lists) `[S39]`
- **Envelopes**: none of gh/aws/kubectl/docker/terraform/cargo wrap in `ok/data`. Cloudflare's *API* uses `{success, errors:[{code,message}], messages, result, result_info}`; cf CLI strips it for success and renders errors from it `[S39, S61]`.
- **Errors**
  - cf: error box on **stderr**, plain text, status-level hints only (`401 → cf auth login`, `403 → check permissions`); "no per-code error mapping" in the CLI by policy; exit 1 `[S61, S27]`
  - gws: structured JSON error on stdout `{error:{code,message,reason,enable_url}}` **and** a human hint on stderr; exit codes 0 ok / 1 API / 2 auth / 3 validation / 4 discovery / 5 internal `[S19]`
  - terraform validate -json: diagnostics array; warns that pre-validation failures "will thus not be subject to the JSON output setting … external software … should be prepared to find data on stdout that isn't valid JSON" `[S16]`
  - gh: documents exit codes only (0 ok, 1 fail, 2 cancelled, 4 auth) and says commands may add their own `[S18]`; no JSON error envelope documented.
- **Exit codes**: see §1.2. Cost of typed tables: AWS reuses 1 and 2 with S3-specific meanings (1 = transfer failed, 2 = parse error *or* skipped files) next to 252/253/254/255 `[S20]`.

### 4.2 Majority / outliers
Majority: stable machine surface = JSON flag + exit status (terraform states it explicitly); versioned by a small independent number; errors = stderr text (plus structured only in newer API-wrapper CLIs). Outliers: gws structured error + 6 exit codes; terraform plan 2 = changes; oxlint all-1.

### 4.3 Recommendation
1. **Keep D4 (0/1/2).** Finer classification belongs in `error.code` + `retryable`, not exit codes (AWS/gws show the cost/benefit; gws's table is the only one that lets agents branch without parsing — if canon wants that, add `error.class` not new exit codes). Terraform's own caveat: exit code `2` can mean "changes" in `plan`; do not copy it.
2. **Errors always in JSON mode, decided from argv before parsing** (skill rule; terraform admits its failure `[S16]`; cf prints only text `[S61]`). Keep canon's plan: `{ok:false, error:{code,message,field,expected,hint,retryable}, meta.command}` — it is richer than any surveyed CLI. Put the human message on stderr too (gws does both `[S19]`).
3. **`schemaVersion`**: do not encode the release in it. Use terraform's contract verbatim (`major.minor`, ignore unknown properties, reject unsupported major) or cargo/wrangler's integer-per-report; put the release in `meta.canonVersion`. Rationale: a consumer cannot tell from `"0.28.gate.check"` whether `0.29.gate.check` is compatible; `[S16, S59, S60]` all use a small number whose semantics are documented. [INFERENCE: the plan says "unchanged while the payload is unchanged", which gets compatibility only if consumers compare strings for equality; the terraform scheme lets them accept additive minors.]
4. **Envelope cost**: cf README says JSON is "condensed for agents for maximum context savings" `[S39]` and unwraps. Canon's `{ok,data,meta}` adds bytes on every call; keep (consumers already read `.data`, plan D3) but keep `meta` small, compact JSON (skill rule) and let `--json` on a large list default-bound output (`--limit`, `truncated`+`hint`).
5. **Reserve `--json`/`--ndjson`/`--pretty` semantics up front** (cf does `[S27]`); streams (NDJSON) and artifacts are not wrapped (terraform `-json` apply = NDJSON stream with a leading `version` message `[S37]`; wrangler ND-JSON file `[S60]`).
6. **Pre-parse failures** (unknown flag) must also print one JSON error in JSON mode; clap's own usage blob is the single biggest source of non-JSON stdout (audit D8).

---

## 5. Dangerous operations

### 5.1 Evidence
| CLI | Ordinary mutation | Dangerous | Preview | Non-interactive |
|---|---|---|---|---|
| terraform | – | `apply` prompts; `-auto-approve` skips; **saved plan: "performs the operations … without prompting"; `-auto-approve` ignored because "the act of passing the plan file" is the approval** | `plan`, `plan -out=FILE`, `-detailed-exitcode` (0 no diff / 1 error / 2 diff) | `-input=false`; JSON `-json` `[S17, S62, S37]` |
| kubectl | executes immediately | `delete` has **no prompt** by default; `--interactive` opts in; `--force` = "bypass graceful deletion" (not "skip prompt"); `--grace-period`, `--now` | `--dry-run=none|client|server` (bool forms deprecated in 1.18 in favor of the string form), `kubectl diff` (exit 0 none / 1 differences / >1 error) | `--warnings-as-errors` `[S63, S64, S65]` |
| gh | executes | `repo delete`: prompt; **`--yes` required when not interactive** (error otherwise); **`--yes` is ignored when no repository argument is given** ("to prevent accidental deletion") — a confirm bound to an explicit target | – | `GH_PROMPT_DISABLED` env `[S66, S33]` |
| docker | executes | `system prune` prompts with a WARNING list; `-f/--force` "Do not prompt for confirmation" | – | `[S67]` |
| cf | executes | DELETE ops and "destructive non-DELETE ops" annotated in the spec (`x-forge-require-confirmation`) prompt; **only `--force` bypasses**; non-interactive prints the prompt + "pass --force" hint and aborts — originally exit success, **changed to exit 1** | **`--dry-run` on *every* generated command** (reads too), output `{command, method, url, pathParams, query, bodyKind, body}` with secret paths `<redacted>` | confirm "fails closed … default false" `[S27, S68, S69, S70]` |
| flyctl | executes | `-y/--yes` "Accept all confirmations (also --auto-confirm)"; `deploy --now` "Deploy now without confirmation" (a second, command-specific skipper) | – | `[S71]` |
| aws | executes | ec2: `--dry-run` (permission check, returns an error response by design); s3: `--dryrun` (different spelling); `terminate-instances --force/--no-force` is an API field | `[S72, S73]` |
| cargo | executes | `cargo fix` refuses a dirty tree unless `--allow-dirty/--allow-staged/--allow-no-vcs` | `publish --dry-run` [INFERENCE: not fetched] | `[S5]` |
| Biome/oxlint | – | fix tiers: `--write` < `--unsafe`; `--fix` < `--fix-suggestions` < `--fix-dangerously` | – | `[S6, S8]` |
| gws | executes | – | `--dry-run` | `[S31]` |

### 5.2 Majority / outliers
- Majority: **execute by default; guard the *destructive* few with a prompt that non-interactive mode turns into an error telling you the flag** (gh, docker, cf, fly). Terraform is the one with a first-class two-phase flow, and its saved-plan file is an approval *artifact*.
- Skip-prompt flag: `--yes` (gh, fly, wrangler docs) vs `--force` (cf, docker prune). kubectl's `--force` means something else. The owner rule (`--yes` = skip prompt, `--force` = proceed although refused) is closest to kubectl + gh and to canon's `--override-reason`; Cloudflare's rule is "always `--force`, never `--skip-confirmations`" `[S32]`. Either works *if one is chosen and enforced*.
- `--dry-run` spelling splits inside AWS (`--dryrun` vs `--dry-run`) `[S72, S73]`.
- Preview-by-default + `--commit`: **no precedent** in these CLIs [INFERENCE after reading all of the above]; it is closest to terraform plan→apply and to kubectl `--dry-run` being separate.

### 5.3 Recommendation for canon (D6 / Q1)
1. Keep `--commit` as the single act-flag for the Q1 set (owner decision). Add the terraform idea: **the plan digest is the approval artifact** — `purge` bare = preview + `planDigest`; act = `--commit --plan-digest <DIGEST>`; refuse when the digest no longer matches the current state (terraform treats a saved plan as bound approval `[S62]`; whether terraform errors on stale state is not in the fetched pages [INFERENCE: it does]).
2. Steal gh's *target-binding* trick for Q1 commands that default to "the current repo/change": `--commit` without an explicit id/digest is refused. (gh ignores `--yes` without an explicit repository `[S66]`.)
3. Steal cf's *declared destructiveness*: destructive status is **schema data** (cf: spec annotation → prompt text surfaced verbatim `[S27, S68]`). Canon's `effect: destructive` should be assigned per Q1 and a test (R11-style) should fail if a `destructive` leaf lacks the preview default or a `write` leaf deletes (X4).
4. Non-interactive refusal = exit 2 (refused) with `hint` containing the exact working invocation (cf: "pass --force"; gh: "--yes required when not running interactively"), and **never exit 0 on abort** (cf fixed exactly this `[S70]`).
5. `--dry-run` output shape: define one `plan` report for all writing leaves (cf's `DryRunOutput` lists exactly what would be sent, with redaction `[S69]`): `{command, effects:[{op, target, before?, after?}], planDigest}`. Same name everywhere (`--dry-run`), unlike AWS.
6. `--no-input`: gh has `GH_PROMPT_DISABLED` env only; cf has `isNonInteractiveOrCI()`; canon's `--no-input` flag + "`--json` implies it" is stricter than all — fine, but remember an env var is the thing CI actually sets; accept `CANON_NO_INPUT=1` as *input-gating only* (does not change output format).
7. Fix-tier flags for `lint`: use `--fix` (+ later `--unsafe`) per Biome/oxlint, not `--commit` (§1.3).

---

## 6. Deprecation and renames

### 6.1 Mechanisms (verified)
| CLI | Mechanism | Window | Evidence |
|---|---|---|---|
| gh / cobra / pflag | Flag rename keeps the old name bound to the same variable and calls `MarkDeprecated("confirm", "use `--yes` instead")` — pflag: "continue to function but will not show up in help or usage messages. Using this flag will also print the given usageMessage"; requires a non-empty message. Commands: cobra `Deprecated string` ("print this string when used"), `Hidden bool`, `Aliases` | no stated window | `gh repo delete` source `[S66]`; pflag `[S74]`; cobra `[S75]` |
| kubectl | Written policy: **Rule 5a** GA CLI elements keep working ≥ 12 months or 2 releases (whichever longer), beta ≥ 3 months/1 release, alpha 0; **Rule 5c** may not be deprecated in favor of a less stable element; **Rule 6** deprecated CLI elements "must emit warnings (optionally disable) when used". Example: boolean/unset `--dry-run` deprecated in 1.18, string form required later | 12 mo / 2 releases | `[S76, S65]` |
| docker | "labeled deprecated … remains for at least one stable release"; status table Deprecated/Removed with "Deprecated in"/"Remove" columns; old verb-first commands stay as aliases (`docker ps`) | ≥ 1 release; aliases effectively permanent | `[S77, S23]` |
| wrangler | Docs page "Deprecations" per major: `publish`, `pages publish`, `generate` "deprecated, but still active in v3 … fully removed in v4"; options `--local`, `--persist`, `--node-compat` etc.; code: `Metadata.deprecated/deprecatedMessage/hidden/status`, and per-arg `deprecated` | one major | `[S78, S54]` |
| Biome | 1.x added `--write`/`--fix` to `lint|check|format|migrate`, declared `--apply`/`--apply-unsafe` deprecated ("has the same behavior as"); 2.0 ships `biome migrate` to rewrite config for removed options (`ignore`/`include` → `includes`, `--config-path` removed) | one major + migrate tool | `[S79, S80]` |
| terraform | `taint` page: "This command is deprecated. Instead, add the `-replace` option to your `terraform apply`"; v1 compat promises list what is and is not promised ("Commands That Might Change") | per major | `[S81, S41]` |
| cf (beta) | breaking renames **without aliases** (camelCase/snake_case commands → kebab-case: `cf accounts billing getCredits` → `get-credits`); `--notification-email` flag removed; stderr/stdout split changed "Breaking change during the beta"; changeset per release; API-deprecated operations are **skipped from the CLI entirely** ("they'll break at sunset; surfacing them encourages users to adopt doomed paths") | none (beta) | `[S82, S27]` |
| AWS | v1→v2 migration guide enumerates breaking changes | per major | `[S83]` |

### 6.2 Majority / outliers
Majority: old spelling keeps working *and warns*, for at least one release (docker, kubectl, wrangler, gh, Biome). Outlier: cf beta hard cut — acceptable only because pre-1.0 and changeset-announced. Nobody can warn through a **JSON shape** change (kubectl/terraform avoid it by versioning the format: kubectl `apiVersion` in objects, terraform `format_version`) [INFERENCE for kubectl's apiVersion; terraform verified `[S16]`].

### 6.3 Recommendation: one release (0.28), honestly staged
The owner wants one release, not four. Both are compatible if "one release" means one *grammar change set*:
1. **0.28 ships every rename (commands + options) with the old spelling still accepted**: clap `alias`/hidden, stderr deprecation line, `meta.deprecated:{since,replacement}`, schema `deprecated`/`deprecatedAliases` (wrangler `Metadata.deprecated/deprecatedMessage`, kubectl Rule 6, pflag `MarkDeprecated` `[S54, S76, S74]`). Cost: one attribute per rename; the plan's `removed.rs` replacement-naming error then fires only in the *next* minor. Removal minor ≥ 1 release later (docker/kubectl floor `[S77, S76]`); no more grammar changes in that removal release, so it is a cleanup, not a fifth grammar release.
2. **Exceptions that cannot warn → hard cut with a precise error in the same 0.28:** the 13 bare-report → `data` envelope changes (JSON shapes), `brief check` exit flip, and any *same name, different meaning* change (the new `format`). Mitigate as terraform/cargo do: bump `schemaVersion` major, document old→new key list in the README Upgrading list, ship a **caller migration command** (Biome `migrate`, cf `cf migrate` codemod `[S80, S84]`; canon already plans `gate install-hooks` rewriting hooks). Same-name-new-meaning (`format`) gets the explicit-mode guard (§1.3).
3. If the owner insists on *no* aliases (pure hard cut), cite cf (beta kebab-case rename, no alias `[S82]`) and require: removed spelling → exit 2 `USAGE` with `error.replacement` and a working invocation. [INFERENCE: no surveyed big CLI does a systematic removed-spelling table; this is canon's own design and cheap in clap.]
4. **Never** rename and change JSON shape of the same command in one release (plan D7 already says so) — kubectl Rule 5c's spirit (replacement must be at least as stable) `[S76]`.
5. Docs: keep a single dated "Deprecations" table (docker status table, wrangler page `[S77, S78]`) generated from the schema's `deprecated` fields, so it cannot drift either (§3).

---

## 7. Other findings worth knowing (not an axis, but relevant to canon)

- **Harness env vars exist and are documented per harness** (cf `agent-context.ts`, each detector has an "Evidence:" link): `CLAUDECODE=1` + `CLAUDE_CODE_SESSION_ID`, `CODEX_THREAD_ID`, `CURSOR_AGENT=1` + `CURSOR_CONVERSATION_ID`/`CURSOR_TRACE_ID`, `AMP_CURRENT_THREAD_ID`, `PI_CODING_AGENT=true` + `PI_SESSION_ID`/`PI_MODEL`, `QWEN_CODE_SESSION_ID` `[S85]`. Canon can *record* `sessionId`/harness from these when writing review/evidence records (value comes from the environment, provenance stays "reported by the process environment, unsigned"). Do **not** switch behavior on them: cf rewrites `--help` when an agent is detected (outlier) and oxlint's switch caused #27569 `[S85, S40]`.
- **cf's `schema` + `agent-context` + `context` triple** maps onto canon's `schema` (structure) + `context` (task-scoped) + `status` (state): `cf context set account-id …` is *defaults*, `cf agent-context <product>` is *task-scoped command + notes*, `cf schema <cmd>` is the *request schema* `[S86]`. Confirms the plan's split; the naming collision canon has (`context` vs `context-pack`, C16) is the same word cf uses for "default account/zone".
- **Wrangler itself is mid-migration to cf** with a doc component that pairs each wrangler command with its reviewed `cf` equivalent (`cfCommand`) `[S56]` — a precedent for publishing an old→new command map from data.
- **Terraform's `-chdir` global before the subcommand** is the same shape as canon `--repo` (context, global, one spelling) `[S24]`.

---

## Source list (all fetched 2026-10-22 unless noted)

[S1] rustfmt README: https://raw.githubusercontent.com/rust-lang/rustfmt/master/README.md ("Verifying code is formatted")
[S2] cargo-fmt: https://doc.rust-lang.org/cargo/commands/cargo-fmt.html
[S3] cargo check: https://doc.rust-lang.org/cargo/commands/cargo-check.html
[S4] Clippy usage: https://doc.rust-lang.org/clippy/usage.html
[S5] cargo fix: https://doc.rust-lang.org/cargo/commands/cargo-fix.html
[S6] Biome CLI reference: https://biomejs.dev/reference/cli/ (commands check/lint/format/ci, `--write|--fix`, `--unsafe`, `--reporter`, `--error-on-warnings`)
[S7] oxfmt CLI: https://oxc.rs/docs/guide/usage/formatter/cli.html
[S8] oxlint CLI: https://oxc.rs/docs/guide/usage/linter/cli.html ; output formats https://oxc.rs/docs/guide/usage/linter/output-formats.html
[S9] oxlint exit mapping: https://raw.githubusercontent.com/oxc-project/oxc/main/apps/oxlint/src/result.rs
[S10] Prettier CLI: https://prettier.io/docs/cli
[S11] ESLint CLI exit codes: https://eslint.org/docs/latest/use/command-line-interface#exit-codes
[S12] ruff formatter: https://docs.astral.sh/ruff/formatter/
[S13] ruff linter exit codes: https://docs.astral.sh/ruff/linter/#exit-codes
[S14] deno fmt: https://docs.deno.com/runtime/reference/cli/fmt/
[S15] terraform fmt: https://developer.hashicorp.com/terraform/cli/commands/fmt
[S16] terraform validate (+ JSON format_version rules): https://developer.hashicorp.com/terraform/cli/commands/validate
[S17] terraform plan (`-out`, `-detailed-exitcode`): https://developer.hashicorp.com/terraform/cli/commands/plan
[S18] gh exit codes: https://cli.github.com/manual/gh_help_exit-codes
[S19] gws README (exit codes, architecture, JSON errors): https://raw.githubusercontent.com/googleworkspace/cli/main/README.md
[S20] AWS CLI return codes: https://docs.aws.amazon.com/cli/latest/userguide/cli-usage-returncodes.html
[S21] gh command-line syntax doc: https://raw.githubusercontent.com/cli/cli/trunk/docs/command-line-syntax.md
[S22] kubectl reference (syntax, output options): https://kubernetes.io/docs/reference/kubectl/
[S23] docker container ls (aliases, `--format`): https://docs.docker.com/reference/cli/docker/container/ls/
[S24] terraform CLI index (`-chdir`, command list): https://developer.hashicorp.com/terraform/cli/commands
[S25] wrangler general commands + global flags: https://developers.cloudflare.com/workers/wrangler/commands/general/
[S26] wrangler d1 commands (`--json` help strings): https://developers.cloudflare.com/workers/wrangler/commands/d1/ ; preview commands https://developers.cloudflare.com/workers/wrangler/commands/workers/
[S27] cloudflare/cf AGENTS.md (generator, DO NOT EDIT, global flags, reserved names, confirmation, deprecated methods, hand-written exceptions): https://raw.githubusercontent.com/cloudflare/cf/main/AGENTS.md ; README https://raw.githubusercontent.com/cloudflare/cf/main/README.md ; CI https://raw.githubusercontent.com/cloudflare/cf/main/.github/workflows/ci.yml ; package.json https://raw.githubusercontent.com/cloudflare/cf/main/package.json
[S28] cf command metadata types: https://raw.githubusercontent.com/cloudflare/cf/main/packages/cli/src/lib/command-meta.ts
[S29] flyctl docs: https://fly.io/docs/flyctl/status/ , https://docs.fly.io/flyctl/cmd/fly_deploy , https://fly.io/docs/flyctl/apps-destroy/
[S30] AWS CLI user guide (botocore-built; skeleton/input): https://docs.aws.amazon.com/cli/latest/userguide/cli-usage-skeleton.html ; botocore models https://github.com/boto/botocore/tree/develop/botocore/data
[S31] gws README (same URL as S19)
[S32] Cloudflare blog, "Building a CLI for all of Cloudflare" (2026-04-13): https://blog.cloudflare.com/cf-cli-local-explorer/
[S33] gh environment: https://cli.github.com/manual/gh_help_environment
[S34] kubectl usage conventions: https://kubernetes.io/docs/reference/kubectl/conventions/
[S35] gh formatting: https://cli.github.com/manual/gh_help_formatting
[S36] AWS output format: https://docs.aws.amazon.com/cli/latest/userguide/cli-usage-output-format.html
[S37] terraform machine-readable UI: https://developer.hashicorp.com/terraform/internals/machine-readable-ui
[S38] wrangler system environment variables (`WRANGLER_OUTPUT_FILE_PATH`): https://developers.cloudflare.com/workers/wrangler/system-environment-variables/
[S39] cf output helper: https://raw.githubusercontent.com/cloudflare/cf/main/packages/cli/src/lib/output.ts
[S40] oxc issue #27569 (agent format breaks `--rules`): https://github.com/oxc-project/oxc/issues/27569 ; agent-formatter proposal https://github.com/oxc-project/oxc/issues/20514
[S41] terraform v1 compatibility promises: https://developer.hashicorp.com/terraform/language/v1-compatibility-promises
[S42] gh docs generator: https://raw.githubusercontent.com/cli/cli/trunk/cmd/gen-docs/main.go
[S43] `gh help reference`: https://cli.github.com/manual/gh_help_reference
[S44] docker docs generator: https://raw.githubusercontent.com/docker/cli/master/docs/generate/generate.go
[S45] docker CI validate (regenerate + `git diff --stat`): https://raw.githubusercontent.com/docker/cli/master/.github/workflows/validate.yml
[S46] cargo man validation: https://raw.githubusercontent.com/rust-lang/cargo/master/ci/validate-man.sh
[S47] cargo `verify_cli`: https://raw.githubusercontent.com/rust-lang/cargo/master/src/bin/cargo/cli.rs
[S48] clap_mangen / clap_complete: https://docs.rs/clap_mangen/latest/clap_mangen/ , https://docs.rs/clap_complete/latest/clap_complete/
[S49] oclif: https://raw.githubusercontent.com/oclif/oclif/main/README.md (`oclif manifest`, `oclif readme`); `--json` flag https://oclif.io/docs/json/
[S50] kubectl explain: https://kubernetes.io/docs/reference/kubectl/generated/kubectl_explain/
[S51] aws s3 rm (`--dryrun`): https://docs.aws.amazon.com/cli/latest/reference/s3/rm.html
[S52] aws ec2 terminate-instances (`--dry-run`): https://docs.aws.amazon.com/cli/latest/reference/ec2/terminate-instances.html
[S53] AWS CLI v1→v2 changes: https://docs.aws.amazon.com/cli/latest/userguide/cliv2-migration-changes.html
[S54] wrangler command types: https://raw.githubusercontent.com/cloudflare/workers-sdk/main/packages/wrangler/src/core/types.ts
[S55] wrangler registry: https://raw.githubusercontent.com/cloudflare/workers-sdk/main/packages/wrangler/src/core/CommandRegistry.ts
[S56] cloudflare-docs `WranglerCommand` component: https://developers.cloudflare.com/style-guide/components/wrangler-command/
[S57] cf agent-context: https://raw.githubusercontent.com/cloudflare/cf/main/packages/cli/src/lib/agent-context.ts ; Zenn tour of `cf` (secondary) https://zenn.dev/temple_slope/articles/2026-05-24_cf-cli-local-explorer?locale=en ; PyShine source tour (secondary, matched against primary) https://pyshine.com/Cloudflare-CF-Agentic-CLI-Source-Tour/
[S58] terraform JSON format: https://developer.hashicorp.com/terraform/internals/json-format
[S59] cargo metadata: https://doc.rust-lang.org/cargo/commands/cargo-metadata.html
[S60] wrangler output entries: https://raw.githubusercontent.com/cloudflare/workers-sdk/main/packages/workers-utils/src/output.ts
[S61] cf error rendering: https://raw.githubusercontent.com/cloudflare/cf/main/packages/cli/src/lib/errors.ts
[S62] terraform apply (saved plan = approval): https://developer.hashicorp.com/terraform/cli/commands/apply
[S63] kubectl delete: https://kubernetes.io/docs/reference/kubectl/generated/kubectl_delete/
[S64] kubectl diff exit status: https://kubernetes.io/docs/reference/kubectl/generated/kubectl_diff/
[S65] k8s 1.18 release notes (`--dry-run` boolean deprecated, PR #87580): https://github.com/kubernetes/sig-release/blob/master/releases/release-1.18/release-notes-draft.md
[S66] gh `repo delete` source: https://raw.githubusercontent.com/cli/cli/trunk/pkg/cmd/repo/delete/delete.go ; manual https://cli.github.com/manual/gh_repo_delete
[S67] docker system prune: https://docs.docker.com/reference/cli/docker/system/prune/
[S68] cf CHANGELOG (destructive non-interactive exit 1; kebab-case break; stderr break): https://raw.githubusercontent.com/cloudflare/cf/main/packages/cli/CHANGELOG.md  (S70 = same file)
[S69] cf dry-run: https://raw.githubusercontent.com/cloudflare/cf/main/packages/cli/src/lib/dry-run.ts
[S70] see S68
[S71] fly deploy/destroy docs (S29)
[S72] see S51
[S73] see S52
[S74] pflag `MarkDeprecated`: https://raw.githubusercontent.com/spf13/pflag/master/flag.go
[S75] cobra `Command.Deprecated`/`Hidden`: https://raw.githubusercontent.com/spf13/cobra/main/command.go
[S76] Kubernetes deprecation policy (Rules 5a–5c, 6): https://kubernetes.io/docs/reference/using-api/deprecation-policy/
[S77] Docker deprecated features + policy: https://docs.docker.com/engine/deprecated/
[S78] Wrangler deprecations page: https://developers.cloudflare.com/workers/wrangler/deprecations/
[S79] Biome v1 changelog (`--apply` → `--write/--fix`): https://biomejs.dev/internals/changelog_v1/
[S80] Biome v2 upgrade guide: https://biomejs.dev/guides/upgrade-to-biome-v2/
[S81] terraform taint (deprecated): https://developer.hashicorp.com/terraform/cli/commands/taint
[S82] cf CHANGELOG (S68)
[S83] AWS v1→v2 (S53)
[S84] cf `wrangler-migration.ts`: https://raw.githubusercontent.com/cloudflare/cf/main/packages/cli/src/lib/wrangler-migration.ts
[S85] cf agent-context (S57)
[S86] cf command list in the Zenn tour (S57)

Local evidence: `canon format --help`, `canon schema format --json` (installed 0.25.0, run in `<scratch>`); `crates/canon-fmt/src/lib.rs:1-12`; audit-surface X1–X9, C7, C8, O7, O19, D1–D8 (origin/docs/cli-grammar-plan); plan D2–D8, Q1–Q5.

Not verified (so not relied on): Biome exit codes (reference page silent); oxfmt exit codes (silent); terraform stale-saved-plan error text; cargo `publish --dry-run` (not fetched); kubectl `apiVersion`-as-format-versioning; any systematic "removed spelling → replacement" table in a big CLI.
