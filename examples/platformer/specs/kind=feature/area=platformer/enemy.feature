Feature: platformer enemy
  # canon: {"schema":1,"at":"2026-07-14T19:26:44.055691Z","actor":{"agent_id":"canon-scaffold"}}

  @platformer.enemy.01
  Scenario: Pinecone contact costs a heart and grants brief invulnerability
  # canon: {"schema":1,"at":"2026-07-14T19:26:44.055691Z","actor":{"agent_id":"canon-scaffold"}}
    Given the player has three hearts and a pinecone patrols a nearby ledge
    When the pinecone's hitbox overlaps the player while not invulnerable
    Then one heart is lost and the counter drops to two
    And the player becomes invulnerable for one second, blinking while it lasts
    And a second overlap during that window costs no further heart

  @platformer.enemy.02
  Scenario: Losing every heart shows game over and returns to menu
  # canon: {"schema":1,"at":"2026-07-14T19:26:44.119258Z","actor":{"agent_id":"canon-scaffold"}}
    Given the player has one heart remaining during play
    When a pinecone contact removes that last heart
    Then the hearts counter reads zero and a game over screen appears
    And the simulation stops accepting movement input
    And dismissing game over returns to the level-select menu
