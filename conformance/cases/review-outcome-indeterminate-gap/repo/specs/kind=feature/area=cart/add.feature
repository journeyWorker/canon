Feature: cart add
  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @subject:cart
  @cart.add.01
  Scenario: Adding an item puts it in the cart
    Given an empty cart
    When the shopper adds an item
    Then the cart holds that item
