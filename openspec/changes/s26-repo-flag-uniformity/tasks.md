# Tasks — s26 repo-flag uniformity

The three fixes touch three different subcommands and share no code path
with each other (only `resolve_repo_root`, already shared, is common) — no
sequencing dependency forces one before another; all three may land in any
order or in parallel.

## 1. `canon fmt --repo` (F3)

- [x] 1.1 `canon-cli/src/main.rs`: `Command::Fmt` gains `#[arg(long)] repo:
      Option<PathBuf>` alongside the existing `check: bool` and `root:
      PathBuf` fields; `root` stays a required positional, unchanged.
- [x] 1.2 `main.rs`'s `run_fmt` (or a small helper in `src/fmt.rs`):
      when `repo` is `Some(r)`, resolve the corpus path as
      `canon_cli::context::resolve_repo_root(&r).join(&root)`; when `repo`
      is `None`, use `root` exactly as given (no new function call on this
      path) — `canon_fmt::check`'s own signature (`fmt.rs::run(root: &Path)
      -> FmtReport`) is unchanged; only the `Path` handed to it differs.
- [x] 1.3 Tests (`crates/canon-cli/tests/fmt_check.rs`): a new case running
      `canon fmt --check <corpus-relative-root> --repo <repo-dir>` against
      the existing `canon-fmt` fixture corpus (splitting the fixture root
      into a `--repo` base + relative suffix) asserts identical stdout/exit
      code to the existing bare-positional case; the existing bare-
      positional test re-run and confirmed byte-identical (no regression).

## 2. `canon tier age --repo` (F4)

- [x] 2.1 `canon-cli/src/main.rs`: `TierCommand::Age.canon_yaml` changes
      from `PathBuf` (`default_value = "canon.yaml"`) to `Option<PathBuf>`
      (no default); `Age` gains `#[arg(long, default_value = ".")] repo:
      PathBuf`.
- [x] 2.2 Introduce (or reuse, raising visibility to `pub(crate)`) a
      `resolve_canon_yaml(repo: &Path, canon_yaml: Option<&Path>) ->
      PathBuf` helper matching `query.rs:253-262`'s existing logic
      byte-for-byte (`Some(path) => path.to_path_buf()`; `None =>
      resolve_repo_root(repo).join("canon.yaml")`) — prefer relocating
      `query.rs`'s existing private fn to a shared location (e.g.
      `context.rs` or `tiers.rs`) and calling it from both `query.rs` and
      the tier-age dispatch, over duplicating the two-line body verbatim.
- [x] 2.3 `main.rs`'s `run_tier_age` (or `src/tier.rs::run`): thread `repo:
      &Path` and `canon_yaml: Option<&Path>` through, resolving the actual
      `canon.yaml` path via 2.2's helper BEFORE calling
      `canon_cli::tier::run` — `tier.rs::run`'s own signature (`canon_yaml:
      &Path, dry_run: bool`) stays unchanged; only the caller's resolution
      step changes.
- [x] 2.4 Tests (`crates/canon-cli/tests/tier_age.rs`): a new case running
      `canon tier age --dry-run --repo <fixture-root>` (no `--canon-yaml`)
      asserts identical stdout to the existing `--canon-yaml <path>`-only
      dry-run case; every EXISTING test in this file re-run and confirmed
      passing UNMODIFIED (each already supplies an explicit `--canon-yaml`
      via `support::Fixture::run_canon`, which must keep bypassing
      `--repo` per 2.2's precedence).

## 3. `canon scenario new @tag` (F5)

- [x] 3.1 `canon-cli/src/scaffold.rs::parse_scenario_tag`: strip at most one
      leading `@` (`s.strip_prefix('@').unwrap_or(s)`) before calling
      `ScenarioId::parse` — `ScenarioId::parse`/`is_scenario_id`
      (`canon-model/src/ids.rs`) themselves stay untouched; the strip is
      local to this one `clap` `value_parser` function.
- [x] 3.2 Tests (`crates/canon-cli/src/scaffold.rs`'s existing `mod tests`
      and/or `crates/canon-cli/tests/scaffold.rs`): `parse_scenario_tag
      ("@story.x.01")` and `parse_scenario_tag("story.x.01")` produce the
      identical `ScenarioId`; an integration test running `canon scenario
      new @story.x.01 --title T --repo .` asserts the written `.feature`
      file's `Scenario:` header is byte-identical to what the bare-tag
      invocation produces; `parse_scenario_tag("@Story.X.01")` (or the
      equivalent CLI invocation) still returns/refuses an error, matching
      the bare malformed-tag case's existing refusal.

## 4. Verification

- [ ] 4.1 `cargo build --workspace` + `cargo clippy --workspace
      --all-targets -- -D warnings` + `cargo test --workspace
      --no-fail-fast` (bare, no pipe masking) all green.
- [ ] 4.2 `bunx openspec validate --strict s26-repo-flag-uniformity` green.
- [ ] 4.3 `canon selftest` (if applicable to this repo's fixture oracle)
      still green — none of the three fixes touch a suite it pins.
- [x] 4.4 Re-confirm every EXISTING `fmt_check.rs`/`tier_age.rs`/
      `scaffold.rs` test (predating this change) still passes with zero
      modification to its own assertions — each fix's default/omitted-flag
      path is required to be a true no-op, not merely "probably fine".
