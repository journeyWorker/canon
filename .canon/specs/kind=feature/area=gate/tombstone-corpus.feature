Feature: gate tombstone-corpus
  # canon: {"schema":1,"at":"2026-11-09T10:12:21Z","actor":{"agent_id":"impl-0-40-retire-followups","role":"planner"}}

  # canon: {"schema":1,"at":"2026-11-09T10:12:21Z","actor":{"agent_id":"impl-0-40-retire-followups","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:failure
  @flow:canon-0-40-retire-followups.F1
  @gate.tombstone-corpus.02
  Scenario: A corpus that was never read still judges the evidence, and an empty corpus with no tombstone is not read as a deleted scenario
    Given the same evidence with the scenario kind routed outside the git tier, so no scenario is read and no tombstone with it
    When canon gate check runs
    Then it reports the evidence as stale-evidence as before
    And a context whose corpus is empty and whose tombstone set is empty does not take a scenario for a deleted one

  # canon: {"schema":1,"at":"2026-11-09T10:13:43Z","actor":{"agent_id":"impl-0-40-retire-followups","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-40-retire-followups.F1
  @gate.tombstone-corpus.01
  Scenario: The old evidence of a scenario whose feature file was the last one deleted is not stale evidence and not an unresolvable cell
    Given one scenario with green evidence, more commits since than max_commits_behind allows, and its feature file, the last one, then deleted and canon inventory sync run
    When canon gate check and canon status run there, and in a shallow clone of it
    Then neither reports stale-evidence nor counts the deleted scenario among the staleness_unresolvable cells
    And restoring the feature file and running canon inventory sync makes canon gate check report stale-evidence again
