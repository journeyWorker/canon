# Design — s26 repo-flag uniformity

## Current state (accurate baseline, verified)

- **`canon_cli::context::resolve_repo_root`** (`context.rs:247-253`) is the
  ONE shared ancestor-walk function every `--repo`-accepting verb already
  calls: `repo != "."` → used as-is (no walk); `repo == "."` (the shared
  default) → `cwd.ancestors().find(|d| d.join("canon.yaml").is_file())`,
  falling back to `cwd` itself when no ancestor has one.
- **`canon_cli::query::resolve_canon_yaml`** (`query.rs:253-262`) layers an
  EXPLICIT override on top: `Some(path) => path.to_path_buf()` (bypasses
  `--repo` entirely, wins outright), `None =>
  resolve_repo_root(repo).join("canon.yaml")`. This is the pair `Command
  ::Query` exposes as `repo: PathBuf` (`default_value = "."`) +
  `canon_yaml: Option<PathBuf>` (no default).
- **`Command::Fmt`** (`main.rs:133-140`) has NEITHER — `root: PathBuf` is a
  bare required positional, no `--repo`, no ancestor walk, no
  `canon.yaml` involvement at all (`canon_fmt::check` never reads
  `canon.yaml` — `canon fmt` validates a corpus directory structurally,
  independent of tier/routing config).
- **`TierCommand::Age`** (`main.rs:873-887`) has `canon_yaml: PathBuf`
  (`default_value = "canon.yaml"`, always `Some`-shaped, joined literally
  at the process CWD — no `resolve_repo_root` call at all) and no `repo`
  field.
- **`ScenarioCommand::New`** (`main.rs:802-833`) ALREADY has `repo: PathBuf`
  (`default_value = "."`), resolved via `resolve_repo_root` inside
  `scaffold::run_scenario_new` — F5 is purely a tag-grammar gap, not a
  `--repo` gap; `parse_scenario_tag` (`scaffold.rs:106-108`) calls
  `ScenarioId::parse` verbatim, and `is_scenario_id`
  (`canon-model/src/ids.rs`) has no `@` in its grammar.
- **`crates/canon-cli/tests/tier_age.rs`'s `support::Fixture::run_canon`**
  (`tests/support/mod.rs:157-163`) appends `--canon-yaml <tmp-path>` to
  EVERY invocation unconditionally — every existing test in that file
  already takes the explicit-override path, never the CWD-default path.

## Decisions

- **D1 — `canon fmt --repo` is `Option<PathBuf>`, not a
  `default_value = "."` field, so the omitted case is a NO-OP, not an
  always-run ancestor walk.** Every other verb's `--repo` doubles as ITS
  OWN primary root argument (there is no separate positional), so
  `default_value = "."` triggering a walk on every invocation is safe —
  the walk's fallback (no ancestor has `canon.yaml` → use `cwd`) always
  reproduces the pre-existing "just use cwd" behavior for that verb.
  `canon fmt` is different: `root` is ALREADY the primary, explicit
  argument (usually a relative corpus path like `spec`), and running the
  walk unconditionally could resolve `root` against a DIFFERENT base than
  the CWD `root` was written relative to (e.g. `root` given relative to
  cwd, but an ancestor two levels up also happens to carry a stray
  `canon.yaml` — walking would silently rebase `root` onto the wrong
  directory). Making `--repo` genuinely optional (`None` when omitted)
  guarantees the omitted-flag code path is IDENTICAL to today's — `root`
  used exactly as given, zero new function calls — satisfying "NEVER
  break the working positional form" by construction, not by walk-fallback
  coincidence. When `--repo` IS given, `resolve_repo_root` is reused
  verbatim (same function, same "." triggers a walk / other value used
  as-is rule every sibling verb documents) so the FLAG's semantics still
  match every other verb's `--repo` — only its ABSENCE behaves specially,
  which is the correct shape for a modifier on top of an already-required
  positional.
