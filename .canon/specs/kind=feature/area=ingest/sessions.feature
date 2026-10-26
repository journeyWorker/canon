Feature: ingest sessions
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:ingest-pipelines
  @case:happy
  @ingest.sessions.01
  Scenario: Every declared transcript adapter is still registered and its parse generation matches what it declares
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given the registered agent-CLI transcript adapters
    When the registry is enumerated
    Then it is exactly omp, hermes, claude-code and codex, and no other id
    And each one reports the parse generation its own declaration pins it to
    And an adapter dropped from the registry fails that same enumeration rather than going unnoticed

  @subject:ingest-pipelines
  @case:happy
  @ingest.sessions.02
  Scenario: One adapter covers both of the roots its CLI writes under
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a home directory carrying transcripts under both of the omp adapter's roots
    When the adapter scans that home
    Then transcripts from both roots are picked up in one pass
    And each session id comes from the transcript's own content, not from its filename
    And a corrupt line inside a transcript is skipped while the valid messages either side of it survive

  @subject:ingest-pipelines
  @case:happy
  @ingest.sessions.03
  Scenario: A Claude Code transcript is located by its project directory and keyed by the workspace that directory encodes
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a home directory holding Claude Code project transcripts
    When the claude-code adapter scans it
    Then every transcript under the projects tree is scanned
    And each row's workspace is derived from the encoded project path

  @subject:ingest-pipelines
  @case:happy
  @ingest.sessions.04
  Scenario: A subagent transcript becomes a child run under the main agent's root run
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a transcript in which the main agent dispatched subagents
    When it is normalized
    Then the session yields exactly one parentless root run
    And each subagent yields one child run parented to that root
    And the child run stays on the dispatching session rather than being re-grouped into a session of its own

  @subject:ingest-pipelines
  @case:happy
  @ingest.sessions.05
  Scenario: Ingest is scoped to this project and its linked worktrees by default
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a machine holding transcripts for this project, for a worktree linked to it, and for a foreign project
    When sessions are ingested with no scope flag
    Then this project's own session and its linked worktree's session are both ingested
    And the worktree session carries the main worktree's project key so the two aggregate together
    And the foreign project's session is excluded
    And the default output carries no directive text

  @subject:ingest-pipelines
  @case:happy
  @ingest.sessions.06
  Scenario: Widening to all workspaces restores the machine-wide scan
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given the same machine holding this project, a linked worktree, and a foreign project
    When sessions are ingested with all workspaces requested
    Then all three sessions are ingested, the foreign one included
    And the run reports its scope as all workspaces rather than a root count

  @subject:ingest-pipelines
  @case:edge
  @ingest.sessions.07
  Scenario: The watermark is per source and privacy policy, so only changed inputs are re-read
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a first pass that wrote a cursor for every scanned source
    When a second pass runs over an unchanged corpus with the same privacy policy
    Then nothing is re-parsed and nothing is written
    And when a session is appended to one source's transcript, only that source re-parses
    And that source is not reported as skipped
    And changing the privacy policy changes the cursor identity even when the source is unchanged

  @subject:ingest-pipelines
  @case:edge
  @ingest.sessions.08
  Scenario: Normalized records persist through the routed tier, never a private write path
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository routing sessions, runs and events to a rung, already ingested once
    When ingest runs again with a full rescan forced
    Then the source is re-parsed instead of reported unchanged
    And the persisted record count is exactly what it was, because an identical record writes to the identical path

  @subject:ingest-pipelines
  @case:edge
  @ingest.sessions.09
  Scenario: Ingesting the same transcripts twice yields the same records
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a fixture home of transcripts
    When the whole pipeline runs over it twice
    Then the normalized output of the second pass is byte-identical to the first
    And the count of rows the pass skipped is identical too, so nothing is rediscovered as new

  @subject:ingest-pipelines
  @case:edge
  @ingest.sessions.10
  Scenario: Absent privacy config omits directive text from durable normalized output
  # canon: {"schema":1,"at":"2026-10-02T00:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a transcript containing a user directive with secret prose
    And a repository with no `ingest.sessions.privacy` section
    When sessions are ingested
    Then no user_directive event reaches durable normalized output
    And the source transcript remains unchanged
    And source transcript ownership and rewriting are out of scope for ingest

  @subject:ingest-pipelines
  @case:happy
  @ingest.sessions.11
  Scenario: Bounded capture replaces secrets before truncating and records deterministic metadata
  # canon: {"schema":1,"at":"2026-10-02T00:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a transcript containing a directive longer than the configured positive bound with a known secret or PII value
    When sessions are ingested with capture explicitly enabled
    Then the known secret or PII value is replaced before truncation
    And the directive event carries deterministic redaction metadata including `redacted: true`
    And it carries `truncated`, `original_chars`, and `captured_chars`
    And the captured text ends at a Unicode scalar boundary

  @subject:ingest-pipelines
  @case:edge
  @ingest.sessions.13
  Scenario: Bounded capture truncation alone is not redaction
  # canon: {"schema":1,"at":"2026-10-02T00:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a transcript containing a directive longer than the configured positive bound with no known secret or PII value
    When sessions are ingested with capture explicitly enabled
    Then the directive event carries `truncated: true`
    And it carries `original_chars` and `captured_chars`
    And it does not carry `redacted: true`
    And the captured text ends at a Unicode scalar boundary

  @subject:ingest-pipelines
  @case:failure
  @ingest.sessions.12
  Scenario: Unwritten JSON is metadata-only even when bounded capture is enabled
  # canon: {"schema":1,"at":"2026-10-02T00:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a routed session tier that is unreachable
    And a transcript containing secret directive, credential, and task/context prose
    When sessions are ingested
    Then the unwritten JSON contains scope, counts, IDs and digests
    And it contains a stable tier failure class and reason
    And it contains none of the directive text, credential text, task/context prose, or raw event detail

  # canon: {"schema":1,"at":"2026-10-23T13:13:55Z","actor":{"agent_id":"canon-impl-028"}}
  @subject:ingest-pipelines
  @case:happy
  @ingest.sessions.14
  Scenario: Explicit directive capture carries a bounded user_directive and rejects an absent, zero or negative bound
    Given a transcript whose user turn is longer than a positive `max_directive_chars`
    When sessions are ingested with `capture_user_directives: true`
    Then the persisted user_directive event carries the user turn cut to that many characters
    And ingest fails with a config error and persists nothing when `max_directive_chars` is absent, zero, or negative

  # canon: {"schema":1,"at":"2026-10-26T18:13:28Z","actor":{"agent_id":"impl-030-rl","role":"implementer","session_id":"rl-2026-10-26"}}
  @subject:ingest-pipelines
  @case:happy
  @flow:canon-0-30-record-links.F5
  @ingest.sessions.15
  Scenario: A command that writes hot records prints the hot store location
    Given a repo whose hot rung is the sqlite file `.canon/hot.db` and a session transcript
    When canon ingest sessions writes the session, run and event records
    Then the output names the hot store path
    And --json carries it as `hotStore`

  # canon: {"schema":1,"at":"2026-10-26T19:30:00Z","actor":{"agent_id":"impl-030-rl","role":"implementer","session_id":"rl-fix-2026-10-26"}}
  @subject:ingest-pipelines
  @case:happy
  @flow:canon-0-30-record-links.F5
  @ingest.sessions.16
  Scenario: The other commands that write hot records print the hot store location too
    Given a repo whose hot rung is the sqlite file `.canon/hot.db`
    When canon ingest plans, ingest records and dispatch begin run, and canon subject new, subject move, subject adopt, subject unadopt and change new write records that are routed to the hot rung
    Then each output names the hot store path
    And the --json documents of ingest plans carry it as `hotStore`
    And the --json documents of change new carry it as `hotStore`, and those of the subject leaves as `meta.hotStore`
