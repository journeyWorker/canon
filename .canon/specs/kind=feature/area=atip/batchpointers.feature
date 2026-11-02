Feature: atip batch pointers: /<index> pointers, BATCH_INCOMPLETE at the index, preflight reads no state
  # canon: {"schema":1,"at":"2026-11-02T09:40:35Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}

  # canon: {"schema":1,"at":"2026-11-02T09:40:35Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}
  @subject:atip-input
  @case:failure
  @flow:canon-0-35-batch-pointers.F1
  @atip.batchpointers.01
  Scenario: A preflight refusal in a batch is the item's own code at /<index>/..., exit 2, no data, and no earlier item ran
    Given a finding close batch whose second item has a wrong type, an enumeration value outside its set, an unknown key, a domain or constraint fault, or is no object
    When the batch runs
    Then it is exit 2 with that item's own code and a field /1/..., no data member, no pointer starting /items/, and no earlier item ran; a single document still says /round

  # canon: {"schema":1,"at":"2026-11-02T09:41:22Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}
  @subject:atip-input
  @case:happy
  @flow:canon-0-35-batch-pointers.F2
  @atip.batchpointers.02
  Scenario: A batch of good items is exit 0 with one written or unchanged report per item in order
    Given a finding close batch of two good items
    When the batch runs, and runs again
    Then the first is exit 0 with one item per submitted item in order, each written, and the second reports each unchanged

  # canon: {"schema":1,"at":"2026-11-02T09:41:50Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}
  @subject:atip-input
  @case:failure
  @flow:canon-0-35-batch-pointers.F3
  @atip.batchpointers.03
  Scenario: An item that fails when it runs is BATCH_INCOMPLETE exit 1 at /<index> with the earlier items done and the later ones skipped
    Given a batch whose second item closes a finding no ledger holds, with an item before and one after it
    When the batch runs
    Then it is exit 1 BATCH_INCOMPLETE with field /1, the items are written, failed (FINDING_NOT_FOUND) and skipped, and a repeat reports the first unchanged and fails at the same place

  # canon: {"schema":1,"at":"2026-11-02T09:42:07Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}
  @subject:atip-input
  @case:edge
  @flow:canon-0-35-batch-pointers.F3
  @atip.batchpointers.04
  Scenario: The first item failing is exit 1 BATCH_INCOMPLETE at /0 and its field is the index alone, the deeper pointer only in the item's error
    Given a batch whose first item fails when it runs, and a scenario new batch whose second item repeats an existing id
    When each runs
    Then the first is exit 1 BATCH_INCOMPLETE with field /0 (not exit 2), and the second has field /1 while the item's own error says /1/operands

  # canon: {"schema":1,"at":"2026-11-02T09:42:27Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}
  @subject:atip-input
  @case:failure
  @flow:canon-0-35-batch-pointers.F4
  @atip.batchpointers.05
  Scenario: Preflight runs no --dry-run: a missing object fails the item with BATCH_INCOMPLETE after the earlier item ran, while a state-free fault still refuses the whole batch
    Given subject adopt, which has --dry-run, with a first item that adopts and a second that names a change that does not exist, and a batch with an unknown key in its second item
    When each runs
    Then the first is exit 1 BATCH_INCOMPLETE at /1 after the first item was written, and the second is exit 2 INPUT_INVALID at /1/bogus with no item run

  # canon: {"schema":1,"at":"2026-11-02T09:42:49Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}
  @subject:atip-input
  @case:happy
  @flow:canon-0-35-batch-pointers.F4
  @atip.batchpointers.06
  Scenario: An item that needs what an earlier item writes passes preflight and runs in order
    Given a subject move batch specced then building, and the same two the other way round
    When each runs
    Then the chain in order is exit 0 with both written, and the other order is exit 1 at /0 with the first item failed

  # canon: {"schema":1,"at":"2026-11-02T09:43:04Z","actor":{"agent_id":"impl-canon-input","role":"implementer","session_id":"s-0-35-canon-input"}}
  @subject:atip-input
  @case:edge
  @flow:canon-0-35-batch-pointers.F5
  @atip.batchpointers.07
  Scenario: A failed item has its own error and no data, and a skipped item has neither
    Given a batch with a failed item followed by two skipped ones
    When the batch runs
    Then the failed item has error (with its code's retryable) and no data, each skipped item has no error and no data, and the report has one entry per submitted item
