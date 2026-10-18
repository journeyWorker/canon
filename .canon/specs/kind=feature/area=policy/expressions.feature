Feature: policy expressions
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:policy-expressions
  @case:failure
  @policy.expressions.01
  Scenario: A misspelled field is refused at write time and the refusal names what was expected
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given an expression bound to the task record kind
    When it references a field the kind never declares
    Then compilation fails with exactly one diagnostic, an undeclared field
    And the diagnostic names the offending field and lists the fields that were expected
    And the author is never left guessing which identifier was wrong

  @subject:policy-expressions
  @case:failure
  @policy.expressions.02
  Scenario: A function outside the allowlist is refused, named
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given an expression bound to a record kind
    When it calls a function the binding set does not allowlist
    Then compilation fails with an unknown-function diagnostic carrying that function's name
    And the expression language stays closed rather than inheriting whatever the host CEL runtime offers

  @subject:policy-expressions
  @case:failure
  @policy.expressions.03
  Scenario: An incompatible comparison is refused at write time, not at evaluation
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a field whose type the schema resolves statically
    When an expression compares it against a literal of an incompatible type
    Then compilation fails with an operator type mismatch naming the operator
    And the mismatch is caught when the policy is written, never deferred to the run that needed the verdict

  @subject:policy-expressions
  @case:edge
  @policy.expressions.04
  Scenario: A field whose shape is genuinely unknown stays deferred rather than over-rejected
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a record field that resolves to open JSON, so its inner shape is unknowable at write time
    When an expression selects a member off that field
    Then compilation accepts it, deferring the question to evaluation
    And the write-time checker refuses only what it can prove wrong

  @subject:policy-expressions
  @case:happy
  @policy.expressions.05
  Scenario: The flat form and the expression form of a policy section resolve the same answer
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given one repo whose policy states its trust requirements as flat values
    And a second repo stating the equivalent requirements as bound expressions
    When both are resolved against every verdict a record can carry
    Then the resolved requirement is identical between the two at every one of them
    And an expression is a way of writing a requirement, never a second policy semantics

  @subject:policy-expressions
  @case:failure
  @policy.expressions.06
  Scenario: One unusable expression is dropped with a diagnostic and its siblings still resolve
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a policy section carrying a flat requirement beside a bound expression that cannot compile
    When the policy is resolved
    Then the unusable entry is dropped and recorded as a diagnostic
    And every other entry in the same section still resolves
    And a single bad predicate never silences the whole policy

  @subject:policy-expressions
  @case:edge
  @policy.expressions.07
  Scenario: An absent optional section resolves to its documented default, not a poisoned value
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a policy file that declares trust requirements and nothing else
    When it is resolved
    Then the optional coverage section resolves as absent
    And absence is distinguishable from a section that was present and broken

  @subject:policy-expressions
  @case:failure
  @policy.expressions.08
  Scenario: A misspelled key inside an opt-in section poisons it rather than being ignored
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a policy file whose coverage section spells one of its keys wrong
    When it is resolved
    Then the section resolves as invalid, not as its default
    And the typo can never quietly disable a requirement the author believed they had turned on

  @subject:policy-expressions
  @case:failure
  @policy.expressions.09
  Scenario: A poisoned section does not discard the rest of the file
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a policy file carrying a valid trust requirement and a coverage section with an illegal value
    When it is resolved
    Then the coverage section resolves as invalid
    And the trust requirement beside it survives unchanged
    And the failure is scoped to the section that caused it

  @subject:policy-expressions
  @case:happy
  @policy.expressions.10
  Scenario: Risk tiers remain declarative and do not extend the closed CEL profile
  # canon: {"schema":1,"at":"2026-10-01T00:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a policy with path and effect matchers under risk_tiers
    When the policy is resolved
    Then rank selects the highest matching tier
    And malformed tier entries produce explicit diagnostics and are not enabled
    And the risk_tiers section remains declarative, adding no CEL expression or InvalidPredicate diagnostic

  # canon: {"schema":1,"at":"2026-10-10T19:28:02Z","actor":{"agent_id":"canon"}}
  @subject:policy-expressions
  @case:happy
  @policy.expressions.30
  Scenario: Approving the policy records its digest, approver, time and note
    Given a repository with a .canon/policy.yaml
    When a human runs canon policy approve --by <who> --note <text>
    Then .canon/policy.approval.json records the sha256 of the policy bytes, the approver, the time and the note
    And the success line says the file was written directly, with nothing to promote
    And neither canon's .gitignore nor the one canon init writes ignores the approval file

  # canon: {"schema":1,"at":"2026-10-10T19:28:02Z","actor":{"agent_id":"canon"}}
  @subject:policy-expressions
  @case:failure
  @policy.expressions.31
  Scenario: Approving a repo with no policy is refused
    Given a repository with no .canon/policy.yaml
    When canon policy approve runs
    Then it is refused with exit 2
    And no approval file is written

  # canon: {"schema":1,"at":"2026-10-10T19:28:02Z","actor":{"agent_id":"canon"}}
  @subject:policy-expressions
  @case:edge
  @policy.expressions.32
  Scenario: The gate fails on a policy changed since its approval until a human approves the change
    Given a repository with a policy
    When canon gate check runs before any approval, after an approval, after the policy changes, and after a human approves the change
    Then it prints a policy-not-approved advisory and exits 0, then is clean, then exits 1 with an unapproved-policy violation, then is clean again
    And the violation names the policy path, the current and the approved digest, the approver and the time, and canon policy approve --by <who> or canon brief approve
    And with --json the violation's class is unapproved-policy

  # canon: {"schema":1,"at":"2026-10-16T17:40:21Z","actor":{"agent_id":"canon-022-p1-impl"}}
  @subject:policy-expressions
  @case:failure
  @policy.expressions.33
  Scenario: An approval file that does not read fails the gate
    Given a repository whose .canon/policy.approval.json does not parse as an approval
    When canon gate check runs
    Then it exits 1 with an unapproved-policy violation naming the approval file and the parse error
    And once a human runs canon policy approve the gate is clean

  # canon: {"schema":1,"at":"2026-10-16T17:40:21Z","actor":{"agent_id":"canon-022-p1-impl"}}
  @subject:policy-expressions
  @case:happy
  @policy.expressions.34
  Scenario: A policy never approved stays an advisory
    Given a repository with a policy and no approval record, and a subject ready to ship
    When canon gate check, canon status and canon subject status --to shipped run
    Then the gate exits 0 with a policy-not-approved advisory naming canon policy approve
    And status keeps the warning and no human step
    And the subject ships

  # canon: {"schema":1,"at":"2026-10-16T17:40:21Z","actor":{"agent_id":"canon-022-p1-impl"}}
  @subject:policy-expressions
  @case:happy
  @policy.expressions.35
  Scenario: canon policy show reads back the approval state
    Given a repository with no policy, then a policy never approved, approved, changed after its approval, and with an unreadable approval file
    When canon policy show runs, with and without --json
    Then it exits 0 each time and reports the state none, unapproved, approved, changed and unreadable
    And it names the policy path and its current digest, and the approver, time, note and approved digest when an approval reads
    And canon policy approve names canon policy show as its next step
