Feature: cart add
  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @cart.add.01
  Scenario: Behavior cart.add.01
    Given a cart
    When the shopper acts
    Then the cart holds the result
