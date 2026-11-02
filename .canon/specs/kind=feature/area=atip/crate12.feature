Feature: atip crate: the 1.2 constants, reserved codes, flag-only sets and minor-keyed rules
  # canon: {"schema":1,"at":"2026-11-02T07:08:02Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}

  # canon: {"schema":1,"at":"2026-11-02T07:08:02Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-crate-1-2.F1
  @atip.crate12.01
  Scenario: The crate knows the minors 1.0, 1.1 and 1.2 and exposes 1.2 as its newest
    Given the atip crate
    When Minor::parse reads 1.0, 1.1 and 1.2 and CONTRACT_VERSION is read
    Then the three minors are V1_0, V1_1 and V1_2, V1_2.at_least(V1_1) holds and CONTRACT_VERSION is 1.2, the newest minor the crate implements

  # canon: {"schema":1,"at":"2026-11-02T07:08:09Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-crate-1-2.F1
  @atip.crate12.02
  Scenario: canon status declares contractVersion 1.1 and atip check its own minor, apart from the crate's 1.2
    Given the canon binary and the atip checker, which keep their own declarations (the checker's moved to 1.2 in canon-0-35-check-1-2)
    When canon status --json runs and atip check prints an envelope
    Then meta.contractVersion is 1.1 for canon and 1.2 for the checker, each its own constant, and atip::CONTRACT_VERSION is read by neither

  # canon: {"schema":1,"at":"2026-11-02T07:08:13Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-1-2.F1
  @atip.crate12.03
  Scenario: A text that is not a known minor is refused, and neither tool reads the crate's CONTRACT_VERSION
    Given the texts 1.3, 1.9, 2.0, 1 and the empty text, and the sources of canon-cli and atip-check
    When Minor::parse reads each text and the sources are scanned for the crate's CONTRACT_VERSION
    Then every text is refused and no source of either tool reads the crate's constant, so removing a tool's own declaration fails its build

  # canon: {"schema":1,"at":"2026-11-02T07:08:17Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-crate-1-2.F2
  @atip.crate12.04
  Scenario: The reserved code table has thirteen rows and the checker keeps no copy of it
    Given the reserved codes table of the atip crate and the sources of atip-check
    When the table is read and the checker's sources are scanned
    Then the table has thirteen rows with STOPPED_MIDWAY at exit 1, not retryable, since 1.2, and the checker holds no table of its own

  # canon: {"schema":1,"at":"2026-11-02T07:08:21Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-1-2.F2
  @atip.crate12.05
  Scenario: STOPPED_MIDWAY registered with exit 2 fails in a 1.2 document and not in a 1.1 one
    Given a registry that lists STOPPED_MIDWAY with exit 2
    When registration_problem and the checker's L1.registry judge it for a 1.2 document and for a 1.1 document
    Then the 1.2 document is a problem naming the reserved exit 1 and the 1.1 document is none, because in 1.1 the code is the tool's own

  # canon: {"schema":1,"at":"2026-11-02T07:08:25Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-crate-1-2.F3
  @atip.crate12.06
  Scenario: The flag-only set is four options at 1.1 and eight at 1.2, and option_doc marks limit flagOnly
    Given the minors 1.1 and 1.2 and an option the tool marks flag-only beside the crate's set
    When flag_only_options is read and option_doc builds the option limit with the 1.2 set
    Then the set is json, input, commit and plan-digest at 1.1 and those with limit, cursor, all and fields at 1.2, and the limit option is flagOnly true

  # canon: {"schema":1,"at":"2026-11-02T07:08:29Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-1-2.F3
  @atip.crate12.07
  Scenario: A flag-only option a command document does not mark flagOnly is a problem
    Given a command's options in which limit is not marked flagOnly
    When flag_only_problems judges them for 1.2 and for 1.1
    Then 1.2 names the option and 1.1 finds nothing

  # canon: {"schema":1,"at":"2026-11-02T07:08:33Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-crate-1-2.F4
  @atip.crate12.08
  Scenario: A STOPPED_MIDWAY row built from canon's code table has every exit-2 row as a reason, whatever their order
    Given canon's code table without a STOPPED_MIDWAY row and the same table in another order
    When a STOPPED_MIDWAY row is built whose reasons are made from every exit-2 row, and put into the table
    Then the reasons are the exit-2 codes of the table sorted by name, the result is the same for both orders, and the table stays sorted for find_code

  # canon: {"schema":1,"at":"2026-11-02T07:08:39Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-1-2.F4
  @atip.crate12.09
  Scenario: A reason made from a code the table does not hold stops the evaluation
    Given a code the table does not hold
    When a reason is made from it
    Then the evaluation stops: a constant does not compile and a call panics

  # canon: {"schema":1,"at":"2026-11-02T07:08:45Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-crate-1-2.F5
  @atip.crate12.10
  Scenario: Rules follow the declared minor: 1.0 differs, 1.1 and 1.2 agree
    Given the minors 1.0, 1.1 and 1.2
    When Rules::for_minor builds the rules of each and the declarations of a map are checked
    Then 1.0 keeps a map whole and refuses a bracketed key, and 1.1 and 1.2 are the same rules

  # canon: {"schema":1,"at":"2026-11-02T07:08:54Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-1-2.F5
  @atip.crate12.11
  Scenario: Rules::for_minor refuses 1.9 and canon builds no Rules by its old field
    Given the text 1.9 and the sources of canon-cli
    When Rules::for_minor reads it and the sources are scanned for a Rules built with its old field
    Then 1.9 is refused and no source builds Rules by the v11 field
