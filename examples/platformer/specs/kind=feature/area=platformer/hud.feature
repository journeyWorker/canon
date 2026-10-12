Feature: platformer hud
  # canon: {"schema":1,"at":"2026-07-14T19:26:44.332203Z","actor":{"agent_id":"canon-scaffold"}}

  @platformer.hud.01
  Scenario: Pause freezes the simulation and resume continues it
  # canon: {"schema":1,"at":"2026-07-14T19:26:44.332203Z","actor":{"agent_id":"canon-scaffold"}}
    Given the game is running with the player in motion
    When the pause key is pressed and the pause menu opens
    Then the simulation halts and the timer stops advancing
    And choosing resume continues from the exact frozen state
    And the elapsed time excludes the paused interval

  @platformer.hud.02
  Scenario: HUD reflects hearts acorns and level in real time
  # canon: {"schema":1,"at":"2026-07-14T19:26:44.372577Z","actor":{"agent_id":"canon-scaffold"}}
    Given the player is mid-level with three hearts and some acorns collected
    When a heart is lost and another acorn is collected
    Then the top-left hearts row and top-right acorn count update the same frame
    And the top-center label shows the current level and running timer
    And no HUD value lags behind the simulation state
