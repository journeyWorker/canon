Feature: finding add
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:finding-lifecycle
  @finding.add.01
  Scenario: The round and the seq are both 1-based and a zero is refused
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given an otherwise well-formed finding
    When it is authored with round zero, or with seq zero
    Then each invocation is refused as a usage error
    And nothing is staged, because a zero would sort ahead of every finding a round really has

  @subject:finding-lifecycle
  @finding.add.02
  Scenario: A second finding at an occupied change round and seq is refused naming the occupant
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a finding already staged at one change, round and seq
    When a different finding is authored at that same triple
    Then the command refuses and names the key that is already taken
    And the staging area still holds exactly the one original record
    And seq is never auto-assigned, so the author transcribes the number the review artifact already gave it

  @subject:finding-lifecycle
  @finding.add.03
  Scenario: A fixed finding that names no closing commit is refused
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given an invocation whose disposition is fixed
    When no resolution sha accompanies it
    Then the command exits as a fixable usage error naming both flags
    And nothing is staged

  @subject:finding-lifecycle
  @finding.add.04
  Scenario: A resolution sha alongside any disposition other than fixed is refused
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a resolution sha on the invocation
    When the disposition is open, rejected or deferred
    Then every one of those combinations is refused rather than quietly dropping the sha
    And the pair holds in both directions, so fixed and a closing commit imply each other

  @subject:finding-lifecycle
  @finding.add.05
  Scenario: A well-formed sha this repository does not hold is refused by flag name
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a checkout and a forty-hex sha naming no commit in it
    When that value is given to the reviewed, introduced-by or resolution flag
    Then the command refuses and the failure names which flag carried the value
    And a sha the checkout does hold is accepted on each of the three flags

  @subject:finding-lifecycle
  @finding.add.06
  Scenario: An annotated tag's own object id is not a commit and is refused
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a checkout carrying an annotated tag
    When the tag object's own id is given to any sha-shaped flag
    Then the command refuses it, because the object this repository holds under that id is a tag and not a commit
    And the refusal names the offending flag

  @subject:finding-lifecycle
  @finding.add.07
  Scenario: Outside a checkout the existence check is skipped rather than failed
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a corpus directory that is not a git work tree
    When a finding is authored citing a sha nothing there can resolve
    Then the record is staged
    And the check is best-effort, so a place where nothing is verifiable refuses nothing

  @subject:finding-lifecycle
  @finding.add.08
  Scenario: A line separator in any free-text field is refused and never escaped
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given the summary, the reviewer, the file ref and the actor id
    When any one of them carries any character the shared row-break set names
    Then the invocation is refused and stages nothing
    And the set is the one the row grammar already owns, so a separator that forges a second rendered row is refused wherever it appears

  @subject:finding-lifecycle
  @finding.add.09
  Scenario: A finding is a recorded observation and the command says canon verifies none of it
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given the help an operator reads before trusting a finding
    When it is read
    Then it calls the record an observation and not proof
    And it states that canon never resolves or reads the commits the record cites, never verifies or guesses the introducing commit, and gates nothing here
    And it admits the author and the beneficiary are the same party
    And it still names what the record is good for: attribution, and a count that stops being typed from memory
