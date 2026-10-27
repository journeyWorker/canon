Feature: Placeholder nodes
  # canon: {"schema":1,"at":"2026-10-27T14:15:18Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}

  # canon: {"schema":1,"at":"2026-10-27T14:16:10Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:happy
  @flow:canon-0-31-search.F4
  @search.graph.01
  Scenario: A finding about a change with no source gets a marked placeholder node and no error
    Given a finding whose change has no brief, no plan source entry and no committed change record
    When canon search runs for the finding with --json
    Then the change is a neighbor with `placeholder` true, the count of references to it, and no error
    And the placeholder is not a hit for any query

  # canon: {"schema":1,"at":"2026-10-27T14:16:11Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:failure
  @flow:canon-0-31-search.F4
  @search.graph.02
  Scenario: Tracing a placeholder id exits 2
    Given the placeholder change of search.graph.01
    When canon trace runs with the change id and with the placeholder's node id
    Then each exits 2 naming the three accepted shapes

  # canon: {"schema":1,"at":"2026-10-27T14:16:13Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:edge
  @flow:canon-0-31-search.F4
  @search.graph.03
  Scenario: The placeholder ids are the distinct ids canon query reports as having no source
    Given findings, evidence and a subject whose changes, tasks, scenarios and review ids have no committed source
    When canon query reports the references of each kind and canon search lists the placeholders in the neighbors of every node
    Then the set of placeholder ids equals the set of distinct ids the query reports
