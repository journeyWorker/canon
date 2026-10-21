Feature: report dashboard
  # canon: {"schema":1,"at":"2026-10-21T11:57:34Z","actor":{"agent_id":"canon-impl-025"}}

  # canon: {"schema":1,"at":"2026-10-21T11:57:34Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:report-generation
  @case:happy
  @report.dashboard.01
  Scenario: The dashboard serves from any repo, from the app embedded in the binary
    Given a repository that is not canon's own checkout, and a canon binary built with the dashboard app
    When canon dashboard is run in it
    Then it serves the app embedded in the binary, gzip-compressed to a browser that accepts gzip and decompressed to one that does not
    And a checkout whose packages/dashboard/package.json names @canon-dev/dashboard and holds a build is served from that build instead, and no other project's packages/dashboard ever is

  # canon: {"schema":1,"at":"2026-10-21T11:57:34Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:report-generation
  @case:failure
  @report.dashboard.02
  Scenario: With no app to serve the dashboard refuses and names how to get one
    Given a repository with no canon dashboard build and a canon binary that embeds none
    When canon dashboard is run
    Then it exits nonzero with DASHBOARD_NOT_BUILT
    And the hint names the release install and the development build, and a packages/dashboard that belongs to another project is not served

  # canon: {"schema":1,"at":"2026-10-21T11:57:34Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:report-generation
  @case:failure
  @report.dashboard.03
  Scenario: A port already in use is its own refusal and names the flag that fixes it
    Given another process holds the port
    When canon dashboard is run with that port
    Then it exits nonzero with DASHBOARD_PORT_IN_USE, not the catch-all DASHBOARD_FAILED
    And the hint names --port <n> and --port 0

  # canon: {"schema":1,"at":"2026-10-21T11:57:34Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:report-generation
  @case:failure
  @report.dashboard.04
  Scenario: A repo whose snapshot cannot be generated is still served, says why, and never shows demo data
    Given a repository whose snapshot cannot be generated, for example because duckdb is not on PATH
    When canon dashboard is run in it
    Then the server starts and warns on stderr and in the JSON meta.warnings
    And GET /api/status reports the snapshot unavailable with the reason and the fix, and the panels page shows that instead of panels
    And the demo fixture is never served at /snapshot/ and the live snapshot route answers 404

  # canon: {"schema":1,"at":"2026-10-21T11:57:34Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:report-generation
  @case:happy
  @report.dashboard.05
  Scenario: The briefs page lists the live briefs that await approval, read by the checker's own parser
    Given a repository with live changes whose briefs are approved, unapproved, and archived
    When GET /api/briefs is requested
    Then it lists only the live unapproved briefs, each with its subjects, raw text, the sha256 of the file's bytes, its sections and flow steps read by canon brief check's own parser (Korean headings included), and its findings with an approvable flag

  # canon: {"schema":1,"at":"2026-10-21T11:57:34Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:report-generation
  @case:happy
  @report.dashboard.06
  Scenario: Approving from the page records exactly what canon brief approve records
    Given two identical copies of a repository with an unapproved brief and a policy file
    When one is approved through POST /api/briefs/approve and the other through canon brief approve
    Then the brief bytes agree except for the timestamp, the policy approval records agree, and the response is the brief approve --json document with meta.next
    And the approved brief leaves the list

  # canon: {"schema":1,"at":"2026-10-21T11:57:34Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:report-generation
  @case:failure
  @report.dashboard.07
  Scenario: A brief that changed since the page loaded is refused and left as it is
    Given a briefs page that loaded a brief and a brief file edited afterwards
    When the page posts the approval with the sha256 it loaded
    Then the server answers 409 BRIEF_CHANGED_SINCE_PAGE_LOAD with a hint to reload
    And the brief file and the policy approval are untouched

  # canon: {"schema":1,"at":"2026-10-21T11:57:34Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:report-generation
  @case:failure
  @report.dashboard.08
  Scenario: A brief with blockers is not approved from the page
    Given a brief with blocker findings, coverage rules excluded
    When the page posts its approval
    Then the server answers 409 BRIEF_HAS_BLOCKERS
    And the brief file and the policy approval are untouched

  # canon: {"schema":1,"at":"2026-10-21T11:57:34Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:report-generation
  @case:failure
  @report.dashboard.09
  Scenario: A request that is not the page's own is refused before the brief is read
    Given a request to the approve endpoint with a foreign or missing Origin, a foreign Host, a content type other than application/json, no Content-Length, or a body over 64 KiB
    When the server receives it
    Then it answers 403 or 400 before the body is read or the brief is opened, and a Host that is not the bound address is refused on every route, the app and the live snapshot included
    And the brief file is untouched, while a well-formed request from localhost on the same port is accepted

  # canon: {"schema":1,"at":"2026-10-21T11:57:34Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:report-generation
  @case:failure
  @report.dashboard.10
  Scenario: An empty or multi-line answer, an unknown change and a malformed body write nothing
    Given an approval request with an empty or multi-line name or note, an unknown or archived change, a change id that is not an id, or a body that is not the request
    When the server receives it
    Then it answers 400 or 404 with the CLI error envelope
    And the brief file and the policy approval are untouched
