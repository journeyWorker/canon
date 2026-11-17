Feature: atip daterefusals
  # canon: {"schema":1,"at":"2026-11-06T16:46:30Z","actor":{"agent_id":"impl-date-refusals","role":"implementer","session_id":"s-0-38-date-refusals"}}

  # canon: {"schema":1,"at":"2026-11-06T16:46:30Z","actor":{"agent_id":"impl-date-refusals","role":"implementer","session_id":"s-0-38-date-refusals"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-38-date-refusals.F1
  @atip.daterefusals.01
  Scenario: A second of 60 is refused in the time places whatever the time, and the second before and the next day are taken
    Given 2030-01-01T23:59:60Z, 1998-12-31T23:59:60Z, 2030-01-01T10:00:60Z, a second of 60 with a fraction and with an offset, and 23:59:59 and the next day's 00:00:00Z
    When each is given to the places that read a time
    Then every second of 60 is refused with USAGE and exit 2 naming the option, the value as received and the same instant written as the second after the preceding second (23:59:60Z names the next day's 00:00:00Z, 10:00:60Z names 10:01:00Z, a following second past year 9999 names none), nothing is written, and the two taken values pass as before

  # canon: {"schema":1,"at":"2026-11-06T16:46:39Z","actor":{"agent_id":"impl-date-refusals","role":"implementer","session_id":"s-0-38-date-refusals"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-38-date-refusals.F2
  @atip.daterefusals.02
  Scenario: A date, a time or an offset that does not exist is refused in every place that takes a date
    Given 2026-02-30, 2026-02-29, 2026-04-31, month 13, hour 25, minute 61 and an offset of +24:00, and 2028-02-29
    When each is given to every place that takes a date, the dates alone to the date options
    Then each missing value is refused with USAGE and exit 2 naming the option, and a value with a time also comes back as received with nothing written, and 2028-02-29 is read as a date

  # canon: {"schema":1,"at":"2026-11-06T16:46:43Z","actor":{"agent_id":"impl-date-refusals","role":"implementer","session_id":"s-0-38-date-refusals"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-38-date-refusals.F3
  @atip.daterefusals.03
  Scenario: A misspelled value is refused in every place that takes a date
    Given 2026-2-30 and two other values spelled wrong
    When each is given to every place that takes a date
    Then each is refused with USAGE and exit 2, naming the option, the value as received and nothing written
