Feature: authoring init
  # canon: {"schema":1,"at":"2026-10-10T15:34:47Z","actor":{"agent_id":"canon"}}

  # canon: {"schema":1,"at":"2026-10-10T15:34:47Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:happy
  @authoring.init.01
  Scenario: A fresh init writes the config, a starter policy, the AGENTS.md block and a plans home
    Given an empty directory
    When canon init runs in it
    Then canon.yaml configures an openspec plans source rooted at the repo and openspec/changes exists with a keep file
    And .canon/policy.yaml turns spec_coverage on with require_evidence, require_cases [failure] and require_review
    And the policy says a human approves it, and so does the init output
    And AGENTS.md holds the canon block naming the loop, canon status and the canon skill

  # canon: {"schema":1,"at":"2026-10-10T15:34:47Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:happy
  @authoring.init.02
  Scenario: The AGENTS.md block is appended to an existing file without touching its text
    Given an AGENTS.md with the project's own text and no canon block
    When canon init runs
    Then the original text is kept byte for byte
    And the canon block is appended after a blank line, between its markers

  # canon: {"schema":1,"at":"2026-10-10T15:34:48Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:edge
  @authoring.init.03
  Scenario: A rerun keeps canon.yaml and the policy and refreshes only the AGENTS.md block
    Given a repo with canon.yaml, a hand-written policy and an AGENTS.md whose canon block is stale
    When canon init runs again
    Then it exits 0 and says canon.yaml is left unchanged
    And canon.yaml and the policy are byte-identical
    And only the text between the canon markers is replaced
    And a further rerun reports the block current and changes no byte

  # canon: {"schema":1,"at":"2026-10-10T15:34:48Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.init.04
  Scenario: An AGENTS.md whose canon block has no end marker is refused and nothing is written
    Given an AGENTS.md with a canon begin marker and no end marker after it
    When canon init runs
    Then it exits 2 naming the missing end marker
    And neither canon.yaml nor any other file is written
    And AGENTS.md is byte-identical

  # canon: {"schema":1,"at":"2026-10-10T15:34:48Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.init.05
  Scenario: A rerun with nothing to refresh refuses to overwrite canon.yaml
    Given a repo that already has canon.yaml
    When canon init runs with --no-agents-md
    Then there is nothing left to refresh, so it exits 2 refusing to overwrite canon.yaml
    And canon.yaml is byte-identical and no AGENTS.md is written

  # canon: {"schema":1,"at":"2026-10-10T15:34:48Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:happy
  @authoring.init.06
  Scenario: The starter policy loads clean and holds an unevidenced scenario to account
    Given the starter policy that canon init writes
    When the gate's policy loader resolves it
    Then it reports zero diagnostics, the unknown-key check included
    And on a fresh init the gate is clean with no scenarios
    And once an unevidenced scenario is indexed the gate is red naming it, with no coverage-off advisory

  # canon: {"schema":1,"at":"2026-10-10T15:34:48Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:happy
  @authoring.init.07
  Scenario: The opt-out flags skip the AGENTS.md block and the starter policy
    Given an empty directory
    When canon init runs with --no-agents-md and --no-policy
    Then canon.yaml is written
    And neither AGENTS.md nor .canon/policy.yaml exists

  # canon: {"schema":1,"at":"2026-10-10T15:34:48Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:edge
  @authoring.init.08
  Scenario: An existing policy is never overwritten
    Given a fresh directory that already has a .canon/policy.yaml
    When canon init runs
    Then the policy is byte-identical and the output says it was left unchanged

  # canon: {"schema":1,"at":"2026-10-10T15:34:48Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:happy
  @authoring.init.09
  Scenario: Plan import is clean on a fresh init
    Given a fresh canon init with other top-level directories present
    When canon ingest plans runs
    Then it exits 0 with the openspec source parsed and zero malformed entries

  # canon: {"schema":1,"at":"2026-10-10T15:51:40Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.init.10
  Scenario: A duplicate or unmatched AGENTS.md marker is refused and nothing changes
    Given an AGENTS.md with two canon begin marker lines and one end marker line
    When canon init runs, in a fresh repo or as a rerun
    Then it exits 2 naming how many begin and end markers it found
    And AGENTS.md, canon.yaml and .canon are exactly as they were

  # canon: {"schema":1,"at":"2026-10-10T15:51:40Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:edge
  @authoring.init.11
  Scenario: Markers inside a code fence are text, not the canon block
    Given an AGENTS.md whose only canon markers sit inside a fenced code block
    When canon init runs
    Then the fenced sample is kept byte for byte and the canon block is appended after it
    And a rerun finds the block current and changes no byte

  # canon: {"schema":1,"at":"2026-10-10T19:28:02Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:happy
  @authoring.init.80
  Scenario: Init names the command that approves its starter policy
    Given an empty repository
    When canon init writes the starter policy
    Then its output says a human approves the policy with `canon policy approve --by <who>`
    And the AGENTS.md block names the same command

  # canon: {"schema":1,"at":"2026-10-11T14:43:35Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:happy
  @authoring.init.30
  Scenario: Default init writes the strict starter policy under a level line
    Given an empty repository
    When canon init runs without a policy level
    Then .canon/policy.yaml opens with the line "# canon policy level: strict"
    And every byte after that line is the starter policy canon 0.15 wrote: evidence, a failure case per surface and independent review
    And canon status reports the level as strict

  # canon: {"schema":1,"at":"2026-10-11T14:43:36Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:happy
  @authoring.init.31
  Scenario: A light starter policy requires evidence only and says how to raise it
    Given an empty repository
    When canon init runs with the light policy level
    Then .canon/policy.yaml opens with the light level line and requires evidence on every scenario
    And the failure-case and review requirements are present only as commented lines that say how to raise the level and that a human approves the edit
    And the gate does not ask a golden-path-only surface for a failure case
    And canon status reports the level as light with how to raise it

  # canon: {"schema":1,"at":"2026-10-11T14:43:36Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:happy
  @authoring.init.32
  Scenario: A standard starter policy adds a failure case per surface and says how to reach strict
    Given an empty repository
    When canon init runs with the standard policy level
    Then .canon/policy.yaml opens with the standard level line and requires evidence and a failure case per feature surface
    And the review requirement is present only as commented lines that say how to raise the level to strict
    And canon status reports the level as standard with how to reach strict

  # canon: {"schema":1,"at":"2026-10-11T14:43:36Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.init.33
  Scenario: An unknown policy level is refused and nothing is written
    Given an empty repository
    When canon init runs with a policy level that is not light, standard or strict
    Then it exits 2 naming the accepted levels
    And it writes no canon.yaml and no .canon directory

  # canon: {"schema":1,"at":"2026-10-11T14:43:36Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.init.34
  Scenario: A policy level together with no-policy is refused
    Given an empty repository
    When canon init runs with a policy level and the no-policy flag together
    Then it exits 2 saying the two cannot be used together
    And it writes no canon.yaml and no .canon directory

  # canon: {"schema":1,"at":"2026-10-11T15:01:54Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.init.70
  Scenario: A fresh init followed by a query leaves git status clean
    Given a git repository freshly initialized with canon init and committed
    When a query reads the ledger and appends to the access audit log
    Then the init gitignore covers .canon/audit/
    And git status reports nothing untracked

  # canon: {"schema":1,"at":"2026-10-11T15:01:54Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:edge
  @authoring.init.71
  Scenario: The scenario index a reader rebuilds stays out of git
    Given a repository initialized with canon init whose feature file is committed
    When a reader rebuilds the stale scenario index
    Then the init gitignore covers .canon/ledger/kind=scenario/
    And git status reports nothing untracked

  # canon: {"schema":1,"at":"2026-10-11T15:45:13Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:edge
  @authoring.init.72
  Scenario: The scenario index lock stays out of git
    Given a repository initialized with canon init whose feature file is committed
    When a reader refreshes the stale scenario index and takes the index lock
    Then the init gitignore covers .canon/scenario-index.lock
    And git status reports nothing untracked

  # canon: {"schema":1,"at":"2026-10-12T10:31:30Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:happy
  @authoring.init.90
  Scenario: Without a terminal init installs the skill for the detected harnesses only
    Given an empty repository holding an .omp directory and an .agents directory, and a skill source
    When canon init runs with no terminal and no --providers
    Then it installs the canon skill for omp only and says where
    And canon.yaml ends with `skills: { providers: [omp] }` and the install lock records omp only
    And canon init --check-config passes the skills section and canon skills doctor reports healthy from canon.yaml

  # canon: {"schema":1,"at":"2026-10-12T10:31:30Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:happy
  @authoring.init.91
  Scenario: --providers codex answers without a prompt and installs only the agents projection
    Given an empty repository holding a .claude directory, and a skill source
    When canon init runs with --providers codex
    Then only .agents/skills/canon is projected and nothing is written under .claude or .codex
    And canon.yaml records `providers: [codex]`, and the policy and AGENTS.md are written as before
    And a rerun with --providers leaves canon.yaml alone and says to use canon skills install --providers instead

  # canon: {"schema":1,"at":"2026-10-12T10:31:30Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.init.92
  Scenario: A repository with no harness directory gets no skill and is told how to choose
    Given an empty repository with no .claude, .codex, .omp or .pi directory
    When canon init runs with no terminal and no --providers
    Then it exits 0, installs nothing and writes no install lock
    And its output says no harness was detected and names `canon skills install --providers=<list>`
    And canon.yaml carries the skills section only as a commented example, which check-config reports as not configured

  # canon: {"schema":1,"at":"2026-10-12T10:31:30Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:edge
  @authoring.init.93
  Scenario: An explicit empty harness choice is recorded
    Given an empty repository holding a .claude directory
    When canon init runs with --providers none, or with an empty --providers
    Then canon.yaml records `providers: []` and nothing is installed
    And canon skills check exits 0 reporting no providers from canon.yaml

  # canon: {"schema":1,"at":"2026-10-12T10:31:30Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.init.94
  Scenario: An unknown harness in --providers is refused before anything is written
    Given an empty repository
    When canon init runs with --providers naming a harness canon does not know
    Then it exits 2 naming the harness and the accepted names
    And the repository is still empty

  # canon: {"schema":1,"at":"2026-10-12T10:31:30Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.init.95
  Scenario: A missing skill source keeps the recorded choice and names the install to run
    Given an empty repository holding a .pi directory, and no skill source
    When canon init runs with no terminal and no --providers
    Then it records `providers: [pi]` in canon.yaml, installs nothing and exits 2
    And it names the missing source and `canon skills install` with CANON_SKILLS_SOURCE or --source
    And that install, once the source exists, projects pi from canon.yaml

  # canon: {"schema":1,"at":"2026-10-12T10:31:30Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:happy
  @authoring.init.96
  Scenario: On a terminal init asks with the detected harnesses checked
    Given detected .claude and .omp directories
    When init asks which harnesses get the canon skill
    Then it lists claude, codex, omp and pi with claude and omp checked
    And Enter or the end of input keeps the checked ones, a comma- or space-separated list or none replaces them
    And an unknown name is reported and asked again
