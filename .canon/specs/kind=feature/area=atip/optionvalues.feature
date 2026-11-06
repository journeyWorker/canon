Feature: atip option values: an option of the same name takes the same value in every command that has it: the id, the one-line name and the one-line sentence
  # canon: {"schema":1,"at":"2026-11-05T14:12:00Z","actor":{"agent_id":"impl-option-values","role":"implementer","session_id":"s-0-37-option-values"}}

  # canon: {"schema":1,"at":"2026-11-05T14:12:11Z","actor":{"agent_id":"impl-option-values","role":"implementer","session_id":"s-0-37-option-values"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-37-option-values.F1
  @atip.optionvalues.01
  Scenario: The id options --project, --role and --domain declare the one kebab slug in every command and take a value inside it
    Given every command document of canon schema, and a value inside the slug for --project, --role and --domain in the commands that can run on the story repository
    When the declarations are read and the calls run
    Then each of the 25 declarations (3 of --domain, 11 of --project, 11 of --role) is the kebab slug pattern, divergence accept --role keeps its reviewer-only refusal as a validator beside it, and the calls with a value inside the slug are taken

  # canon: {"schema":1,"at":"2026-11-05T14:12:22Z","actor":{"agent_id":"impl-option-values","role":"implementer","session_id":"s-0-37-option-values"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-37-option-values.F1
  @atip.optionvalues.02
  Scenario: A project, role or domain that is no kebab slug is refused with exit 2 and nothing is written
    Given --project a- and --project a--b, which 0.36.0 took, and the values -a, A, a_b, a b and the empty one for --project and --role, and --domain Dev, a_b, a-, a--b and a b for query, retrieve and subject new
    When each call runs on a copy of the story repository with --json
    Then each exits 2 with the code the command already used for that option and the field of the option, writes nothing, and a role that is a slug but not the reviewer is still DIVERGENCE_DENIED for divergence accept

  # canon: {"schema":1,"at":"2026-11-05T14:12:33Z","actor":{"agent_id":"impl-option-values","role":"implementer","session_id":"s-0-37-option-values"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-37-option-values.F2
  @atip.optionvalues.03
  Scenario: A name is trimmed and the trimmed value is what is recorded and signed, and every name and sentence option declares one domain in every command
    Given --actor with white space around it on scenario new and finding add, --approver with white space around it on evidence approval-payload, and every command document that has --actor, --approver, --relayed-via, --reason, --summary or --override-reason
    When the calls run and the documents are read
    Then the record holds alice and not the padded value, the payload signed over the padded approver is the payload of the trimmed one, and the 43 declarations of the six options are one pattern with the class [^\p{Cc}\p{Zl}\p{Zp}]

  # canon: {"schema":1,"at":"2026-11-05T14:12:44Z","actor":{"agent_id":"impl-option-values","role":"implementer","session_id":"s-0-37-option-values"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-37-option-values.F2
  @atip.optionvalues.04
  Scenario: A blank name, or one with a control character or a line break inside, is refused with the command's own code and nothing is recorded
    Given a name that is empty, only white space, or has a line feed, a tab or an escape inside, for --actor and --approver in every command that takes them
    When the declared domain of each command judges the values and six commands answer the same values with --json
    Then each domain refuses what the table says and takes the spaces, the no-break space and the scripts of the table, and each call exits 2 with the code of its command, the field and no written file

  # canon: {"schema":1,"at":"2026-11-05T14:12:55Z","actor":{"agent_id":"impl-option-values","role":"implementer","session_id":"s-0-37-option-values"}}
  @subject:atip-schema
  @lane:behavior
  @case:edge
  @flow:canon-0-37-option-values.F2
  @atip.optionvalues.05
  Scenario: The tool of a relayed approval is one word, the same on the four commands that take --relayed-via
    Given brief approve, policy approve, divergence accept and migrate called with --relayed-via, with a tool of one word and with tools that have a space, a parenthesis, a quote, a no-break space, a tab, a line break, an escape or nothing
    When each call runs with --as implementer
    Then the four commands declare the same pattern and the same one-word validator in the same words, take ask, AskUserQuestion and chat, and refuse every other value with USAGE naming --relayed-via and writing nothing

  # canon: {"schema":1,"at":"2026-11-05T14:13:06Z","actor":{"agent_id":"impl-option-values","role":"implementer","session_id":"s-0-37-option-values"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-37-option-values.F3
  @atip.optionvalues.06
  Scenario: A sentence is trimmed and the trimmed value is what is recorded and hashed
    Given --reason on finding close and divergence add, --summary on finding add, each with white space around it, and --reason on strategy rollback parsed with white space around it
    When the calls run and the command line is parsed
    Then each record holds the trimmed sentence, and the reason of a rollback that is hashed and signed is the trimmed one

  # canon: {"schema":1,"at":"2026-11-05T14:13:17Z","actor":{"agent_id":"impl-option-values","role":"implementer","session_id":"s-0-37-option-values"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-37-option-values.F3
  @atip.optionvalues.07
  Scenario: A blank sentence, or one with a control character, a line break or an escape inside, is refused with the command's own code
    Given --reason, --summary and --override-reason with a line feed, a tab, an escape, white space only and nothing, in the ten commands that take one, and a rollback reason with a line break
    When each call runs with --json
    Then each exits 2 with the code its command used for that option (FINDING_REASON_REQUIRED for the reason of finding close), names the option and writes nothing

  # canon: {"schema":1,"at":"2026-11-05T14:13:28Z","actor":{"agent_id":"impl-option-values","role":"implementer","session_id":"s-0-37-option-values"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-37-option-values.F4
  @atip.optionvalues.08
  Scenario: retrieve and dispatch begin declare the same regime key and take the same keys
    Given the documents of retrieve and dispatch begin and the keys reviewer/canon/join-spine/9c93d024b1a2, dev/my-repo/api/abcdef and one with a 64 digit hash
    When the two --regime declarations are read and each key is judged and then passed to each command
    Then the domains are equal, every key is inside both, and neither command answers about --regime

  # canon: {"schema":1,"at":"2026-11-05T14:13:39Z","actor":{"agent_id":"impl-option-values","role":"implementer","session_id":"s-0-37-option-values"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-37-option-values.F4
  @atip.optionvalues.09
  Scenario: A regime key of three parts or with a space is refused by dispatch begin as retrieve refuses it
    Given keys with three parts, a space, an uppercase hash, a hash of five digits, an empty part and a fifth part, and a key whose first part is not the role
    When each key is passed to dispatch begin and to retrieve with --json
    Then both exit 2 with USAGE naming --regime and write nothing, and dispatch begin still answers DISPATCH_ROLE_CONFLICT for a key of another role

  # canon: {"schema":1,"at":"2026-11-05T14:13:50Z","actor":{"agent_id":"impl-option-values","role":"implementer","session_id":"s-0-37-option-values"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-37-option-values.F5
  @atip.optionvalues.10
  Scenario: --round and --limit start at 1 in every command and finding close takes fixed, rejected and deferred
    Given every command document with --round or --limit, finding close and finding add, and calls with --round 1, --limit 1 and --disposition fixed
    When the documents are read and the calls run
    Then every minimum is 1, finding close lists three dispositions and finding add four, and the calls inside the bounds are taken

  # canon: {"schema":1,"at":"2026-11-05T14:14:01Z","actor":{"agent_id":"impl-option-values","role":"implementer","session_id":"s-0-37-option-values"}}
  @subject:finding-lifecycle
  @lane:behavior
  @case:failure
  @flow:canon-0-37-option-values.F5
  @atip.optionvalues.11
  Scenario: --round 0, --limit 0 and finding close --disposition open are refused with exit 2 and nothing is recorded
    Given divergence add, defer and resolve with --round 0, finding close with --round 0, retrieve with --limit 0, and finding close with --disposition open on the command line and in an --input document
    When each call runs with --json
    Then each exits 2 and writes nothing, the open disposition is USAGE naming --disposition and the three it takes, and the document one is INPUT_INVALID at /disposition

  # canon: {"schema":1,"at":"2026-11-05T14:14:12Z","actor":{"agent_id":"impl-option-values","role":"implementer","session_id":"s-0-37-option-values"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-37-option-values.F6
  @atip.optionvalues.12
  Scenario: Every option of the same name has one domain in every command
    Given every command document of canon schema and the one name allowed by a reason, --kind (`--harness` declares no domain at all since `canon-0-38-provider-option`: it is an enumeration)
    When the sweep groups the declarations by option name
    Then no name has two domains or closed sets that are not nested outside the allowed list, the sweep reads more than 80 names and more than 25 shared ones, and every option of the three rules has one domain

  # canon: {"schema":1,"at":"2026-11-05T14:14:23Z","actor":{"agent_id":"impl-option-values","role":"implementer","session_id":"s-0-37-option-values"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-37-option-values.F6
  @atip.optionvalues.13
  Scenario: A command that declares another domain for the same name fails the sweep
    Given the real documents with one command given the pattern of a bare id for --actor, one without a domain for --reason and one with a closed set that is not nested for --disposition
    When the sweep reads each seeded set of documents
    Then it finds each seeded fault by the option name and the commands, finds --kind when it is not allowed, and passes the real documents with the one allowed

  # canon: {"schema":1,"at":"2026-11-05T14:14:34Z","actor":{"agent_id":"impl-option-values","role":"implementer","session_id":"s-0-37-option-values"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-37-option-values.F7
  @atip.optionvalues.14
  Scenario: The CHANGELOG has a row for every value 0.36 took and this build refuses, and an entry for the values recorded trimmed
    Given the golden file of what 0.36.0 accepted per option, command and class of value, measured on the released binary, and the Breaking table of the Unreleased entry
    When the build judges every golden value with the declared domain of its command and the table is read
    Then each value the build refuses has a row that names its option, its command and its class, with the answer and the way to move, no row names a value that is not refused, and one entry says the values are recorded without the white space around them

  # canon: {"schema":1,"at":"2026-11-05T14:14:45Z","actor":{"agent_id":"impl-option-values","role":"implementer","session_id":"s-0-37-option-values"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-37-option-values.F7
  @atip.optionvalues.15
  Scenario: The check of the CHANGELOG fails for a missing row, a missing answer and a missing entry
    Given the CHANGELOG with one row taken out, with the class of the other control characters left out of every row, with a row that has no way to move, and with the entry for the trimmed values reworded, and a refused value no row names
    When the check reads each
    Then it finds the missing row by its option and class, the missing class by name, the row with no way to move, the missing entry and the unlisted class, and passes the complete CHANGELOG

  # canon: {"schema":1,"at":"2026-11-05T14:14:56Z","actor":{"agent_id":"impl-option-values","role":"implementer","session_id":"s-0-37-option-values"}}
  @subject:atip-contract
  @lane:behavior
  @case:edge
  @flow:canon-0-37-option-values.F1
  @atip.optionvalues.16
  Scenario: The checker fixture lists values the new domains refuse, and the recipes run the checker at 1.5
    Given the checker fixture and the justfile
    When each listed value is judged by the declared domain and the recipes are read
    Then every listed value is outside the domain of its command, the cells of the new rules are listed, the fixture claims 1.5 and the recipes pass --contract 1.5
