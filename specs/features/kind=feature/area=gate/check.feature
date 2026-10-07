Feature: gate check
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:gate-trust-spine
  @gate.check.01
  Scenario: A repository with nothing to complain about gates green
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository carrying no policy and no ledger records at all
    When the gate checks it
    Then it exits zero
    And the report reads clean rather than reporting nothing at all

  @subject:gate-trust-spine
  @gate.check.02
  Scenario: A policy-required cell with no evidence is gate-red and names its subject
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a policy that routes every task to a reviewer
    And a ledger holding only an implementer record for that task
    When the gate checks the repository
    Then it exits one, gate-red rather than a usage failure
    And the report carries uncovered-cell against that task id

  @subject:gate-trust-spine
  @gate.check.03
  Scenario: Each of the eight failure classes fires on its own fixture and only there
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given the shipped fixture corpus, one fixture per failure class
    When every fixture is evaluated against its own recorded expectations
    Then each fixture produces exactly its class and nothing else
    And no fixture is missing an expected violation or carrying an extra one
    And the whole selftest exits zero

  @subject:gate-trust-spine
  @gate.check.04
  Scenario: The failure-class vocabulary is closed at eight grep-stable strings
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a hook or a fixture that matches violations by substring rather than by type
    When the published wire strings are compared against the classes the gate raises
    Then the eight strings are that set, in that order, with no duplicates
    And every string parses back to the class it names
    And a plausible-looking class name the gate never raises does not parse

  @subject:gate-trust-spine
  @gate.check.05
  Scenario: The release-scoped trust requirement never fires on an ordinary run
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a reviewed record tagged with a class whose policy demands human trust, and no matching review record
    When the gate checks the repository once plainly and once as a release
    Then both runs gate-red on unreviewed-promotion
    And only the release run can report trust-below-required
    And the release run still evaluates the always-on trust ladder rather than replacing it

  @subject:gate-trust-spine
  @gate.check.06
  Scenario: Run from a subdirectory the gate answers for the nearest ancestor repository
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository whose root declares its ledger and whose policy requires a reviewer
    And a nested subdirectory two levels below that root
    When the gate is checked from the subdirectory with no repository named
    Then it resolves the ancestor root, not the directory it was invoked in
    And it reports that root's uncovered-cell violation, not a subdirectory-relative default

  @subject:gate-trust-spine
  @gate.check.07
  Scenario: An absent policy resolves to documented defaults and says it is missing
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository with no policy file
    When policy is resolved for it
    Then the resolution is not clean, and carries a diagnostic naming the file as missing
    And no trust level is required of anything
    And the staleness ceiling and surface scoping still answer, at their documented defaults

  @subject:gate-trust-spine
  @gate.check.08
  Scenario: A policy edit alone tightens the gate with no artifact touched
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given an evidence corpus that satisfies a policy requiring only an implementer
    When the policy grows to require a reviewer as well, and nothing else changes
    Then the same corpus now reports uncovered-cell against that task
    And the violation is attributable to the policy diff, since no record was added or edited

  @subject:gate-trust-spine
  @gate.check.09
  Scenario: A record whose own fields do not parse is malformed-evidence, not silence
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a ledger record present in the corpus but carrying a native field of the wrong type
    When the gate checks the repository
    Then the record is reported as malformed-evidence
    And it is caught once where the corpus is read, not re-validated by each check

  # canon: {"schema":1,"at":"2026-09-16T11:04:54Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @lane:behavior
  @gate.check.10
  Scenario: A scenario in an excluded lane is outside coverage
    Given spec coverage is enabled with one lane listed under exclude_lanes
    And an in-scope scenario in that lane has no evidence
    When the gate runs
    Then that scenario produces no uncovered-cell violation
    And a scenario in an unlisted lane with no evidence still does

  # canon: {"schema":1,"at":"2026-09-16T11:04:54Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @lane:behavior
  @gate.check.11
  Scenario: A non-slug entry in exclude_lanes poisons the section rather than vanishing
    Given a spec_coverage section whose exclude_lanes carries a value that is not a kebab-case slug
    When the policy is resolved
    Then the section is reported unusable, naming the value
    And the gate refuses rather than treating the section as absent

  @subject:gate-trust-spine
  @gate.check.12
  Scenario: A matching high risk tier requires distinct human approvals
  # canon: {"schema":1,"at":"2026-10-01T00:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a risk_tiers policy matching src/auth/** with two human approvals required
    And an evidence record for that path with no approval attestation
    When the gate checks the repository
    Then it reports uncovered-cell for the existing evidence subject
    And the detail names the high tier, minimum, and observed approval count
    And an approval whose role is agent does not satisfy the tier

  @subject:gate-trust-spine
  @gate.check.13
  Scenario: A high risk tier requires distinct verified SSH signers
  # canon: {"schema":1,"at":"2026-10-01T00:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a high tier matching src/auth/** with two approvals required
    When two evidence records carry the same verified SSH signer identity
    Then the tier remains uncovered
    And two distinct verified SSH signers satisfy it