- **D2 — `canon tier age` adopts `canon query`'s EXACT `repo` +
  `canon_yaml` shape, not a bespoke pair.** `TierCommand::Age.canon_yaml`
  changes from `PathBuf` (`default_value = "canon.yaml"`) to
  `Option<PathBuf>` (no default), and gains `repo: PathBuf`
  (`default_value = "."`) — reusing (or, if visibility requires, a
  byte-identical `pub(crate)` copy of) `query::resolve_canon_yaml`'s
  two-line resolution: explicit `--canon-yaml` wins outright regardless of
  `--repo`; omitted, `resolve_repo_root(repo).join("canon.yaml")` governs.
  Rejected alternative: keep `canon_yaml: PathBuf` with its current
  literal-join semantics and bolt `--repo` on as a separate, unrelated
  prefix — rejected because it would produce a THIRD resolution shape
  (`query` has one, `fmt` gains its own per D1, and this would be a
  third), defeating the "consistent with every other verb" goal the
  finding names explicitly. The chosen shape means every EXISTING
  `--canon-yaml <path>` invocation (including all of
  `tests/tier_age.rs`'s current fixture calls, which always pass it) stays
  on the untouched override arm — genuinely zero behavior change for every
  invocation shape in use today; only a bare, flagless `canon tier age`
  run from a `canon.yaml`-less subdirectory-of-a-project newly succeeds
  instead of failing (the ancestor-walk fallback still resolves to `cwd`
  when no ancestor has one, so a truly config-less directory keeps failing
  exactly as before).
- **D3 — the `@`-strip lives in `canon_cli::scaffold::parse_scenario_tag`
  (the clap `value_parser`), never in `ScenarioId::parse` or
  `is_scenario_id`.** `ScenarioId::parse` is called from multiple
  non-CLI-argument call sites across the workspace (gate evidence
  matching, inventory sync, query `--change-id` scoping) where an `@`
  prefix would be a genuine, silent grammar widening with no `clap`
  boundary to confine it — every one of those call sites expects a bare,
  already-normalized id. Confining the strip to ONE function, exercised
  ONLY as `ScenarioCommand::New.tag`'s `value_parser`
  (`main.rs:811`), means `canon scenario new`'s `<tag>` argument alone
  becomes lenient (`@story.x.01` and `story.x.01` are equivalent INPUT
  spellings) while `ScenarioId`'s actual stored/serialized/compared form
  is unchanged everywhere else — no new equivalence class is introduced
  into the model layer, only into this one argument's acceptance.

## Risks

- **R1 — `canon fmt --repo` must never change the resolved path for an
  invocation that omits `--repo`.** Mitigated by D1's `Option<PathBuf>`
  choice: the omitted-flag branch calls zero new code, so this is
  true by construction; the acceptance test asserts the EXISTING
  bare-positional `fmt_check.rs` case is untouched, byte-identical stdout.
- **R2 — `canon tier age`'s `--repo`-driven ancestor walk must never
  resolve a DIFFERENT `canon.yaml` than an explicit `--canon-yaml` would
  have when both happen to be present.** Mitigated by D2's reused
  precedence rule (`Some(path) => path.to_path_buf()`, unconditional,
  checked before any `--repo`/ancestor-walk logic runs at all) — identical
  to `query.rs`'s own already-shipped, already-tested precedence.
- **R3 — the scenario-tag `@`-strip must not accept a DOUBLE-`@`d or
  otherwise malformed tag.** Mitigated by stripping AT MOST one leading
  `@` (`strip_prefix`, not a `trim_start_matches` loop) and still routing
  the result through the UNCHANGED `ScenarioId::parse`/`is_scenario_id`
  grammar check — `@@story.x.01` strips to `@story.x.01`, which
  `is_scenario_id` still rejects (its grammar has no `@` at all), so the
  command still refuses it exit `2`, same as today.
