Feature: checkout pay
  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @subject:checkout
  @case:golden
  @checkout.pay.01
  Scenario: Paying with a valid card places the order
    Given a cart ready for checkout
    When the shopper pays with a valid card
    Then the order is placed

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @subject:checkout
  @case:failure
  @checkout.pay.02
  Scenario: Paying with an expired card is refused
    Given a cart ready for checkout
    When the shopper pays with an expired card
    Then the payment is refused
