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
    When canon --help --json, canon -h --json and canon finding --help --json run
    Then the root is exit 2 USAGE with field --help and the hint "canon schema --json" and no meta.command, the group has the hint "canon schema finding --json" and meta.command finding

  # canon: {"schema":1,"at":"2026-11-04T00:53:21Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-36-help-json.F1
  @atip.helpjson.03
  Scenario: A word after -- is not a help request or a JSON switch, and --help without --json is still text with exit 0
    Given a search whose query is the word --help and a call that gives --json only after the end-of-options marker
    When canon search --json -- --help runs, canon search -- --help --json runs, and canon finding close --help runs without --json
    Then the first is a search of the query --help and is no refusal of --help, the second is a text USAGE on stderr whose field is --json and not --help with stdout empty, and the third prints the text help on stdout with exit 0 and no JSON

  # canon: {"schema":1,"at":"2026-11-04T00:53:31Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-36-help-json.F2
  @atip.helpjson.04
  Scenario: A word that is no command still answers UNKNOWN_COMMAND with the words that would parse, whether --help comes before or after it
    Given the words nosuch, finding nosuch and the flag --help placed before the unknown word
    When canon nosuch --help --json, canon finding nosuch --help --json and canon --help nosuch --json run
    Then each is exit 2 UNKNOWN_COMMAND with field command, the word as received and expected the commands that are valid at that place, and none is the --help refusal

  # canon: {"schema":1,"at":"2026-11-04T00:53:36Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-36-help-json.F3
  @atip.helpjson.05
  Scenario: --version or -V with --json is refused with USAGE, field --version and the hint canon --version, on the root, a group and a command
    Given the root, the group finding and the command finding close
    When canon --version --json, canon -V --json, canon finding --version --json and canon finding close -V --json run
    Then each is exit 2 USAGE with field --version and the hint "canon --version", the version is not printed outside meta.toolVersion, and meta.command is the resolved path

  # canon: {"schema":1,"at":"2026-11-04T00:53:42Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-36-help-json.F3
  @atip.helpjson.06
  Scenario: --version --json after an unknown word answers UNKNOWN_COMMAND, and --version without --json prints one line and exits 0
    Given an unknown word before the flag, and a call without --json
    When canon nosuch --version --json, canon finding nosuch -V --json, canon --version and canon -V run
    Then the first two are exit 2 UNKNOWN_COMMAND and the last two print one line "canon <version>" on stdout with exit 0

  # canon: {"schema":1,"at":"2026-11-04T00:53:47Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-36-help-json.F1
  @atip.helpjson.07
  Scenario: The refusal reads and writes nothing: the same answer outside a repository, and a repository is byte-identical after it
    Given an empty directory with no canon.yaml and a repository with a change and a finding
    When canon finding close --help --json and canon --version --json run in each
    Then the answers are the same USAGE refusals with exit 2 and never CONFIG_MISSING, and the tree of the repository (files and bytes) is identical before and after

  # canon: {"schema":1,"at":"2026-11-04T00:53:51Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-schema
  @case:edge
  @flow:canon-0-36-help-json.F4
  @atip.helpjson.08
  Scenario: The text help and version are as they were: the first line of every help points at schema and skills show, and canon help <command> stays text
    Given the root help, a group help, a leaf help, the schema help and canon help finding
    When each runs without --json, and canon help finding runs with --json
    Then the first line of every help is the pointer to canon schema --json and canon skills show, and canon help finding --json still prints text with exit 0

  # canon: {"schema":1,"at":"2026-11-04T00:54:04Z","actor":{"agent_id":"impl-help-json","role":"implementer","session_id":"s-0-36-help-json"}}
  @subject:atip-schema
  @case:edge
  @flow:canon-0-36-help-json.F4
  @atip.helpjson.09
  Scenario: No guide, README, doc page, script or skill recommends --help --json as the machine description
    Given the guides, README files, doc pages, scripts, skills and website sources of the repository, apart from the changelog, the briefs and the research notes that name the old behavior
    When their text is searched for --help with --json on one line
    Then none recommends them as the machine description; the recommendation is canon schema
