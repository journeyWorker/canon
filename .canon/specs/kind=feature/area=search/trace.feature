Feature: Trace chain
  # canon: {"schema":1,"at":"2026-10-27T14:15:18Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}

  # canon: {"schema":1,"at":"2026-10-27T14:16:00Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:happy
  @flow:canon-0-31-search.F3
  @search.trace.01
  Scenario: Tracing a finding lists the review, the finding, its fix, the re-review and the evidence in time order with the archived versions
    Given review R1 that raised a finding about a scenario, the finding closed fixed at commit S2, review R2 pinned at S2 that pushed R1 to the archive, and the scenario's evidence recorded at S2 and again at S3, which pushed the S2 record to the archive
    When canon trace runs with the finding key and --json
    Then `data.chain` is R1, the finding, the fix, R2 and the evidence at S2 in that time order, each with its time, id, kind, role, sha and edges
    And R1 and the S2 evidence are the archived versions, and the evidence at S3 is not in the chain

  # canon: {"schema":1,"at":"2026-10-27T14:16:02Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:happy
  @flow:canon-0-31-search.F3
  @search.trace.02
  Scenario: Tracing the fix commit gives the same chain as tracing the finding
    Given the records of the chain of search.trace.01
    When canon trace runs with the full S2 sha, and with its first seven characters
    Then both print the chain the finding key printed
    And a sha prefix that fits one commit needs no more characters

  # canon: {"schema":1,"at":"2026-10-27T14:16:03Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:happy
  @flow:canon-0-31-search.F3
  @search.trace.03
  Scenario: Tracing a review id starts from every review the run wrote, and a 26-character operand with no review is read as a sha prefix
    Given a review run that wrote reviews of two scenarios and a finding that names the run and one scenario
    When canon trace runs with the review id, and with a 26-character hex operand that is no review id and prefixes a commit
    Then the first starts from both reviews and reaches the finding, whose review edge goes to the review of its scenario and not to the other
    And the second traces the commit

  # canon: {"schema":1,"at":"2026-10-27T14:16:05Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:failure
  @flow:canon-0-31-search.F3
  @search.trace.04
  Scenario: An unknown id, an ambiguous sha prefix and an unrecognized shape exit 2 and name the three accepted shapes
    Given a repo with a review, findings and evidence at two commits that share a prefix
    When canon trace runs with an id no record names, with a prefix both commits share, and with a word that has none of the three shapes
    Then each exits 2 and the message lists the review id, the commit sha and the finding key forms
    And nothing is written apart from the index

  # canon: {"schema":1,"at":"2026-10-27T14:16:07Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:edge
  @flow:canon-0-31-search.F3
  @search.trace.05
  Scenario: A finding recorded without a scenario has no evidence in its chain
    Given a finding with no scenario, fixed at a commit where a scenario has evidence
    When canon trace runs with the finding key
    Then the chain holds the finding and its fix and no evidence

  # canon: {"schema":1,"at":"2026-10-27T14:16:08Z","actor":{"agent_id":"impl-031-search","role":"implementer","session_id":"search-2026-10-27"}}
  @subject:search-index
  @case:edge
  @flow:canon-0-31-search.F3
  @search.trace.06
  Scenario: A fix reviewed again and a fix of a fix chain through every round in time order
    Given round 1 finding A fixed at commit X, round 2 finding B reviewed at X and fixed at Y, and round 3 finding C introduced by Y
    When canon trace runs with the key of finding A
    Then the chain holds A, its fix, B, its fix and C joined by re-review and fix-of-fix edges
    And equal times are ordered by id
