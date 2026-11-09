Feature: gate git-env
  # canon: {"schema":1,"at":"2026-11-09T10:11:10Z","actor":{"agent_id":"Impl40_git_env","role":"planner"}}

  # canon: {"schema":1,"at":"2026-11-09T10:11:10Z","actor":{"agent_id":"Impl40_git_env","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-40-git-env-allowlist.F1
  @gate.git-env.01
  Scenario: GIT_DIFF_OPTS, GIT_CONFIG_COUNT, GIT_CONFIG_PARAMETERS or GIT_TRACE in the shell does not change the id or state digest of a context pack on a dirty tree
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:12:13Z","actor":{"agent_id":"Impl40_git_env","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-40-git-env-allowlist.F1
  @gate.git-env.02
  Scenario: The names on the allow list reach the git child as they are, a GIT_* off the list (one git does not know too) is removed, and canon's own variables keep their values
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:12:27Z","actor":{"agent_id":"Impl40_git_env","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:failure
  @flow:canon-0-40-git-env-allowlist.F1
  @gate.git-env.03
  Scenario: A GIT_* off the list never reaches git whatever its value, and GIT_DIR and GIT_INDEX_FILE are still removed
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:12:35Z","actor":{"agent_id":"Impl40_git_env","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-40-git-env-allowlist.F2
  @gate.git-env.04
  Scenario: A home gitconfig that changes the shape of a diff or status does not change the state digest or id of a dirty context pack, and the digest is the one 0.39 made
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:12:42Z","actor":{"agent_id":"Impl40_git_env","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-40-git-env-allowlist.F2
  @gate.git-env.05
  Scenario: The same settings in the repository's own .git/config do not change the state digest or id of a dirty context pack
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:12:54Z","actor":{"agent_id":"Impl40_git_env","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-40-git-env-allowlist.F2
  @gate.git-env.06
  Scenario: diff.ignoreSubmodules=all does not hide a submodule update: evidence that declares the submodule as a surface is stale-evidence at gate check
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:13:01Z","actor":{"agent_id":"Impl40_git_env","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:failure
  @flow:canon-0-40-git-env-allowlist.F2
  @gate.git-env.07
  Scenario: core.autocrlf is not pinned: a CRLF file of a repository set to autocrlf=true has the code tree of HEAD and the tree is not dirty, without it the tree is dirty
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:13:08Z","actor":{"agent_id":"Impl40_git_env","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-40-git-env-allowlist.F3
  @gate.git-env.08
  Scenario: The CHANGELOG entry names the allowed names, the shells that now fail, and the one-time change of a dirty pack id
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:13:24Z","actor":{"agent_id":"Impl40_git_env","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:failure
  @flow:canon-0-40-git-env-allowlist.F3
  @gate.git-env.09
  Scenario: A CHANGELOG without the entry or an entry missing an item, or a coding-rules page that only says it clears the git environment, fails the release check
    Given a step
