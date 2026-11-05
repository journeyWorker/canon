Feature: atip option forms: the time options take RFC 3339 and nothing else, and the provider list takes the exact names
  # canon: {"schema":1,"at":"2026-11-05T15:55:04Z","actor":{"agent_id":"impl-option-forms","role":"implementer","session_id":"s-0-37-option-forms"}}

  # canon: {"schema":1,"at":"2026-11-05T15:55:14Z","actor":{"agent_id":"impl-option-forms","role":"implementer","session_id":"s-0-37-option-forms"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-37-option-forms.F1
  @atip.optionforms.01
  Scenario: An approval time written in any RFC 3339 spelling of one instant is one payload, and the payload reads no clock
    Given evidence approval-payload and strategy approval-payload, and the instant 2030-01-01T10:00:00Z written as 2030-01-01T10:00:00Z, with a lowercase t and z, with a fraction of zeros, as 19:00:00+09:00 and as 05:00:00-05:00
    When each payload is printed, the same arguments are run twice, and another instant is run
    Then every spelling of the instant prints the same bytes, which hold it as UTC to the nanosecond, the same arguments print the same bytes with no clock read, another instant prints other bytes, and nothing is written

  # canon: {"schema":1,"at":"2026-11-05T15:55:23Z","actor":{"agent_id":"impl-option-forms","role":"implementer","session_id":"s-0-37-option-forms"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-37-option-forms.F1
  @atip.optionforms.02
  Scenario: A date alone, or a padded, one-digit, signed, space-separated or unicode-minus time, is refused for --approval-at with exit 2 and no bytes
    Given --approval-at with a date alone, white space around, a one-digit month or day, a sign on the year, a year of three digits, a space for the T, the minus sign of Unicode in an offset, a time with no seconds or no offset, an impossible date or hour, and nothing
    When evidence approval-payload, strategy approval-payload, strategy rollback and evidence add run with it, with and without --json
    Then each exits 2 with USAGE naming --approval-at, no payload bytes reach stdout, and nothing is written

  # canon: {"schema":1,"at":"2026-11-05T15:55:29Z","actor":{"agent_id":"impl-option-forms","role":"implementer","session_id":"s-0-37-option-forms"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-37-option-forms.F2
  @atip.optionforms.03
  Scenario: evidence add records the approval signed over the time, written in any RFC 3339 spelling of that instant
    Given a story repository whose policy pins an allowed signer, and a detached signature over the payload of 2030-01-01T10:00:00Z
    When evidence add runs with the signature and --approval-at written as that instant in several RFC 3339 spellings
    Then the one approval record is written with the approver, the role human and the instant as UTC, and each later spelling finds the record already written

  # canon: {"schema":1,"at":"2026-11-05T15:55:35Z","actor":{"agent_id":"impl-option-forms","role":"implementer","session_id":"s-0-37-option-forms"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-37-option-forms.F2
  @atip.optionforms.04
  Scenario: evidence add with another instant or a refused spelling records nothing
    Given the same repository and signature, and another instant, an offset that makes another instant, and the spellings that --approval-at refuses
    When evidence add runs with each
    Then another instant is EVIDENCE_APPROVAL_INVALID with the reason SIGNATURE_MISMATCH, a refused spelling is USAGE naming --approval-at, a call without --approval-at is EVIDENCE_APPROVAL_REQUIRED, and nothing is recorded

  # canon: {"schema":1,"at":"2026-11-05T15:55:40Z","actor":{"agent_id":"impl-option-forms","role":"implementer","session_id":"s-0-37-option-forms"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-37-option-forms.F2
  @atip.optionforms.05
  Scenario: strategy rollback records the rollback signed over the time, written in any RFC 3339 spelling of that instant
    Given a promoted strategy, and a detached signature over its rollback payload at an instant
    When strategy rollback runs with --approval-at written as that instant with Z, in lowercase and with offsets
    Then each rollback is recorded, the strategy is rolled back and the actor is recorded

  # canon: {"schema":1,"at":"2026-11-05T15:55:46Z","actor":{"agent_id":"impl-option-forms","role":"implementer","session_id":"s-0-37-option-forms"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-37-option-forms.F2
  @atip.optionforms.06
  Scenario: strategy rollback with another instant or a refused spelling records nothing
    Given the same strategy and signature, another instant of a nanosecond, a second and a day, and the spellings that --approval-at refuses
    When strategy rollback runs with each
    Then another instant is STRATEGY_SIGNATURE_INVALID, a refused spelling is USAGE naming --approval-at, nothing is written and the strategy is still active

  # canon: {"schema":1,"at":"2026-11-05T15:55:51Z","actor":{"agent_id":"impl-option-forms","role":"implementer","session_id":"s-0-37-option-forms"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-37-option-forms.F3
  @atip.optionforms.07
  Scenario: The five date options take a full-date or a date-time, and a date is that day's UTC midnight
    Given --expiry, --as-of, --before, --after and --since in the seven commands that take them, and 2030-01-01, 2030-01-01T00:00:00Z, a lowercase t and z, a fraction with an offset, and 0001-01-01
    When each call runs, a deferral is recorded with --expiry 2030-01-01, and an export window is given the date and the date-time of one day
    Then no call is refused for its time option, the date is the day's UTC midnight and the deferral holds 2030-01-01T00:00:00Z, and the window is taken

  # canon: {"schema":1,"at":"2026-11-05T15:55:57Z","actor":{"agent_id":"impl-option-forms","role":"implementer","session_id":"s-0-37-option-forms"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-37-option-forms.F3
  @atip.optionforms.08
  Scenario: A padded, one-digit, signed, space-separated or impossible time is refused for each date option and nothing is recorded
    Given each date option in each command that takes it, and a one-digit month or day, white space before, after or inside, a sign on the year, a year of three or five digits, a space for the T, the minus sign of Unicode, a time with no seconds or offset, an impossible date, a word and nothing
    When each call runs with --json
    Then each exits 2 with USAGE naming the option and writes nothing, and an --after later than --before is still refused naming --after

  # canon: {"schema":1,"at":"2026-11-05T15:56:02Z","actor":{"agent_id":"impl-option-forms","role":"implementer","session_id":"s-0-37-option-forms"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-37-option-forms.F4
  @atip.optionforms.09
  Scenario: init and the three skills commands take none and the exact provider names
    Given init in a new git directory and skills install, check and doctor in the story repository, with claude, claude,omp, none, a name twice, three names and all four
    When each call runs, and skills install records none
    Then init records the list in canon.yaml, none is recorded as an empty list, and the skills commands are not refused for --providers

  # canon: {"schema":1,"at":"2026-11-05T15:56:08Z","actor":{"agent_id":"impl-option-forms","role":"implementer","session_id":"s-0-37-option-forms"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-37-option-forms.F4
  @atip.optionforms.10
  Scenario: A provider list that is not exact is refused by the four commands and nothing is written
    Given Claude, NONE, None, an empty item at the start, in the middle and at the end, a lone comma, white space around or beside a comma, an empty value, white space only, none beside a name and an unknown name
    When init and the three skills commands run with each, with --json
    Then init exits 2 with INIT_ARGUMENT_INVALID and the skills commands with SKILLS_ARGUMENT_INVALID, each naming --providers, and nothing is written, canon.yaml included

  # canon: {"schema":1,"at":"2026-11-05T15:56:13Z","actor":{"agent_id":"impl-option-forms","role":"implementer","session_id":"s-0-37-option-forms"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-37-option-forms.F5
  @atip.optionforms.11
  Scenario: Each of the seven options declares one domain in every command that has it, as a pattern of characters, and the help says the new spelling
    Given every command document of canon schema, the four commands that take --approval-at, the five date options in their eight places and the four commands that take --providers
    When the declarations and the help are read
    Then the four places of --approval-at declare one pattern and the eight of the date options another, the four of --providers one, init's the skills commands', each is a pattern of characters with no white-space class that the 1.5 profile reads, the help names the date-time, the RFC 3339 date and the lowercase names

  # canon: {"schema":1,"at":"2026-11-05T15:56:19Z","actor":{"agent_id":"impl-option-forms","role":"implementer","session_id":"s-0-37-option-forms"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-37-option-forms.F5
  @atip.optionforms.12
  Scenario: The table of values inside and outside each domain agrees with the real parsers, and a command that declares another domain fails the sweep
    Given the table of values inside and outside each domain, and seeded documents in which a command declares another domain, none or the pattern of the other option
    When each value is judged by the declared pattern and by the parser of its option, and the sweep reads each document
    Then the pattern and the parser agree on every value, a date or time that does not exist is the pattern's and the parser's refusal with its cause, and the sweep finds each seeded fault by the option and the command

  # canon: {"schema":1,"at":"2026-11-05T15:56:25Z","actor":{"agent_id":"impl-option-forms","role":"implementer","session_id":"s-0-37-option-forms"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-37-option-forms.F6
  @atip.optionforms.13
  Scenario: The CHANGELOG has a row for every spelling 0.36.0 took and this build refuses
    Given the golden file of what 0.36.0 took, measured on the released binary, and the Breaking table of the CHANGELOG
    When each value of it is judged by the domain of the build
    Then every spelling 0.36.0 took and the build refuses has a row for its option, its command and its class, with the 0.37 answer and the way to move, the table has one row per class, the spellings that are still taken have none, and what 0.36.0 refused already is still refused

  # canon: {"schema":1,"at":"2026-11-05T15:56:31Z","actor":{"agent_id":"impl-option-forms","role":"implementer","session_id":"s-0-37-option-forms"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-37-option-forms.F6
  @atip.optionforms.14
  Scenario: The check of the CHANGELOG fails for a missing row, a missing answer and a missing note
    Given the CHANGELOG with a row taken out, a row with no way to move, the note about the signature taken out, and a refused value of a class no row names
    When the check of the CHANGELOG runs on each
    Then it names the missing row by option, command and class, the row with no way to move, the missing note and the class, and the real CHANGELOG passes

  # canon: {"schema":1,"at":"2026-11-05T15:56:36Z","actor":{"agent_id":"impl-option-forms","role":"implementer","session_id":"s-0-37-option-forms"}}
  @subject:atip-schema
  @lane:behavior
  @case:edge
  @flow:canon-0-37-option-forms.F5
  @atip.optionforms.15
  Scenario: The checker's fixture lists values the new domains refuse, and the recipes run at 1.5
    Given the checker's fixture and the recipes of the justfile
    When the values it lists as refused are judged by the declared domains
    Then each of the sixteen places of the seven options lists a value its domain refuses, and the recipes run the checker at 1.5

  # canon: {"schema":1,"at":"2026-11-05T15:56:42Z","actor":{"agent_id":"impl-option-forms","role":"implementer","session_id":"s-0-37-option-forms"}}
  @subject:atip-schema
  @lane:behavior
  @case:edge
  @flow:canon-0-37-option-forms.F6
  @atip.optionforms.16
  Scenario: The golden file is the measure of 0.36.0 and each class of it is described
    Given atip/option_forms_0_36.json
    When its classes, its accepted and its refused entries are read
    Then each class is described, each place of the seven options was measured, and the file says what the brief says 0.36.0 took: a date alone, white space, one digit, a space for the T and the minus sign of Unicode for --approval-at, and a capital, an empty item, a space and nothing for --providers
