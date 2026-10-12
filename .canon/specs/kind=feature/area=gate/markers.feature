Feature: gate markers
  # canon: {"schema":1,"at":"2026-10-12T18:00:00Z","actor":{"agent_id":"018-n3-impl"}}

  # canon: {"schema":1,"at":"2026-10-12T18:00:00Z","actor":{"agent_id":"018-n3-impl"}}
  @subject:gate-trust-spine
  @case:edge
  @gate.markers.01
  Scenario: Fabrication markers match whole tokens, never inside a path, a file name, a URL or a longer word
    Given evidence notes where "would pass", "TBD" or "n/a" stands on its own, in any letter case, beside punctuation or non-ASCII text
    And notes where the same letters sit inside "pack.json/atlas", "assets/n/a/", "n/a.json", "https://example.com/tbd", "tbd_notes.md", "x-tbd" or "tbdx"
    When each note is scanned
    Then every standalone marker is a fabricated-evidence finding, one per marker per field
    And no marker inside a path, a file name, a URL or a longer word is a finding

  # canon: {"schema":1,"at":"2026-10-12T18:00:00Z","actor":{"agent_id":"018-n3-impl"}}
  @subject:gate-trust-spine
  @case:failure
  @gate.markers.02
  Scenario: A standalone marker beside a path holding its letters still blocks the flip
    Given a faithful evidence record whose note reads "loaded pack.json/atlas; GPU path: n/a"
    When the note is scanned and the flip is decided
    Then the standalone "n/a" is one fabricated-evidence finding
    And the flip is blocked with fabricated-evidence only
