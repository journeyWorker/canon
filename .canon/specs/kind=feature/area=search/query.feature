Feature: Search query
  # canon: {"schema":1,"at":"2026-10-27T14:15:17Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}

  # canon: {"schema":1,"at":"2026-10-27T14:15:40Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:happy
  @flow:canon-0-31-search.F2
  @search.query.01
  Scenario: A keyword search returns ranked hits with their neighbors and edges
    Given findings, reviews, scenarios and a change that name each other, and two findings that contain the word
    When canon search runs with the word and --json
    Then `data.hits` lists both findings with id, kind, rank, score, title, snippet and placeholder, best score first and equal scores by id
    And `data.neighbors` and `data.edges` hold the one-hop neighbors of each hit, and `data.counts` and `data.index` are present

  # canon: {"schema":1,"at":"2026-10-27T14:15:41Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:happy
  @flow:canon-0-31-search.F2
  @search.query.02
  Scenario: A finding summary written in Korean is found by a two-character word and by a word of three or more characters
    Given a finding whose summary is a Korean sentence, raised in a review of a scenario of the same change
    When canon search runs with a two-character word of it, and with a three-character word of it
    Then both list the finding as a hit
    And its neighbors include the review and the scenario of the same change, and a two-character query orders its hits by id

  # canon: {"schema":1,"at":"2026-10-27T14:15:43Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:edge
  @flow:canon-0-31-search.F2
  @search.query.03
  Scenario: --kind keeps only seeds of those kinds, --limit caps the hits and --expand sets the hops
    Given a finding, a review and a scenario that all contain the word, and a change one hop from the finding
    When canon search runs with --kind finding, with --kind finding --kind review, with --limit 1, and with --expand 0 and --expand 2
    Then only seeds of the given kinds are hits, at most the limit are listed, and `data.counts` says how many matched
    And with --expand 0 there are no neighbors and with --expand 2 the neighbors include nodes two hops away

  # canon: {"schema":1,"at":"2026-10-27T14:15:44Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:failure
  @flow:canon-0-31-search.F2
  @search.query.04
  Scenario: An empty or whitespace query exits 2 and builds nothing
    Given a repo with records and no index
    When canon search runs with an empty argument and with an argument of only spaces
    Then each exits 2 with SEARCH_QUERY_EMPTY
    And no `.canon/index` directory was created

  # canon: {"schema":1,"at":"2026-10-27T14:15:46Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:edge
  @flow:canon-0-31-search.F2
  @search.query.05
  Scenario: A query that matches nothing exits 0 with empty hits
    Given a repo with records
    When canon search runs with a word no record contains
    Then it exits 0 and `data.hits` is empty, as are the neighbors and the edges

  # canon: {"schema":1,"at":"2026-10-27T14:15:47Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:edge
  @flow:canon-0-31-search.F2
  @search.query.06
  Scenario: The query is read as literal text: quotes and AND are not syntax
    Given a finding whose summary contains the text `"retry" AND timeout (v2)`
    When canon search runs with that exact text, and with a query holding a lone double quote
    Then the first finds the finding and the second exits 0 without a syntax error
    And a query of `retry AND nothing` does not match it

  # canon: {"schema":1,"at":"2026-10-27T14:15:48Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:edge
  @flow:canon-0-31-search.F2
  @search.query.07
  Scenario: Scenario bodies, evidence companions and brief sections are searchable
    Given a scenario under a Rule with a Background, a doc string, a table and Examples, evidence with a summary and a command result, and a brief with a flow
    When canon search runs with a word from the Background, one from the doc string, one from a table cell, one from the evidence summary, one from the command result, and one from a brief section and a flow step
    Then each finds the scenario, the evidence or the brief node that holds it
    And a hit's id starts with its kind, such as `scenario:`, `evidence:` and `brief:`

  # canon: {"schema":1,"at":"2026-10-27T14:15:50Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:edge
  @flow:canon-0-31-search.F2
  @search.query.08
  Scenario: The graph joins records by reference fields, commits and flow tags
    Given a finding whose fix commit is the reviewed commit of a later finding, a review pinned at that fix commit, evidence at that commit for the scenario, and a scenario tagged with a brief flow step
    When canon search runs for each of them with --json
    Then `data.edges` carries re-review, fix-reviewed, evidence-at-commit and flow edges, and an explicit edge named after the reference field
    And a fix-of-fix edge joins a finding to the finding whose fix commit introduced it, and a finding's review id goes to every review that carries it

  # canon: {"schema":1,"at":"2026-10-27T14:15:52Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:edge
  @flow:canon-0-31-search.F2
  @search.query.09
  Scenario: Only the current version of a record can be a hit
    Given evidence recorded again with a new summary word, which moved the first record to the archive, and a review replaced by a later review of the same scenario by the same reviewer
    When canon search runs with a word only the older versions held, from the evidence summary and from the review's upstream reference
    Then there are no hits
    And a word of the current versions finds them
