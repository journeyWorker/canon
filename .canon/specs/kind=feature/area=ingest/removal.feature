Feature: ingest removal
  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:ingest-pipelines
  @lane:behavior
  @case:happy
  @flow:canon-0-39-session-ingest-removal.F1
  @ingest.removal.01
  Scenario: canon ingest sessions is refused as an unknown subcommand and the help names plans and records
    Given a repo and the command `canon ingest sessions`, bare, with --watch and with --home and --all-workspaces
    When each is run with --json, and `canon ingest --help` is read
    Then each exits 2 with the code UNKNOWN_COMMAND, received `sessions` and expected `plans` and `records`
    And the help names `canon ingest records` and `canon ingest plans` as the only examples and never `sessions`

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:ingest-pipelines
  @lane:behavior
  @case:failure
  @flow:canon-0-39-session-ingest-removal.F1
  @ingest.removal.02
  Scenario: The selftest suite session-ingest is a closed-value refusal and the generated documents keep no trace of the command
    Given the closed values of `selftest --suite` and the generated command document, the schema, the website reference and the orchestrator guide
    When `canon selftest --suite session-ingest` is run, and a bare `canon selftest`, and the documents are read
    Then the suite exits 2 as a usage error that does not list `session-ingest`, and the bare run exits 0
    And no document names `ingest sessions`, `session-ingest` or the removed report

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:ingest-pipelines
  @lane:behavior
  @case:happy
  @flow:canon-0-39-session-ingest-removal.F2
  @ingest.removal.03
  Scenario: The 12-character content digest that names the plan cursor file and hashes the regime key is byte for byte what 0.38 computed
    Given the five fixed values whose digests were computed from the 0.38 definition: SHA-256 of the compact key-sorted JSON, first 12 hex characters
    When content_digest runs over each value from the module that kept it
    Then each digest equals its pinned 12 characters, so one changed character of any value fails the test
    And the store's own 8-character content digest is another function and is not touched

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:ingest-pipelines
  @lane:behavior
  @case:happy
  @flow:canon-0-39-session-ingest-removal.F3
  @ingest.removal.04
  Scenario: canon dispatch begin and end write the Run to the routed tier
    Given a repo made by canon init and a role, a regime and an actor
    When canon dispatch begin and then canon dispatch end run
    Then begin writes the Run to the hot rung and query --kind run finds one record
    And end exits 0 on the same Run

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:ingest-pipelines
  @lane:behavior
  @case:failure
  @flow:canon-0-39-session-ingest-removal.F3
  @ingest.removal.05
  Scenario: A Run the tier already holds is rewritten without an error and other content under the same run id is a new version
    Given a git-routed repo whose Run was begun
    When the same Run is persisted again, and then the same run id is persisted with another actor model
    Then the byte-identical write is not an error and no second row appears
    And the other content is a new version the fold ranks, so one run id still resolves to one row and nothing is overwritten silently

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:ingest-pipelines
  @lane:behavior
  @case:happy
  @flow:canon-0-39-session-ingest-removal.F4
  @ingest.removal.06
  Scenario: An old ingest section in canon.yaml is not read
    Given a repo and a copy of it whose canon.yaml also holds the old `ingest:` section with sources and privacy keys, one of them invalid
    When canon status and canon gate check run in both
    Then the exit codes and the data are the same in both
    And no warning names the old section

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:ingest-pipelines
  @lane:behavior
  @case:failure
  @flow:canon-0-39-session-ingest-removal.F4
  @ingest.removal.07
  Scenario: A broken canon.yaml is still the command's configuration error whatever sections it holds
    Given a canon.yaml that holds the old `ingest:` section and an unclosed flow sequence
    When canon gate check runs
    Then it exits nonzero with a CONFIG_ error code
    And the old section changed no result

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:ingest-pipelines
  @lane:behavior
  @case:happy
  @flow:canon-0-39-session-ingest-removal.F5
  @ingest.removal.08
  Scenario: The skills bundle no longer carries the session ingest guide
    Given the skills source tree, its index, the knowledge index and the artifact ingest guide
    When they are read and `canon skills show canon-session-ingest` is run
    Then the guide file does not exist and no index names it or the `session-reference` entry
    And the show command exits nonzero

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:ingest-pipelines
  @lane:behavior
  @case:failure
  @flow:canon-0-39-session-ingest-removal.F5
  @ingest.removal.09
  Scenario: An old install's copy of the guide is an orphan that skills check names and skills install removes
    Given a repo that installed the new bundle and then holds the guide again, with its hash in the install lock
    When canon skills check runs, then canon skills install, then check again
    Then the first check exits 1 and names `canon-session-ingest` as an orphan
    And the install removes the file while its bytes match the lock, and the next check exits 0

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:ingest-pipelines
  @lane:behavior
  @case:happy
  @flow:canon-0-39-session-ingest-removal.F6
  @ingest.removal.10
  Scenario: The sessions scenarios left with their feature file and the hot store location one moved to the plans feature
    Given the specs of area ingest
    When the corpus is read
    Then `sessions.feature` does not exist and no feature file names a removed sessions scenario id
    And `plans.feature` holds the hot store location scenario as ingest.plans.14 with its flow tag and body

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:ingest-pipelines
  @lane:behavior
  @case:failure
  @flow:canon-0-39-session-ingest-removal.F6
  @ingest.removal.11
  Scenario: A scenario whose feature file is deleted is tombstoned on the next sync and is not an uncovered cell
    Given a repo with two declared scenarios in two feature files, one file then deleted
    When canon inventory sync runs and canon query --kind scenario --all lists the scenarios
    Then the deleted file's scenario is gone from the listing and the other stays
    And the sync exits 0

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:ingest-pipelines
  @lane:behavior
  @case:happy
  @flow:canon-0-39-session-ingest-removal.F7
  @ingest.removal.12
  Scenario: The README, the architecture document and the website do not name the removed command
    Given README.md, ARCHITECTURE.md, the website sources and the generated command document
    When they are read
    Then none names `ingest sessions` or `canon-session-ingest`, the privacy pages do not exist, and the spec names neither removed environment variable

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:ingest-pipelines
  @lane:behavior
  @case:failure
  @flow:canon-0-39-session-ingest-removal.F7
  @ingest.removal.13
  Scenario: The release check fails when the CHANGELOG does not list what was removed
    Given the 0.39 section of the CHANGELOG
    When it is read for the command, both environment variables, both configuration keys, the selftest suite, the skill, the cursor file and UNKNOWN_COMMAND
    Then each is named, and a missing entry fails the test naming it

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:ingest-pipelines
  @lane:behavior
  @case:edge
  @flow:canon-0-39-session-ingest-removal.F2
  @ingest.removal.14
  Scenario: A fresh canon init store takes a Run-writing command with no environment variable and no service
    Given a repo made by canon init whose hot rung is the sqlite file `.canon/hot.db`
    When canon dispatch begin runs there
    Then it exits 0 and the sqlite file exists
    And no service and no environment variable was needed

  # canon: {"schema":1,"at":"2026-11-08T08:20:21Z","actor":{"agent_id":"impl-0-39","role":"implementer","session_id":"s-0-39-impl"}}
  @subject:ingest-pipelines
  @lane:behavior
  @case:failure
  @flow:canon-0-39-session-ingest-removal.F2
  @ingest.removal.15
  Scenario: A plans source that yields no record is refused as before and writes neither a record nor a cursor file
    Given a plans source whose root is one level above the change directories, so it holds no readable change
    When canon ingest plans runs on it
    Then it exits 2 with INGEST_SOURCE_INVALID and no data, as 0.38 did
    And no change or task record is written and no plan cursor file exists under .canon/ingest/cursors

  # canon: {"schema":1,"at":"2026-11-08T08:30:56Z","actor":{"agent_id":"impl-0-39","role":"implementer","session_id":"s-0-39-impl"}}
  @subject:ingest-pipelines
  @lane:behavior
  @case:failure
  @flow:canon-0-39-session-ingest-removal.F6
  @ingest.removal.16
  Scenario: The old evidence of a scenario whose feature file is deleted is not stale evidence once enough commits have passed
    Given two scenarios with green evidence, both past max_commits_behind, so the gate names both as stale-evidence, and one feature file then deleted and canon inventory sync run
    When canon gate check runs
    Then it names the scenario that is still declared as stale-evidence and does not name the deleted one
    And the count of cells whose base commit git could not read does not include the deleted scenario
