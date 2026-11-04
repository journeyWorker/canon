Feature: atip canon cursor: the 1.2 binding of atip::cursor, flag-only paging options and the next page from the call's own arguments
  # canon: {"schema":1,"at":"2026-11-02T13:36:00Z","actor":{"agent_id":"impl-canon-cursor","role":"implementer","session_id":"s-0-35-canon-cursor"}}

  # canon: {"schema":1,"at":"2026-11-02T13:36:20Z","actor":{"agent_id":"impl-canon-cursor","role":"implementer","session_id":"s-0-35-canon-cursor"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-canon-cursor-1-2.F1
  @flow:canon-0-35-crate-cursor.F2
  @flow:canon-0-35-crate-cursor.F4
  @atip.canoncursor.01
  Scenario: A page's next call holds the call's explicit arguments and --cursor and nothing else, and reads the next page, on the four paged commands
    Given a repo with three subjects, a change with three findings and two divergent scenarios
    When query, finding list, divergence list and export run with --limit 1 --json, with and without an explicit --repo and --as, and the call in the first entry of meta.next is run until a page has no cursor
    Then that entry holds the explicit arguments written --name=value (--limit=1, --json, --repo and --as only when given) and --cursor=<value>, no default and no value CANON_ROLE supplied, and the pages cover every record once

  # canon: {"schema":1,"at":"2026-11-02T13:36:56Z","actor":{"agent_id":"impl-canon-cursor","role":"implementer","session_id":"s-0-35-canon-cursor"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-canon-cursor-1-2.F1
  @flow:canon-0-35-crate-cursor.F1
  @flow:canon-0-35-crate-cursor.F5
  @atip.canoncursor.02
  Scenario: A changed CANON_ROLE, an explicit --as, an explicit default added or dropped refuses the cursor with ARGUMENTS_CHANGED; --limit, --fields and --json do not
    Given a first page of each of the four commands issued with no CANON_ROLE, and one issued with CANON_ROLE=reviewer
    When the next call sets CANON_ROLE, sets it to another role, drops it, gives it as --as, adds --repo equal to its default or drops one the first call gave, and when only --limit, --fields or --json change
    Then every change of the first kind is exit 2 CURSOR_INVALID with reason ARGUMENTS_CHANGED and field --cursor, the same environment is accepted, and a change of --limit, --fields or --json only continues at the same record

  # canon: {"schema":1,"at":"2026-11-02T13:37:35Z","actor":{"agent_id":"impl-canon-cursor","role":"implementer","session_id":"s-0-35-canon-cursor"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-canon-cursor-1-2.F1
  @flow:canon-0-35-crate-cursor.F2
  @atip.canoncursor.03
  Scenario: The cursor is one ASCII word and one changed character makes it FOREIGN
    Given a first page of each of the four commands issued with --limit 1
    When the cursor is read as text, and characters of it are changed in turn
    Then the cursor is one word of letters, digits, dot, dash and underscore (no whitespace or quote, so it is an argv word as it is), and every changed cursor is exit 2 CURSOR_INVALID with reason FOREIGN; the cursor is bound, not encrypted: nothing is promised about what a reader can decode from it

  # canon: {"schema":1,"at":"2026-11-02T13:38:14Z","actor":{"agent_id":"impl-canon-cursor","role":"implementer","session_id":"s-0-35-canon-cursor"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-canon-cursor-1-2.F2
  @flow:canon-0-35-crate-cursor.F5
  @atip.canoncursor.04
  Scenario: The next call with --all in place of --limit reads the remaining records without refusal, on the four paged commands
    Given a first page of each of the four commands issued with --limit 1
    When the entry of meta.next is run with --all in place of --limit=1
    Then it is not refused, reads every remaining record, and has no cursor

  # canon: {"schema":1,"at":"2026-11-02T13:38:49Z","actor":{"agent_id":"impl-canon-cursor","role":"implementer","session_id":"s-0-35-canon-cursor"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-canon-cursor-1-2.F2
  @atip.canoncursor.05
  Scenario: --limit with --all is USAGE with --limit, the first name of the rule, as field, on the four paged commands
    Given a repo with three subjects, a change with three findings and two divergent scenarios
    When each of the four commands is given --limit 1 --all, and --all --limit 1
    Then each is exit 2 USAGE and field is --limit, the first name of the exclusion, whichever the line wrote first, and nothing is written

  # canon: {"schema":1,"at":"2026-11-02T13:39:21Z","actor":{"agent_id":"impl-canon-cursor","role":"implementer","session_id":"s-0-35-canon-cursor"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-canon-cursor-1-2.F3
  @flow:canon-0-35-crate-cursor.F3
  @atip.canoncursor.06
  Scenario: A cursor used with another filter, kind or repo is ARGUMENTS_CHANGED, another command's is FOREIGN and one whose record is gone is EXPIRED
    Given a first page of each of the four commands, a cursor of another command and a cursor whose finding was deleted
    When it is used with another filter, another --kind, another --repo, in another command, and after the deletion
    Then the filter, kind and repo changes are exit 2 ARGUMENTS_CHANGED, the other command's cursor is FOREIGN, and the one pointing at the deleted finding is EXPIRED, each with field --cursor

  # canon: {"schema":1,"at":"2026-11-02T13:39:56Z","actor":{"agent_id":"impl-canon-cursor","role":"implementer","session_id":"s-0-35-canon-cursor"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-canon-cursor-1-2.F3
  @atip.canoncursor.07
  Scenario: A refused cursor writes nothing, and a refused export writes no audit line
    Given a repo with three subjects and a first page of each of the four commands
    When each cursor is refused for other arguments, as foreign and as expired, and export is given --output
    Then the repository is byte-identical afterwards, export wrote no file and no audit line, and the same export with a good cursor does write its audit line

  # canon: {"schema":1,"at":"2026-11-02T13:40:30Z","actor":{"agent_id":"impl-canon-cursor","role":"implementer","session_id":"s-0-35-canon-cursor"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-canon-cursor-1-2.F4
  @atip.canoncursor.08
  Scenario: --fields with a path the command does not declare is the same USAGE in text mode and in JSON mode
    Given a repo with three subjects, a change with three findings and two divergent scenarios
    When query, finding list, divergence list and export run with --fields nosuch, once with --json and once without
    Then both are exit 2 USAGE with field --fields and the same expected paths, and nothing is written

  # canon: {"schema":1,"at":"2026-11-02T13:41:02Z","actor":{"agent_id":"impl-canon-cursor","role":"implementer","session_id":"s-0-35-canon-cursor"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-canon-cursor-1-2.F5
  @flow:canon-0-35-crate-1-2.F3
  @atip.canoncursor.09
  Scenario: limit, cursor, all and fields are flagOnly in the command document of the four commands and are not in their fragment schema
    Given the command documents of query, finding list, divergence list and export, and of retrieve and search
    When canon schema <command> --json is read
    Then every option of the contract's 1.2 set that the command has (limit, cursor, all, fields among them) is flagOnly: true, and no schema of the document (the fragment and the arguments schema) has a key of theirs

  # canon: {"schema":1,"at":"2026-11-02T13:41:55Z","actor":{"agent_id":"impl-canon-cursor","role":"implementer","session_id":"s-0-35-canon-cursor"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-canon-cursor-1-2.F5
  @atip.canoncursor.10
  Scenario: A document key limit, cursor, all or fields is INPUT_INVALID with reason FLAG_ONLY_KEY at the key's pointer, and nothing is written
    Given export documents {"limit":5}, {"cursor":"x"}, {"all":true}, {"fields":"id"} and a batch whose second element has a limit
    When each is given as --input
    Then each is exit 2 INPUT_INVALID with reason FLAG_ONLY_KEY and field the pointer of the key (/limit, /cursor, /all, /fields, /1/limit), and nothing is written, no audit line included

  # canon: {"schema":1,"at":"2026-11-02T13:42:44Z","actor":{"agent_id":"impl-canon-cursor","role":"implementer","session_id":"s-0-35-canon-cursor"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-canon-cursor-1-2.F5
  @flow:canon-0-35-crate-1-2.F3
  @atip.canoncursor.11
  Scenario: canon's flag-only set is the crate's 1.2 set plus repo and as, and the source holds no list of its own
    Given canon's flag-only set and the atip crate's set at 1.2
    When they are compared and the sources of canon-cli are read
    Then the set is the crate's eight options and repo and as, the schema documents mark each of them, and no source file lists the contract's options by name

  # canon: {"schema":1,"at":"2026-11-02T13:44:33Z","actor":{"agent_id":"impl-canon-cursor","role":"implementer","session_id":"s-0-35-canon-cursor"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-canon-cursor-1-2.F6
  @flow:canon-0-35-crate-cursor.F3
  @atip.canoncursor.12
  Scenario: A cursor the 0.34 binary issued is CURSOR_INVALID with reason FOREIGN
    Given a cursor string issued by the 0.34 binary for query --kind subject --limit 1
    When it is used with --cursor
    Then it is exit 2 CURSOR_INVALID with reason FOREIGN and field --cursor, and nothing is written

  # canon: {"schema":1,"at":"2026-11-02T13:45:40Z","actor":{"agent_id":"impl-canon-cursor","role":"implementer","session_id":"s-0-35-canon-cursor"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-canon-cursor-1-2.F6
  @atip.canoncursor.13
  Scenario: The CHANGELOG carries the upgrade note: 0.34 cursors are refused, the flag-only keys are FLAG_ONLY_KEY
    Given the CHANGELOG of this repository
    When its Unreleased section is read
    Then the entry for canon-0-35-canon-cursor-1-2 says the cursors of 0.34 are refused (FOREIGN), starting over, the FLAG_ONLY_KEY refusal, the --cursor=<value> form of the next page and the environment binding

  # canon: {"schema":1,"at":"2026-11-02T13:46:39Z","actor":{"agent_id":"impl-canon-cursor","role":"implementer","session_id":"s-0-35-canon-cursor"}}
  @subject:atip-contract
  @case:edge
  @flow:canon-0-35-canon-cursor-1-2.F1
  @flow:canon-0-35-crate-cursor.F1
  @flow:canon-0-35-crate-cursor.F4
  @atip.canoncursor.14
  Scenario: A call given by --input is carried as argv options: the next page holds no --input
    Given export documents given as --input inline and on stdin
    When export runs with --limit 1 --json and the entry of meta.next is run
    Then the entry holds the document's keys as --name=value options and no --input, and the next page is read

  # canon: {"schema":1,"at":"2026-11-03T04:43:46Z","actor":{"agent_id":"impl-canon-cursor","role":"implementer","session_id":"s-0-35-canon-cursor"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-canon-cursor-1-2.F4
  @atip.canoncursor.15
  Scenario: A --fields value the command does not declare or cannot read (unknown, empty, trailing comma, array on a scalar, unclosed key, a kept path) is the same USAGE in text mode as in JSON mode, on the four paged commands, and nothing is written
    Given a repo with three subjects, a change with three findings and two divergent scenarios
    When query, finding list, divergence list and export run with --fields nosuch, with an empty value, with subject_id[], with a trailing comma, with an unclosed bracketed key and with a path the command always keeps, each with --json and without
    Then every call is exit 2 USAGE with field --fields; the JSON mode lists the declared paths as expected and the text mode prints the same refusal on stderr with nothing on stdout, and the repository is byte-identical afterwards

  # canon: {"schema":1,"at":"2026-11-04T19:20:46Z","actor":{"agent_id":"impl-flowfix-036","role":"implementer","session_id":"s-0-36-flowfix-3"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-crate-cursor.F1
  @atip.canoncursor.16
  Scenario: A cursor continues the same call written in another order or spelling: options reversed, a value apart from its option or as --name=value, the page size spelled 01 or moved, and the cursor first or between the options
    Given a first page of each of the four commands issued with --limit 1, and the call the next page names
    When the cursor is given to the same call with its options in the reverse order and each value apart from its option and the page size as 01, to the call with every option as --name=value and the cursor first, and to the call with the cursor and the page size between the options
    Then each is accepted and reads the same page as the call the next page names: the binding is the call's arguments, not their order or their spelling

  # canon: {"schema":1,"at":"2026-11-04T19:20:51Z","actor":{"agent_id":"impl-flowfix-036","role":"implementer","session_id":"s-0-36-flowfix-3"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-cursor.F4
  @atip.canoncursor.17
  Scenario: A next page holds one cursor, the one it issues and not the one that was run, and a value that starts with a dash stays one word with its option
    Given each of the four commands paged with --limit 1 and followed through the call each page names, and a repository in a directory called -dash
    When the next call of every page after the first is read, and query --kind subject --limit 1 --repo=-dash runs and its next call is run as it is
    Then the next call holds exactly one --cursor, the page's own and not the one that was run, and for -dash it holds --repo=-dash as one word with no bare word that starts with a dash and runs to the second page
