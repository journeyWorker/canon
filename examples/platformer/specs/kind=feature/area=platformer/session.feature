Feature: Session flow: collecting, winning, restarting
  # canon: {"schema":1,"at":"2026-07-14T18:43:46.462180Z","actor":{"agent_id":"canon-scaffold"}}

  @platformer.session.01
  Scenario: Collecting every acorn shows a full counter
  # canon: {"schema":1,"at":"2026-07-14T18:43:56.266882Z","actor":{"agent_id":"canon-scaffold"}}
    Given a fresh level with ten acorns
    When the player collects every acorn and reaches the squirrel
    Then the counter reads ten out of ten on the win overlay

  @platformer.session.02
  Scenario: Win overlay freezes gameplay input
  # canon: {"schema":1,"at":"2026-07-14T18:43:57.532151Z","actor":{"agent_id":"canon-scaffold"}}
    Given the win overlay is showing
    When movement and jump keys are held
    Then the player's position does not change

  @platformer.session.03
  Scenario: R restarts to a clean initial state
  # canon: {"schema":1,"at":"2026-07-14T18:43:58.788993Z","actor":{"agent_id":"canon-scaffold"}}
    Given a won game with a nonzero score
    When R is pressed
    Then the player, score, camera, and every acorn reset to the initial state
    And a second win-and-restart cycle resets just as cleanly

  @platformer.session.04
  Scenario: Idle and resize sessions stay error-free
  # canon: {"schema":1,"at":"2026-07-14T18:44:00.046852Z","actor":{"agent_id":"canon-scaffold"}}
    Given a running game left idle for sixty seconds
    When the viewport is resized repeatedly during play
    Then the console stays free of errors and warnings
    And the player's settled position shows no drift
