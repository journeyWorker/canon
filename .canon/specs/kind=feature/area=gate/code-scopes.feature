Feature: gate code-scopes
  # canon: {"schema":1,"at":"2026-11-06T05:02:00Z","actor":{"agent_id":"impl-code-scopes","session_id":"s-0-38-code-scopes"}}

  # canon: {"schema":1,"at":"2026-11-06T05:02:01Z","actor":{"agent_id":"impl-code-scopes","session_id":"s-0-38-code-scopes"}}
  @subject:gate-trust-spine
  @case:happy
  @flow:canon-0-38-code-scopes.F1
  @gate.code-scopes.01
  Scenario: Approving the policy makes a scope stand: a chore that changes one app file asks the app scenarios and not the core one
    Given a verifying subject with scenarios `core.engine.01`, `app.bulkread.01` and `app.screen.01` evidenced and reviewed, and an approved policy whose `code_scopes` give `core` the paths `core/**` and `app` the paths `app/**` and `core/**`
    When a chore is approved and then changes one file under `app/`, and the subject is moved to shipped
    Then the refusal asks for `app.bulkread.01` and `app.screen.01` and names the changed path `app/main.rs`
    And it does not ask for `core.engine.01`
    And it leads with one batch command, and after that command only the chore's change-level review remains

  # canon: {"schema":1,"at":"2026-11-06T05:02:02Z","actor":{"agent_id":"impl-code-scopes","session_id":"s-0-38-code-scopes"}}
  @subject:gate-trust-spine
  @case:failure
  @flow:canon-0-38-code-scopes.F1
  @gate.code-scopes.02
  Scenario: A policy edited after its approval, or never approved, declares no scope
    Given the same subject and chore, with the policy edited after its approval, and another repository whose policy was never approved
    When the chore changes one file under `app/` and the subject is moved to shipped, and `canon gate check` runs
    Then the edited policy fails the move closed with `POLICY_APPROVAL_STALE`
    And `canon gate check` asks for all three scenarios and names no changed path
    And the policy nobody approved asks for all three scenarios too

  # canon: {"schema":1,"at":"2026-11-06T05:02:03Z","actor":{"agent_id":"impl-code-scopes","session_id":"s-0-38-code-scopes"}}
  @subject:gate-trust-spine
  @case:happy
  @flow:canon-0-38-code-scopes.F2
  @gate.code-scopes.03
  Scenario: A surface key wins over its area's key: the surface the changed file is outside of is not asked for
    Given a policy whose `code_scopes` also give the surface `app.bulkread` the paths `app/bulk/**`
    When the approved chore changes `app/main.rs` and the subject is moved to shipped
    Then the refusal asks for `app.screen.01` alone and names `app/main.rs`
    And neither `core.engine.01` nor `app.bulkread.01` is asked for
    And `canon status` says two scenarios need nothing, by the keys `app.bulkread` and `core`

  # canon: {"schema":1,"at":"2026-11-06T05:02:04Z","actor":{"agent_id":"impl-code-scopes","session_id":"s-0-38-code-scopes"}}
  @subject:gate-trust-spine
  @case:failure
  @flow:canon-0-38-code-scopes.F2
  @gate.code-scopes.04
  Scenario: A chore that changes a core file asks for the core and the app scenarios
    Given the subject and the policy of the first scenario
    When the approved chore changes a file under `core/` and the subject is moved to shipped
    Then the refusal asks for all three scenarios
    And it names the path `core/lib.rs`

  # canon: {"schema":1,"at":"2026-11-06T05:02:05Z","actor":{"agent_id":"impl-code-scopes","session_id":"s-0-38-code-scopes"}}
  @subject:gate-trust-spine
  @case:edge
  @flow:canon-0-38-code-scopes.F2
  @gate.code-scopes.05
  Scenario: A file renamed out of the scope asks for the scenarios by its old path
    Given the subject and the policy of the first scenario
    When the approved chore moves `core/lib.rs` to `util/lib.rs`
    Then the refusal asks for all three scenarios and names `core/lib.rs`
    And it does not name `util/lib.rs`, which is in no scope

  # canon: {"schema":1,"at":"2026-11-06T05:02:06Z","actor":{"agent_id":"impl-code-scopes","session_id":"s-0-38-code-scopes"}}
  @subject:gate-trust-spine
  @case:happy
  @flow:canon-0-38-code-scopes.F3
  @gate.code-scopes.06
  Scenario: The ship guard, status and gate check ask for the same scenarios, paths and batch command
    Given the subject and the policy of the first scenario, and an approved chore that changed one file under `app/`
    When the subject is moved to shipped, `canon status` runs and `canon gate check` runs
    Then each names `app.bulkread.01` and `app.screen.01` and the path `app/main.rs`, and none names `core.engine.01`
    And each leads with the same batch command

  # canon: {"schema":1,"at":"2026-11-06T05:02:07Z","actor":{"agent_id":"impl-code-scopes","session_id":"s-0-38-code-scopes"}}
  @subject:gate-trust-spine
  @case:edge
  @flow:canon-0-38-code-scopes.F3
  @gate.code-scopes.07
  Scenario: The chore line says how many scenarios the scopes let go and by which keys, and a chore outside every scope asks for no evidence
    Given the subject and the policy of the first scenario
    When the approved chore changes only a file outside every scope, and `canon status`, `canon gate check` and the move to shipped run
    Then status says three scenarios need no new evidence by the keys `app` and `core` in `scopeExempt` of `canon status --json`
    And gate check names the chore and the keys `app` and `core` in the advisory `chore_code_scopes`
    And the move asks for the chore's review and no evidence, and for a chore that changed one app file `scopeExempt` of status says one scenario by the key `core`

  # canon: {"schema":1,"at":"2026-11-06T05:02:08Z","actor":{"agent_id":"impl-code-scopes","session_id":"s-0-38-code-scopes"}}
  @subject:gate-trust-spine
  @case:happy
  @flow:canon-0-38-code-scopes.F4
  @gate.code-scopes.08
  Scenario: No key, an area without a scope and evidence with no recorded tree are judged as in 0.37
    Given a policy without `code_scopes`, a policy that gives only `app` a scope, and a scenario whose evidence recorded no code tree
    When the approved chore changes a file outside every scope
    Then with no key all three scenarios are asked for, no changed path is named and status carries no `scopeExempt`
    And the area without a scope is asked for while the scoped area is let go
    And the scenario with no recorded tree is asked for although its scope is untouched

  # canon: {"schema":1,"at":"2026-11-06T05:02:09Z","actor":{"agent_id":"impl-code-scopes","session_id":"s-0-38-code-scopes"}}
  @subject:gate-trust-spine
  @case:failure
  @flow:canon-0-38-code-scopes.F4
  @gate.code-scopes.09
  Scenario: Evidence whose old tree cannot be had is asked for, never exempted
    Given a scenario whose evidence names a code tree that the commit it names does not give, and another whose evidence names a commit that is gone
    When the approved chore changes a file outside every scope
    Then both scenarios are asked for while the third is let go
    And the refusal names no path, because no difference was computed

  # canon: {"schema":1,"at":"2026-11-06T05:02:10Z","actor":{"agent_id":"impl-code-scopes","session_id":"s-0-38-code-scopes"}}
  @subject:gate-trust-spine
  @case:happy
  @flow:canon-0-38-code-scopes.F5
  @gate.code-scopes.10
  Scenario: A pattern that matches no file and a key that matches no scenario are named, and only where scopes are declared
    Given a policy whose `code_scopes` hold the pattern `nothing/**` that matches no file and the key `ghost` that matches no scenario
    When `canon status` and `canon gate check` run
    Then both warn about the pattern and the key, status with the code `CODE_SCOPES_UNMATCHED` and gate check as the advisory `code_scopes`
    And a repository that declares no scope, or only good ones, hears nothing

  # canon: {"schema":1,"at":"2026-11-06T05:02:11Z","actor":{"agent_id":"impl-code-scopes","session_id":"s-0-38-code-scopes"}}
  @subject:gate-trust-spine
  @case:failure
  @flow:canon-0-38-code-scopes.F5
  @gate.code-scopes.11
  Scenario: The warnings never block
    Given the policy of the warning scenario and the same repository without scopes
    When `canon status` and `canon gate check` run in both
    Then the exit codes are the same and gate check is clean
    And a `code_scopes` key that is not a map of lists is warned about in the advisory `code_scopes` and gate check still exits 0

  # canon: {"schema":1,"at":"2026-11-06T09:07:32Z","actor":{"agent_id":"impl-code-scopes","session_id":"s-0-38-code-scopes"}}
  @subject:gate-trust-spine
  @case:failure
  @flow:canon-0-38-code-scopes.F5
  @gate.code-scopes.14
  Scenario: A key whose patterns match no file declares no scope, so a misspelt pattern never exempts
    Given a policy whose `code_scopes` give `core` the pattern `Core/**` (matching is byte-exact), approved by the human
    When the approved chore changes `core/lib.rs` and the subject is moved to shipped, and `canon gate check` runs
    Then the refusal asks for `core.engine.01`, names no path and lets no scenario go (status carries no `scopeExempt`)
    And a change outside every real scope asks for it too, and the warning names the pattern `Core/**`
    And a key with one live pattern and one misspelt one still stands

  # canon: {"schema":1,"at":"2026-11-06T09:07:33Z","actor":{"agent_id":"impl-code-scopes","session_id":"s-0-38-code-scopes"}}
  @subject:gate-trust-spine
  @case:failure
  @flow:canon-0-38-code-scopes.F5
  @gate.code-scopes.15
  Scenario: An empty list is malformed: warned, and no scenario has a scope
    Given a policy whose `code_scopes` give `core` an empty list and `app` the pattern `app/**`
    When the approved chore changes a file outside every scope and the subject is moved to shipped, and `canon status` and `canon gate check` run
    Then all three scenarios are asked for and none is let go
    And `canon gate check` warns about `core` and `canon status` warns with `CODE_SCOPES_UNMATCHED`

  # canon: {"schema":1,"at":"2026-11-06T09:07:34Z","actor":{"agent_id":"impl-code-scopes","session_id":"s-0-38-code-scopes"}}
  @subject:gate-trust-spine
  @case:failure
  @flow:canon-0-38-code-scopes.F1
  @gate.code-scopes.16
  Scenario: Approving a brief does not approve a policy whose scopes changed since its last approval
    Given a policy approved by the human, then edited to widen the scope of `core`, with an unapproved chore brief
    When `canon brief approve` approves the chore
    Then the brief is approved, the policy approval file is as it was, and the output names `core` alone (`--json`: `policyApproved` false and `policyHeld` lists `core`)
    And the move to shipped fails closed with `POLICY_APPROVAL_STALE` until a human runs `canon policy approve`
    And a first policy approval, and a brief approval over unchanged scopes, record the policy approval as before

  # canon: {"schema":1,"at":"2026-11-06T09:25:24Z","actor":{"agent_id":"impl-code-scopes","session_id":"s-0-38-code-scopes"}}
  @subject:gate-trust-spine
  @case:failure
  @flow:canon-0-38-code-scopes.F3
  @gate.code-scopes.17
  Scenario: Over a change in core the ship guard, status and gate check ask for every scenario alike, so no one of them lets one go
    Given the subject and the policy of the first scenario, and an approved chore that changed one file under `core/`
    When the subject is moved to shipped, `canon status` runs and `canon gate check` runs
    Then each names all three scenarios and the path `core/lib.rs`, and the chore's status carries no `scopeExempt`
    And each leads with the same batch command over every report of the suite
