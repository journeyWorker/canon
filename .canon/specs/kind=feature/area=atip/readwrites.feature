Feature: atip read writes: a refusal and a read command write only what they declare
  # canon: {"schema":1,"at":"2026-11-02T13:02:35Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}

  # canon: {"schema":1,"at":"2026-11-02T13:02:36Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-read-writes.F1
  @atip.readwrites.01
  Scenario: A refused call of subject show, subject move, evidence add, evidence audit, review, divergence accept or artifacts store leaves the repository, HOME and temp dir byte for byte as they were, and the stale scenario index stays unwritten
    Given a repository whose scenario index is stale, a private HOME and a private temp dir
    When subject show, subject move, evidence add, evidence audit, review scenario, divergence accept are each refused (a missing subject, a transition no chain allows, a bad value, a scenario no feature declares)
    Then each is exit 2 and the files of the repository, HOME and the temp dir are byte for byte the same, and no scenario index record was written

  # canon: {"schema":1,"at":"2026-11-02T13:03:20Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-read-writes.F1
  @atip.readwrites.02
  Scenario: A call that is accepted refreshes the stale scenario index it owes, a preview does not, and a scenario only a feature file declares is known to the call from the index it computes in memory
    Given the same stale repository
    When subject show, subject move, evidence add with a report naming a scenario only a feature file declares, evidence audit, review scenario, divergence accept and artifacts store are accepted, and artifacts store --dry-run is run
    Then each accepted call exits 0 and the scenario index is written, and the preview writes nothing

  # canon: {"schema":1,"at":"2026-11-02T13:04:07Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}
  @subject:query-reads
  @case:happy
  @flow:canon-0-35-read-writes.F2
  @atip.readwrites.03
  Scenario: subject show declares the cache refresh in its effectNote, says a refused call writes nothing, and writes no line to stderr in JSON mode
    Given a stale repository
    When schema subject show runs and subject show runs in JSON mode, then in text mode
    Then the effectNote names the cache refresh and says a refused call writes nothing, stderr is empty in JSON mode, and the text mode says once on stderr that it refreshed

  # canon: {"schema":1,"at":"2026-11-02T13:04:37Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-read-writes.F4
  @atip.readwrites.04
  Scenario: Every --dry-run, --help, --version, schema and skills show call leaves the repository, HOME and temp dir byte for byte as they were
    Given a repository with a private HOME and temp dir
    When each of the commands that have --dry-run runs with it, every command runs with --help and with schema, and --version and skills show run
    Then every preview that reaches its work says dryRun and the files of the repository, HOME and the temp dir are byte for byte the same

  # canon: {"schema":1,"at":"2026-11-02T13:05:12Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-read-writes.F4
  @atip.readwrites.05
  Scenario: report check and selftest leave no file in the OS temp directory on success, refusal and failure
    Given a private temp dir and a repository
    When report check and selftest run on success, on a missing repository or suite, and on a canon.yaml that does not parse
    Then the temp dir holds no file afterwards

  # canon: {"schema":1,"at":"2026-11-02T13:05:44Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}
  @subject:atip-contract
  @case:edge
  @flow:canon-0-35-read-writes.F5
  @atip.readwrites.06
  Scenario: dashboard is declared a server: one report, then it serves until signalled, and the schema has no stream lines
    Given the schema of dashboard
    When its effectNote and its outputs are read
    Then the note says it prints one report and serves until signalled, and the one output is the DashboardReport with no stream line

  # canon: {"schema":1,"at":"2026-11-02T13:06:22Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-read-writes.F3
  @atip.readwrites.07
  Scenario: artifacts store refused because the store cannot take a blob leaves the stale scenario index unwritten
    Given an evidence record that binds a file whose blob is gone, a stale index, and a store directory that is not writable
    When artifacts store runs
    Then it is refused with exit 2, the files are byte for byte the same and the scenario index is not refreshed

  # canon: {"schema":1,"at":"2026-11-03T05:07:06Z","actor":{"agent_id":"impl-flow-failures","role":"implementer","session_id":"s-0-35-flow-failures"}}
  @subject:query-reads
  @case:failure
  @flow:canon-0-35-read-writes.F2
  @atip.readwrites.08
  Scenario: subject show of a subject that does not exist is exit 2 SUBJECT_NOT_FOUND with an empty stderr in JSON mode and writes nothing
    Given a stale repository and a subject that does not exist
    When subject show nosuch runs in JSON mode
    Then it exits 2 with SUBJECT_NOT_FOUND, stderr is empty, no refresh warning is given, nothing is written and the stale scenario index stays unwritten, while the subject that exists is shown

  # canon: {"schema":1,"at":"2026-11-03T05:08:42Z","actor":{"agent_id":"impl-flow-failures","role":"implementer","session_id":"s-0-35-flow-failures"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-read-writes.F5
  @atip.readwrites.09
  Scenario: A dashboard call that is refused is one error envelope and exit 2, serves nothing and writes nothing
    Given a repository and a port that is no integer, then an option dashboard does not have
    When canon dashboard runs with each in JSON mode
    Then each exits 2 with USAGE naming the argument, as one envelope with nothing on stderr, returns at once without serving, and the repository, HOME and temp dir are as they were
