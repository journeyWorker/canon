## ADDED Requirements

### Requirement: The canonical user-facing bundle has one provider-aware skill
The canonical user-facing source SHALL be authored at `canon/skills/SKILL.src.md`.
Its `reference/**` and `scripts/**` files SHALL remain lazy-loadable bundle
content. Legacy directory-shaped sources such as `canon/skills-dev` MAY retain
individual `<name>/SKILL.md` files for developer-only compatibility.

#### Scenario: A canonical bundle projects only the selected providers
- **WHEN** `canon skills install --source canon/skills --providers=claude,codex,omp,pi`
  runs inside a consumer repo
- **THEN** the consumer gains one Claude skill at
  `.claude/skills/canon/SKILL.md`, one Codex skill at
  `.codex/skills/canon.md`, one OMP skill at
  `.omp/skills/canon/SKILL.md`, and one Pi skill at
  `.pi/skills/canon/SKILL.md`, plus matching `reference/**` and `scripts/**`
  sidecars under each selected provider's `canon` bundle
- **AND** Claude, OMP, and Pi preserve the source frontmatter/body verbatim
- **AND** OMP/Pi projections are project-local passive bundles; the
  `canon-retrieve-pre-dispatch.sh` file is only a sidecar, not a native hook
- **AND** no old per-topic user skill directory/file is generated
- **AND** no `.agents/` or `.gemini/` file is created or modified.

#### Scenario: Provider selection and detection are deterministic
- **WHEN** `--providers=claude,codex,omp,pi` is supplied, or when existing
  `.claude`, `.codex`, `.omp`, and `.pi` targets are detected without the flag
- **THEN** only the selected/detected providers are projected in stable enum
  order (`claude`, `codex`, `omp`, `pi`)
- **AND** an invalid provider name fails before any target write.
- **AND** when none of the four target roots exists, Claude and Codex are
  selected for backwards compatibility.

### Requirement: Canonical installation is target-owned and read-only at source
The canonical installer SHALL NOT mutate `canon/skills` or any installed npm
source tree. It SHALL write a timestamp-free, content-addressed manifest under
the target (for example `.canon/skills/.install-lock.json`) recording source,
provider, and projected-file hashes. Existing symlinks and unrelated user files
MUST NOT be overwritten.

#### Scenario: Re-running with no source changes is a byte-identical no-op
- **WHEN** `canon skills install` runs twice in a row with no change to the
  canonical source bundle
- **THEN** every projected file and the target manifest are byte-identical
  across both runs and the second run reports unchanged.

#### Scenario: Read-only check and doctor expose drift
- **WHEN** a projected file is missing or changed, or an old `canon-*` user
  skill remains
- **THEN** `canon skills check` reports drift without writing
- **AND** `canon skills doctor` reports the missing/stale projection or legacy
  remnant without deleting user data.

### Requirement: Developer-only legacy materialization remains compatible
- **WHEN** `canon skills install --source canon/skills-dev` runs
- **THEN** it retains `.claude/skills/<name>/SKILL.md`, `.codex/skills/<name>.md`,
  and the source-local legacy `.install-lock.json` behavior.

### Requirement: Published npm packages resolve the canonical source
The npm build SHALL package the canonical bundle under the CLI package's
`dist/skills` (or equivalent package-owned path), and the launcher SHALL pass
that path to the native binary through `CANON_SKILLS_SOURCE`. A checkout
launcher SHALL fall back to `canon/skills` when no generated package bundle is
present.
