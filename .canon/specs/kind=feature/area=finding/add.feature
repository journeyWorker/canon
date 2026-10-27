Feature: finding add
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:finding-lifecycle
  @case:failure
  @finding.add.01
  Scenario: The round and the seq are both 1-based and a zero is refused
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given an otherwise well-formed finding
    When it is authored with round zero, or with seq zero
    Then each invocation is refused as a usage error
    And nothing is staged, because a zero would sort ahead of every finding a round really has

  @subject:finding-lifecycle
  @case:failure
  @finding.add.02
  Scenario: A second finding at an occupied change round and seq is refused naming the occupant
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a finding already staged at one change, round and seq
    When a different finding is authored at that same triple
    Then the command refuses and names the key that is already taken
    And the staging area still holds exactly the one original record
    And seq is never auto-assigned, so the author transcribes the number the review artifact already gave it

  @subject:finding-lifecycle
  @case:happy
  @finding.add.03
  Scenario: A fixed finding that names no closing commit records HEAD as its resolution
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given an invocation whose disposition is fixed
    When no --resolution-rev accompanies it
    Then the finding is staged with the full sha HEAD resolves to as its resolution

  @subject:finding-lifecycle
  @case:failure
  @finding.add.04
  Scenario: A resolution revision alongside any disposition other than fixed is refused
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a --resolution-rev on the invocation
    When the disposition is open, rejected or deferred
    Then every one of those combinations is refused with exit 2 rather than quietly dropping the revision
    And only a fixed finding carries a resolution, and a fixed finding always does

  @subject:finding-lifecycle
  @case:failure
  @finding.add.05
  Scenario: A revision this repository does not hold is refused on every revision option
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a checkout, a forty-hex sha naming no commit in it, and a branch name that does not exist
    When either is given to --rev, --introduced-rev or --resolution-rev
    Then the command exits 2 and stages nothing
    And a revision the checkout does hold is accepted on each of the three options in any spelling git resolves, and stored as the full sha

  @subject:finding-lifecycle
  @case:edge
  @finding.add.06
  Scenario: An annotated tag resolves to the commit it names
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a checkout carrying an annotated tag
    When the tag's name, or the tag object's own id, is given to --rev and --introduced-rev
    Then the record stores the full sha of the commit the tag points at
    And never the tag object's own id

  @subject:finding-lifecycle
  @case:failure
  @finding.add.07
  Scenario: Outside a checkout there is no commit to bind and the add is refused
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a corpus directory that is not a git work tree
    When a finding is authored
    Then the command exits 2
    And nothing is staged, because no commit can be named there

  @subject:finding-lifecycle
  @case:failure
  @finding.add.08
  Scenario: A line separator in any free-text field is refused and never escaped
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given the summary, the file ref and the actor id, which names the reviewer
    When any one of them carries any character the shared row-break set names
    Then the invocation is refused and stages nothing
    And the set is the one the row grammar already owns, so a separator that forges a second rendered row is refused wherever it appears

  @subject:finding-lifecycle
  @case:happy
  @finding.add.09
  Scenario: A finding is a recorded observation and its text is never checked against the code
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a finding whose summary claims a defect and whose file ref names a path that does not exist in the tree
    When it is added
    Then the add succeeds and the record carries the summary and the file ref exactly as given
    And the record names who raised it, in which round, against which change, at which severity and disposition
    And no introducing commit is recorded, because none was sourced and none is guessed

  # canon: {"schema":1,"at":"2026-10-10T16:02:33Z","actor":{"agent_id":"canon"}}
  @subject:finding-lifecycle
  @case:happy
  @finding.add.10
  Scenario: A defect the author found in its own work is accepted and blocks like a reviewer's
    Given a subject whose evidence the author attested and an independent review
    When the author raises an open blocker against its own change, naming itself as the actor
    Then the finding is staged and promotes like any other
    And the subject cannot enter verifying while that blocker is open

  # canon: {"schema":1,"at":"2026-10-10T16:02:37Z","actor":{"agent_id":"canon"}}
  @subject:finding-lifecycle
  @case:failure
  @finding.add.11
  Scenario: An introduced-rev of self is refused
    Given an author recording a defect it found itself
    When it passes --introduced-rev self
    Then the command exits 2, because --introduced-rev names a commit and the reviewer is the actor id
    And nothing is staged

  # canon: {"schema":1,"at":"2026-10-10T16:02:37Z","actor":{"agent_id":"canon"}}
  @subject:finding-lifecycle
  @case:failure
  @finding.add.12
  Scenario: A finding session id outside the SessionId grammar is refused
    Given a session id with surrounding whitespace
    When finding add is run with it
    Then the command exits 2 naming --session and the SessionId grammar
    And nothing is staged

  # canon: {"schema":1,"at":"2026-10-10T19:28:02Z","actor":{"agent_id":"canon"}}
  @subject:finding-lifecycle
  @case:happy
  @finding.add.30
  Scenario: A short revision is resolved to the full commit it names
    Given a git repository holding a commit
    When canon finding add names it by a short sha on --rev or --introduced-rev, or canon finding close does on --resolution-rev
    Then git resolves the short sha in that repository
    And the staged finding stores the full 40-character sha, never the abbreviation

  # canon: {"schema":1,"at":"2026-10-10T19:28:02Z","actor":{"agent_id":"canon"}}
  @subject:finding-lifecycle
  @case:failure
  @finding.add.31
  Scenario: A short revision that names no commit is refused
    Given a git repository
    When canon finding add names a short revision that matches no commit on --rev, --introduced-rev or --resolution-rev
    Then the add is refused with exit 2 and NO_COMMIT, naming the value
    And nothing is staged

  # canon: {"schema":1,"at":"2026-10-21T09:00:00Z","actor":{"agent_id":"canon-impl-0241"}}
  @subject:finding-lifecycle
  @case:happy
  @finding.add.32
  Scenario: A finding names the scenarios it is about with a repeatable --scenario
    Given a repository whose index knows two scenarios
    When canon finding add names one of them twice and the other once through --scenario, and a second finding names none
    Then the first finding's record carries each scenario id once, in the order given
    And the second finding's record carries no scenario_ids field at all

  # canon: {"schema":1,"at":"2026-10-21T09:00:01Z","actor":{"agent_id":"canon-impl-0241"}}
  @subject:finding-lifecycle
  @case:failure
  @finding.add.33
  Scenario: A --scenario the index does not know is refused with its code
    Given a repository whose index does not know the scenario id world.demo.99
    When canon finding add names world.demo.99 through --scenario
    Then the command exits 2 with FINDING_UNKNOWN_SCENARIO, the field --scenario and a hint, naming the unknown id
    And nothing is staged

  # canon: {"schema":1,"at":"2026-10-23T12:05:39Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:finding-lifecycle
  @case:happy
  @finding.add.34
  Scenario: One finding add document records one finding and an array records one per element
    Given a git work tree
    When finding add is given its arguments as one JSON object with --input, then as a JSON array
    Then the object returns the ordinary report and records one finding
    And the array returns a batch report with one item per element, in order, each written

  # canon: {"schema":1,"at":"2026-10-23T12:05:44Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:finding-lifecycle
  @case:failure
  @finding.add.35
  Scenario: A batch with one invalid element writes nothing and points at the element
    Given a batch of two finding documents whose second carries a severity outside the closed set, or lacks a required key
    When finding add is run on it with --input
    Then it exits 2 and the error's field points at that element and key, such as /items/1/severity
    And nothing is staged or committed, not even the valid first element

  # canon: {"schema":1,"at":"2026-10-23T12:05:48Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:finding-lifecycle
  @case:edge
  @finding.add.36
  Scenario: Repeating a finding add that already landed writes nothing and is reported unchanged
    Given a finding add that already landed, staged or committed
    When the identical add is repeated
    Then it exits 0 and the report says unchanged
    And nothing is written, and a different finding at the same round and seq is still refused

  # canon: {"schema":1,"at":"2026-10-23T12:05:53Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:finding-lifecycle
  @case:happy
  @finding.add.37
  Scenario: A finding names the commit it reviewed, HEAD by default, with the code tree it saw
    Given a finding add that names no revision, in a git work tree
    When it is invoked
    Then the record names the full sha HEAD resolves to as the commit it reviewed
    And it carries the code tree it saw and a worktree-dirty flag, and no introduced or resolution commit

  # canon: {"schema":1,"at":"2026-10-26T18:12:50Z","actor":{"agent_id":"impl-030-rl","role":"implementer","session_id":"rl-2026-10-26"}}
  @subject:finding-lifecycle
  @case:happy
  @flow:canon-0-30-record-links.F1
  @finding.add.38
  Scenario: A finding added with a review id points at that review and a query shows the id
    Given a review written by canon review scenario and the review id it printed
    When canon finding add is run with that id as --review and the finding is promoted
    Then the committed finding carries `review_id` with that value
    And canon query --kind finding --json shows the review id on the record
    And a finding added without --review has no `review_id` field

  # canon: {"schema":1,"at":"2026-10-26T18:12:54Z","actor":{"agent_id":"impl-030-rl","role":"implementer","session_id":"rl-2026-10-26"}}
  @subject:finding-lifecycle
  @case:failure
  @flow:canon-0-30-record-links.F1
  @finding.add.39
  Scenario: A review id that no review has is refused and the ledger is unchanged
    Given a ledger and an archive in which no review carries the id given
    When canon finding add is run with that id as --review
    Then it is refused with FINDING_REVIEW_NOT_FOUND, exit 2, naming the id
    And nothing is staged and the ledger, the staging area and the archive are as they were
    And a value that is not a ULID is refused as an invalid argument before the ledger is read

  # canon: {"schema":1,"at":"2026-10-26T18:12:57Z","actor":{"agent_id":"impl-030-rl","role":"implementer","session_id":"rl-2026-10-26"}}
  @subject:finding-lifecycle
  @case:edge
  @flow:canon-0-30-record-links.F1
  @finding.add.40
  Scenario: The id of a review that moved to the archive is still accepted
    Given a review that a later review of the same scenario by the same reviewer moved to the archive
    When canon finding add is run with the earlier review's id
    Then the finding is recorded and carries that review id

  # canon: {"schema":1,"at":"2026-10-26T18:13:02Z","actor":{"agent_id":"impl-030-rl","role":"implementer","session_id":"rl-2026-10-26"}}
  @subject:finding-lifecycle
  @case:failure
  @flow:canon-0-30-record-links.F2
  @finding.add.41
  Scenario: A new blocker or should-fix with no introducing commit and no unknown mark is refused
    Given a change under review
    When canon finding add records a blocker and then a should-fix with neither --introduced-rev nor --introduced-unknown
    Then each is refused with FINDING_INTRODUCED_REQUIRED, exit 2, and the message names both options
    And nothing is staged

  # canon: {"schema":1,"at":"2026-10-26T18:13:04Z","actor":{"agent_id":"impl-030-rl","role":"implementer","session_id":"rl-2026-10-26"}}
  @subject:finding-lifecycle
  @case:happy
  @flow:canon-0-30-record-links.F2
  @finding.add.42
  Scenario: A new defect finding is recorded with an introducing commit or with the unknown mark, and a note needs neither
    Given a change under review
    When canon finding add records a blocker with --introduced-rev, a should-fix with --introduced-unknown and a note with neither
    Then the blocker carries `introduced_by` and no `introduced_unknown`
    And the should-fix carries `introduced_unknown: true` and no `introduced_by`
    And the note carries neither and is recorded

  # canon: {"schema":1,"at":"2026-10-26T18:13:06Z","actor":{"agent_id":"impl-030-rl","role":"implementer","session_id":"rl-2026-10-26"}}
  @subject:finding-lifecycle
  @case:failure
  @flow:canon-0-30-record-links.F2
  @finding.add.43
  Scenario: An introducing commit together with the unknown mark is refused
    Given a change under review
    When canon finding add is given --introduced-rev together with --introduced-unknown
    Then it is refused with FINDING_INVALID_ARGUMENT, exit 2, naming the two options as exclusive
    And nothing is staged
