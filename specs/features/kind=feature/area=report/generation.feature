Feature: report generation
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:report-generation
  @report.generation.01
  Scenario: The report is generated into the repo, never authored by hand
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository with no report written yet
    When the report is generated against the resolved repo root
    Then the command exits zero and names the path it wrote
    And the file exists at that path under the repo root
    And it opens with the generated report heading, not with anything a person typed

  @subject:report-generation
  @report.generation.02
  Scenario: Checking a repo that never generated a report is a missing verdict, not a pass
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository with no report file on disk
    When the drift gate is run
    Then it exits nonzero and reports the report as missing
    And an absent report is never mistaken for an up-to-date one

  @subject:report-generation
  @report.generation.03
  Scenario: The drift gate passes immediately after a write
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a report just written over an unchanged corpus
    When the drift gate is run against it
    Then it exits zero and reports no drift
    And nothing about the corpus had to change for the gate to agree

  @subject:report-generation
  @report.generation.04
  Scenario: A hand edit to the committed report is drift
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a generated report that a person has since edited on disk
    When the drift gate is run
    Then it exits nonzero and names the outcome as drift
    And the comparison is over the file's bytes, so an edit anywhere in it is caught

  @subject:report-generation
  @report.generation.05
  Scenario: The full check lifecycle moves missing then clean then drifted
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a fresh repository driven through the real command
    When the gate is run before the write, after the write, and after a hand edit
    Then the three runs report missing, then no drift, then drift
    And only the middle one exits zero

  @subject:report-generation
  @report.generation.06
  Scenario: Two consecutive renders of one unchanged corpus are byte-identical
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a fixture corpus that does not change between runs
    When the report is rendered twice in a row
    Then the two renderings are equal byte for byte
    And the drift gate above is therefore measuring the corpus, not run-to-run noise

  @subject:report-generation
  @report.generation.07
  Scenario: A snapshot exports every mart as Parquet plus a manifest listing exactly them
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository and an output directory for the snapshot
    When the report is run in snapshot mode against that directory
    Then it reports nine tables and writes one Parquet file per mart
    And the manifest beside them lists exactly those nine tables
    And the markdown report is not written, because exporting and generating are different actions

  @subject:report-generation
  @report.generation.08
  Scenario: A multi-tier repo is warned about the kinds the report does not read directly
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repo routing some kinds to rungs the report does not read directly and others to a directly-read rung
    When the report is generated
    Then it carries a section naming exactly the kinds it did not read directly, sorted
    And it points the reader at the query command for those kinds
    And a directly-read kind is never named there

  @subject:report-generation
  @report.generation.09
  Scenario: A repo whose every routed rung is read directly gets no boundary warning at all
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repo whose routing resolves every kind to a directly-read rung
    When the report is generated
    Then the boundary section is absent entirely
    And the warning is a statement about this repo's routing, not boilerplate
