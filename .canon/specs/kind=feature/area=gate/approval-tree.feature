Feature: gate approval-tree
  # canon: {"schema":1,"at":"2026-11-05T10:32:26Z","actor":{"agent_id":"impl-approval-evidence-037","session_id":"s-0-37-approval-evidence"}}

  # canon: {"schema":1,"at":"2026-11-05T10:32:26Z","actor":{"agent_id":"impl-approval-evidence-037","session_id":"s-0-37-approval-evidence"}}
  @subject:gate-trust-spine
  @case:happy
  @flow:canon-0-37-approval-evidence.F1
  @gate.approval-tree.01
  Scenario: Approving a chore records the code tree of that moment outside the digest, and approving again over other code records the new tree
    Given a git repository with a commit, a subject with an unapproved chore change, and a brief that passes `canon brief check`
    When the human approves the chore with `canon brief approve`
    Then the `Approved:` line carries `(code tree <id>)` before the brief digest, and the id is the code tree an evidence record binds for the work tree as it is, uncommitted code included
    And the brief digest the line binds is the digest it had without the marker, so the brief still reads as approved
    And approving again after the code changed writes a new line with the new tree, and approving again over the same code writes nothing

  # canon: {"schema":1,"at":"2026-11-05T10:32:29Z","actor":{"agent_id":"impl-approval-evidence-037","session_id":"s-0-37-approval-evidence"}}
  @subject:gate-trust-spine
  @case:failure
  @flow:canon-0-37-approval-evidence.F1
  @gate.approval-tree.02
  Scenario: An approval in a repository with no git work tree or no commit is written without a tree and with no error
    Given a directory that is not a git work tree, and another that is a git work tree with no commit, each holding a chore brief that passes the check
    When the chore is approved in each
    Then the approval exits 0 in both with no error, and the `Approved:` line has no `(code tree …)` marker
    And the brief reads as approved, and its approval reads as one written before 0.37

  # canon: {"schema":1,"at":"2026-11-05T10:32:32Z","actor":{"agent_id":"impl-approval-evidence-037","session_id":"s-0-37-approval-evidence"}}
  @subject:gate-trust-spine
  @case:edge
  @flow:canon-0-37-approval-evidence.F1
  @gate.approval-tree.03
  Scenario: The dashboard's approval, the command's approval and a relayed approval record the tree alike, and the relay still reads back
    Given a git repository with clean briefs, served by `canon dashboard`
    When one brief is approved from the briefs page, one with `canon brief approve`, and one as a relayed approval with `--relayed-via`
    Then the three `Approved:` lines carry the same `(code tree <id>)`, the relayed line keeps `(relayed by <role> <actor> via <tool>)` before it
    And the briefs list still names the relayed approval with its role, actor and tool, and the page's approval carries the same marker as the command's

  # canon: {"schema":1,"at":"2026-11-05T10:32:35Z","actor":{"agent_id":"impl-approval-evidence-037","session_id":"s-0-37-approval-evidence"}}
  @subject:gate-trust-spine
  @case:happy
  @flow:canon-0-37-approval-evidence.F2
  @gate.approval-tree.04
  Scenario: A chore approved over unchanged code asks for no evidence where the evidence is on an older tree, only its change review
    Given a verifying subject whose scenarios have evidence and reviews on an older code tree, because another change edited the code after the evidence
    When the human approves a chore over that code and the subject is moved to shipped
    Then the refusal names no evidence for any scenario and asks for the chore's change-level review alone
    And once the change review is recorded the move succeeds

  # canon: {"schema":1,"at":"2026-11-05T10:32:38Z","actor":{"agent_id":"impl-approval-evidence-037","session_id":"s-0-37-approval-evidence"}}
  @subject:gate-trust-spine
  @case:failure
  @flow:canon-0-37-approval-evidence.F2
  @gate.approval-tree.05
  Scenario: Without the change review the move to shipped is refused with exit 2 and the review command leads
    Given the same subject and an approved chore over unchanged code, with no change-level review
    When the subject is moved to shipped
    Then it exits 2 and the refusal names the missing review of the chore
    And the hint leads with `canon review change <chore> …` and no `canon evidence add` command appears

  # canon: {"schema":1,"at":"2026-11-05T10:32:41Z","actor":{"agent_id":"impl-approval-evidence-037","session_id":"s-0-37-approval-evidence"}}
  @subject:gate-trust-spine
  @case:happy
  @flow:canon-0-37-approval-evidence.F3
  @gate.approval-tree.06
  Scenario: A chore approved before a code change asks for the scenarios with one batch command, and the command closes the gap
    Given a verifying subject with evidence and reviews, and a chore approved by `canon brief approve`
    When the code is changed after the approval and the subject is moved to shipped
    Then the refusal lists every evidenced scenario and leads its hint with one batch command, `canon evidence add --all-scenarios --report junit:a.xml --report junit:b.xml --role implementer`
    And running that command closes the evidence gap, leaving the chore's change review

  # canon: {"schema":1,"at":"2026-11-05T10:32:44Z","actor":{"agent_id":"impl-approval-evidence-037","session_id":"s-0-37-approval-evidence"}}
  @subject:gate-trust-spine
  @case:failure
  @flow:canon-0-37-approval-evidence.F3
  @gate.approval-tree.07
  Scenario: One changed file after the approval brings the requirement back, and a file code.exclude drops does not
    Given the same approved chore, with `generated/**` among the `code.exclude` globs of canon.yaml
    When one source file is added after the approval, and when only a file under `generated/` changes
    Then the added file makes the tree differ, so the evidence requirement stands with its batch command
    And a change to the excluded file alone leaves the tree as it was, and the move asks for no evidence

  # canon: {"schema":1,"at":"2026-11-05T10:32:47Z","actor":{"agent_id":"impl-approval-evidence-037","session_id":"s-0-37-approval-evidence"}}
  @subject:gate-trust-spine
  @case:happy
  @flow:canon-0-37-approval-evidence.F4
  @gate.approval-tree.08
  Scenario: An approval with no recorded tree is judged as in 0.36, over unchanged code too
    Given a chore whose `Approved:` line has no recorded tree, as every approval written before 0.37
    When the code changed after the approval, and when it did not and the evidence is on an older tree
    Then both ask for every evidenced scenario with the batch command, as 0.36 did
    And a chore over code that did not change since the evidence stays exempt, as 0.36 did

  # canon: {"schema":1,"at":"2026-11-05T10:32:51Z","actor":{"agent_id":"impl-approval-evidence-037","session_id":"s-0-37-approval-evidence"}}
  @subject:gate-trust-spine
  @case:failure
  @flow:canon-0-37-approval-evidence.F4
  @gate.approval-tree.09
  Scenario: An approval line written before 0.37 is read by status and gate check with no error and no new information
    Given a chore brief approved by a line written by 0.36, human or relayed, with a digest and no tree
    When `canon status`, `canon gate check` and `canon brief check` read it
    Then each reads the approval with no error and no warning about the missing tree, and `status` shows no unchanged-tree line for it
    And the relayed line still reads back its role, actor and tool

  # canon: {"schema":1,"at":"2026-11-05T10:32:54Z","actor":{"agent_id":"impl-approval-evidence-037","session_id":"s-0-37-approval-evidence"}}
  @subject:gate-trust-spine
  @case:happy
  @flow:canon-0-37-approval-evidence.F5
  @gate.approval-tree.10
  Scenario: The move to shipped, status and gate check state the same requirement in the same repository
    Given one repository in each state: a chore approved over unchanged code, and the same chore after the code changed
    When the move to shipped, `canon status` and `canon gate check` are read
    Then over unchanged code none of them asks for evidence and each asks for the change review
    And after the code changed all three name the same scenarios, and the move and `status` lead with the same batch command

  # canon: {"schema":1,"at":"2026-11-05T10:32:57Z","actor":{"agent_id":"impl-approval-evidence-037","session_id":"s-0-37-approval-evidence"}}
  @subject:gate-trust-spine
  @case:failure
  @flow:canon-0-37-approval-evidence.F5
  @gate.approval-tree.11
  Scenario: Status says why a chore asks for nothing and leads with no evidence command; over other code it says neither
    Given an approved chore whose recorded tree is the current tree, and the same chore after the code changed
    When `canon status` is read, in text and with `--json`
    Then the unchanged chore's line says the code tree is the one it was approved on and asks for no evidence, its brief has `treeUnchanged: true`, and `meta.next` names no evidence command
    And over changed code the line is absent, `treeUnchanged` is absent and the batch command leads

  # canon: {"schema":1,"at":"2026-11-05T10:33:00Z","actor":{"agent_id":"impl-approval-evidence-037","session_id":"s-0-37-approval-evidence"}}
  @subject:gate-trust-spine
  @case:happy
  @flow:canon-0-37-approval-evidence.F6
  @gate.approval-tree.12
  Scenario: The CHANGELOG says an unchanged-tree chore approval asks for no evidence and names the new information of the approval line
    Given the CHANGELOG
    When its Unreleased section is read
    Then a Changed entry says a chore approval over an unchanged code tree asks for no evidence, names the `(code tree <id>)` marker of the approval line, and says an approval written before 0.37 behaves as it did

  # canon: {"schema":1,"at":"2026-11-05T10:33:05Z","actor":{"agent_id":"impl-approval-evidence-037","session_id":"s-0-37-approval-evidence"}}
  @subject:gate-trust-spine
  @case:failure
  @flow:canon-0-37-approval-evidence.F6
  @gate.approval-tree.13
  Scenario: A CHANGELOG without that entry is named by the release check
    Given the CHANGELOG's Unreleased section without that entry, and without the marker or the line about older approvals
    When the release check reads it
    Then it names each missing piece, and the real section has none
