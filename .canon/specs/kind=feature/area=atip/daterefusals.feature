Feature: atip daterefusals
  # canon: {"schema":1,"at":"2026-11-06T16:46:30Z","actor":{"agent_id":"impl-date-refusals","role":"implementer","session_id":"s-0-38-date-refusals"}}

  # canon: {"schema":1,"at":"2026-11-06T16:46:30Z","actor":{"agent_id":"impl-date-refusals","role":"implementer","session_id":"s-0-38-date-refusals"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-38-date-refusals.F1
  @atip.daterefusals.01
  Scenario: A second of 60 is refused in the twelve time places whatever the time, and the second before and the next day are taken
    Given 2030-01-01T23:59:60Z, 1998-12-31T23:59:60Z, 2030-01-01T10:00:60Z, a second of 60 with a fraction and with an offset, and 23:59:59 and the next day's 00:00:00Z
    When each is given to the twelve places that read a time
    Then every second of 60 is refused with USAGE and exit 2 naming the option, the value as received and the same instant written as the second after the preceding second (23:59:60Z names the next day's 00:00:00Z, 10:00:60Z names 10:01:00Z, a following second past year 9999 names none), nothing is written, and the two taken values pass as before

  # canon: {"schema":1,"at":"2026-11-06T16:46:39Z","actor":{"agent_id":"impl-date-refusals","role":"implementer","session_id":"s-0-38-date-refusals"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-38-date-refusals.F2
  @atip.daterefusals.02
  Scenario: A date, a time or an offset that does not exist is refused in the twelve places and the message says what does not exist
    Given 2026-02-30, 2026-02-29, 2026-04-31, month 13, hour 25, minute 61 and an offset of +24:00, and 2028-02-29
    When each is given to the twelve places
    Then each missing value is refused with USAGE and exit 2 and a message naming the day, month, hour, minute or offset that does not exist, 2028-02-29 is taken, and nothing is written

  # canon: {"schema":1,"at":"2026-11-06T16:46:43Z","actor":{"agent_id":"impl-date-refusals","role":"implementer","session_id":"s-0-38-date-refusals"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-38-date-refusals.F3
  @atip.daterefusals.03
  Scenario: A misspelled value keeps the refusal of 0.37 that says the spelling
    Given 2026-2-30 and two other values spelled wrong
    When each is given to the twelve places
    Then each is refused with the 0.37 sentence that names the RFC 3339 spelling and not a missing date, and the order check of --after and --before is as before

  # canon: {"schema":1,"at":"2026-11-06T16:46:46Z","actor":{"agent_id":"impl-date-refusals","role":"implementer","session_id":"s-0-38-date-refusals"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-38-date-refusals.F4
  @atip.daterefusals.04
  Scenario: The CHANGELOG lists the leap second as a breaking change and the new sentence as a change
    Given the CHANGELOG of the unreleased entry
    When its Breaking table and its Changed list are read
    Then a row names the refused second of 60 and the second after the preceding second as the way to move, and a Changed entry says the refusal names what does not exist

  # canon: {"schema":1,"at":"2026-11-06T16:47:42Z","actor":{"agent_id":"impl-date-refusals","role":"implementer","session_id":"s-0-38-date-refusals"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-38-date-refusals.F4
  @atip.daterefusals.05
  Scenario: Without the row of the leap second or the entry of the new sentence, the check of the CHANGELOG fails
    Given the CHANGELOG with the row of the leap second taken out, with its way to move (the second after the preceding second) taken out, and with the entry of the new sentence taken out
    When the check of the CHANGELOG reads each
    Then each is reported as a missing row, a missing way to move or a missing entry, and the CHANGELOG as it is passes
