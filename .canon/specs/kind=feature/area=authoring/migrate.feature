Feature: canon migrate
  # canon: {"schema":1,"at":"2026-10-12T11:44:52Z","actor":{"agent_id":"canon"}}

  # canon: {"schema":1,"at":"2026-10-12T11:44:52Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:happy
  @authoring.migrate.01
  Scenario: A repo canon 0.16 set up moves onto the 0.17 layout in one command
    Given a git repo canon 0.16 set up: its corpus under specs/features/kind=feature/, a change canon change new wrote under openspec/changes/ and adopted, the init openspec source, an install lock and a gitignore missing canon's newer lines
    When canon migrate --dry-run runs
    Then it prints every move, the canon.yaml and gitignore edits and the index rebuild, and writes nothing
    When canon migrate runs
    Then the corpus is at .canon/specs/kind=feature/ by git mv and canon.yaml names root .canon/specs with former_roots [specs], every other byte kept
    And the change is at .canon/changes/<slug>/ with proposal.md renamed brief.md, re-imported from canon's own source under the same change id with its adoption and task ids
    And the init openspec source and its .gitkeep are gone, skills.providers holds the lock's providers, the missing gitignore lines are added and the index is rebuilt
    And a second run finds nothing to migrate and changes nothing

  # canon: {"schema":1,"at":"2026-10-12T11:44:52Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:edge
  @authoring.migrate.02
  Scenario: A spec root other than the repo-level specs only drops its features level
    Given a repo outside git whose CRLF canon.yaml names a spec root game/specs still holding features/kind=feature/
    When canon migrate runs
    Then the corpus moves to game/specs/kind=feature/ with a plain rename and the emptied features/ directory is removed
    And other files in the root stay, and canon.yaml is byte-identical

  # canon: {"schema":1,"at":"2026-10-12T11:44:52Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.migrate.03
  Scenario: Uncommitted changes under a path migrate moves refuse the migration
    Given a repo on the 0.16 layout with an edited and an untracked feature file under specs/features/kind=feature/
    When canon migrate runs
    Then it exits 2 naming the uncommitted paths and asks to commit or stash them
    And nothing moves, canon.yaml is unchanged and no index is written

  # canon: {"schema":1,"at":"2026-10-12T11:44:52Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.migrate.04
  Scenario: A destination that already exists refuses the migration
    Given a repo on the 0.16 layout that already holds .canon/specs/kind=feature/
    When canon migrate --dry-run runs
    Then it exits 2 naming the spec root, its source and the existing destination, and nothing moves

  # canon: {"schema":1,"at":"2026-10-12T11:44:52Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:edge
  @authoring.migrate.05
  Scenario: An approval 0.16 recorded in an external proposal moves into a canon pointer brief
    Given an external openspec change whose proposal.md holds the Approved: line canon 0.16 recorded there
    When canon migrate runs
    Then .canon/changes/<change>/brief.md holds a Source: line pointing at the proposal and that Approved: line
    And the proposal is not edited and the output names the line left in it
    And the openspec source stays because a change is still under it, and canon status reads the approval from the canon brief
    And a second run writes no brief

  # canon: {"schema":1,"at":"2026-10-12T11:44:52Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:edge
  @authoring.migrate.06
  Scenario: A change with only one sign that canon change new wrote it stays where it is and is named
    Given one openspec change in canon change new's scaffold whose first stored version was not adopted, and one adopted at once whose files are not the scaffold
    When canon migrate --dry-run runs
    Then both stay where they are and the plan names each with the signal it lacks
    And the change that carries both signals still moves

  # canon: {"schema":1,"at":"2026-10-12T11:44:52Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:happy
  @authoring.migrate.07
  Scenario: Reviews pinned before the move stay current after the spec root moves into .canon
    Given require_review and a review written before 0.15 pinned to a commit where the scenario sat under specs/
    When canon migrate moves the root to .canon/specs
    Then canon gate check prints the same legacy advisory before and after, the review checked current at its pin
    And without former_roots in canon.yaml the review would be undecidable

  # canon: {"schema":1,"at":"2026-10-12T11:44:52Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.migrate.08
  Scenario: A canon.yaml migrate cannot edit with every other byte kept is refused
    Given a repo on the 0.16 layout whose specs section is a flow-style mapping
    When canon migrate runs
    Then it exits 2 saying it cannot rewrite specs.roots safely and naming the edit to make by hand
    And nothing moves and canon.yaml is unchanged
