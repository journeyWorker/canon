Feature: ledger migrate
  # canon: {"schema":1,"at":"2026-10-26T16:24:26Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}

  # canon: {"schema":1,"at":"2026-10-26T16:24:26Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:gate-trust-spine
  @case:happy
  @flow:canon-0-30-ledger-naming.F2
  @ledger.migrate.01
  Scenario: A 0.29 repo warns on read commands and sends the user to canon migrate
    Given a 0.29 fixture repo with old-named files and `evidence_record` keys in canon.yaml
    When `canon status`, `canon gate check` and `canon context` run
    Then each succeeds in reading with no config error and warns that the layout is 0.29
    And `canon migrate --json` is the first next step
    And no old-named file counts toward any verdict

  # canon: {"schema":1,"at":"2026-10-26T16:24:27Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:evidence-attestation
  @case:failure
  @flow:canon-0-30-ledger-naming.F2
  @ledger.migrate.02
  Scenario: A write command is refused in a 0.29 repo with the same hint and writes nothing
    Given a 0.29 fixture repo
    When `canon evidence add` runs
    Then it is refused OLD_LAYOUT with the `canon migrate` hint, and the tree is byte-identical afterwards
    And no file in the new name appears beside the old ones

  # canon: {"schema":1,"at":"2026-10-26T16:24:28Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:gate-trust-spine
  @case:failure
  @flow:canon-0-30-ledger-naming.F2
  @ledger.migrate.03
  Scenario: One 0.29-named file in a 0.30 ledger is reported alone under its own code
    Given a ledger on the 0.30 layout with one file named by the 0.29 rule
    When `canon gate check` runs
    Then exactly that file is reported, as `old-layout` with the hint `canon migrate`
    And the 0.30 records are not reported and the old file counts toward no verdict

  # canon: {"schema":1,"at":"2026-10-26T16:24:29Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:tier-storage
  @case:happy
  @flow:canon-0-30-ledger-naming.F3
  @ledger.migrate.04
  Scenario: The preview lists everything it would do and writes nothing
    Given a 0.29 repo with ledger, archive, staging and overlay files, a canon.yaml, a gitignore negation and a sqlite file
    When `canon migrate` runs bare
    Then it lists the files to move, the evidence bodies to rewrite with their old key, digest and body hash, the canon.yaml keys, the gitignore lines and the sqlite rows, with a plan digest
    And the working tree, the sqlite file and canon.yaml are byte-identical afterwards

  # canon: {"schema":1,"at":"2026-10-26T16:24:30Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:tier-storage
  @case:failure
  @flow:canon-0-30-ledger-naming.F3
  @ledger.migrate.05
  Scenario: An interrupted promote in staging, or uncommitted changes under a moved path, refuse the plan
    Given staging that holds a body a committed record names as its staging id, and separately an uncommitted change under a moved directory
    When `canon migrate` runs
    Then each is refused naming the file, and nothing is written

  # canon: {"schema":1,"at":"2026-10-26T16:24:31Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:tier-storage
  @case:happy
  @flow:canon-0-30-ledger-naming.F4
  @ledger.migrate.06
  Scenario: Committing the previewed plan renames everything and a second run has nothing to move
    Given a 0.29 repo and the plan digest its preview printed
    When `canon migrate --commit --plan-digest` runs, then `canon migrate` runs again
    Then the files are moved with git, evidence bodies carry `kind: evidence` and their recomputed digests, canon.yaml says `evidence` and the gitignore negation follows
    And gate check, status and query give the same judgements and records as before, apart from names, the kind name and the order of records sharing an `at`
    And the second run reports nothing to move

  # canon: {"schema":1,"at":"2026-10-26T16:24:33Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:review-attestation
  @case:happy
  @flow:canon-0-30-ledger-naming.F4
  @ledger.migrate.07
  Scenario: Reviews pinned before 0.23 are judged the same after the move
    Given a fixture with reviews that pin evidence files by their 0.29 key and 12-hex digest, current and stale
    When the repo is migrated
    Then the 0.29 key and digest are judged from the plan's record of the old world through the frozen 0.29 functions
    And each review that was current is current and each that was stale is stale

  # canon: {"schema":1,"at":"2026-10-26T16:24:34Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:tier-storage
  @case:failure
  @flow:canon-0-30-ledger-naming.F4
  @ledger.migrate.08
  Scenario: Another digest is refused with PLAN_STALE and nothing changes
    Given a 0.29 repo
    When `canon migrate --commit --plan-digest` runs with a digest that is not the preview's
    Then it is refused PLAN_STALE and the tree, the sqlite file and the config are unchanged

  # canon: {"schema":1,"at":"2026-10-26T16:24:35Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:tier-storage
  @case:edge
  @flow:canon-0-30-ledger-naming.F4
  @ledger.migrate.09
  Scenario: The move can be undone from the backup
    Given a migrated fixture and the backup the migration made
    When the backup is restored and the moves are reverted
    Then the tree and the sqlite file are byte-identical to before the migration
    And Postgres and R2 keys are restored from the old keys the plan recorded

  # canon: {"schema":1,"at":"2026-10-26T16:24:36Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:tier-storage
  @case:happy
  @flow:canon-0-30-ledger-naming.F4
  @ledger.migrate.10
  Scenario: sqlite rows and the configured object-store keys are re-keyed through the same plan
    Given a sqlite file with rows under 0.29 keys, and an object store with objects under 0.29 keys
    When the plan is committed
    Then each row and object has its 0.30 key and its kind spelled `evidence`, in one transaction for the rows, and a plan's key that another row holds with other JSON refuses the whole run

  # canon: {"schema":1,"at":"2026-10-26T16:24:37Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:evidence-attestation
  @case:happy
  @flow:canon-0-30-ledger-naming.F4
  @ledger.migrate.11
  Scenario: Evidence written after the move shows up in git status
    Given a migrated repo whose gitignore re-includes the ledger's evidence directory
    When `canon evidence add` records evidence
    Then the new file under `kind=evidence/` is not ignored and `git status` lists it

  # canon: {"schema":1,"at":"2026-10-26T16:24:43Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:context-capability
  @case:failure
  @flow:canon-0-30-ledger-naming.F2
  @ledger.migrate.12
  Scenario: A canon.yaml with the old evidence_record key is read only far enough to point at canon migrate
    Given a canon.yaml whose routing and aging keys say `evidence_record`
    When `canon status` and `canon context` run
    Then neither fails on configuration: the key routes the kind and a warning names `canon migrate`
    And canon itself never writes the 0.29 key

  # canon: {"schema":1,"at":"2026-10-26T16:24:45Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:review-attestation
  @case:failure
  @flow:canon-0-30-ledger-naming.F4
  @ledger.migrate.13
  Scenario: A review that was stale before the move stays stale, and one that cannot be judged says so
    Given pre-0.23 reviews whose evidence changed since, and one whose commit is not in the clone
    When the repo is migrated
    Then the stale ones are left as they are and still stale, and the one that cannot be checked is pinned as it stands and counted

  # canon: {"schema":1,"at":"2026-10-26T16:24:46Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:authoring-scaffold
  @case:failure
  @flow:canon-0-30-ledger-naming.F2
  @ledger.migrate.14
  Scenario: Scenario and feature authoring is refused in a 0.29 repo
    Given a 0.29 fixture repo
    When `canon scenario new` and `canon feature new` run
    Then each is refused OLD_LAYOUT with the `canon migrate` hint and writes no file

  # canon: {"schema":1,"at":"2026-10-26T16:24:47Z","actor":{"agent_id":"impl-030-ln","role":"implementer","session_id":"ln-2026-10-26"}}
  @subject:divergence-lifecycle
  @case:failure
  @flow:canon-0-30-ledger-naming.F2
  @ledger.migrate.15
  Scenario: Divergence writes are refused in a 0.29 repo
    Given a 0.29 fixture repo
    When `canon divergence add` and `canon gate promote` run
    Then each is refused OLD_LAYOUT with the `canon migrate` hint and stages or commits nothing

  # canon: {"schema":1,"at":"2026-10-26T18:12:42Z","actor":{"agent_id":"impl-030-rl","role":"implementer","session_id":"rl-2026-10-26"}}
  @subject:review-attestation
  @case:happy
  @flow:canon-0-30-record-links.F1
  @ledger.migrate.16
  Scenario: A review keeps its review id through canon migrate
    Given a ledger of reviews that carry review ids, and one review written before 0.30 that has none
    When canon migrate rewrites the repo's records
    Then every review still carries the same review id in its body
    And the review written before 0.30 still has none and stays valid
