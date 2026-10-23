Feature: report generation
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:report-generation
  @case:happy
  @report.generation.01
  Scenario: The report is generated into the repo, never authored by hand
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository with no report written yet
    When the report is generated against the resolved repo root
    Then the command exits zero and names the path it wrote
    And the file exists at that path under the repo root
    And it opens with the generated report heading, not with anything a person typed

  @subject:report-generation
  @case:failure
  @report.generation.02
  Scenario: Checking a repo that never generated a report is a missing verdict, not a pass
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository with no report file on disk
    When the drift gate is run
    Then it exits nonzero and reports the report as missing
    And an absent report is never mistaken for an up-to-date one

  @subject:report-generation
  @case:happy
  @report.generation.03
  Scenario: The drift gate passes immediately after a write
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a report just written over an unchanged corpus
    When the drift gate is run against it
    Then it exits zero and reports no drift
    And nothing about the corpus had to change for the gate to agree

  @subject:report-generation
  @case:failure
  @report.generation.04
  Scenario: A hand edit to the committed report is drift
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a generated report that a person has since edited on disk
    When the drift gate is run
    Then it exits nonzero and names the outcome as drift
    And the comparison is over the file's bytes, so an edit anywhere in it is caught

  @subject:report-generation
  @case:failure
  @report.generation.05
  Scenario: The full check lifecycle moves missing then clean then drifted
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a fresh repository driven through the real command
    When the gate is run before the write, after the write, and after a hand edit
    Then the three runs report missing, then no drift, then drift
    And only the middle one exits zero

  @subject:report-generation
  @case:edge
  @report.generation.06
  Scenario: Two consecutive renders of one unchanged corpus are byte-identical
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a fixture corpus that does not change between runs
    When the report is rendered twice in a row
    Then the two renderings are equal byte for byte
    And the drift gate above is therefore measuring the corpus, not run-to-run noise

  @subject:report-generation
  @case:happy
  @report.generation.07
  Scenario: A snapshot exports every mart as Parquet plus a manifest listing exactly them
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository and an output directory for the snapshot
    When canon report snapshot is run against that directory
    Then it reports ten tables and writes one Parquet file per mart
    And the manifest beside them lists exactly those ten tables
    And the markdown report is not written, because exporting and generating are different actions

  @subject:report-generation
  @case:happy
  @report.generation.08
  Scenario: A multi-tier repo is warned about the kinds the report does not read directly
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repo routing some kinds to rungs the report does not read directly and others to a directly-read rung
    When the report is generated
    Then it carries a section naming exactly the kinds it did not read directly, sorted
    And it points the reader at the query command for those kinds
    And a directly-read kind is never named there

  @subject:report-generation
  @case:edge
  @report.generation.09
  Scenario: A repo whose every routed rung is read directly gets no boundary warning at all
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repo whose routing resolves every kind to a directly-read rung
    When the report is generated
    Then the boundary section is absent entirely
    And the warning is a statement about this repo's routing, not boilerplate

  # canon: {"schema":1,"at":"2026-10-12T14:00:00Z","actor":{"agent_id":"canon"}}
  @subject:report-generation
  @case:happy
  @report.generation.10
  Scenario: A fresh canon init repo's report fills its task columns from the sqlite hot tier
    Given a fresh canon init repo, whose canon.yaml routes task to a sqlite hot rung, with a change carrying two tasks and its brief approved
    When one task is attested and flipped with canon task done, and the report is generated with canon report build
    Then the trust matrix carries each task's title and status, the flipped one done
    And no task record reached the git ledger, so those rows were read from sqlite
    And no kind is named as not read directly and no warning is printed
    And canon report check reads the same report back

  # canon: {"schema":1,"at":"2026-10-12T14:00:00Z","actor":{"agent_id":"canon"}}
  @subject:report-generation
  @case:failure
  @report.generation.11
  Scenario: A missing hot db is no rows and an unreadable one fails the report loud
    Given a canon.yaml backing hot with a sqlite file that does not exist yet
    When the report is generated
    Then it succeeds with no warning and does not create the file
    And once that file holds something that is not a sqlite database, the report exits 1 naming the file instead of rendering without its tasks

  # canon: {"schema":1,"at":"2026-10-12T16:00:00Z","actor":{"agent_id":"canon"}}
  @subject:report-generation
  @case:edge
  @report.generation.12
  Scenario: A record held by both the git ledger and the sqlite hot tier is counted once
    Given a fresh canon init repo whose flipped task has one evidence record in the git ledger
    And a byte-identical copy of that record written to the sqlite hot tier
    When the report is generated
    Then the task's evidence count is 1, not 2

  # canon: {"schema":1,"at":"2026-10-22T04:50:24Z","actor":{"agent_id":"canon-impl-027"}}
  @subject:report-generation
  @case:edge
  @report.generation.13
  Scenario: A report check reads its empty sources from scratch and writes nothing into the repo
    Given a committed report and a repository whose r2 and learn sources are empty
    When canon report check runs
    Then it compares and reports no drift, and creates no seed file, directory or parquet under .canon
    And the empty sources are seeded in a scratch directory that is gone when the run ends
