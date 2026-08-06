Feature: plugin overlay
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:plugin-overlay
  @plugin.overlay.01
  Scenario: An overlay body is admitted only when it satisfies its manifest exactly
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a manifest declaring an overlay over scenario, joined on project and scenario, with a bool field and a string-list field
    When a body carrying both join-key values and both declared fields at their declared types is validated
    Then validation passes with no diagnostic
    And the join-key fields are recognized rather than flagged as outside the declared set

  @subject:plugin-overlay
  @plugin.overlay.02
  Scenario: A field the manifest never declared is refused, named
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a body that is otherwise well formed against its manifest
    When it carries one extra field the manifest never declared
    Then validation fails with an undeclared-field diagnostic naming that field
    And an overlay may only ever say what its manifest declared it could say

  @subject:plugin-overlay
  @plugin.overlay.03
  Scenario: An invalid overlay body never reaches disk
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a body missing two of its manifest's declared fields
    When a write of that overlay is attempted against a tier
    Then the write fails as a validation error
    And no directory for that overlay identity exists afterwards, because validation gates the write

  @subject:plugin-overlay
  @plugin.overlay.04
  Scenario: A synced overlay projects its declared fields onto the matching core read
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repo whose scenario index has been synced and whose overlay has then been synced over it
    When the scenarios are queried with that plugin
    Then the covered scenario carries the overlay's covered flag as true under the overlay's own identity
    And it carries the surface reference the overlay recorded

  @subject:plugin-overlay
  @plugin.overlay.05
  Scenario: A projection read never rewrites the core record
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a core scenario record on disk and a matching overlay record beside it
    When the scenarios are queried with the plugin and the projection is confirmed to have matched
    Then the core record's on-disk bytes are unchanged
    And the overlay read is therefore a view, never a migration of the core corpus

  @subject:plugin-overlay
  @plugin.overlay.06
  Scenario: A malformed overlay record is diagnosed and skipped while its siblings still project
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given two scenarios, one with a malformed overlay record and one with a well-formed overlay record
    When the scenarios are queried with that plugin
    Then the command still succeeds, because one bad record never aborts the whole projection
    And the malformed record is diagnosed on the error stream
    And the scenario it targeted carries no projected overlay at all
    And the well-formed sibling still projects its declared field

  @subject:plugin-overlay
  @plugin.overlay.07
  Scenario: Without the plugin flag the core read is unchanged even with overlay data present
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a scenario queried before any plugin manifest or overlay record exists
    When a manifest and a matching overlay record are planted and the same query is run again without naming the plugin
    Then the two outputs are byte-identical
    And no plugin, overlays or overlay key appears anywhere in the payload

  @subject:plugin-overlay
  @plugin.overlay.08
  Scenario: A plugin projects only its own overlays, never a namespace sibling's
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given two installed plugins sharing one namespace but declaring different overlay kinds
    And a record of each planted against the same scenario
    When the scenarios are queried naming only the first plugin
    Then that plugin's own overlay projects
    And the sibling's overlay never appears, because a shared namespace is not shared authority

  @subject:plugin-overlay
  @plugin.overlay.09
  Scenario: A second sync over an unchanged index writes nothing new
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a synced scenario index of two scenarios, over which the overlay has been synced once, reporting two written
    When the overlay is synced a second time with nothing changed
    Then it reports zero written and two deduped
    And the overlay record count on disk is exactly what it was
