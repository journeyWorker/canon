Feature: gate check
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:gate-trust-spine
  @case:edge
  @gate.check.01
  Scenario: A repository with nothing to complain about gates green
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository carrying no policy and no ledger records at all
    When the gate checks it
    Then it exits zero
    And the report reads clean rather than reporting nothing at all

  @subject:gate-trust-spine
  @case:failure
  @gate.check.02
  Scenario: A policy-required cell with no evidence is gate-red and names its subject
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a policy that routes every task to a reviewer
    And a ledger holding only an implementer record for that task
    When the gate checks the repository
    Then it exits one, gate-red rather than a usage failure
    And the report carries uncovered-cell against that task id

  @subject:gate-trust-spine
  @case:failure
  @gate.check.03
  Scenario: Each of the ten failure classes fires on its own fixture and only there
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given the shipped fixture corpus, one fixture per failure class
    When every fixture is evaluated against its own recorded expectations
    Then each fixture produces exactly its class and nothing else
    And no fixture is missing an expected violation or carrying an extra one
    And the whole selftest exits zero

  @subject:gate-trust-spine
  @case:edge
  @gate.check.04
  Scenario: The failure-class vocabulary is closed at ten grep-stable strings
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a hook or a fixture that matches violations by substring rather than by type
    When the published wire strings are compared against the classes the gate raises
    Then the ten strings are that set, in that order, with no duplicates
    And every string parses back to the class it names
    And a plausible-looking class name the gate never raises does not parse

  @subject:gate-trust-spine
  @case:failure
  @gate.check.05
  Scenario: The release-scoped trust requirement never fires on an ordinary run
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a reviewed record tagged with a class whose policy demands human trust, and no matching review record
    When the gate checks the repository once plainly and once as a release
    Then both runs gate-red on unreviewed-promotion
    And only the release run can report trust-below-required
    And the release run still evaluates the always-on trust ladder rather than replacing it

  @subject:gate-trust-spine
  @case:edge
  @gate.check.06
  Scenario: Run from a subdirectory the gate answers for the nearest ancestor repository
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository whose root declares its ledger and whose policy requires a reviewer
    And a nested subdirectory two levels below that root
    When the gate is checked from the subdirectory with no repository named
    Then it resolves the ancestor root, not the directory it was invoked in
    And it reports that root's uncovered-cell violation, not a subdirectory-relative default

  @subject:gate-trust-spine
  @case:edge
  @gate.check.07
  Scenario: An absent policy resolves to documented defaults and says it is missing
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository with no policy file
    When policy is resolved for it
    Then the resolution is not clean, and carries a diagnostic naming the file as missing
    And no trust level is required of anything
    And the staleness ceiling and surface scoping still answer, at their documented defaults

  @subject:gate-trust-spine
  @case:failure
  @gate.check.08
  Scenario: A policy edit alone tightens the gate with no artifact touched
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given an evidence corpus that satisfies a policy requiring only an implementer
    When the policy grows to require a reviewer as well, and nothing else changes
    Then the same corpus now reports uncovered-cell against that task
    And the violation is attributable to the policy diff, since no record was added or edited

  @subject:gate-trust-spine
  @case:failure
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
  @case:edge
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
  @case:failure
  @gate.check.11
  Scenario: A non-slug entry in exclude_lanes poisons the section rather than vanishing
    Given a spec_coverage section whose exclude_lanes carries a value that is not a kebab-case slug
    When the policy is resolved
    Then the section is reported unusable, naming the value
    And the gate refuses rather than treating the section as absent

  @subject:gate-trust-spine
  @case:failure
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
  @case:failure
  @gate.check.13
  Scenario: A high risk tier requires distinct verified SSH signers
  # canon: {"schema":1,"at":"2026-10-01T00:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a high tier matching src/auth/** with two approvals required
    When two evidence records carry the same verified SSH signer identity
    Then the tier remains uncovered
    And two distinct verified SSH signers satisfy it

  # canon: {"schema":1,"at":"2026-10-08T14:02:45Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @case:failure
  @gate.check.14
  Scenario: A surface that specifies only its golden path is reported when failure cases are required
    Given a spec_coverage policy that requires the failure case
    And one surface whose scenarios are all tagged happy and another with a failure scenario
    When the gate checks the repository
    Then it reports uncovered-cell for the golden-path-only surface naming the missing case
    And an untagged or out-of-scope failure scenario does not satisfy its surface

  # canon: {"schema":1,"at":"2026-10-08T15:13:25Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @case:edge
  @gate.check.15
  Scenario: Experimental evidence binding is off by default, advisory in warn and enforced in require
    Given scenario evidence that carries no bound file
    When the gate checks a repository without the experimental section
    Then nothing about binding is checked or printed
    And in warn mode the scenario is listed as an advisory while the gate still passes
    And in require mode it is an uncovered cell until a passing report is bound

  # canon: {"schema":1,"at":"2026-10-08T15:13:25Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @case:failure
  @gate.check.16
  Scenario: A malformed evidence binding section fails the gate instead of reading as off
    Given an experimental evidence binding section with an unknown mode or key
    When the policy is resolved
    Then the section is invalid and the gate reports it as an uncovered cell
    And the spec coverage section beside it still resolves

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @case:failure
  @gate.check.17
  Scenario: With require_review the gate reports unreviewed and open-blocker gaps on a subject claimed done
    Given a verifying subject whose scenario carries only its implementer's evidence and no review
    And an open blocker finding on a change the subject adopted
    And a spec_coverage policy whose require_review section takes its defaults
    When the gate checks the repository
    Then it reports unreviewed-promotion naming the scenario and the missing review
    And it reports open-blocker naming the change, round and seq of the finding

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @case:happy
  @gate.check.18
  Scenario: require_review takes its defaults from an empty section and resolves each field on its own
    Given a spec_coverage section with an empty require_review mapping
    When the policy is resolved
    Then review is required at verifying and shipped, by a distinct actor, with open blockers blocking
    And setting scope, distinct_actor or block_on_findings changes only that field
    And a section without require_review resolves exactly as it did before

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @case:failure
  @gate.check.19
  Scenario: A malformed require_review fails the gate instead of reading as off
    Given a require_review section with an unknown status, an unknown key, a null or a scalar
    When the policy is resolved
    Then the whole spec_coverage section is invalid and names the problem
    And the gate reports it rather than skipping review

  # canon: {"schema":1,"at":"2026-10-10T15:34:48Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @case:edge
  @gate.check.20
  Scenario: The gate says when spec coverage is off without changing its exit code
    Given a spec corpus with scenarios and a policy without a spec_coverage section
    When the gate checks the repository
    Then it prints one advisory on stdout naming the scenario count and .canon/policy.yaml
    And the exit code is what it would be without the advisory

  # canon: {"schema":1,"at":"2026-10-10T15:34:48Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @case:edge
  @gate.check.21
  Scenario: The coverage-off advisory is silent with the section present or no scenarios
    Given a spec_coverage section, even one with require_evidence false, or a corpus with no scenarios
    When the gate checks the repository
    Then no coverage-off advisory is printed

  # canon: {"schema":1,"at":"2026-10-10T15:43:50Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @case:failure
  @gate.check.22
  Scenario: A bound file whose bytes are gone from both the store and the working tree is stale evidence
    Given a committed record bound to a report, whose stored blob is deleted
    When the report is rewritten in the working tree
    Then the gate exits 1 with stale-evidence for the scenario
    And the line names the path, the recorded digest, the missing blob and the working tree's new digest

  # canon: {"schema":1,"at":"2026-10-10T15:43:50Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @case:edge
  @gate.check.23
  Scenario: An unstored binding the working tree still proves is an advisory, not a violation
    Given a committed record bound to a report that has no stored blob
    When the working-tree report still hashes to the recorded digest
    Then the gate stays clean and lists the binding as unstored, pointing to canon evidence vault

  # canon: {"schema":1,"at":"2026-10-10T18:33:15Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @case:failure
  @gate.check.24
  Scenario: An altered stored blob is stale evidence that says it was altered, not gone
    Given a committed record whose stored blob's bytes were changed while the working-tree report still matches
    When the gate checks the repository
    Then the gate exits 1 with stale-evidence naming the blob's actual digest
    And the line says the stored blob does not match its name, never that the bound bytes are gone

  # canon: {"schema":1,"at":"2026-10-10T19:35:33Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @case:failure
  @gate.check.60
  Scenario: A refuted latest audit is false-success even beside a faithful declared verdict
    Given a scenario with a faithful declared verdict and a refuted latest audit
    When the gate is checked
    Then it exits 1 with false-success naming the evaluator and the counterexample path
    And a later faithful declared re-attest does not clear it
    And moving the owning subject to shipped is refused with false-success

  # canon: {"schema":1,"at":"2026-10-10T19:35:33Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @case:happy
  @gate.check.61
  Scenario: A newer confirmed audit clears false-success and lets the subject ship
    Given a scenario whose latest audit is refuted
    When a newer confirmed audit is recorded, binding only what it names
    Then the gate is clean and the owning subject can ship

  # canon: {"schema":1,"at":"2026-10-10T19:35:33Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @case:failure
  @gate.check.62
  Scenario: A scenario the require_audit sample selects is uncovered-cell until a decided audit exists
    Given spec_coverage.require_audit with rate 0.5, seed canon-0.15 and every scenario in scope
    When the gate is checked
    Then exactly the scenarios whose sha256(seed, 0x00, project, 0x00, scenario) first 8 bytes fall below rate times 2^64 are uncovered-cell with an audit detail
    And an indeterminate latest audit still owes one, and a confirmed audit clears it
    And canon status lists the owed audits

  # canon: {"schema":1,"at":"2026-10-10T19:35:33Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @case:edge
  @gate.check.63
  Scenario: require_audit scope defaults to claimed-done subjects and its rate bounds are inclusive
    Given require_audit with rate 1 and no scope
    When the owning subject is building, then verifying
    Then the gate is clean while building and red while verifying, and the ship guard refuses the owed audit
    And with rate 0 nothing is sampled

  # canon: {"schema":1,"at":"2026-10-10T19:35:33Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @case:failure
  @gate.check.64
  Scenario: A malformed require_audit poisons the spec_coverage section
    Given require_audit with a rate out of range, no rate, an unknown key, a string rate, an unknown scope status, or null
    When the gate is checked
    Then it exits 1 with spec_coverage unusable, naming the problem

  # canon: {"schema":1,"at":"2026-10-10T19:30:46Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @case:edge
  @gate.check.40
  Scenario: A review that predates 0.23 pinning does not count and the gate says canon migrate
    Given require_review and a review written before 0.23, pinned to nothing or to evidence record files
    When `canon gate check` runs
    Then the gate exits 1 with unreviewed-promotion on that scenario, saying the review predates 0.23 review pinning and that `canon migrate` rewrites it
    And one advisory line counts the scenarios whose review is not migrated, and `canon status` and `canon subject status` print the same advisory

  # canon: {"schema":1,"at":"2026-10-10T20:02:32Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @case:edge
  @gate.check.65
  Scenario: The false-success line names the counterexamples and a new audit replaces the refutation
    Given a scenario whose latest audit is refuted and binds a counterexample report whose path has a space and a quote
    When the gate is checked and then a confirmed audit is recorded that names no file
    Then the false-success line names the counterexample path and no drop flag
    And the confirmed audit binds nothing from the refutation and the gate is clean

  # canon: {"schema":1,"at":"2026-10-11T15:01:53Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @case:edge
  @gate.check.80
  Scenario: Gate check re-indexes only the edited feature file and is silent when the index is fresh
    Given a synced scenario index and an edited feature file
    When gate check runs
    Then one stderr line says the index was refreshed from that one file
    And only the edited scenario gets a new record
    And a later inventory sync writes nothing
    And the next reader prints no refresh line

  # canon: {"schema":1,"at":"2026-10-12T00:00:00Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @case:happy
  @gate.check.90
  Scenario: canon migrate pins a review written before 0.15 that is still current, and it counts afterwards
    Given require_review and reviews written before 0.15 whose pin is the commit they reviewed, and the scenarios unchanged since
    When `canon gate check` runs, then `canon migrate`, then `canon gate check` again
    Then the first gate exits 1 and names `canon migrate`
    And migrate rewrites each review with the verdict and checks of its scenario's latest evidence, keeping its reviewer
    And the second gate exits 0, and a second migrate finds nothing to convert

  # canon: {"schema":1,"at":"2026-10-12T00:00:00Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @case:failure
  @gate.check.91
  Scenario: canon migrate leaves a review stale when its scenario changed after the commit it pinned
    Given require_review and reviews written before 0.15 of two scenarios, pinned to the commit they reviewed
    When one scenario's steps change in a later commit and `canon migrate` runs
    Then the unchanged scenario's review is pinned and counts, and the changed one's review is left as it was
    And `canon gate check` exits 1 with unreviewed-promotion on the changed scenario only

  # canon: {"schema":1,"at":"2026-10-12T00:00:00Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @case:failure
  @gate.check.92
  Scenario: canon migrate leaves a review stale when its scenario has evidence newer than the review
    Given require_review and a review written before 0.15 of a scenario unchanged since its pinned commit
    When evidence for that scenario is recorded after the review and `canon migrate` runs
    Then the review is left as it was and `canon gate check` exits 1 with unreviewed-promotion on that scenario

  # canon: {"schema":1,"at":"2026-10-12T00:00:00Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @case:edge
  @gate.check.93
  Scenario: canon migrate pins a review whose commit or scenario is not in the clone as the scenario stands
    Given require_review and reviews written before 0.15, one pinned to a commit this clone does not have and one to a commit that predates its scenario
    When `canon migrate` runs and then `canon gate check`
    Then both reviews are pinned and count and the gate exits 0, never reporting a missing commit as a violation
    And migrate counts them as unchecked and suggests checking out full history with fetch-depth 0

  # canon: {"schema":1,"at":"2026-10-12T00:00:00Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @case:edge
  @gate.check.94
  Scenario: canon migrate finds a review's scenario in the file it lived in at the pinned commit
    Given require_review and reviews written before 0.15 pinned to a commit where their scenarios lived in another feature file
    When the scenarios have since moved, unchanged, to a new file and `canon migrate` runs
    Then the scenarios are found in the old file at the pinned commit
    And both reviews are pinned and the gate exits 0

  # canon: {"schema":1,"at":"2026-10-12T11:44:52Z","actor":{"agent_id":"canon"}}
  @subject:gate-trust-spine
  @case:edge
  @gate.check.96
  Scenario: A moved spec root is searched at its former path and only for its own project
    Given a spec root moved from specs to .canon/specs with former_roots [specs], and a second root declaring the same scenario ids with the same text
    And reviews written before 0.15 for both projects, pinned to a commit from before the move, when only the first project existed
    When canon migrate runs
    Then the first project's review is found at its former root and pinned as current
    And the second project's review is undecidable and pinned as the scenario stands, never matched against the first project's scenario

  # canon: {"schema":1,"at":"2026-10-21T09:00:03Z","actor":{"agent_id":"canon-impl-0241"}}
  @subject:gate-trust-spine
  @case:happy
  @gate.check.97
  Scenario: A fixed blocker that names its scenarios is re-verified by evidence for those scenarios alone
    Given a subject with scenarios A and B, both evidenced and reviewed, and a blocker naming only A closed fixed after that evidence
    When the subject is moved to shipped, and gate check, subject status and status are read
    Then each refuses naming A only, and the remedy is the per-scenario canon evidence add --scenario-id, never the whole-suite batch
    And evidence for B recorded after the close does not clear it, and evidence for A recorded before the close does not either
    And evidence for A recorded after the close clears gate check and the move, while B's evidence stays older than the close

  # canon: {"schema":1,"at":"2026-10-21T09:00:04Z","actor":{"agent_id":"canon-impl-0241"}}
  @subject:gate-trust-spine
  @case:edge
  @gate.check.98
  Scenario: A fixed blocker that names no scenario still asks every evidenced scenario
    Given a subject with scenarios A and B, both evidenced, and a blocker naming no scenario closed fixed after that evidence
    When evidence is recorded after the close for A only
    Then the move to shipped is refused naming B, with the whole-suite batch as the remedy
    And once B has evidence after the close the move succeeds
