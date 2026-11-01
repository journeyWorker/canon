Feature: atip schema: the L1 self-description documents
  # canon: {"schema":1,"at":"2026-11-01T09:32:51Z","actor":{"agent_id":"impl-schema-doc","role":"implementer","session_id":"s-0-34-schema-doc"}}

  # canon: {"schema":1,"at":"2026-11-01T09:33:08Z","actor":{"agent_id":"impl-schema-doc","role":"implementer","session_id":"s-0-34-schema-doc"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-34-atip-schema-doc.F1
  @atip.schemadoc.01
  Scenario: The schema tree is a tree document at level L1 that lists every group and command once, a group before its commands, and --as filters while CANON_ROLE does not
    Given a built canon and no environment
    When canon schema --json runs twice, then with --as reviewer, and with CANON_ROLE=reviewer
    Then data.kind is tree and level is L1; the 27 groups and 78 commands appear once each with a group before its commands; the two plain answers are byte-equal; --as keeps only the reviewer commands and their groups and the tree names the role in x-canon.as; the unfiltered trees have no x-canon; CANON_ROLE changes nothing

  # canon: {"schema":1,"at":"2026-11-01T09:33:12Z","actor":{"agent_id":"impl-schema-doc","role":"implementer","session_id":"s-0-34-schema-doc"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-34-atip-schema-doc.F1
  @atip.schemadoc.02
  Scenario: schema answers from an empty directory with no environment and writes nothing
    Given an empty directory with an empty HOME and no environment
    When canon schema --json, schema finding --json, schema finding close --json and schema --errors --json run there
    Then each answers ok with an empty stderr and the directory and HOME are still empty

  # canon: {"schema":1,"at":"2026-11-01T09:33:15Z","actor":{"agent_id":"impl-schema-doc","role":"implementer","session_id":"s-0-34-schema-doc"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-34-atip-schema-doc.F2
  @atip.schemadoc.03
  Scenario: A group document lists its direct children, and a path given in words or as one string is one answer
    Given a built canon
    When canon schema finding --json runs, and canon schema "finding close" --json and canon schema finding close --json run
    Then the group document has kind group and the three direct children; the two path forms answer the same document; every group of the tree answers with the entries the tree lists for its children

  # canon: {"schema":1,"at":"2026-11-01T09:33:18Z","actor":{"agent_id":"impl-schema-doc","role":"implementer","session_id":"s-0-34-schema-doc"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-34-atip-schema-doc.F2
  @atip.schemadoc.04
  Scenario: An unknown path is UNKNOWN_COMMAND with the valid words, overlapping selectors are USAGE and a bare group is COMMAND_REQUIRED
    Given a built canon
    When canon schema finding nosuch, schema finding clos, schema finding --errors, schema --errors --warnings and finding --json run
    Then the unknown path is exit 2 UNKNOWN_COMMAND with expected the commands of finding and close suggested for the misspelling; the overlapping selectors are USAGE; the bare group is COMMAND_REQUIRED

  # canon: {"schema":1,"at":"2026-11-01T09:33:21Z","actor":{"agent_id":"impl-schema-doc","role":"implementer","session_id":"s-0-34-schema-doc"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-34-atip-schema-doc.F3
  @atip.schemadoc.05
  Scenario: Every option has a valueType, a typed default, flagOnly and env where they apply, and --help prints the defaults the document holds
    Given the command documents of every command
    When each option and operand is read, and every [default: X] of every --help is compared with the document
    Then every option has a valueType, a typed default (round is integer, --max-artifact-mib defaults to 25, --limit to 100), flagOnly on json and input, env CANON_ACTOR on --actor, and no name ends in -id; the help defaults equal the document defaults and one option name keeps one valueName and valueType

  # canon: {"schema":1,"at":"2026-11-01T09:33:25Z","actor":{"agent_id":"impl-schema-doc","role":"implementer","session_id":"s-0-34-schema-doc"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-34-atip-schema-doc.F3
  @atip.schemadoc.06
  Scenario: An argument with a value parser of canon's own and no table entry fails the generation, and a default that does not parse as its type fails it too
    Given a clap command with an argument whose value parser is canon's own and a default that does not parse as its type
    When the projection builds its option document
    Then a parser with no table entry and a default that is not its type each fail with a message naming the argument, and a table entry makes the first pass

  # canon: {"schema":1,"at":"2026-11-01T09:33:28Z","actor":{"agent_id":"impl-schema-doc","role":"implementer","session_id":"s-0-34-schema-doc"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-34-atip-schema-doc.F4
  @atip.schemadoc.07
  Scenario: Every command document lists the codes of its features, names outputs that resolve in its schemas, and brief approve has three distinct outputs
    Given the command documents of every command
    When their errors, outputs and schemas are read and each document is checked against the contract schema and the rules of spec 10.13
    Then the input commands list INPUT_INVALID, INPUT_CONFLICT and INPUT_TOO_LARGE, the paging ones CURSOR_INVALID, the committing ones PLAN_STALE; every schemaRef resolves and compiles; brief approve has three outputs with distinct refs, one of them the batch

  # canon: {"schema":1,"at":"2026-11-01T09:33:31Z","actor":{"agent_id":"impl-schema-doc","role":"implementer","session_id":"s-0-34-schema-doc"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-34-atip-schema-doc.F4
  @atip.schemadoc.08
  Scenario: A ref that does not resolve, a schema that does not compile, a shared schemaRef and a missing reserved code are each found
    Given a command document with a dangling schemaRef, one with a schema that does not compile, one with two outputs sharing a ref and one without a reserved code
    When the rules are applied to each
    Then each is reported by name

  # canon: {"schema":1,"at":"2026-11-01T09:33:34Z","actor":{"agent_id":"impl-schema-doc","role":"implementer","session_id":"s-0-34-schema-doc"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-34-atip-schema-doc.F5
  @atip.schemadoc.09
  Scenario: The 47 input declarations hold a fragment schema without required or flag-only keys and an example that passes it, and every example parses with the real parser
    Given the 47 commands that take --input
    When the fragment, the arguments schema, the example and every example line are read
    Then no fragment has a root required or a flag-only property, additionalProperties is false, the example passes both schemas, the 30 that had required publish it as argumentsRef, and every examples line parses with the real parser, ends with --json after the path and holds no placeholder or --input

  # canon: {"schema":1,"at":"2026-11-01T09:33:38Z","actor":{"agent_id":"impl-schema-doc","role":"implementer","session_id":"s-0-34-schema-doc"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-34-atip-schema-doc.F5
  @atip.schemadoc.10
  Scenario: An example that does not parse, one with a placeholder, a fragment with required and an example the fragment refuses are each found
    Given examples that do not parse, that hold a placeholder, a fragment with required, an open fragment and an example the fragment refuses
    When the checks are applied to each
    Then each is reported, and an integer written as a number or a numeric string is accepted

  # canon: {"schema":1,"at":"2026-11-01T09:33:41Z","actor":{"agent_id":"impl-schema-doc","role":"implementer","session_id":"s-0-34-schema-doc"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-34-atip-schema-doc.F6
  @atip.schemadoc.11
  Scenario: The first line of the help of the root, every group and every command points at schema
    Given a built canon
    When the --help of the root, of finding, of finding close and of schema, and help finding run
    Then the first line of each is Agents: "canon schema --json" describes every command as JSON. and the introduction is below it

  # canon: {"schema":1,"at":"2026-11-01T09:33:44Z","actor":{"agent_id":"impl-schema-doc","role":"implementer","session_id":"s-0-34-schema-doc"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-34-atip-schema-doc.F6
  @atip.schemadoc.12
  Scenario: A command whose help lacks the pointer line is found by the same check
    Given a command tree whose commands were not given the line
    When the help of each node is read
    Then the nodes without the line are named, and none is left once the line is applied

  # canon: {"schema":1,"at":"2026-11-01T09:33:47Z","actor":{"agent_id":"impl-schema-doc","role":"implementer","session_id":"s-0-34-schema-doc"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-34-atip-schema-doc.F7
  @atip.schemadoc.13
  Scenario: The checked-in spec holds the tree and, for every command, the document canon schema answers, and no consumer reads a name the old spec had
    Given the checked-in spec file and the guides, the website pages, the evaluation scripts and the design notes
    When canon schema <command> --json runs for every command and the files are scanned for the names the old spec had
    Then the file holds the tree and, for each of the 78 commands, the document the answer carries, byte for byte; no guide, page, script or design note holds a member name the 0.33 spec had

  # canon: {"schema":1,"at":"2026-11-01T09:33:51Z","actor":{"agent_id":"impl-schema-doc","role":"implementer","session_id":"s-0-34-schema-doc"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-34-atip-schema-doc.F7
  @atip.schemadoc.14
  Scenario: A revived old name in a guide, a page or a script is found and the new names are not mistaken for it
    Given texts that revive an old name and texts that use the new names
    When the scan is applied to each
    Then every revived name is found and no new name is mistaken for one
