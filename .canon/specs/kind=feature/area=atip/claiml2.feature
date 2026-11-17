Feature: atip canon claim l2: canon declares atip/1 (1.4 in 0.36, 1.5 in 0.37, 1.6 since 0.41) and L2, and the CI jobs prove it
  # canon: {"schema":1,"at":"2026-11-02T21:07:05Z","actor":{"agent_id":"impl-claim-l2","role":"implementer","session_id":"s-0-35-claim-l2"}}

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
    Then none of the first three is found, the only read of stdin is --input - (a terminal is refused with STDIN_TTY), the source of init holds no prompt, and init finishes with success and records claude as the harness
