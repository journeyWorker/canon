Feature: atip canon plan jcs: the digest of a destructive plan is sha256: and the JCS of effects and inputs, a commit holds the repository write lock
  # canon: {"schema":1,"at":"2026-11-02T14:58:12Z","actor":{"agent_id":"impl-plan-jcs","role":"implementer","session_id":"s-0-35-plan-jcs"}}

  # canon: {"schema":1,"at":"2026-11-02T14:58:25Z","actor":{"agent_id":"impl-plan-jcs","role":"implementer","session_id":"s-0-35-plan-jcs"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-35-plan-jcs.F1
  @atip.planjcs.01
  Scenario: The preview of each of the six destructive commands gives a planDigest of sha256: and 64 lower-case hex digits, the same plan the same digest, and the call in meta.next[0] commits it with committed true
    Given a repo set up for each of purge, artifacts move, migrate, migrate --revert, ledger compact and tier age, each with something to do
    When each is run bare, twice, and then the call in the first entry of meta.next is run
    Then both previews give one planDigest of sha256: and 64 lower-case hex digits and committed false, the call carries --commit, --plan-digest=<that digest> and --json, and its run exits 0 with committed true and the effects made

  # canon: {"schema":1,"at":"2026-11-02T14:59:00Z","actor":{"agent_id":"impl-plan-jcs","role":"implementer","session_id":"s-0-35-plan-jcs"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-35-plan-jcs.F1
  @atip.planjcs.02
  Scenario: A digest without the sha256: prefix and a digest that differs in one character are PLAN_STALE, exit 2, with nothing changed and the preview as meta.next[0], on the six commands
    Given the preview of each of the six commands and its digest
    When the commit is run with the digest without its sha256: prefix, and with the digest changed in one character
    Then both are exit 2 PLAN_STALE with field --plan-digest, the tree of the repository is the same, and meta.next[0] is the preview call without --commit and --plan-digest

  # canon: {"schema":1,"at":"2026-11-02T14:59:30Z","actor":{"agent_id":"impl-plan-jcs","role":"implementer","session_id":"s-0-35-plan-jcs"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-35-plan-jcs.F1
  @atip.planjcs.03
  Scenario: The digest is sha256: and the SHA-256 of the RFC 8785 canonical JSON of {effects, inputs}, with the normalised arguments and the digests of what the plan read as inputs
    Given a purge preview over an empty ledger of one kind, whose arguments and canon.yaml are known
    When the digest is computed by hand as sha256: and the SHA-256 hex of the RFC 8785 text of {effects, inputs: {arguments, reads}}
    Then it is the planDigest the preview prints: the arguments are the explicit ones by option name without --repo, --json, --commit and --plan-digest, and the reads are the digests of canon.yaml and of the records seen

  # canon: {"schema":1,"at":"2026-11-02T15:00:13Z","actor":{"agent_id":"impl-plan-jcs","role":"implementer","session_id":"s-0-35-plan-jcs"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-35-plan-jcs.F2
  @atip.planjcs.04
  Scenario: The same arguments and the same inputs commit, one argument changed changes the digest, and a digest made for other arguments is PLAN_STALE for another call
    Given the preview of a purge and of a migrate
    When the commit is run with the same arguments, then with one argument changed, then with the digest of the first call given to the second
    Then the same arguments and inputs commit with committed true, the changed argument gives another digest, and the digest of other arguments is exit 2 PLAN_STALE with nothing changed; the options that only record a relay do not change the digest of migrate

  # canon: {"schema":1,"at":"2026-11-02T15:00:53Z","actor":{"agent_id":"impl-plan-jcs","role":"implementer","session_id":"s-0-35-plan-jcs"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-35-plan-jcs.F2
  @atip.planjcs.05
  Scenario: A file or record the plan read, changed after the preview, makes the same commit PLAN_STALE on the five commands and nothing is written
    Given the preview of each of purge, artifacts move, migrate, ledger compact and tier age
    When after the preview a file or record the plan read is changed without changing its effects (a record that stays, canon.yaml, a ledger file, a spec file to be moved) and the same commit is run
    Then each is exit 2 PLAN_STALE, nothing is written, and meta.next[0] is the preview

  # canon: {"schema":1,"at":"2026-11-02T15:01:59Z","actor":{"agent_id":"impl-plan-jcs","role":"implementer","session_id":"s-0-35-plan-jcs"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-35-plan-jcs.F2
  @atip.planjcs.06
  Scenario: A commit holds the repository write lock for its whole run: another canon writer waits for it and writes after it ends, and a preview holds nothing and does not wait
    Given a repo whose repository write lock is held by one process for the length of a commit (a commit run with the test hold after its run)
    When another canon process runs a writer (finding add) and then a preview of the same destructive command
    Then the writer waits until the commit has ended and then writes, the preview does not wait and writes nothing, and the commit's own effects are all made before the writer's record

  # canon: {"schema":1,"at":"2026-11-02T15:03:42Z","actor":{"agent_id":"impl-plan-jcs","role":"implementer","session_id":"s-0-35-plan-jcs"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-35-plan-jcs.F2
  @atip.planjcs.07
  Scenario: A commit that waits for the lock past its time starts nothing and answers REPO_WRITE_BUSY, exit 2, retryable; a killed holder releases the lock and the lock file stays
    Given a repo whose lock is held by another process and a commit whose wait is a few hundred milliseconds
    When the commit is run, and then a holder is killed with SIGKILL and the commit is run again
    Then the first is exit 2 REPO_WRITE_BUSY, retryable, with a hint to run it again and the tree of the repository the same, and after the kill the commit proceeds and the lock file is still there

  # canon: {"schema":1,"at":"2026-11-02T15:04:29Z","actor":{"agent_id":"impl-plan-jcs","role":"implementer","session_id":"s-0-35-plan-jcs"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-35-plan-jcs.F2
  @atip.planjcs.08
  Scenario: Every leaf that writes is judged by write_lock::POLICY, the holders hold from the start of the run, a polling run holds it for each pass, and the readers, dry runs and previews do not
    Given the table of the leaves of canon schema and the policy of the repository write lock
    When each leaf whose effect is not read is looked up, with and without --dry-run, --commit and its writing option
    Then every one has a row with its reason, a leaf not in it fails the test, a run holds the lock only when the row says so for the options given, and `ingest records --watch` does not block a writer between its passes

  # canon: {"schema":1,"at":"2026-11-02T15:04:56Z","actor":{"agent_id":"impl-plan-jcs","role":"implementer","session_id":"s-0-35-plan-jcs"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-35-plan-jcs.F3
  @atip.planjcs.09
  Scenario: The purge preview lists the audit line as an append effect with its path, the commit writes that file and that line, and a purge with nothing to delete has neither
    Given a repo with session records before a cutoff and one after
    When purge is previewed and committed, and again with nothing before the cutoff
    Then the preview's effects end with an append effect naming the audit file by path and the line's operation, kind, scope and counts, the commit appends exactly that line to exactly that file, and the empty purge has neither an effect nor a line

  # canon: {"schema":1,"at":"2026-11-02T15:05:19Z","actor":{"agent_id":"impl-plan-jcs","role":"implementer","session_id":"s-0-35-plan-jcs"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-35-plan-jcs.F3
  @atip.planjcs.10
  Scenario: A purge preview and a refused purge commit write nothing: the tree of the repository is the same
    Given a repo with records of a kind and an audit file
    When purge is previewed, and committed with a stale digest
    Then the tree of the repository before and after each call is the same, the audit file included

  # canon: {"schema":1,"at":"2026-11-02T15:05:47Z","actor":{"agent_id":"impl-plan-jcs","role":"implementer","session_id":"s-0-35-plan-jcs"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-35-plan-jcs.F4
  @atip.planjcs.11
  Scenario: A digest made the 0.34 way (SHA-256 hex of the effects, no prefix) is PLAN_STALE on the six commands, and the hint says to run the preview again
    Given the six commands and a digest made as 0.34 made it: the SHA-256 of the effects' JSON in 64 hex digits
    When the commit is run with it
    Then each is exit 2 PLAN_STALE and no 0.34 form is accepted, and the hint says to run the preview again and commit the digest it prints

  # canon: {"schema":1,"at":"2026-11-02T15:06:10Z","actor":{"agent_id":"impl-plan-jcs","role":"implementer","session_id":"s-0-35-plan-jcs"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-35-plan-jcs.F5
  @atip.planjcs.12
  Scenario: The CHANGELOG has the upgrade note (the digest form, JCS and the inputs, the previews of 0.34 and the lock) and the sources hold no code that makes a digest without the prefix
    Given the CHANGELOG and the sources of canon-cli
    When the entry of this version is read, and src/safety.rs is read
    Then the entry gives the digest form (sha256:, JCS, the inputs), says that a preview made by 0.34 is PLAN_STALE after the upgrade, lists what each command reads and the writers that hold the lock, and no code makes a digest without the prefix or over the effects alone

  # canon: {"schema":1,"at":"2026-11-02T15:06:36Z","actor":{"agent_id":"impl-plan-jcs","role":"implementer","session_id":"s-0-35-plan-jcs"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-35-plan-jcs.F1
  @atip.planjcs.13
  Scenario: The report schemas of the five commands take the preview's members from the contract's Effect and raise the minor of schemaVersion
    Given canon schema of purge, artifacts move, migrate, ledger compact and tier age
    When their report schemas are read
    Then each holds planDigest, effects and committed as the contract's Effect and preview give them, and the minor of schemaVersion is above that of 0.34
