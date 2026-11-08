Feature: gate git-boundary
  # canon: {"schema":1,"at":"2026-11-08T06:17:00Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}

  # canon: {"schema":1,"at":"2026-11-08T06:17:00Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-39-git-boundary.F1
  @gate.git-boundary.01
  Scenario: A shell that points git at another repository does not move the evidence record: it names the project's commit and code tree
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:17:24Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-39-git-boundary.F1
  @gate.git-boundary.02
  Scenario: A GIT_INDEX_FILE in the shell does not change the state digest of a context pack
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:17:31Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-39-git-boundary.F1
  @gate.git-boundary.03
  Scenario: GIT_DIR in the shell does not make migrate --commit change the index of the repository it names
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:17:48Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:failure
  @flow:canon-0-39-git-boundary.F1
  @gate.git-boundary.04
  Scenario: GIT_DIR does not make a directory that is no work tree one: the evidence is refused as COMMIT_REQUIRED
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:17:58Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-39-git-boundary.F2
  @gate.git-boundary.05
  Scenario: A surface declared with a non-ASCII name or a quote goes stale when the file changes
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:18:06Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-39-git-boundary.F2
  @gate.git-boundary.06
  Scenario: The approval payload lists a non-ASCII surface as it is
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:18:17Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:failure
  @flow:canon-0-39-git-boundary.F2
  @gate.git-boundary.07
  Scenario: A name that is not UTF-8 gives no surface for the risk approval and never a stale verdict
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:18:23Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-39-git-boundary.F3
  @gate.git-boundary.08
  Scenario: A git that stops answering ends gate check within seconds of the deadline
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:18:29Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-39-git-boundary.F3
  @gate.git-boundary.09
  Scenario: The one git path stops a call past its deadline or its output limit, and a grandchild holding the pipes is not waited for
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:18:36Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:failure
  @flow:canon-0-39-git-boundary.F3
  @gate.git-boundary.10
  Scenario: evidence add is refused as COMMIT_REQUIRED, naming git add and the deadline, when the code tree cannot be made in time
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:18:42Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:failure
  @flow:canon-0-39-git-boundary.F3
  @gate.git-boundary.11
  Scenario: A context pack that cannot ask git in time is refused as CONTEXT_GIT_REQUIRED
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:18:53Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:failure
  @flow:canon-0-39-git-boundary.F4
  @gate.git-boundary.12
  Scenario: A git launch outside the one path is reported with its file and line and the scan exits 1
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:19:00Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-39-git-boundary.F4
  @gate.git-boundary.13
  Scenario: The repository and a tree without a launch outside the path pass the scan
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:19:06Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-39-git-boundary.F4
  @gate.git-boundary.14
  Scenario: The scan is wired into just lint, the CI clippy job and the coding rules
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:19:13Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-39-git-boundary.F5
  @gate.git-boundary.15
  Scenario: The CHANGELOG says the environment is ignored, the names, the deadline and limits, and when to approve again
    Given a step
