Feature: vocab typed
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:vocab-typed
  @vocab.typed.01
  Scenario: A plugin declares its directives and enums and both resolve into the active vocabulary
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a plugin manifest exporting a directives directory and an enums file
    And a directive declaring one required typed attribute
    And an enum declaring its members
    When the plugin is loaded
    Then the manifest's declared id is what identifies it, not its directory name
    And the directive is in the loaded index
    And the enum resolves to exactly the members declared, in the order declared

  @subject:vocab-typed
  @vocab.typed.02
  Scenario: The evidence-kind domain is the policy's own required-trust keys and nothing else
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a policy declaring which evidence kinds require which trust level
    When the evidence-kind domain is resolved for that project
    Then the domain is exactly the kinds the policy names
    And a project with no policy resolves to an empty domain rather than failing
    And so the vocabulary's evidence requirement is derived from policy, never restated beside it

  @subject:vocab-typed
  @vocab.typed.03
  Scenario: This repository's own pilot atom declares an evidence kind its real policy admits
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given this repository's committed policy and vocabulary, resolved together
    When the pilot task atom's declared evidence kind is looked for in the resolved domain
    Then it is there
    And so the requirement is a real policy-derived binding, not merely a string that parses

  @subject:vocab-typed
  @vocab.typed.04
  Scenario: A directive nobody declared is refused
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a resolved vocabulary declaring a task directive
    When an atom is written against a directive name the vocabulary does not declare
    Then exactly one diagnostic is raised, coded as an unknown directive
    And no attempt is made to check its attributes, because there is no declaration to check them against

  @subject:vocab-typed
  @vocab.typed.05
  Scenario: An enum value outside its declared members is refused and the message lists what is allowed
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a task directive whose status attribute is typed by a declared enum
    When an atom sets that status to a value the enum does not declare
    Then a bad-enum diagnostic is raised
    And the message names the offending value, the attribute, the directive, and every accepted member
    And an author reading only the refusal learns the whole accepted domain

  @subject:vocab-typed
  @vocab.typed.06
  Scenario: An evidence kind outside the policy-derived domain is refused the same way an enum value is
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a task directive with a required evidence attribute
    When an atom declares an evidence kind the resolved policy domain does not carry
    Then a bad-evidence-kind diagnostic is raised
    And the message lists the kinds the policy does admit
    And the required attribute being present is not enough, because presence is not membership

  @subject:vocab-typed
  @vocab.typed.07
  Scenario: A well-formed typed task atom compiles into a task record carrying its evidence
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given an atom naming a declared task directive with every required attribute set to a declared value
    When it is compiled
    Then a task record is produced under the atom's own id
    And its title is the atom's description and its status is the declared enum member
    And it carries an evidence note, because the evidence attribute was required and satisfied

  @subject:vocab-typed
  @vocab.typed.08
  Scenario: An atom that fails the vocabulary produces diagnostics and no record at all
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given an otherwise valid task atom whose status is not a declared enum member
    When it is compiled
    Then compilation yields diagnostics instead of a record
    And a bad-enum diagnostic is among them
    And nothing partial is emitted, so an invalid atom can never become a half-written task

  @subject:vocab-typed
  @vocab.typed.09
  Scenario: An unrecognized field inside an evidence value is refused, never carried through unchecked
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a task atom whose evidence value carries a valid kind and ref plus one extra field
    When it is checked
    Then an unknown-attribute diagnostic names the extra field and lists the fields evidence accepts
    And compiling the same atom yields that diagnostic and no record
    And so an unchecked nested field can never reach the written task
