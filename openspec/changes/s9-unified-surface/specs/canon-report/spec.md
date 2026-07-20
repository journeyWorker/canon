## ADDED Requirements

### Requirement: Generated-only status report
`canon report` SHALL generate a markdown status report from the S1 model over S2's
tiered storage. The report SHALL be treated as generated output only: no other tool
or human process SHALL hand-edit it, and the report itself SHALL declare this in a
header comment.

#### Scenario: Report is regenerated, not edited
- **WHEN** an operator runs `canon report` in a repo with `.canon/canon.yaml`
  configured
- **THEN** the command writes a markdown file whose header states it is generated
  by `canon report` and must not be hand-edited

### Requirement: Report embeds input digests, never a timestamp
The report header SHALL embed input digests (corpus hash, policy hash, ledger-head
hash, `source_git_sha`) so every verdict in the report is reproducible from named
inputs. The report SHALL NOT embed a `generated_at` field or any other timestamp.

#### Scenario: Same inputs produce a digest-identical header
- **WHEN** `canon report` runs twice against the same corpus, policy, and ledger head
- **THEN** the digest fields in both report headers are identical
- **AND** neither header contains a timestamp field

### Requirement: `--check` is byte-stable drift detection
`canon report --check` SHALL regenerate the report in memory and compare it
byte-for-byte against the existing report file. Given unchanged input, the command
SHALL exit 0. Given changed input or a modified report file, the command SHALL exit
1 with a message identifying drift. Given no existing report file, the command
SHALL exit 1 with a message instructing the operator to run `canon report` first.

#### Scenario: No drift on unchanged input
- **GIVEN** a report file was generated from the current corpus/policy/ledger state
- **WHEN** the operator runs `canon report --check` without changing any input
- **THEN** the command exits 0 and prints a "no drift" message

#### Scenario: Drift detected on changed input
- **GIVEN** a report file was generated from a prior corpus/policy/ledger state
- **WHEN** the operator changes the corpus (e.g. a task flips done) and runs
  `canon report --check`
- **THEN** the command exits 1 and prints a message naming the stale report path

#### Scenario: Missing report file
- **GIVEN** no report file has ever been generated in this repo
- **WHEN** the operator runs `canon report --check`
- **THEN** the command exits 1 and instructs the operator to run `canon report`
  first

### Requirement: Parquet snapshot export with a declared manifest
`canon report --snapshot <dir>` SHALL export every report-backing DuckDB view to a
Parquet file per table (one file per table, filename identical to the table name)
under `<dir>`, plus a single `manifest.json` at `<dir>/manifest.json` containing
`generated_at`, `source_git_sha`, `source_digest`, and a `tables` array of
`{table, file}` entries covering every exported table.

#### Scenario: Snapshot export produces a complete manifest
- **WHEN** an operator runs `canon report --snapshot ./snapshot`
- **THEN** `./snapshot/manifest.json` lists one `{table, file}` entry for every
  table exported to `./snapshot/*.parquet`
- **AND** every listed `file` exists under `./snapshot/`

#### Scenario: Table names with digits export without filename escaping
- **GIVEN** a report-backing view whose name contains a digit (e.g.
  `mart_axis2_conformance`)
- **WHEN** an operator runs `canon report --snapshot ./snapshot`
- **THEN** the exported filename is byte-identical to the table name
  (`mart_axis2_conformance.parquet`, never an escaped variant)
