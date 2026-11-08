Feature: canon artifacts prune deletes the blobs no record binds
  # canon: {"schema":1,"at":"2026-11-06T05:10:29Z","actor":{"agent_id":"impl-artifacts-prune","role":"implementer","session_id":"s-0-38-artifacts-prune"}}

  # canon: {"schema":1,"at":"2026-11-06T05:10:48Z","actor":{"agent_id":"impl-artifacts-prune","role":"implementer","session_id":"s-0-38-artifacts-prune"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-38-artifacts-prune.F1
  @artifacts.prune.01
  Scenario: The preview names the blobs to delete and the blob to keep, with the reason, and writes nothing
    Given a store of three blobs: one the latest evidence binds and two no record binds
    When canon artifacts prune is run bare, with and without --json
    Then it exits 0 with committed false and says two blobs go and one stays, the kept blob with the reason latest-evidence
    And the list names the two blobs to delete, meta.next leads with the exact --commit command carrying the planDigest, and no file changed

  # canon: {"schema":1,"at":"2026-11-06T05:11:29Z","actor":{"agent_id":"impl-artifacts-prune","role":"implementer","session_id":"s-0-38-artifacts-prune"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-38-artifacts-prune.F1
  @artifacts.prune.02
  Scenario: A preview with nothing to delete says so and offers no --commit command
    Given a store whose every blob a record binds, and a repo with no blob at all
    When canon artifacts prune is run bare, and then committed with the digest it printed
    Then the preview says there is nothing to delete, has an empty effects list and offers no --commit command
    And the commit of that digest deletes nothing and no file changed

  # canon: {"schema":1,"at":"2026-11-06T05:12:10Z","actor":{"agent_id":"impl-artifacts-prune","role":"implementer","session_id":"s-0-38-artifacts-prune"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:edge
  @flow:canon-0-38-artifacts-prune.F1
  @artifacts.prune.03
  Scenario: The list of blobs to delete is cut at --limit and continues by cursor while the totals count the whole
    Given a store of one bound blob and three blobs no record binds
    When canon artifacts prune --limit 2 is run, then again with the cursor, and with --all
    Then the first page lists two blobs while the totals say three, meta.page gives total 3 and a nextCursor, and meta.next leads with the commit of the whole plan and follows it with the next page
    And the second page lists the third blob with no nextCursor and the same planDigest, --all lists the three, and the commit of the first page deletes all three

  # canon: {"schema":1,"at":"2026-11-06T05:13:13Z","actor":{"agent_id":"impl-artifacts-prune","role":"implementer","session_id":"s-0-38-artifacts-prune"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-38-artifacts-prune.F2
  @artifacts.prune.04
  Scenario: Committing the previewed plan deletes the blobs no record binds and leaves verify and the gate answering as before
    Given a store of a latest evidence blob, a blob a superseded record bound, and two blobs no record binds
    When the digest the preview printed is committed
    Then the three blobs no latest record binds are deleted, the report counts the blobs, the bytes and the local copies and says the removal stays in git history
    And artifacts verify, artifacts verify --full and gate check answer as before, a stale digest is refused, and the next preview has nothing to delete

  # canon: {"schema":1,"at":"2026-11-06T05:14:09Z","actor":{"agent_id":"impl-artifacts-prune","role":"implementer","session_id":"s-0-38-artifacts-prune"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-38-artifacts-prune.F2
  @artifacts.prune.05
  Scenario: A repo whose records cannot be read in full deletes nothing and is refused
    Given a store with a blob no record binds, a committed evidence record that is not where its content puts it, and then a staged record that is no record
    When canon artifacts prune is run, and its earlier digest is committed
    Then each is refused with exit 2 naming what could not be read, and no blob is deleted
    And once the committed record is back, the preview reads again

  # canon: {"schema":1,"at":"2026-11-06T05:16:00Z","actor":{"agent_id":"impl-artifacts-prune","role":"implementer","session_id":"s-0-38-artifacts-prune"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:edge
  @flow:canon-0-38-artifacts-prune.F2
  @artifacts.prune.06
  Scenario: A blob a staged record, a superseded record or both a latest record and a current review binds is kept or deleted by the reasons that hold
    Given a superseded evidence record, the latest evidence, a current review that looked at the same report, and a record that is staged and not promoted yet, each binding its own blob
    When canon artifacts prune is run
    Then the blob of the latest evidence and the review names latest-evidence and current-review, the staged record's blob names staged, and the superseded record's blob is the only one to delete
    And after the commit the staged record promotes and artifacts verify --full and gate check are clean

  # canon: {"schema":1,"at":"2026-11-06T05:16:45Z","actor":{"agent_id":"impl-artifacts-prune","role":"implementer","session_id":"s-0-38-artifacts-prune"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-38-artifacts-prune.F2
  @artifacts.prune.07
  Scenario: A delete that fails part-way says what was deleted, and the next preview and commit finish the rest
    Given a store of one bound blob and three blobs no record binds, and a delete that fails after the first blob is gone
    When the previewed plan is committed
    Then it exits 1 with ARTIFACTS_PRUNE_INCOMPLETE and the reason WRITE_FAILED, data.deleted says one blob was deleted and meta.next leads with canon artifacts verify
    And the old digest is PLAN_STALE, a new preview lists the two blobs that remain, its commit deletes them, and the bound blob was never touched

  # canon: {"schema":1,"at":"2026-11-06T05:17:25Z","actor":{"agent_id":"impl-artifacts-prune","role":"implementer","session_id":"s-0-38-artifacts-prune"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-38-artifacts-prune.F3
  @artifacts.prune.08
  Scenario: --keep-days keeps the blobs the records of that many days bind and deletes the older ones
    Given the latest evidence, a superseded record written minutes ago and a superseded record written 90 days ago, each binding its own blob
    When canon artifacts prune is run without --keep-days, with --keep-days 30 and with --keep-days 120
    Then without the option only the latest evidence's blob stays, with 30 the minutes-old record's blob stays too with the reason within-keep-days and only the 90-day-old record's blob goes, and with 120 nothing goes
    And the commit command of the preview carries --keep-days and its commit deletes exactly that blob

  # canon: {"schema":1,"at":"2026-11-06T05:18:16Z","actor":{"agent_id":"impl-artifacts-prune","role":"implementer","session_id":"s-0-38-artifacts-prune"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-38-artifacts-prune.F3
  @artifacts.prune.09
  Scenario: --keep-days 0 and a negative number are refused with exit 2 and delete nothing
    Given a store with a blob no record binds
    When canon artifacts prune is run with --keep-days 0, -1, 1.5, a word and nothing, with and without --commit
    Then each exits 2 with USAGE naming --keep-days and nothing is deleted

  # canon: {"schema":1,"at":"2026-11-06T05:19:19Z","actor":{"agent_id":"impl-artifacts-prune","role":"implementer","session_id":"s-0-38-artifacts-prune"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-38-artifacts-prune.F4
  @artifacts.prune.10
  Scenario: With an object-store tier the unneeded objects are deleted by the same rule, the preview says that cannot be undone, and verify --full passes
    Given a repo whose artifact_store is the object-store tier cold holding a superseded record's object and two objects no record binds, and a blob copy left in the local directory that no record binds
    When canon artifacts prune is run, and its digest is committed
    Then the preview counts the objects and the local copy, says deleting an object cannot be undone, and deletes nothing
    And the commit deletes the unneeded objects and the local copy, the latest evidence's object stays, and artifacts verify --full and gate check are clean

  # canon: {"schema":1,"at":"2026-11-06T05:20:06Z","actor":{"agent_id":"impl-artifacts-prune","role":"implementer","session_id":"s-0-38-artifacts-prune"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-38-artifacts-prune.F4
  @artifacts.prune.11
  Scenario: A tier that refuses the delete, or cannot be listed, stops the run and the kept objects stay
    Given a tier that refuses to delete an object, and a tier that does not answer
    When the previewed plan is committed against the first and canon artifacts prune is run against the second
    Then the first exits 1 with ARTIFACTS_STORE_FAILED saying nothing was deleted and every object, the kept one included, is still there
    And a tier that gives out after its first delete exits 1 with ARTIFACTS_PRUNE_INCOMPLETE and the reason ARTIFACTS_STORE_FAILED, data.deleted says one object was deleted and the kept object is untouched, and a new preview and its commit delete the rest
    And the second exits 2 with ARTIFACTS_STORE_UNAVAILABLE and deletes nothing

  # canon: {"schema":1,"at":"2026-11-06T05:20:58Z","actor":{"agent_id":"impl-artifacts-prune","role":"implementer","session_id":"s-0-38-artifacts-prune"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-38-artifacts-prune.F5
  @artifacts.prune.12
  Scenario: A plan whose evidence changed since its preview is refused with PLAN_STALE and nothing is deleted
    Given a preview of canon artifacts prune that printed a plan digest
    And new evidence was bound to one of the blobs the plan deletes, or a record was staged, after that preview
    When canon artifacts prune is run with --commit and the old digest
    Then it exits 2 with PLAN_STALE saying to run the preview again and no blob is deleted
    And previewing again keeps the blob the new evidence binds and prints a new digest that commits

  # canon: {"schema":1,"at":"2026-11-06T05:21:24Z","actor":{"agent_id":"impl-artifacts-prune","role":"implementer","session_id":"s-0-38-artifacts-prune"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-38-artifacts-prune.F5
  @artifacts.prune.13
  Scenario: --commit without --plan-digest is refused and nothing is deleted
    Given a store with a blob no record binds
    When canon artifacts prune is run with --commit and no --plan-digest, and with --plan-digest and no --commit
    Then each exits 2 with USAGE naming --plan-digest or --commit, and nothing is deleted

  # canon: {"schema":1,"at":"2026-11-06T05:22:03Z","actor":{"agent_id":"impl-artifacts-prune","role":"implementer","session_id":"s-0-38-artifacts-prune"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-38-artifacts-prune.F6
  @artifacts.prune.14
  Scenario: The guides and the CHANGELOG name the command and say a deleted local blob stays in git history
    Given the built-in guides and the CHANGELOG of this repo
    When canon skills show canon, canon-storage and role-orchestrator are read, and the Unreleased section of the CHANGELOG
    Then each guide names canon artifacts prune and says a deleted local blob stays in git history
    And the Added section of the Unreleased CHANGELOG names the command and says the same

  # canon: {"schema":1,"at":"2026-11-06T09:29:35Z","actor":{"agent_id":"impl-artifacts-prune","role":"implementer","session_id":"s-0-38-artifacts-prune"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-38-artifacts-prune.F6
  @artifacts.prune.15
  Scenario: A release without the announcement fails the release check, each missing item named
    Given the guides and the CHANGELOG of this repo, and copies of them with the command or the git-history sentence taken out, with no Added section, and with no Unreleased section
    When the release check reads each copy
    Then it passes on the real text and fails on each copy, naming the guide or the CHANGELOG section and what it never says
    And the failure names the missing command and the missing sentence separately

  # canon: {"schema":1,"at":"2026-11-08T06:17:13Z","actor":{"agent_id":"spec-prune","role":"planner","session_id":"s-0-39-prune-temps"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-39-prune-temps.F1
  @artifacts.prune.16
  Scenario: The preview lists a stale staging temp file with its bytes beside the blobs and changes no file
    Given a store of a stale temp file named .tmp-<sha>-<pid>-<nanos> written six minutes ago, a file named .tmp-x and a directory named in the staging shape
    When canon artifacts prune is run bare, with and without --json
    Then staleTemps names the one stale temp file with its bytes and its git note, no blob is to delete, and the preview offers the --commit command
    And no file changed

  # canon: {"schema":1,"at":"2026-11-08T06:17:28Z","actor":{"agent_id":"spec-prune","role":"planner","session_id":"s-0-39-prune-temps"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-39-prune-temps.F1
  @artifacts.prune.28
  Scenario: A name that is not the staging shape, a directory of the shape and a young file are not listed, and with nothing at all the preview offers no --commit command
    Given a store with a file named .tmp-x, a directory named in the staging shape and a temp file written a minute ago, and a store with no blob and no temp file at all
    When canon artifacts prune is run bare
    Then staleTemps counts no file and lists none, the human output says there is nothing to delete and no --commit command is offered
    And none of those files is deleted or changed

  # canon: {"schema":1,"at":"2026-11-08T06:17:36Z","actor":{"agent_id":"spec-prune","role":"planner","session_id":"s-0-39-prune-temps"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:edge
  @flow:canon-0-39-prune-temps.F1
  @artifacts.prune.24
  Scenario: With an object-store tier the local temp files are counted by the same rule, apart from the tier's objects
    Given a repo whose artifact_store is the object-store tier cold holding the latest evidence's object, and a stale temp file in the local directory
    When canon artifacts prune is run and its digest is committed
    Then the preview counts the temp file, counts no tier object to delete and has no irreversible text
    And the commit deletes the temp file, the tier object stays and artifacts verify --full is clean

  # canon: {"schema":1,"at":"2026-11-08T06:17:44Z","actor":{"agent_id":"spec-prune","role":"planner","session_id":"s-0-39-prune-temps"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:edge
  @flow:canon-0-39-prune-temps.F1
  @artifacts.prune.25
  Scenario: The listed temp file names are cut at the limit while the totals count every stale temp file
    Given a store of 52 stale temp files
    When canon artifacts prune is run
    Then staleTemps counts 52 files while the list names 50, the human output says and 2 more, and the commit deletes all 52

  # canon: {"schema":1,"at":"2026-11-08T06:17:51Z","actor":{"agent_id":"spec-prune","role":"planner","session_id":"s-0-39-prune-temps"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-39-prune-temps.F2
  @artifacts.prune.17
  Scenario: Committing the previewed plan deletes the listed temp files with the blobs and counts them apart
    Given a store of a latest evidence blob, a blob no record binds, a stale temp file and a file named .tmp-x
    When the digest the preview printed is committed
    Then it exits 0, deleted.blobs counts the blob and deleted.staleTemps counts the temp file and its bytes apart from the blob bytes, and .tmp-x stays
    And the latest evidence blob stays, artifacts verify and gate check answer as before, and the next preview has nothing to delete
    And a repo with only a stale temp file previews it and its commit deletes it

  # canon: {"schema":1,"at":"2026-11-08T06:17:58Z","actor":{"agent_id":"spec-prune","role":"planner","session_id":"s-0-39-prune-temps"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-39-prune-temps.F2
  @artifacts.prune.18
  Scenario: A store directory that cannot be written refuses the commit before any temp file is deleted
    Given a stale temp file, a preview that printed a plan digest, and then a store directory that cannot be written
    When the digest is committed
    Then it is refused with exit 2 and a write error before anything is deleted, and the temp file is still there
    And once the directory can be written again the same digest commits and deletes the temp file

  # canon: {"schema":1,"at":"2026-11-08T06:18:04Z","actor":{"agent_id":"spec-prune","role":"planner","session_id":"s-0-39-prune-temps"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-39-prune-temps.F2
  @artifacts.prune.23
  Scenario: A temp file delete that fails part-way says what was deleted, and the next preview and commit finish the rest
    Given a store of two stale temp files, and a delete that fails after the first is gone
    When the previewed plan is committed
    Then it exits 1 with ARTIFACTS_PRUNE_INCOMPLETE and the reason WRITE_FAILED, data.deleted.staleTemps says one file was deleted and meta.next leads with canon artifacts verify
    And a new preview lists the one that remains, its commit deletes it and no blob was touched

  # canon: {"schema":1,"at":"2026-11-08T06:18:15Z","actor":{"agent_id":"spec-prune","role":"planner","session_id":"s-0-39-prune-temps"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-39-prune-temps.F3
  @artifacts.prune.19
  Scenario: A temp file written a minute ago is not in the preview and stays after a commit
    Given a store with a temp file written a minute ago, a temp file just under the floor, one stamped in the future and a stale one
    When the preview is run and its digest is committed
    Then only the stale temp file is listed and deleted, the others stay, and the next preview lists none
    And that preview offers no --commit command

  # canon: {"schema":1,"at":"2026-11-08T06:18:21Z","actor":{"agent_id":"spec-prune","role":"planner","session_id":"s-0-39-prune-temps"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-39-prune-temps.F3
  @artifacts.prune.29
  Scenario: A temp file stamped in the future or just under the five-minute floor is no stale file and is never deleted
    Given a store with a temp file stamped in the future and one written four and a half minutes ago
    When canon artifacts prune is run bare
    Then staleTemps counts no file, no --commit command is offered and no file changed

  # canon: {"schema":1,"at":"2026-11-08T06:18:28Z","actor":{"agent_id":"spec-prune","role":"planner","session_id":"s-0-39-prune-temps"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-39-prune-temps.F4
  @artifacts.prune.20
  Scenario: artifacts verify counts the stale temp files, names prune, exits as the bound files say and deletes nothing
    Given a store with a latest evidence blob, a stale temp file and a temp file written a minute ago, and a store with no temp file
    When canon artifacts verify --full is run, with and without --json
    Then it exits 0 with problems 0, the stale temp file is counted with its bytes and the line names canon artifacts prune, and the young file is not counted
    And no file changed, and without a temp file staleTemps is 0 and the line is not printed

  # canon: {"schema":1,"at":"2026-11-08T06:18:34Z","actor":{"agent_id":"spec-prune","role":"planner","session_id":"s-0-39-prune-temps"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-39-prune-temps.F4
  @artifacts.prune.21
  Scenario: A store directory that cannot be read leaves the temp file count unknown, never zero
    Given a store with a stale temp file whose directory cannot be read, and then a regular file where the directory belongs
    When canon artifacts verify is run
    Then it exits 0 as the bound files say, the temp file line says the files could not be counted and the JSON has no count
    And it never says the count is 0

  # canon: {"schema":1,"at":"2026-11-08T06:18:41Z","actor":{"agent_id":"spec-prune","role":"planner","session_id":"s-0-39-prune-temps"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-39-prune-temps.F5
  @artifacts.prune.22
  Scenario: A temp file that changed since the preview makes the plan stale, and a commit without the digest is refused
    Given a preview with two stale temp files that printed a plan digest
    When one of them is deleted, or one more is written, and canon artifacts prune is run with --commit and the old digest, and with --commit and no digest
    Then each commit with the old digest exits 2 with PLAN_STALE saying to run the preview again, the commit without a digest exits 2 with USAGE, and every file stays
    And previewing again prints a new digest whose commit deletes the listed temp files

  # canon: {"schema":1,"at":"2026-11-08T06:18:52Z","actor":{"agent_id":"spec-prune","role":"planner","session_id":"s-0-39-prune-temps"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-39-prune-temps.F6
  @artifacts.prune.26
  Scenario: The guides, the command help and the CHANGELOG say prune deletes the stale temp files and verify counts them
    Given the built-in guides, the help of canon artifacts prune and canon artifacts verify, and the Unreleased section of the CHANGELOG
    When they are read
    Then each guide names the stale `.tmp-` files, and the help of prune says it deletes them with `data.staleTemps` and the help of verify says it counts them and points to canon artifacts prune
    And the Unreleased entry of this change says prune deletes the stale temp files, verify counts them, the five minutes, and that a committed temp file is taken out of the commit

  # canon: {"schema":1,"at":"2026-11-08T06:18:58Z","actor":{"agent_id":"spec-prune","role":"planner","session_id":"s-0-39-prune-temps"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-39-prune-temps.F6
  @artifacts.prune.27
  Scenario: A release without the temp file announcement fails the release check, each missing item named
    Given the guides and the CHANGELOG of this repo, and copies of them with the Unreleased entry of this change taken out, with one sentence of it taken out, and with the `.tmp-` files taken out of one guide
    When the release check reads each copy
    Then it passes on the real text and fails on each copy, naming the missing entry, the missing sentence or the guide
    And the failures name each missing piece separately
