Feature: ATIP: check L2 streams, guides and search
  # canon: {"schema":1,"at":"2026-11-02T10:34:21Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}

  # canon: {"schema":1,"at":"2026-11-02T11:57:34Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-35-check-l2-guides.F1
  @atip.checkl2guides.01
  Scenario: A conforming tool passes skills show, each listed guide and an unknown name with the guide names in expected
    Given a conforming tool
    When atip check runs at L2
    Then skills show, each listed guide and an unknown name pass

  # canon: {"schema":1,"at":"2026-11-02T11:57:53Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-35-check-l2-guides.F1
  @atip.checkl2guides.02
  Scenario: A guide name left out of the list, and a skills show that needs state or writes a file, fail only L2.skills
    Given a guide name left out of the list, a skills show that needs state or writes a file
    When atip check runs at L2
    Then only L2.skills fails

  # canon: {"schema":1,"at":"2026-11-02T11:58:16Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-35-check-l2-guides.F1
  @atip.checkl2guides.03
  Scenario: An unknown guide name that is not refused with the guide names in expected fails L2.skills
    Given an unknown guide name that is not refused with the guide names in expected
    When atip check runs at L2
    Then L2.skills fails

  # canon: {"schema":1,"at":"2026-11-02T11:58:48Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-35-check-l2-guides.F2
  @atip.checkl2guides.04
  Scenario: A guide line with a wrong option fails only L2.guide-commands and the finding names the guide and the line
    Given a guide line with a wrong option
    When atip check runs at L2
    Then only L2.guide-commands fails and the finding names the guide and the line

  # canon: {"schema":1,"at":"2026-11-02T11:59:11Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-35-check-l2-guides.F2
  @atip.checkl2guides.05
  Scenario: A line with a pipe is skipped and reported, and a placeholder is replaced by a value valid for its argument before the line is read
    Given a guide line with a pipe, and a placeholder in a line
    When atip check runs at L2
    Then the pipe line is skipped and reported, and the placeholder is replaced by a valid value

  # canon: {"schema":1,"at":"2026-11-02T11:59:31Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-35-check-l2-guides.F2
  @atip.checkl2guides.06
  Scenario: Only the lines of fenced code blocks that start with the tool name are targets
    Given a guide with prose that names the tool and a line that does not start with it
    When atip check runs at L2
    Then only the fenced lines that start with the tool name are targets

  # canon: {"schema":1,"at":"2026-11-02T11:59:50Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-35-check-l2-guides.F3
  @atip.checkl2guides.07
  Scenario: A root --help first line without skills show fails only L2.help-guide
    Given a root --help whose first line lacks skills show
    When atip check runs at L2
    Then only L2.help-guide fails

  # canon: {"schema":1,"at":"2026-11-02T12:00:10Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-35-check-l2-guides.F4
  @atip.checkl2guides.08
  Scenario: A conforming stream passes for every declared output and names the output on the start line and the end result
    Given a conforming stream command with two stream outputs
    When atip check runs at L2
    Then every declared output passes and both lines name the output

  # canon: {"schema":1,"at":"2026-11-02T12:00:45Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-35-check-l2-guides.F4
  @atip.checkl2guides.09
  Scenario: A stream without an end line, an event that breaks its schema and a start before the usage error fail only L2.stream
    Given a stream without an end line, an event that breaks its schema, and a start before the usage error
    When atip check runs at L2
    Then each fails only L2.stream

  # canon: {"schema":1,"at":"2026-11-02T12:01:14Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-35-check-l2-guides.F4
  @atip.checkl2guides.10
  Scenario: A tool without a stream command has L2.stream unsupported, and a command left out with notRun is never run
    Given a tool without a stream command, and a command left out with notRun
    When atip check runs at L2
    Then L2.stream is unsupported and the left-out command is never run

  # canon: {"schema":1,"at":"2026-11-02T12:01:47Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-35-check-l2-guides.F5
  @atip.checkl2guides.11
  Scenario: The fixture question finds the expected command in the first five, and a sixth place is an advisory that leaves the level met
    Given a fixture question and a ranking
    When atip check runs at L2
    Then the expected command in the first five passes, and a later place is an advisory that leaves the level met

  # canon: {"schema":1,"at":"2026-11-02T12:02:16Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-35-check-l2-guides.F5
  @atip.checkl2guides.12
  Scenario: A tool without schema --search has L2.search unsupported, and one with it and no fixture question is unprovided
    Given a tool without schema --search, and one with it and no question in the fixture
    When atip check runs at L2
    Then L2.search is unsupported, and unprovided

  # canon: {"schema":1,"at":"2026-11-02T12:02:53Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-35-check-l2-guides.F5
  @atip.checkl2guides.13
  Scenario: Every condition of the guide rows has a seeded case that fails its owner alone, and the conforming tool judges each
    Given the seeded cases of every condition of the guide rows
    When atip check runs at L2 with each case, with its condition off, and with its row forced to pass
    Then the owner alone fails and the conforming tool judges every condition

  # canon: {"schema":1,"at":"2026-11-02T12:03:20Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-35-check-l2-guides.F5
  @atip.checkl2guides.14
  Scenario: The conforming fake and the checker itself pass, and a row forced to pass makes its seeded cases fail
    Given the conforming fake and the checker itself
    When atip check runs at L2 on the fake and at L0 on atip
    Then both pass
