Feature: atip claim 1.6: canon declares atip/1 1.6 and uses neither optional member (commaList, pathShape)
  # canon: {"schema":1,"at":"2026-11-13T18:20:00Z","actor":{"agent_id":"impl-atip-1-6","role":"planner"}}

  # canon: {"schema":1,"at":"2026-11-13T18:20:00Z","actor":{"agent_id":"impl-atip-1-6","role":"planner"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-41-atip-1-6.F1
  @atip.claim16.01
  Scenario: Every envelope and schema answer says contractVersion 1.6, a schema answer says schemaVersion 1.6, and the tree says level L2
    Given the errors and warnings registries, the schemas of finding close, a refusal, an unknown command and the answer of schema for every command, run with --json
    When each envelope is read and the tree of canon schema --json is read
    Then each says contractVersion 1.6, the schema answers say schemaVersion 1.6, the tree says level L2 and the generated spec says contractVersion 1.6

  # canon: {"schema":1,"at":"2026-11-13T18:20:00Z","actor":{"agent_id":"impl-atip-1-6","role":"planner"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-41-atip-1-6.F1
  @atip.claim16.02
  Scenario: A declaration of 1.5 fails the declaration check: an envelope, a schema answer or a constant that says 1.5 is found and named
    Given the declaration check seeded with an envelope, a schema answer and a command document that say 1.5, and one with no declaration
    When the check reads each seeded answer
    Then it finds each by the member that is wrong, names the command whose document says 1.5, and finds nothing in an answer that says 1.6

  # canon: {"schema":1,"at":"2026-11-13T18:20:00Z","actor":{"agent_id":"impl-atip-1-6","role":"planner"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-41-atip-1-6.F2
  @atip.claim16.03
  Scenario: No document has commaList or pathShape, and no generated spec or conformance expectation prints one
    Given the tree, the 78 command documents, the registry and schemas answers, the generated spec and the conformance expectations of canon at 1.6
    When each is scanned for the members commaList and pathShape
    Then none holds either member, so a 1.5 caller reads the documents as it read them and they differ from 0.40.0 in the declaration strings alone

  # canon: {"schema":1,"at":"2026-11-13T18:20:00Z","actor":{"agent_id":"impl-atip-1-6","role":"planner"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-41-atip-1-6.F2
  @atip.claim16.04
  Scenario: A document with commaList or pathShape, and an expectation that differs beyond the declaration strings, fail the check
    Given a command document with commaList on --harness, a tree with pathShape, and two expectations that differ in the declaration strings and in another member
    When the scan reads the documents and the two expectations are compared with the declaration strings replaced
    Then the scan finds each member by its place, the expectations that differ in the declaration alone are equal and the one that differs in another member is not

  # canon: {"schema":1,"at":"2026-11-13T18:20:00Z","actor":{"agent_id":"impl-atip-1-6","role":"planner"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-41-atip-1-6.F3
  @atip.claim16.05
  Scenario: A comma list for --harness or --kind is refused as a whole with USAGE, an --input list entry with a comma is INPUT_INVALID at its pointer, and one option per value passes
    Given init --harness claude,omp, search x --kind change,task, an --input document with harness claude,omp, and init with --harness claude --harness omp
    When each is run with --json
    Then the first two are USAGE with received the comma string and expected the allowed values, the input is INPUT_INVALID with field /harness/0, and the call with one option per value succeeds

  # canon: {"schema":1,"at":"2026-11-13T18:20:00Z","actor":{"agent_id":"impl-atip-1-6","role":"planner"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-41-atip-1-6.F3
  @atip.claim16.06
  Scenario: A declared commaList on an option whose parser does not split is found, and the real parser refuses the comma string
    Given the document of init with commaList seeded on --harness, the repeatable options of every command, and init --harness claude,omp
    When the scan reads the seeded document and init is run with the comma string
    Then the scan finds the declared commaList, the sweep sees the repeatable options, and the parser still refuses the comma string with USAGE

  # canon: {"schema":1,"at":"2026-11-13T18:20:00Z","actor":{"agent_id":"impl-atip-1-6","role":"planner"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-41-atip-1-6.F4
  @atip.claim16.07
  Scenario: The longest command path of canon schema is two words and the tree has no pathShape
    Given the tree of canon schema --json
    When every entry's path is counted and the tree is read for pathShape
    Then the paths have one or two words, none has more, and the tree declares no pathShape

  # canon: {"schema":1,"at":"2026-11-13T18:20:00Z","actor":{"agent_id":"impl-atip-1-6","role":"planner"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-41-atip-1-6.F4
  @atip.claim16.08
  Scenario: A tree with a three-word entry or a pathShape fails the depth check
    Given the tree seeded with an entry of three words and a tree seeded with a pathShape
    When the depth check reads each
    Then it names the three-word path and the pathShape

  # canon: {"schema":1,"at":"2026-11-13T18:20:00Z","actor":{"agent_id":"impl-atip-1-6","role":"planner"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-41-atip-1-6.F5
  @atip.claim16.09
  Scenario: Every option name has one value name and one value kind in every command document
    Given the shared options of the tree and the options of the 78 command documents
    When each name is grouped with the value names and value kinds it carries
    Then no name has two value names or two value kinds, and --kind keeps the one value name KIND in gate promote and purge

  # canon: {"schema":1,"at":"2026-11-13T18:20:00Z","actor":{"agent_id":"impl-atip-1-6","role":"planner"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-41-atip-1-6.F5
  @atip.claim16.10
  Scenario: A document set that gives one name two value names or two value kinds fails the check
    Given the documents with --kind of purge given the value name RECORD in one set and the value kind string in another
    When the check groups the names
    Then it finds the name with two value names and the name with two value kinds

  # canon: {"schema":1,"at":"2026-11-13T18:20:00Z","actor":{"agent_id":"impl-atip-1-6","role":"planner"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-41-atip-1-6.F6
  @atip.claim16.11
  Scenario: No consumer names the crate or the checker at 0.2.0, and the dependency, the install lines and the fixture say 0.3.0 and 1.6
    Given Cargo.toml, Cargo.lock, the justfile, the workflow, README, ARCHITECTURE, the install constant of the test and the fixture
    When each is read for the tags and the declaration
    Then none names a tag of 0.2.0, the install lines and the cache key name atip-check-v0.3.0, the dependency is atip-crate-v0.3.0 at its commit and the fixture declares 1.6

  # canon: {"schema":1,"at":"2026-11-13T18:20:00Z","actor":{"agent_id":"impl-atip-1-6","role":"planner"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-41-atip-1-6.F6
  @atip.claim16.12
  Scenario: An install line or a dependency at a tag of 0.2.0 is found
    Given a workflow line that installs atip-check-v0.2.0 and a manifest line that depends on atip-crate-v0.2.0
    When the stale-tag check reads the lines
    Then it names both lines by number and finds nothing in a line at 0.3.0

  # canon: {"schema":1,"at":"2026-11-13T18:20:00Z","actor":{"agent_id":"impl-atip-1-6","role":"planner"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-41-atip-1-6.F7
  @atip.claim16.13
  Scenario: The CHANGELOG entry canon declares atip/1 1.6 has the before and after table, the callers note, the reasons for the members not used and the contributors line
    Given the newest section of the CHANGELOG
    When the entry is checked for its table, the callers paragraph, the paragraph on the members not used and the contributors line
    Then the table holds the declaration, the tags, the options with commaList, pathShape, the longest path, the names with two meanings and the advice, and both paragraphs and the line say what the brief decided

  # canon: {"schema":1,"at":"2026-11-13T18:20:00Z","actor":{"agent_id":"impl-atip-1-6","role":"planner"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-41-atip-1-6.F7
  @atip.claim16.14
  Scenario: An entry without the callers note, a table row, the reasons or the contributors commit is found
    Given a complete entry with the callers paragraph removed, one with the reasons removed, one with a row removed, one with another commit and a section with no entry
    When the CHANGELOG check reads each
    Then it names the missing paragraph, row or line each time and finds nothing in the complete entry
