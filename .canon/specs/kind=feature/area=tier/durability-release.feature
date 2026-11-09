Feature: tier durability-release
  # canon: {"schema":1,"at":"2026-11-09T10:13:15Z","actor":{"agent_id":"impl-0-40-retire-followups","role":"planner"}}

  # canon: {"schema":1,"at":"2026-11-09T10:13:15Z","actor":{"agent_id":"impl-0-40-retire-followups","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:happy
  @flow:canon-0-40-retire-followups.F4
  @tier.durability-release.01
  Scenario: The durability order tests still pass in a debug build, where the trace exists
    Given a debug build of canon-store and canon-gate
    When the durability order tests run
    Then file sync, rename and directory sync, and two renames then one directory sync, are seen in that order

  # canon: {"schema":1,"at":"2026-11-09T10:13:23Z","actor":{"agent_id":"impl-0-40-retire-followups","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:failure
  @flow:canon-0-40-retire-followups.F4
  @tier.durability-release.02
  Scenario: A release build has no trace module and builds, and no trace call stands outside debug_assertions
    Given a release build of canon-store and canon-gate
    When they are built, and their tests compiled
    Then both succeed, the trace module and its Event do not exist, and every trace call in the source stands under cfg(debug_assertions)
