Feature: format corpus
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:format-corpus
  @format.corpus.01
  Scenario: A corpus carrying violations fails the command and every class it hit is named
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a corpus root whose records carry real, audited drift
    When canon format is run over that root with no flag beyond the root itself
    Then the command exits nonzero
    And its report names each failure class it observed, bracketed, one heading per class
    And validation ran unconditionally, with no opt-in check flag to forget

  @subject:format-corpus
  @format.corpus.02
  Scenario: The root is a positional argument and a repo-relative root resolves to the same corpus
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a corpus root named as a bare positional path
    When the same corpus is named again as a repo-relative positional root under an explicit repo directory
    Then both invocations exit with the identical code
    And their stdout is byte-identical
    And so the positional root, not a flag, is what selects what gets checked

  @subject:format-corpus
  @format.corpus.03
  Scenario: The failure vocabulary is a closed set and the fixture corpus reaches every audited member
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given the checked-in corpus built to reproduce one donor project's audited drift
    When it is checked
    Then every audited failure class the corpus was built to exercise is observed
    And a class the corpus fails to surface is reported by name as a gap, never passed over
    And no class outside the frozen set can be reported, because the set is the enum

  @subject:format-corpus
  @format.corpus.04
  Scenario: A record sitting outside its kind's partition layout is a layout-grammar violation
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a feature file written flat instead of under its partitioned path
    When the corpus is checked
    Then the file is reported under layout-grammar
    And the violation names that file, not the corpus as a whole

  @subject:format-corpus
  @format.corpus.05
  Scenario: A fourth ad hoc file format is a layout-grammar violation, not a new class
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a corpus holding a lock file in a bespoke shape no partitioned kind declares
    When the corpus is checked
    Then that file is reported
    And it is classed as layout-grammar, because inventing a format is a layout fault
    And the closed class set absorbs it rather than growing a class for it

  @subject:format-corpus
  @format.corpus.06
  Scenario: A partition key that disagrees with the file it labels is refused
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given an inventory file whose optional surface partition segment names one surface
    And whose content declares a different one
    When the corpus is checked
    Then the file is reported under layout-grammar
    And the same segment omitted entirely is accepted, because the segment is optional, not the agreement

  @subject:format-corpus
  @format.corpus.07
  Scenario: Rewording a violation's message changes neither the files scanned nor the violations counted
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a root holding one empty feature stub and one well-formed sibling
    When the corpus is checked
    Then the report says two files were checked, never four, because no file is scanned twice
    And exactly one layout-grammar violation is reported
    And the well-formed sibling accounts for none of it
