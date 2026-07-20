## ADDED Requirements

### Requirement: Adapter registry enumerates configured session sources
`canon-ingest` SHALL expose a static adapter registry where each entry declares
a source id, a root-path resolver (which MAY return more than one root — the
`codex` adapter unions two sibling roots, and `hermes` unions its default
`state.db` with any per-profile `state.db` files it discovers), a scan glob,
and a parser function. The registry SHALL ship four adapters at S3
completion, in declaration order: `omp` (`~/.omp/agent/sessions/`
UNIONED with `~/.pi/agent/sessions/`), `hermes` (`~/.hermes/state.db`
UNIONED with any `~/.hermes/profiles/*/state.db`), `claude`
(`~/.claude/projects/`), and `codex`
(`${CODEX_HOME:-~/.codex}/sessions/` UNIONED with
`${CODEX_HOME:-~/.codex}/archived_sessions/`).

#### Scenario: Registry enumeration is deterministic
- **WHEN** `canon ingest sessions` runs with no adapter selection flag
- **THEN** it scans the `omp`, `hermes`, `claude`, and `codex` adapters, in
  that declared order, and the run's manifest lists exactly those four
  source ids

#### Scenario: Unconfigured or absent source root is skipped, not fatal
- **WHEN** a configured adapter's root directory (e.g. `~/.codex/sessions/`)
  does not exist on the host running the ingest
- **THEN** that adapter contributes zero records and the run exits 0; the
  absence is reported as an informational line, never a violation or crash

#### Scenario: Codex adapter unions live and archived session roots
- **WHEN** the `codex` adapter's `resolve_roots()` runs
- **THEN** it returns both `${CODEX_HOME:-~/.codex}/sessions` and
  `${CODEX_HOME:-~/.codex}/archived_sessions` as scan roots feeding the same
  `codex` adapter identity, so a session Codex CLI has rotated into
  `archived_sessions/` is still ingested and not silently under-counted

### Requirement: Normalized output conforms to the S1 envelope and join spine
Every record an adapter emits SHALL be normalized into canon-model's
`Session`, `Run`, or `Event` type carrying the envelope `{schema, kind, at,
actor}` (S1), and every token/cost row SHALL carry a `session_id` matching the
join-spine grammar (S1: "agent-CLI UUID, the vendored upstream launcher's join key").

#### Scenario: Claude Code session normalizes to a session_id-keyed cost row
- **WHEN** the `claude` adapter parses a `~/.claude/projects/**/*.jsonl`
  fixture file containing a session with a known UUID and token usage
- **THEN** the normalized output includes a token/cost row whose `session_id`
  field equals that UUID and whose envelope `actor.agent_id` identifies the
  Claude Code client

#### Scenario: Malformed adapter record is skipped as a violation, never crashes
- **WHEN** an adapter's scan encounters a record it cannot parse (truncated
  JSON line, missing required field)
- **THEN** `canon ingest sessions` skips that record, counts it as a violation
  in the run summary, and continues processing the remaining records in the
  same source and the other adapters
