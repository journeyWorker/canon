Feature: platformer hud
  # canon: {"schema":1,"at":"2026-07-14T19:26:44.332203Z","actor":{"agent_id":"canon-scaffold"}}

  @platformer.hud.01 @subject:platformer-traversal @case:happy
  Scenario: Pause freezes the simulation and resume continues it
  # canon: {"schema":1,"at":"2026-07-14T19:26:44.332203Z","actor":{"agent_id":"canon-scaffold"}}
    Given the game is running with the player in motion
    When the pause key is pressed and the pause menu opens
    Then the simulation halts and the timer stops advancing
    And choosing resume continues from the exact frozen state
    And the elapsed time excludes the paused interval

  @platformer.hud.02 @subject:platformer-traversal @case:happy
  Scenario: HUD reflects hearts acorns and level in real time
  # canon: {"schema":1,"at":"2026-07-14T19:26:44.372577Z","actor":{"agent_id":"canon-scaffold"}}
    Given the player is mid-level with three hearts and some acorns collected
    When a heart is lost and another acorn is collected
    Then the top-left hearts row and top-right acorn count update the same frame
    And the top-center label shows the current level and running timer
    And no HUD value lags behind the simulation state

  # canon: {"schema":1,"at":"2026-10-26T11:45:24Z","actor":{"agent_id":"impl-030-sb-f4","role":"implementer","session_id":"sb-f4-2026-10-26"}}
  @subject:platformer-traversal
  @case:failure
  @flow:canon-0-30-spec-backfill.F6
  @platformer.hud.03
  Scenario: The pause key opens no pause menu outside play
    Given the level-select menu is showing, and later a completed level's complete screen
    When the pause key is pressed on each
    Then no pause menu opens and the simulation is not paused
    And the screen shown stays the screen that was showing
