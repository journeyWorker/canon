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
    Then the install fails with the provider named
    And no projection or lock file is written to the target

  # canon: {"schema":1,"at":"2026-10-10T14:03:19Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:happy
  @skills.install.08
  Scenario: Codex receives the skill where Codex discovers skills, frontmatter intact
    Given a canonical skill source whose frontmatter names the skill and describes it, with reference and script sidecars
    When the skill is installed for Codex
    Then Codex's skill file is the source byte for byte, in the canon skill directory under the agents skills root Codex scans
    And its frontmatter opens the file and carries the skill's name and a non-empty description
    And the reference and script sidecars sit beside it
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
  Scenario: The projected skill claims build work in a canon repo and opens with the working loop
    Given canon's own skill source and its committed Claude, Codex, OMP and Pi projections
    When each projected SKILL.md is read
    Then it is the source byte for byte
    And its description claims build, implement, feature and fix work in a repo that has canon.yaml, ahead of generic site, app or game builder skills
    And its description stays within the 1024-character limit skill loaders enforce
    And its first section is the working loop, naming brief, subject, scenarios, units, implement, evidence, independent review and transition in that order
    And the command reference comes after the loop
    And the AGENTS.md block canon init writes names the same steps in the same order

  # canon: {"schema":1,"at":"2026-10-11T14:44:50Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:skills-install
  @case:happy
  @skills.install.20
  Scenario: The skill and the AGENTS.md block carry one human approval per scope
    Given canon's own skill source and the AGENTS.md block canon init writes
    When each is read with its wrapped lines joined
    Then each says a new subject or a scope change needs one human answer approving the brief and the policy
    And each tells the agent to record that answer at once with canon brief approve --change <slug> --by <the human> --note "<their words>", which writes the "Approved: <who> <when> — "<words>" (brief sha256:<digest>)" line into .canon/changes/<slug>/brief.md and records the policy approval in the same call, and never to approve without a human answer
    And each says a brief edited after its approval needs a new human answer before it counts as approved again
    And each says a change from an external plan gets its brief there too, a brief.md whose "Source: <path>" line points at the plan, which canon never edits
    And each says that inside an approved scope the agent never stops for a human, that independent review happens on every change, and that when no human can answer it writes the assumption and proceeds
    And each says a fix or follow-up stays in the approved change, never a canon change new, because a new change is a new scope, and that tasks.md is an optional work note nothing gates on
    And each says the agent moves the subject to shipped itself once the guard passes, unless the brief says the human ships

  # canon: {"schema":1,"at":"2026-10-11T14:44:50Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:skills-install
  @case:failure
  @skills.install.21
  Scenario: The skill and the AGENTS.md block name no second human stop
    Given canon's own skill source and the AGENTS.md block canon init writes
    When each is read with its wrapped lines joined
    Then neither keeps a fixed pair of human stops, a brief approval for every request, or a rule that the agent never runs the policy approval for the human

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
  Scenario: The skill, the AGENTS.md block and the write help name no bookkeeping step the commands already do
    Given canon's own skill source, the AGENTS.md block canon init writes, and the help of canon evidence, canon finding and canon finding add
    When each is read with its wrapped lines joined
    Then the working loop never tells the agent to run canon format specs or canon ingest plans
    And it names canon inventory sync only for a command that printed "NOT refreshed"
    And every mention of canon gate promote is conditioned on write.promote manual
    And no help loop lists canon gate promote as a step of its own

  # canon: {"schema":1,"at":"2026-10-12T07:31:04Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:failure
  @skills.install.44
  Scenario: The skill and the AGENTS.md block stop at the brief and never assume the approval
    Given canon's own skill source and the AGENTS.md block canon init writes
    When each is read with its wrapped lines joined
    Then each tells the agent to write the brief, then stop and report it, and to build and attest nothing for that scope until the human answers
    And each says the approval is never assumed, even when the human said they cannot answer until the agent reports, because the brief is that report
    And each limits proceeding on an assumption to the brief's questions and work inside an approved scope, never the approval
    And each calls the ship refusal and the evidence add refusal backstops, with the stop at the brief

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
    Then each exits 1 naming the expected shape or the unknown provider
    And nothing is projected and canon.yaml is byte-identical
    And canon init --check-config reports the skills section as FAIL with the same reason

  # canon: {"schema":1,"at":"2026-10-12T14:00:00Z","actor":{"agent_id":"canon"}}
  @subject:skills-install
  @case:happy
  @skills.install.56
  Scenario: Install and doctor print the paths canon owns for formatter and linter ignore lists
    Given a target whose canon.yaml selects Claude and Codex
    When canon skills install and canon skills doctor run
    Then each ends with an owned-paths block of one glob per line
    And the block lists the selected providers' projections first, then every machine-written path under .canon sorted, including .canon/ledger/** and .canon/artifacts/**
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
    Given an installed Claude projection where SKILL.md gained a paragraph, the hook script's `rm -rf a b` became `rm -rfab` and its `\` continuation was joined, and a command inside a reference file's code fence gained a space
    When canon skills check and canon skills doctor run
    Then the check exits 1 and calls each file stale
    And the doctor reports each stale and never as formatter-rewritten, because whitespace between tokens, backslashes and code fences are never normalized away
