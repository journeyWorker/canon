Feature: atip constraints: the relations, domains and validators of the command documents
  # canon: {"schema":1,"at":"2026-11-01T10:58:38Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints"}}

  # canon: {"schema":1,"at":"2026-11-01T10:58:51Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-34-atip-constraints.F1
  @atip.constraints.01
  Scenario: What clap enforces is in the constraints of the command document: the options that conflict with --check, --policy with --no-policy, the exactly-one pair of review scenario, and the --commit and --plan-digest pair
    Given a built canon
    When canon schema init --json, schema review scenario --json and schema purge --json run
    Then init lists --check with each option it conflicts with and --policy with --no-policy as atMostOne, review scenario lists exactlyOne --upstream-ref and --original-spec-ref, purge lists the two requires of --commit and --plan-digest, and no command states one relation twice

  # canon: {"schema":1,"at":"2026-11-01T10:59:19Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-34-atip-constraints.F1
  @atip.constraints.02
  Scenario: A relation stated twice in one command is found, and --commit without --plan-digest is USAGE naming --plan-digest
    Given a built canon and a document with the same atMostOne written twice
    When the document check runs, and canon purge runs with only --commit, then with only --plan-digest
    Then the check names the relation as stated twice; each purge exits 2 with USAGE and field --plan-digest, then --commit

  # canon: {"schema":1,"at":"2026-11-01T10:59:32Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-34-atip-constraints.F2
  @atip.constraints.03
  Scenario: The relations the handlers enforce are constraints with their codes, and every declared rule is refused with its code when broken and not refused for it when met
    Given a built canon and the story repo
    When canon schema retrieve, finding close, finding add and evidence add run, and every declared rule of every command is driven through the binary with a call that breaks only it and a call that meets it
    Then retrieve lists exactlyOne regime or domain and subject requires domain; finding close demands --reason for rejected or deferred with FINDING_REASON_REQUIRED; each breaking call exits 2 with the rule's code and a field that is one of its arguments and writes nothing; each meeting call is not refused for the rule

  # canon: {"schema":1,"at":"2026-11-01T10:59:43Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-34-atip-constraints.F2
  @atip.constraints.04
  Scenario: A handler check that no document declares and no table classifies fails the survey of the sources, and a classified check that is gone fails it too
    Given a built canon and the source of its handlers
    When a new refusal of an argument that no document declares is added to a handler, and a classified check is deleted
    Then the survey names the new refusal and the stale classification, and passes when neither is there

  # canon: {"schema":1,"at":"2026-11-01T10:59:55Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-34-atip-constraints.F3
  @atip.constraints.05
  Scenario: Every domain agrees with the real parser just inside and just outside its bounds, with the code its row names
    Given a built canon and the story repo
    When every domain of every command is asked of the parser with a value just inside and a value just outside it
    Then the parser agrees at every bound with the code the row names: finding close --round 0 is FINDING_INVALID_ARGUMENT, query --limit 0 and --max-artifact-mib 0 are USAGE, an id outside its pattern is refused

  # canon: {"schema":1,"at":"2026-11-01T11:00:03Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-34-atip-constraints.F3
  @atip.constraints.06
  Scenario: A pattern that is not a restricted I-Regexp, one the Rust regex refuses, a keyword outside the profile and a default outside its domain are each found, and every id pattern agrees with its Rust parser
    Given domains that leave the contract's profile
    When the document check and the id pattern comparison run
    Then a pattern with an alternation outside parentheses, a class escape or a dot, a pattern the Rust regex refuses, a keyword outside the profile of the value type, a bound beyond 2^53-1 and a default outside the domain are each named; each id pattern accepts and refuses what its Rust parser does, and the time pattern admits everything the time parser accepts

  # canon: {"schema":1,"at":"2026-11-01T11:00:10Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-34-atip-constraints.F4
  @atip.constraints.07
  Scenario: Every validator refuses every call it rejects with its code, and an impossible calendar date is refused by all 12 time options
    Given a built canon and the story repo
    When the rejects call of every validator runs, and --since, --before, --after, --expiry, --as-of and --approval-at are each given 2026-02-30
    Then every rejects call exits 2 with the validator's code and a field that is one of its arguments and writes nothing; each of the 12 time options is named by the calendar validator of its command and refuses the date

  # canon: {"schema":1,"at":"2026-11-01T11:00:15Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-34-atip-constraints.F4
  @atip.constraints.08
  Scenario: An export window whose --after is later than its --before is USAGE naming --after and writes nothing, and an equal or ordered window still succeeds
    Given a built canon and the story repo
    When canon export --kind change --before 2026-01-02 --after 2026-02-01 runs, also with --dry-run and with --output, and with an equal pair and an earlier --after
    Then the late --after exits 2 with USAGE and field --after before any read or write and no output file; the equal pair and the ordered pair exit 0

  # canon: {"schema":1,"at":"2026-11-01T11:00:20Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-34-atip-constraints.F4
  @atip.constraints.09
  Scenario: A validator whose rejects call is accepted, is refused with another code or is also an example is found
    Given a validator whose rejects call the binary accepts, one whose code the binary does not give and one whose rejects is also an example
    When the validator sweep runs on each
    Then each is found with the reason

  # canon: {"schema":1,"at":"2026-11-01T11:00:25Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-34-atip-constraints.F5
  @atip.constraints.10
  Scenario: The edge of the safe integer range, 9007199254740991, is accepted by every 64-bit integer option and no declared domain goes beyond it
    Given a built canon and the story repo
    When the 64-bit integer options run with 9007199254740991
    Then every one accepts it, and no declared domain maximum is above it

  # canon: {"schema":1,"at":"2026-11-01T11:00:32Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-34-atip-constraints.F5
  @atip.constraints.11
  Scenario: A 64-bit integer option outside plus or minus 2^53-1 is refused with USAGE naming the option and is never rounded into range
    Given a built canon and the story repo
    When the 64-bit integer options run with 9007199254740992, 9007199254740993, 18446744073709551616, -9007199254740992 and -1
    Then every one exits 2 with USAGE and the option as field; 9007199254740993 is not read as 9007199254740992

  # canon: {"schema":1,"at":"2026-11-01T11:00:38Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-34-atip-constraints.F6
  @atip.constraints.12
  Scenario: Every example meets the constraints and the domains of its command, and the guides and the checked-in spec read the constraints of the documents
    Given a built canon
    When every example of every command is read against its own constraints and domains, and the guides read the relations from the constraints
    Then every example satisfies them and parses with the real parser; the relations a guide writes are the rules the document lists

  # canon: {"schema":1,"at":"2026-11-01T11:00:43Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-34-atip-constraints.F6
  @atip.constraints.13
  Scenario: A constraint, a code or a domain declared with no check behind it is found by the sweeps
    Given a constraint declared with no check behind it, a rule with a code the binary does not give and a domain the parser does not hold
    When the rule and the domain sweeps run on each
    Then each is found: the call that should break the rule is not refused for it, and the value outside the domain is accepted

  # canon: {"schema":1,"at":"2026-11-01T14:23:07Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints-r2"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-34-atip-constraints.F2
  @atip.constraints.14
  Scenario: A relayed approval is a relation: its role, its actor and its answer are validators over --relayed-via, the environment is declared on --as and --actor, and the human's blank answer has its own code
    Given a built canon
    When canon schema brief approve, policy approve, divergence accept and migrate run
    Then each lists a validator over --relayed-via and --as and one over --relayed-via, --approver and --note, all USAGE; brief approve, policy approve and migrate list one over --relayed-via and --actor, and --as and --actor carry env CANON_ROLE and CANON_ACTOR; --approver and --note of brief approve and policy approve have no domain and a validator each with APPROVAL_INCOMPLETE or POLICY_APPROVAL_REFUSED

  # canon: {"schema":1,"at":"2026-11-01T14:23:12Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints-r2"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-34-atip-constraints.F2
  @atip.constraints.15
  Scenario: A relayed approval without a role, an actor or an answer, and a blank human answer, are refused in an empty directory with their codes, and CANON_ACTOR and CANON_ROLE fill the first two
    Given an empty directory with an empty HOME and no CANON_ROLE or CANON_ACTOR
    When brief approve and policy approve run relayed without a role, with a role and no actor, and with a blank approver or note, and unrelayed with a blank answer; then with CANON_ROLE and CANON_ACTOR set
    Then each refusal is exit 2 with its code (USAGE naming --relayed-via, --actor, --approver or --note, APPROVAL_INCOMPLETE, POLICY_APPROVAL_REFUSED) and writes nothing; with the two variables set the call goes on to the repository and answers CONFIG_MISSING

  # canon: {"schema":1,"at":"2026-11-01T14:23:16Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints-r2"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-34-atip-constraints.F4
  @atip.constraints.16
  Scenario: The shape of --input is the parser's: an array on a command that takes no batch, --input twice and --input - beside another stdin reader are refused by the parser with no validator declared
    Given an empty directory with an empty HOME
    When purge, ledger compact, tier age, artifacts move, dashboard and migrate run with --input a two-object array, purge runs with --input twice, and evidence add runs with --input - and --report junit:- or --artifact - in both orders
    Then each array is exit 2, INPUT_INVALID, reason BATCH_UNSUPPORTED, field --input; the twice is USAGE naming --input; the stdin pair is USAGE naming the option given later; no document declares a validator over input, and no rejected call or example carries --input; nothing is written

  # canon: {"schema":1,"at":"2026-11-01T14:23:19Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints-r2"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-34-atip-constraints.F2
  @atip.constraints.17
  Scenario: EVIDENCE_APPROVAL_INVALID is one code with the reasons MALFORMED, from the arguments alone, and SIGNATURE_MISMATCH, from the signature
    Given a built canon and the story repo
    When canon schema --error EVIDENCE_APPROVAL_INVALID runs, evidence add runs with one to three of the four approval options or a blank approver in an empty directory, and with all four and a signature file that does not exist in the repo
    Then the registry lists the reasons MALFORMED and SIGNATURE_MISMATCH under the code, exit 2; the incomplete sets are MALFORMED naming the missing option; the missing file is SIGNATURE_MISMATCH naming --signature-file

  # canon: {"schema":1,"at":"2026-11-01T16:13:30Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints-r3"}}
  @subject:atip-schema
  @case:happy
  @flow:canon-0-34-atip-constraints.F2
  @atip.constraints.18
  Scenario: The domains and the constraints a document declares answer before any state is read, in the contract's order: domains, constraints in the document's order, then validators
    Given an empty directory with an empty HOME, and the story repo
    When finding close without --reason, with --round 0, divergence add --state deferred without --expiry, evidence add --command-result without --summary, --task with --report-case, evidence approval-payload without --task or --scenario, and a call that breaks a domain, a constraint and a validator together
    Then each is refused with the code the document declares and the argument at fault, in the empty directory and in the repository alike, and writes nothing; of a domain, a constraint and a validator broken together the domain answers, then the constraint in the document's order, then the validator; field is the first argument of the rule in its own order, not in argv order

  # canon: {"schema":1,"at":"2026-11-01T16:13:35Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints-r3"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-34-atip-constraints.F2
  @atip.constraints.19
  Scenario: Every combination of the arguments that the rules of a command name is refused by a rule it breaks, or by none, whatever else is given with it
    Given an empty directory
    When every combination of the arguments each group of rules of a command names runs, with the values an if.in list needs, all of them up to 1500 per group and a fixed-seed sample beyond
    Then a call that breaks a rule is refused with the code of a rule it breaks and a field that rule names, and writes nothing; a call that breaks none is refused by none of the rules

  # canon: {"schema":1,"at":"2026-11-01T16:13:39Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints-r3"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-34-atip-constraints.F2
  @atip.constraints.20
  Scenario: A requires that clap skips because the argument it names conflicts with one present is enforced: migrate with --backup and --relayed-via, or --revert with --actor
    Given an empty directory
    When migrate runs with --backup and --relayed-via, with --revert and --actor, --approver or --note, and with --backup alone
    Then each is USAGE naming the first missing argument of the rule (--revert or --relayed-via), although clap's own check skips it; the same calls with their requirements met are not refused for the rule

  # canon: {"schema":1,"at":"2026-11-01T16:13:42Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints-r3"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-34-atip-constraints.F5
  @atip.constraints.21
  Scenario: A negative integer after an integer option names the option in field, and -0 is the integer 0
    Given an empty directory
    When query --limit, artifacts store --max-artifact-mib, ingest sessions --interval-secs, retrieve --limit and finding close --round run with a negative number after a space, and with -0 and +0
    Then the negative number is USAGE with field the option, never the number; -0 and +0 are the integer 0 where 0 is in the domain and refused for the minimum where it is not; 9007199254740991 is accepted and 9007199254740992 refused

  # canon: {"schema":1,"at":"2026-11-01T16:13:45Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints-r3"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-34-atip-constraints.F2
  @atip.constraints.22
  Scenario: The retrieve refusal of a role that disagrees with its regime names --for-role
    Given an empty directory
    When canon retrieve --for-role reviewer --regime dev/canon/join-spine/9c93d024b1a2 runs
    Then the refusal is USAGE naming --regime and its message names --for-role, not --role

  # canon: {"schema":1,"at":"2026-11-01T17:50:33Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints-r4"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-34-atip-constraints.F4
  @atip.constraints.23
  Scenario: The stateless stage is part of batch preflight: a domain, constraint or validator fault in any element refuses the batch with exit 2 before it writes
    Given the story repo
    When finding add runs with --input an array of two elements, one good and one with a blocker and no origin, a round of 0, both origins, or a line separator, in either order
    Then each is exit 2 with the code of the fault (FINDING_INTRODUCED_REQUIRED, FINDING_INVALID_ARGUMENT) and a field pointed at the faulty element, and nothing is written, not even the good element; the same elements one at a time are refused the same way, and an array of good elements writes

  # canon: {"schema":1,"at":"2026-11-01T17:50:38Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints-r4"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-34-atip-constraints.F2
  @atip.constraints.24
  Scenario: The calendar of a time option, the form of --surface-ref and --introduced-rev are validators of the stage and answer after the domains and constraints
    Given an empty directory
    When a call breaks a constraint and carries an impossible date, a finding add breaks exclusivity and carries --introduced-rev self, an evidence add breaks all-scenarios and carries --surface-ref /etc/passwd, and each of them alone
    Then the constraint answers first; alone, the shape of a time is the declared domain's refusal, the calendar, the surface-ref form and `self` are USAGE naming the option; a batch with such an element is refused in preflight

  # canon: {"schema":1,"at":"2026-11-01T17:50:42Z","actor":{"agent_id":"impl-constraints","role":"implementer","session_id":"s-0-34-constraints-r4"}}
  @subject:atip-schema
  @case:failure
  @flow:canon-0-34-atip-constraints.F5
  @atip.constraints.25
  Scenario: An integer refusal names the declared bound and clap's range sentence never contradicts a declared minimum
    Given an empty directory
    When finding close runs with --round 0, --round -1 and --seq 4294967296, and query --limit 0
    Then the refusal says the declared bound (at least 1; from 1 to 4294967295; from 1 to 9007199254740991) and clap's `0..=4294967295` never appears
