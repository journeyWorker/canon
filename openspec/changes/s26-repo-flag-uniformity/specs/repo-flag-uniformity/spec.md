## ADDED Requirements

### Requirement: `canon fmt` accepts an optional `--repo <REPO>`, resolving `<ROOT>` under it
`canon fmt --check <root>` SHALL continue to accept `<root>` as a required
positional, resolved exactly as today, when `--repo` is omitted. `canon fmt
--check <root> --repo <repo>` SHALL additionally be accepted: `--repo`
SHALL resolve through the SAME `canon_cli::context::resolve_repo_root`
ancestor walk every other `--repo`-accepting verb uses (`--repo .` walks
`cwd.ancestors()` for the nearest `canon.yaml`; any other explicit `--repo
<dir>` is used as-is), and the corpus directory actually checked SHALL be
the resolved repo root joined with `<root>`. Omitting `--repo` SHALL NEVER
invoke the ancestor walk or otherwise alter how `<root>` resolves.

#### Scenario: The bare positional form still succeeds unchanged
- **WHEN** `canon fmt --check <root>` runs (no `--repo`) against a corpus
  at `<root>`
- **THEN** the command validates `<root>` exactly as it did before this
  change and exits with the identical status/output shape

#### Scenario: `--repo` combined with the positional root succeeds
- **WHEN** `canon fmt --check <root> --repo <repo>` runs, where `<root>` is
  a path relative to `<repo>` (e.g. `spec`)
- **THEN** the command resolves `<repo>` via `resolve_repo_root` and
  validates `<resolved-repo>/<root>`, producing the same diagnostics
  `cd <repo> && canon fmt --check <root>` would have produced

### Requirement: `canon tier age` accepts an optional `--repo <REPO>`, matching every other config-reading verb
`canon tier age [--dry-run]` SHALL accept `--repo <REPO>`, resolved via the
SAME `repo` + `canon_yaml` precedence `canon query` already ships: an
explicit `--canon-yaml <path>` SHALL bypass `--repo` entirely and be read
AS-IS, regardless of `--repo`; when `--canon-yaml` is omitted, the
`canon.yaml` actually loaded SHALL be `resolve_repo_root(--repo).join
("canon.yaml")`. `--repo` SHALL default to `.`, matching every sibling
verb's own default. Omitting BOTH `--repo` and `--canon-yaml` (today's only
supported invocation shape) SHALL continue to succeed whenever it succeeds
today.

#### Scenario: `--repo` succeeds for a dry-run
- **WHEN** `canon tier age --dry-run --repo <repo>` runs against a repo
  whose `canon.yaml` lives at `<repo>/canon.yaml` and declares an `aging`
  rule
- **THEN** the command loads `<repo>/canon.yaml` via the `resolve_repo_root`
  walk and reports the same dry-run preview `cd <repo> && canon tier age
  --dry-run` would have produced

#### Scenario: The bare CWD-default form still succeeds unchanged
- **WHEN** `canon tier age --dry-run` runs (no `--repo`, no
  `--canon-yaml`) from a directory whose own `canon.yaml` is present
- **THEN** the command loads that `canon.yaml` and behaves exactly as it
  did before this change

#### Scenario: An explicit --canon-yaml still overrides --repo
- **WHEN** `canon tier age --canon-yaml <path>` runs, with or without a
  `--repo` also supplied
- **THEN** the command reads `<path>` AS-IS, ignoring `--repo` entirely —
  byte-identical to every existing `--canon-yaml`-only invocation (e.g.
  `crates/canon-cli/tests/tier_age.rs`'s fixture calls) before this change

### Requirement: `canon scenario new` accepts an `@`-prefixed tag as equivalent to the bare form
`canon scenario new <tag> --title <label> [--repo <repo>]` SHALL accept
`<tag>` written either as the bare form (`story.x.01`) or with a single
leading `@` (`@story.x.01`) — the spelling used inside scenario bodies
(`Scenario: @story.x.01`). Both spellings SHALL parse to the identical
`ScenarioId` and SHALL therefore write the identical `Scenario:` header and
resolve the identical target `.feature` path. A tag that is malformed
after stripping at most one leading `@` SHALL still be refused (clap usage
error, exit `2`), identically to today's bare-form refusal. This
normalization SHALL be confined to `canon scenario new`'s own tag
argument; `ScenarioId::parse`'s grammar as used by every other call site
(gate evidence matching, inventory sync, query scope filters) SHALL remain
unchanged.

#### Scenario: An @-prefixed tag writes the same header a bare tag would
- **WHEN** `canon scenario new @story.x.01 --title T --repo .` runs
  against a repo with one configured spec root
- **THEN** the command writes the identical `Scenario: @story.x.01` header,
  at the identical target `.feature` path, that `canon scenario new
  story.x.01 --title T --repo .` would have written

#### Scenario: The bare form still succeeds unchanged
- **WHEN** `canon scenario new story.x.02 --title T --repo .` runs (no `@`
  prefix)
- **THEN** the command behaves exactly as it did before this change

#### Scenario: A malformed tag is still refused after stripping a leading @
- **WHEN** `canon scenario new @Story.X.01 --title T --repo .` runs (an
  uppercase segment, invalid under `ScenarioId`'s grammar either way)
- **THEN** the command refuses with a clap usage error and exits `2`,
  identically to `canon scenario new Story.X.01 --title T --repo .`
