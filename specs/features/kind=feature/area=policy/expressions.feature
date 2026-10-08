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
