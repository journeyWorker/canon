Feature: Durable record writes
  # canon: {"schema":1,"at":"2026-11-08T06:19:20Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}

  # canon: {"schema":1,"at":"2026-11-08T06:19:36Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:happy
  @flow:canon-0-39-write-durability.F1
  @tier.durability.01
  Scenario: A written record is synced, renamed into place and then its directory entry is synced, in that order
    Given a record path in a directory, and a trace of the file syncs, renames and directory syncs
    When write_atomic writes the record
    Then it syncs the temp file, renames it to the record's name and syncs the parent directory, in that order
    And the file it synced is the file it renamed, the rename's target is the record path, and the directory synced is the parent

  # canon: {"schema":1,"at":"2026-11-08T06:20:03Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:failure
  @flow:canon-0-39-write-durability.F1
  @tier.durability.02
  Scenario: A path with no parent directory is refused as invalid input, nothing is written and nothing is synced
    Given the path / , which has no parent directory
    When write_atomic is asked to write it
    Then it fails with an invalid-input error and writes no file
    And no file sync, rename or directory sync is recorded

  # canon: {"schema":1,"at":"2026-11-08T06:22:26Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:happy
  @flow:canon-0-39-write-durability.F2
  @tier.durability.03
  Scenario: Publishing blobs syncs the store directory once after the last rename, and publishing only a blob that already exists syncs nothing
    Given two staged evidence blobs, and a third that another writer already placed
    When the two are published, and then the third
    Then the two renames are followed by exactly one sync of the evidence store directory
    And publishing the blob that already exists records no sync at all

  # canon: {"schema":1,"at":"2026-11-08T06:23:07Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:failure
  @flow:canon-0-39-write-durability.F2
  @tier.durability.04
  Scenario: A store directory that cannot be synced takes the placed blobs back and fails
    Given two staged blobs and an evidence store directory that can be written and searched but not read
    When the blobs are published
    Then the renames succeed and the directory sync fails, the error points at the last blob placed and names the store directory
    And both blobs placed are taken back, so no record can point at a blob whose name might not be on disk

  # canon: {"schema":1,"at":"2026-11-08T06:23:37Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:failure
  @flow:canon-0-39-write-durability.F3
  @tier.durability.05
  Scenario: A parent directory that cannot be synced fails the write and the error names the directory
    Given a parent directory the user can write and search but not read (the test is skipped as root and on Windows)
    When write_atomic writes a record under it
    Then it fails with permission denied and the error names the directory
    And the bytes were already renamed into place, so the error means not known to be on disk, and the write is never reported as a success

  # canon: {"schema":1,"at":"2026-11-08T06:24:29Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:tier-storage
  @lane:architecture
  @case:happy
  @flow:canon-0-39-write-durability.F4
  @tier.durability.06
  Scenario: No source outside canon-store keeps a private temp-and-rename write copy
    Given the source of every crate
    When it is searched for the fixed temp names of the four private write copies: canon.yaml, the legacy migration, the brief migration and the strategy activation file
    Then none of them is found
    And the one write path left is canon_store::write_atomic

  # canon: {"schema":1,"at":"2026-11-08T06:24:46Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:failure
  @flow:canon-0-39-write-durability.F4
  @tier.durability.07
  Scenario: A write that cannot happen fails with its cause, and a migrate that cannot replace canon.yaml stops incomplete and the next run finishes it
    Given a legacy file whose parent is a file, so the write cannot happen, and an old repo whose root takes no new entry
    When the legacy write is made, and migrate --commit is run on the repo
    Then the legacy error reads could not write <file> and keeps its io cause, and migrate exits 1 with MIGRATION_INCOMPLETE, reason STOPPED, and canon.yaml as it was
    And running migrate again finishes the migration and writes the migrated canon.yaml

  # canon: {"schema":1,"at":"2026-11-08T06:25:56Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:happy
  @flow:canon-0-39-write-durability.F5
  @tier.durability.08
  Scenario: An audit line and its newline leave in one write call
    Given an audit sink that counts the write calls it receives
    When one audit line is written
    Then the sink receives exactly one write call, and it carries the JSON and the newline
    And the bytes are one JSON line

  # canon: {"schema":1,"at":"2026-11-08T06:27:42Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:edge
  @flow:canon-0-39-write-durability.F5
  @tier.durability.09
  Scenario: Two audit lines stay two lines of the file
    Given a repo and its access audit file
    When two audit lines are appended
    Then the file has two lines
    And each line parses as JSON on its own

  # canon: {"schema":1,"at":"2026-11-08T06:27:50Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:failure
  @flow:canon-0-39-write-durability.F5
  @tier.durability.10
  Scenario: An audit file that cannot be opened is an error, and a new audit file is still owner-only
    Given a repo where a directory stands where the audit file belongs, and a repo with no audit file yet
    When an audit line is appended in each
    Then the first append fails with an error
    And the second creates the audit file with mode 0600

  # canon: {"schema":1,"at":"2026-11-08T06:27:56Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:happy
  @flow:canon-0-39-write-durability.F6
  @tier.durability.11
  Scenario: A rekey holds the write lock from its first statement, so a writer that gets in between its read and its write cannot make it fail
    Given a hot.db holding a record, and another connection that writes in the gap between rekey's held-key read and its first write
    When rekey moves the record
    Then rekey succeeds and the other writer is refused with database is locked while it runs
    And the other writer may write once rekey has committed, and the rows are the moved record and the later write

  # canon: {"schema":1,"at":"2026-11-08T06:28:02Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:failure
  @flow:canon-0-39-write-durability.F6
  @tier.durability.12
  Scenario: A transaction that begins plain and reads first loses to a writer in the gap with database is locked, which is the shape rekey left
    Given a hot.db, and a transaction that began plain, read the held key and left the write lock free
    When another connection commits a write before the transaction's own write
    Then the other write commits
    And the transaction's update fails with database is locked at once, which is why rekey begins immediately

  # canon: {"schema":1,"at":"2026-11-08T06:28:10Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:happy
  @flow:canon-0-39-write-durability.F7
  @tier.durability.13
  Scenario: A new hot.db and the -wal and -shm files SQLite makes beside it are readable and writable by their owner only
    Given a repo with no hot.db, on Unix
    When the sqlite tier is opened and a record is written
    Then hot.db is created with mode 0600
    And the -wal and -shm files SQLite makes beside it are 0600 as well

  # canon: {"schema":1,"at":"2026-11-08T06:28:17Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:failure
  @flow:canon-0-39-write-durability.F7
  @tier.durability.14
  Scenario: An existing hot.db keeps its mode when it is opened, written to or opened read-only
    Given a hot.db whose mode is 0644
    When it is opened, written to, and opened read-only
    Then its mode is still 0644
    And no open changes the mode of a file that already exists

  # canon: {"schema":1,"at":"2026-11-08T06:28:34Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:happy
  @flow:canon-0-39-write-durability.F8
  @tier.durability.15
  Scenario: The CHANGELOG names the directory fsync, the one write path, the one-write audit line, the rekey write lock and the new hot.db mode
    Given the CHANGELOG section this release ships in
    When the release check reads the Fixed and Changed entries of canon-0-39-write-durability
    Then Fixed names the directory fsync, the artifact blobs, the private write copies, the one-write audit line, BEGIN IMMEDIATE and Windows
    And Changed names the 0600 hot.db, chmod 600 .canon/hot.db for an existing one, and Windows

  # canon: {"schema":1,"at":"2026-11-08T06:28:50Z","actor":{"agent_id":"spec-durasecret","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:failure
  @flow:canon-0-39-write-durability.F8
  @tier.durability.16
  Scenario: A CHANGELOG without the entry or a piece of it is named by the release check
    Given an empty section, a section without the entry, and one without chmod 600 .canon/hot.db or BEGIN IMMEDIATE
    When the release check reads each
    Then it names the missing entry and each missing piece
    And a section that has them all is named by nothing
