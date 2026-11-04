Feature: atip input order: the --input document is processed in the order of spec 11.7
  # canon: {"schema":1,"at":"2026-11-02T07:07:14Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}

  # canon: {"schema":1,"at":"2026-11-02T07:07:25Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}
  @subject:atip-input
  @case:happy
  @flow:canon-0-35-input-order.F1
  @atip.inputorder.01
  Scenario: An NDJSON file's non-empty lines are all documents, blank lines are skipped, and the file is a batch
    Given a file with three JSON lines separated by blank lines, naming a leaf that takes a batch
    When the leaf runs with --input and that file
    Then every non-empty line is one document of the batch, the blank lines are skipped, and the report has one item per document

  # canon: {"schema":1,"at":"2026-11-02T07:07:30Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}
  @subject:atip-input
  @case:failure
  @flow:canon-0-35-input-order.F1
  @atip.inputorder.02
  Scenario: A # line of an NDJSON file is NOT_JSON; a value that is neither object nor array, a missing file and an oversize source are refused as before, and nothing is written
    Given NDJSON files whose first, middle or only line is a # comment, a file holding a bare number, a path that does not exist, and a file over the byte limit
    When each is passed as --input
    Then the # file is exit 2 INPUT_INVALID reason NOT_JSON with field --input, the bare number is NOT_OBJECT, the missing path is UNREADABLE, the large file is INPUT_TOO_LARGE, and the repository is byte-identical afterwards

  # canon: {"schema":1,"at":"2026-11-02T07:07:34Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}
  @subject:atip-input
  @case:failure
  @flow:canon-0-35-input-order.F2
  @atip.inputorder.03
  Scenario: A document with two wrong keys reports both in details in the document's key order, and the first is the error's field
    Given a finding close document {"roundd":1,"seq":"x"}
    When canon finding close runs with it as --input
    Then it is exit 2 INPUT_INVALID with field /roundd, and details holds two items in the document's order, /roundd UNKNOWN_KEY first and /seq INVALID_VALUE second, and the same keys in the other order give the other order

  # canon: {"schema":1,"at":"2026-11-02T07:07:38Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}
  @subject:atip-input
  @case:edge
  @flow:canon-0-35-input-order.F2
  @atip.inputorder.04
  Scenario: One problem has no details, a document with every key right is not refused, and null, an unknown key and a flag-only key are each a problem
    Given a document with one wrong key, a document with every key right, and a document with a null, an unknown key and a flag-only key
    When each runs as --input
    Then the one-problem error has no details, the right document is not refused as a document, and the three problems are three details items with reasons INVALID_VALUE, UNKNOWN_KEY and FLAG_ONLY_KEY in key order

  # canon: {"schema":1,"at":"2026-11-02T07:07:41Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}
  @subject:atip-input
  @case:failure
  @flow:canon-0-35-input-order.F3
  @atip.inputorder.05
  Scenario: A value outside an enumeration in a document is INPUT_INVALID INVALID_VALUE at the key, the same value on argv stays USAGE
    Given a finding close document {"disposition":"nope"} and the argv --disposition nope
    When each is run
    Then the document is exit 2 INPUT_INVALID reason INVALID_VALUE with field /disposition and the valid set as expected, the argv is USAGE with field --disposition

  # canon: {"schema":1,"at":"2026-11-02T07:07:45Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}
  @subject:atip-input
  @case:failure
  @flow:canon-0-35-input-order.F4
  @atip.inputorder.06
  Scenario: A command line that does not parse answers before the conflict check, with its argv code
    Given --round abc together with the document {"round":1}, and --disposition bogus with the same kind of document
    When canon finding close runs with each
    Then each is exit 2 USAGE with the argv field (--round, --disposition) and never INPUT_CONFLICT, in a batch too

  # canon: {"schema":1,"at":"2026-11-02T07:07:49Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}
  @subject:atip-input
  @case:failure
  @flow:canon-0-35-input-order.F4
  @atip.inputorder.07
  Scenario: The same argument given differently on argv and in the document is INPUT_CONFLICT at the document pointer, and argv alone or a document alone is not checked
    Given --round 1 on argv and the document {"round":2}, the same value on both sides, an argv alone and a document alone
    When canon finding close runs with each
    Then the differing pair is INPUT_CONFLICT with field /round, and the equal pair, the argv alone and the document alone are not a conflict

  # canon: {"schema":1,"at":"2026-11-02T07:07:53Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}
  @subject:atip-input
  @case:happy
  @flow:canon-0-35-input-order.F5
  @atip.inputorder.08
  Scenario: One integer written as 1, "1", -0, "-0" and 0, or +1 on argv, is the same value and never a conflict
    Given argv --round 1 with the documents {"round":"1"}, {"round":1} and the argv --round +1 with {"round":1}, argv 0 with the documents -0, "-0" and 0
    When each pair runs
    Then no pair is INPUT_CONFLICT

  # canon: {"schema":1,"at":"2026-11-02T07:07:57Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}
  @subject:atip-input
  @case:failure
  @flow:canon-0-35-input-order.F5
  @atip.inputorder.09
  Scenario: A document integer written +1, 0x1, 1e3, with a space, as 1.0 or beyond 2^53-1 is INPUT_INVALID INVALID_VALUE, and so is a number for a string option
    Given documents with round written "+1", "0x1", "1e3", " 1", 1.0, 9007199254740992 and a number for a string option
    When each runs as --input
    Then each is exit 2 INPUT_INVALID reason INVALID_VALUE at the key's pointer, and 9007199254740992 on argv is refused too

  # canon: {"schema":1,"at":"2026-11-02T07:08:00Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}
  @subject:atip-input
  @case:edge
  @flow:canon-0-35-input-order.F6
  @atip.inputorder.10
  Scenario: An environment variable never conflicts with or validates a key the document gives, and fills an option the merged arguments lack
    Given CANON_ACTOR set in the environment and a document that gives actor, another document that does not
    When scenario new runs with each
    Then the given actor is the document's and the environment neither conflicts nor is read for that key, and the other document's actor is the environment's

  # canon: {"schema":1,"at":"2026-11-03T18:11:34Z","actor":{"agent_id":"impl-shipfix","role":"implementer","session_id":"s-0-35-shipfix"}}
  @subject:atip-input
  @case:failure
  @flow:canon-0-35-input-order.F6
  @atip.inputorder.11
  Scenario: An environment value the document cannot give, an unknown CANON_ROLE, still refuses the call although the document gives every key it can
    Given CANON_ROLE set to a value that is no role and a document that gives every key scenario new accepts
    When scenario new runs with the document
    Then it is exit 2 ROLE_INVALID at the field CANON_ROLE and nothing is written, and the same document with a valid CANON_ROLE is exit 0
