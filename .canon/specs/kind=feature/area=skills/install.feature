Feature: skills install
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:skills-install
  @case:happy
  @skills.install.01
  Scenario: Installing materializes both agent conventions and records what it wrote
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a source tree holding one agent guide
    When the guides are installed into a target repo
    Then the guide is reported installed at version one as changed
    And its file under the Claude convention is byte-verbatim against the source
    And its file under the Codex convention, the skill directory Codex discovers, is byte-verbatim against the source as well
    And the lock records that guide's content hash and its version
    And no third agent directory is created

  @subject:skills-install
  @case:edge
  @skills.install.02
  Scenario: A rerun with no source change reports unchanged rather than rewriting
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a target repo where the guides have already been installed once
    When install is run a second time with nothing changed in the source
    Then both materialized files and the lock are byte-identical to the first run
    And the guide is reported unchanged, because its content hash matched the lock entry
    And its recorded version did not move

  @subject:skills-install
  @case:happy
  @skills.install.03
  Scenario: An edited guide bumps its recorded version by exactly one
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a guide installed at version one
    When its source content is edited and install is run again
    Then the guide is reported changed
    And its recorded version is two, incremented by exactly one

  @subject:skills-install
  @case:edge
  @skills.install.04
  Scenario: The lock keys on content, never on the clock
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given the same bytes hashed twice and one differing byte hashed once
    When the hashes are compared
    Then the two identical inputs produce the same hash and the differing input a different one
    And the hash names its algorithm, so an install decision is reproducible from content alone

  # canon: {"schema":1,"at":"2026-10-08T14:02:45Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:failure
  @skills.install.07
  Scenario: An unknown provider is refused before anything is written
    Given a canonical skill source and an empty target
    When the skill is installed for a provider canon does not know
    Then the install is refused with exit 2, the provider named
    And no projection or lock file is written to the target

  # canon: {"schema":1,"at":"2026-10-10T14:03:19Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:happy
  @skills.install.08
  Scenario: Codex receives the skill where Codex discovers skills, as a stub with the source's frontmatter
    Given a canonical skill source whose frontmatter names the skill and describes it, with reference and script sidecars
    When the skill is installed for Codex
    Then Codex's skill file is a stub in the canon skill directory under the agents skills root Codex scans, carrying the source's frontmatter, pointing at canon skills show canon and naming this canon's version
    And its frontmatter opens the file and carries the skill's name and a non-empty description
    And the script sidecars sit beside it and the reference files are not projected, because the guide comes from the binary
    And nothing is written under the Codex config directory
    And a target holding the Codex config directory selects Codex when no provider is named
    And a second install reports unchanged and the read-only check is clean

  # canon: {"schema":1,"at":"2026-10-10T14:03:19Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:edge
  @skills.install.09
  Scenario: Install migrates a legacy Codex projection, removing only what canon's install lock proves it wrote
    Given a target holding the flattened Codex projection an earlier canon wrote under the Codex config directory, with an install lock recording each file's hash
    And a file of the user's own beside it, and a legacy file the user edited after install
    When the skill is installed
    Then the Codex skill lands under the agents skills root
    And every legacy file whose bytes still match its recorded hash is removed, along with each legacy directory left empty
    And the user's own file and the edited legacy file are kept
    And the legacy skills directory is removed only when it ends up empty, and the Codex config directory itself is kept
    And a rerun reports unchanged and leaves the user's file in place

  # canon: {"schema":1,"at":"2026-10-10T14:03:19Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:failure
  @skills.install.10
  Scenario: A leftover legacy Codex projection is reported as a remnant with its fix, never deleted
    Given a target where a legacy Codex projection remains that no install lock proves canon wrote
    When the read-only check and the doctor inspect the target
    Then the check lists the legacy path as a remnant, names the install command that migrates it, and exits non-zero
    And the doctor reports it as a legacy remnant naming the same fix
    And a following install keeps the file, because canon cannot prove it wrote it

  # canon: {"schema":1,"at":"2026-10-10T16:35:35Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:edge
  @skills.install.11
  Scenario: The stub claims build work in a canon repo and the guide it points at opens with the working loop
    Given canon's own skill source and the guide built into the binary
    When the skill is installed for Claude, Codex, OMP and Pi, and canon skills show canon prints the guide
    Then each projected SKILL.md is a stub that keeps the source's frontmatter, points at canon skills show canon and names this canon's version, with no reference files beside it
    And its description claims build, implement, feature and fix work in a repo that has canon.yaml, within the 1024-character limit skill loaders enforce
    And the guide's first section is the working loop, naming brief, subject, scenarios, units, implement, evidence, independent review and transition in that order
    And the AGENTS.md block canon init writes names the same steps in the same order

  # canon: {"schema":1,"at":"2026-10-11T14:44:50Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:skills-install
  @case:happy
  @skills.install.20
  Scenario: One human answer approves each brief it names and the policy, and an edited brief needs a new answer
    Given a change whose brief has no blocker
    When the human's answer is recorded with one call, canon brief approve <slug> --approver <the human> --note "<their words>"
    Then the "Approved: <who> <when> — "<words>" (brief sha256:<digest>)" line is written into .canon/changes/<slug>/brief.md and the policy approval is recorded in that same call
    And a brief edited after its approval counts as not approved: status and evidence add name the change and the canon brief approve command that records a new answer, and evidence add exits 1 until it is recorded
    And approving again replaces the one Approved line instead of adding a second

  # canon: {"schema":1,"at":"2026-10-11T14:44:50Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:skills-install
  @case:failure
  @skills.install.21
  Scenario: Only three acts stop for a human and every other leaf stays open to the roles that call it
    Given canon's command listing
    When the leaves that name no agent role are collected
    Then they are exactly brief approve, policy approve and divergence accept
    And every other leaf names at least one agent role, so no second human stop exists

  # canon: {"schema":1,"at":"2026-10-11T14:43:36Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:happy
  @skills.install.40
  Scenario: The install lock records the canon version and the source commit
    Given a canonical skill source inside a git checkout
    When canon skills install writes the projections
    Then the install lock records this canon's version and the checkout's HEAD commit
    And a rerun with nothing changed reports unchanged and leaves the lock byte-identical, even after a new commit that leaves the bundle as it was
    And a source outside any git checkout records no source commit

  # canon: {"schema":1,"at":"2026-10-11T14:43:36Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:failure
  @skills.install.41
  Scenario: A lock another canon version wrote is reported with the install fix
    Given projections whose install lock records another canon version
    When the read-only check and the doctor inspect the target
    Then both say "installed by canon <that version>, this canon is <this version>" and name `canon skills install` for the selected providers
    And the check exits 1 while the doctor exits 0
    And after the install both are clean and the lock records this canon's version

  # canon: {"schema":1,"at":"2026-10-11T14:43:37Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:failure
  @skills.install.42
  Scenario: A lock from before 0.16 is reported as an unknown canon version with the same fix
    Given projections whose install lock was written before canon 0.16 and records no canon version
    When the read-only check and the doctor inspect the target
    Then both report the projections as installed by an unknown canon version and name the same install command
    And the check exits 1 until the install rewrites the lock

  # canon: {"schema":1,"at":"2026-10-11T14:43:37Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:happy
  @skills.install.43
  Scenario: Doctor warns about co-installed skills that claim build work and never edits them
    Given a target with canon's projections and, in the provider skill roots, a skill whose description claims site, app or game work and a skill that claims none
    When canon skills doctor runs
    Then it names the claiming skill, its directory and the build words its description holds, and warns that if an agent picks it for build work canon's loop will not run
    And it says nothing about canon's own skill or the skill that claims no build work
    And it exits 0 and still reports the projections as healthy
    And every other skill's files are byte-identical afterwards

  # canon: {"schema":1,"at":"2026-10-12T03:41:11Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:failure
  @skills.install.22
  Scenario: The write commands do their own bookkeeping, so no extra format, import, index or promote step is needed
    Given a repo with a subject
    When a scenario is written with canon scenario new, then canon format and canon gate check run, and evidence is added to the scenario
    Then canon format finds the written corpus clean and canon gate check indexes the new scenario itself, naming it as having no evidence, with no canon inventory sync or canon ingest plans in between
    And canon evidence add promotes its record in the same run, so the gate no longer names the scenario
    And with --no-promote the record stays staged, the gate still names the scenario, and the next step is canon gate promote

  # canon: {"schema":1,"at":"2026-10-12T07:31:04Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:failure
  @skills.install.44
  Scenario: An unapproved brief blocks attestation until the human's answer is recorded, and the approval is never assumed
    Given a verifying subject whose only adopted change has a brief with no Approved line, and whose scenarios are tagged for it or none of its changes is approved
    When canon evidence add runs for any of the subject's scenarios, with no override reason
    Then each call exits 1 with BRIEF_UNAPPROVED naming the change's brief.md, so nothing is attested for that scope
    And once the human's answer is recorded with canon brief approve, alone or with other ids in one call, the same call succeeds
    And an approved brief edited afterwards refuses again with BRIEF_CHANGED_SINCE_APPROVAL naming the brief

  # canon: {"schema":1,"at":"2026-10-12T10:31:30Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:edge
  @skills.install.50
  Scenario: Codex is detected from its config directory, never from the agents root alone
    Given a canonical skill source and a target with no canon.yaml skills list and no install lock
    When the skill is installed with no provider named, into a target holding the Codex config directory, or that and the agents root, or the agents root alone
    Then the Codex config directory selects Codex and the skill lands under the agents skills root
    And the agents root alone selects nothing, because other tools use it too, and nothing is projected

  # canon: {"schema":1,"at":"2026-10-12T10:31:30Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:failure
  @skills.install.51
  Scenario: A target with no harness directory gets nothing and is told how to choose
    Given a canonical skill source and a target with no .claude, .codex, .omp or .pi directory, no canon.yaml skills list and no install lock
    When canon skills install runs with no provider named
    Then it selects no provider, writes no projection and no install lock, and adds no undetected default
    And it says nothing was installed and names `canon skills install --providers=<list>`
    And the read-only check exits 0 naming the same flag, and the doctor reports no-providers instead of healthy

  # canon: {"schema":1,"at":"2026-10-12T10:31:30Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:happy
  @skills.install.52
  Scenario: An explicit provider list is written to canon.yaml and later runs follow it
    Given a target with a canon.yaml that has comments and no skills section, and a .claude directory
    When canon skills install runs with --providers codex,omp
    Then it projects Codex and OMP only and appends `skills: { providers: [codex, omp] }` to canon.yaml, keeping every other byte
    And install, check and doctor with no provider named follow that list and say it came from canon.yaml
    And a later --providers replaces the section in place, a flow-style section in the middle of the file included, and --providers none records an empty list
    And a target without canon.yaml gets no canon.yaml

  # canon: {"schema":1,"at":"2026-10-12T10:31:30Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:edge
  @skills.install.53
  Scenario: A repo set up before 0.17 keeps working from its install lock
    Given a target whose canon.yaml has no skills section and whose install lock records Claude and Codex, beside an .omp directory
    When canon skills check, doctor and install run with no provider named
    Then each serves Claude and Codex and says the list came from the install lock because canon.yaml has no skills key
    And the check exits 0, the doctor reports healthy and the install reports unchanged
    And the lock and canon.yaml are byte-identical afterwards, and nothing is projected for OMP

  # canon: {"schema":1,"at":"2026-10-12T10:31:30Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:failure
  @skills.install.54
  Scenario: Doctor names a canon projection left for a provider that is not selected
    Given a target whose canon.yaml selects Claude and that still holds canon's Codex projection from an earlier install
    When canon skills check and canon skills doctor run
    Then the check exits 0 and does not mention the Codex projection
    And the doctor still reports healthy and adds a stray line naming the directory, saying codex is not a selected provider and canon no longer updates it, with `rm -r <dir>` to remove it and the install command that selects it
    And the projection is left on disk

  # canon: {"schema":1,"at":"2026-10-12T10:31:30Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:failure
  @skills.install.55
  Scenario: A malformed skills section in canon.yaml fails loud
    Given a canon.yaml whose skills section is a bare list, has a misspelled key, or names a provider canon does not know
    When canon skills install, check and doctor run with no provider named
    Then each is refused with exit 2 (SKILLS_INVALID_ARGUMENT), naming the expected shape or the unknown provider
    And nothing is projected and canon.yaml is byte-identical
    And a --skills-source directory that does not exist is refused the same way, exit 2 naming the option
    And canon init --check exits 1 reporting the skills section as FAIL with the same reason

  # canon: {"schema":1,"at":"2026-10-12T14:00:00Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:happy
  @skills.install.56
  Scenario: Install and doctor print the paths canon owns for formatter and linter ignore lists
    Given a target whose canon.yaml selects Claude and Codex
    When canon skills install and canon skills doctor run
    Then each ends with an owned-paths block of one glob per line
    And the block lists the selected providers' projections first, then every machine-written path under .canon sorted, including .canon/ledger/**, .canon/artifacts/** and the migration backups .canon/migrate-backup/**
    And an unchanged install prints the same block
    And .canon/specs, .canon/policy.yaml and .canon/changes are never listed, since people write them by hand and linters should still see them

  # canon: {"schema":1,"at":"2026-10-12T14:00:00Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:edge
  @skills.install.57
  Scenario: A projected file a formatter rewrote is named as a rewrite with the ignore entry to add
    Given an installed Claude projection whose SKILL.md a formatter rewrote with other line endings, spacing and blank lines
    When canon skills check and canon skills doctor run
    Then the check still exits 1 and calls the file formatter-rewritten
    And the doctor reports formatter-rewritten for the file, names `.claude/skills/canon/**` as the ignore entry and the install command that restores it
    And the doctor does not report the file as stale and does not report healthy

  # canon: {"schema":1,"at":"2026-10-12T14:00:00Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:failure
  @skills.install.58
  Scenario: A projected file whose content changed stays drift
    Given installed Claude and Codex projections where the Claude SKILL.md gained a paragraph, the hook script's `rm -rf a b` became `rm -rfab` and its `\` continuation was joined, and an inline command in the Codex stub gained a space
    When canon skills check and canon skills doctor run
    Then the check exits 1 and calls each file stale
    And the doctor reports each stale and never as formatter-rewritten, because whitespace between tokens and backslashes are never normalized away

  # canon: {"schema":1,"at":"2026-10-21T11:50:35Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:skills-install
  @case:happy
  @skills.install.59
  Scenario: Install prunes a projected file the bundle no longer ships when its bytes still match the lock
    Given a target with canon's projection installed for Claude, then a canon whose bundle no longer ships one reference file in a directory of its own
    When the skill is installed again
    Then the dropped file whose bytes still match the install lock is removed, and the install reports its path
    And the directory that held only it is removed, while the canon projection directory itself and every shipped file stay
    And the JSON lists the path under `pruned`, and the install lock lists only shipped files
    And a rerun reports unchanged and prunes nothing

  # canon: {"schema":1,"at":"2026-10-21T11:50:43Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:skills-install
  @case:failure
  @skills.install.60
  Scenario: Install keeps a dropped projected file whose bytes changed and says what to do
    Given a target with canon's projection installed for Claude, then a canon whose bundle no longer ships one reference file
    And the projected file's bytes were edited after the install
    When the skill is installed again
    Then the edited file is kept and the install reports its path, that its bytes differ from the install lock, and that it can be removed by hand
    And the JSON lists it under `kept` with a hint, and nothing under `pruned`
    And the install lock no longer lists it, and a rerun reports unchanged

  # canon: {"schema":1,"at":"2026-10-21T11:50:51Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:skills-install
  @case:edge
  @skills.install.61
  Scenario: Install touches only the selected providers' projections and only files the lock listed
    Given a target with canon's projection installed for Claude and Codex, a file the user added inside the Claude projection that the lock never listed, and a canon whose bundle no longer ships one reference file
    When the skill is installed for Claude only
    Then the dropped file is removed from the Claude projection, and the check for Claude is clean
    And the Codex projection is untouched, including its copy of the dropped file
    And the user's file inside the Claude projection is untouched
    And the install lock keeps the entries it recorded for the providers not selected, so a later install for one of them still prunes its dropped files and the check for it still names them

  # canon: {"schema":1,"at":"2026-10-21T11:51:00Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:skills-install
  @case:failure
  @skills.install.62
  Scenario: Check and doctor name each orphan with the fix
    Given a target with canon's projection installed for Claude, then a canon whose bundle no longer ships two reference files
    And one dropped file whose bytes still match the lock, and another whose bytes were edited
    When canon skills check and canon skills doctor run
    Then the check exits 1 and names each orphan path with its provider, whether its bytes match the lock, and the fix, in the output and in the JSON `orphans` list, instead of only a generic manifest mismatch
    And for the unmodified orphan the fix is `canon skills install`, which removes it; for the modified one it says install keeps it and to remove it by hand
    And the doctor reports an `orphan:` line for each, names the same fix, and does not report healthy
    And both commands are read-only: every file is where it was
    And after the install, which removes the unmodified orphan and keeps the edited one, the check is clean and the doctor reports healthy

  # canon: {"schema":1,"at":"2026-10-21T12:37:45Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:skills-install
  @case:failure
  @skills.install.63
  Scenario: Install ignores a lock key that is not a plain relative path inside a canon projection
    Given a target with canon's projection installed for Claude, and an install lock edited to list keys with empty, `.` or `..` segments or a trailing `/` inside the Claude projection directory
    When canon skills check, canon skills doctor and the install run
    Then those keys are never treated as orphans, and nothing outside the projection is touched or removed
    And the doctor reports an `invalid-lock-entry:` line for each
    And the install exits 0 and rewrites the lock without them

  # canon: {"schema":1,"at":"2026-10-21T16:36:17Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:skills-install
  @case:failure
  @skills.install.64
  Scenario: skills doctor reports a missing managed gitignore line with the fix
    Given a canon repo whose gitignore lacks a line canon init manages
    When canon skills doctor runs
    Then it prints a gitignore line naming the missing entries and the fix, canon migrate, instead of status: healthy

  # canon: {"schema":1,"at":"2026-10-23T11:49:14Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:skills-install
  @case:happy
  @skills.install.65
  Scenario: skills show prints the guide built into the binary, with no repo and for any role
    Given a directory that holds no canon repo
    When canon skills show canon runs, and canon skills show role-implementer, and with --json
    Then stdout is the embedded guide byte for byte and stderr is empty, whatever role is declared, even one that is no role
    And --json envelopes name, this canon's version and the markdown
    And --full appends every reference after the core guide, and changes nothing on another name

  # canon: {"schema":1,"at":"2026-10-23T11:49:18Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:skills-install
  @case:failure
  @skills.install.66
  Scenario: skills show refuses a name that is not a built-in guide
    Given the names canon schema skills show lists: canon and each reference
    When canon skills show names anything else
    Then it exits 2 with ok false

  # canon: {"schema":1,"at":"2026-10-23T11:49:27Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:skills-install
  @case:happy
  @skills.install.67
  Scenario: Install writes a stub that points at the guide and no reference files
    Given an empty repository and no canon/skills tree
    When canon init --providers claude,codex runs, and then canon skills install
    Then each projected SKILL.md is a short stub with the skill's frontmatter, this canon's version, and pointers to canon skills show canon and canon schema
    And no reference directory is projected and the install lock lists no reference file, while the retrieve script is projected from the binary
    And the AGENTS.md block points at canon skills show canon, canon skills check exits 0 and a second install changes nothing

  # canon: {"schema":1,"at":"2026-10-23T11:49:36Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:skills-install
  @case:failure
  @skills.install.68
  Scenario: A human-only act refuses every agent role
    Given a repo set up by canon init
    When brief approve, policy approve and divergence accept run as planner, implementer, reviewer or orchestrator, by --as and by CANON_ROLE
    Then each exits 2 with HUMAN_ONLY, not retryable, and a hint naming the leaf and the options only the human supplies
    And the refusal changes nothing, and without a declared role the leaf runs

  # canon: {"schema":1,"at":"2026-10-27T10:24:06Z","actor":{"agent_id":"impl-031-loop","role":"implementer","session_id":"loop-2026-10-27"}}
  @subject:skills-install
  @case:happy
  @flow:canon-0-31-loop.F1
  @skills.install.69
  Scenario: One brief approve call approves several changes with one name, one answer and one time
    Given three changes whose briefs have no blocker, no Approved line, and a repo with a policy file
    When the human runs canon brief approve with the three change ids, one --approver and one --note
    Then each brief ends with one Approved line carrying that approver, that answer and the same timestamp, and the policy approval records them once
    And each brief and the policy approval read as if the three had been approved one at a time with the same answer
    And running the same call again changes nothing and reports every id as unchanged

  # canon: {"schema":1,"at":"2026-10-27T10:24:07Z","actor":{"agent_id":"impl-031-loop","role":"implementer","session_id":"loop-2026-10-27"}}
  @subject:skills-install
  @case:failure
  @flow:canon-0-31-loop.F1
  @skills.install.70
  Scenario: A brief approve call with a blocked, a missing and an unclosed brief refuses all of them and writes nothing
    Given three changes: one whose brief has a blocker finding, one id that has no brief, and one whose brief ends inside an open code fence
    When the human runs canon brief approve with all three ids and one answer
    Then the command exits 2 with one BRIEF_APPROVE_REFUSED naming all three ids, each with its own code and reason
    And no brief and no policy approval was written or changed, and the same call with the three fixed approves all of them

  # canon: {"schema":1,"at":"2026-10-27T10:24:09Z","actor":{"agent_id":"impl-031-loop","role":"implementer","session_id":"loop-2026-10-27"}}
  @subject:skills-install
  @case:failure
  @flow:canon-0-31-loop.F1
  @skills.install.71
  Scenario: A brief approve call with several ids is human-only and the hint carries every id
    Given three changes whose briefs have no blocker and the planner role declared
    When canon brief approve runs with the three ids, by --as planner and by CANON_ROLE, with any flag
    Then each exits 2 with HUMAN_ONLY and nothing is written
    And the hint gives the human one canon brief approve line holding all three ids

  # canon: {"schema":1,"at":"2026-10-27T10:24:10Z","actor":{"agent_id":"impl-031-loop","role":"implementer","session_id":"loop-2026-10-27"}}
  @subject:skills-install
  @case:edge
  @flow:canon-0-31-loop.F1
  @skills.install.72
  Scenario: A repeated id counts once, one id prints as before and several print one entry each
    Given two changes whose briefs have no blocker
    When canon brief approve runs with one of them written twice, then with one id, then with both ids and --json
    Then the repeated id is approved once, a single id prints the document it always printed, and several ids print one entry per id with written, unchanged, failed or skipped
    And a --input array still stops at its first failure and keeps the approvals before it, one name and answer per element

  # canon: {"schema":1,"at":"2026-10-27T10:24:11Z","actor":{"agent_id":"impl-031-loop","role":"implementer","session_id":"loop-2026-10-27"}}
  @subject:skills-install
  @case:edge
  @flow:canon-0-31-loop.F3
  @skills.install.73
  Scenario: The skills, README, status help and schema state the governing-change rule and the status briefs list
    Given the packaged skills, the README, the status help and the schema documentation of this release
    When they are read for the rule on which changes a subject answers for and for the status JSON
    Then they say that reopen, ship and status judge every tagged, approved-line and chore change, that a never-approved untagged feature change blocks nothing, and that brief approve takes several ids
    And the README Upgrading section says the status brief field became the briefs list and the status schema version rose, and status --json carries that new version

  # canon: {"schema":1,"at":"2026-10-27T12:04:58Z","actor":{"agent_id":"impl-031-bf","role":"implementer","session_id":"bf-2026-10-27"}}
  @subject:skills-install
  @case:happy
  @flow:canon-0-31-brief-format.F3
  @skills.install.74
  Scenario: The brief skill documents carry the section table, the guidance, the frontmatter waiver and the old-format path
    Given the brief-shape and brief-polish skill documents
    When they are read
    Then brief-shape carries the section table with each section's heading, role and the same guidance text brief show prints, the frontmatter waiver form and the old-format path
    And brief-polish shows waivers as frontmatter entries and no skill document names a canon:section, canon:hint or canon:waive comment as the way to write a brief
