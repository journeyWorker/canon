## ADDED Requirements

### Requirement: canon/skills/ is the single source of truth for every spec's companion skill
Every companion skill SHALL be authored under `canon/skills/<name>/SKILL.md`
(design §5's cross-cutting deliverable, decision 9) — never directly under a consumer
repo's `.claude/` or `.codex/` tree.

#### Scenario: A skill authored once is installable into any consumer repo
- **WHEN** `canon skills install` runs inside a consumer repo (e.g. the donor monorepo)
  with `canon/skills/<name>/SKILL.md` present in the canon checkout it was
  invoked from
- **THEN** the consumer repo gains `.claude/skills/<name>/SKILL.md`
  (verbatim copy) and `.codex/skills/<name>.md` (canon's flattened
  convention, D4), and no other runtime directory is written (gemini is
  dropped per decision 11).

### Requirement: Materialization is deterministic and timestamp-free
`canon skills install` SHALL produce a lock recording each installed
skill's content hash and a monotonic version integer, and MUST NOT embed
wall-clock time (no `generatedAt` field), per decision 11.

#### Scenario: Re-running with no source changes is a byte-identical no-op
- **WHEN** `canon skills install` runs twice in a row with no change to any
  `canon/skills/**` file
- **THEN** every materialized file and the lock file are byte-identical
  across both runs, producing zero git diff on the second run.

#### Scenario: A content change bumps the version, not the timestamp
- **WHEN** `canon/skills/<name>/SKILL.md` content changes between two
  `canon skills install` runs
- **THEN** the lock's `contentHash` for `<name>` changes and its `version`
  integer increments by exactly one; unrelated skills' lock entries are
  untouched.

#### Scenario: Only Claude Code and Codex targets are materialized
- **WHEN** `canon skills install` runs in a repo that also has a `.gemini/`
  directory
- **THEN** no file under `.gemini/` is created, modified, or referenced by
  the install — gemini is out of scope per decision 11.
