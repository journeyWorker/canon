Feature: platformer moving
  # canon: {"schema":1,"at":"2026-07-14T19:26:44.232886Z","actor":{"agent_id":"canon-scaffold"}}

  @platformer.moving.01 @subject:platformer-traversal @case:happy
  Scenario: A moving platform carries the standing player
  # canon: {"schema":1,"at":"2026-07-14T19:26:44.232886Z","actor":{"agent_id":"canon-scaffold"}}
    Given a platform oscillates by one hundred twenty pixels at sixty pixels per second
    And the player stands still on top of it
    When the platform advances one simulation step
    Then the player is displaced by the same delta as the platform
    And the player stays planted on the platform without sinking or sliding off

  @platformer.moving.02 @subject:platformer-traversal @case:edge
  Scenario: Stepping off a platform edge drops cleanly
  # canon: {"schema":1,"at":"2026-07-14T19:26:44.284750Z","actor":{"agent_id":"canon-scaffold"}}
    Given the player stands on a moving platform near its edge
    When the player walks past the platform edge into open air
    Then the player leaves the platform and falls under normal gravity
    And the position changes smoothly with no warp, snap, or clipping
    And the player is no longer carried once contact is lost

  # canon: {"schema":1,"at":"2026-10-26T11:45:24Z","actor":{"agent_id":"impl-030-sb-f4","role":"implementer","session_id":"sb-f4-2026-10-26"}}
  @subject:platformer-traversal
  @case:failure
  @flow:canon-0-30-spec-backfill.F6
  @platformer.moving.03
  Scenario: A player standing on the ground under a moving platform is not carried by it
    Given the player stands on the ground under a platform that oscillates overhead
    When the platform moves through a full cycle, both turnarounds included
    Then the player is not carried: its position does not change and it stays on the ground
    And the platform keeps moving
