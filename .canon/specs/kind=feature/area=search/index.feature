Feature: Search index
  # canon: {"schema":1,"at":"2026-10-27T14:14:20Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}

  # canon: {"schema":1,"at":"2026-10-27T14:15:22Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:happy
  @flow:canon-0-31-search.F1
  @search.index.01
  Scenario: A search in a repo with no index builds it and writes an ignore file that holds only a star
    Given a repo with records and no `.canon/index` directory
    When canon search runs with a word that occurs in a finding summary
    Then it exits 0, `.canon/index/search.db` exists and `.canon/index/.gitignore` holds only `*`
    And the finding is a hit, `data.index.rebuilt` is true, and a second run of the same search reports it false

  # canon: {"schema":1,"at":"2026-10-27T14:15:24Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:happy
  @flow:canon-0-31-search.F1
  @search.index.02
  Scenario: A finding promoted just before a search is among its results
    Given an index built by an earlier search
    When a finding is added and promoted and the same search runs
    Then the finding is a hit and `data.index.rebuilt` is true
    And a finding staged and not promoted is not a hit

  # canon: {"schema":1,"at":"2026-10-27T14:15:26Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:happy
  @flow:canon-0-31-search.F1
  @search.index.03
  Scenario: Deleting the index file and searching again gives the same result apart from the rebuilt field
    Given a repo whose index was built by a search and a trace
    When the index file and its sidecar files are deleted and the same search and trace run again
    Then each prints the same document as before except `data.index.rebuilt`
    And no record was written from the index

  # canon: {"schema":1,"at":"2026-10-27T14:15:27Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:failure
  @flow:canon-0-31-search.F1
  @search.index.04
  Scenario: A corrupt index file is deleted and rebuilt once with a warning
    Given a file at `.canon/index/search.db` that is not a database
    When canon search runs
    Then it exits 0 with the same hits a clean run gives and `data.index.rebuilt` is true
    And a warning names the file, and the file is now a database that the next run reuses

  # canon: {"schema":1,"at":"2026-10-27T14:15:29Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:failure
  @flow:canon-0-31-search.F1
  @search.index.05
  Scenario: An index directory that cannot be written is built in memory for the run with a warning naming the path
    Given a repo where `.canon/index` is a regular file, so no directory can be created there
    When canon search runs
    Then it exits 0 with the same hits a clean run gives
    And a warning names the `.canon/index` path, and nothing under the repo changed

  # canon: {"schema":1,"at":"2026-10-27T14:15:30Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:failure
  @flow:canon-0-31-search.F1
  @search.index.06
  Scenario: A record input that cannot be read exits 1 with the cause and writes no index
    Given a repo whose sqlite hot store file is not a database
    When canon search runs, and canon trace runs
    Then each exits 1 with SEARCH_INDEX_FAILED naming the hot store and why it cannot be read
    And no index file was written

  # canon: {"schema":1,"at":"2026-10-27T14:15:32Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:edge
  @flow:canon-0-31-search.F1
  @search.index.07
  Scenario: A full rebuild of a stale index over 2,300 records finishes within five seconds
    Given a repo with 2,300 ledger records (945 reviews, 612 scenarios, 515 evidence, 175 findings, 34 subjects, 19 changes) and 19 briefs, and an index built before one record was added
    When canon search runs
    Then the index is rebuilt in full and the run finishes within 5 seconds
    And the rebuilt index holds every record as a node

  # canon: {"schema":1,"at":"2026-10-27T14:15:34Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:edge
  @flow:canon-0-31-search.F1
  @search.index.08
  Scenario: Records in postgres or an object store and staged records are not read and the unread stores are named
    Given a repo whose canon.yaml routes tasks to a postgres hot rung and trajectories to an s3 cold rung, and a finding that is staged and not promoted
    When canon search runs
    Then `data.index.unreadStores` names postgres and s3, and the staged finding is not a hit
    And the run exits 0 without connecting to either store

  # canon: {"schema":1,"at":"2026-10-27T14:15:35Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:edge
  @flow:canon-0-31-search.F1
  @search.index.09
  Scenario: The read leaves name the cache in their effect note, init ignores it, and until then they warn
    Given a repo whose `.gitignore` lacks the `.canon/index/` line
    When canon search runs and then canon init runs and canon search runs again
    Then the first search warns that `.gitignore` lacks the line and names `canon init`, and the second does not
    And `canon schema search` and `canon schema trace` are effect read, and each effectNote names `.canon/index/search.db` and its ignore file

  # canon: {"schema":1,"at":"2026-10-27T14:15:37Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:edge
  @flow:canon-0-31-search.F1
  @search.index.10
  Scenario: Searches that run at the same time on a stale index all succeed and leave one usable index
    Given a repo whose index is stale
    When eight canon search runs start at once
    Then each exits 0 with the same hits
    And the index file is a database that the next run reuses without rebuilding

  # canon: {"schema":1,"at":"2026-10-27T14:15:38Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:edge
  @flow:canon-0-31-search.F1
  @search.index.11
  Scenario: Any change to a record, a brief, a feature file or the hot store makes the index stale and the next run rebuilds before reading
    Given a repo whose index is current
    When a feature file is edited, then a brief is edited, then a hot store row is added, and canon search runs after each
    Then each run reports `data.index.rebuilt` true and finds the new text
    And a run after no change reports it false
