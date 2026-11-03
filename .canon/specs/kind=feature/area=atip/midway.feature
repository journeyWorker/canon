Feature: atip: STOPPED_MIDWAY, the code of a command that stopped after its earlier steps wrote
  # canon: {"schema":1,"at":"2026-11-02T09:39:12Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}

  # canon: {"schema":1,"at":"2026-11-02T09:39:28Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-canon-stopped-midway.F1
  @atip.midway.01
  Scenario: STOPPED_MIDWAY is registered at exit 1, not retryable, its reasons are exit-2 codes and both schema views agree
    Given canon's error registry
    When canon schema --errors --json and canon schema --error STOPPED_MIDWAY --json are read
    Then both give one row with exit 1, not retryable, and the same reasons, each the name of a registered exit-2 code with a meaning and a fix

  # canon: {"schema":1,"at":"2026-11-02T09:39:56Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-canon-stopped-midway.F1
  @atip.midway.02
  Scenario: A reason that is not a registered exit-2 code, or a row with another exit or retryability, fails the registry check
    Given the registry with a reason that names no code, an exit-1 code or unsorted names, and with the row at exit 2 or retryable
    When the registry check runs and an error that names a reason the registry does not hold is rendered
    Then each fault is named and the check fails, and rendering the unregistered reason fails

  # canon: {"schema":1,"at":"2026-11-02T09:40:02Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-canon-stopped-midway.F1
  @atip.midway.03
  Scenario: canon init stops midway with STOPPED_MIDWAY and the reason SKILLS_INVALID_ARGUMENT when the skill install meets a refusal after its writes
    Given a fresh repository holding .pi and CANON_SKILLS_SOURCE naming a directory that does not exist
    When canon init --json runs
    Then it exits 1 with STOPPED_MIDWAY, reason SKILLS_INVALID_ARGUMENT, not retryable, and canon.yaml, the policy and AGENTS.md stay written

  # canon: {"schema":1,"at":"2026-11-02T09:40:07Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-canon-stopped-midway.F1
  @atip.midway.04
  Scenario: A skill directory that cannot be written after the writes is WRITE_FAILED and keeps its name
    Given a fresh repository holding a read-only .pi directory and a valid skill source
    When canon init --json runs
    Then it exits 1 with WRITE_FAILED and no reason, canon.yaml stays written and meta.next leads with canon status

  # canon: {"schema":1,"at":"2026-11-02T09:40:13Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-canon-stopped-midway.F1
  @atip.midway.05
  Scenario: A skill install failure that is not a refusal keeps SKILLS_FAILED and no reason
    Given a fresh repository whose .pi/skills is a symlink and a valid skill source
    When canon init --json runs
    Then it exits 1 with SKILLS_FAILED and no reason, canon.yaml stays written and meta.next leads with canon status

  # canon: {"schema":1,"at":"2026-11-02T09:40:18Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-canon-stopped-midway.F1
  @atip.midway.06
  Scenario: A refusal with no effect is exit 2 with the refusal's own code, never STOPPED_MIDWAY
    Given a repository holding .pi and a skill source that does not exist
    When canon init --dry-run --json runs, and canon init --providers with an unknown harness runs
    Then the first exits 2 with SKILLS_INVALID_ARGUMENT and the second with INIT_INVALID_ARGUMENT, neither is STOPPED_MIDWAY and nothing is written

  # canon: {"schema":1,"at":"2026-11-02T09:40:24Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-canon-stopped-midway.F2
  @atip.midway.07
  Scenario: The first meta.next step of a stopped command reads what was done and the repeat comes after it
    Given the failure envelope of the stopped canon init, and envelopes with no next, an empty next, a first step that repeats the command and a first step that writes
    When the first step is judged and run
    Then it is canon status --json, a read command that runs, the repeat of the install comes second, and each bad envelope fails the check

  # canon: {"schema":1,"at":"2026-11-02T09:40:30Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-canon-stopped-midway.F1
  @atip.midway.08
  Scenario: A batch element that stops midway is failed with STOPPED_MIDWAY and its reason, the elements after it are skipped and the batch is BATCH_INCOMPLETE
    Given a batch of two canon init elements in a fresh repository whose skill source does not exist, and a child failure envelope carrying STOPPED_MIDWAY and a reason
    When the batch runs and the child's error becomes the item error
    Then the first element is failed with STOPPED_MIDWAY and the reason SKILLS_INVALID_ARGUMENT in its item, the second is skipped, the batch is BATCH_INCOMPLETE with exit 1 and what the first element wrote stays

  # canon: {"schema":1,"at":"2026-11-02T09:40:37Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}
  @subject:evidence-attestation
  @case:happy
  @flow:canon-0-35-canon-stopped-midway.F3
  @atip.midway.09
  Scenario: The audit lists every exit-1 code as moved or kept and each code exists where the table says
    Given the registry, the sources and the audit table of the exit-1 codes
    When each code is decided as moved or kept and checked against the registry and the sources
    Then every exit-1 code has a line, each kept code is registered and built by a source, the moved INIT_SKILL_INSTALL_FAILED is in neither, and a code the table lacks, a kept code no source builds and a moved code that returns each fail

  # canon: {"schema":1,"at":"2026-11-02T09:40:43Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}
  @subject:tier-storage
  @case:happy
  @flow:canon-0-35-canon-stopped-midway.F4
  @atip.midway.10
  Scenario: The real I/O failure codes keep their names, exits and retryability
    Given the I/O failure codes of 0.34: WRITE_FAILED and the *_FAILED codes that write
    When the registry is read
    Then each keeps its name, exit 1 and retryability, has no reason, and is no reason of STOPPED_MIDWAY

  # canon: {"schema":1,"at":"2026-11-02T09:40:49Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-canon-stopped-midway.F5
  @atip.midway.11
  Scenario: No consumer spells a code that moved to STOPPED_MIDWAY, and the check catches one that comes back
    Given the guides, the website, the dashboard, the eval scripts, the design documents, the scenarios outside area=atip, the sources, the conformance cases and the generated spec
    When they are scanned for INIT_SKILL_INSTALL_FAILED, a code that moved to STOPPED_MIDWAY
    Then none spells it, and a text that spells it, or a longer name that contains it, is judged as the check says

  # canon: {"schema":1,"at":"2026-11-02T09:40:57Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-35-canon-stopped-midway.F3
  @atip.midway.12
  Scenario: A half-written adoption keeps CHANGE_ADOPTION_INCOMPLETE and its repair completes the link
    Given a change store that cannot be written after the subject record was written
    When canon subject adopt runs, then its repair runs
    Then the first exits 1 with CHANGE_ADOPTION_INCOMPLETE, no reason, never STOPPED_MIDWAY, naming the repair, and the repair completes the link and a third run writes nothing

  # canon: {"schema":1,"at":"2026-11-03T04:59:34Z","actor":{"agent_id":"impl-flow-failures","role":"implementer","session_id":"s-0-35-flow-failures"}}
  @subject:tier-storage
  @case:failure
  @flow:canon-0-35-canon-stopped-midway.F4
  @atip.midway.13
  Scenario: An I/O failure code that is made a reason of STOPPED_MIDWAY, given a reason, a refusal's exit or another retryability fails the check of the I/O codes
    Given the registry with an I/O failure code removed, moved to exit 2, made retryable or not, given a reason, or named as a reason of STOPPED_MIDWAY
    When the check of the I/O failure codes runs on each
    Then each fault is named with its own message, and the registry as it stands passes
