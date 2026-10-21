# Surface — a-harness

**Vendor paths**: `vendors/addyosmani-agent-skills/skills`, `vendors/addyosmani-agent-skills/_vendor_claude`, `vendors/addyosmani-agent-skills/_vendor_gemini`, `vendors/addyosmani-agent-skills/commands`, `vendors/addyosmani-agent-skills/_vendor_codex-plugin`, `vendors/addyosmani-agent-skills/hooks`, `vendors/addyosmani-agent-skills/agents`, `vendors/addyosmani-agent-skills/references`, `vendors/addyosmani-agent-skills/scripts`, `vendors/addyosmani-agent-skills/docs`, `vendors/addyosmani-agent-skills/plugin.json`  
**Scale**: 25 shared skills, 9 command wrappers in each slash-command adapter, 4 personas, 7 references, 4 hook scripts plus hook documentation/tests, and provider/setup/validation tooling  
**Primary language(s)**: Markdown, shell, JSON, JavaScript, TOML  
**Canon analog (if any)**: `crates/canon-cli/src/skills.rs` and `canon/skills/SKILL.src.md` — canon already projects one canonical skill bundle to selected harness roots, while this surface distributes a larger portable catalog plus native command and lifecycle adapters.

## 1. Overview

This surface is a multi-harness distribution and orchestration layer around a shared catalog of engineering skills. The portable `skills/` directory is the core workflow product; Claude, Gemini, and Antigravity receive native command wrappers, while Codex and Copilot consume the shared skills through plugin manifests and convention-based discovery. Hooks add opt-in lifecycle behavior for session routing, HTTP freshness caching, and protecting marked code during simplification. The surface is coupled to host discovery conventions, but its workflow text is intentionally shared and progressively disclosed rather than duplicated per harness.

## 2. Entry points & architecture

- **Entry file(s)**: `vendors/addyosmani-agent-skills/plugin.json:1-4` is the minimal root plugin metadata; `vendors/addyosmani-agent-skills/_vendor_codex-plugin/plugin.json:1-25` is the Codex manifest and points at `./skills/`; `vendors/addyosmani-agent-skills/_vendor_claude/commands/*.md:1-5` and `vendors/addyosmani-agent-skills/_vendor_gemini/commands/*.toml:1-11` are native lifecycle entry points.
- **Core abstractions**:
  - `skills/<name>/SKILL.md` — portable workflow with trigger description, process, anti-rationalization rules, red flags, and verification.
  - `commands/<name>` / `_vendor_claude/commands/<name>` / `_vendor_gemini/commands/<name>` — thin harness-specific adapters that invoke one or more skills and translate host syntax.
  - `agents/<persona>.md` — specialist perspective and report format; personas do not orchestrate one another.
  - `hooks/*.sh` — event-driven host integrations whose exit code or JSON envelope changes tool behavior at a lifecycle seam.
  - `references/*.md` — supplementary checklists loaded on demand by a skill or whole-repo install.
  - `scripts/validate-*.js` — repository-level parity and structural checks, including command parity and synchronized descriptions.
- **Cross-package dependencies**: the adapters reference the local `skills/` catalog, `agents/`, `references/`, and `hooks/`; the validation scripts share parsing logic in `scripts/lib/skill-lint.js`.
- **External runtime dependencies**: host-specific command loaders, Bash 3.2+, `jq`, `curl`, `shasum`/`sha256sum`, and the Claude/Gemini/Codex hook JSON conventions. The commands intentionally use different prompt syntaxes while keeping descriptions synchronized.

## 3. Pattern catalog

### 3.1 One portable workflow, native projections per harness

