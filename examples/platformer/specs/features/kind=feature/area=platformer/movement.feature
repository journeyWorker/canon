Feature: Red panda movement and collision
  # canon: {"schema":1,"at":"2026-07-14T18:43:45.186397Z","actor":{"agent_id":"canon-scaffold"}}

  @platformer.movement.01
  Scenario: Left world edge clamps the player
  # canon: {"schema":1,"at":"2026-07-14T18:43:48.830756Z","actor":{"agent_id":"canon-scaffold"}}
    Given the player stands at spawn near the world's left edge
    When left is held for two seconds
    Then the player's x clamps at the world boundary
    And spamming jump against the wall produces no horizontal drift

  @platformer.movement.02
  Scenario: Ceiling contact cancels upward motion without sticking
  # canon: {"schema":1,"at":"2026-07-14T18:43:50.116558Z","actor":{"agent_id":"canon-scaffold"}}
    Given the player stands under a platform tile
    When the player jumps into the tile's underside
    Then vertical velocity cancels the instant the head makes contact
    And the player falls back without sticking or horizontal drift

  @platformer.movement.03
  Scenario: No double jump; coyote and buffered jumps fire
  # canon: {"schema":1,"at":"2026-07-14T18:43:52.540391Z","actor":{"agent_id":"canon-scaffold"}}
    Given the player is airborne over a gap
    When jump is pressed repeatedly mid-air
    Then no second jump fires
    And a jump pressed within coyote time after leaving a ledge still fires
    And a jump buffered just before landing fires on the landing frame

  @platformer.movement.04
  Scenario: Falling into a pit respawns without touching score
  # canon: {"schema":1,"at":"2026-07-14T18:43:54.971812Z","actor":{"agent_id":"canon-scaffold"}}
    Given the player has collected some acorns
    When the player falls more than 200 pixels below the world
    Then the player respawns at the spawn point
    And the score and collected acorns are unchanged
