Feature: atip crate 1.2, seen from canon: the minor canon declares, the reserved codes, the flag-only set, the reasons made from other rows and the field rules
  # canon: {"schema":1,"at":"2026-11-04T19:17:36Z","actor":{"agent_id":"impl-flowfix-036","role":"implementer","session_id":"s-0-36-flowfix-3"}}

  # canon: {"schema":1,"at":"2026-11-04T19:17:37Z","actor":{"agent_id":"impl-flowfix-036","role":"implementer","session_id":"s-0-36-flowfix-3"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-crate-1-2.F1
  @atip.crate12.12
  Scenario: canon declares the minor of its answers by a constant of its own that the atip crate knows, and a success and a refusal both carry it
    Given the canon binary, the atip crate and the minor canon declares
    When Minor::parse reads the declared minor, and canon schema and canon status in a directory with no canon.yaml print their meta
    Then the crate knows the minor, contractVersion and schemaVersion of the self-description are the declared 1.4, the refusal carries it too and passes the contract's envelope schema

  # canon: {"schema":1,"at":"2026-11-04T19:17:41Z","actor":{"agent_id":"impl-flowfix-036","role":"implementer","session_id":"s-0-36-flowfix-3"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-1-2.F1
  @atip.crate12.13
  Scenario: A text that is no minor the crate knows is refused by Minor::parse and no source of canon reads the crate's CONTRACT_VERSION, so removing canon's own declaration fails its build
    Given the texts 1.9, 1.6, 2.0, 1, the empty text, 1.1.0 and v1.1, the six minors 1.0 to 1.5, and the sources of canon-cli
    When Minor::parse reads each text and the sources are scanned for the crate's CONTRACT_VERSION
    Then every text that is no minor is refused as itself and never as the nearest minor, the six minors are read back as written, and no source of canon reads the crate's constant

  # canon: {"schema":1,"at":"2026-11-04T19:17:46Z","actor":{"agent_id":"impl-flowfix-036","role":"implementer","session_id":"s-0-36-flowfix-3"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-crate-1-2.F2
  @atip.crate12.14
  Scenario: The reserved codes canon registers have the exit and retryability of the crate's table for the minor canon declares
    Given the reserved code table of the crate and canon schema --errors
    When the reserved codes of the declared minor are read from the table and each is looked up in the registry canon prints
    Then twelve codes are reserved at 1.4 and canon registers eleven of them, each at the table's exit and retryability and with no problem from registration_problem, the twelfth (OPERATION_REUSED) is the one canon has no --operation for, and the table has thirteen rows with STOPPED_MIDWAY at exit 1, not retryable, reserved in 1.2 alone, which canon does not register

  # canon: {"schema":1,"at":"2026-11-04T19:17:51Z","actor":{"agent_id":"impl-flowfix-036","role":"implementer","session_id":"s-0-36-flowfix-3"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-1-2.F2
  @atip.crate12.15
  Scenario: A reserved code registered at another exit or retryability is found by the crate's check, and STOPPED_MIDWAY is judged in a document that declares 1.2 alone
    Given canon's registry, and the same rows with the exit or the retryability of a reserved code changed or left out
    When registration_problem judges each row for the minor canon declares, and STOPPED_MIDWAY at exit 2 for 1.2, 1.1 and the declared minor
    Then the rows as canon has them are no problem, each changed row names the code and the exit the table reserves, a row with no exit or no retryability is a problem, STOPPED_MIDWAY at exit 2 is a problem in a 1.2 document and none in a 1.1 one or in canon's own, and canon schema --error STOPPED_MIDWAY is USAGE at --error

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
    Given the minors 1.0, 1.1 and 1.4, the texts 1.9, 1.6, 2.0, 1 and the empty text, and the declarations canon publishes
    When Rules::for_minor builds the rules of each, a bracketed key is declared under each, every published declaration is checked under canon's, and the sources of canon-cli are scanned for a Rules built by its old field
    Then the declared minor has the rules of 1.1 (a bracketed key is allowed) and not those of 1.0 (it is refused), each text that is no minor is refused as itself, the eight published declarations have no problem, and no source builds Rules by the v11 field
