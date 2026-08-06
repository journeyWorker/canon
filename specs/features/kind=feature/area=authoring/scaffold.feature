Feature: authoring scaffold
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:authoring-scaffold
  @authoring.scaffold.01
  Scenario: The tag alone determines where a scenario is written
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository with one spec root and no feature file yet
    When a scenario is created from its tag with no path given
    Then the file appears at the partitioned path the tag's area and surface derive
    And its Feature heading is the area and surface, and the tag is inside it
    And that path is the same one the feature scaffolding command would have produced

  @subject:authoring-scaffold
  @authoring.scaffold.02
  Scenario: Scaffolding a feature that already exists is refused and the existing bytes survive
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a feature file already scaffolded at its derived path
    When the same area and surface is scaffolded again under a different title
    Then the command exits nonzero and says the file already exists
    And the file on disk is byte-for-byte what it was before
    And the create is atomic, so no window exists where the new title half-landed

  @subject:authoring-scaffold
  @authoring.scaffold.03
  Scenario: A scenario tag that is already present is refused rather than duplicated
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a feature file already carrying a scenario at some tag
    When that same tag is created again under a different title
    Then the command exits nonzero and names the tag
    And the feature file is byte-for-byte unchanged
    And the tag still appears exactly once in it

  @subject:authoring-scaffold
  @authoring.scaffold.04
  Scenario: A repository with a single spec root never has to name it
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given exactly one configured spec root
    When a feature is scaffolded with no project selector
    Then the lone root is taken and the file is written under it
    And that is a resolution, not a default, because there is nothing else it could mean

  @subject:authoring-scaffold
  @authoring.scaffold.05
  Scenario: Naming a project selects that root and leaves every other root alone
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given two configured spec roots
    When a feature is scaffolded naming the second by its configured id
    Then the file is written under the second root
    And the first root's directory is not so much as created
    And naming a root by id is valid even when it is the only one, so a script never has to branch

  @subject:authoring-scaffold
  @authoring.scaffold.06
  Scenario: A project id no configured root carries is refused with nothing written
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given two configured spec roots
    When a feature is scaffolded naming an id neither of them carries
    Then the command exits refusing, and the refusal lists the ids that do exist
    And neither root's directory is created
    And the id is never guessed at by nearest match

  @subject:authoring-scaffold
  @authoring.scaffold.07
  Scenario: Several configured roots with no selector is ambiguous and refuses rather than guessing
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given two configured spec roots
    When a feature is scaffolded with no project selector
    Then the command refuses, saying how many roots are configured and listing their ids
    And it names the selector that would resolve the ambiguity
    And it names the command that refused, so the message is actionable as written

  @subject:authoring-scaffold
  @authoring.scaffold.08
  Scenario: A fresh feature stub is not yet a valid corpus entry and the command says what closes that
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a freshly scaffolded feature with no scenario in it
    When the scaffolding command reports success
    Then it prints the next-step invocation, already carrying the first derived tag for that surface
    And checking the corpus still fails, calling the file an empty feature stub not yet a valid corpus entry
    And running the printed invocation is what turns that stub clean
