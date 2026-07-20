@idolive-hub
Feature: Idolive hub
  # rationale comment about the feature

  @idolive.hub.01 @p2 @live-api @reviewed
  Scenario: Opening the hub shows recent replays
    Given the guest opens the idolive hub
    When the hub finishes loading
    Then the screen shows recent replays

  @idolive.hub.25 @p2 @web @render @reviewed
  Scenario: The hub shows the map button
    Given the hub has loaded
    Then a map button is visible
