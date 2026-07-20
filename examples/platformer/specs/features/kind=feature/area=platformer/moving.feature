Feature: platformer moving
  # canon: {"schema":1,"at":"2026-07-14T19:26:44.232886Z","actor":{"agent_id":"canon-scaffold"}}

  @platformer.moving.01
  Scenario: A moving platform carries the standing player
  # canon: {"schema":1,"at":"2026-07-14T19:26:44.232886Z","actor":{"agent_id":"canon-scaffold"}}
    Given a platform oscillates by one hundred twenty pixels at sixty pixels per second
    And the player stands still on top of it
    When the platform advances one simulation step
    Then the player is displaced by the same delta as the platform
    And the player stays planted on the platform without sinking or sliding off

  @platformer.moving.02
  Scenario: Stepping off a platform edge drops cleanly
  # canon: {"schema":1,"at":"2026-07-14T19:26:44.284750Z","actor":{"agent_id":"canon-scaffold"}}
    Given the player stands on a moving platform near its edge
    When the player walks past the platform edge into open air
    Then the player leaves the platform and falls under normal gravity
    And the position changes smoothly with no warp, snap, or clipping
    And the player is no longer carried once contact is lost
