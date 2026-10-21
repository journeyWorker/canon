Feature: cart add
  # canon: {"schema":1,"at":"2026-10-12T00:00:00Z","actor":{"agent_id":"canon"}}

  # canon: {"schema":1,"at":"2026-10-12T00:00:00Z","actor":{"agent_id":"canon"}}
  @cart.add.01
  Scenario: Behavior cart.add.01
    Given a cart
    When the shopper acts 01
    Then the cart holds result 01

  # canon: {"schema":1,"at":"2026-10-12T00:00:00Z","actor":{"agent_id":"canon"}}
  @cart.add.02
  Scenario: Behavior cart.add.02
    Given a cart
    When the shopper acts 02
    Then the cart holds result 02

  # canon: {"schema":1,"at":"2026-10-12T00:00:00Z","actor":{"agent_id":"canon"}}
  @cart.add.03
  Scenario: Behavior cart.add.03
    Given a cart
    When the shopper acts 03
    Then the cart holds result 03
