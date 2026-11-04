Feature: atip canon claim l2: canon declares atip/1 1.4 and L2, and the CI jobs prove it
  # canon: {"schema":1,"at":"2026-11-02T21:07:05Z","actor":{"agent_id":"impl-claim-l2","role":"implementer","session_id":"s-0-35-claim-l2"}}

  # canon: {"schema":1,"at":"2026-11-02T21:07:16Z","actor":{"agent_id":"impl-claim-l2","role":"implementer","session_id":"s-0-35-claim-l2"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-35-claim-l2.F1
  @atip.claiml2.01
  Scenario: Every canon envelope, an error and the schema answer included, declares contractVersion 1.4, the schema answer has schemaVersion 1.4 and its tree says level L2
    Given a repository with canon init
    When status, an unknown command, schema, schema --errors and schema --warnings are run with --json
    Then every envelope has meta.contractVersion 1.4 and passes the contract's envelope schema, the schema answer has meta.schemaVersion 1.4, and the tree of schema has level L2

  # canon: {"schema":1,"at":"2026-11-02T21:07:24Z","actor":{"agent_id":"impl-claim-l2","role":"implementer","session_id":"s-0-35-claim-l2"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-35-claim-l2.F1
  @atip.claiml2.02
  Scenario: No consumer of the declaration (skills, website pages, eval scripts, conformance expectations, the generated spec) spells canon's contractVersion as 1.1, 1.2 or 1.3
    Given the skills, the website pages, the eval scripts, the conformance expectations and the generated spec of the repository
    When they are scanned for canon's own contractVersion
    Then none spells it 1.1, 1.2 or 1.3, and the files that name an earlier minor on purpose (the checker's tests of an earlier claim, the CHANGELOG, the ledger) are the only exceptions

  # canon: {"schema":1,"at":"2026-11-02T21:07:59Z","actor":{"agent_id":"impl-claim-l2","role":"implementer","session_id":"s-0-35-claim-l2"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-35-claim-l2.F2
  @atip.claiml2.06
  Scenario: INPUT_TOO_LARGE carries the field --input, for a source over the byte limit and for a batch over the item limit
    Given a repository for canon
    When a source over 8 MiB and a batch of 10001 documents are given as --input
    Then both are exit 2 INPUT_TOO_LARGE with the reason TOO_MANY_BYTES and TOO_MANY_ITEMS and the field --input

  # canon: {"schema":1,"at":"2026-11-02T21:08:39Z","actor":{"agent_id":"impl-claim-l2","role":"implementer","session_id":"s-0-35-claim-l2"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-35-claim-l2.F3
  @atip.claiml2.07
  Scenario: just atip-deep and the CI job atip run canon at L2 with --contract 1.4 and --deep, just atip runs it without --deep, and just ci includes atip-deep
    Given the justfile and the workflow of the repository
    When the recipes atip-deep and atip and the job atip are read
    Then the recipe atip-deep and the job run the installed checker once on the canon binary at L2 with --contract 1.4, --deep and the canon fixture, both build canon only, the recipe atip runs the same check without --deep, the jobs atip-l0, atip-l1, atip-l2 and atip-check-tests are gone because the L2 report holds every L0 and L1 row and the checker's tests are the atip repository's, and the recipe ci lists atip-deep

  # canon: {"schema":1,"at":"2026-11-02T21:09:34Z","actor":{"agent_id":"impl-claim-l2","role":"implementer","session_id":"s-0-35-claim-l2"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-35-claim-l2.F4
  @atip.claiml2.09
  Scenario: canon's fixture claims 1.4 and holds prepare, notRun, the preview of each destructive command, batchFailsAt, batchFailsFirst and allowedEffects
    Given canon's fixture
    When it is read
    Then it has contractVersion 1.4, prepare with a git repository and one empty commit, notRun naming dashboard, a preview for each of the five destructive commands, batchFailsAt and batchFailsFirst for the batch commands it lists, and allowedEffects for the commands that write the audit line, and its registry is the generated one

  # canon: {"schema":1,"at":"2026-11-02T21:10:16Z","actor":{"agent_id":"impl-claim-l2","role":"implementer","session_id":"s-0-35-claim-l2"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-35-claim-l2.F5
  @atip.claiml2.11
  Scenario: The 17.2 evidence list of the CHANGELOG names a test that exists for each obligation of the contract
    Given the evidence list of the CHANGELOG, one row per obligation of the contract 17.2 and the tests that prove it
    When the check reads the list and looks for each test in the sources of the workspace
    Then every obligation has a row, every named test exists, and the tests are part of the workspace run

  # canon: {"schema":1,"at":"2026-11-02T21:10:29Z","actor":{"agent_id":"impl-claim-l2","role":"implementer","session_id":"s-0-35-claim-l2"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-35-claim-l2.F5
  @atip.claiml2.12
  Scenario: An obligation removed from the evidence list, or a test of it that does not exist, fails the evidence check
    Given the evidence list of the CHANGELOG
    When a row is taken out of it, and when a row names a test that does not exist
    Then the check fails and names the obligation or the test

  # canon: {"schema":1,"at":"2026-11-02T21:10:48Z","actor":{"agent_id":"impl-claim-l2","role":"implementer","session_id":"s-0-35-claim-l2"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-35-claim-l2.F5
  @atip.claiml2.13
  Scenario: The 0.35.0 entry of the CHANGELOG carries every breaking change of the release and the upgrade notes
    Given the CHANGELOG section of the release and the list of breaking changes the release carries (cursor, flag-only, digest, NDJSON, +1, batch pointers, skills show, STOPPED_MIDWAY, declaration)
    When the section is read as committed
    Then it carries every entry, the upgrade notes included, and the evidence list of contract 17.2 stands in the same section

  # canon: {"schema":1,"at":"2026-11-02T21:11:00Z","actor":{"agent_id":"impl-claim-l2","role":"implementer","session_id":"s-0-35-claim-l2"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-35-claim-l2.F5
  @atip.claiml2.14
  Scenario: An entry removed from the 0.35.0 section of the CHANGELOG fails the check that reads it
    Given the CHANGELOG section of the release and the list of breaking changes the release carries (cursor, flag-only, digest, NDJSON, +1, batch pointers, skills show, STOPPED_MIDWAY, declaration)
    When the section is read with one entry taken out of it, for each entry
    Then the check names the missing entry and fails, and the section as committed has none missing

  # canon: {"schema":1,"at":"2026-11-02T21:53:58Z","actor":{"agent_id":"impl-claim-l2","role":"implementer","session_id":"s-0-35-claim-l2"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-35-claim-l2.F2
  @atip.claiml2.15
  Scenario: A batch whose elements are all unchanged is exit 0 with data holding only items, each element unchanged, so the batch report stays the contract's closed one
    Given a repo with a finding closed once
    When the same close is given again as a batch of one element and as a batch of two
    Then both exit 0, data has the member items and no other, every element has the status unchanged and carries its ordinary report

  # canon: {"schema":1,"at":"2026-11-02T21:54:09Z","actor":{"agent_id":"impl-claim-l2","role":"implementer","session_id":"s-0-35-claim-l2"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-35-claim-l2.F2
  @atip.claiml2.16
  Scenario: The preview of artifacts move on a store that is already local leads with the commit call, and the commit says committed true
    Given a repo with canon init, whose artifact store is local
    When artifacts move --tier local runs without --commit, then the call in meta.next[0] runs
    Then the preview has data.committed false and meta.next[0] holds --commit, --plan-digest=<its planDigest> and --json, and the commit exits 0 with committed true

  # canon: {"schema":1,"at":"2026-11-03T07:27:50Z","actor":{"agent_id":"impl-claim-l2","role":"implementer","session_id":"s-0-35-claim-l2-fix"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-35-claim-l2.F5
  @atip.claiml2.17
  Scenario: A known secret of the environment (a Postgres DSN password and the s3 keys) reaches no stdout, no stderr and no file of the repository, for the commands that reach the tiers
    Given a repo whose canon.yaml routes kinds to a Postgres and an s3 tier, and an environment with a marker as the DSN password and as the s3 keys
    When fourteen commands that reach the tiers run with and without --json
    Then the marker is in no stdout and no stderr, nor in any file of the repository, and the answers name the remote tiers, so the secret was in play

  # canon: {"schema":1,"at":"2026-11-03T07:28:38Z","actor":{"agent_id":"impl-claim-l2","role":"implementer","session_id":"s-0-35-claim-l2-fix"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-35-claim-l2.F5
  @atip.claiml2.18
  Scenario: No command of canon reads a terminal for input, and canon init, which used to ask, asks nothing on one
    Given the sources of every crate of the workspace, and canon init run with a terminal on stdin and stderr in a repo with a .claude directory
    When the sources are scanned for /dev/tty, a prompt library, a terminal-mode call and every read of stdin, and init runs
    Then none of the first three is found, the only read of stdin is --input - (a terminal is refused with STDIN_TTY), init prints no question, and it installs for claude and records it