- **Where**: `vendors/addyosmani-agent-skills/README.md:349-370`; `vendors/addyosmani-agent-skills/docs/codex-setup.md:30-43`; `vendors/addyosmani-agent-skills/_vendor_codex-plugin/plugin.json:1-25`
- **What it does**: The shared `skills/` tree is the canonical workflow source, while Claude uses `.claude/commands/`, Gemini uses `.gemini/commands/`, Antigravity uses `commands/`, and Codex uses a plugin manifest that points directly at `./skills/`. The setup documentation explicitly avoids copying or preloading the meta-skill on Codex, because Codex already performs progressive skill discovery; hosts without native routing may use the standalone SessionStart helper.
- **Why it's interesting**: Canon currently has provider selection and projection, but its projection is specifically one `canon` skill plus reference/script sidecars. A catalog with native adapters needs a declared capability matrix, a clear distinction between shared source and host wrappers, and an explicit policy for when not to inject a second router.
- **Canon applicability**: **Canon has it, partially** in `crates/canon-cli/src/skills.rs:1-32,110-122,1386-1490`: `canon skills install|check` already selects providers and projects a canonical bundle; `canon/skills/SKILL.src.md:1-238` and `canon/skills/reference/role-orchestrator.md:1-100` provide the shared workflow and role entry points. It does not currently model per-provider command adapters or a native-routing/no-double-injection capability flag.
- **Effort to lift**: moderate.

### 3.2 Command wrappers compose skills and expose a bounded autonomous mode

- **Where**: `vendors/addyosmani-agent-skills/_vendor_claude/commands/build.md:1-44`; `vendors/addyosmani-agent-skills/_vendor_gemini/commands/build.toml:1-40`; `vendors/addyosmani-agent-skills/_vendor_claude/commands/ship.md:1-72`; `vendors/addyosmani-agent-skills/docs/getting-started.md:124-145`
- **What it does**: `/build` invokes incremental implementation plus TDD and executes one pending task; `/build auto` requires a known spec, checks for unrelated working-tree changes, creates or reads a plan, obtains one explicit approval, then runs each dependency-ordered task through RED/GREEN/regression/build/commit boundaries. `/ship` composes a parallel fan-out of code-reviewer, security-auditor, and test-engineer personas, then merges findings into a GO/NO-GO decision with a mandatory rollback plan. The same command names and descriptions are kept across Claude, Gemini, and Antigravity while body syntax is host-specific.
- **Why it's interesting**: Canon has a stronger evidence/review/ship gate, but no user-facing command layer that chains role steps into a bounded execution mode. The vendor's explicit distinction between removing human pauses and removing verification is a useful orchestration contract, as is the restartable per-task boundary.
- **Canon applicability**: **Canon has it, partially** in `canon/skills/SKILL.src.md:1-238`, `canon/skills/reference/role-orchestrator.md:1-100`, and `crates/canon-cli/src/status.rs:404-407,1241-1244`: role-aware status and orchestrator transitions exist, but there is no `/build auto` equivalent, no command adapter projection, and no single command that composes planner → implementer → independent reviewer while preserving canon's actor/session requirements. `canon gate check` and `subject status` remain the authoritative guard and should not be replaced by a command wrapper.
- **Effort to lift**: significant.

### 3.3 Lifecycle hooks enforce behavior at narrow event seams and fail safely where appropriate

- **Where**: `vendors/addyosmani-agent-skills/hooks/session-start.sh:1-35`; `vendors/addyosmani-agent-skills/hooks/SDD-CACHE.md:17-39,61-79`; `vendors/addyosmani-agent-skills/hooks/SIMPLIFY-IGNORE.md:19-40,49-69`; `vendors/addyosmani-agent-skills/hooks/sdd-cache-pre.sh:1-22,47-88`
- **What it does**: The SessionStart hook emits the standard JSON envelope and injects the meta-skill only when explicitly wired; it exits successfully with a diagnostic if `jq` is unavailable. The WebFetch PreToolUse/PostToolUse pair keys a cache by URL, revalidates with `ETag`/`Last-Modified`, blocks only on HTTP 304, and otherwise lets the fetch proceed. The simplify hook uses PreToolUse Read to replace marked blocks with content-hashed placeholders, restores/re-filters them after Edit/Write, and expands them at Stop so edits through other routes do not lose protected code.
- **Why it's interesting**: Canon already has evidence/gate hooks, but this gives concrete patterns for hook scope: event + matcher, deterministic input/output envelopes, fail-soft advisory hooks, and stateful protections that have explicit cleanup semantics. It also demonstrates that enforcement can occur at the tool boundary without pretending the hook itself is proof of a successful task.
- **Canon applicability**: **Canon has it, partially and with a conflict to avoid** in `crates/canon-cli/src/gate.rs:1071-1126` and `canon/skills/reference/canon-gate.md:1-183`: `canon gate install-hooks` idempotently merges hook entries into Claude/Codex settings and the gate remains authoritative. The vendor's exit-2 cache substitution and in-place file rewriting should not be copied into evidence semantics; canon should use hooks for advisory context, task/evidence routing, or preflight refusal only where the gate can explain the refusal and preserve artifacts.
- **Effort to lift**: significant.

