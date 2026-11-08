Feature: retired record kinds
  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:happy
  @flow:canon-0-39-session-kinds-removal.F1
  @tier.retired.01
  Scenario: An old canon.yaml that routes or ages the retired kinds loads, warns RETIRED_KIND_KEYS and leads with canon migrate
    Given a canon.yaml with `routing.session`, `routing.event` and `aging.event`, whose values need not be rungs
    When canon status, canon gate check and canon context run
    Then each exits without an error and warns RETIRED_KIND_KEYS naming each key found, in one sorted message
    And `meta.next` leads with `canon migrate --json`, nothing is routed by those keys and their values are not checked

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:failure
  @flow:canon-0-39-session-kinds-removal.F1
  @tier.retired.02
  Scenario: Any other unknown kind name in routing is still a configuration error
    Given a canon.yaml that routes a misspelled kind such as `sesion`
    When the policy is loaded
    Then it is refused as an unknown record kind, and the retired names are the only ones read as keys that route nothing

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:failure
  @flow:canon-0-39-session-kinds-removal.F1
  @tier.retired.03
  Scenario: A repo holding the retired keys refuses no write
    Given a repo whose canon.yaml holds the retired keys
    When canon change new and canon evidence add run
    Then both succeed, as the keys are not a 0.29 layout

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:happy
  @flow:canon-0-39-session-kinds-removal.F2
  @tier.retired.04
  Scenario: canon migrate deletes exactly the retired key lines and revert restores them
    Given a canon.yaml with the three retired keys among other keys and comments
    When canon migrate runs as a preview, then with --commit and the plan digest, then bare again, then with --revert
    Then the preview names the lines to delete by number and writes nothing
    And the commit deletes only those lines, a section left empty becomes an empty mapping, the warning is gone and the next plan is empty
    And --revert restores the old file byte for byte, and a file without the keys is returned byte for byte

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:failure
  @flow:canon-0-39-session-kinds-removal.F2
  @tier.retired.05
  Scenario: A retired key that cannot be deleted line by line is refused and the file stays as it was
    Given a canon.yaml whose retired key is a flow mapping entry, an anchor or a tagged value
    When canon migrate runs
    Then it is refused as a configuration error that says to remove the key by hand, naming the key
    And the file is unchanged

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:edge
  @flow:canon-0-39-session-kinds-removal.F2
  @tier.retired.06
  Scenario: A 0.29-named file under kind=session does not make migrate refuse
    Given a repo whose git tier holds `kind=session/legacy-session__0123456789ab.json` with a body that is no JSON
    When canon migrate runs
    Then it does not refuse on that file and does not rename it

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:happy
  @flow:canon-0-39-session-kinds-removal.F3
  @tier.retired.07
  Scenario: canon purge --kind session deletes the stored rows of every tier whatever the routing says
    Given a git row and a sqlite row of kind session before the cutoff and one after, an event row, and a canon.yaml that still routes the keys or had them migrated away
    When canon purge --kind session --before the cutoff runs bare, then with --commit and the plan digest
    Then the preview matches the two old rows, deletes none and writes nothing
    And the commit deletes only the rows before the cutoff, never an event row and never a newer session row
    And every configured tier is searched without reading the routing, a symlink is refused and a file with no readable body or time is not a candidate

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:failure
  @flow:canon-0-39-session-kinds-removal.F3
  @tier.retired.08
  Scenario: purge refuses every kind but the four it takes and a stale plan digest
    Given the stored retired rows
    When canon purge --kind change is run, and a commit carries the digest of a removal set that changed since the preview
    Then the kind is a closed-value refusal that lists run, trajectory, session and event, and the rows stay
    And the stale digest is PLAN_STALE and nothing is deleted

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:happy
  @flow:canon-0-39-session-kinds-removal.F4
  @tier.retired.09
  Scenario: query, export and search refuse the retired kind names and point at purge
    Given a repo and the commands query, export and search with --kind session and --kind event
    When each is run
    Then each exits 2 with the code USAGE, field `--kind` and `expected` the twelve kinds
    And the message says the kind was removed in 0.39.0 and the hint names canon purge --kind <name> --before <date>

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:failure
  @flow:canon-0-39-session-kinds-removal.F4
  @tier.retired.10
  Scenario: The twelve kinds still read and purge still takes the retired names
    Given a repo
    When canon query --kind run, query --kind evidence, search --kind run and purge --kind session run
    Then each answers as 0.38 did

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:happy
  @flow:canon-0-39-session-kinds-removal.F5
  @tier.retired.11
  Scenario: canon context lists the twelve kinds and no retired schema file is shipped
    Given a repo and the model's schema directory
    When canon context runs and the directory is read
    Then the kinds are the twelve record kinds with neither `session` nor `event`
    And no `session.schema.json` or `event.schema.json` exists

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:failure
  @flow:canon-0-39-session-kinds-removal.F5
  @tier.retired.12
  Scenario: The schema registry has one schema per record kind so a leftover session schema fails it
    Given the closed record kinds of the model
    When the schemas are exported
    Then there are exactly twelve, one per kind, and a leftover one fails the registry test

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:happy
  @flow:canon-0-39-session-kinds-removal.F6
  @tier.retired.13
  Scenario: The report snapshot has nine tables and a manifest listing exactly those
    Given a repo and canon report snapshot
    When it writes its Parquet files
    Then it reports nine tables and one Parquet file per mart, none for the session costs
    And the manifest lists exactly those nine tables

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:failure
  @flow:canon-0-39-session-kinds-removal.F6
  @tier.retired.14
  Scenario: A report built by 0.38 is drift until it is rebuilt
    Given a built report, then the same report with the `## Session costs` section 0.38 wrote
    When canon report check runs, then canon report build, then check again
    Then the new report has no `## Session costs` and its Run observability names only Run columns
    And the first check exits 1 naming DRIFT and after the build the check exits 0

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:happy
  @flow:canon-0-39-session-kinds-removal.F7
  @tier.retired.15
  Scenario: A kind=session directory in the git tier breaks no command and is left alone
    Given a git tier holding `kind=session` and `kind=event` files, one with a body that does not parse
    When canon status, canon gate check and canon ledger compact run
    Then each succeeds and every file is as it was

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:happy
  @flow:canon-0-39-session-kinds-removal.F8
  @tier.retired.16
  Scenario: canon init routes the twelve kinds and no retired one
    Given a fresh repo
    When canon init runs and then canon init --check
    Then the canon.yaml has no `session` or `event` line and routes `task`, `handoff` and `run` to hot
    And the check passes without a warning

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:failure
  @flow:canon-0-39-session-kinds-removal.F8
  @tier.retired.17
  Scenario: init --check on an old scaffold warns RETIRED_KIND_KEYS
    Given a repo whose canon.yaml carries the retired routing lines an older scaffold wrote
    When canon init --check runs
    Then it passes and warns RETIRED_KIND_KEYS with `canon migrate --json` first in `meta.next`

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:failure
  @flow:canon-0-39-session-kinds-removal.F9
  @tier.retired.18
  Scenario: The release check fails when the CHANGELOG does not announce the retired kinds
    Given the 0.39 section of the CHANGELOG
    When it is read for the retired keys and the warning, migrate, the one-release purge, the refusals and the removed marts
    Then each is named, and a missing entry fails the release check naming it

  # canon: {"schema":1,"at":"2026-11-08T06:20:00Z","actor":{"agent_id":"spec-ingest","role":"planner"}}
  @subject:tier-storage
  @lane:behavior
  @case:happy
  @flow:canon-0-39-session-kinds-removal.F9
  @tier.retired.19
  Scenario: The architecture document, the README and the website list no session or event record kind
    Given ARCHITECTURE.md, README.md and the website pages
    When they are read
    Then no page lists the two names as record kinds
