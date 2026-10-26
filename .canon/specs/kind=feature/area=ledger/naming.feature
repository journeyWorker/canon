Feature: ledger naming
  # canon: {"schema":1,"at":"2026-10-26T16:24:05Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}

  # canon: {"schema":1,"at":"2026-10-26T16:24:05Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:tier-storage
  @case:happy
  @flow:canon-0-30-ledger-naming.F1
  @ledger.naming.01
  Scenario: Every one of the fifteen kinds is stored under the one name rule
    Given a record of each of the fourteen core kinds and one plugin overlay record
    When each is written through its store
    Then the write output and the stored file are `kind=<kind>/<components>_<digest8>.json`, with the components the component table gives that kind, in table order
    And the directory is one level, `kind=<kind>/`, and no `area=` level exists

  # canon: {"schema":1,"at":"2026-10-26T16:24:06Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:tier-storage
  @case:happy
  @flow:canon-0-30-ledger-naming.F1
  @ledger.naming.02
  Scenario: A project leads the name, an absent project is a dash, and an alternative key says which field it holds
    Given a scenario, a review of a scenario, a review of a change, a divergence and evidence keyed by task, by scenario and by run, one of them with no project
    When they are written
    Then a kind whose schema has a project id starts with it, and an absent optional project is `-`
    And the key that can be several kinds is written after `scenario`, `change`, `task` or `run`, and evidence picks task, then scenario, then run
    And a review's name carries no commit sha: its pin is in the body and the digest separates two reviews of one target

  # canon: {"schema":1,"at":"2026-10-26T16:24:08Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:tier-storage
  @case:happy
  @flow:canon-0-30-ledger-naming.F1
  @ledger.naming.03
  Scenario: Integers are written plain and every ordering comes from the body, not the name
    Given findings of rounds 2 and 10, events 9 and 10 of one run, and two evidence records sharing one `at`
    When they are written and read back through the registry
    Then the names hold `2`, `10`, `9` and `10` with no padding
    And the order is the body's `at`, then the typed components in table order with numbers compared as numbers, then the body digest, whatever order the store scanned them in

  # canon: {"schema":1,"at":"2026-10-26T16:24:09Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:tier-storage
  @case:edge
  @flow:canon-0-30-ledger-naming.F1
  @ledger.naming.04
  Scenario: An id that needs escaping is escaped only in the name, and a run id is written lowercase, the same in git and R2
    Given a session id with an uppercase letter, an underscore and a slash, and a run id that is an uppercase ULID
    When they are written to the git tier and to an R2 tier and read back
    Then every byte outside `[a-z0-9.-]` is `+` and two lowercase hex digits, and the run id is its 26 lowercase characters
    And the git path and the R2 object key are the same string, which unescapes to the original value
    And a query, a cursor and an error message show the value before escaping

  # canon: {"schema":1,"at":"2026-10-26T16:24:10Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:tier-storage
  @case:edge
  @flow:canon-0-30-ledger-naming.F1
  @ledger.naming.05
  Scenario: A name over 200 bytes replaces its free-text component with a hash of the value
    Given a session id long enough that its file name would pass 200 bytes
    When it is written
    Then the free-text component is `h` and the first 16 hex digits of the SHA-256 of the value
    And the original value is still in the body

  # canon: {"schema":1,"at":"2026-10-26T16:24:11Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:tier-storage
  @case:failure
  @flow:canon-0-30-ledger-naming.F1
  @ledger.naming.06
  Scenario: Writing other content under an occupied git key is refused and the batch leaves nothing
    Given a git ledger that holds a record at a name, and a batch whose second record would take that name with different bytes
    When the batch is written
    Then the write is refused as a key conflict naming the path
    And the first record of the batch is not in the ledger either, and the ledger holds exactly what it held before

  # canon: {"schema":1,"at":"2026-10-26T16:24:12Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:tier-storage
  @case:failure
  @flow:canon-0-30-ledger-naming.F1
  @ledger.naming.07
  Scenario: An R2 object under the same key with other content is refused, not reported deduped
    Given an R2 tier holding an object at a key whose stored row differs from the record being written
    When the record is written
    Then the write is refused as a key conflict instead of reporting that the record was already stored
    And the same record written twice reports deduped, and a batch with a conflict puts no object

  # canon: {"schema":1,"at":"2026-10-26T16:24:14Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:tier-storage
  @case:failure
  @flow:canon-0-30-ledger-naming.F1
  @ledger.naming.08
  Scenario: sqlite and Postgres rows under the same key with other canonical JSON are refused and roll the batch back
    Given a hot tier holding a row at a key, and a batch whose later record lands on that key with different JSON
    When the batch is written, in one transaction
    Then the batch is refused as a key conflict, comparing the canonical JSON of the two bodies rather than relying on do-nothing
    And no row of the batch is stored, whichever chunk it was in

  # canon: {"schema":1,"at":"2026-10-26T16:24:15Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:tier-storage
  @case:failure
  @flow:canon-0-30-ledger-naming.F1
  @ledger.naming.09
  Scenario: A new ledger path the archive holds with other bytes is refused
    Given a name the archive already holds with different bytes
    When a record that resolves to that name is written to the ledger
    Then the write is refused as a key conflict before anything lands
    And an archive copy with the same bytes does not refuse

  # canon: {"schema":1,"at":"2026-10-26T16:24:16Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:query-reads
  @case:happy
  @flow:canon-0-30-ledger-naming.F1
  @ledger.naming.10
  Scenario: Query shows the id before escaping
    Given records whose ids need escaping
    When `canon query --json` and `canon query` list them and page through them
    Then the ID column and the cursor key hold the typed components joined by `_`, never the escaped form, and the digest is 8 hex digits

  # canon: {"schema":1,"at":"2026-10-26T16:24:17Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:tier-storage
  @case:happy
  @flow:canon-0-30-ledger-naming.F1
  @ledger.naming.11
  Scenario: Tier age effects and export and purge ids follow the new rule
    Given records past their aging threshold, an export and a purge of operational records
    When `canon tier age`, `canon export` and `canon purge` run
    Then the effect and manifest ids are `<kind>:<components>:<digest8>` with the components unescaped

  # canon: {"schema":1,"at":"2026-10-26T16:24:18Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:evidence-attestation
  @case:happy
  @flow:canon-0-30-ledger-naming.F1
  @ledger.naming.12
  Scenario: The write result line names the file under the new rule
    Given a scenario with no evidence
    When `canon evidence add --scenario-id` records evidence
    Then the output line names `kind=evidence/<project>_scenario_<scenario id>_<digest8>.json`
    And the file is stored under that name with `kind: evidence` in its body

  # canon: {"schema":1,"at":"2026-10-26T16:24:20Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:finding-lifecycle
  @case:failure
  @flow:canon-0-30-ledger-naming.F1
  @ledger.naming.13
  Scenario: Messages that quote a key quote the typed key
    Given a finding at round 2 seq 3 and a second add at the same key
    When `canon finding add` runs twice
    Then FINDING_SEQ_OCCUPIED quotes `<change id>_2_3`, and a promote refusal quotes the same key

  # canon: {"schema":1,"at":"2026-10-26T16:24:21Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:report-generation
  @case:failure
  @flow:canon-0-30-ledger-naming.F1
  @ledger.naming.14
  Scenario: A file name that disagrees with its body changes no count the report makes
    Given a ledger whose file names carry different digests than their bodies hash to, and the same ledger with correct names
    When the report views and the archive scoping read them
    Then the digest the views use is computed from each record's body, so both ledgers give the same counts
    And archive scoping groups records by their typed components, not by parsing a stem

  # canon: {"schema":1,"at":"2026-10-26T16:24:22Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:plugin-overlay
  @case:happy
  @flow:canon-0-30-ledger-naming.F1
  @ledger.naming.15
  Scenario: An overlay record is named by its declared join key values
    Given an overlay whose join key is a project id and a scenario id
    When a record is written and the kind is scanned
    Then the name is `kind=<namespace>.<kind>/<project>_<scenario>_<digest8>.json`
    And the scan accepts a file whose name components are the body's join key values and reports one that is not

  # canon: {"schema":1,"at":"2026-10-26T16:24:23Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:context-capability
  @case:happy
  @flow:canon-0-30-ledger-naming.F1
  @ledger.naming.16
  Scenario: The kind is called evidence in every place
    Given a repo on the 0.30 layout
    When its ledger directory, a record's kind field, `canon.yaml` routing and aging keys, the schemas, `canon context`, the report views and `--kind` are read
    Then all of them say `evidence`, and none says `evidence_record`

  # canon: {"schema":1,"at":"2026-10-26T16:24:24Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:skills-install
  @case:failure
  @flow:canon-0-30-ledger-naming.F1
  @ledger.naming.17
  Scenario: A guide that still describes the 0.29 layout is refused as drift
    Given the guides canon ships and installs
    When the repo's guide check reads them
    Then any guide that names `evidence_record`, an `area=` ledger level or a 12-hex digest suffix is reported
    And the storage and tier guides describe `kind=<kind>/<components>_<digest8>.json` and name `canon migrate` for a 0.29 repo

  # canon: {"schema":1,"at":"2026-10-26T16:24:38Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:evidence-attestation
  @case:failure
  @flow:canon-0-30-ledger-naming.F5
  @ledger.naming.18
  Scenario: Evidence that names no task, scenario or run is refused by the model for every writer
    Given an evidence record with no task id, scenario id or run id
    When it is written through the git, sqlite, Postgres and R2 tiers, by plugin sync and by ingest
    Then the model refuses it and no store changes
    And the CLI's own requirement of a task or scenario id is the same rule

  # canon: {"schema":1,"at":"2026-10-26T16:24:40Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:query-reads
  @case:failure
  @flow:canon-0-30-ledger-naming.F1
  @ledger.naming.19
  Scenario: A cursor from canon 0.29 is refused with a way to start over
    Given a cursor `canon query` issued in 0.29, which names a record by its 0.29 id and 12-hex digest
    When it is passed with `--cursor` to the 0.30 query
    Then the query is refused INVALID_CURSOR, the message says the cursor names records by 0.29 names, and the hint says to drop `--cursor` and start from the first page

  # canon: {"schema":1,"at":"2026-10-26T16:24:41Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:plugin-overlay
  @case:failure
  @flow:canon-0-30-ledger-naming.F1
  @ledger.naming.20
  Scenario: An overlay file whose name does not match its body's join key values is reported
    Given an overlay directory holding a file whose key components are not the body's join key values, and one named by the 0.29 rule
    When the overlay kind is scanned
    Then each is reported as a violation and skipped, and the matching records are returned

  # canon: {"schema":1,"at":"2026-10-26T16:24:42Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:divergence-lifecycle
  @case:happy
  @flow:canon-0-30-ledger-naming.F1
  @ledger.naming.21
  Scenario: A divergence is named by project, scenario, run sequence and round, and promotion breaks ties explicitly
    Given staged divergences and evidence that share every sort field but their body
    When they are promoted
    Then each is named `<project>_<scenario>_<run seq>_<round>_<digest8>` with no padding
    And the order they commit in is the typed components in table order and then the body digest, not the order staging was scanned in
