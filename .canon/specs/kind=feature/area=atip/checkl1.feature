Feature: atip check: the L1 checks of contract 1.1.1
  # canon: {"schema":1,"at":"2026-11-01T12:49:29Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}

  # canon: {"schema":1,"at":"2026-11-01T12:49:29Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-34-atip-check-l1.F1
  @atip.checkl1.01
  Scenario: atip check --level L1 runs the 28 L0 and the 31 L1 checks and a conforming 1.1 tool passes them: 25 MUST and 6 SHOULD apply at L1
    Given a fake tool that follows atip/1 1.1 at L1 and a fixture with the L0 keys and the L1 entries of its commands
    When atip check --bin <tool> --level L1 --contract 1.1 --fixture <file> --json runs
    Then it exits 0 with ok true, data.level L1, data.contractVersion 1.1 and data.checks listing the 28 L0 ids and then L1.tree … L1.fields in table order, 59 of them, every one pass or unsupported, and of the L1 checks 25 MUST and 6 SHOULD apply

  # canon: {"schema":1,"at":"2026-11-01T12:49:36Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-l1.F1
  @atip.checkl1.02
  Scenario: --level L2 runs and is never LEVEL_UNSUPPORTED, and a level that is none of L0, L1, L2 is USAGE
    Given a conforming fake tool and its fixture
    When atip check runs with --level L2, then with --level L9
    Then the first runs the checks: a tool that declares L1 exits 1 with LEVEL_NOT_MET, data.level L2 and L1.tree failed; the second exits 2 with USAGE

  # canon: {"schema":1,"at":"2026-11-01T12:49:39Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-l1.F1
  @atip.checkl1.03
  Scenario: A tree whose level is not the claim, and a tool that claims L1 without schema, fail L1.tree
    Given a fake tool whose schema tree says level L2, and one that refuses schema --json with USAGE
    When atip check --level L1 runs against each
    Then L1.tree is fail naming the level (or the refusal) and no other check fails for the first; the second exits 1 with LEVEL_NOT_MET and L1.tree is fail

  # canon: {"schema":1,"at":"2026-11-01T12:49:43Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-l1.F2
  @atip.checkl1.04
  Scenario: Each pass condition of L1.tree, L1.no-state, L1.walk, L1.quoted-path and L1.schema-errors fails only its owner and names the document and the rule
    Given a fake tool with one planted violation per condition: a group listed after its commands, a duplicate id, an entry whose command is not its path, a schema that needs state or writes, a document that is not valid, a group document with the wrong children, each rule 1 to 11 and 13 of spec 10.13, a quoted path that answers otherwise, an unknown path, overlapping options and an unknown --error that are not refused as the contract says
    When atip check --level L1 runs once per planted violation
    Then the owner check fails with the command id and the rule in its reason, no other check fails, and a MUST owner exits 1 with LEVEL_NOT_MET naming it

  # canon: {"schema":1,"at":"2026-11-01T12:49:46Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-l1.F2
  @atip.checkl1.05
  Scenario: Each pass condition of L1.registry, L1.reasons, L1.reasons-reserved, L1.warnings, L1.reserved-applicable, L1.codes-known and L1.groups-not-runnable fails only its owner
    Given a fake tool with a code listed twice, a reserved code with another exit, a missing required code, a command whose errors are not registered, a reason that is not UPPER_SNAKE_CASE or is listed twice or has an exit of its own or differs between --error and --errors, a reserved reason under another name, a warning code listed twice or a refused unknown warning that is not USAGE, a code in both registries with two meanings, a command that offers a feature without its reserved code, a failure code not in its command's errors, and a group that runs or whose expected is not its verbs
    When atip check --level L1 runs once per planted violation
    Then the owner check fails and no other check fails; from L1 the reasons of an entry are L1.reasons' and a group that runs is L1.groups-not-runnable's, so L0.registry and L0.global-on-group do not repeat them

  # canon: {"schema":1,"at":"2026-11-01T12:49:49Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-l1.F2
  @atip.checkl1.06
  Scenario: Each pass condition of L1.grammar, L1.depth, L1.one-meaning, L1.one-meaning-values, L1.help-line, L1.defaults and L1.deterministic fails only its owner, the SHOULD ones as advisories
    Given a fake tool with an option that is not kebab-case, one that ends in -id, a reserved option with another shape, a path of three words, one option name with two value names or types or disjoint enum values, a --help first line that does not mention schema, a --help default that differs from the document, and a document that prints different bytes the second time
    When atip check --level L1 runs once per planted violation
    Then the owner check fails (the MUST ones exit 1 with LEVEL_NOT_MET, the SHOULD ones exit 0 with the warning SHOULD_FAILED) and no other check fails; the schema's own finding about a word that L1.grammar owns is L1.grammar's

  # canon: {"schema":1,"at":"2026-11-01T12:49:53Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-l1.F2
  @atip.checkl1.07
  Scenario: Each pass condition of L1.examples, L1.example-runs and L1.hint-command fails only its owner, and a hint that names no command is an advisory
    Given a fake tool with an example that does not start with the tool name, names a path that is not the command's, does not parse, keeps a placeholder, has a value outside its domain, leaves a required argument out, breaks a constraint, fails the --input arguments schema or reads stdin, an example of a read command that is refused as USAGE or crashes, and a USAGE hint that names no command
    When atip check --level L1 runs once per planted violation
    Then the owner check fails and no other check fails; L1.hint-command is a SHOULD so the level is met

  # canon: {"schema":1,"at":"2026-11-01T12:49:57Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-34-atip-check-l1.F2
  @atip.checkl1.08
  Scenario: A tool that declares 1.0 has no warning registry check and no hint check, and a warning registry whose meaning differs from the error registry's fails L1.warnings
    Given a fake tool that declares 1.0, and one with a code in both registries with two meanings
    When atip check --level L1 runs against each
    Then the 1.0 tool passes with L1.warnings and L1.hint-command unsupported; the other fails only L1.warnings

  # canon: {"schema":1,"at":"2026-11-01T12:50:01Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-l1.F3
  @atip.checkl1.09
  Scenario: A constraint that is declared and not enforced fails only L1.constraints; a rule every violating call also breaks another rule for is entailed and passes naming what entails it
    Given a fake tool whose command declares atLeastOne, exactlyOne, atMostOne, requires and forbids rules with in, one with a declared code, one that is implied by required, and two that are equivalent, and a variant that does not enforce one rule, one that answers another code, and equivalent rules that declare different codes
    When atip check --level L1 runs with the fixture's passing invocation of the command
    Then the checker runs one invocation that breaks one rule group and nothing else; the unenforced rule, the wrong code and the disagreeing codes fail L1.constraints only; the entailed rule is pass with the rules that entail it in data.checks[].notes

  # canon: {"schema":1,"at":"2026-11-01T12:50:05Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-l1.F3
  @atip.checkl1.10
  Scenario: A domain that accepts a value one beyond its bound, refuses one inside it, or refuses without naming the argument fails only L1.domains
    Given a fake tool whose arguments have minimum, maximum, exclusive bounds, multipleOf, length limits, a pattern and a structural domain, and variants that accept minimum - 1 or maximum + 1, refuse a value inside, or name another field
    When atip check --level L1 runs, taking the pattern and structural values from the fixture's domainRejects
    Then each variant fails L1.domains only and the conforming tool passes

  # canon: {"schema":1,"at":"2026-11-01T12:50:09Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-l1.F3
  @atip.checkl1.11
  Scenario: A published pattern that is not of the restricted I-Regexp form, that the Rust regex crate refuses at its default limits, or on which the two engines differ fails only L1.patterns
    Given a fake tool that publishes the pattern ^[a-z0-9&&a-z]+$, one that publishes ^\p{L}{1,500}$ and the contract's invalid pattern documents
    When atip check --level L1 runs, and the contract's schema-domain-pattern-* examples are judged by the same functions
    Then L1.patterns fails with the pattern and the reason and no other check fails

  # canon: {"schema":1,"at":"2026-11-01T12:50:12Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-34-atip-check-l1.F3
  @atip.checkl1.12
  Scenario: A difference between the Unicode versions of the two engines passes and the report names both versions and the code point
    Given a fake tool that publishes ^\p{L}+$ and a fixture whose domainRejects holds U+11DB0
    When atip check --level L1 runs
    Then L1.patterns is pass and its notes say that U+11DB0 is Lo in the ECMAScript engine's Unicode 17.0 tables and Cn in the Rust engine's Unicode 16.0.0 tables; a difference that no code point and category explains fails

  # canon: {"schema":1,"at":"2026-11-01T12:50:16Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-l1.F3
  @atip.checkl1.13
  Scenario: An integer argument that accepts 9007199254740993 rounded, or 9007199254740992 at all, fails only L1.numbers
    Given a fake tool that rounds integers beyond the safe range and one that refuses them with exit 2
    When atip check --level L1 runs, giving 9007199254740992 and its negation to each integer argument
    Then the rounding tool fails L1.numbers only; the other passes

  # canon: {"schema":1,"at":"2026-11-01T12:50:20Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-l1.F3
  @atip.checkl1.14
  Scenario: A validator whose rejects line is not refused, is refused with another code or field, or answers a precondition code without setup fails only L1.validators
    Given a fake tool with a validator and variants that accept the rejects line, answer another code, name an argument that is not the validator's, or read its state first
    When atip check --level L1 runs the rejects lines in an empty directory, without prepare and without setup, in a fixture that has both
    Then each variant fails L1.validators only, and a tool that answers another code for a rejects line when the state a prepare step made is missing fails it too while the same tool with its validators first passes

  # canon: {"schema":1,"at":"2026-11-01T12:50:23Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-34-atip-check-l1.F3
  @atip.checkl1.15
  Scenario: A write command the fixture does not list is never run and the report names it unlisted without making any check unprovided
    Given a fake tool with a write command and a fixture that does not list it
    When atip check --level L1 runs
    Then no invocation of the write command appears in any check's observed invocations, constraints, domains and validators stay pass and each carries the command's id in unlisted, and no check is unprovided for it

  # canon: {"schema":1,"at":"2026-11-01T12:50:27Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-l1.F4
  @atip.checkl1.16
  Scenario: Each pass condition of L1.output-data and L1.next fails only its owner
    Given a fake tool whose report lacks a required field, has a wrong schemaVersion, prints no meta.output when a command has two outputs, and whose next entry names no command or is refused as USAGE
    When atip check --level L1 runs once per planted violation
    Then the owner check fails and no other check fails

  # canon: {"schema":1,"at":"2026-11-01T12:50:30Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-l1.F4
  @atip.checkl1.17
  Scenario: A cursor that is accepted with a changed filter argument, a page that repeats, or a page that returns more than the limit fails only L1.paging
    Given a fake tool that pages and variants that ignore --limit, repeat the first page, ignore a changed filter or refuse it with the wrong code or field
    When atip check --level L1 runs, calling the paging command with --limit 1, running the next entry in the same sequence and reusing the cursor with a changed argument
    Then each variant fails L1.paging only

  # canon: {"schema":1,"at":"2026-11-01T12:50:34Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-34-atip-check-l1.F4
  @atip.checkl1.18
  Scenario: Which commands page is decided by their documents, a paged fixture entry is used first, and a paging command with neither a paged entry nor an observed nextCursor is unprovided
    Given a fake tool whose command document declares --cursor and fixtures with a paged entry, without one, and without the command
    When atip check runs at L0 and at L1
    Then with the entry or an observed nextCursor L1.paging passes; with neither it is unprovided and the level is not met; at L0 a command that refuses --cursor like an unknown option does not page and one that answers CURSOR_INVALID does

  # canon: {"schema":1,"at":"2026-11-01T12:50:37Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-l1.F4
  @atip.checkl1.19
  Scenario: Each pass condition of L1.fields fails only its owner
    Given a fake tool whose --fields leaves a member that was not selected, drops a selected one, prints a meta.fields that is not the selection and the keep paths, breaks the projected schema, or accepts a path nothing declares
    When atip check --level L1 runs the fixture's selections, with a nested path, an [] path and a map key
    Then the owner check fails and no other check fails

  # canon: {"schema":1,"at":"2026-11-01T12:50:40Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-34-atip-check-l1.F4
  @atip.checkl1.20
  Scenario: A tool whose documents declare neither --fields nor --cursor has no paging or projection to check
    Given a fake tool whose documents do not declare --fields or --cursor
    When atip check --level L1 runs
    Then L1.paging and L1.fields are unsupported and the level is met

  # canon: {"schema":1,"at":"2026-11-01T12:50:44Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-34-atip-check-l1.F5
  @atip.checkl1.21
  Scenario: A fixture without the entries a MUST check needs makes it unprovided and exit 1, and a violation the run saw still fails
    Given a conforming fake tool with a fixture that lists only one command, and one with a violation the run observes
    When atip check --level L1 runs
    Then the first exits 1 with LEVEL_NOT_MET and a MUST check unprovided; the second has the observed check failing

  # canon: {"schema":1,"at":"2026-11-01T12:50:47Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-l1.F5
  @atip.checkl1.22
  Scenario: A fixture with a malformed L1 entry is FIXTURE_INVALID with the reason WRONG_SHAPE naming the member, and no binary runs
    Given fixtures whose domainRejects, paged, fields, prepare or registry entry has the wrong shape, and one that has both a from-schema registry and warnings
    When atip check --level L1 runs with each
    Then each exits 2 with FIXTURE_INVALID, reason WRONG_SHAPE and a message that names the member

  # canon: {"schema":1,"at":"2026-11-01T12:50:50Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-34-atip-check-l1.F5
  @atip.checkl1.23
  Scenario: prepare programs run before setup with the fixture environment, their effects are not judged, and a failing prepare or setup step is a fixture error with the reason PREPARE_FAILED or SETUP_FAILED and no check
    Given a fixture whose prepare writes a file, one whose prepare exits 3 or runs the binary under test, and one whose setup exits non-zero
    When atip check --level L1 runs
    Then the first passes, lists its prepare steps in data.prepare, and L0.refusal-effects does not charge the file; the others exit 2 with FIXTURE_INVALID, PREPARE_FAILED or SETUP_FAILED, the program's stderr in the message, and no data.checks

  # canon: {"schema":1,"at":"2026-11-01T12:50:54Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-34-atip-check-l1.F5
  @atip.checkl1.24
  Scenario: A fixture whose registry is from-schema takes the registry and the warning codes from the tool and compares nothing
    Given a fake tool and a fixture with registry from-schema and no warnings
    When atip check --level L1 runs, and then against a tool whose schema omits a code it emits
    Then L0.registry passes for the first and L1.registry fails for the second; a tool with no schema --errors makes the checks that need a registry unprovided

  # canon: {"schema":1,"at":"2026-11-01T12:50:57Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-34-atip-check-l1.F6
  @atip.checkl1.25
  Scenario: The ECMAScript engine and the Rust engine answer every case of the contract's regex record as recorded, and only the recorded Unicode-version difference passes
    Given the contract's scripts/regex-cases.json at tag v1.1.1
    When every case is run through regress with the u flag and through the regex crate at its default limits
    Then each answer equals the record, the form of each pattern is the recorded one, and the one Unicode-version difference is named with its code point and categories

  # canon: {"schema":1,"at":"2026-11-01T12:51:01Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-34-atip-check-l1.F6
  @atip.checkl1.26
  Scenario: The contract's schema-* examples are judged by the checks that own their rules: the valid ones pass and each invalid one is rejected by its row and no other
    Given every examples/schema-*.json and examples/invalid/schema-*.json of the contract at tag v1.1.1, copied unmodified
    When the functions an L1 run calls judge each one
    Then the valid ones are rejected by no check, and each invalid one by the checks of its row in tests/l1_examples.rs

  # canon: {"schema":1,"at":"2026-11-01T12:51:04Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-l1.F6
  @atip.checkl1.27
  Scenario: The contract's report examples that break a data rule are rejected by L1.output-data or L1.fields and no other L1 check
    Given the report-* examples with --fields and with a wrong output or version
    When the data checks judge each report with its command document
    Then a leaked member, a missing selected member, an undeclared path and a wrong version are rejected by the rows of tests/l1_examples.rs

  # canon: {"schema":1,"at":"2026-11-01T12:51:07Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-l1.F6
  @atip.checkl1.28
  Scenario: A check forced to pass makes its seeded test fail, and every condition a conforming run judges has a planted violation
    Given a conforming fake tool and the first planted violation of each L1 check
    When atip check runs with the owner forced to pass and with the planted condition switched off
    Then the owner no longer fails, and the set of conditions a conforming run judges is the set the cases plant

  # canon: {"schema":1,"at":"2026-11-01T12:51:10Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-34-atip-check-l1.F6
  @atip.checkl1.29
  Scenario: The checker run on itself at L0 with its own fixture passes
    Given the built atip and crates/atip-check/atip.fixture.json
    When atip check --bin atip --level L0 runs
    Then it exits 0 and passed is true

  # canon: {"schema":1,"at":"2026-11-01T12:51:14Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-34-atip-check-l1.F7
  @atip.checkl1.30
  Scenario: canon's fixture reads at L1, the CI job atip-l1 and just atip run canon at L1, and a canon example that does not parse turns the job red
    Given canon's fixture with prepare, setup, paged, fields and domainRejects, the workflow and the justfile
    When the fixture is parsed, the job and the recipe are read, and the planted examples violation is run
    Then the fixture parses with a git prepare and three paged commands, the job and the recipe run --level L1, and the violation fails L1.examples with exit 1

  # canon: {"schema":1,"at":"2026-11-01T12:51:18Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-34-atip-check-l1.F7
  @atip.checkl1.31
  Scenario: canon passes every applicable L1 check of the CI job and its tree's level is L1
    Given the built canon with the constraints and paging-fields changes merged, and its fixture
    When atip check --bin canon --level L1 --fixture canon.fixture.json runs in CI
    Then it exits 0 with the applicable L0 and L1 checks passing; this scenario is bound once atip-constraints and atip-paging-fields are on main

  # canon: {"schema":1,"at":"2026-11-01T12:51:23Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-34-atip-check-l1.F4
  @atip.checkl1.32
  Scenario: L0.page and L1.paging follow the paged key and the documents, and at L0 the observation of --cursor x (an L0 check of contract 1.1.1)
    Given a fake tool that pages and fixtures that list a paged invocation, list no paging command, list a paging command without a paged entry, and list a paged entry that returns no nextCursor
    When atip check runs at L0
    Then L0.page passes with a paged entry, is unsupported when no listed command pages, unprovided when a listed command pages with neither a paged entry nor an observed nextCursor, and fails when a paged entry returns none

  # canon: {"schema":1,"at":"2026-11-01T12:51:26Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-34-atip-check-l1.F2
  @atip.checkl1.33
  Scenario: From L1 a group that runs is L1.groups-not-runnable's failure alone, and USAGE stands for its registered finer replacement (L0 checks of contract 1.1.1)
    Given a fake tool whose group runs, and one that registers UNKNOWN_OPTION and never emits USAGE
    When atip check runs at L0 and at L1
    Then at L0 L0.global-on-group fails; at L1 L1.groups-not-runnable fails and L0.global-on-group is unsupported; the second tool passes every L1 check that expects USAGE

  # canon: {"schema":1,"at":"2026-11-01T16:14:13Z","actor":{"agent_id":"impl-check-l1","role":"implementer","session_id":"s-0-34-check-l1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-l1.F7
  @atip.checkl1.34
  Scenario: A canon command whose example does not parse fails only L1.examples, so the CI job that runs canon at L1 turns red on it
    Given the built canon, its fixture, and a wrapper that rewrites the documented example of status so that it names another tool
    When atip check --level L1 runs on the wrapper with the committed canon fixture
    Then the level is not met, L1.examples fails naming the example, and no other MUST check fails
