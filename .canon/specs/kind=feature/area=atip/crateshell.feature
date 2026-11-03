Feature: atip crate: split_argv reads what quote writes
  # canon: {"schema":1,"at":"2026-11-02T06:41:17Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}

  # canon: {"schema":1,"at":"2026-11-02T06:41:24Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-crate-shell.F1
  @atip.crateshell.01
  Scenario: quote writes a word with a single quote as 'it'\''s' and split_argv reads it back
    Given the word it's
    When quote writes it and split_argv reads the result
    Then quote gives 'it'\''s' and split_argv gives back the one word it's

  # canon: {"schema":1,"at":"2026-11-02T06:41:27Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-crate-shell.F2
  @atip.crateshell.02
  Scenario: Every word list of up to four words over the awkward characters survives quote then split_argv
    Given every word list of length 0 to 4 over the characters a ' " \ space $ backtick newline and tab
    When each list is quoted word by word, joined with spaces and split again
    Then every list comes back equal to itself

  # canon: {"schema":1,"at":"2026-11-02T06:41:31Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:edge
  @flow:canon-0-35-crate-shell.F2
  @atip.crateshell.03
  Scenario: split_argv reads a backslash outside quotes as a POSIX shell does
    Given command strings with a backslash outside any quote
    When split_argv reads them
    Then a\ b is the one word "a b", a\\b is a\b, a backslash and a newline join the line, and a backslash at the end of the input is one backslash

  # canon: {"schema":1,"at":"2026-11-02T06:41:34Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-shell.F2
  @atip.crateshell.04
  Scenario: A backslash inside quotes stays as it was, and an unclosed quote does not panic
    Given a backslash and an unknown character inside double quotes, a backslash inside single quotes, and quotes that never close
    When split_argv reads them
    Then a backslash inside double quotes before anything but a quote or a backslash keeps both characters, a backslash inside single quotes is a literal backslash, and an unclosed quote reads to the end without a panic

  # canon: {"schema":1,"at":"2026-11-02T06:41:37Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:edge
  @flow:canon-0-35-crate-shell.F1
  @atip.crateshell.05
  Scenario: quote still writes exactly the strings canon already prints
    Given the words the quote tests of 0.34 used
    When quote writes them
    Then safe words are unchanged, every other word is single-quoted, an inner quote is written '\'' and the empty word is ''

  # canon: {"schema":1,"at":"2026-11-02T06:41:41Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-shell.F3
  @atip.crateshell.06
  Scenario: A tool that puts a single-quoted value in meta.next passes L0.next
    Given a fake tool whose meta.next carries a command built with quote from an argv with a single-quoted value
    When the checker's L0.next judges the envelope
    Then command-matches-argv holds and no other L0.next condition finds a problem

  # canon: {"schema":1,"at":"2026-11-03T05:05:26Z","actor":{"agent_id":"impl-flow-failures","role":"implementer","session_id":"s-0-35-flow-failures"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-shell.F1
  @atip.crateshell.07
  Scenario: A word that would break unquoted is never written bare, and written raw it does not split back to itself
    Given every word of up to three characters over the awkward characters, and the words that carry a shell's meaning
    When quote writes each one and split_argv reads it back, and split_argv reads the raw word
    Then only a word of safe characters stays bare, every other word is single-quoted and comes back whole, and a raw word with white space, a quote or nothing in it does not
