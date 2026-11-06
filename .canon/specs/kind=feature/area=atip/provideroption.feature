Feature: atip harness option: the harnesses are chosen by a repeatable --harness and a switch --no-harness, and --providers is refused
  # canon: {"schema":1,"at":"2026-11-06T11:00:00Z","actor":{"agent_id":"impl-provider-option","role":"implementer","session_id":"s-0-38-provider-option"}}

  # canon: {"schema":1,"at":"2026-11-06T11:01:00Z","actor":{"agent_id":"impl-provider-option","role":"implementer","session_id":"s-0-38-provider-option"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-38-provider-option.F1
  @atip.provideroption.01
  Scenario: Harness names given once each, in any order, are one selection and the same config and install as 0.37 wrote
    Given init in a new git directory and skills install in the story repository, with --harness claude --harness omp, the same two in the other order, and omp twice, and each of the four names alone
    When each call runs
    Then canon.yaml records `providers: [claude, omp]` in name order and the install lock lists claude and omp, the three orders write the same tree, only claude and omp are projected, and each name alone is recorded alone

  # canon: {"schema":1,"at":"2026-11-06T11:01:13Z","actor":{"agent_id":"impl-provider-option","role":"implementer","session_id":"s-0-38-provider-option"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-38-provider-option.F1
  @atip.provideroption.02
  Scenario: A name that is not one of the four is refused with the four names and nothing is written
    Given Claude, a name with a space after it and gemini as the value of --harness
    When init and the three skills commands run with each
    Then each exits 2 with USAGE naming --harness, `expected` is claude, codex, omp and pi, `received` is the value, a near name is offered as a suggestion and an unknown one is not, and canon.yaml and the install are as they were

  # canon: {"schema":1,"at":"2026-11-06T11:01:26Z","actor":{"agent_id":"impl-provider-option","role":"implementer","session_id":"s-0-38-provider-option"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-38-provider-option.F2
  @atip.provideroption.03
  Scenario: --no-harness records an empty list, installs nothing and empties the lock's providers
    Given the story repository after an install for claude, and a new git directory holding a .claude directory
    When skills install --no-harness and init --no-harness (and its --dry-run) run
    Then canon.yaml ends with an empty harness list and the install lock lists no harness, init installs nothing whatever it detects, and a preview writes nothing

  # canon: {"schema":1,"at":"2026-11-06T11:01:39Z","actor":{"agent_id":"impl-provider-option","role":"implementer","session_id":"s-0-38-provider-option"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-38-provider-option.F2
  @atip.provideroption.04
  Scenario: --no-harness beside --harness, or beside --check for init, is refused naming both options and nothing is written
    Given --no-harness and --harness claude in both orders on init and the three skills commands, and init with --check beside --harness or --no-harness
    When each call runs
    Then each exits 2 with USAGE and a message that names every option of the clash, and nothing is written, canon.yaml included

  # canon: {"schema":1,"at":"2026-11-06T11:02:00Z","actor":{"agent_id":"impl-provider-option","role":"implementer","session_id":"s-0-38-provider-option"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-38-provider-option.F3
  @atip.provideroption.05
  Scenario: skills check and skills doctor serve the harnesses they are given, and only those
    Given a repository installed for claude and omp
    When skills check and skills doctor run with no option, with --harness omp, with --harness pi and with --no-harness
    Then with none they follow canon.yaml, with a name they serve that harness alone (the doctor names the other projection as a stray, the check finds a harness never installed missing), --no-harness selects nothing, and neither command writes

  # canon: {"schema":1,"at":"2026-11-06T11:02:13Z","actor":{"agent_id":"impl-provider-option","role":"implementer","session_id":"s-0-38-provider-option"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-38-provider-option.F3
  @atip.provideroption.06
  Scenario: skills check and skills doctor refuse what skills install refuses
    Given an unknown name, --harness beside --no-harness, and the old --providers
    When skills check and skills doctor run with each
    Then each exits 2 with USAGE naming the option, the old spelling's hint shows the corrected line, and nothing is written

  # canon: {"schema":1,"at":"2026-11-06T11:02:26Z","actor":{"agent_id":"impl-provider-option","role":"implementer","session_id":"s-0-38-provider-option"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-38-provider-option.F4
  @atip.provideroption.07
  Scenario: An --input document chooses as the flags do
    Given the documents {"harness":["claude","omp"]}, {"noHarness":true} and {"harness":["pi"]}, alone and as an array element, and the same choices as flags
    When init and skills install run with each document and with each choice as flags
    Then the document and the flags write the same tree, and an array of documents chooses for each element

  # canon: {"schema":1,"at":"2026-11-06T11:02:39Z","actor":{"agent_id":"impl-provider-option","role":"implementer","session_id":"s-0-38-provider-option"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-38-provider-option.F4
  @atip.provideroption.08
  Scenario: An --input document with no name, none as a name, an unknown name, the old key or a clash is refused and nothing is recorded
    Given {"harness":[]}, {"harness":["none"]}, {"harness":["Claude"]}, {"harness":"claude"}, {"providers":"claude"}, {"noHarness":"yes"}, a document that has both keys and a flag that differs from its key
    When init and skills install run with each
    Then each exits 2 (INPUT_INVALID, USAGE for both keys, INPUT_CONFLICT for the differing flag) naming the pointer of the fault, the old key is an unknown key and `expected` names the keys the command takes (harness and noHarness among them), and nothing is written

  # canon: {"schema":1,"at":"2026-11-06T11:03:00Z","actor":{"agent_id":"impl-provider-option","role":"implementer","session_id":"s-0-38-provider-option"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-38-provider-option.F5
  @atip.provideroption.09
  Scenario: The old --providers is refused with a hint whose line runs
    Given --providers claude,omp, --providers=omp,claude, --providers none and --providers pi,pi on init, and --providers codex,pi with the other options of each skills command
    When each runs, and then the line the hint shows runs
    Then each exits 2 with USAGE naming --providers and nothing is written, the hint is the typed line with one --harness for each name or --no-harness and every other option in place, and that line, run as the hint shows it, is no usage error

  # canon: {"schema":1,"at":"2026-11-06T11:03:13Z","actor":{"agent_id":"impl-provider-option","role":"implementer","session_id":"s-0-38-provider-option"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-38-provider-option.F5
  @atip.provideroption.10
  Scenario: A --providers value the old spelling did not take is refused and the hint names only the new options
    Given --providers with Claude, an empty item, a space, an unknown name, none beside a name, NONE and an empty value, on the four commands
    When each runs
    Then each exits 2 with USAGE naming --providers and nothing is written, `expected` is --harness, and the hint names --harness and --no-harness and offers no command line

  # canon: {"schema":1,"at":"2026-11-06T11:03:26Z","actor":{"agent_id":"impl-provider-option","role":"implementer","session_id":"s-0-38-provider-option"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-38-provider-option.F5
  @atip.provideroption.11
  Scenario: No help, message, guide or generated document says the old spelling
    Given the help and the command document of the four commands, what init and the three skills commands print in a bare repository and on a rerun, the guides the binary carries, the generated spec without its removed section, and the README, canon.yaml, the skill guides and the website pages
    When each is read for --providers
    Then none holds it, the help names --harness and --no-harness, and a text that does hold it is found

  # canon: {"schema":1,"at":"2026-11-06T11:03:39Z","actor":{"agent_id":"impl-provider-option","role":"implementer","session_id":"s-0-38-provider-option"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-38-provider-option.F6
  @atip.provideroption.12
  Scenario: The four commands declare one repeatable enumeration whose names are in possibleValues once, and the table of values agrees with the parser
    Given the command documents of init, skills install, skills check and skills doctor and a table of values, each name, Claude, none, an empty value, two names in one value, a leading space and gemini
    When the declarations are read and each value is run through each command
    Then each command declares --harness as a repeatable enum of claude, codex, omp and pi with no domain, --no-harness as a switch and an atMostOne of the two (and of --check for init), the four declarations are one, the input fragment names harness and noHarness, and the parser takes exactly the declared names

  # canon: {"schema":1,"at":"2026-11-06T11:04:00Z","actor":{"agent_id":"impl-provider-option","role":"implementer","session_id":"s-0-38-provider-option"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-38-provider-option.F6
  @atip.provideroption.13
  Scenario: A command that declares other names, a pattern, no repetition or the old option fails the check
    Given the real documents and copies in which one command lists a fifth name or a name twice, declares a pattern, is not repeatable, still declares --providers, lacks --no-harness or lacks the exclusion
    When the declarations are checked
    Then the real documents pass and each seeded fault is found by command and by what is wrong

  # canon: {"schema":1,"at":"2026-11-06T11:04:13Z","actor":{"agent_id":"impl-provider-option","role":"implementer","session_id":"s-0-38-provider-option"}}
  @subject:atip-schema
  @lane:behavior
  @case:happy
  @flow:canon-0-38-provider-option.F7
  @atip.provideroption.14
  Scenario: The CHANGELOG has a row for each shape of the old value
    Given the Breaking table of the Unreleased entry
    When its rows are read
    Then it has a row for a comma list, for none and for the providers key of an input document (the old spelling), each with the old spelling, what 0.37 did, what 0.38 does and how to move, and the move of each is a line that runs

  # canon: {"schema":1,"at":"2026-11-06T11:04:26Z","actor":{"agent_id":"impl-provider-option","role":"implementer","session_id":"s-0-38-provider-option"}}
  @subject:atip-schema
  @lane:behavior
  @case:failure
  @flow:canon-0-38-provider-option.F7
  @atip.provideroption.15
  Scenario: The check of the CHANGELOG fails for a missing row, an empty cell and a missing header
    Given the CHANGELOG with each row taken out, a row with no way to move and the header taken out
    When the check of the CHANGELOG runs on each
    Then it names the missing shape, the row with an empty cell and the missing header, and the real CHANGELOG passes
