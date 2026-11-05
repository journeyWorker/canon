Feature: atip claim 1.5: canon declares atip/1 1.5 and writes the white space of its domain patterns as the Unicode property
  # canon: {"schema":1,"at":"2026-11-05T12:08:10Z","actor":{"agent_id":"impl-claim-1-5","role":"implementer","session_id":"s-0-37-claim-1-5"}}

  # canon: {"schema":1,"at":"2026-11-05T12:08:24Z","actor":{"agent_id":"impl-claim-1-5","role":"implementer","session_id":"s-0-37-claim-1-5"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-37-claim-1-5.F1
  @atip.claim15.01
  Scenario: Every envelope and schema answer says contractVersion 1.5, a schema answer says schemaVersion 1.5, and the tree says level L2
    Given canon status, an unknown command, a refusal, schema, schema finding close, schema finding close --schemas, schema --errors and schema --warnings run with --json, and the answer of schema for every command
    When each envelope is read and the tree of canon schema --json is read
    Then each says contractVersion 1.5 and the schema answers say schemaVersion 1.5, the tree says level L2, the generated spec says contractVersion 1.5, and schema --json is the same bytes from another directory

  # canon: {"schema":1,"at":"2026-11-05T12:08:35Z","actor":{"agent_id":"impl-claim-1-5","role":"implementer","session_id":"s-0-37-claim-1-5"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-37-claim-1-5.F1
  @atip.claim15.02
  Scenario: A declaration of 1.4 fails the declaration check, and the crate judges a property pattern by the declared minor: a problem of a 1.4 document, none of a 1.5 one
    Given the declaration check seeded with an envelope that says 1.4, a schema answer that says 1.4 and one with no declaration, and the property pattern of Annex V2 judged by the atip crate at the minors 1.4 and 1.5
    When the checks read the seeded answers and the crate judges the pattern and a document that has it
    Then the declaration check finds each seeded answer, the crate reads the pattern as no problem at the declared minor 1.5 and as a problem at 1.4, and a document that declares 1.5 has no problem while one that declares 1.4 has

  # canon: {"schema":1,"at":"2026-11-05T12:08:43Z","actor":{"agent_id":"impl-claim-1-5","role":"implementer","session_id":"s-0-37-claim-1-5"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-37-claim-1-5.F2
  @atip.claim15.03
  Scenario: No declared pattern lists white space by hand, each of the 59 declarations says it as the property, and none uses a shorthand or an alias
    Given every pattern of the tree and of every command document of canon schema, and the 59 places the 0.36.0 build declared a list of white space
    When each pattern is read for a class that lists three or more white-space members, for a shorthand and for a property that is not named
    Then no pattern lists white space by hand, none has a shorthand or an alias, the crate finds none outside the 1.5 form, the 29 places that kept their values say the property now, the 17 places of the six name and sentence options and the places that had no pattern say the one text pattern of 0.37 (`atip.optionvalues.03`), the 13 places of the time options and the provider list say the RFC 3339 and exact-name patterns of 0.37, which name no white space (`atip.optionforms.11`), and the distinct patterns that name white space are 11

  # canon: {"schema":1,"at":"2026-11-05T12:08:49Z","actor":{"agent_id":"impl-claim-1-5","role":"implementer","session_id":"s-0-37-claim-1-5"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-37-claim-1-5.F2
  @atip.claim15.04
  Scenario: Each new pattern of a place that kept its values takes and refuses the values its pattern of 0.36.0 did, over every scalar value and the near misses of the bases
    Given the 17 patterns of 0.36.0 as canon schema printed them, the new pattern of each of the 29 places that kept their values (the 17 places of the six name and sentence options are held to the text rule of `atip.optionvalues`, the 13 places of the time options and the provider list to `atip.optionforms`) and the words of its refusal, every scalar value up to U+3100 with a sample of the rest, and the near misses of the bases built from the controls and the white space and their neighbours
    When each old and new pattern decides each value, and the one-word token pattern that --relayed-via declared decides every scalar value against the white space of the standard library
    Then no value changes sides, each pattern takes some values and refuses some, the property is exactly the 25 code points of White_Space, and the refusal of each of the 29 places says what it said in 0.36.0 (where it quotes its pattern it quotes the one declared now)

  # canon: {"schema":1,"at":"2026-11-05T12:08:54Z","actor":{"agent_id":"impl-claim-1-5","role":"implementer","session_id":"s-0-37-claim-1-5"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-37-claim-1-5.F2
  @atip.claim15.05
  Scenario: The checks find a listed class, a shorthand, an alias and a value that changed sides
    Given the 17 patterns of 0.36.0, the shorthands and aliases of white space, and a class that lost one member, a pattern that gained one and a pattern that was narrowed to the separators
    When the checks read each
    Then the check for a list finds every pattern of 0.36.0 and none of the 1.5 spellings, the crate and the check find each shorthand and alias, and the value check finds the no-break space, the bell and the tab as values that changed sides

  # canon: {"schema":1,"at":"2026-11-05T12:09:01Z","actor":{"agent_id":"impl-claim-1-5","role":"implementer","session_id":"s-0-37-claim-1-5"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-37-claim-1-5.F3
  @atip.claim15.06
  Scenario: The rules of 1.4 hold at 1.5: compact documents, USAGE for --help --json, no member at its default, commit and plan-digest on the destructive commands
    Given every document of canon schema for the tree and each command, canon --help --json, canon finding close --help --json, canon --version --json and the five destructive command documents
    When each document is read for whitespace outside a string, for a member at its default and for a member of the 1.3 form, and each call is made
    Then no document has whitespace outside a string or a member at its default, path or schemas, each call is refused with USAGE at exit 2 naming contractVersion 1.5, and each destructive document lists commit and plan-digest

  # canon: {"schema":1,"at":"2026-11-05T12:09:06Z","actor":{"agent_id":"impl-claim-1-5","role":"implementer","session_id":"s-0-37-claim-1-5"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-37-claim-1-5.F3
  @atip.claim15.07
  Scenario: A formatted document and a member at its default fail the checks
    Given a pretty-printed document, a document with a space after a colon, and command documents seeded with path, schemas, input.schemaRef and members at their defaults
    When the whitespace check and the form check read them
    Then each fault is found by name, and a compact document and a document of the 1.4 form are not

  # canon: {"schema":1,"at":"2026-11-05T12:09:13Z","actor":{"agent_id":"impl-claim-1-5","role":"implementer","session_id":"s-0-37-claim-1-5"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-37-claim-1-5.F4
  @atip.claim15.08
  Scenario: A caller of the 1.4 profile reads the property as a problem and a caller that only compiles the pattern takes and refuses the same values
    Given each of the 11 distinct property patterns of canon, the crate's profile of 1.4 and of 1.5, and the pattern compiled by the Rust regex crate
    When the profile of each minor reads the pattern and the compiler takes and refuses the corpus of values
    Then the profile of 1.4 reads each as a problem that names White_Space and 1.5, the profile of 1.5 reads none, each compiles and, where it has a 0.36.0 pattern, takes and refuses the values it did, and every pattern of 0.36.0 is still read as none by the profile of 1.4

  # canon: {"schema":1,"at":"2026-11-05T12:09:18Z","actor":{"agent_id":"impl-claim-1-5","role":"implementer","session_id":"s-0-37-claim-1-5"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-37-claim-1-5.F4
  @atip.claim15.09
  Scenario: A document that declares 1.4 has its property pattern read as a problem and a shorthand is a problem at both minors
    Given a command document with a property pattern judged as a document that declares 1.4 and as one that declares 1.5, and a document with the shorthand \s
    When the crate checks each document
    Then the document that declares 1.4 has the pattern as a problem, the one that declares 1.5 has none, and the shorthand is not I-Regexp at both minors

  # canon: {"schema":1,"at":"2026-11-05T12:09:24Z","actor":{"agent_id":"impl-claim-1-5","role":"implementer","session_id":"s-0-37-claim-1-5"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-37-claim-1-5.F5
  @atip.claim15.10
  Scenario: The job and the recipes run atip check --level L2 --contract 1.5 --deep with the fixture, and the fixture and the declaration say 1.5
    Given the job atip of the workflow, the recipes atip and atip-deep of the justfile, the fixture of the checker, its source and the declared minor
    When each command line is read
    Then the job and atip-deep run atip check --level L2 --contract 1.5 --deep with the canon fixture, atip runs it without --deep, the fixture and the declared minor say 1.5, the checker is installed at atip-check-v0.2.0, and the fixture lists values a domain refuses for the L1.patterns row

  # canon: {"schema":1,"at":"2026-11-05T12:09:30Z","actor":{"agent_id":"impl-claim-1-5","role":"implementer","session_id":"s-0-37-claim-1-5"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-37-claim-1-5.F5
  @atip.claim15.11
  Scenario: Lowering the declaration, the fixture, the job or a recipe to 1.4 fails the agreement
    Given the declared minor, the fixture, the job atip and the recipes atip and atip-deep with one of them lowered to 1.4 at a time
    When the agreement check reads each
    Then the check fails for the lowered one by name and for no other, fails for a missing declaration, and the readers of the workflow and the justfile find a lowered value

  # canon: {"schema":1,"at":"2026-11-05T12:09:36Z","actor":{"agent_id":"impl-claim-1-5","role":"implementer","session_id":"s-0-37-claim-1-5"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-37-claim-1-5.F6
  @atip.claim15.12
  Scenario: The guides, website pages, eval scripts, conformance expectations, generated spec, README, ARCHITECTURE, workflow and justfile declare 1.5 and show no list of white space
    Given the tracked guides, website pages in both languages, eval scripts, conformance expectations, generated spec, README, ARCHITECTURE, workflow and justfile
    When each file is scanned for a declaration below 1.5 and for a list of white space shown by hand, and each envelope an expectation prints is read
    Then no file declares a minor below 1.5 or shows a list of white space, every envelope the expectations print says 1.5, and the two cli pages and the guide say 1.5 where they name the declaration

  # canon: {"schema":1,"at":"2026-11-05T12:09:42Z","actor":{"agent_id":"impl-claim-1-5","role":"implementer","session_id":"s-0-37-claim-1-5"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-37-claim-1-5.F6
  @atip.claim15.13
  Scenario: The consumer scan finds a declaration below 1.5 and a list of white space shown by hand
    Given lines that declare 1.2, 1.3 and 1.4 in the spellings the repository uses, lines that declare 1.5, a pattern of 0.36.0 shown in a guide and the classes of 1.5
    When the consumer scan reads each
    Then the scan finds each declaration below 1.5 by its version and a shown list of white space, and finds nothing in a declaration of 1.5 or in the classes of 1.5

  # canon: {"schema":1,"at":"2026-11-05T12:09:47Z","actor":{"agent_id":"impl-claim-1-5","role":"implementer","session_id":"s-0-37-claim-1-5"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-37-claim-1-5.F7
  @atip.claim15.14
  Scenario: The CHANGELOG Unreleased section holds the entry canon declares atip/1 1.5 with its before and after table, the length of one pattern and the callers note
    Given the CHANGELOG Unreleased section, the pattern of --session as 0.36.0 printed it and as this build prints it
    When the entry is read
    Then one entry names canon declares atip/1 1.5, has a before and after table with the declaration, the white space in a pattern and the length of the pattern of --session (93 then 63 characters, the measured ones), and a Callers paragraph that tells a 1.4 caller that checks patterns by its profile that it refuses the escape and one that only compiles that it is unchanged

  # canon: {"schema":1,"at":"2026-11-05T12:09:53Z","actor":{"agent_id":"impl-claim-1-5","role":"implementer","session_id":"s-0-37-claim-1-5"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-37-claim-1-5.F7
  @atip.claim15.15
  Scenario: The entry check fails when the table, the length row or the callers paragraph is missing
    Given the entry check seeded with an entry that lacks the table, the length row, the declaration row, the callers paragraph or the word profile, one with a wrong length and one with a longer pattern
    When the entry check reads each
    Then it finds each missing part by name, the wrong length and the longer pattern, and passes the complete entry
