Feature: ATIP: the order of the stateless checks
  # canon: {"schema":1,"at":"2026-11-02T07:40:03Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}

  # canon: {"schema":1,"at":"2026-11-02T07:40:03Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-35-canon-check-order.F1
  @atip.checkorder.01
  Scenario: Two failures to parse are answered by the one earliest on the command line, in every command: two unknown options, and an unknown option beside a value that is not one of the possible values of --as
    Given a built canon and the example of every command as its passing invocation
    When each call adds two unknown options, or an unknown option and --as zzz, in both orders
    Then every call exits 2 with USAGE and the field of the one written first

  # canon: {"schema":1,"at":"2026-11-02T07:40:23Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-35-canon-check-order.F1
  @atip.checkorder.02
  Scenario: A value of the wrong type written before an unknown option is the one that answers, and canon finding close with --bogus before --seq x is answered by the unknown option
    Given a built canon
    When canon finding close add-login runs with --seq x before --bogus, with --bogus before --seq x, and with two unknown options, and canon finding nosuch runs with --round 0
    Then the first failure to parse answers each time: --seq, --bogus, --bogus, and UNKNOWN_COMMAND for the word

  # canon: {"schema":1,"at":"2026-11-02T07:40:31Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-35-canon-check-order.F2
  @atip.checkorder.03
  Scenario: A missing required argument is answered after a failure to parse and before a broken domain or a broken constraint, operands first and then options in declaration order
    Given a built canon and the example of every command with a required argument
    When the first required argument, the last, and both are left out, alone, beside an unknown option, beside a broken domain written first and last, and beside each broken constraint
    Then the missing required argument answers, the first in operands-then-options order, except that the unknown option answers before it

  # canon: {"schema":1,"at":"2026-11-02T07:41:31Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-35-canon-check-order.F2
  @atip.checkorder.04
  Scenario: A missing required argument alone is USAGE naming it, a broken domain alone is the domain's code, and the two together are the required argument's
    Given a built canon
    When canon finding add runs without --change, with --round 0, and with both, and canon finding close runs without its change
    Then they answer USAGE --change, FINDING_INVALID_ARGUMENT --round, USAGE --change, and USAGE CHANGE_ID

  # canon: {"schema":1,"at":"2026-11-02T07:41:40Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-35-canon-check-order.F3
  @atip.checkorder.05
  Scenario: A broken domain is answered before a broken constraint, and of two broken domains the first in the document's order answers whichever the command line writes first
    Given a built canon and the example of every command
    When two domains are broken, written in either order, and a domain and each constraint are broken together
    Then the domain first in the document's order answers with its own code and field, and a domain beats every constraint

  # canon: {"schema":1,"at":"2026-11-02T07:41:48Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-35-canon-check-order.F3
  @atip.checkorder.06
  Scenario: finding add with --round 0 --promote --no-promote is FINDING_INVALID_ARGUMENT naming --round, and the conflict alone is USAGE naming --promote
    Given a built canon
    When canon finding add runs with --round 0 --promote --no-promote, with the two promote flags first, with --round 1 and the conflict, and with --seq 0 written before --round 0
    Then they answer FINDING_INVALID_ARGUMENT --round, FINDING_INVALID_ARGUMENT --round, USAGE --promote, and FINDING_INVALID_ARGUMENT --round

  # canon: {"schema":1,"at":"2026-11-02T07:41:56Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-35-canon-check-order.F4
  @atip.checkorder.07
  Scenario: Of two broken constraints with different codes the first the document lists answers, with its code and its field
    Given a built canon and every pair of rules of a command that have different codes
    When a call breaks both, written in either order
    Then the first rule the document lists answers with its code and the field of its first argument at fault

  # canon: {"schema":1,"at":"2026-11-02T07:42:04Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:edge
  @flow:canon-0-35-canon-check-order.F4
  @atip.checkorder.08
  Scenario: A validator's rejected call changed to break a constraint too is answered by the constraint, and unchanged by the validator
    Given a built canon and the rejected call of every validator
    When the call is also made to break a constraint of another code
    Then the call as it stands answers with the validator's code, and the changed call with the constraint's code and field

  # canon: {"schema":1,"at":"2026-11-02T07:42:12Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-35-canon-check-order.F5
  @atip.checkorder.09
  Scenario: An unknown CANON_ROLE answers after every other stateless check, and alone it is UNKNOWN_ROLE
    Given a built canon and CANON_ROLE=bogus
    When canon status runs, and canon finding add runs with a broken domain, a missing required argument, a broken constraint, a broken validator and an unknown option, and canon brief approve runs with a relayed approval
    Then UNKNOWN_ROLE answers for the bare call and for the relayed approval, and each other fault answers before it

  # canon: {"schema":1,"at":"2026-11-02T07:42:21Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-35-canon-check-order.F5
  @atip.checkorder.10
  Scenario: An explicit --as that is no role is USAGE naming --as ahead of the later stages, and --as overrides an unknown CANON_ROLE
    Given a built canon and CANON_ROLE set to implementer, or to bogus
    When canon finding add runs with --as zzz, alone and beside a broken domain, and with --as implementer
    Then --as zzz is USAGE naming --as in every case, and --as implementer is never refused for the variable

  # canon: {"schema":1,"at":"2026-11-02T07:42:30Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-35-canon-check-order.F6
  @atip.checkorder.11
  Scenario: The field of a violated rule follows the contract's table for requires, forbids, atMostOne, exactlyOne and atLeastOne
    Given a built canon
    When calls break one rule of each kind: a requires, a forbids written in the other order, an atMostOne, an exactlyOne with two given and with none, and an atLeastOne
    Then the field is the first argument of the rule at fault in the rule's own order, not the command line's

  # canon: {"schema":1,"at":"2026-11-02T07:42:37Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:edge
  @flow:canon-0-35-canon-check-order.F6
  @atip.checkorder.12
  Scenario: Every rule of every command is answered with its code and the field of the first argument at fault
    Given a built canon and every rule of every command
    When a call breaks the rule and is run
    Then the answer is the code of the first rule the call breaks and the field its violation names

  # canon: {"schema":1,"at":"2026-11-02T07:42:41Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:edge
  @flow:canon-0-35-canon-check-order.F1
  @atip.checkorder.13
  Scenario: The order is the same through an --input document as on the command line
    Given a built canon
    When canon finding add runs as an --input document that breaks a domain and a constraint, one that omits a required key, and one that breaks a constraint
    Then each answers with the code its command line gives, and the field is the pointer of the key

  # canon: {"schema":1,"at":"2026-11-02T07:42:48Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:edge
  @flow:canon-0-35-canon-check-order.F1
  @atip.checkorder.14
  Scenario: The command tree declares no relation to clap, so the parser alone accepts a call that breaks one
    Given the command tree of canon
    When every leaf is asked for its argument groups and conflicts, and a line that breaks a relation is parsed by the relaxed tree
    Then no leaf declares any, and the parser accepts the line

  # canon: {"schema":1,"at":"2026-11-02T07:42:54Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-35-canon-check-order.F1
  @atip.checkorder.15
  Scenario: A command document lists exactly the rules the stateless stage enforces, in the order it checks them
    Given the command documents of canon schema and the rules the stateless stage lists for each command
    When they are compared command by command
    Then every document lists those rules in that order

  # canon: {"schema":1,"at":"2026-11-02T07:42:59Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:edge
  @flow:canon-0-35-canon-check-order.F5
  @atip.checkorder.16
  Scenario: A domain judges the argument object with its default and the integer bound of its type, a constraint only what was given
    Given the argument object of a command with a default, and the real and relaxed trees
    When a default is read, a limit of 0 or -3 is parsed, and a value that is no integer is parsed
    Then a constraint sees only what was given, the domain stage refuses 0 and -3, and the relaxed tree refuses the text that is no integer

  # canon: {"schema":1,"at":"2026-11-02T11:27:03Z","actor":{"agent_id":"impl-canon-misc","role":"implementer","session_id":"s-0-35-canon-misc"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-35-canon-check-order.F1
  @atip.checkorder.17
  Scenario: A leading --as that is no role answers before an unknown or missing command, and an excluding pair names the closed set at fault
    Given a built canon
    When canon runs with --as zzz before an unknown command word, before nothing, before a group, and after an unknown option, and init runs with --policy and --no-policy
    Then --as answers USAGE first, an earlier unknown option still answers first, and the excluding pair names light, standard and strict as expected
