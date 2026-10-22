Feature: context capability
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:context-capability
  @case:edge
  @context.capability.01
  Scenario: The authoring surface answers in full even when the corpus it describes is broken
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a corpus a sibling format check proves carries real violations
    When the capability query is run against that same corpus
    Then it exits zero, because it is a capability question and not a validation one
    And the outline leads with the capability version a consumer reads to detect surface growth
    And every registered record kind, every enum domain, every join key and the per-kind CEL section are all present

  @subject:context-capability
  @case:edge
  @context.capability.02
  Scenario: A repo with no canon state at all still resolves the whole surface
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a directory carrying no manifest, no policy file and no ledger
    When the surface is resolved against it
    Then every registered kind still appears, and the enum domains and join keys are still populated
    And only the policy section degrades, to documented defaults plus a diagnostic
    And nothing about the absence is reported as a failure

  @subject:context-capability
  @case:edge
  @context.capability.03
  Scenario: Two resolutions of an unchanged repo are byte-identical
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repo whose policy declares both a flat requirement and a CEL-routed one
    When the surface is resolved and rendered twice with nothing changed in between
    Then the two renderings are byte-identical
    And the outline is therefore diffable as a record of repo state, not a timestamped report

  @subject:context-capability
  @case:happy
  @context.capability.04
  Scenario: The machine form and the human form name exactly the same surface
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given one resolved authoring surface
    When it is rendered as JSON and as the default outline
    Then every kind, enum domain and join key named in the JSON also appears by name in the outline
    And the JSON flag selects a renderer, never a resolution input

  @subject:context-capability
  @case:happy
  @context.capability.05
  Scenario: Kinds and enum domains come from the one shared registry, never a second hand-kept list
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a fresh independent walk of the schema registry performed alongside the resolution
    When the resolved kinds, their envelope fields, their partition templates and their enum domains are compared against that walk
    Then they are identical, name for name and field for field
    And there is no seam through which a caller could hand the resolver a different registry

  @subject:context-capability
  @case:happy
  @context.capability.06
  Scenario: The CEL binding surface is exactly what a policy expression is checked against
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given the per-kind CEL section of a resolved surface
    When it is compared against a fresh independent bindings walk over the same registry
    Then the CEL section covers the same kind set as the kinds section
    And each kind's referenceable fields and allowlisted functions match that walk
    And an author reading the surface can never be told a field the validator would reject

  @subject:context-capability
  @case:edge
  @context.capability.07
  Scenario: Run from a subdirectory it surfaces the repo root's policy, not a local absence of one
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repo root carrying a manifest and a policy file that requires human trust at p1
    And a nested subdirectory carrying neither
    When the capability query is run from that subdirectory with no explicit repo flag
    Then the nearest ancestor manifest is resolved as the project root
    And the root's own policy loads cleanly and its p1 requirement surfaces, rather than degrading to a default

  @subject:context-capability
  @case:happy
  @context.capability.08
  Scenario: The typed vocabulary index is the vocabulary's own resolved snapshot
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repo carrying a declared vocabulary package
    When the surface is resolved and its vocabulary section compared against a fresh independent snapshot resolution
    Then the directives, enums and evidence kinds are identical
    And the surface projects no second independent view of the vocabulary

  @subject:context-capability
  @case:edge
  @context.capability.09
  Scenario: An absent vocabulary directory resolves an empty index rather than failing
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repo with no vocabulary directory at all
    When the surface is resolved
    Then the vocabulary index holds no directives and no evidence kinds
    And the resolution still returns a surface rather than failing

  @subject:context-capability
  @case:happy
  @context.capability.10
  Scenario: The policy surface exposes resolved effect-aware risk tiers
  # canon: {"schema":1,"at":"2026-10-01T00:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a policy declaring rank, paths, effects, and minimum human approvals
    When the capability surface is resolved as JSON and outline
    Then risk_tiers carries the same resolved values in both forms
    And an absent risk_tiers section is an empty no-op

  # canon: {"schema":1,"at":"2026-10-08T14:02:45Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:failure
  @context.capability.11
  Scenario: A malformed coverage policy reads as invalid, never as not enforced
    Given a policy whose spec_coverage lists a required case that is not a kebab-case slug
    When the capability surface is resolved
    Then spec_coverage reads INVALID and names the offending value
    And the policy is reported unclean rather than as a repository that never opted in

  # canon: {"schema":1,"at":"2026-10-10T15:35:48Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:happy
  @context.capability.12
  Scenario: The outline opens with a pointer to status and leaves the JSON surface unchanged
    Given a repository whose policy enforces spec coverage
    When the capability outline is printed
    Then its first line points to status, followed by a blank line and the capability version
    And the JSON surface carries exactly the same top-level keys and capability version as before

  # canon: {"schema":1,"at":"2026-10-10T15:35:48Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:failure
  @context.capability.13
  Scenario: The outline header warns when no policy makes the gate enforce coverage
    Given a repository with a manifest but no policy file
    When the capability outline is printed
    Then its second line warns that the gate requires no evidence, failure cases or review here
    And the schema outline follows after a blank line

  # canon: {"schema":1,"at":"2026-10-18T15:07:37Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:happy
  @context.capability.14
  Scenario: One concept has one flag spelling across every command
    Given the whole command tree as `canon schema --all --json` and `canon schema <command> --json` show it
    When every visible flag of every command is read
    Then no two flags differ only by an -id suffix
    And change, project, subject, actor, scenario, session and run each appear only as --<name>-id

  # canon: {"schema":1,"at":"2026-10-18T15:07:37Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:happy
  @context.capability.15
  Scenario: A --kind value never carries the _record suffix
    Given the --kind options of query, export and purge
    When their accepted values are listed by `canon schema <command> --json`
    Then every value is a record kind without the _record suffix, so evidence_record is evidence
    And the record's own kind field, the ledger directories and the canon.yaml routing keys keep their stored names

  # canon: {"schema":1,"at":"2026-10-18T15:07:37Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:failure
  @context.capability.16
  Scenario: A removed flag spelling is a usage error whose hint names the flag that replaced it
    Given a repo with a change and a brief
    When `canon brief check --change <slug> --json` runs, the spelling 0.23 took and 0.24 deprecated
    Then it exits 2 with the USAGE error, field `--change`, and the hint `--change was renamed --change-id in 0.24.0; use --change-id`
    And nothing runs, and `--change=<slug>` and `--change` placed before the subcommand are refused the same way
    And `--change-id <slug>` runs, and the JSON envelope carries no meta.deprecated

  # canon: {"schema":1,"at":"2026-10-18T15:07:37Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:failure
  @context.capability.17
  Scenario: A removed --kind value is a usage error whose hint names the value that replaced it
    Given a repo with no records
    When `canon query --kind review_record --json` runs
    Then it exits 2 with the USAGE error and the hint `--kind review_record was renamed --kind review in 0.24.0; use --kind review`
    And query, export and purge refuse every <kind>_record value the same way, and `--kind review` runs

  # canon: {"schema":1,"at":"2026-10-18T15:07:37Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:edge
  @context.capability.18
  Scenario: Help and schema show only the one name, with no deprecated field
    Given the help of a command that renamed a flag, and of query
    When the help and `canon schema <command> --json` are read
    Then they show --change-id and the kinds without the _record suffix, and none of the removed spellings
    And no option carries deprecatedAliases or deprecatedValues

  # canon: {"schema":1,"at":"2026-10-18T15:07:37Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:edge
  @context.capability.19
  Scenario: A token after -- or the value of an option is never read as a removed spelling
    Given a command line that carries a removed spelling only after `--`, or as the value of another option
    When the command runs
    Then it runs, and no usage error names a removed spelling
    And the same spelling as a flag, with a space or an equals sign, is refused

  # canon: {"schema":1,"at":"2026-10-18T15:07:37Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:failure
  @context.capability.20
  Scenario: A removed spelling is refused only on the commands that renamed it
    Given a command that never had the old spelling as a flag
    When it runs with that spelling
    Then it is the ordinary unknown-argument usage error, with no rename hint
    And on a command that renamed it, the refusal carries the rename hint even when the flag lacks its value

  # canon: {"schema":1,"at":"2026-10-18T15:07:37Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:failure
  @context.capability.21
  Scenario: A scenario batch line with a removed key name refuses the batch and names the key to use
    Given an NDJSON file for `canon scenario new --from` whose line uses the key subject, change or project
    When the batch runs
    Then it exits 2 and writes nothing
    And the line's error names the key that replaced it, as `subject-id`, in its hint
    And a file using subject-id, change-id and project-id is written as before

  # canon: {"schema":1,"at":"2026-10-18T15:07:37Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:failure
  @context.capability.22
  Scenario: A checker over the command tree reports a concept spelled two ways
    Given a list of commands whose flags include both --change and --change-id
    When the one-name checker runs over it
    Then it reports the split by the two spellings and the commands that carry each
    And a concept written without its -id suffix and a --kind value ending in _record are each reported

  # canon: {"schema":1,"at":"2026-10-18T15:07:37Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:happy
  @context.capability.23
  Scenario: Gate promote tallies the staged kinds by the names --kind takes
    Given a staged evidence record and a staged finding
    When `canon gate promote` runs
    Then its tally reads `evidence=1, finding=1`, the names `canon query --kind` takes
    And with nothing staged, the line that lists the promotable kinds uses the same names

  # canon: {"schema":1,"at":"2026-10-22T04:48:34Z","actor":{"agent_id":"canon-impl-027"}}
  @subject:context-capability
  @case:edge
  @context.capability.24
  Scenario: A command that needs canon state refuses without a canon.yaml, and the ones that run without it still do
    Given a directory with no canon.yaml in it or above it
    When a command that reads or records repository state runs there, such as status, query, finding list, report or subject new
    Then it exits 2 with CONFIG_MISSING and writes nothing
    And init, schema, context, regime-key and retrieve still run there, and init creates the canon.yaml the others look for

  # canon: {"schema":1,"at":"2026-10-22T04:48:48Z","actor":{"agent_id":"canon-impl-027"}}
  @subject:context-capability
  @case:edge
  @context.capability.25
  Scenario: The schema reports a --repo default of the nearest canon.yaml, not the literal dot
    Given a command whose --repo defaults to the dot
    When canon schema <command> --json lists its options
    Then the default of --repo reads as the nearest ancestor directory with a canon.yaml
    And it is no longer reported as the literal dot, which is not what the command does

  # canon: {"schema":1,"at":"2026-10-22T04:49:11Z","actor":{"agent_id":"canon-impl-027"}}
  @subject:context-capability
  @case:edge
  @context.capability.26
  Scenario: A command declared read leaves the repo tree as it found it, or names the cache it writes
    Given a scratch repository holding a subject, a change, a scenario, evidence, a review and a finding, with a scenario index that is out of date
    When every command the schema declares effect read runs in it, with its example and the other reads that used to write
    Then the file tree, ignored files included, and git status are the same afterwards
    And the only exception is a path the command's effectNote names, which is gitignored and never committed
