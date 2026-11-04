Feature: atip: the shell words of canon's next steps
  # canon: {"schema":1,"at":"2026-11-04T19:14:58Z","actor":{"agent_id":"impl-flowfix-036","role":"implementer","session_id":"s-0-36-flowfix-3"}}

  # canon: {"schema":1,"at":"2026-11-04T19:15:32Z","actor":{"agent_id":"impl-flowfix-036","role":"implementer","session_id":"s-0-36-flowfix-3"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-crate-shell.F1
  @atip.shellquote.01
  Scenario: The command of a next step quotes each word as a POSIX shell reads it back: a word with an apostrophe is closed, escaped and reopened, and a word of safe characters stays bare
    Given repositories whose directory name holds an apostrophe, two apostrophes, a space, a double quote, a backslash, a dollar sign, a backtick, a tab, a newline or Korean letters, and one whose name is safe
    When canon query --kind subject --limit 1 --repo <that directory> --json runs and the command of meta.next[0] is read
    Then --repo=<name> is one word that is quoted as in a POSIX shell (an apostrophe is written '\'' inside single quotes, a safe word has no quote at all), every other word of the command is bare, and the command is canon followed by exactly those words

  # canon: {"schema":1,"at":"2026-11-04T19:15:37Z","actor":{"agent_id":"impl-flowfix-036","role":"implementer","session_id":"s-0-36-flowfix-3"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-crate-shell.F2
  @flow:canon-0-35-crate-shell.F3
  @atip.shellquote.02
  Scenario: The command of a next step is the call again: it splits into canon and the argv, and a shell that runs it reads the same page, whatever the value holds
    Given the same repositories, the first page of the listing and the steps of canon status
    When the command of meta.next[0] is split into words and run by sh -c, and the command of every step of canon status is split into words
    Then the words are canon followed by the argv, sh reads the page the argv reads when canon is run on it directly, no step of canon status compares unequal to its argv, and the word 'it'\''s' splits to it's and not to it\s

  # canon: {"schema":1,"at":"2026-11-04T19:15:41Z","actor":{"agent_id":"impl-flowfix-036","role":"implementer","session_id":"s-0-36-flowfix-3"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-shell.F1
  @flow:canon-0-35-crate-shell.F2
  @flow:canon-0-35-crate-shell.F3
  @atip.shellquote.03
  Scenario: A command with its apostrophe left bare, a word left unquoted or a closing quote dropped no longer splits into the argv and is told apart from canon's own, and a quote that never closes does not panic the split
    Given the commands of the first page of each of those repositories and the argv of each
    When the word that holds a character the shell reads is written bare, an apostrophe is written without its escape, or the closing quote of the word is dropped, and each command is split
    Then canon's own command splits into canon and the argv for every repository, every damaged command splits into words that are not canon and the argv, and splitting a command whose quote never closes returns words and does not panic
