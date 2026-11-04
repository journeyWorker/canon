Feature: atip: refusals are exit 2 and checks are exit 1
  # canon: {"schema":1,"at":"2026-11-04T02:35:46Z","actor":{"agent_id":"impl-codes-rename","role":"implementer","session_id":"s-0-36-codes-rename"}}

  # canon: {"schema":1,"at":"2026-11-04T02:35:47Z","actor":{"agent_id":"impl-codes-rename","role":"implementer","session_id":"s-0-36-codes-rename"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-exit.F1
  @atip.codeexit.01
  Scenario: A precondition that starts nothing is exit 2 with its code, no data, and the work tree and .canon/ are what they were
    Given a repository and each call that the table refuses before it starts
    When the call runs with --json
    Then it exits 2 with the new code and no data, and the work tree and .canon/ hold the bytes they held

  # canon: {"schema":1,"at":"2026-11-04T02:35:54Z","actor":{"agent_id":"impl-codes-rename","role":"implementer","session_id":"s-0-36-codes-rename"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-exit.F1
  @atip.codeexit.02
  Scenario: An approved brief takes the call, and a brief edited after its approval is BRIEF_APPROVAL_STALE
    Given a repository with an approved brief, and the same brief edited after its approval
    When evidence add runs on each
    Then the approved brief takes the call and the edited one exits 2 with BRIEF_APPROVAL_STALE, with nothing written

  # canon: {"schema":1,"at":"2026-11-04T02:35:59Z","actor":{"agent_id":"impl-codes-rename","role":"implementer","session_id":"s-0-36-codes-rename"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-exit.F2
  @atip.codeexit.03
  Scenario: A gate promote that meets two refused records is exit 2 with two details that carry their own codes and no data
    Given two staged records the gate refuses
    When gate promote runs
    Then it exits 2 with the first record's code, error.details holds two entries that each carry their own code, there is no data, and the records stay staged

  # canon: {"schema":1,"at":"2026-11-04T02:36:04Z","actor":{"agent_id":"impl-codes-rename","role":"implementer","session_id":"s-0-36-codes-rename"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-exit.F2
  @atip.codeexit.04
  Scenario: A subject move the gate blocks changes no status, and one cause is the error itself with no details
    Given a subject in verifying whose shipment the gate blocks for one cause
    When subject move --to shipped runs
    Then it exits 2 with the cause as the error itself, no details and no data, and the status is unchanged

  # canon: {"schema":1,"at":"2026-11-04T02:36:08Z","actor":{"agent_id":"impl-codes-rename","role":"implementer","session_id":"s-0-36-codes-rename"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-exit.F2
  @atip.codeexit.05
  Scenario: The gate of every record of an evidence batch is judged before the first write, and a refused record is exit 2 with its code and field /<index>
    Given an evidence batch whose second record the promotion gate refuses
    When evidence add runs on the batch
    Then it exits 2 with PROMOTION_INVALID and field /1, and nothing is staged, promoted or stored

  # canon: {"schema":1,"at":"2026-11-04T02:36:13Z","actor":{"agent_id":"impl-codes-rename","role":"implementer","session_id":"s-0-36-codes-rename"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-exit.F3
  @atip.codeexit.06
  Scenario: A check command that answers no is exit 1 with its FAILED code and the result in data
    Given an invalid adapter response, a denied authorization, a missing report and a pack changed after its creation
    When adapter validate, adapter authorize, report check and context-pack verify run
    Then each exits 1 with its FAILED code and the result in data

  # canon: {"schema":1,"at":"2026-11-04T02:36:18Z","actor":{"agent_id":"impl-codes-rename","role":"implementer","session_id":"s-0-36-codes-rename"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-exit.F3
  @atip.codeexit.07
  Scenario: A check command that cannot start is exit 2 and not FAILED
    Given the same four commands with no input file, no canon.yaml or no such pack
    When each runs
    Then each exits 2 with its refusal code and none is a FAILED code

  # canon: {"schema":1,"at":"2026-11-04T02:36:23Z","actor":{"agent_id":"impl-codes-rename","role":"implementer","session_id":"s-0-36-codes-rename"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-exit.F4
  @atip.codeexit.08
  Scenario: A command that cannot start is exit 2 with its new code and writes nothing
    Given a repository with no git on the PATH, a plan source with no usable construct, a taken port and a repository with no dashboard app
    When each command runs
    Then each exits 2 with its new code, no data, and nothing written

  # canon: {"schema":1,"at":"2026-11-04T02:36:28Z","actor":{"agent_id":"impl-codes-rename","role":"implementer","session_id":"s-0-36-codes-rename"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-exit.F4
  @atip.codeexit.09
  Scenario: The retry values of the commands that cannot start are the table's
    Given the registry rows of the five commands that cannot start
    When their retry values are read
    Then the port and the closing run are retryable and the other three are not

  # canon: {"schema":1,"at":"2026-11-04T02:36:33Z","actor":{"agent_id":"impl-codes-rename","role":"implementer","session_id":"s-0-36-codes-rename"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-exit.F5
  @atip.codeexit.10
  Scenario: Every row of the exit table is the registry's
    Given the exit table of the brief and the registry
    When each row is compared
    Then every new code exists with the new exit, every old code is gone, and the CHANGELOG collects the codes that changed from exit 1 to exit 2

  # canon: {"schema":1,"at":"2026-11-04T02:36:38Z","actor":{"agent_id":"impl-codes-rename","role":"implementer","session_id":"s-0-36-codes-rename"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-exit.F5
  @atip.codeexit.11
  Scenario: No branch of canon's own on exit 1 depends on a code that changed
    Given the pre-commit hook, the npm launcher and the scan for a branch on exit 1
    When the shipped sources are scanned
    Then none compares an exit status with 1, and the scan finds such a branch in a sample

  # canon: {"schema":1,"at":"2026-11-04T02:36:43Z","actor":{"agent_id":"impl-codes-rename","role":"implementer","session_id":"s-0-36-codes-rename"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-exit.F5
  @atip.codeexit.12
  Scenario: A refusal built with the refuse helper or a gate cause literal is matched against the registry
    Given a source that builds a refusal with the refuse helper or a gate cause literal
    When the registry-match scan reads the sources
    Then a code that is not registered is found, in both shapes
