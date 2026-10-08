Feature: cart remove
  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @case:golden
  @cart.remove.01
  Scenario: Removing an item empties its line
    Given a cart holding an item
    When the shopper removes it
    Then the cart is empty

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @case:failure
  @cart.remove.02
  Scenario: Removing an item not in the cart is refused
    Given an empty cart
    When the shopper removes an item
    Then the removal is refused
