Feature: atip claim 1.4: canon declares atip/1 1.4 and writes its schema answers in the 1.4 form
  # canon: {"schema":1,"at":"2026-11-04T15:57:29Z","actor":{"agent_id":"impl-claim-1-4","role":"implementer","session_id":"s-0-36-claim-1-4"}}

  # canon: {"schema":1,"at":"2026-11-04T15:57:29Z","actor":{"agent_id":"impl-claim-1-4","role":"implementer","session_id":"s-0-36-claim-1-4"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-36-claim-1-4.F1
  @atip.claim14.01
  Scenario: Every envelope and schema answer says contractVersion 1.4, and the tree says level L2 with shared options and no path, no kind command and no member at its default
    Given canon status, an unknown command, schema, schema finding close, schema --errors, schema --warnings and schema finding close --schemas run with --json
    When each envelope is read and the tree of canon schema --json is walked
    Then each says contractVersion 1.4 and passes the contract's envelope schema, the schema answers say schemaVersion 1.4, the tree says level L2 and has options, and no entry has path or kind command and no member of the tree is at its default

  # canon: {"schema":1,"at":"2026-11-04T15:58:06Z","actor":{"agent_id":"impl-claim-1-4","role":"implementer","session_id":"s-0-36-claim-1-4"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-36-claim-1-4.F1
  @atip.claim14.02
  Scenario: canon schema --json answers the same bytes outside a repository with an empty environment, and the form check finds a 1.3-form tree
    Given canon schema --json run outside a repository with an empty environment from two different working directories, and seeded 1.3-form trees
    When the two answers are compared and the form check reads the seeded trees
    Then the answers are the same bytes, and the form check finds a tree with kind command, with path and with a member at its default

  # canon: {"schema":1,"at":"2026-11-04T15:58:21Z","actor":{"agent_id":"impl-claim-1-4","role":"implementer","session_id":"s-0-36-claim-1-4"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-36-claim-1-4.F2
  @atip.claim14.03
  Scenario: A command document carries the call surface only: no schemas, path, schemaRef, option or rejects, no implied option or code, and the five destructive commands list commit and plan-digest
    Given canon schema finding close --json and the documents of purge, artifacts move, migrate, ledger compact and tier age
    When the documents are read
    Then finding close has no schemas, path, input.schemaRef, input.option or rejects and lists no --json, --input or --fields and no USAGE, INTERNAL or INPUT_* code, and each destructive document lists commit and plan-digest by name

  # canon: {"schema":1,"at":"2026-11-04T15:58:27Z","actor":{"agent_id":"impl-claim-1-4","role":"implementer","session_id":"s-0-36-claim-1-4"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-36-claim-1-4.F2
  @atip.claim14.04
  Scenario: The form check finds a command document that lists an implied option, an implied code or a member at its default
    Given command documents seeded with an implied option, an implied code and a member at its default, one fault per kind
    When the form check reads each
    Then it finds each fault and finds nothing in any real command document

  # canon: {"schema":1,"at":"2026-11-04T15:58:31Z","actor":{"agent_id":"impl-claim-1-4","role":"implementer","session_id":"s-0-36-claim-1-4"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-36-claim-1-4.F2
  @atip.claim14.05
  Scenario: A bare run of each destructive command whose document lists --commit is a preview that writes nothing and says so
    Given a repository with records and the five destructive commands purge, artifacts move, migrate, ledger compact and tier age, whose documents list --commit
    When each runs bare with --json
    Then each is a preview with committed false or a planDigest, and the tree of the repository is identical before and after

  # canon: {"schema":1,"at":"2026-11-04T15:58:37Z","actor":{"agent_id":"impl-claim-1-4","role":"implementer","session_id":"s-0-36-claim-1-4"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-36-claim-1-4.F2
  @atip.claim14.06
  Scenario: --commit without --plan-digest and the reverse are refused with exit 2 and write nothing, and a destructive document without commit is found by the form check
    Given a repository and the destructive commands, with --commit only and with --plan-digest only, and a command document seeded with a destructive effect and no commit
    When each runs and the form check reads the seeded document
    Then each call is exit 2 and the repository tree is unchanged, and the form check finds the document that offers a destructive effect without commit

  # canon: {"schema":1,"at":"2026-11-04T15:58:43Z","actor":{"agent_id":"impl-claim-1-4","role":"implementer","session_id":"s-0-36-claim-1-4"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-36-claim-1-4.F3
  @atip.claim14.07
  Scenario: schema finding close --schemas --json answers exactly the referenced schemas, the derived Input and one rejects list per validator, and every schema compiles and every ref resolves
    Given canon schema finding close --schemas --json and the document of finding close
    When the schemas, Input and rejects are compared with the refs of the document and the options
    Then kind is schemas with exactly the schemas the refs name and those reach, Input equals the schema derived from the options (object, additionalProperties false, no required, operands and one property per non-flag-only option), every schema compiles in draft 2020-12 and every ref resolves, and rejects has one list per validator in order

  # canon: {"schema":1,"at":"2026-11-04T15:58:53Z","actor":{"agent_id":"impl-claim-1-4","role":"implementer","session_id":"s-0-36-claim-1-4"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-36-claim-1-4.F3
  @atip.claim14.08
  Scenario: --schemas without a path, with a group, or with --errors, --error, --warnings or --warning is exit 2 USAGE with field --schemas, and an unknown path is UNKNOWN_COMMAND
    Given --schemas with no path, with the group finding, with --errors, --error CODE, --warnings and --warning CODE, and an unknown path
    When each runs with --json
    Then the first six are exit 2 USAGE with field --schemas and the unknown path is UNKNOWN_COMMAND

  # canon: {"schema":1,"at":"2026-11-04T15:59:06Z","actor":{"agent_id":"impl-claim-1-4","role":"implementer","session_id":"s-0-36-claim-1-4"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-36-claim-1-4.F4
  @atip.claim14.09
  Scenario: No schema answer for any command, group or the tree, and no success, refusal or batch envelope, has whitespace outside its strings
    Given every command and group of the tree, the tree, and a success, a refusal and a batch envelope
    When schema <command> --json and schema <command> --schemas --json run, and the text of each is tokenised
    Then no space, tab, CR or LF stands outside a string in any of them

  # canon: {"schema":1,"at":"2026-11-04T15:59:15Z","actor":{"agent_id":"impl-claim-1-4","role":"implementer","session_id":"s-0-36-claim-1-4"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-36-claim-1-4.F4
  @atip.claim14.10
  Scenario: The whitespace check finds a pretty-printed document, a space after a colon, a trailing space and a newline inside the document, and accepts whitespace inside a string
    Given a pretty-printed document, a space after a colon, a trailing space, a newline inside the document, and a string that holds whitespace
    When the whitespace check reads each
    Then it finds the first four and accepts the last

  # canon: {"schema":1,"at":"2026-11-04T15:59:24Z","actor":{"agent_id":"impl-claim-1-4","role":"implementer","session_id":"s-0-36-claim-1-4"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-36-claim-1-4.F5
  @atip.claim14.11
  Scenario: The guides, website pages, eval scripts, conformance expectations, generated spec, README and ARCHITECTURE declare 1.4 and state no old-form member as current
    Given the guides, website pages in en and ko, eval scripts, conformance expectations, the generated spec, README and ARCHITECTURE
    When their text is scanned for the declaration and for members of the old form stated as current
    Then no file spells canon's declaration as 1.0 to 1.3 and none states input.schemaRef, input.option, path of a command document, schemas inside a command document, kind command of a tree entry or rejects in a command document as current

  # canon: {"schema":1,"at":"2026-11-04T15:59:33Z","actor":{"agent_id":"impl-claim-1-4","role":"implementer","session_id":"s-0-36-claim-1-4"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-36-claim-1-4.F5
  @atip.claim14.12
  Scenario: The consumer scan finds a consumer that spells the declaration 1.2, 1.3 or 1.1 and one that states input.schemaRef as current
    Given seeded consumer sentences that spell the declaration 1.2, 1.3 and 1.1 and one that states input.schemaRef as current
    When the consumer scan reads them
    Then it finds each of them

  # canon: {"schema":1,"at":"2026-11-04T15:59:40Z","actor":{"agent_id":"impl-claim-1-4","role":"implementer","session_id":"s-0-36-claim-1-4"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-36-claim-1-4.F6
  @atip.claim14.13
  Scenario: The declared minor, the fixture, the CI job atip and the recipes atip and atip-deep agree on 1.4, and the check fails when one of them says 1.3
    Given DECLARED_MINOR, the fixture's contractVersion, the --contract of the CI job atip and of the recipes atip and atip-deep, with one of them seeded as 1.3
    When the agreement check reads them
    Then all real values are 1.4 and the check fails for the seeded one

  # canon: {"schema":1,"at":"2026-11-04T15:59:46Z","actor":{"agent_id":"impl-claim-1-4","role":"implementer","session_id":"s-0-36-claim-1-4"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-36-claim-1-4.F6
  @atip.claim14.14
  Scenario: The CI job and the recipe atip-deep run atip check --level L2 --contract 1.4 --deep with the canon fixture, the recipe atip runs it without --deep, and just ci includes atip-deep
    Given the CI job atip, the recipes atip and atip-deep, the recipe ci and the canon fixture
    When their commands are read
    Then the job and atip-deep run atip check --level L2 --contract 1.4 --deep with the fixture at the tag atip-check-v0.1.4, atip runs it without --deep, ci includes atip-deep and the fixture claims 1.4

  # canon: {"schema":1,"at":"2026-11-04T15:59:52Z","actor":{"agent_id":"impl-claim-1-4","role":"implementer","session_id":"s-0-36-claim-1-4"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-36-claim-1-4.F7
  @atip.claim14.15
  Scenario: The CHANGELOG Unreleased section holds the entry canon declares atip/1 1.4 with its before and after table, the sizes measured from the v0.35.0 binary and from this build, every tree plus one command sum, and the callers note
    Given the Unreleased section of the CHANGELOG and the built binary
    When the entry is read and the sizes of schema finding close, finding list, purge, skills show, evidence add and the tree are measured, with the sum of the tree and each one command
    Then the entry has its table with the declaration, the schema answer forms, --schemas and --help --json, each stated 1.4 size equals the measure, each command document is smaller than its 0.35 one, the tree may be larger, every sum is stated as the tree plus the command and a sum that is larger has its reason (the crate's sharing rule), and a callers paragraph mentions 1.3

  # canon: {"schema":1,"at":"2026-11-04T15:59:58Z","actor":{"agent_id":"impl-claim-1-4","role":"implementer","session_id":"s-0-36-claim-1-4"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-36-claim-1-4.F7
  @atip.claim14.16
  Scenario: The entry check fails when the table, a size row or the callers paragraph is taken out
    Given seeded entries without the table, without a size row and without the callers paragraph
    When the entry check reads each
    Then it finds each omission and finds nothing in a complete entry
