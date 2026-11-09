Feature: tier retired-keys
  # canon: {"schema":1,"at":"2026-11-09T10:12:57Z","actor":{"agent_id":"impl-0-40-retire-followups","role":"planner"}}

  # canon: {"schema":1,"at":"2026-11-09T10:12:57Z","actor":{"agent_id":"impl-0-40-retire-followups","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:happy
  @flow:canon-0-40-retire-followups.F3
  @tier.retired-keys.01
  Scenario: A retired key canon cannot delete by line is named by its dotted name, with its line when the line is known
    Given a canon.yaml that holds routing.session as a flow mapping, an anchored value or a tagged value
    When canon migrate and canon migrate --commit run
    Then each exits 2 with CONFIG_INVALID and a message that names routing.session
    And an anchored or tagged key is named with its line, and the file is unchanged

  # canon: {"schema":1,"at":"2026-11-09T10:13:05Z","actor":{"agent_id":"impl-0-40-retire-followups","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:failure
  @flow:canon-0-40-retire-followups.F3
  @tier.retired-keys.02
  Scenario: Only the keys that cannot be deleted are named, and a canon.yaml without retired keys is not refused
    Given a canon.yaml whose routing.event is a clean line and whose routing.session is anchored, and one with no retired key at all
    When canon migrate runs
    Then the refusal of the first names routing.session and not routing.event
    And the second is not refused
