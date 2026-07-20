## Why

The `loom` round-3 dogfood (`target/usage-review/loom/SYNTHESIS-ROUND3.md`)
re-ran the same four personas against canon's first LIVE multi-tier repo and
re-surfaced a `--repo`-convention outlier that is **growing, not
shrinking**: "Persisting / minor findings" F3 (persists from round 2,
unfixed) and F4 (new this round) name a second CLI verb rejecting `--repo`,
and F5 names a related tag-grammar leniency gap. All three are cheap,
additive CLI-surface fixes over already-existing resolution machinery — no
new architecture, no `canon.yaml` schema change.

**F3 (persists) — `canon fmt` takes positional `<ROOT>`, rejects `--repo`;
every other verb takes `--repo`.** Verified at
`crates/canon-cli/src/main.rs:133-140`: `Command::Fmt { check: bool, root:
PathBuf }` has no `repo` field at all — `canon fmt --check spec --repo
.` is a clap "unexpected argument" error (exit `2`), the ONLY canon
subcommand with zero acceptance of `--repo` in ANY form. Every sibling verb
(`query`, `context`, `gate check`, `ingest plans`, `scenario new`, `feature
new`, …) accepts `--repo <dir>` resolved via the shared
`canon_cli::context::resolve_repo_root` ancestor walk
(`context.rs:247-253`: `--repo == "."` walks `cwd.ancestors()` for the
nearest `canon.yaml`; any other explicit `--repo <dir>` is used as-is).
`canon fmt` is the one command a fresh dogfooder reaches for FIRST after
`feature new`/`scenario new` (the `najun-art-dummy` round-1/round-2 core
loop), so the outlier is maximally visible.

**F4 (new) — `canon tier age` also rejects `--repo` (CWD-only).** Verified
at `main.rs:873-887`: `TierCommand::Age { dry_run: bool, canon_yaml:
PathBuf }` (`default_value = "canon.yaml"`, a literal path joined at the
process CWD — no ancestor walk, no `--repo` field). Every other config-
reading verb resolves its `canon.yaml` through `--repo` +
`resolve_repo_root`, most directly `canon query`'s own
`Query.repo`/`Query.canon_yaml` pair
(`main.rs:85-101`, `query.rs:253-262`'s `resolve_canon_yaml`: an explicit
`--canon-yaml` BYPASSES `--repo` entirely and wins outright; omitted, `--repo`'s
ancestor walk governs). `canon tier age` is the one remaining config-driven
verb outside that convention.

