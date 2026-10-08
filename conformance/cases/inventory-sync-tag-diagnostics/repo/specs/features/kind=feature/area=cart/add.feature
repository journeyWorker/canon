Feature: cart add
  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @case:failure
  @lane:smoke
  @cart.add.01
  Scenario: Adding an out-of-stock item is refused
    Given an out-of-stock item
    When the shopper adds it
    Then the add is refused

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @case:Not_Kebab
  @cart.add.02
  Scenario: Adding an item puts it in the cart
    Given an empty cart
    When the shopper adds an item
    Then the cart holds that item

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @lane:UPPER
  @cart.add.03
  Scenario: Adding an item twice raises its quantity
    Given a cart holding an item
    When the shopper adds the same item
    Then the cart holds that item with quantity two

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @owner:alice
  @cart.add.04
  Scenario: Adding an item updates the cart total
    Given an empty cart
    When the shopper adds an item
    Then the cart total is the item price
