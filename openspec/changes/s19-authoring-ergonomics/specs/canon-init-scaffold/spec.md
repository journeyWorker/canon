## ADDED Requirements

### Requirement: canon init writes a fresh, working canon.yaml skeleton and never overwrites an existing one
`canon init [--repo <dir>]` SHALL write a `canon.yaml` at `<repo>/canon.yaml`
carrying: a `tiers:` section with a working `git:` root plus commented
`pg:`/`r2:` example stanzas; a `routing:` section routing every one of
`RecordKind::ALL`'s twelve wire strings to `git`; a `specs:` section with
one working `{id: root, root: specs}` entry; and a `plans: { sources: [] }`
section. `canon init` SHALL refuse to overwrite an existing `canon.yaml`
(atomic create-fails-if-exists, mirroring `canon feature new`'s own
refusal convention) — exit `2`, the existing file's bytes UNTOUCHED, never
a silent overwrite or a check-then-write race.

#### Scenario: canon init scaffolds a working config in a fresh repo
- **WHEN** `canon init --repo .` runs in a directory with no `canon.yaml`
- **THEN** a `canon.yaml` is written carrying `tiers.git`, all twelve
  `routing:` entries (each mapped to `git`), one `specs.roots[]` entry,
  and a `plans: { sources: [] }` section, and the command exits `0`

#### Scenario: canon init refuses to overwrite an existing canon.yaml
- **WHEN** `canon init --repo .` runs in a directory that already has a
  `canon.yaml`
- **THEN** the command refuses with an error naming the existing file,
  exits `2`, and the existing file's bytes are byte-identical
  before and after

### Requirement: The scaffolded config resolves cleanly through every existing strict loader with zero further edits
Every section `canon init` writes SHALL parse successfully through the
SAME loader `canon inventory sync`/`canon ingest plans`/`canon tier age`
would use against it, with no additional operator edits: `TierPolicy::
from_yaml` for `tiers:`/`routing:`/`aging:`, `load_spec_roots` for
`specs:`, `load_plan_sources_from_config` for `plans:`.

#### Scenario: A freshly-init'd repo's inventory sync runs with zero further config
- **WHEN** `canon init --repo .` runs, one well-formed `.feature` file is
  added under the scaffolded `specs/` root, and `canon inventory sync
  --repo .` runs immediately after
- **THEN** the sync resolves the scaffolded `specs.roots[]` entry and
  materializes the scenario — no `canon.yaml` edit was required between
  `init` and `sync`

#### Scenario: A freshly-init'd repo's plan ingest runs as a clean no-op
- **WHEN** `canon init --repo .` runs and `canon ingest plans --repo .`
  runs immediately after, with no `plans.sources[]` entries added
- **THEN** the command resolves zero configured sources and exits `0` —
  the scaffolded `plans: { sources: [] }` parses as a legitimate,
  explicit zero-source configuration, never a parse failure

### Requirement: canon init --check-config validates an existing canon.yaml read-only, reusing the existing per-section loaders
`canon init --check-config [--repo <dir>]` SHALL be READ-ONLY (it writes
no file). It SHALL require an EXISTING `canon.yaml` at `<repo>/canon.yaml`
— a missing file fails loud, exit `2`, distinct from any success/failure
report about config CONTENT. When the file exists, it SHALL run each of
the three existing loaders (`TierPolicy::from_yaml`, `load_spec_roots`,
`load_plan_sources_from_config`) against it in turn, reporting one
PASS/FAIL (or "not configured" for a legitimately absent optional
section) line per section, and SHALL exit `0` only when every PRESENT
section parses cleanly under its own existing rules — a failure in one
section SHALL NOT prevent the other two sections from being checked and
reported in the same run.

#### Scenario: --check-config on a freshly-scaffolded config reports all sections clean
- **WHEN** `canon init --repo .` runs, then `canon init --check-config
  --repo .` runs immediately after with no edits in between
- **THEN** the report shows `tiers`/`routing`/`aging`, `specs`, and
  `plans` all PASS, and the command exits `0`

#### Scenario: --check-config on a missing canon.yaml fails loud
- **WHEN** `canon init --check-config --repo .` runs in a directory with
  no `canon.yaml`
- **THEN** the command fails with an error naming the missing file, exits
  `2`, and reports no per-section PASS/FAIL lines

#### Scenario: --check-config surfaces a malformed section without hiding the others
- **WHEN** `canon init --check-config --repo .` runs against a
  `canon.yaml` whose `plans:` section has an unregistered dialect id
  while `tiers:`/`routing:`/`specs:` are well-formed
- **THEN** the report shows `tiers`/`routing`/`aging` and `specs` as PASS
  and `plans` as FAIL naming the unregistered dialect id (the exact
  message `load_plan_sources_from_config` already produces), and the
  command exits nonzero

#### Scenario: --check-config treats an absent optional section as "not configured", not a failure
- **WHEN** `canon init --check-config --repo .` runs against a
  `canon.yaml` with no `plans:` key at all, `tiers:`/`routing:`/`specs:`
  otherwise well-formed
- **THEN** the report shows `plans` as "not configured" (not FAIL) and
  the command exits `0`
