## ADDED Requirements

### Requirement: canon query accepts --repo resolved through the same ancestor walk every sibling verb uses
`canon query` SHALL accept `--repo <REPO>` (default `.`), resolved through
`canon_cli::context::resolve_repo_root` — the identical function
`canon context`, `canon gate check`/`task`/`promote`, `canon inventory
sync`, `canon plugin sync`, `canon ingest artifacts`, `canon ingest plans`,
`canon report`, `canon dashboard`, and `canon retrieve` already call.
`--repo == "."` (the default) SHALL walk `cwd.ancestors()` for the nearest
directory containing a `canon.yaml` file; any OTHER explicit `--repo <dir>`
SHALL be used as-is, with no walk. `canon.yaml` is then read from
`<resolved-repo>/canon.yaml`.

#### Scenario: canon query with no flags resolves from a subdirectory
- **WHEN** `canon query --kind change` runs with cwd inside
  `<repo>/some/nested/dir`, where `<repo>/canon.yaml` exists and no
  `canon.yaml` exists in any directory between cwd and `<repo>`
- **THEN** the command resolves `<repo>/canon.yaml` via the ancestor walk
  and succeeds — the same outcome `canon context`/`canon gate check` already
  produce from the identical cwd

#### Scenario: An explicit non-default --repo is used as-is, no walk
- **WHEN** `canon query --kind change --repo /some/other/repo` runs from
  any cwd
- **THEN** `canon.yaml` is read from exactly `/some/other/repo/canon.yaml`
  — no ancestor walk is performed, identical to `resolve_repo_root`'s
  documented `repo != "."` short-circuit for every other verb

#### Scenario: Running from the repo root itself is unaffected
- **WHEN** `canon query --kind change` runs with cwd exactly at `<repo>`
  (where `<repo>/canon.yaml` exists)
- **THEN** the command resolves and succeeds exactly as it did before this
  change — the ancestor walk starting at cwd finds `canon.yaml` immediately

### Requirement: --canon-yaml remains an explicit override that bypasses the ancestor walk
`canon query` SHALL retain a `--canon-yaml <path>` flag as an explicit,
literal-path override: when supplied, `canon.yaml` is read from exactly
that path, with no `resolve_repo_root` walk applied to it and no
dependency on `--repo`'s value. `--canon-yaml`, when present, SHALL take
precedence over `--repo`. A caller supplying neither flag SHALL get
`--repo`'s default-`.`-ancestor-walk behavior; a caller supplying only
`--canon-yaml` SHALL get pre-existing (pre-this-change) behavior
byte-for-byte.

#### Scenario: An explicit --canon-yaml still resolves the literal path from any cwd
- **WHEN** `canon query --kind change --canon-yaml /some/snapshot/canon.yaml`
  runs from a cwd that is NOT an ancestor of `/some/snapshot`
- **THEN** the command reads exactly `/some/snapshot/canon.yaml` — the
  same literal-path behavior this flag had before this change, unaffected
  by cwd or any ancestor walk

#### Scenario: --canon-yaml wins when both flags are supplied and would resolve differently
- **WHEN** `canon query --kind change --repo /repo/a --canon-yaml
  /repo/b/canon.yaml` runs, where `/repo/a/canon.yaml` and
  `/repo/b/canon.yaml` are different files
- **THEN** the command reads `/repo/b/canon.yaml` (the `--canon-yaml`
  value) — `--repo`'s resolution is not consulted

#### Scenario: A missing canon.yaml at every ancestor still fails loud, named
- **WHEN** `canon query --kind change` runs from a cwd with no `canon.yaml`
  anywhere among its ancestors (a genuinely unconfigured location)
- **THEN** the command fails with an error naming the path the ancestor
  walk ultimately attempted to read — never a silent empty-result success
