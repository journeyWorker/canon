Feature: atip paging: a cursor bound to its call, and field selection as the contract's projection
  # canon: {"schema":1,"at":"2026-11-01T10:52:23Z","actor":{"agent_id":"impl-paging-fields","role":"implementer","session_id":"s-0-34-paging-fields"}}

  # canon: {"schema":1,"at":"2026-11-01T10:52:37Z","actor":{"agent_id":"impl-paging-fields","role":"implementer","session_id":"s-0-34-paging-fields"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-34-atip-paging-fields.F1
  @atip.paging.01
  Scenario: A paged listing reports meta.page and a next call with --cursor, the next call reads the next page, and the last page has neither
    Given a repo with three subjects
    When canon query --kind subject --limit 1 --json runs, then the --cursor step of meta.next, until a page has no cursor
    Then every page has meta.page returned and total, meta.next holds the call with --cursor, three pages cover the three subjects once, and data holds no paging member of its own

  # canon: {"schema":1,"at":"2026-11-01T10:52:42Z","actor":{"agent_id":"impl-paging-fields","role":"implementer","session_id":"s-0-34-paging-fields"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-34-atip-paging-fields.F1
  @atip.paging.02
  Scenario: A page size below 1 and --limit with --all are USAGE naming the option
    Given a repo with three subjects
    When --limit 0 and --limit 1 --all are given to query, finding list, divergence list and export
    Then each is exit 2 USAGE with field --limit (or --all for the pair)

  # canon: {"schema":1,"at":"2026-11-01T10:52:46Z","actor":{"agent_id":"impl-paging-fields","role":"implementer","session_id":"s-0-34-paging-fields"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-34-atip-paging-fields.F2
  @atip.paging.03
  Scenario: A cursor given other arguments is CURSOR_INVALID with reason ARGUMENTS_CHANGED and nothing is written
    Given a first page issued with --limit 1 for query, finding list, divergence list and export
    When the next call changes --kind, --since, --domain, --change, --open, --as-of, --before or adds --history
    Then each is exit 2 CURSOR_INVALID with reason ARGUMENTS_CHANGED and field --cursor, the repo is byte-identical afterwards, and the same arguments in another order or spelling are accepted

  # canon: {"schema":1,"at":"2026-11-01T10:52:49Z","actor":{"agent_id":"impl-paging-fields","role":"implementer","session_id":"s-0-34-paging-fields"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-34-atip-paging-fields.F2
  @atip.paging.04
  Scenario: A cursor of another command or not a cursor is FOREIGN, one whose record is gone is EXPIRED, the three reasons are registered, and a refusal writes nothing
    Given cursors of another command, an arbitrary string, hex that is not a cursor, an empty one and one of 0.33, and a cursor whose finding file was deleted
    When each is given with --cursor
    Then the first five are CURSOR_INVALID reason FOREIGN, the deleted one is EXPIRED, field is --cursor, exit 2, nothing is written, and canon schema --error CURSOR_INVALID lists ARGUMENTS_CHANGED, EXPIRED and FOREIGN with meaning and fix

  # canon: {"schema":1,"at":"2026-11-01T10:52:52Z","actor":{"agent_id":"impl-paging-fields","role":"implementer","session_id":"s-0-34-paging-fields"}}
  @subject:atip-contract
  @case:edge
  @flow:canon-0-34-atip-paging-fields.F2
  @atip.paging.05
  Scenario: A call that changes only --limit, --all or --fields continues at the same place
    Given a first page with --limit 1
    When the next call changes --limit, then --all, then --fields, and its next page is read
    Then it continues at the same record, the next page keeps --fields, and the last page has no cursor

  # canon: {"schema":1,"at":"2026-11-01T10:52:55Z","actor":{"agent_id":"impl-paging-fields","role":"implementer","session_id":"s-0-34-paging-fields"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-34-atip-paging-fields.F3
  @atip.paging.06
  Scenario: Each of the six outputs declares fields: records, paths, keep, with default inside paths, keep apart from paths, every path a member of the record schema
    Given the six commands and the eight outputs that declare fields
    When canon schema <command> --json is read for finding list, divergence list, query, export, status and context
    Then one output of each has fields with records, paths, keep where it has some; default is inside paths, keep and paths are disjoint, every path is a member of the record schema and written in its one spelling, and the declaration passes the contract's rule 11 checker

  # canon: {"schema":1,"at":"2026-11-01T10:52:58Z","actor":{"agent_id":"impl-paging-fields","role":"implementer","session_id":"s-0-34-paging-fields"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-34-atip-paging-fields.F3
  @atip.paging.07
  Scenario: A declaration or schema that breaks the contract is found: a non-projectable cut node, a path outside the record schema, an array crossed without [], a default outside paths, an overlap
    Given the command document of finding list and a schema with seeded violations
    When each of const, allOf, enum, oneOf, minProperties, uniqueItems, no type, a boolean schema, a path outside the record, an array path without [], a default outside paths, an overlap and a bracketed identifier is declared
    Then the checker reports a problem for each, and none for the published declaration

  # canon: {"schema":1,"at":"2026-11-01T10:53:02Z","actor":{"agent_id":"impl-paging-fields","role":"implementer","session_id":"s-0-34-paging-fields"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-34-atip-paging-fields.F4
  @atip.paging.08
  Scenario: --fields keeps the selected and the always-kept members only, and meta.fields lists them
    Given a repo with three subjects and a change with three findings
    When canon query --kind subject --fields subject_id,status and canon finding list --fields summary run
    Then each record holds exactly the selected and the kept members, at any depth, each equal to the unprojected one; meta.fields lists the kept paths; without --fields there is no meta.fields

  # canon: {"schema":1,"at":"2026-11-01T10:53:13Z","actor":{"agent_id":"impl-paging-fields","role":"implementer","session_id":"s-0-34-paging-fields"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-34-atip-paging-fields.F4
  @atip.paging.09
  Scenario: A projected answer of each output passes its published schema with required reduced to the members that are present
    Given the eight outputs (six commands, three of them export's) and a call of each with --fields
    When each projected answer (eight outputs: export has three) is checked against the schema its command document publishes with required reduced
    Then none has a problem, and selecting every path of an output is the whole answer

  # canon: {"schema":1,"at":"2026-11-01T10:53:20Z","actor":{"agent_id":"impl-paging-fields","role":"implementer","session_id":"s-0-34-paging-fields"}}
  @subject:atip-contract
  @case:edge
  @flow:canon-0-34-atip-paging-fields.F4
  @atip.paging.10
  Scenario: Bracketed keys, array paths, map keys and parent-with-child paths project as the contract shows
    Given a record with a bracketed key, an array of owners, a map and a profile
    When the contract's paths are projected: id, profile.name, owners[].email, ["display-name"], attributes.region, attributes["http.status-code"], attributes[""], labels[].region, profile with profile.name
    Then each projection equals the contract's example, a path is written back as it was read, and a comma inside a bracketed key does not split the list

  # canon: {"schema":1,"at":"2026-11-01T10:53:24Z","actor":{"agent_id":"impl-paging-fields","role":"implementer","session_id":"s-0-34-paging-fields"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-34-atip-paging-fields.F4
  @atip.paging.11
  Scenario: A schema that cannot hold a projection and a meta member outside the contract are found
    Given a schema whose record carries an enum, and a paged and projected answer
    When the projected value is checked against the schema, the declaration is checked, and meta.hasMore is added to the answer
    Then the projection of a valid record fails the schema, the declaration is not projectable, and the envelope validation fails

  # canon: {"schema":1,"at":"2026-11-01T10:53:28Z","actor":{"agent_id":"impl-paging-fields","role":"implementer","session_id":"s-0-34-paging-fields"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-34-atip-paging-fields.F5
  @atip.paging.12
  Scenario: A path the declaration does not hold, an empty value, a malformed path or a kept path is USAGE with the selectable paths and writes nothing
    Given a repo with a subject, a change and findings
    When --fields is nosuch, changeIds, empty, subject_id[], a trailing comma, a bracketed identifier, an unclosed bracketed key, a kept path, a path of another output
    Then each is exit 2 USAGE, field --fields, expected the declared paths of that command, and the repo is byte-identical afterwards

  # canon: {"schema":1,"at":"2026-11-01T10:53:31Z","actor":{"agent_id":"impl-paging-fields","role":"implementer","session_id":"s-0-34-paging-fields"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-34-atip-paging-fields.F4
  @atip.paging.13
  Scenario: export is three outputs, each named by meta.output, and --fields cuts the manifest and the file it writes
    Given a repo with three subjects
    When canon export runs plain, with --dry-run, with --output and --fields kind (read against the written answer), in text mode with --output and --fields id, and with --fields id,digest, and its command document is read
    Then the document lists ExportReport (2.1), ExportWrittenReport and ExportPreviewReport, each with a when and its own fields (the batch output has none); meta.output names the output printed; the json --output file holds the whole manifest and the text-mode file the projected records, each with the page; data holds no meta member

  # canon: {"schema":1,"at":"2026-11-01T10:53:36Z","actor":{"agent_id":"impl-paging-fields","role":"implementer","session_id":"s-0-34-paging-fields"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-34-atip-paging-fields.F1
  @atip.paging.14
  Scenario: A paged or projected answer carries only the contract's meta members and the report has no paging member of its own
    Given a repo with three subjects, a change with three findings and two divergent scenarios
    When a page of each paged command runs with --limit 1 and --fields, and each command document is read
    Then meta holds only the contract's members (page, fields, next, context, ...) and no meta.truncated; data and the report schemas hold no page, nextCursor, returned, hasMore or meta member

  # canon: {"schema":1,"at":"2026-11-01T10:53:39Z","actor":{"agent_id":"impl-paging-fields","role":"implementer","session_id":"s-0-34-paging-fields"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-34-atip-paging-fields.F5
  @atip.paging.15
  Scenario: The camelCase alias of a snake_case key and the snake_case alias of a report member are refused
    Given a repo with three subjects, a change with three findings and two divergent scenarios
    When canon query, finding list and divergence list run with --fields in the spellings 0.33 accepted: changeIds, resolution_sha, project_id, Reward
    Then each is exit 2 USAGE with field --fields and expected the declared paths; the spellings the records and reports use select
