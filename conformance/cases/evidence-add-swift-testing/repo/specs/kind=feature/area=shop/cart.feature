Feature: shop cart
  # canon: {"schema":1,"at":"2026-10-12T00:00:00Z","actor":{"agent_id":"canon"}}

  # canon: {"schema":1,"at":"2026-10-12T00:00:00Z","actor":{"agent_id":"canon"}}
  @shop.cart.01
  Scenario: Behavior shop.cart.01
    Given a cart
    When the shopper acts 01
    Then the cart holds result 01

  # canon: {"schema":1,"at":"2026-10-12T00:00:00Z","actor":{"agent_id":"canon"}}
  @shop.cart.02
  Scenario: Behavior shop.cart.02
    Given a cart
    When the shopper acts 02
    Then the cart holds result 02

  # canon: {"schema":1,"at":"2026-10-12T00:00:00Z","actor":{"agent_id":"canon"}}
  @shop.cart.03
  Scenario: Behavior shop.cart.03
    Given a cart
    When the shopper acts 03
    Then the cart holds result 03

  # canon: {"schema":1,"at":"2026-10-12T00:00:00Z","actor":{"agent_id":"canon"}}
  @shop.cart.05
  Scenario: Behavior shop.cart.05
    Given a cart
    When the shopper acts 05
    Then the cart holds result 05
