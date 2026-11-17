Feature: change drop
  # canon: {"schema":1,"at":"2026-11-16T05:57:09Z","actor":{"agent_id":"canon-scaffold"}}

  # canon: {"schema":1,"at":"2026-11-16T05:57:09Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:happy
  @flow:canon-0-42-change-drop.F1
  @change.drop.01
  Scenario: A human defers an approved chore and the decision is recorded with reason, references, approver and time
    Given an approved chore on a repo whose canon.yaml routes change_state
    When a human runs canon change defer with --reason, one --ref and --approver
    Then one change_state record is written with state deferred, the reason as given, the references, the approver and the time
    And the report names the change, the state, the scenarios set aside and the subjects that stop requiring it

  # canon: {"schema":1,"at":"2026-11-16T05:57:20Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:happy
  @flow:canon-0-42-change-drop.F1
  @change.drop.02
  Scenario: An agent relaying the human's answer leaves the relay role, actor and tool on the record, and a repeat writes nothing
    Given an agent that declared a role and has the human's answer
    When it runs canon change defer with --relayed-via, --actor and --approver
    Then the record carries the relaying role, actor and tool
    And the same call again writes nothing and answers unchanged with exit 0

  # canon: {"schema":1,"at":"2026-11-16T05:57:24Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:edge
  @flow:canon-0-42-change-drop.F1
  @change.drop.03
  Scenario: A change with evidence, review and finding records is deferred with a warning that keeps every one of them
    Given a change with evidence, review and finding records of its own and of its scenarios
    When a human defers it
    Then it succeeds with the warning CHANGE_RECORDS_KEPT counting each kind
    And every one of those records is still there and still queried

  # canon: {"schema":1,"at":"2026-11-16T05:57:28Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-42-change-drop.F1
  @change.drop.04
  Scenario: A defer an agent makes without the human's answer, or with a bad relay or bad argument, is refused and writes nothing
    Given an agent role without a relayed answer, a repo with approval relay denied, and calls with no actor, no role, an empty approver, an empty reason or no reference
    When canon change defer is run in each case
    Then each is refused (ACTOR_DENIED, USAGE, APPROVAL_INVALID or CHANGE_ARGUMENT_INVALID) with exit 2
    And nothing is written

  # canon: {"schema":1,"at":"2026-11-16T05:57:32Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-42-change-drop.F1
  @change.drop.05
  Scenario: A defer of an unknown change, or a different repeat on a closed change, is refused and writes nothing
    Given a change that has no record and a change already deferred
    When canon change defer names the unknown change, and names the deferred one with another reason
    Then the first is CHANGE_NOT_FOUND and the second CHANGE_CONFLICT with reason CLOSED
    And nothing is written for either

  # canon: {"schema":1,"at":"2026-11-16T05:57:37Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-42-change-drop.F1
  @change.drop.06
  Scenario: A change that already shipped cannot be closed, and one that never shipped can, whatever its later approvals
    Given a subject that moved to shipped with the change, then was reopened and the change approved again; another change adopted into the already shipped subject; one that was an unapproved draft when the subject shipped; and one that was deferred when its subject shipped without it
    When canon change defer or drop is run on each
    Then the first is CHANGE_CONFLICT with reason SHIPPED and writes nothing, the other three are closed
    And a change reopened and shipped with again after that deferral is refused as SHIPPED, and a move history or a decision history that cannot be read is refused the same way with a hint to write a new change

  # canon: {"schema":1,"at":"2026-11-16T05:57:41Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:happy
  @flow:canon-0-42-change-drop.F2
  @change.drop.07
  Scenario: A human drops an approved change, a deferred change and an unapproved draft, each recorded as dropped
    Given an approved change, a deferred change and an unapproved draft
    When a human runs canon change drop on each
    Then each is recorded as dropped
    And the report says the dropped change is not brought back, only a new change is

  # canon: {"schema":1,"at":"2026-11-16T05:57:45Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-42-change-drop.F2
  @change.drop.08
  Scenario: A dropped change takes no other state, and a dry run refuses with the same codes and writes nothing
    Given a dropped change
    When canon change defer or drop is run on it with other values, or with --dry-run
    Then other values are CHANGE_CONFLICT with reason CLOSED, the same values are unchanged
    And a dry run refuses with the same codes and writes nothing

  # canon: {"schema":1,"at":"2026-11-16T05:57:49Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:happy
  @flow:canon-0-42-change-drop.F3
  @change.drop.09
  Scenario: A closed change stops turning gate check red and stops being asked for by ship, reopen and status
    Given a shipped subject whose approved chore has no change review and turns gate check red
    When the chore is deferred
    Then gate check is green, the ship guard, the reopen guard and the status next steps no longer ask for the chore
    And status shows the change as deferred with the command that reopens it

  # canon: {"schema":1,"at":"2026-11-16T05:57:53Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:happy
  @flow:canon-0-42-change-drop.F3
  @change.drop.10
  Scenario: The scenarios only a closed change made are set aside, reported by status and gate check, and its open blocker finding no longer blocks
    Given a closed change that made scenarios of its own and has an open blocker finding
    When gate check and status are read
    Then the scenarios are set aside and listed in status setAsideScenarios and in the gate advisory closed_change
    And the finding stays on record and no longer blocks

  # canon: {"schema":1,"at":"2026-11-16T05:57:56Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:happy
  @flow:canon-0-42-change-drop.F3
  @change.drop.11
  Scenario: A set-aside scenario's stale evidence and its open or divergent verdict no longer turn gate check red
    Given a set-aside scenario whose evidence is past max_commits_behind and another whose verdict is divergent
    When gate check is run
    Then neither turns it red
    And the same scenarios of an open change still do

  # canon: {"schema":1,"at":"2026-11-16T05:58:01Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-42-change-drop.F3
  @change.drop.12
  Scenario: What another change requires stays required, and status, gate check and subject move set aside the same scenarios
    Given a closed change beside an open one, a scenario tagged for both, and an untagged scenario
    When status, gate check and subject move are read
    Then the scenarios the open change, the shared tag and the missing tag require stay required
    And the three readers set aside the same scenarios, a subject left with none is refused at ship

  # canon: {"schema":1,"at":"2026-11-16T05:58:05Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-42-change-drop.F3
  @change.drop.13
  Scenario: A closed change's tag on a scenario that predates it, follows the close, or has no creation time is ignored and reported
    Given a scenario older than the change, one made after it was closed, and one with no creation time, each tagged for the closed change
    When gate check is run
    Then each stays required
    And the advisory closed_change names each ignored tag with its scenario id and reason

  # canon: {"schema":1,"at":"2026-11-16T05:58:08Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:happy
  @flow:canon-0-42-change-drop.F3
  @change.drop.14
  Scenario: A fresh full clone sets aside the same scenarios and gate check is green
    Given a deferred change with scenarios made inside its window
    When the repository is cloned in full and inventory sync and gate check are run there
    Then gate check is green
    And status lists the same set-aside scenarios as the original work tree

  # canon: {"schema":1,"at":"2026-11-16T05:58:12Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-42-change-drop.F3
  @change.drop.15
  Scenario: A shallow clone sets nothing aside and says the history is unavailable
    Given the same repository cloned with --depth 1
    When status and gate check are run there
    Then nothing is set aside and the reason history_unavailable is said by both
    And the scenarios stay required while the change-level requirement is gone

  # canon: {"schema":1,"at":"2026-11-16T05:58:16Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:happy
  @flow:canon-0-42-change-drop.F4
  @change.drop.16
  Scenario: Query and search find a change's decision by change, state, history and the words of its reason
    Given a deferred change that was reopened
    When canon query --kind change_state is run with --change, --status and --history, and canon search with a word of the reason
    Then the current state, the history before the reopen, and the record with its change and subject as neighbours are returned
    And the reason, references, approver and time are in each record

  # canon: {"schema":1,"at":"2026-11-16T05:58:20Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-42-change-drop.F4
  @change.drop.17
  Scenario: An unknown state is a usage refusal, a change never closed is an empty answer, and a canon.yaml without the line is refused
    Given an unknown --status value, a change that was never closed and a canon.yaml without the change_state line
    When canon query --kind change_state and canon change defer are run
    Then the unknown value is USAGE with the three states as expected, the never closed change is an empty list, the missing line is QUERY_FAILED for query and CONFIG_INVALID TIERS for defer
    And the CONFIG_INVALID hint says canon migrate

  # canon: {"schema":1,"at":"2026-11-16T05:58:24Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:happy
  @flow:canon-0-42-change-drop.F5
  @change.drop.18
  Scenario: A human reopens a deferred change, it returns to its subject with its scenarios and needs a new approval
    Given a deferred change
    When a human runs canon change reopen with --reason and --approver
    Then a reopened record is written, the change and its set-aside scenarios are required again
    And the earlier approval no longer counts, canon brief approve writes a new line even with the same name and answer

  # canon: {"schema":1,"at":"2026-11-16T05:58:27Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-42-change-drop.F5
  @change.drop.19
  Scenario: A dropped or never closed change is not reopened, an agent needs the human's answer, and shipping before the new approval is refused
    Given a dropped change, a change never closed, an agent without the human's answer and a repo that denies relay
    When canon change reopen is run in each case
    Then they are CHANGE_CONFLICT with reason DROPPED, NOT_DEFERRED, ACTOR_DENIED and a relay refusal
    And a reopened change shipped without a new approval is refused for its approval

  # canon: {"schema":1,"at":"2026-11-16T05:58:31Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:happy
  @flow:canon-0-42-change-drop.F6
  @change.drop.20
  Scenario: Unadopt keeps refusing an approved or flow-tagged change and its hint names change defer and change drop
    Given an approved change and a change with a flow-tagged scenario
    When canon subject unadopt is run on each
    Then both refusals are as before
    And the hint says a human decides with canon change defer or canon change drop

  # canon: {"schema":1,"at":"2026-11-16T05:58:35Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-42-change-drop.F6
  @change.drop.21
  Scenario: A brief approval, a dashboard approval and a flow-tagged scenario for a closed change are refused and write nothing
    Given a closed change
    When canon brief approve (alone and among several ids), the dashboard's approval and canon scenario new --flow --change are run
    Then each is CHANGE_CONFLICT with reason CLOSED
    And nothing is written, the other ids of a multi-id call stay unapproved

  # canon: {"schema":1,"at":"2026-11-16T05:58:39Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:happy
  @flow:canon-0-42-change-drop.F7
  @change.drop.22
  Scenario: A shipped subject's approved chore turns gate check red, a deferral makes it green and a reopen makes it red again, in a fresh clone too
    Given a shipped subject with an approved chore and no change review in a temporary repository
    When gate check, canon change defer, gate check in a new clone and canon change reopen are run
    Then gate check is red, then green, green in the new clone with the history in canon query, then red again
    And with no such change in the tree the call is CHANGE_NOT_FOUND

  # canon: {"schema":1,"at":"2026-11-16T05:58:42Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-42-change-drop.F7
  @change.drop.23
  Scenario: A tree without the change, an agent without the human's answer and a gitignore that drops the records each leave the first use refused or visible
    Given a tree without the change, an agent without the human's answer and a .gitignore that ignores the ledger without re-including change_state
    When canon change defer is run, and a clone is made
    Then the first is CHANGE_NOT_FOUND, the second ACTOR_DENIED
    And the clone from the bad .gitignore requires the change again, while this repo's .gitignore and canon init's do not ignore the records

  # canon: {"schema":1,"at":"2026-11-16T05:58:46Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:happy
  @flow:canon-0-42-change-drop.F8
  @change.drop.24
  Scenario: canon init routes every record kind and change_state to local
    Given a new repository
    When canon init runs
    Then the canon.yaml routing has a line for every record kind and for change_state, set to local

  # canon: {"schema":1,"at":"2026-11-16T05:58:54Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:happy
  @flow:canon-0-42-change-drop.F9
  @change.drop.26
  Scenario: Migrate adds the change_state line where the routing section lacks it, previews its line number, and reverts it
    Given a canon.yaml whose routing section has no change_state line
    When canon migrate previews and then commits with the plan digest
    Then the preview names the line number and change_state: local, the commit adds only that line, the result equals the old document plus that key
    And change defer runs, the next migrate plan is empty and --revert restores the file

  # canon: {"schema":1,"at":"2026-11-16T05:58:57Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-42-change-drop.F9
  @change.drop.27
  Scenario: Migrate leaves a routing section it cannot add a line to, a file without routing and a stale preview unchanged
    Given a routing section written with an anchor or a flow mapping, a canon.yaml without routing, and a stale digest
    When canon migrate is run on each
    Then the first is CONFIG_INVALID with reason CANON_YAML naming the line to add by hand, the second is left byte for byte, the third is PLAN_STALE
    And no file is changed in any case

  # canon: {"schema":1,"at":"2026-11-16T05:59:01Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-42-change-drop.F9
  @change.drop.28
  Scenario: A closing command on a canon.yaml without the line is refused with a hint to migrate, and the migrate commit needs no human answer
    Given a canon.yaml without the change_state line and an agent with a declared role
    When canon change defer is run, then the migrate commit
    Then the closing command is CONFIG_INVALID TIERS with a hint to canon migrate
    And the agent runs the migrate commit without a human's answer

  # canon: {"schema":1,"at":"2026-11-16T05:59:05Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:edge
  @flow:canon-0-42-change-drop.F1
  @change.drop.29
  Scenario: The three closing leaves read a JSON document, preview with a dry run, refuse an unwritable store and a held lock, and repeat safely
    Given the three closing leaves
    When they are run with --input, --dry-run, on an unwritable store, under a held lock and twice
    Then each behaves as the general rules say: one JSON document is read, a dry run writes nothing, an unwritable store and a held lock are refused
    And a repeat is safe
