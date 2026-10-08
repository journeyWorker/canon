Feature: wishlist save
  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @subject:wishlist
  @wishlist.save.01
  Scenario: Saving an item keeps it on the wishlist
    Given an empty wishlist
    When the shopper saves an item
    Then the wishlist holds that item
