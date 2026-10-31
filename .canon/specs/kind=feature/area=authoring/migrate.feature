Feature: canon migrate
  # canon: {"schema":1,"at":"2026-10-12T11:44:52Z","actor":{"agent_id":"canon"}}

  # canon: {"schema":1,"at":"2026-10-12T11:44:52Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:happy
  @authoring.migrate.01
  Scenario: A repo canon 0.16 set up moves onto the 0.17 layout: a preview, then the commit of that plan
    Given a git repo canon 0.16 set up: its corpus under specs/features/kind=feature/, a change canon change new wrote under openspec/changes/ and adopted, the init openspec source, an install lock and a gitignore missing canon's newer lines
    When canon migrate runs without --commit
    Then it previews every move, the canon.yaml and gitignore edits and the index rebuild, names the command that commits the plan, and writes nothing
    When canon migrate --commit --plan-digest <the previewed digest> runs
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
    When canon migrate is committed with the digest of its preview
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
    When canon migrate runs
    Then it exits 2 naming the spec root, its source and the existing destination, and nothing moves

  # canon: {"schema":1,"at":"2026-10-12T11:44:52Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:edge
  @authoring.migrate.05
  Scenario: An approval 0.16 recorded in an external proposal moves into a canon pointer brief
    Given an external openspec change whose proposal.md holds the Approved: line canon 0.16 recorded there
    When canon migrate is committed with the digest of its preview
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
    When canon migrate runs and previews the plan
    Then both stay where they are and the plan names each with the signal it lacks
    And the change that carries both signals still moves

  # canon: {"schema":1,"at":"2026-10-12T11:44:52Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:happy
  @authoring.migrate.07
  Scenario: Reviews pinned before the move stay current after the spec root moves into .canon
    Given require_review and a review written before 0.15 pinned to a commit where the scenario sat under specs/
    When canon migrate is committed and moves the root to .canon/specs
    Then the review is pinned to what its evidence says and counts, as it would have had the root not moved
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

  # canon: {"schema":1,"at":"2026-10-12T12:49:07Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.migrate.09
  Scenario: A migrate run that stopped before writing canon.yaml is finished by the next run
    Given a repo canon 0.16 set up, and a committed canon migrate run whose canon.yaml write fails after the corpus and the change moved
    Then that run exits 1 with MIGRATE_STOPPED and canon.yaml is byte-identical, never half-written
    When canon migrate is committed again
    Then it sees the root already at .canon/specs while canon.yaml names specs and writes root .canon/specs with former_roots [specs]
    And it re-imports the moved change under the same change id with its adoption, removes the init openspec source, and a third run finds nothing to migrate

  # canon: {"schema":1,"at":"2026-10-12T12:49:07Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.migrate.10
  Scenario: A pointer brief that cannot be written keeps the openspec source that holds the approval
    Given an external openspec change whose proposal.md holds an Approved: line, the init openspec source, and a .canon/changes directory canon cannot write into
    When canon migrate is committed with the digest of its preview
    Then it exits 1 with MIGRATE_STOPPED naming the brief it could not write, and canon.yaml still lists the init openspec source and its .gitkeep stays
    When the directory is writable again and canon migrate is committed
    Then it writes the pointer brief with Source: and that Approved: line, and only then removes the init openspec source

  # canon: {"schema":1,"at":"2026-10-12T12:49:07Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.migrate.11
  Scenario: A change move that could stop halfway is refused, and a half-moved change is finished by the next run
    Given a repo canon 0.16 set up whose canon-written change directory is not writable
    When canon migrate runs
    Then it exits 2 naming the directory before anything moves, and canon.yaml is unchanged
    Given an earlier run moved that change to .canon/changes/<slug>/ and stopped before renaming proposal.md
    When canon migrate is committed
    Then it renames proposal.md to brief.md, re-imports the change from canon's own source, and finishes the rest of the migration

  # canon: {"schema":1,"at":"2026-10-12T12:49:07Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.migrate.12
  Scenario: Change material the openspec reader cannot read keeps the openspec source
    Given a repo canon 0.16 set up with a canon-written change, a change directory with no proposal.md and a loose file under openspec/changes/
    When canon migrate runs and previews the plan
    Then the directory is named as staying with the openspec reader's diagnostic, the canon-written change still moves, and the init openspec source is kept
    When only the loose file is left and canon migrate is committed
    Then the plan names the source as staying because canon has not read that file, and the file and the source stay

  # canon: {"schema":1,"at":"2026-10-21T16:36:09Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:authoring-scaffold
  @case:edge
  @authoring.migrate.13
  Scenario: migrate adds the missing managed gitignore lines to an existing block and a second run adds none
    Given a repo whose gitignore already holds canon's block without the scenario archive, dashboard-snapshot, learn and r2 lines
    When canon migrate is committed, and again
    Then the first run adds exactly the missing lines under their comments and keeps every other byte of the gitignore
    And the second run adds none

  # canon: {"schema":1,"at":"2026-10-21T17:24:55Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.migrate.14
  Scenario: migrate adds only the missing line when its managed comment is kept
    Given a gitignore that kept a managed comment but lost the line under it
    When canon migrate is committed, and again
    Then the first run writes only the missing line, directly under its comment, and no comment twice
    And the second run changes nothing

  # canon: {"schema":1,"at":"2026-10-23T11:48:21Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:authoring-scaffold
  @case:happy
  @authoring.migrate.15
  Scenario: canon migrate previews by default, commits only with the digest of that plan, and writes nothing before
    Given a repo canon 0.16 set up
    When canon migrate runs with --json
    Then it exits 0 with data.committed false, a planDigest, and effects listing each write, and the first next step is canon migrate --commit --plan-digest with that digest
    And the tree is byte-identical afterwards
    And canon migrate --commit --plan-digest with that digest exits 0 with data.committed true and the corpus moved
    And the 0.27 spellings --dry-run, --commit alone and --plan-digest alone are usage errors that exit 2

  # canon: {"schema":1,"at":"2026-10-23T11:48:36Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.migrate.16
  Scenario: A commit whose plan digest no longer matches the repo is refused PLAN_STALE and writes nothing
    Given a repo whose migrate preview gave a digest, and the repo changed afterwards so the plan differs
    When canon migrate --commit --plan-digest runs with the old digest
    Then it exits 2 with PLAN_STALE and nothing is written
    And a fresh preview gives a new digest that commits

  # canon: {"schema":1,"at":"2026-10-23T11:48:42Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:authoring-scaffold
  @case:edge
  @authoring.migrate.17
  Scenario: The preview lists every review record the commit rewrites
    Given a repo with two legacy review records and no scenario index built
    When canon migrate previews, then is committed with the digest
    Then the preview exits 0, writes nothing and lists one rewrite effect per review record, naming the file it replaces and the file that takes its place
    And after the commit the review files that were removed are exactly the ones the preview named

  # canon: {"schema":1,"at":"2026-10-23T11:48:48Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:authoring-scaffold
  @case:happy
  @authoring.migrate.18
  Scenario: migrate rewrites a 0.27 setup of hooks, scripts and skill and backs up what it replaces
    Given a repo set up by canon 0.27: the skill with its reference files in the Claude and Codex projections, canon gate task hook entries in both settings files, a retrieve hook script copy and the install lock
    When canon migrate previews, then is committed with the digest
    Then the preview lists each settings file, the script copy, the skill files, the reference files to remove and the lock as effects, and writes nothing
    And the commit replaces canon gate task with canon task done in each settings file, keeps the other hook entries, regenerates the scripts, projects the stub in place of the skill and drops the reference files
    And every rewritten or removed file is kept with its original bytes in one directory under .canon/migrate-backup
    And a second run lists no effect, makes no new backup and leaves the tree untouched

  # canon: {"schema":1,"at":"2026-10-23T11:48:54Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:authoring-scaffold
  @case:edge
  @authoring.migrate.19
  Scenario: A 0.27 setup that is not migrated yet is detected and migrate is named first
    Given a repo set up by canon 0.27 that has not been migrated
    When canon status, canon gate check and canon context run with --json
    Then each puts canon migrate --json first in meta.next and warns that canon migrate is needed
    And a hook entry that still calls canon gate task is enough to be detected
    And after the migration is committed neither status nor context names canon migrate

  # canon: {"schema":1,"at":"2026-10-27T12:04:50Z","actor":{"agent_id":"impl-031-bf","role":"implementer","session_id":"bf-2026-10-27"}}
  @subject:authoring-scaffold
  @case:happy
  @flow:canon-0-31-brief-format.F5
  @authoring.migrate.20
  Scenario: A preview of the brief rewrite writes nothing and says first whether it re-binds an approval
    Given a repo whose briefs are in the 0.30 format, one approved
    When canon migrate runs without --commit
    Then the working tree is byte-identical afterwards and the plan's first brief line says whether it re-binds approvals and review pins
    And per brief it shows the comments removed, the waivers moved, the headings translated, the chore marker moved and the Approved: digest old and new

  # canon: {"schema":1,"at":"2026-10-27T12:04:51Z","actor":{"agent_id":"impl-031-bf","role":"implementer","session_id":"bf-2026-10-27"}}
  @subject:authoring-scaffold
  @case:failure
  @flow:canon-0-31-brief-format.F5
  @authoring.migrate.21
  Scenario: A preview refuses uncommitted briefs and names the briefs it will not re-bind
    Given a git repo whose old brief has an uncommitted edit, a brief edited after its approval, and a brief whose marker role differs from the table
    When canon migrate runs
    Then the uncommitted brief is refused with MIGRATE_UNCOMMITTED and nothing is written
    And the edited brief and the role-mismatched brief are named as not re-bound, with the reason

  # canon: {"schema":1,"at":"2026-10-27T12:04:52Z","actor":{"agent_id":"impl-031-bf","role":"implementer","session_id":"bf-2026-10-27"}}
  @subject:authoring-scaffold
  @case:happy
  @flow:canon-0-31-brief-format.F6
  @authoring.migrate.22
  Scenario: Committing the plan rewrites every brief, keeps approved briefs approved and leaves the prose, and a second run has nothing to do
    Given a repo with approved and unapproved 0.30 briefs, feature and chore, with waivers, Korean headings and a review pinned to a chore
    When canon migrate --commit with the plan digest runs, and then runs again
    Then every brief is in the 0.31 format with no HTML comment, the approved ones read approved with no BRIEF_CHANGED_SINCE_APPROVAL, and their blockers and advisories, waivers included, are as before except for line numbers
    And the prose outside comments and headings is byte-identical, the Approved: line keeps its approver, time and words, and the chore review is a new version pinned to the new digest with every other field as before
    And the second run says there is nothing to do

  # canon: {"schema":1,"at":"2026-10-27T12:04:53Z","actor":{"agent_id":"impl-031-bf","role":"implementer","session_id":"bf-2026-10-27"}}
  @subject:authoring-scaffold
  @case:failure
  @flow:canon-0-31-brief-format.F6
  @authoring.migrate.23
  Scenario: A changed plan digest is refused with nothing written, and a brief edited after its approval stays changed
    Given a plan previewed and then a brief changed
    When canon migrate --commit runs with the old digest
    Then it is refused with PLAN_STALE and nothing is written
    And a brief edited after its approval is rewritten and still reads BRIEF_CHANGED_SINCE_APPROVAL

  # canon: {"schema":1,"at":"2026-10-27T12:04:55Z","actor":{"agent_id":"impl-031-bf","role":"implementer","session_id":"bf-2026-10-27"}}
  @subject:authoring-scaffold
  @case:failure
  @flow:canon-0-31-brief-format.F6
  @authoring.migrate.24
  Scenario: A plan that re-binds an approval or a review pin is human-only to commit, and one that re-binds nothing is not
    Given a plan that re-binds an Approved: line and one that re-binds nothing
    When canon migrate --commit runs as planner and as orchestrator
    Then the re-binding plan is refused with HUMAN_ONLY, exit 2, and the tree is unchanged
    And the plan that re-binds nothing is run by the orchestrator, and a preview is open to every role

  # canon: {"schema":1,"at":"2026-10-27T12:04:56Z","actor":{"agent_id":"impl-031-bf","role":"implementer","session_id":"bf-2026-10-27"}}
  @subject:authoring-scaffold
  @case:failure
  @flow:canon-0-31-brief-format.F6
  @authoring.migrate.25
  Scenario: Revert restores briefs, reviews and digests byte for byte, refuses when a file changed since, and a stopped run is finished by the next
    Given a migrated repo, and a repo where the run stopped after some briefs
    When canon migrate --revert runs, and canon migrate runs again
    Then the revert restores the old briefs, the old digests and the old reviews byte for byte
    And a revert is refused naming the file when a brief, an approval or a review version changed since the migrate, and writes nothing
    And the next run finishes a stopped one, and the revert then restores every original
