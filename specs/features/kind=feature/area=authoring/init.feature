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
