Feature: atip help json: --help and --version with --json are refused and point at schema (spec 4.4.1)
  # canon: {"schema":1,"at":"2026-11-04T00:52:51Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}

  # canon: {"schema":1,"at":"2026-11-04T00:53:03Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-36-help-json.F1
  @atip.helpjson.01
  Scenario: --help or -h with --json on a command is refused with USAGE, field --help and the hint of its schema call, wherever --json sits
    Given the command finding close
    When canon finding close --help --json, canon finding close --json -h and canon -h --json finding close run
    Then each is exit 2 USAGE with field --help and the hint "canon schema finding close --json", meta.command is finding close, and stdout is one JSON document and stderr is empty

  # canon: {"schema":1,"at":"2026-11-04T00:53:13Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-schema
  @case:edge
  @flow:canon-0-36-help-json.F1
  @atip.helpjson.02
  Scenario: The root and a group are refused the same way, and the hint is the schema call of the path the call named
    Given the root canon and the group canon finding
    When canon -h --json, canon --json --help and canon --json finding -h run
    Then the root is exit 2 USAGE with field --help and the hint "canon schema --json" and no meta.command, the group has the hint "canon schema finding --json" and meta.command finding

  # canon: {"schema":1,"at":"2026-11-04T00:53:21Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-36-help-json.F1
  @atip.helpjson.03
  Scenario: A word after -- is not a help request, and --help without --json is still text with exit 0
    Given a search whose query is the word --help after the end-of-options marker
    When canon search --json -- --help runs, and canon finding close -h, canon --help and canon finding --help run without --json
    Then the first is a search of the query --help and is no refusal of --help, and each of the others exits 0 with no JSON document and nothing on stderr

  # canon: {"schema":1,"at":"2026-11-04T00:53:31Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-36-help-json.F2
  @atip.helpjson.04
  Scenario: A word that is no command still answers UNKNOWN_COMMAND, with --help before it
    Given the words nosuch and finding nosuch and the flag --help or -h placed before the unknown word
    When canon --help nosuch --json, canon --json -h finding nosuch and canon --help finding nosuch --json run
    Then each is exit 2 UNKNOWN_COMMAND with field command, the word as received and expected holding a command that is valid at that place, and none is the --help refusal

  # canon: {"schema":1,"at":"2026-11-04T00:53:36Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-36-help-json.F3
  @atip.helpjson.05
  Scenario: --version or -V with --json is refused with USAGE, field --version and the hint canon --version, on the root, a group and a command
    Given the root, the group finding and the command finding close
    When canon --json --version, canon finding --version --json and canon finding close -V --json run
    Then each is exit 2 USAGE with field --version and the hint "canon --version", and meta.command is the resolved path

  # canon: {"schema":1,"at":"2026-11-04T00:53:42Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-36-help-json.F3
  @atip.helpjson.06
  Scenario: -V --json after an unknown word answers UNKNOWN_COMMAND, and -V without --json prints one line and exits 0
    Given an unknown word before the flag, and a call without --json
    When canon finding nosuch -V --json and canon -V run
    Then the first is exit 2 UNKNOWN_COMMAND and the second prints one line "canon <version>" on stdout with exit 0 and nothing on stderr

  # canon: {"schema":1,"at":"2026-11-04T00:53:47Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-36-help-json.F1
  @atip.helpjson.07
  Scenario: The refusal reads and writes nothing: the same answer outside a repository, and a repository is byte-identical after it
    Given an empty directory with no canon.yaml and a repository with a change and a finding
    When canon finding close --help --json, canon --version --json and canon --help --json run in each
    Then the answers are the same JSON document in both and never CONFIG_MISSING, and the tree of the repository (files and bytes) is identical before and after

  # canon: {"schema":1,"at":"2026-11-04T00:53:51Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-schema
  @case:edge
  @flow:canon-0-36-help-json.F4
  @atip.helpjson.08
  Scenario: The text help is as it was: every help points at schema and skills show, also canon help <command> with or without --json
    Given the root help, a group help, a leaf help, the schema help and canon help finding
    When each runs without --json, and canon help finding runs with --json
    Then every help names canon schema --json and canon skills show with exit 0, and so does canon help finding --json
