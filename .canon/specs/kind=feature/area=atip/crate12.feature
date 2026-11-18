Feature: atip crate 1.2, seen from canon: the minor canon declares, the reserved codes, the flag-only set, the reasons made from other rows and the field rules
  # canon: {"schema":1,"at":"2026-11-04T19:17:36Z","actor":{"agent_id":"impl-flowfix-036","role":"implementer","session_id":"s-0-36-flowfix-3"}}

  # canon: {"schema":1,"at":"2026-11-04T19:17:46Z","actor":{"agent_id":"impl-flowfix-036","role":"implementer","session_id":"s-0-36-flowfix-3"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-crate-1-2.F2
  @atip.crate12.14
  Scenario: The reserved codes canon registers have the exit and retryability of the crate's table for the minor canon declares
    Given the reserved code table of the crate and canon schema --errors
    When the reserved codes of the declared minor are read from the table and each is looked up in the registry canon prints
    Then the codes reserved at the declared minor are registered by canon, each at the table's exit and retryability and with no problem from registration_problem, except OPERATION_REUSED, the one reserved code canon has no --operation for, and the table has STOPPED_MIDWAY at exit 1, not retryable, reserved in 1.2 alone, which canon does not register

  # canon: {"schema":1,"at":"2026-11-04T19:17:55Z","actor":{"agent_id":"impl-flowfix-036","role":"implementer","session_id":"s-0-36-flowfix-3"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-crate-1-2.F4
  @atip.crate12.16
  Scenario: A row made for a code whose reasons are all the exit-2 rows of canon's code table has each row's own name, meaning and fix, whatever the order of the table
    Given canon's code table without a row for the probe code, and the same table in another order
    When a row is made whose reasons come from every exit-2 row, and put into the table
    Then the reasons are the exit-2 codes sorted by name, each with the meaning and the fix of its own row, the result is the same for both orders, and the table stays sorted for find_code

  # canon: {"schema":1,"at":"2026-11-04T19:18:00Z","actor":{"agent_id":"impl-flowfix-036","role":"implementer","session_id":"s-0-36-flowfix-3"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-1-2.F4
  @atip.crate12.17
  Scenario: A reason made from a code the table does not hold, or in a table of the wrong length, stops the evaluation and is no reason
    Given canon's code table, a code it does not hold, a reasons list one row long and a probe row whose code the table already holds
    When a reason is made from each, the way the table's own constants are
    Then the code that is held makes its reason from its own row, and each of the others stops the evaluation (a constant of the table would not compile)

  # canon: {"schema":1,"at":"2026-11-04T19:18:04Z","actor":{"agent_id":"impl-flowfix-036","role":"implementer","session_id":"s-0-36-flowfix-3"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-1-2.F5
  @atip.crate12.18
  Scenario: The rules a fields declaration is held to are those of the minor canon declares, and a minor the crate does not know is refused
    Given the minors 1.0 and 1.1, the texts 1.9, 1.7, 2.0, 1 and the empty text, and the declarations canon publishes
    When Rules::for_minor builds the rules of each, a bracketed key is declared under each, every published declaration is checked under canon's, and the sources of canon-cli are scanned for a Rules built by its old field
    Then the declared minor has the rules of 1.1 (a bracketed key is allowed) and not those of 1.0 (it is refused), each text that is no minor is refused as itself, every published declaration has no problem, and no source builds Rules by the v11 field
