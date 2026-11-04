Feature: atip input 1.3: step 3, batch order, integers, next page, empty arrays and several problems of --input
  # canon: {"schema":1,"at":"2026-11-04T01:06:17Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}

  # canon: {"schema":1,"at":"2026-11-04T01:06:26Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-input
  @case:happy
  @flow:canon-0-36-input-1-3.F1
  @atip.input13.01
  Scenario: A document supplies the required options and argv gives only the operand: the call passes and a spelling of argv's own value that equals the document's merges
    Given a repository with change add-login and three open findings
    When canon finding close add-login runs with the document {"round":1,"seq":1,"disposition":"deferred","reason":"..."} as --input, and again with --round 01 beside a document that says round 1
    Then both calls pass, the finding is deferred, and an argument equal to the document's after normalisation merges

  # canon: {"schema":1,"at":"2026-11-04T01:06:41Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-input
  @case:failure
  @flow:canon-0-36-input-1-3.F1
  @atip.input13.02
  Scenario: An argument of argv that differs from the document after normalisation is INPUT_CONFLICT at the key's pointer, and nothing is written
    Given the same repository and a document with round 1
    When canon finding close runs with --round 2 and that document, and with another operand than the document's operands
    Then each is exit 2 INPUT_CONFLICT with field /round or /operands (the document pointer, spec 10.8), and the repository is byte-identical afterwards

  # canon: {"schema":1,"at":"2026-11-04T01:06:48Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-input
  @case:failure
  @flow:canon-0-36-input-1-3.F1
  @atip.input13.03
  Scenario: Argv is read alone at step 3: an unknown option or a malformed value is its usual USAGE though the document is fine, what neither gives is missing after the merge, and the merged call is checked as one call
    Given a document that is valid, a document missing --seq, and a document with round 0
    When canon finding close runs with an unknown option, with --disposition nope, and with those documents
    Then the unknown option and the bad value are USAGE at their option though the document is fine, the missing --seq is USAGE at /seq after the merge, round 0 has the code of --round 0 on argv, and nothing is written

  # canon: {"schema":1,"at":"2026-11-04T01:06:57Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-input
  @case:failure
  @flow:canon-0-36-input-1-3.F2
  @atip.input13.04
  Scenario: A batch whose last item is wrongly typed runs none of the items and answers exit 2 at /<last>/..., and a conflict in the last item is the same
    Given a batch of three finding close items whose last has round "x"
    When the batch runs, and again with a conflict in the last item against argv
    Then it is exit 2 at /2/round (INPUT_CONFLICT at /1/round for the second), with no data, and the earlier items did not run

  # canon: {"schema":1,"at":"2026-11-04T01:07:08Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-input
  @case:failure
  @flow:canon-0-36-input-1-3.F2
  @atip.input13.05
  Scenario: An item that fails when it runs is BATCH_INCOMPLETE at /<index> with exit 1, the later items are skipped, and the failed item's code is a refusal (exit 2) that left no effect of that item
    Given a batch of three finding close items whose second names a finding that does not exist
    When the batch runs
    Then it is exit 1 BATCH_INCOMPLETE at /1, the statuses are written, failed, skipped, the failed item's code is a refusal of the registry (exit 2) and has no effect, and the third item did not run

  # canon: {"schema":1,"at":"2026-11-04T01:07:21Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-input
  @case:edge
  @flow:canon-0-36-input-1-3.F2
  @atip.input13.06
  Scenario: The report has one entry per submitted item in the order submitted, with written, unchanged, failed and skipped items all listed
    Given batches of finding close items in each outcome
    When they run
    Then every submitted item has one entry in the order submitted: written, unchanged, failed and skipped

  # canon: {"schema":1,"at":"2026-11-04T01:07:30Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-input
  @case:failure
  @flow:canon-0-36-input-1-3.F3
  @atip.input13.07
  Scenario: The largest safe integer passes the integer check and 9007199254740992 and 9007199254740993 are INPUT_INVALID at the value's pointer, as numbers and as digit strings, with the value as written and the repository unchanged
    Given finding add documents whose round is 9007199254740991, 9007199254740992, 9007199254740993 and -9007199254740992, as numbers and as digit strings, and a batch item with seq 9007199254740993
    When each runs as --input
    Then the largest is not refused as out of range (the argument's own domain judges it), the others are INPUT_INVALID INVALID_VALUE at /round (/1/seq in the batch) with received the value as written, and the repository is byte-identical afterwards

  # canon: {"schema":1,"at":"2026-11-04T01:07:40Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-input
  @case:failure
  @flow:canon-0-36-input-1-3.F3
  @atip.input13.08
  Scenario: Argv refuses the same integers beyond the range with the usual USAGE at the option, and the largest safe integer is not refused as out of range
    Given finding add calls with --round 9007199254740991, 9007199254740992, 9007199254740993 and -9007199254740993
    When each runs
    Then the largest is not refused as out of range, the others are the usual USAGE at --round naming the range, and nothing is written

  # canon: {"schema":1,"at":"2026-11-04T01:07:49Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-input
  @case:happy
  @flow:canon-0-36-input-1-3.F4
  @atip.input13.09
  Scenario: The next page is the meta.next arguments as given, and changing only --limit, --all or --fields still continues the listing
    Given a listing of three findings with --limit 1
    When the next page runs with the arguments of meta.next as they are, with --limit 2, with --all and with --fields summary
    Then each continues from the same place and returns the findings after the first

  # canon: {"schema":1,"at":"2026-11-04T01:07:58Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-input
  @case:failure
  @flow:canon-0-36-input-1-3.F4
  @atip.input13.10
  Scenario: A changed filter, an explicit default, or a changed CANON_ROLE is CURSOR_INVALID ARGUMENTS_CHANGED at --cursor and writes nothing
    Given a first page of finding list and the meta.next arguments
    When the call adds --open, names another change, adds the explicit default --repo ., or runs with CANON_ROLE set, dropped or changed
    Then each is exit 2 CURSOR_INVALID ARGUMENTS_CHANGED with field --cursor, and the repository is byte-identical afterwards

  # canon: {"schema":1,"at":"2026-11-04T01:08:06Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-input
  @case:failure
  @flow:canon-0-36-input-1-3.F5
  @atip.input13.11
  Scenario: [] for a repeatable option is INPUT_INVALID INVALID_VALUE at the key's pointer for every repeatable option of every leaf, writes nothing, and the omitted key is the call as it was
    Given every leaf that takes --input and each repeatable option of it, and the call evidence add with artifact []
    When canon runs the leaf with a document whose only key is that option with the value []
    Then each is exit 2 INPUT_INVALID reason INVALID_VALUE with field the key's pointer (/artifact), the hint says none is the key left out, a batch item says /0/artifact, nothing is written, and the document without the key is not refused for it

  # canon: {"schema":1,"at":"2026-11-04T01:08:14Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-input
  @case:failure
  @flow:canon-0-36-input-1-3.F5
  @atip.input13.12
  Scenario: [] for an option that is not repeatable is refused the same way, and so is [] as the operands
    Given every leaf that takes --input and one option of it that is not repeatable, and operands []
    When canon runs the leaf with a document whose only key is that option with the value []
    Then each is exit 2 INPUT_INVALID reason INVALID_VALUE with field the key's pointer and nothing is written

  # canon: {"schema":1,"at":"2026-11-04T01:08:22Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-input
  @case:failure
  @flow:canon-0-36-input-1-3.F6
  @atip.input13.13
  Scenario: With several problems field is the first details entry and details lists them in the order the document met them, in a batch with the index in each pointer; one problem has no details
    Given finding add documents with two or three wrong keys in either order, one batch item with two wrong keys, and one with a single wrong key
    When each runs as --input
    Then field is the first error.details entry and the details list every problem in the order the document met them (with the item's index in a batch), and a single problem has no details