### 3.4 Cross-harness command parity is validated as a catalog invariant

- **Where**: `vendors/addyosmani-agent-skills/scripts/validate-commands.js:1-38,73-110,145-212`; `vendors/addyosmani-agent-skills/README.md:355-366`
- **What it does**: The validator treats Claude command stems as canonical, maps the intentional `plan` → `planning` filename difference, requires every command in each adapter, checks Claude frontmatter validity, and requires identical descriptions while allowing prompt bodies to differ for host syntax. This turns distribution drift into a CI failure rather than a setup-time surprise.
- **Why it's interesting**: Canon's provider projection checks bytes and manifests for the projected skill bundle, but a future command/role adapter layer would have semantic parity requirements that byte equality cannot express. A provider capability check should distinguish required command identity/description from intentionally different bodies.
- **Canon applicability**: **Canon has it, partially** in `crates/canon-cli/src/skills.rs:1482-1515` (per-file `ok`/`missing`/`stale` checks and manifest comparison) and `crates/canon-cli/src/schema.rs:180-183` (provider command availability). It lacks an adapter parity validator and does not currently check that provider-specific command descriptions or role entry points remain equivalent.
- **Effort to lift**: moderate.

### 3.5 Persona orchestration separates roles from the command that invokes them

- **Where**: `vendors/addyosmani-agent-skills/_vendor_AGENTS.md:48-73`; `vendors/addyosmani-agent-skills/_vendor_claude/commands/ship.md:9-25,68-72`; `vendors/addyosmani-agent-skills/README.md:355-366`
- **What it does**: Skills define the process, agents define a perspective and output format, and commands define the user-facing trigger. The `/ship` command fans out independent personas in one turn and performs the merge itself; personas cannot spawn or invoke other personas. This gives a clean lifecycle for parallel review and makes the merge point responsible for severity aggregation, accessibility/infrastructure checks, and rollback.
- **Why it's interesting**: Canon already distinguishes planner, implementer, reviewer, and orchestrator, but a reviewer is currently a role/session contract rather than a reusable set of specialist perspectives. The vendor pattern can improve reviewer task decomposition without weakening canon's distinct-session requirement.
- **Canon applicability**: **Canon has it, partially** in `canon/skills/reference/role-reviewer.md:1-100`, `canon/skills/reference/role-orchestrator.md:1-100`, and `crates/canon-cli/src/role.rs:31-47`: role ownership and independent review are present, but specialist persona fan-out and a structured merge report are missing. Any lift must preserve canon's evidence pins and different-session rule rather than treating persona output as review evidence by itself.
- **Effort to lift**: moderate.

## 4. Risks & incompatibilities

- **Effect version**: not applicable; this surface is Markdown, shell, JSON, JavaScript, and TOML rather than Effect.
- **Framework**: no UI framework; command adapters depend on each host's native command and hook schema.
- **Heavy deps**: `jq`, `curl`, hashing utilities, and host CLIs are runtime requirements for optional hooks; the cache and simplification hooks use filesystem state under `.claude/`.
- **Coupling**: command wrappers only work when the referenced skill names and host discovery locations are present; `/ship` additionally couples to the named persona files and an Agent/subagent facility. The SessionStart hook is intentionally not wired for hosts with native routing.
- **TOS-violation patterns**: none observed in this surface. The HTTP cache uses ordinary conditional requests and does not forge provider fingerprints or evade rate limits.
- **License footnote**: `plugin.json` declares MIT. Canon's default is clean-room reimplementation from this audit; no verbatim lift is recommended or required.

## 5. Recommended actions

