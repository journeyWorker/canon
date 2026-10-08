Feature: query read
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:query-reads
  @case:edge
  @query.read.01
  Scenario: A re-materialized scenario reads as one current record and the survivor is the latest
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a scenario index record written once and then written again an hour later under a changed title
    When the ledger is read raw, both generations are on disk, because the ledger is append-only
    Then a query for that kind reports exactly one record for the natural key
    And the record it reports carries the later title, never an arbitrary generation

  @subject:query-reads
  @case:happy
  @query.read.02
  Scenario: A subject walked forward through its status chain reads as one row
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a subject advanced across the forward status chain, each step appending a new version
    When the subject corpus is queried
    Then one row answers for that subject id
    And its status is the last one the chain reached

  @subject:query-reads
  @case:edge
  @query.read.03
  Scenario: A corpus carrying no supersession folds to itself
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a corpus in which every natural key holds exactly one version
    When it is queried
    Then the returned row count equals the stored record count
    And the fold has taken nothing away, because there was nothing to supersede

  @subject:query-reads
  @case:edge
  @query.read.04
  Scenario: A review's natural key carries the pinned commit, so two attestations at two pins are two records
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a review attesting one scenario of one project at one pinned commit
    When its natural key is resolved
    Then the key is the project, the scenario and the pin joined together
    And two attestations of the same scenario at two commits are two distinct keys, not two versions of one
    And folding by key would therefore drop an attestation rather than collapse a duplicate

  @subject:query-reads
  @case:edge
  @query.read.05
  Scenario: A since cutoff admits records at or after it and nothing older
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given two records of one kind, one five days old and one written now
    When the corpus is queried with a cutoff two days ago
    Then only the record at or after the cutoff is returned
    And the older record is filtered out even though it lives in a different tier than the newer one

  @subject:query-reads
  @case:happy
  @query.read.06
  Scenario: A status filter scopes the result to the records that carry that status
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given two tasks under one change, one open and one done
    When the task corpus is queried filtered to open
    Then only the open task is returned
    And it is named by its own task id, not by position

  @subject:query-reads
  @case:happy
  @query.read.07
  Scenario: A scope filter narrows the rollup, not only the printed rows
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given six tasks under one change, two of them done, and a seventh task under a different change
    When the task corpus is scoped to the first change
    Then the rollup counts two done out of six
    And the task belonging to the other change contributes to neither number

  @subject:query-reads
  @case:failure
  @query.read.08
  Scenario: A kind routed to a tier this repo cannot attach fails loud instead of reading empty
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a kind routed to a rung whose backend has no live connection configured
    When that kind is queried
    Then the command exits nonzero
    And the message names the unreachable rung and the reason it could not be attached
    And no empty result is reported, because an empty read and an unreachable tier are different facts
