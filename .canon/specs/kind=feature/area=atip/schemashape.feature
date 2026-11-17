Feature: atip schema shape: id operands name their target, the id options name their id, and a command of all four roles says all
  # canon: {"schema":1,"at":"2026-11-06T15:54:42Z","actor":{"agent_id":"impl-schema-shape","role":"implementer","session_id":"s-0-38-schema-shape"}}

  # canon: {"schema":1,"at":"2026-11-06T15:54:55Z","actor":{"agent_id":"impl-schema-shape","role":"implementer","session_id":"s-0-38-schema-shape"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-38-schema-shape.F1
  @atip.schemashape.01
  Scenario: The trace operand is named for what it takes: its value name is TRACE_ID in the schema and in the help
    Given a built canon and no repository
    When canon schema trace --json runs, and canon trace --help
    Then the one operand is named trace with value name TRACE_ID, the help shows <TRACE_ID>, and the document passes the contract's command schema

  # canon: {"schema":1,"at":"2026-11-06T15:55:08Z","actor":{"agent_id":"impl-schema-shape","role":"implementer","session_id":"s-0-38-schema-shape"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-38-schema-shape.F1
  @atip.schemashape.02
  Scenario: A trace operand that is none of the three shapes is refused as before and its field is TRACE_ID
    Given a repository holding two commits whose shas start alike, and the operands abc, a_1 and not a shape! (none of the three shapes), deadbeefdead, a review id and a finding key that no record says, and the shared sha prefix
    When canon trace runs with each, with --json
    Then each exits 2 with TRACE_OPERAND_INVALID, error.field is TRACE_ID and error.received is the value as given; a sha, a review id and a finding key that no record says are TRACE_NOT_FOUND with the field TRACE_ID and a sha prefix of two commits is TRACE_CONFLICT with it, and nothing is written

  # canon: {"schema":1,"at":"2026-11-06T15:55:21Z","actor":{"agent_id":"impl-schema-shape","role":"implementer","session_id":"s-0-38-schema-shape"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-38-schema-shape.F2
  @atip.schemashape.03
  Scenario: The six commands whose operand was id name it for what it is: subject, pack or trace
    Given a built canon
    When canon schema --json and canon schema <command> --json run for subject new, subject show, subject move, context-pack show, context-pack verify and trace
    Then the operand names are subject, subject, subject, pack, pack and trace, their value names SUBJECT_ID, SUBJECT_ID, SUBJECT_ID, PACK_ID, PACK_ID and TRACE_ID, and no operand of any command document is named id

  # canon: {"schema":1,"at":"2026-11-06T15:55:34Z","actor":{"agent_id":"impl-schema-shape","role":"implementer","session_id":"s-0-38-schema-shape"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-38-schema-shape.F2
  @atip.schemashape.04
  Scenario: An --input document that gives the operand in the operands array runs as before and the operand name appears in no input document
    Given a repository and the two of the six commands that take --input (subject new and subject move)
    When each runs with --input holding {"operands": ["demo"]} and its options, and with a key subject or id in place of operands
    Then the operands array is read as it was in 0.37 and each run records what the flag-line run records; a key named subject or id is an unknown key (INPUT_INVALID, reason UNKNOWN_KEY, field /subject or /id, expected holding operands) and nothing is written; the four commands that take no --input still refuse it with USAGE

  # canon: {"schema":1,"at":"2026-11-06T15:55:47Z","actor":{"agent_id":"impl-schema-shape","role":"implementer","session_id":"s-0-38-schema-shape"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-38-schema-shape.F3
  @atip.schemashape.05
  Scenario: The three options that took a bare ID name what they take: SKILL_ID, EVALUATOR_ID and TRAJECTORY_ID
    Given a built canon
    When canon schema dispatch begin, evidence audit and strategy rollback run with --json, and each help
    Then --skill has value name SKILL_ID, --evaluator EVALUATOR_ID and --contradicting-trajectory TRAJECTORY_ID, their option names, value types and domains are as in 0.37, and no option or operand of the tree has the value name ID

  # canon: {"schema":1,"at":"2026-11-06T15:56:00Z","actor":{"agent_id":"impl-schema-shape","role":"implementer","session_id":"s-0-38-schema-shape"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-38-schema-shape.F3
  @atip.schemashape.06
  Scenario: A value the three options refuse is refused with the code and the domain of 0.37
    Given a repository, a blank --evaluator, --skill-digest without --skill and --contradicting-trajectory beside a strategy no record says
    When evidence audit, dispatch begin and strategy rollback run with each
    Then each is refused with the code 0.37 gave (EVIDENCE_ARGUMENT_INVALID and DISPATCH_ARGUMENT_INVALID with the option as the field, STRATEGY_NOT_FOUND), the domain of --evaluator in the document is the one it is checked against and unchanged, and nothing is written

  # canon: {"schema":1,"at":"2026-11-06T15:56:13Z","actor":{"agent_id":"impl-schema-shape","role":"implementer","session_id":"s-0-38-schema-shape"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-38-schema-shape.F4
  @atip.schemashape.07
  Scenario: A command all four roles use carries the string all in x-canon.roles and every other command keeps the array
    Given a built canon
    When canon schema --json and canon schema <command> --json run for the thirteen commands used by all four roles and for the others
    Then x-canon.roles is the string all for completion, context, divergence list, finding list, format, gate check, query, retrieve, schema, search, skills show, status and trace in the tree and in each command document, the array of role names for every other command, and the empty array for the human-only commands

  # canon: {"schema":1,"at":"2026-11-06T15:56:26Z","actor":{"agent_id":"impl-schema-shape","role":"implementer","session_id":"s-0-38-schema-shape"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-38-schema-shape.F4
  @atip.schemashape.08
  Scenario: A role's listings are the same commands as before: schema --as, context, status and the guide command blocks read all as every role
    Given a built canon and each of the four roles
    When canon schema --as <role> --json, canon context --as <role> --json, canon status --as <role> --json run and the guide command blocks are rendered from the spec
    Then each lists the commands the role's rows of the metadata table name, the thirteen commands of all four roles among them for every role, the same commands and effects as 0.37 gave, and the blocks of the four role guides hold those commands

  # canon: {"schema":1,"at":"2026-11-06T15:56:39Z","actor":{"agent_id":"impl-schema-shape","role":"implementer","session_id":"s-0-38-schema-shape"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-38-schema-shape.F4
  @atip.schemashape.09
  Scenario: A roles value that is not all or an array of role names, or a command with no roles member, fails the check
    Given the tree of the built canon, and copies of it with a roles value of the string any, the string ALL, an object, a number, an array holding an unknown name, an array that names all four roles, an empty array on a command with agent roles, and a command entry with no roles member
    When the check of the roles of a tree runs on each
    Then the check passes the real tree and names each copy's fault, and a tree read with a missing roles member is not read as all and not read as humans only

  # canon: {"schema":1,"at":"2026-11-06T15:56:52Z","actor":{"agent_id":"impl-schema-shape","role":"implementer","session_id":"s-0-38-schema-shape"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-38-schema-shape.F4
  @atip.schemashape.10
  Scenario: The root of the tree says the two forms of roles once, and no group or command document says it again
    Given a built canon
    When canon schema --json, canon schema --as reviewer --json, canon schema <group> --json and canon schema <command> --json run
    Then the root of the tree has x-canon.rolesForm and nothing else in x-canon, a tree filtered by role has it beside x-canon.as, and no group or command document says it again