1. **EXTRACT** provider capability metadata and a native-routing policy → `crates/canon-cli/src/skills.rs` and the provider projection model — canon already selects providers and projects files, so recording whether a provider consumes native commands/skills would prevent duplicate meta-router injection.
2. **INSPIRE** a command-adapter parity check → `crates/canon-cli/src/skills.rs` or a new read-only `canon skills check` diagnostic — byte-level projection checks are insufficient when provider command names/descriptions intentionally differ in syntax but must remain semantically aligned.
3. **INSPIRE** bounded plan execution with explicit approval and restartable task boundaries → `canon/skills/reference/role-orchestrator.md` and a future orchestration command — `/build auto` removes pauses, not tests or task boundaries, which aligns with canon's approval, evidence, and independent-review contract if canon remains the final gate.
4. **EXTRACT** event/matcher/timeout hook registration and idempotent dual-provider wiring → `canon gate install-hooks` in `crates/canon-cli/src/gate.rs:1071-1126` — the vendor demonstrates concrete lifecycle seams, while canon already owns an additive Claude/Codex hook installer that should remain the single registration authority.
5. **INSPIRE** fail-soft, non-proof advisory hooks with explicit JSON envelopes → `canon/skills/reference/canon-retrieve.md` and `canon/skills/reference/canon-gate.md` — retrieval hooks should enrich dispatch context without blocking or becoming evidence; gate refusals must remain explainable and authoritative.
6. **SKIP** the simplify-ignore in-place placeholder rewrite as a canon evidence mechanism → `canon/skills/reference/canon-gate.md` — it mutates working-tree files during reads and has crash/recovery edge cases, while canon's gate must inspect durable artifacts and never hide evidence from the evaluator.
7. **SKIP** treating `/ship` persona output as a substitute for canon review records → `canon/skills/reference/role-reviewer.md` and `canon/skills/reference/canon-review.md` — canon requires a distinct session, pinned evidence/spec, and signed review; persona fan-out can inform that review but cannot satisfy it alone.
8. **SKIP** copying `_vendor_AGENTS.md` or `_vendor_CLAUDE.md` into target repositories → `crates/canon-cli/src/init.rs:203-238` — canon's owned, bounded AGENTS block is harness-neutral and conflict-safe; vendor repository instructions explicitly say they configure the vendor repo itself, not consumer projects.

## 6. Citations

- `vendors/addyosmani-agent-skills/README.md:349-370` — shared core, per-harness adapters, personas, hooks, references, scripts, and docs.
- `vendors/addyosmani-agent-skills/plugin.json:1-4` — root plugin metadata.
- `vendors/addyosmani-agent-skills/_vendor_codex-plugin/plugin.json:1-25` — Codex plugin manifest and capabilities.
- `vendors/addyosmani-agent-skills/docs/codex-setup.md:30-43` — Codex consumes shared skills directly and does not use Claude slash commands/personas/hooks.
- `vendors/addyosmani-agent-skills/_vendor_claude/commands/build.md:1-44` — `/build` and `/build auto` modes, approval, tests, commits, and stop conditions.
- `vendors/addyosmani-agent-skills/_vendor_claude/commands/ship.md:1-72` — parallel persona fan-out and GO/NO-GO merge contract.
- `vendors/addyosmani-agent-skills/hooks/session-start.sh:1-35` — SessionStart JSON envelope, optional routing, and jq fallback.
- `vendors/addyosmani-agent-skills/hooks/SDD-CACHE.md:17-39,61-79` — WebFetch hook registration, validator-based freshness, and exit-code behavior.
- `vendors/addyosmani-agent-skills/hooks/SIMPLIFY-IGNORE.md:19-40,49-69` — Read/Edit/Write/Stop lifecycle and content-hashed placeholder protection.
- `vendors/addyosmani-agent-skills/scripts/validate-commands.js:1-38,73-110,145-212` — command parity, description synchronization, and intentional filename/body differences.
- `crates/canon-cli/src/skills.rs:24-32,110-122,749-770` — canon provider selection and provider roots/markers.
- `crates/canon-cli/src/skills.rs:1386-1415,1482-1515` — canonical bundle projection, locks, and read-only drift states.
- `crates/canon-cli/src/init.rs:203-238` — canon-owned, harness-neutral AGENTS block and role guidance.
- `crates/canon-cli/src/gate.rs:1071-1126` — idempotent hook installation into Claude and Codex settings.
- `canon/skills/reference/canon-gate.md:1-183` — evidence gate authority and failure classes.
- `canon/skills/reference/canon-retrieve.md:1-80` — fail-soft advisory pre-dispatch retrieval and replay semantics.
- `canon/skills/reference/role-orchestrator.md:1-100` — canon orchestrator role, independent sessions, and transition guard.
