Feature: atip incomplete
  # canon: {"schema":1,"at":"2026-11-04T10:51:55Z","actor":{"agent_id":"impl-codes-incomplete","role":"implementer","session_id":"s-0-36-codes-incomplete"}}

  # canon: {"schema":1,"at":"2026-11-04T10:51:55Z","actor":{"agent_id":"impl-codes-incomplete","role":"implementer","session_id":"s-0-36-codes-incomplete"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-incomplete.F2
  @atip.incomplete.01
  Scenario: A record the gate refuses after it was staged is its subject's INCOMPLETE code, stays staged and leads meta.next with the read of what is staged
    Given a repository with an indexed scenario, a committed finding, and a debug-build seam that damages each staged record between its staging and its promotion
    When finding add, finding close, evidence add and divergence add run with --promote, then a batch of two finding add elements, then gate promote, then finding add without the damage
    Then each of the four exits 1 with FINDING_INCOMPLETE, EVIDENCE_INCOMPLETE or DIVERGENCE_INCOMPLETE, the reason PROMOTION_INVALID, retryable false and no data, its record stays staged, nothing is committed and meta.next leads with gate promote --dry-run; the batch element is failed with its own code and reason, the second is skipped and the batch is BATCH_INCOMPLETE; gate promote refuses the damaged records at exit 2 with PROMOTION_INVALID and no data; the call without the damage commits

  # canon: {"schema":1,"at":"2026-11-04T10:52:03Z","actor":{"agent_id":"impl-codes-incomplete","role":"implementer","session_id":"s-0-36-codes-incomplete"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-incomplete.F2
  @atip.incomplete.02
  Scenario: A promotion the gate blocks after a contradicting trajectory demoted the strategy is STRATEGY_INCOMPLETE, and one with nothing demoted is a refusal
    Given an active strategy whose git-tier file has its front matter, a contradicting failed trajectory and a regime the promotion gate blocks, and a second repository with a blocked strategy and no contradicting trajectory
    When strategy promote runs on each
    Then the first exits 1 with STRATEGY_INCOMPLETE, the reason STRATEGY_PROMOTION_INVALID, not retryable, meta.next leading with the preview of the same strategy and the strategy still demoted; the second exits 2 with STRATEGY_PROMOTION_INVALID, no data and nothing written

  # canon: {"schema":1,"at":"2026-11-04T10:52:07Z","actor":{"agent_id":"impl-codes-incomplete","role":"implementer","session_id":"s-0-36-codes-incomplete"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-incomplete.F4
  @atip.incomplete.03
  Scenario: An artifact store that stops after a blob was stored is ARTIFACTS_STORE_INCOMPLETE, retryable, and the same call stores the rest
    Given a repository with two bound artifacts whose store was emptied, and a debug-build seam that fails the second blob write after one was stored
    When artifacts store runs with the seam, then again without it
    Then the first exits 1 with ARTIFACTS_STORE_INCOMPLETE, retryable true, the reason ARTIFACTS_STORE_FAILED, meta.next leading with artifacts verify and one blob stored; the second exits 0 and the store holds both

  # canon: {"schema":1,"at":"2026-11-04T10:52:12Z","actor":{"agent_id":"impl-codes-incomplete","role":"implementer","session_id":"s-0-36-codes-incomplete"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-incomplete.F5
  @atip.incomplete.04
  Scenario: A hook install that stops after the first file is HOOKS_INSTALL_INCOMPLETE and one that wrote nothing is WRITE_FAILED
    Given a git repository and a debug-build seam that fails the write of the second hook file
    When gate install-hooks runs with the seam, then without it, then with it again over the finished install, then as a preview with it, then on a fresh repository with a seam that fails the pre-commit script after both hook files
    Then the first exits 1 with HOOKS_INSTALL_INCOMPLETE, the reason WRITE_FAILED, not retryable, meta.next leading with status, the Claude hook file installed and the Codex one not written; the second finishes the install; the third exits 1 with WRITE_FAILED and no reason because nothing of its own was kept; the preview exits 0; the script failure exits 1 with HOOKS_INSTALL_INCOMPLETE and the reason WRITE_FAILED, both hook files installed, and the rerun writes the script. The reason of every other stop here is WRITE_FAILED too

  # canon: {"schema":1,"at":"2026-11-04T10:52:17Z","actor":{"agent_id":"impl-codes-incomplete","role":"implementer","session_id":"s-0-36-codes-incomplete"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-incomplete.F5
  @atip.incomplete.05
  Scenario: A purge that stops after it deleted is RETENTION_PURGE_INCOMPLETE and its digest is stale
    Given a repository with two trajectories past the cutoff and a debug-build seam that stops the purge once the tiers were deleted
    When purge runs with the seam on the plan it previewed, then the same digest again, then on a second repository without the seam, then with a seam that fails the purge before it deleted anything
    Then the first exits 1 with RETENTION_PURGE_INCOMPLETE, the reason WRITE_FAILED, not retryable, data naming the counts, meta.next leading with status and the records gone; the same digest exits 2 with PLAN_STALE; the unstopped purge exits 0 and deletes the same records; the purge that deleted nothing exits 2 with RETENTION_TIERS_INVALID, never RETENTION_PURGE_INCOMPLETE, and the records stay

  # canon: {"schema":1,"at":"2026-11-04T10:52:21Z","actor":{"agent_id":"impl-codes-incomplete","role":"implementer","session_id":"s-0-36-codes-incomplete"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-incomplete.F5
  @atip.incomplete.06
  Scenario: A compaction that stops after it moved is LEDGER_COMPACT_INCOMPLETE and its digest is stale
    Given a ledger of three versions of one subject with an empty archive and a debug-build seam that stops the compaction once the versions moved
    When ledger compact runs with the seam on the plan it previewed, then the same digest again, then on a second ledger without the seam, then as a preview and as a commit over a ledger whose kind directories are mode 000
    Then the first exits 1 with LEDGER_COMPACT_INCOMPLETE, the reason WRITE_FAILED, not retryable, meta.next leading with status and the superseded versions in the archive; the same digest exits 2 with PLAN_STALE; the unstopped compaction exits 0 and moves the same versions; over the unreadable ledger the preview and the commit exit 2 with REPO_DENIED, never LEDGER_COMPACT_INCOMPLETE, with no read-what-moved next, and nothing moved

  # canon: {"schema":1,"at":"2026-11-04T10:52:26Z","actor":{"agent_id":"impl-codes-incomplete","role":"implementer","session_id":"s-0-36-codes-incomplete"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-incomplete.F5
  @atip.incomplete.07
  Scenario: Tier aging that stops after records moved is TIER_INCOMPLETE and one that moved none is TIER_FAILED
    Given a repository with two trajectories past the aging threshold and one recent, and a debug-build seam that stops the aging once the records moved
    When tier age runs with the cold tier not attached, then with the seam on the plan it previewed
    Then the first exits 1 with TIER_FAILED, not retryable, and every record is where it was; the second exits 1 with TIER_INCOMPLETE, the reason WRITE_FAILED, not retryable, meta.next leading with status and only the recent record left in the tier

  # canon: {"schema":1,"at":"2026-11-04T10:52:30Z","actor":{"agent_id":"impl-codes-incomplete","role":"implementer","session_id":"s-0-36-codes-incomplete"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-incomplete.F3
  @atip.incomplete.08
  Scenario: A plugin sync that could not write a candidate is PLUGIN_SYNC_INCOMPLETE and the written ones stay
    Given the porting plugin with two candidates and a debug-build seam that fails the write of the second
    When plugin sync runs with the seam, then without it, then with a seam that fails every candidate, as a write and as a preview
    Then the first exits 1 with PLUGIN_SYNC_INCOMPLETE, the reason WRITE_FAILED, not retryable, data.roots naming the error and one written, meta.next leading with status and one overlay record stored; the second exits 0, writes the left candidate and finds the first already there; when every candidate fails the write exits 1 with WRITE_FAILED, written 0 and no read-what-was-written next, and the preview exits 2 with a REPO_ refusal, never PLUGIN_SYNC_INCOMPLETE

  # canon: {"schema":1,"at":"2026-11-04T10:52:35Z","actor":{"agent_id":"impl-codes-incomplete","role":"implementer","session_id":"s-0-36-codes-incomplete"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-incomplete.F3
  @atip.incomplete.09
  Scenario: A spec root left unindexed after an earlier root was indexed is INVENTORY_INCOMPLETE, and with nothing indexed it is the refusal INVENTORY_INVALID
    Given two spec roots whose first is clean and whose second declares one scenario twice, then has a scenario without provenance, then is clean
    When inventory sync runs on each corpus, then on a single root with a violation, then as a preview over the first corpus, then with a seam that fails the store while the second root is indexed, on a fresh repository and again over the indexed one
    Then the first exits 1 with INVENTORY_INCOMPLETE, not retryable, the reason INVENTORY_INVALID, data.roots saying the first root was indexed and the second was not, meta.next leading with status and the first root's records stored; the rerun, the violation and the single root index nothing in the run and exit 2 with INVENTORY_INVALID, the reasons DUPLICATE_SCENARIOS and VIOLATIONS, each violation and duplicate by root in error.details and no data, a message that says nothing was indexed, no read-what-was-indexed next and no ledger created; the preview exits 2 with INVENTORY_INVALID saying it is a preview and writes nothing; the store failure exits 1 with INVENTORY_INCOMPLETE and the reason INVENTORY_FAILED on the fresh repository and with INVENTORY_FAILED over the indexed one; a clean corpus exits 0

  # canon: {"schema":1,"at":"2026-11-04T10:52:40Z","actor":{"agent_id":"impl-codes-incomplete","role":"implementer","session_id":"s-0-36-codes-incomplete"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-incomplete.F3
  @atip.incomplete.10
  Scenario: A migrate that stopped after it began is MIGRATION_INCOMPLETE with the reason STOPPED, PINS or INDEX
    Given a 0.16 repository whose canon.yaml temp file cannot be created, one run with a debug-build seam that fails the review pins, and one whose corpus has a scenario without provenance
    When migrate runs with its previewed digest on each, then again after the cause is gone
    Then they exit 1 with MIGRATION_INCOMPLETE, not retryable, the reasons STOPPED, PINS and INDEX, meta.next leading with status, what moved stays moved and the index report in data.index for the last; running migrate again finishes the rest

  # canon: {"schema":1,"at":"2026-11-04T10:52:44Z","actor":{"agent_id":"impl-codes-incomplete","role":"implementer","session_id":"s-0-36-codes-incomplete"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-incomplete.F1
  @atip.incomplete.11
  Scenario: A command lists one INCOMPLETE code of its own and two fail the check
    Given the registry and the code list each command answers with, a registry that adds a second INCOMPLETE code to one command and one that adds the batch's code
    When the check of the one-per-subject rule runs on each
    Then the real commands pass and each lists the INCOMPLETE code of its subject, the doubled command fails naming the rule, and the batch's code is not counted

  # canon: {"schema":1,"at":"2026-11-04T10:52:49Z","actor":{"agent_id":"impl-codes-incomplete","role":"implementer","session_id":"s-0-36-codes-incomplete"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-incomplete.F6
  @atip.incomplete.12
  Scenario: The CHANGELOG replacement table covers every replaced code
    Given the CHANGELOG section of the release and the list of the codes this change replaced and added
    When the replacement table is checked, then with a row dropped, then with an INCOMPLETE code left out
    Then every replaced code has a row and every INCOMPLETE code is said; the dropped row and the left-out code are each named

  # canon: {"schema":1,"at":"2026-11-05T09:00:01Z","actor":{"agent_id":"impl-flowfix-036","role":"implementer","session_id":"s-0-36-flowfix-2"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-incomplete.F6
  @atip.incomplete.13
  Scenario: A replacement table row with a retry value the registry does not give, a code the registry lacks, or no row or two rows for a replaced code fails the check
    Given the CHANGELOG's replacement table and the codes this change replaced
    When one row's retry value after the arrow is flipped, a new code is misspelled, a row is deleted and a row is doubled, each in turn
    Then the table as it stands passes and each fault is named with its row
