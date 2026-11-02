Feature: atip crate: plan, JCS digest, commit gate, commit argv
  # canon: {"schema":1,"at":"2026-11-02T12:37:32Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}

  # canon: {"schema":1,"at":"2026-11-02T12:37:41Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-crate-plan.F1
  @atip.crateplan.01
  Scenario: The digest is sha256: and the SHA-256 of the RFC 8785 text of the effects and inputs
    Given a plan of two effects and inputs of arguments and a state digest, and the same text written by hand with every object's members sorted
    When the digest is made
    Then it is sha256: and 64 lower-case hex digits, the SHA-256 of that text, and the empty plan is the digest of the empty effects and inputs

  # canon: {"schema":1,"at":"2026-11-02T12:39:38Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:edge
  @flow:canon-0-35-crate-plan.F1
  @atip.crateplan.02
  Scenario: The order of the inputs does not change the digest and every other change does
    Given a plan, the same plan with the keys of its inputs in another order, and plans that change an effect, the order of the effects, one input character, a null or an empty array
    When their digests are made
    Then the reordered inputs have the same digest and every other plan has another

  # canon: {"schema":1,"at":"2026-11-02T12:40:34Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-plan.F1
  @atip.crateplan.03
  Scenario: A number a digest cannot hold is an error
    Given the numbers 1.5, 9007199254740993, 1e300 and -0.1 in inputs or in an effect, and the largest safe integer
    When the plan's digest is made
    Then each unsafe number is an error and the largest safe integer is accepted

  # canon: {"schema":1,"at":"2026-11-02T12:41:26Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-crate-plan.F2
  @atip.crateplan.04
  Scenario: The preview report has planDigest, effects and committed false and passes the crate's schema
    Given a plan with effects, some with a member of the tool's own
    When the preview is made and checked against preview_schema
    Then data has planDigest, effects and committed false and passes, committed true passes, and a report without a member or with another type does not

  # canon: {"schema":1,"at":"2026-11-02T12:42:15Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-crate-plan.F2
  @atip.crateplan.05
  Scenario: The commit step is the normalised arguments, commit, digest and json, and splits back
    Given a preview of a plan and the tool name acme
    When commit_next writes the first meta.next entry
    Then the argv has the arguments as options, --commit, --plan-digest and --json, the command is the tool name and the words quoted, splitting it gives the same words, and an envelope holding it validates

  # canon: {"schema":1,"at":"2026-11-02T12:43:03Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-plan.F2
  @atip.crateplan.06
  Scenario: Arguments from input or with a dash are written as options, and defaults, env values and input are not written
    Given arguments that read --input -, a value -x, an empty value, a default, an environment value, a stale commit and plan-digest and a repeated json
    When commit_next writes the step
    Then every valued option is --name=value, none of --input, the environment value, the default or the stale options is written, and json and plan-digest appear once

  # canon: {"schema":1,"at":"2026-11-02T12:44:17Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-crate-plan.F3
  @atip.crateplan.07
  Scenario: The same digest commits and a call without the options is a preview
    Given a commit with the digest the recomputed plan has, and a call with neither option
    When the gate decides
    Then the commit passes, and the call without options is a preview that never compares digests

  # canon: {"schema":1,"at":"2026-11-02T12:44:45Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-plan.F3
  @atip.crateplan.08
  Scenario: Another digest is PLAN_STALE exit 2 with the preview as next
    Given a commit whose digest differs in one character, lacks the sha256: prefix, is another plan's, is empty, is upper-cased or has a space
    When the gate compares it with the recomputed digest
    Then each is PLAN_STALE exit 2 not retryable with field --plan-digest, and the next is the preview call without the commit options

  # canon: {"schema":1,"at":"2026-11-02T12:45:01Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:edge
  @flow:canon-0-35-crate-plan.F3
  @atip.crateplan.09
  Scenario: A digest another system computed is compared as it is
    Given a digest computed by a remote system in a format of its own
    When it is committed through the gate with the digest the system says is current
    Then the gate compares it as it is and passes it, and another digest is PLAN_STALE

  # canon: {"schema":1,"at":"2026-11-02T12:45:23Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-plan.F4
  @atip.crateplan.10
  Scenario: One option without the other is USAGE or the finer code with the missing field
    Given --commit alone, --plan-digest alone, both, and neither, and a tool that registered a finer code
    When mode reads them and the error of an unpaired option is made
    Then each lone option is refused as USAGE or the finer code with the field the missing option, and both or neither is not refused

  # canon: {"schema":1,"at":"2026-11-02T12:45:39Z","actor":{"agent_id":"impl-crate-0-35","role":"implementer","session_id":"s-0-35-crate"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-crate-plan.F5
  @atip.crateplan.11
  Scenario: A changed plan after some effects is STOPPED_MIDWAY with the reason PLAN_STALE
    Given a commit that made 2 of 5 effects and then met a changed plan, and the command that reads what was done
    When stopped_by_stale builds the stop
    Then the error is STOPPED_MIDWAY exit 1, not retryable, with the reason PLAN_STALE, and the next leads with the command that reads what was done, never a retry
