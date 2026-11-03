Feature: atip crate: cursor binding by minor, JCS, argv writing
  # canon: {"schema":1,"at":"2026-11-02T10:05:27Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}

  # canon: {"schema":1,"at":"2026-11-02T10:05:28Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-crate-cursor.F1
  @atip.cratecursor.01
  Scenario: The same arguments bind the same whatever their order or spelling
    Given the same arguments in another order, as --round=1 or --round 1, and as 1 or 1.0
    When their bindings are made
    Then they are equal, a reordered repeated option is not, and the binding is the SHA-256 of the RFC 8785 text

  # canon: {"schema":1,"at":"2026-11-02T10:07:18Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-crate-cursor.F1
  @atip.cratecursor.02
  Scenario: A 1.2 binding leaves out only json, limit, all, cursor and fields, and binds defaults and the environment
    Given acme member list --status active and a 1.2 declaration
    When bindings are made with --json, --limit, --all, --cursor, --fields, a default value, a flag-only option of the tool, a false switch, a zero count and an environment value
    Then the first five change nothing, the default, the tool's flag-only option and the environment value each change the binding, and the false switch and the zero count do not

  # canon: {"schema":1,"at":"2026-11-02T10:07:36Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-crate-cursor.F1
  @atip.cratecursor.03
  Scenario: A 1.1 binding binds all and json, has no environment source, and 1.0 follows 1.1
    Given acme member list --status active and a 1.1 and a 1.0 declaration
    When bindings are made with --all, --json, --limit, --cursor, --fields, --input and an environment value added
    Then --all and --json bind, the others do not, an environment value is ignored, and 1.0 binds as 1.1 does

  # canon: {"schema":1,"at":"2026-11-02T10:08:25Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-cursor.F1
  @atip.cratecursor.04
  Scenario: An unknown minor and an integer a digest cannot hold are refused
    Given the minors 1.3, 2.0 and the empty text, and the integers 9007199254740993 and 18446744073709551615 as argument values
    When Cursors::for_declared and binding read them
    Then the minors and the integers are refused, while 9007199254740991 and the fraction 1.5 are accepted

  # canon: {"schema":1,"at":"2026-11-02T10:08:43Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-crate-cursor.F2
  @atip.cratecursor.05
  Scenario: A cursor decodes to its scope, binding and position and is plain ASCII
    Given scopes, positions and bindings with dots, spaces, quotes, newlines and non-ASCII text
    When a cursor is encoded and decoded
    Then the scope, binding and position come back equal, and the cursor is ASCII with no whitespace or quote so it needs no shell quoting

  # canon: {"schema":1,"at":"2026-11-02T10:08:58Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-cursor.F2
  @atip.cratecursor.06
  Scenario: Changing one character of a cursor, or giving any other text, is FOREIGN
    Given a valid cursor, every one-character change and removal of it, and arbitrary texts
    When each is decoded
    Then every one is FOREIGN

  # canon: {"schema":1,"at":"2026-11-02T10:09:12Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-cursor.F3
  @atip.cratecursor.07
  Scenario: Another command's cursor is FOREIGN, another argument set is ARGUMENTS_CHANGED, and the error is CURSOR_INVALID on --cursor
    Given a cursor of member list and the arguments it was issued for
    When it is decoded for member tag, then for member list with other arguments, then as issued
    Then the first is FOREIGN, the second ARGUMENTS_CHANGED, the third the position, and the error is CURSOR_INVALID exit 2 with field --cursor and the reason

  # canon: {"schema":1,"at":"2026-11-02T10:09:29Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-cursor.F3
  @atip.cratecursor.08
  Scenario: A cursor of another minor is refused and EXPIRED is the tool's to say
    Given a cursor issued by a tool that declared 1.1 and one that declared 1.2
    When each tool decodes the other's cursor, and a tool says EXPIRED
    Then each is refused as ARGUMENTS_CHANGED and the EXPIRED error carries that reason, with the three reasons registered sorted by name

  # canon: {"schema":1,"at":"2026-11-02T10:09:47Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-crate-cursor.F4
  @atip.cratecursor.09
  Scenario: The next page argv carries the explicit arguments and the cursor, not defaults, env values or --input
    Given the arguments of a call that read --input -, with a default, an environment value, a stale cursor and operands
    When next_page_argv writes the call that continues the page
    Then the argv has the explicit arguments, --cursor with the new value, --json and the operands after one --, and none of the default, the environment value or --input -, and the same call binds the same

  # canon: {"schema":1,"at":"2026-11-02T10:10:05Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-cursor.F4
  @atip.cratecursor.10
  Scenario: A value that starts with a dash is written joined and an earlier cursor is replaced
    Given an earlier --cursor in the arguments, a value -x that starts with a dash, an empty value and --json already given
    When the next page argv is written
    Then every valued option is written --name=value, one --cursor stays and --json appears once

  # canon: {"schema":1,"at":"2026-11-02T10:10:21Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-crate-cursor.F5
  @atip.cratecursor.11
  Scenario: The contract's member list example binds differently at 1.1 and 1.2
    Given acme member list --status active --limit 5 and the variants that add --all, --json, a default value and an environment value
    When a 1.1 and a 1.2 binding are made of each
    Then --all and --json change the 1.1 binding only, the environment value changes the 1.2 binding only, a default value changes both, and --fields and --limit change neither

  # canon: {"schema":1,"at":"2026-11-02T10:10:35Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-cursor.F1
  @atip.cratecursor.12
  Scenario: atip::jcs writes RFC 8785 text and refuses an integer a digest cannot hold
    Given RFC 8785's appendix B number forms, a test document with unsorted members, control characters and an emoji key, and 64-bit integers beyond 2^53-1
    When atip::jcs writes each of them
    Then the numbers are the ECMAScript form of their doubles, the document is the RFC's canonical text with members sorted by UTF-16 code units, and an integer beyond 2^53-1 is an error, never a digest

  # canon: {"schema":1,"at":"2026-11-03T05:03:44Z","actor":{"agent_id":"impl-flow-failures","role":"implementer","session_id":"s-0-35-flow-failures"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-cursor.F5
  @atip.cratecursor.13
  Scenario: What only one minor binds, added between two pages, is refused by that minor and accepted by the other
    Given acme member list --status active --limit 5 and a cursor issued for it at 1.1 and at 1.2
    When the second page adds --all, --json, an environment value or a changed default, and each cursor is decoded
    Then the minor that binds the addition answers ARGUMENTS_CHANGED, the minor that leaves it out takes the cursor on, and a changed default is refused by both
