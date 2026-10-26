Feature: Demo repo scaffold and review
  # canon: {"schema":1,"at":"2026-10-26T11:43:01Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}

  # canon: {"schema":1,"at":"2026-10-26T11:43:05Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}
  @subject:authoring-scaffold
  @case:happy
  @flow:canon-0-30-spec-backfill.F5
  @authoring.demo.01
  Scenario: demo init scaffolds a demo repo whose gate starts red
    Given a directory that holds no canon.yaml
    When canon demo init --repo names it
    Then it exits 0 with data.repo the directory and data.scenario auth.login.01
    And the directory now holds a canon.yaml, and canon gate check there exits 1 with GATE_VIOLATIONS

  # canon: {"schema":1,"at":"2026-10-26T11:43:05Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}
  @subject:authoring-scaffold
  @case:failure
  @flow:canon-0-30-spec-backfill.F5
  @authoring.demo.02
  Scenario: demo init in a directory that already has a canon.yaml is refused and changes nothing
    Given a directory that canon demo init already set up
    When canon demo init --repo names it again
    Then it exits 2 with INIT_CONFIG_EXISTS
    And the directory tree is identical to before the call

  # canon: {"schema":1,"at":"2026-10-26T11:43:05Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}
  @subject:authoring-scaffold
  @case:happy
  @flow:canon-0-30-spec-backfill.F5
  @authoring.demo.03
  Scenario: demo review records the reviewer review and the gate turns green
    Given a repo made by canon demo init, whose gate is red
    When canon demo review --repo names it
    Then it exits 0 with data.scenario auth.login.01
    And canon gate check there now exits 0 with no violations

  # canon: {"schema":1,"at":"2026-10-26T11:43:06Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}
  @subject:authoring-scaffold
  @case:failure
  @flow:canon-0-30-spec-backfill.F5
  @authoring.demo.04
  Scenario: demo review in a directory that demo init never set up is refused and changes nothing
    Given a directory that holds no canon.yaml
    When canon demo review --repo names it
    Then it exits 2 with DEMO_NOT_INITIALIZED
    And the directory tree is identical to before the call
