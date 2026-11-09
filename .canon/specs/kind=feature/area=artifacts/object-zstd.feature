Feature: Evidence blobs in the object-store tier are zstd frames
  # canon: {"schema":1,"at":"2026-11-09T10:11:49Z","actor":{"agent_id":"Impl40_zstd","role":"planner"}}

  # canon: {"schema":1,"at":"2026-11-09T10:11:59Z","actor":{"agent_id":"Impl40_zstd","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-object-store-zstd.F1
  @artifacts.object-zstd.01
  Scenario: evidence add with an object-store tier writes the blob as <digest>.zst, a zstd frame that decompresses to the bound file; no plain object and no local copy exist and the gate passes
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:12:18Z","actor":{"agent_id":"Impl40_zstd","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-object-store-zstd.F1
  @artifacts.object-zstd.02
  Scenario: bytes that no longer hash to the bound digest are refused as Changed and the tier holds no object of either form
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:12:26Z","actor":{"agent_id":"Impl40_zstd","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-object-store-zstd.F2
  @artifacts.object-zstd.03
  Scenario: a digest held only as <digest>.zst and the same digest held only as a plain object give the same answer to gate check and artifacts verify
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:12:38Z","actor":{"agent_id":"Impl40_zstd","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-object-store-zstd.F2
  @artifacts.object-zstd.04
  Scenario: a digest with no object of either form is missing and gate check says stale-evidence; a .zst whose size contradicts its canon-stored-size is altered
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:12:45Z","actor":{"agent_id":"Impl40_zstd","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-object-store-zstd.F3
  @artifacts.object-zstd.05
  Scenario: artifacts verify --full decompresses a .zst object while hashing it and passes when the plain bytes hash to the digest
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:12:52Z","actor":{"agent_id":"Impl40_zstd","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-object-store-zstd.F3
  @artifacts.object-zstd.06
  Scenario: a flipped, cut or non-zstd .zst object, or one that decompresses past its recorded size, is altered with exit 1; an unreachable tier is exit 2
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:13:03Z","actor":{"agent_id":"Impl40_zstd","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-object-store-zstd.F4
  @artifacts.object-zstd.07
  Scenario: artifacts move uploads local blobs as .zst, removes the local copies, and does not re-upload a digest the tier already holds in either form
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:13:10Z","actor":{"agent_id":"Impl40_zstd","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-object-store-zstd.F4
  @artifacts.object-zstd.08
  Scenario: artifacts move refuses a tier .zst that does not decompress to its digest as ARTIFACTS_BLOB_CORRUPT and keeps that local copy
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:13:17Z","actor":{"agent_id":"Impl40_zstd","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-object-store-zstd.F5
  @artifacts.object-zstd.09
  Scenario: artifacts prune lists a digest held as .zst and as a plain object once with both objects' bytes, and --commit deletes both
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:13:27Z","actor":{"agent_id":"Impl40_zstd","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-object-store-zstd.F5
  @artifacts.object-zstd.10
  Scenario: when the tier refuses the second name of a digest prune says what it deleted and a repeat deletes the name that remains
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:13:39Z","actor":{"agent_id":"Impl40_zstd","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-object-store-zstd.F6
  @artifacts.object-zstd.11
  Scenario: with the local store evidence add, artifacts store and prune write and keep the plain blobs they always did and never a .zst
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:13:46Z","actor":{"agent_id":"Impl40_zstd","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-object-store-zstd.F6
  @artifacts.object-zstd.12
  Scenario: a <digest>.zst in the local blob directory is no blob: the store reads the digest as missing, verify and move skip it and prune does not delete it
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:13:53Z","actor":{"agent_id":"Impl40_zstd","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-object-store-zstd.F7
  @artifacts.object-zstd.13
  Scenario: the CHANGELOG and the guides name the .zst key, the older canon's behavior, the order of upgrade and zstd -d
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:14:00Z","actor":{"agent_id":"Impl40_zstd","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-object-store-zstd.F7
  @artifacts.object-zstd.14
  Scenario: a CHANGELOG or guide that lacks one of those items is named by the release check
    Given a step
