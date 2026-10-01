Feature: context capability
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:context-capability
  @context.capability.01
  Scenario: The authoring surface answers in full even when the corpus it describes is broken
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a corpus a sibling format check proves carries real violations
    When the capability query is run against that same corpus
    Then it exits zero, because it is a capability question and not a validation one
    And the outline leads with the capability version a consumer reads to detect surface growth
    And every registered record kind, every enum domain, every join key and the per-kind CEL section are all present

  @subject:context-capability
  @context.capability.02
  Scenario: A repo with no canon state at all still resolves the whole surface
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a directory carrying no manifest, no policy file and no ledger
    When the surface is resolved against it
    Then every registered kind still appears, and the enum domains and join keys are still populated
    And only the policy section degrades, to documented defaults plus a diagnostic
    And nothing about the absence is reported as a failure

  @subject:context-capability
  @context.capability.03
  Scenario: Two resolutions of an unchanged repo are byte-identical
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repo whose policy declares both a flat requirement and a CEL-routed one
    When the surface is resolved and rendered twice with nothing changed in between
    Then the two renderings are byte-identical
    And the outline is therefore diffable as a record of repo state, not a timestamped report

  @subject:context-capability
  @context.capability.04
  Scenario: The machine form and the human form name exactly the same surface
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given one resolved authoring surface
    When it is rendered as JSON and as the default outline
    Then every kind, enum domain and join key named in the JSON also appears by name in the outline
    And the JSON flag selects a renderer, never a resolution input

  @subject:context-capability
  @context.capability.05
  Scenario: Kinds and enum domains come from the one shared registry, never a second hand-kept list
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a fresh independent walk of the schema registry performed alongside the resolution
    When the resolved kinds, their envelope fields, their partition templates and their enum domains are compared against that walk
    Then they are identical, name for name and field for field
    And there is no seam through which a caller could hand the resolver a different registry

  @subject:context-capability
  @context.capability.06
  Scenario: The CEL binding surface is exactly what a policy expression is checked against
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given the per-kind CEL section of a resolved surface
    When it is compared against a fresh independent bindings walk over the same registry
    Then the CEL section covers the same kind set as the kinds section
    And each kind's referenceable fields and allowlisted functions match that walk
    And an author reading the surface can never be told a field the validator would reject

  @subject:context-capability
  @context.capability.07
  Scenario: Run from a subdirectory it surfaces the repo root's policy, not a local absence of one
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repo root carrying a manifest and a policy file that requires human trust at p1
    And a nested subdirectory carrying neither
    When the capability query is run from that subdirectory with no explicit repo flag
    Then the nearest ancestor manifest is resolved as the project root
    And the root's own policy loads cleanly and its p1 requirement surfaces, rather than degrading to a default

  @subject:context-capability
  @context.capability.08
  Scenario: The typed vocabulary index is the vocabulary's own resolved snapshot
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repo carrying a declared vocabulary package
    When the surface is resolved and its vocabulary section compared against a fresh independent snapshot resolution
    Then the directives, enums and evidence kinds are identical
    And the surface projects no second independent view of the vocabulary

  @subject:context-capability
  @context.capability.09
  Scenario: An absent vocabulary directory resolves an empty index rather than failing
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repo with no vocabulary directory at all
    When the surface is resolved
    Then the vocabulary index holds no directives and no evidence kinds
    And the resolution still returns a surface rather than failing

  @subject:context-capability
  @context.capability.10
  Scenario: The policy surface exposes resolved effect-aware risk tiers
  # canon: {"schema":1,"at":"2026-10-01T00:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a policy declaring rank, paths, effects, and minimum human approvals
    When the capability surface is resolved as JSON and outline
    Then risk_tiers carries the same resolved values in both forms
    And an absent risk_tiers section is an empty no-op
