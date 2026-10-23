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
    And the JSON surface carries the same sections under the same capability version, the commands and families included, and the header adds no field

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
    Given the whole command tree as `canon schema --json` and `canon schema <command> --json` show it
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
    When `canon scenario new demo.login.01 --title A --change add-login --json` runs, the spelling 0.23 took and 0.24 deprecated
    Then it exits 2 with the USAGE error, field `--change`, `expected` listing `--change-id`, and a hint
    And nothing runs, and `--change=<slug>` and a `--change` missing its value are refused the same way
    And `--change-id <slug>` runs, and the JSON envelope carries no meta.deprecated

  # canon: {"schema":1,"at":"2026-10-18T15:07:37Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:failure
  @context.capability.17
  Scenario: A removed --kind value is a usage error whose hint names the value that replaced it
    Given a repo with no records
    When `canon query --kind review_record --json` runs
    Then it exits 2 with the USAGE error, field `--kind`, and `expected` naming `--kind review`
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
  Scenario: A scenario batch element with a removed key name refuses the batch and names the key to use
    Given a JSON array for `canon scenario new --input` whose second element uses the key subject, change or project
    When the batch runs
    Then it exits 2 with INPUT_UNKNOWN_FIELD and writes nothing
    And the error's field points at the element and the key, as `/items/1/subject`, and `expected` lists the key that replaced it, `subjectId`
    And an array using subjectId, projectId and changeId is written as before

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
    When a command that reads or records repository state runs there, such as status, query, finding list, report build or subject new
    Then it exits 2 with CONFIG_MISSING and writes nothing
    And init, schema, context, regime show and retrieve still run there, and init creates the canon.yaml the others look for

  # canon: {"schema":1,"at":"2026-10-22T04:48:48Z","actor":{"agent_id":"canon-impl-027"}}
  @subject:context-capability
  @case:edge
  @context.capability.25
  Scenario: The schema reports no --repo default, because an absent --repo means the nearest canon.yaml, not the literal dot
    Given a command that acts on a repository
    When canon schema <command> --json lists its options
    Then --repo has no default, and its help says an absent --repo is the nearest ancestor directory with a canon.yaml
    And init is the one exception: it sets up the directory it is given, which defaults to the current one

  # canon: {"schema":1,"at":"2026-10-22T04:49:11Z","actor":{"agent_id":"canon-impl-027"}}
  @subject:context-capability
  @case:edge
  @context.capability.26
  Scenario: A command declared read leaves the repo tree as it found it, or names the cache it writes
    Given a scratch repository holding a subject, a change, a scenario, evidence, a review and a finding, with a scenario index that is out of date
    When every command the schema declares effect read runs in it, with its example and the other reads that used to write
    Then the file tree, ignored files included, and git status are the same afterwards
    And the only exception is a path the command's effectNote names, which is gitignored and never committed

  # canon: {"schema":1,"at":"2026-10-23T11:48:18Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:context-capability
  @case:happy
  @context.capability.27
  Scenario: The schema of a command is the node of the checked-in command spec
    Given the command spec checked in at docs/cli/canon.cli.json
    When canon schema is run for every leaf command of that spec
    Then each answer's data is the spec's node for that command, field for field
    And the bare listing names exactly the spec's leaf commands and no other

  # canon: {"schema":1,"at":"2026-10-23T11:48:38Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:context-capability
  @case:happy
  @context.capability.28
  Scenario: The drift check compares the checked-in spec with the spec the CLI generates, byte for byte
    Given the checked-in command spec and the spec the CLI generates
    When the two are compared as text
    Then they are identical on the unmodified tree
    And the generated spec is the same text on every generation

  # canon: {"schema":1,"at":"2026-10-23T11:48:50Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:context-capability
  @case:happy
  @context.capability.29
  Scenario: A success is the envelope ok, data and meta
    Given a repository set up with canon init
    When status, schema, context and skills show run with --json
    Then each exits 0 and prints one document whose only keys are ok, data and meta, with ok true
    And meta names the command, a major.minor schema version, the canon version and the repository context

  # canon: {"schema":1,"at":"2026-10-23T11:49:05Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:context-capability
  @case:failure
  @context.capability.30
  Scenario: A failure is an error object with a code, a message, a hint and a retryable flag
    Given a repository with a change whose brief is still the template
    When a command is refused, given a usage error, or finds blockers, each with --json
    Then the document is ok false with an error carrying a registered code, a message, a hint and retryable
    And a usage error names the argument it refused in field, and its hint holds a call that works
    And a refusal carries no data, while a failing check keeps its report in data
    And canon schema --error explains each code that was printed

  # canon: {"schema":1,"at":"2026-10-23T11:49:15Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:context-capability
  @case:edge
  @context.capability.31
  Scenario: In JSON mode a warning is carried in meta.warnings and stderr stays empty
    Given a repository and a role value no policy or default names
    When a record is written with that role and --json, and again without --json
    Then with --json the command exits 0, stderr is empty and the warning is in meta.warnings
    And without --json the same warning is on stderr

  # canon: {"schema":1,"at":"2026-10-23T11:49:29Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:context-capability
  @case:failure
  @context.capability.32
  Scenario: In JSON mode a failure leaves stderr empty, even one clap or the removed-spelling table refused
    Given a usage error clap raises, a removed spelling, an unknown command, a refused read, a missing repo flag target and a directory with no canon.yaml
    When each runs with --json, wherever the flag sits in the argv
    Then each exits 2 with stderr empty and one document on stdout that is ok false with an error code
    And the same refusal without --json goes to stderr and leaves stdout empty

  # canon: {"schema":1,"at":"2026-10-23T11:49:47Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:context-capability
  @case:happy
  @context.capability.33
  Scenario: Exit 0 is ok, exit 1 is the command's finding and exit 2 is a refusal
    Given a repository holding a change whose brief has blockers, and a corpus with a format violation
    When status, format, brief check, gate check and a refused call are run
    Then status exits 0, and format, brief check and a gate check that examined nothing exit 1 with their finding in data
    And a usage error or an unknown change exits 2
    And the exit table of the spec lists exactly 0, 1 and 2

  # canon: {"schema":1,"at":"2026-10-23T11:49:58Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:context-capability
  @case:failure
  @context.capability.34
  Scenario: A usage error is exit 2 and its hint is a call that parses
    Given a repository holding a subject and a change
    When commands are run missing a required argument, with an unknown option, or with no operand
    Then each exits 2 with the USAGE error and a hint containing a working call
    And the call in the hint, followed by --help, parses and exits 0

  # canon: {"schema":1,"at":"2026-10-23T11:50:10Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:context-capability
  @case:failure
  @context.capability.35
  Scenario: A removed leaf is a usage error whose hint names the leaf that replaced it
    Given the 0.27 leaves evidence vault, gate task, subject status, learn promote, review add, regime-key and divergence status
    When each is run with --json
    Then each exits 2 with the USAGE error, the removed spelling in field and the 0.28 spelling in expected
    And the hint names the leaf that replaced it, and that leaf parses as written
    And nothing was run, so the repository is unchanged

  # canon: {"schema":1,"at":"2026-10-23T11:50:24Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:context-capability
  @case:failure
  @context.capability.36
  Scenario: A removed option is a usage error whose hint names the option that replaced it
    Given the 0.27 options format --check, query --canon-yaml, retrieve -k, init --check-config, evidence add --kind audit and tier age --dry-run
    When each is run with --json
    Then each exits 2 with the USAGE error and the removed option in field
    And the hint names the option that replaced it, which the command takes
    And nothing was run, so the repository is unchanged

  # canon: {"schema":1,"at":"2026-10-23T11:50:40Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:context-capability
  @case:failure
  @context.capability.37
  Scenario: A bare group is a usage error whose hint names the leaf to run
    Given canon brief and canon report, which ran bare in 0.27, and a group that never did
    When each is run bare with --json
    Then each exits 2 with the USAGE error and stderr empty
    And the hint of brief and report names the leaf to run, canon brief show and canon report build, and that leaf parses as written
    And the other group is still the ordinary usage error with a hint

  # canon: {"schema":1,"at":"2026-10-23T11:51:05Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:context-capability
  @case:happy
  @context.capability.38
  Scenario: Context lists every command, filtered to the role that asks
    Given the generated command spec and a repository with no canon.yaml
    When canon context is run with no role, and again as each of planner, implementer, reviewer and orchestrator
    Then the commands section lists every leaf of the spec, each with its effect and roles
    And each role's run lists exactly the leaves that role may use

  # canon: {"schema":1,"at":"2026-10-23T11:51:23Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:context-capability
  @case:happy
  @context.capability.39
  Scenario: Context names the kinds users see, evidence included, and every artifact family
    Given the record kinds and artifact families the model registers
    When canon context is run
    Then its kinds are every record kind under the name users type, evidence rather than evidence_record, and its cel section names the same kinds
    And its families are every artifact family in order, and the capability version is 6
    And every closed --kind value of query, export and purge is one of those kinds

  # canon: {"schema":1,"at":"2026-10-23T11:51:38Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:context-capability
  @case:failure
  @context.capability.40
  Scenario: Context refuses an unknown field with exit 2
    Given a directory with no canon.yaml
    When canon context is run with --fields naming a section that does not exist, with --json and without
    Then it exits 2 and, with --json, names UNKNOWN_FIELD
    And the named sections that do exist are the only keys canon context returns when --fields lists them

  # canon: {"schema":1,"at":"2026-10-23T11:51:50Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:context-capability
  @case:happy
  @context.capability.41
  Scenario: Completion prints a script for each supported shell
    Given a directory with no canon.yaml
    When canon completion is run for bash, zsh and fish
    Then each exits 0 and prints a script that names canon and completes its own command
    And with --json the document carries the shell and the same script, and stderr is empty

  # canon: {"schema":1,"at":"2026-10-23T11:52:01Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:context-capability
  @case:failure
  @context.capability.42
  Scenario: Completion refuses a shell it does not know
    Given a directory with no canon.yaml
    When canon completion is run with a shell it does not support, or with none
    Then each exits 2
    And a supported shell still runs with --as given, because completion needs no repo

  # canon: {"schema":1,"at":"2026-10-23T11:52:25Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:context-capability
  @case:edge
  @context.capability.43
  Scenario: An unknown role value runs and warns with the closest known role
    Given a repository whose policy names the custom role auditor
    When a record is written with --role reviwer, with --role zzzzzzzz, and with --role reviewer and auditor
    Then the misspelt role runs and the warning names the closest known role, reviewer
    And a role with nothing close runs and warns without a suggestion
    And a known role and a policy-named role run with no warning

  # canon: {"schema":1,"at":"2026-10-23T11:53:03Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:context-capability
  @case:failure
  @context.capability.44
  Scenario: An unknown CANON_ROLE value is refused with UNKNOWN_ROLE by status, while schema and context are never failed by it
    Given CANON_ROLE set to a value that is not a role
    When canon status runs, and canon schema and canon context run, and canon status runs with --as naming a valid role
    Then status exits 2 with UNKNOWN_ROLE, naming the value, and with stderr empty under --json
    And schema and context still exit 0, never filtered or failed by the variable, and context names the value in meta.warnings
    And the flag wins over the variable, and an unknown --as value is a usage error
