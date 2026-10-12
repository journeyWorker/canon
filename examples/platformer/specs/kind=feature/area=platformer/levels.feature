Feature: platformer levels
  # canon: {"schema":1,"at":"2026-07-14T19:26:44.158043Z","actor":{"agent_id":"canon-scaffold"}}

  @platformer.levels.01
  Scenario: Completing a level unlocks the next in the menu
  # canon: {"schema":1,"at":"2026-07-14T19:26:44.158043Z","actor":{"agent_id":"canon-scaffold"}}
    Given only level one is unlocked in the level-select menu
    When the player reaches the squirrel goal and completes level one
    Then the level-complete screen shows collected acorns and elapsed time
    And level two becomes selectable in the menu
    And returning to the menu keeps level two unlocked

  @platformer.levels.02
  Scenario: Best acorns and time persist per level across reloads
  # canon: {"schema":1,"at":"2026-07-14T19:26:44.197954Z","actor":{"agent_id":"canon-scaffold"}}
    Given level one was cleared with a best of eleven acorns in fifty seconds
    When the browser is reloaded and the menu is reopened
    Then level one still shows eleven acorns and fifty seconds as its best
    And clearing it again with fewer acorns or a slower time leaves the best unchanged
    And a faster time or more acorns replaces the stored best
