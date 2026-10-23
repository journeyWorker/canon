Feature: context status
  # canon: {"schema":1,"at":"2026-10-10T15:35:47Z","actor":{"agent_id":"canon"}}

  # canon: {"schema":1,"at":"2026-10-10T15:35:47Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:happy
  @context.status.01
  Scenario: Status counts each subject's scenarios, evidence, reviews, blockers and missing cases through the gate's own joins
    Given a building subject owning scenarios on two surfaces, with evidence on some, one divergent verdict, a review by another actor on one, a self-review on another, and an open blocker on its adopted change
    When status is run
    Then the subject is listed under its status with its counted scenarios, evidenced, divergent and reviewed counts and its open blockers
    And a self-review does not count when the review rule wants a distinct actor
    And the surface with no failure case is named as a missing required case
    And the header names the canon version and the effective coverage, case and review settings

  # canon: {"schema":1,"at":"2026-10-10T15:35:47Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:happy
  @context.status.02
  Scenario: Status ends with the next commands, one per gap, chosen by fixed rules
    Given a verifying subject with a surface lacking a failure case, an unevidenced scenario, an open blocker and an evidenced but unreviewed scenario, and a building subject owning no scenario
    When status is run
    Then the next list names, in order, a new failure scenario with the next free number, an evidence attestation for the unevidenced scenario, the finding close for the blocker, a review by another actor, and a new scenario for the empty subject
    And each command carries a one-line reason

  # canon: {"schema":1,"at":"2026-10-10T15:35:47Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:happy
  @context.status.03
  Scenario: Status suggests shipping only a subject the ship gate accepts
    Given a verifying subject whose every scenario is evidenced and independently reviewed, with a failure case on its surface and its change's brief approved
    When status is run
    Then the only next command is moving that subject to shipped
    And running that command passes the ship and review gates
    And afterwards status lists the subject as shipped with nothing next

  # canon: {"schema":1,"at":"2026-10-10T15:35:47Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:edge
  @context.status.04
  Scenario: Status lists the scenarios no subject owns and how to tag them
    Given scenarios that carry no subject tag beside one that does
    When status is run
    Then the untagged scenarios are listed as unowned
    And the next list ends with tagging them in their .feature files and running canon status again, never an inventory sync

  # canon: {"schema":1,"at":"2026-10-10T15:35:47Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:failure
  @context.status.05
  Scenario: Status warns when no policy makes the gate enforce coverage
    Given a repository with subjects and scenarios but no policy file
    When status is run
    Then it exits zero, reports the policy absent and warns that the gate requires no evidence, failure cases or review
    And with a policy file that has no coverage section it warns that the section is missing instead

  # canon: {"schema":1,"at":"2026-10-10T15:35:47Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:failure
  @context.status.06
  Scenario: Status on an unreadable manifest still prints its report and exits 1 as unverifiable
    Given a manifest whose tier section does not parse
    When status is run
    Then it still prints the report and warns that the ledger cannot be read, naming the parse error
    And it exits 1 with UNVERIFIABLE and names the skipped ledger in its coverage, never a silent zero
    And nothing was counted, so no scenario or subject is reported clean

  # canon: {"schema":1,"at":"2026-10-10T15:35:47Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:edge
  @context.status.07
  Scenario: Status is a read that exits zero and writes nothing
    Given a repository with an unevidenced scenario and an open blocker
    When status is run in both the human and the JSON form
    Then each run exits zero
    And no file in the repository is created or changed, including the access audit log

  # canon: {"schema":1,"at":"2026-10-10T15:35:47Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:edge
  @context.status.08
  Scenario: Status caps the next list at five and counts the rest
    Given seven building subjects that each own no scenario and one unowned scenario
    When status is run
    Then exactly five next commands are listed
    And the three further commands are counted, not listed

  # canon: {"schema":1,"at":"2026-10-10T15:35:48Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:edge
  @context.status.09
  Scenario: Status on subjects with no scenario records points at the inventory sync
    Given subjects and a .feature corpus with scenarios that its own refresh cannot index, so the ledger holds no scenario records
    When status is run
    Then it warns that the ledger holds no scenario records
    And its only next command is the inventory sync, not one empty-subject step per subject
    And its reason says the automatic refresh could not index the files and that the sync names each format violation or duplicate id

  # canon: {"schema":1,"at":"2026-10-10T15:59:34Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:failure
  @context.status.10
  Scenario: Status fails closed when a corpus kind routes away from the rung it reads
    Given a manifest that routes scenario records to a rung other than the local one status reads
    And a verifying subject that would otherwise read as owning nothing
    When status is run
    Then it warns that the scenario kind routes away and its records are not counted
    And its only next command is the configuration check, with the reason naming the routing setting to restore
    And no step derived from the unread corpus is listed

  # canon: {"schema":1,"at":"2026-10-10T15:59:34Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:failure
  @context.status.11
  Scenario: An unusable coverage policy is a blocking gap that withholds every move-on suggestion
    Given a policy whose coverage section is unusable
    And a verifying subject whose every scenario is evidenced
    When status is run
    Then it warns that the coverage section is unusable
    And the first next command is the gate check, with the reason naming the section to fix
    And no subject is suggested to move to its next status

  # canon: {"schema":1,"at":"2026-10-10T15:59:34Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:edge
  @context.status.12
  Scenario: With several spec roots a suggested scenario command names its root
    Given a manifest that configures two spec roots
    And a subject owning a scenario under one root with no failure case, and a subject owning no scenario
    When status is run
    Then the suggested failure scenario names its owning root with the project flag, and running it as printed succeeds
    And the empty subject's suggestion carries a root placeholder and its reason lists the configured roots
    And with a single spec root no project flag is suggested

  # canon: {"schema":1,"at":"2026-10-10T15:59:34Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:edge
  @context.status.13
  Scenario: The header prints the policy settings as resolved, not as defaults
    Given a policy that narrows the coverage scope, excludes a lane, requires no case and has no review rule
    When status is run
    Then the header prints each setting as resolved, with no case required and the review rule absent

  # canon: {"schema":1,"at":"2026-10-10T18:06:29Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:edge
  @context.status.14
  Scenario: Every status write suggestion names the unit and its session
    Given a subject with an unevidenced scenario, an open blocker finding and a due review
    When status is run
    Then every suggested command parses as printed
    And every write suggestion carries `--actor-id <unit>` and `--session-id <session>` wherever its command takes them

  # canon: {"schema":1,"at":"2026-10-10T18:33:15Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:edge
  @context.status.15
  Scenario: With no feature scenarios at all, status says to write them rather than sync
    Given a subject and no .feature file holding any scenario
    When status is run
    Then the next list names canon feature new, then a scenario new step with --subject for the subject
    And it never suggests canon inventory sync, which would index nothing

  # canon: {"schema":1,"at":"2026-10-10T19:28:02Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:failure
  @context.status.40
  Scenario: Status warns that the policy in force is not the approved one
    Given a repository whose policy has no approval record, or whose policy changed after it was approved
    When canon status runs
    Then it warns that the policy is not approved, or changed since approval, and names `canon policy approve`
    And the warning appears in the --json warnings too
    And status still exits 0

  # canon: {"schema":1,"at":"2026-10-11T14:44:45Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:context-capability
  @case:happy
  @context.status.60
  Scenario: Status reads a subject's brief approval from its newest change's canon brief.md
    Given a building subject adopting two changes, the newer of whose brief.md has a line starting "Approved:" with the human's words
    And a shipped subject with an approved brief and a specced subject
    When canon status runs
    Then the building and shipped subjects each show their brief as approved, naming the newest change, its brief.md and the recorded words, in --json and the human form
    And the specced subject carries no brief field
    And no step is addressed to a human

  # canon: {"schema":1,"at":"2026-10-11T14:44:50Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:context-capability
  @case:failure
  @context.status.61
  Scenario: An unapproved brief is the only human step and withholds the move-on
    Given a verifying subject whose scenarios are all evidenced and reviewed, and whose newest change's brief.md has no "Approved:" line
    And a building subject that adopts no change
    When canon status runs
    Then the verifying subject gets one step whose actor is human: stop and report the brief, building and attesting nothing for the subject until the human answers, one answer covering brief and policy
    And its reason names the brief.md that has no "Approved:" line, says canon evidence add refuses the subject's scenarios and tasks until then, and names the canon brief approve <change> --approver <the human> --note "<their words>" command, which records the brief and the policy approval in one call
    And the human step is the verifying subject's only step, so it is not suggested to move on
    And the subject with no change has no brief to answer, so it gets no human step and its next step is the agent's
    And the human form prints the step as "human:" and both briefs as not approved, the one with no change as an advisory

  # canon: {"schema":1,"at":"2026-10-11T14:44:50Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:context-capability
  @case:edge
  @context.status.62
  Scenario: With the brief approved and the guard passing, shipping is the agent's own step
    Given a verifying subject whose scenarios are all evidenced and reviewed, and whose change's brief.md carries an "Approved:" line
    When canon status runs
    Then the only step is the agent's canon subject move <id> --to shipped command
    And its reason says to ship it yourself unless the brief says the human ships, and asks nobody
    And following it succeeds, after which status lists nothing next

  # canon: {"schema":1,"at":"2026-10-11T14:44:50Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:context-capability
  @case:failure
  @context.status.63
  Scenario: An approval that is only mentioned, commented out or quoted in code does not approve the brief
    Given building subjects whose change's brief.md holds an empty "Approved:" line, "Approved:" mid-line, an "Approved:" line in a one-line or multi-line HTML comment, or one inside a backtick, tilde, nested or unclosed code fence
    And control subjects whose "Approved:" line follows a comment or fence that has closed
    When canon status runs
    Then each of the first reads as not approved and gets the human step to get the brief approved
    And each control reads as approved

  # canon: {"schema":1,"at":"2026-10-11T14:44:50Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:context-capability
  @case:edge
  @context.status.64
  Scenario: An external change is answered in a canon brief that points at it
    Given canon.yaml configures a superpowers source, then an openspec source rooted at a changes dir, then a second openspec source
    And a building subject's change exists in both openspec sources with an "Approved:" line in each proposal.md and no canon brief
    And a shipped subject's archived external change has a canon brief.md holding a "Source:" line that points at it and the "Approved:" line
    When canon status runs
    Then the building subject's brief is enforced and not approved: the proposal's own "Approved:" line is not read
    And its human step reports the brief in the first openspec source's proposal.md and says to create .canon/changes/<change>/brief.md with a "Source:" line naming it and record the approval there
    And the shipped subject's brief reads as approved from its canon brief.md, naming its source
    And no external proposal.md is edited
    And with plans.sources empty the building subject's change has no brief to answer, while the shipped subject's canon brief still approves

  # canon: {"schema":1,"at":"2026-10-11T15:16:45Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:context-capability
  @case:failure
  @context.status.65
  Scenario: An older change's approval does not cover a newer adopted change
    Given a ship-ready verifying subject whose older change is approved and whose newer adopted change's brief.md has no "Approved:" line
    And a building subject whose older change is approved and whose newer change has no brief
    When canon status runs
    Then each brief reads as not approved and names the newest change
    And the verifying subject gets the human step naming the newest change's brief.md, marked as a pending draft that is not yet part of its scope because no scenario is tagged for it, and is still suggested to ship for its approved scope
    And the building subject's newest change has no brief to answer, so it gets no human step and its brief line is an advisory

  # canon: {"schema":1,"at":"2026-10-11T14:43:36Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:edge
  @context.status.80
  Scenario: Status names the policy level from its level line, else from its sections, else custom
    Given policies whose level line names a level, whose level line is missing or names no known level, and whose sections match no level
    When status is run on each
    Then the level is the level line's level when it names a known one, even if the sections differ
    And without a usable level line it is the level the spec_coverage section matches exactly
    And a section with any other setting, such as a scope, an audit or a non-default review, is custom
    And --json carries the same level under policy.level, null when there is no policy

  # canon: {"schema":1,"at":"2026-10-12T03:41:10Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:failure
  @context.status.66
  Scenario: A proposed subject whose brief is unapproved shows the brief and asks only the human
    Given a proposed subject owning no scenario, whose newest adopted change has a brief.md with no "Approved:" line, in a repo whose .feature files hold no scenario
    When canon status runs
    Then the subject shows its brief as not approved, naming that brief.md, in --json and the human form
    And its only next step is the human's: get the brief approved, one human answer covering brief and policy
    And no step starts the work: neither canon feature new nor canon scenario new is suggested
    And once the brief.md carries an "Approved:" line the brief reads as approved and the feature and scenario steps come back

  # canon: {"schema":1,"at":"2026-10-12T03:41:10Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:edge
  @context.status.67
  Scenario: A proposed subject with no brief to answer keeps its steps, and showing a brief adds no transition check
    Given a proposed subject that adopts no change, and a proposed subject whose newest adopted change has no brief
    When canon status runs
    Then neither carries a brief field or a brief line, and neither gets a human step
    And each still gets its canon scenario new step, after one canon feature new step
    And a proposed subject whose brief is unapproved still moves to specced, with no brief line in its output: status reads that brief, the move to specced does not

  # canon: {"schema":1,"at":"2026-10-12T03:41:10Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:happy
  @context.status.68
  Scenario: No status step names a sync, a format run or a plans import for a corpus the gate can read
    Given a building subject owning no scenario beside one that owns scenarios, and a scenario with no subject tag
    When canon status runs
    Then the empty subject's canon scenario new step gives its reason without naming canon inventory sync
    And the untagged scenario's step is canon status, with a reason to add the @subject tag in its .feature file
    And no step names canon inventory sync, canon format or canon ingest plans

  # canon: {"schema":1,"at":"2026-10-12T03:41:11Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:edge
  @context.status.69
  Scenario: The evidence step names gate promote only when writes stay staged
    Given a building subject with an unevidenced scenario
    When canon status runs in a repo whose canon.yaml says write promote auto
    Then the evidence step's reason says to attest each scenario and does not name canon gate promote
    And in a repo with write promote manual, or with no write section, the same reason ends by naming canon gate promote

  # canon: {"schema":1,"at":"2026-10-12T18:00:06Z","actor":{"agent_id":"018-n2-impl"}}
  @subject:context-capability
  @case:failure
  @context.status.70
  Scenario: A shipped subject whose gate went red is flagged and its step is the reopen
    Given a shipped subject that gained an unattested, unreviewed scenario and an open blocker on its change after it shipped
    When canon status runs
    Then the subject is flagged red, naming the uncovered scenario, the unreviewed scenario and the open blocker
    And its only step is canon subject move with --to verifying and a reason, which parses as printed
    And the reason says to keep the follow-up in its approved change (never canon change new), then scenario, evidence and review, and ship it again
    And once the reopen is followed the flag and the reopen step are gone

  # canon: {"schema":1,"at":"2026-10-21T16:24:33Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:context-capability
  @case:happy
  @context.status.81
  Scenario: A clean brief with advisories left gets the planner's polish step before the human stop
    Given a subject whose unapproved brief has no blockers and advisories left
    When canon status runs or a write returns meta.next
    Then the planner's polish step comes first, naming canon skills show brief-polish, the advisory count, the top rule ids and canon brief check for the change
    And the human stop stays after it in the list, and while blockers remain only the canon brief check step is given

  # canon: {"schema":1,"at":"2026-10-21T16:24:53Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:context-capability
  @case:happy
  @context.status.82
  Scenario: A pending draft with advisories left gets the polish step before its report step
    Given a subject with an approved change and a pending draft whose brief has no blockers and advisories left
    When canon status runs
    Then the first step for the draft is the planner's polish step naming canon skills show brief-polish, the advisory count and rule ids, then canon brief check for it
    And the human report step follows it, and the subject's own steps are not held back

  # canon: {"schema":1,"at":"2026-10-21T16:25:01Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:context-capability
  @case:edge
  @context.status.83
  Scenario: A brief with no advisories keeps the human stop alone, and approval ignores advisories
    Given a brief with no blockers and no advisories, and another with advisories whose subject has a brief approval
    When canon status runs and canon brief approve runs on the brief that has advisories
    Then the clean brief's only step is the human stop, with no polish step
    And the approval is recorded: advisories never refuse it