**F5 (minor) — `canon scenario new @story.render.01` is rejected; bare
`story.render.01` works.** Verified at `main.rs:811`/`scaffold.rs:106-108`:
`ScenarioCommand::New.tag`'s clap `value_parser` is
`canon_cli::scaffold::parse_scenario_tag`, which calls
`ScenarioId::parse` (`canon-model/src/ids.rs:330-337`) verbatim — `@` is not
part of `ScenarioId`'s grammar (`is_scenario_id`), so a copy-pasted `@tag`
(the spelling used INSIDE scenario bodies, e.g. `Scenario: @story.render.01`,
and what `feature new`'s own printed next-step hint shows) is rejected at
the clap boundary before `run_scenario_new` ever sees it.

**Scope discipline.** All three are additive `clap`-arg / value-parser
changes over EXISTING resolution functions (`resolve_repo_root`,
`resolve_canon_yaml`) and an existing grammar (`ScenarioId::parse`) — no new
`canon.yaml` section, no new `RecordKind`, no change to `canon gate check`'s
byte-identical checkbox authority, no live-pg read anywhere near
`canon-report` (that is a DIFFERENT, sibling change's F2 — see
`s27-report-pg-boundary-note` — not touched here).

## What Changes

- **`canon fmt` gains `--repo <REPO>` (additive; positional `<ROOT>`
  UNCHANGED).** `Command::Fmt` gains `#[arg(long)] repo: Option<PathBuf>`.
  Omitted (the default, i.e. every existing invocation): `root` is used
  EXACTLY as given, byte-identical to today — no new code path runs.
  Given: `repo` resolves through the SAME `resolve_repo_root` ancestor walk
  every sibling verb uses (`--repo .` walks for the nearest `canon.yaml`;
  any other explicit `--repo <dir>` is used as-is), and the corpus root
  actually checked becomes `resolve_repo_root(repo).join(root)` — `root`
  stays the corpus-relative suffix (e.g. `spec`), `--repo` supplies the
  base a dogfooder would otherwise need to `cd` into first. `root` remains
  a REQUIRED positional either way; `--repo` never substitutes for it.
- **`canon tier age` gains `--repo <REPO>` (additive; CWD-default
  behavior preserved when `--repo` is omitted).** `TierCommand::Age` gains
  `#[arg(long, default_value = ".")] repo: PathBuf`, and its existing
  `canon_yaml: PathBuf` field (`default_value = "canon.yaml"`) becomes
  `canon_yaml: Option<PathBuf>` (no default) — the IDENTICAL `repo`/
  `canon_yaml` override pair `canon query` already ships, resolved through
  the SAME `resolve_canon_yaml`-shaped logic (`query.rs:253-262`): an
  explicit `--canon-yaml` bypasses `--repo` and wins outright (so today's
  `--canon-yaml <path>` invocations, including every existing
  `crates/canon-cli/tests/tier_age.rs` fixture call, are byte-identical);
  omitted, `resolve_repo_root(repo).join("canon.yaml")` governs. When
  `--repo` is also omitted (`"."`, the default) and the process CWD itself
  carries a `canon.yaml` (today's only supported shape), the ancestor walk
  returns CWD immediately — identical resolved file, same as today. The
  walk only changes outcome for a CWD *without* its own `canon.yaml` but
  with an ancestor that has one — a case that fails today and now
  succeeds, matching every sibling verb's own already-shipped convention;
  it never turns a working invocation into a failing one.
- **`canon scenario new <tag>` accepts an optional leading `@` on `<tag>`
  (additive; bare form UNCHANGED).** `canon_cli::scaffold::parse_scenario_tag`
  strips ONE leading `@` (`s.strip_prefix('@').unwrap_or(s)`) before handing
  the rest to `ScenarioId::parse` verbatim — `ScenarioId::parse` ITSELF is
  untouched (its grammar stays `@`-free everywhere else it's called: gate
  evidence matching, inventory sync, query scoping), so this is a single,
  CLI-argument-boundary normalization, not a model-grammar change. `@story
  .render.01` and `story.render.01` produce the IDENTICAL `ScenarioId` and
  therefore the identical `Scenario:` header write; a malformed tag (with
  or without `@`) is refused exactly as today (clap usage error, exit `2`).

### Added Capabilities

- `repo-flag-uniformity`: `canon fmt --repo`, `canon tier age --repo`, and
  `canon scenario new @tag` — three additive CLI-surface fixes closing the
  round-3 `--repo`/tag-grammar outliers (F3, F4, F5), each verified against
  the actually-built `canon` binary.

### Explicit non-goals

- **No fix for F2** (the report's structural blindness to the pg tier) —
  that is a genuine architecture-level gap with its own operator-locked
  resolution (a config-derived, drift-safe boundary note, never a live-pg
  read from `canon-report`) and belongs to a separate, sibling change. This
  change touches `canon fmt`, `canon tier age`, and `canon scenario new`
  ONLY.
- **No change to `canon fmt`'s validation behavior, exit codes, or
  `FmtFailureClass` set.** `--repo` only affects WHICH directory `root` is
  resolved against before `canon_fmt::check` runs; `canon_fmt::check`
  itself is called with the identical resolved `Path` shape as today
  (`fmt.rs::run(root: &Path)` unchanged).
- **No change to `canon tier age`'s aging algorithm, `TierRegistry::
  age_all` semantics, or its all-or-nothing failure contract** (s22's
  design D3: aging deliberately stays strict, never lenient, unlike
  `query`). `--repo`/`--canon-yaml` only change WHICH `canon.yaml` is
  loaded before aging runs; the loaded policy is consumed identically to
  today.
- **No change to `ScenarioId`'s stored/serialized grammar or to
  `ScenarioId::parse`'s behavior anywhere else it's called** (gate
  evidence, inventory sync, query `--change-id`/scope filters, every
  existing fixture asserting a bare, `@`-free `ScenarioId`). The `@`-strip
  is confined to `canon_cli::scaffold::parse_scenario_tag`, the ONE clap
  `value_parser` for `canon scenario new`'s positional `<tag>` argument.
- **No `--repo` alias/flag added to any OTHER command** — `canon fmt`,
  `canon tier age`, and `canon scenario new` (already had `--repo` before
  this change; only the tag grammar changes there) are the full, closed
  set this change touches.
- **No live-pg read added anywhere; `canon gate check` stays
  byte-identical; the closed 12-`RecordKind` set is unchanged.**

## Impact

- **`canon-cli`**: `src/main.rs` (`Command::Fmt.repo: Option<PathBuf>`,
  `TierCommand::Age.repo: PathBuf` + `.canon_yaml: Option<PathBuf>`, the
  `run_fmt`/`run_tier_age` dispatch signatures threading the new/changed
  fields through), `src/fmt.rs` or `src/main.rs`'s `run_fmt` (root
  resolution: `resolve_repo_root(repo).join(root)` when `--repo` is
  `Some`, `root` as-is otherwise), `src/tier.rs` or `src/main.rs`'s
  `run_tier_age` (a `resolve_canon_yaml`-shaped helper mirroring
  `query.rs:253-262`, reused rather than re-derived — see design.md D2),
  `src/scaffold.rs` (`parse_scenario_tag`'s one-line `@`-strip).
- **`canon-model`**: UNCHANGED — `ScenarioId::parse`/`is_scenario_id` are
  not touched.
- **`canon-fmt` / `canon-store` / `canon-gate` / `canon-learn` /
  `canon-vocab` / `canon-plugin` / `canon-ingest` / `canon-report`**:
  UNCHANGED.
- **Tests**: `crates/canon-cli/tests/fmt_check.rs` (new case: `--repo
  <dir> <corpus-relative-root>` resolves and checks the identical corpus
  the existing bare-positional case does; existing bare-positional case
  re-asserted byte-identical), `tests/tier_age.rs` (new case: `--repo
  <fixture-root>` without `--canon-yaml` succeeds identically to the
  existing `--canon-yaml <path>`-only fixture calls; every EXISTING test
  in this file re-asserted passing unmodified, since `support::Fixture::
  run_canon` always supplies an explicit `--canon-yaml` that keeps
  bypassing `--repo`), `tests/scaffold.rs` (new case: `@story.x.01` and
  `story.x.01` write the identical `Scenario: @story.x.01` header; a
  malformed `@` tag, e.g. `@Story.X.01`, still refuses exit `2`).
