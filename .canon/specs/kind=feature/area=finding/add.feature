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
  @case:failure
  @finding.add.03
  Scenario: A fixed finding that names no closing commit is refused
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given an invocation whose disposition is fixed
    When no resolution sha accompanies it
    Then the command exits as a fixable usage error naming both flags
    And nothing is staged

  @subject:finding-lifecycle
  @case:failure
  @finding.add.04
  Scenario: A resolution sha alongside any disposition other than fixed is refused
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a resolution sha on the invocation
    When the disposition is open, rejected or deferred
    Then every one of those combinations is refused rather than quietly dropping the sha
    And the pair holds in both directions, so fixed and a closing commit imply each other

  @subject:finding-lifecycle
  @case:failure
  @finding.add.05
  Scenario: A well-formed sha this repository does not hold is refused by flag name
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a checkout and a forty-hex sha naming no commit in it
    When that value is given to the reviewed, introduced-by or resolution flag
    Then the command refuses and the failure names which flag carried the value
    And a sha the checkout does hold is accepted on each of the three flags

  @subject:finding-lifecycle
  @case:failure
  @finding.add.06
  Scenario: An annotated tag's own object id is not a commit and is refused
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a checkout carrying an annotated tag
    When the tag object's own id is given to any sha-shaped flag
    Then the command refuses it, because the object this repository holds under that id is a tag and not a commit
    And the refusal names the offending flag

  @subject:finding-lifecycle
  @case:edge
  @finding.add.07
  Scenario: Outside a checkout the existence check is skipped rather than failed
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a corpus directory that is not a git work tree
    When a finding is authored citing a sha nothing there can resolve
    Then the record is staged
    And the check is best-effort, so a place where nothing is verifiable refuses nothing

  @subject:finding-lifecycle
  @case:failure
  @finding.add.08
  Scenario: A line separator in any free-text field is refused and never escaped
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given the summary, the reviewer, the file ref and the actor id
    When any one of them carries any character the shared row-break set names
    Then the invocation is refused and stages nothing
    And the set is the one the row grammar already owns, so a separator that forges a second rendered row is refused wherever it appears

  @subject:finding-lifecycle
  @case:happy
  @finding.add.09
  Scenario: A finding is a recorded observation and the command says canon verifies none of it
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given the help an operator reads before trusting a finding
    When it is read
    Then it calls the record an observation and not proof
    And it states that canon never resolves or reads the commits the record cites, never verifies or guesses the introducing commit, and gates nothing here
    And it admits the author and the beneficiary are the same party
    And it still names what the record is good for: attribution, and a count that stops being typed from memory

  # canon: {"schema":1,"at":"2026-10-10T16:02:33Z","actor":{"agent_id":"canon"}}
  @subject:finding-lifecycle
  @case:happy
  @finding.add.10
  Scenario: A defect the author found in its own work is accepted and blocks like a reviewer's
    Given a subject whose evidence the author attested and an independent review
    When the author raises an open blocker against its own change with --reviewer set to itself
    Then the finding is staged and promotes like any other
    And the subject cannot enter verifying while that blocker is open

  # canon: {"schema":1,"at":"2026-10-10T16:02:37Z","actor":{"agent_id":"canon"}}
  @subject:finding-lifecycle
  @case:failure
  @finding.add.11
  Scenario: An introduced-by of self is refused, pointing at the reviewer flag
    Given an author recording a defect it found itself
    When it passes --introduced-by self
    Then the command exits 2, saying --introduced-by names a commit and pointing at --reviewer
    And nothing is staged

  # canon: {"schema":1,"at":"2026-10-10T16:02:37Z","actor":{"agent_id":"canon"}}
  @subject:finding-lifecycle
  @case:failure
  @finding.add.12
  Scenario: A finding session id outside the SessionId grammar is refused
    Given a session id with surrounding whitespace
    When finding add is run with it
    Then the command exits 2 naming --session-id and the SessionId grammar
    And nothing is staged

  # canon: {"schema":1,"at":"2026-10-10T19:28:02Z","actor":{"agent_id":"canon"}}
  @subject:finding-lifecycle
  @case:happy
  @finding.add.30
  Scenario: A short sha is resolved to the full commit it names
    Given a git repository holding a commit
    When canon finding add names it by a short sha on --reviewed-sha, --resolution-sha or --introduced-by
    Then git resolves the short sha in that repository
    And the staged finding stores the full 40-character sha, never the abbreviation

  # canon: {"schema":1,"at":"2026-10-10T19:28:02Z","actor":{"agent_id":"canon"}}
  @subject:finding-lifecycle
  @case:failure
  @finding.add.31
  Scenario: A short sha that names no single commit is refused
    Given a git repository
    When canon finding add names a short sha that matches no commit, more than one commit, or only a tag that peels to a commit
    Then the add is refused with exit 2 naming the flag and the value
    And outside a git work tree a short sha is refused, since nothing can resolve it
    And nothing is staged

  # canon: {"schema":1,"at":"2026-10-21T09:00:00Z","actor":{"agent_id":"canon-impl-0241"}}
  @subject:finding-lifecycle
  @case:happy
  @finding.add.32
  Scenario: A finding names the scenarios it is about with a repeatable --scenario-id
    Given a repository whose index knows two scenarios
    When canon finding add names one of them twice and the other once through --scenario-id, and a second finding names none
    Then the first finding's record carries each scenario id once, in the order given
    And the second finding's record carries no scenario_ids field at all

  # canon: {"schema":1,"at":"2026-10-21T09:00:01Z","actor":{"agent_id":"canon-impl-0241"}}
  @subject:finding-lifecycle
  @case:failure
  @finding.add.33
  Scenario: A --scenario-id the index does not know is refused with its code
    Given a repository whose index does not know the scenario id world.demo.99
    When canon finding add names world.demo.99 through --scenario-id
    Then the command exits 2 with FINDING_UNKNOWN_SCENARIO, the field --scenario-id and a hint, naming the unknown id
    And nothing is staged
