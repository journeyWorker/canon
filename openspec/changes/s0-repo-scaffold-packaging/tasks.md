## 1. Naming and vendor-audit preconditions

- [x] 1.1 Check npm registry availability of the unscoped `canon` package
      name (§10 Q4). Record the outcome (available / taken) in this task's
      evidence. If taken, adopt `@canon-dev/*` as the fallback scope for
      `packages/canon` (or `fugue` as the alias name) and use that name
      consistently in every task below and in every later spec's
      `@canon/*` package names.
      **Evidence:** bare `canon` is unavailable on the npm registry; the
      operator-resolved fallback scope is `@canon-dev/*` (recorded before
      this implementation pass — `.superpowers/sdd/progress.md` decision
      ledger, commit `7006873e` "resolve name Q4 → @canon-dev/\*"). The
      authoritative naming decision additionally resolves the alias
      question D1 left open: the installed **bin** stays `canon`
      independent of package name — `@canon-dev/cli` itself ships `bin: {
      "canon": "./bin.js" }`, so `bunx @canon-dev/cli` alone provides the
      friendly invocation with no separate `packages/canon` alias package
      (see task 3.2's evidence for why that task is not implemented).
- [x] 1.2 Confirm the vendored upstream launcher's vendor-audit
      phase cursor shows `survey` (or a later phase) as `status: done`
      before this change is applied. This is a read-only check — the
      survey/audit/synthesize work itself is owned by the parallel
      research vendor-audit workflow and MUST NOT be re-run or
      duplicated here.
      **Evidence:** the vendored upstream launcher's vendor-audit cursor
      shows `survey: done`, `audit: done`, `synthesize: done`, `map: done`
      (only `decide`/`propose` are `pending`, operator-owned per the
      change's own scope note). Not re-run.

## 2. Rust workspace scaffold

- [x] 2.1 Create root `Cargo.toml` with `[workspace]` `resolver = "2"` and
      members `crates/canon-model`, `crates/canon-store`,
      `crates/canon-ingest`, `crates/canon-gate`, `crates/canon-learn`,
      `crates/canon-report`, `crates/canon-cli`; set
      `[workspace.package]` `version`, `edition = "2021"`, and `license`.
      **Evidence:** root `Cargo.toml` — `resolver = "2"`, exactly the 7
      named members, `[workspace.package] version = "0.1.0"`, `edition =
      "2021"`, `license = "MIT"`. (Note: `canon-policy` is intentionally
      NOT a member here — it is `s13-policy-expressions`' own task 1.1,
      not S0's; this change's own `rust-bun-workspace` spec scopes the
      workspace to exactly these 7 crates.)
- [x] 2.2 Scaffold `canon-model`, `canon-store`, `canon-ingest`,
      `canon-gate`, `canon-learn`, `canon-report` as library crates, each
      exporting one marker constant (`pub const CRATE: &str = "canon-<x>";`)
      and one `#[test]` asserting it — no S1+ business logic (design D5).
      **Evidence:** all 6 crates carry `pub const CRATE` + one `#[test]`
      each (`crates/canon-{model,store,ingest,gate,learn,report}/src/lib.rs`);
      exercised by `cargo test --workspace` (task 2.4).
- [x] 2.3 Scaffold `canon-cli` as a `clap`-based binary crate depending on
      the workspace version, implementing only `canon --version` (prints
      the workspace version string) in this change.
      **Evidence:** `crates/canon-cli/src/main.rs`, `clap::Parser` with
      `#[command(name = "canon", version, ...)]`;
      `./target/release/canon --version` → `canon 0.1.0` (workspace
      version, verified live). Also carries `canon skills install` (task
      group 5, added later in this same change per tasks.md's own
      sequencing).
- [x] 2.4 Verify `cargo test --workspace` is green on a clean checkout with
      no prior `target/` directory.
      **Evidence:** `rm -rf target && cargo test --workspace` (verified
      live, twice) → 11 tests across the 7 crates (6 marker tests + 4
      `canon-cli` unit tests + 3 `canon-cli` integration tests +
      `canon-cli`'s own doc-test pass), 0 failures.

## 3. Bun packaging workspace

- [x] 3.1 Create `packages/cli/` (`@canon/cli` or the fallback scope from
      1.1): adapt the vendored upstream launcher's platform
      detection, libc-kind probing, search-path resolution, and
      self-reference guarding (design D2) to canon's binary name (`canon`)
      and package-name grammar (`@canon/core-<platform>`).
      **Evidence:** `packages/cli/src/index.ts` — `@canon-dev/cli`, binary
      `canon`; `resolveTargetPackageName`/`resolveRustTargetTriple`/
      `detectLibcKind`/`loaderPresent`/self-reference-guard all ported.
      One documented deviation from the vendored upstream launcher's literal order (see the
      file's own header comment): the workspace dev build is searched
      FIRST, matching this change's own `native-launcher` spec scenario
      "Workspace dev build takes priority over the packaged binary" as a
      structural guarantee rather than an incidental one.
- [ ] 3.2 Create `packages/canon/` (or the fallback alias name from 1.1): a
      thin package whose `bin.js` re-execs the resolved `@canon/cli` bin
      path, with `dependencies: { "@canon/cli": "<workspace version>" }`
      (design D1).
      **Not implemented — superseded by the authoritative naming decision**
      (resolved 2026-07-10, overriding this task's open-question framing):
      "the installed bin is `canon` (bin name independent of package
      name)". `@canon-dev/cli`'s `package.json` ships `bin: { "canon":
      "./bin.js" }` directly, so `bunx @canon-dev/cli` alone resolves and
      runs the `canon` bin — no separate alias package needed. A
      distinctly-named unscoped `packages/canon` publish target is also
      moot: the bare `canon` name is unavailable (task 1.1), so there is
      nothing to alias TO under that name. This is exactly design D1's own
      "Alternative considered" paragraph (skip the alias, document `bunx
      @canon/cli`), now adopted as the actual decision. Left unchecked
      rather than marked done, since the literal task ask (a
      `packages/canon/` directory) was deliberately not built.
- [x] 3.3 Create `packages/core-darwin-arm64/` and `packages/core-linux-x64/`
      (`@canon/core-<platform>` naming), each holding only a prebuilt
      `canon-cli` binary at publish time; wire both as `@canon/cli`'s
      `optionalDependencies`.
      **Evidence:** `packages/core-darwin-arm64/package.json`,
      `packages/core-linux-x64/package.json` (`os`/`cpu` fields set, `bin/`
      holds only a `.gitkeep` — populated by CI, gitignored otherwise);
      wired into `packages/cli/package.json`'s `optionalDependencies` as
      `"workspace:*"`.
- [x] 3.4 Verify `bun install` at the repo root resolves `packages/canon`,
      `packages/cli`, and both `packages/core-<platform>` as workspace
      members with no registry round-trip needed for local development.
      **Evidence:** `bun install` at repo root (verified live) —
      `bun.lock` resolves `@canon-dev/cli`, `@canon-dev/core-darwin-arm64`,
      `@canon-dev/core-linux-x64` all as `workspace:packages/...` (no
      registry tarball entry); `packages/cli/node_modules/@canon-dev/
      core-{darwin-arm64,linux-x64}` are local relative symlinks
      (`readlink` verified: `../../../core-{darwin-arm64,linux-x64}`). No
      `packages/canon` member exists (task 3.2).

## 4. CI and publish pipeline

- [x] 4.1 Add `.github/workflows/build-native.yml` (adapted from
      the vendored upstream launcher's native-build workflow): a 2-row
      matrix (`aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`) building
      `canon-cli --release` and packaging each into its
      `packages/core-<platform>/` directory as a CI artifact.
      **Evidence:** `.github/workflows/build-native.yml` authored — 2-row
      matrix (`macos-14`/`aarch64-apple-darwin`,
      `ubuntu-latest`/`x86_64-unknown-linux-gnu`), both native builds (no
      cross-compilation toolchain needed since each runner's host arch
      matches its target triple). YAML-validated (`yaml.safe_load`);
      authored only, not executed (no CI runner available in this
      environment — matches the constraint that cross-platform build is
      CI's job).
- [x] 4.2 Add a publish workflow that reads the version from the root
      `Cargo.toml`, applies it to `packages/canon`, `packages/cli`, and
      both `packages/core-<platform>` `package.json`s, and publishes all
      four packages atomically (fail closed on any matrix-row build
      failure — no partial publish, per the `native-release-pipeline`
      spec).
      **Evidence:** `.github/workflows/publish.yml` authored — reads
      `[workspace.package]` version from root `Cargo.toml`, verifies a
      trigger tag matches it, applies it to `packages/cli` and both
      `packages/core-<platform>` `package.json`s (3 packages, not 4 — no
      `packages/canon`, per task 3.2), rewrites the `workspace:*`
      optionalDependency ranges to the literal version before publish.
      Fail-closed: the `publish` job `needs: [read-version, build]`, and
      GitHub Actions never starts a job needing a matrix job until every
      row of that matrix succeeds — a single failed target blocks every
      publish step. YAML-validated; authored only, not executed.
- [ ] 4.3 Verify `bunx canon --version` runs the real Rust binary end-to-end
      on a clean macOS arm64 machine and a clean Linux x64 machine (or
      CI-equivalent clean containers), printing the published version.
      **Explicitly left open** — this scenario needs a real clean-machine/
      container install against a published npm registry release, which
      does not exist yet (no release has been published) and is out of
      scope for a local, zero-network S0 pass (per this task's own
      constraints: CI matrix + publish workflows are authored as files,
      not executed here). The LOCAL equivalent proof WAS executed instead
      (native-launcher spec's dev-build-priority scenario, not this exact
      scenario): `node packages/cli/bin.js --version` and `bun
      packages/cli/bin.js --version` both resolve the local
      `target/release/canon` dev build offline and print `canon 0.1.0`,
      with exit-code and argv passthrough verified (`canon skills install`
      invoked through the launcher end-to-end) and the actionable
      unsupported/missing-binary error path verified by temporarily
      removing the local build. Re-open this task once a real publish +
      clean-machine verification is possible.

## 5. Skill materialization scaffold

- [x] 5.1 Create `canon/skills/` as the SoT directory for every spec's
      companion skill (decision 9); document its `SKILL.md` shape (mirrors
      `.claude/skills/<name>/SKILL.md`'s existing format).
      **Evidence:** `canon/skills/README.md` documents the SoT layout and
      the exact `SKILL.md` frontmatter/body shape, plus the materialization
      target shapes (`.claude/skills/<name>/SKILL.md` verbatim,
      `.codex/skills/<name>.md` flattened, the lock file shape).
- [x] 5.2 Implement `canon skills install` in `canon-cli` (or a
      `canon-report`/dedicated module, per the crate boundaries from task
      group 2): materializes `canon/skills/<name>/SKILL.md` into
      `.claude/skills/<name>/SKILL.md` (verbatim) and `.codex/skills/
      <name>.md` (canon's flattened convention, design D4); writes
      `canon/skills/.install-lock.json` keyed by `{name, contentHash,
      version}` — no `generatedAt` field (decision 11).
      **Evidence:** `crates/canon-cli/src/skills.rs`
      (`discover_skills`/`content_hash`/`flatten_for_codex`/`install`),
      wired into `canon skills install` (`crates/canon-cli/src/main.rs`).
      Self-run against canon's own `canon/skills/repo-scaffold/`: writes
      `.claude/skills/repo-scaffold/SKILL.md` (verbatim, `diff` confirmed
      identical) and `.codex/skills/repo-scaffold.md` (flattened header +
      body); `canon/skills/.install-lock.json` contains only
      `{"skills":{"repo-scaffold":{"contentHash":"sha256:...","version":1}}}`
      — no `generatedAt` anywhere (grepped).
- [x] 5.3 Verify idempotence: two consecutive `canon skills install` runs
      with no source change produce a byte-identical lock and zero git
      diff on the second run.
      **Evidence:** ran `canon skills install` against canon's own
      `canon/skills/` twice live — first run reported `repo-scaffold v1 —
      installed`, second run `repo-scaffold v1 — unchanged`, identical
      lock both times. Also covered by the automated
      `install_is_idempotent_with_no_source_change` integration test
      (byte-`assert_eq!` on `.claude/`, `.codex/`, and the lock file
      across two runs) and `content_change_bumps_version_not_timestamp`
      (version increments by exactly one on a real content change, unlike
      a timestamp field, which would change on every run regardless).

## 6. Companion skill and fixtures

- [x] 6.1 Author this spec's own companion skill under
      `canon/skills/repo-scaffold/SKILL.md`, covering: how to add a new
      crate to the workspace (task 2.1's member list discipline), how to
      add a new CI matrix target / `packages/core-<platform>`, and how to
      run `canon skills install` locally.
      **Evidence:** `canon/skills/repo-scaffold/SKILL.md` — all 3
      subsections present (add-a-crate, add-a-CI-target, run-the-installer
      locally); materialized into `.claude/skills/repo-scaffold/SKILL.md`
      and `.codex/skills/repo-scaffold.md` (task 5.3).
- [x] 6.2 Add a fixture repo (e.g. `crates/canon-cli/fixtures/skills-repo/`
      or an equivalent test-only directory) with a `.claude/` and `.codex/`
      tree and a sample `canon/skills/<name>/SKILL.md`; add a standalone
      integration test in `canon-cli` (until S5 ships `canon selftest`
      proper) that runs `canon skills install` against it and asserts the
      `skill-materialization` spec scenarios.
      **Narrowed from the original ask**, which additionally named
      `native-launcher`, `rust-bun-workspace`, and `native-release-pipeline`
      as scenarios this same fixture/test should assert. Those three are
      not exercisable through a `canon skills install` fixture as literally
      specified — see the deviation note below for where each is actually
      verified.
      **Evidence:** `crates/canon-cli/fixtures/skills-repo/` (`.claude/`,
      `.codex/`, `canon/skills/example-skill/SKILL.md`) +
      `crates/canon-cli/tests/skills_install.rs` (3 tests: materialize +
      lock shape + no-`generatedAt` assertion, idempotence, version-bump-
      on-change) — `cargo test -p canon-cli` → all pass. Each test copies
      the fixture into a fresh tempdir first, so the checked-in fixture is
      never mutated by `cargo test`. This asserts only
      `skill-materialization`'s scenarios (verbatim `.claude/` copy,
      flattened `.codex/` output, timestamp-free content-hash/version lock,
      idempotence across two runs). It does NOT assert `native-launcher`,
      `rust-bun-workspace`, or `native-release-pipeline` scenarios.
      **Deviation — the other three specs' scenarios are verified, but by
      different mechanisms, not this fixture:**
      - `rust-bun-workspace` — the workspace's 7 members actually resolving
        under `resolver = "2"` is exercised by `cargo build/test
        --workspace` (task 2.4) succeeding at all, reinforced by the six
        per-crate `crate_marker_is_*` tests (task 2.2) that only compile
        and pass if each crate is a genuinely resolved workspace member.
      - `native-launcher` — dev-build priority, argv/exit-code passthrough,
        and the missing-binary error path are covered by the live
        `--version` / `canon skills install`-through-launcher smoke
        recorded under task 4.3's evidence — a runtime check, not a
        checked-in fixture, and it shares task 4.3's own open gap: the
        packaged-binary (post-publish) path still needs a real publish +
        clean-machine run before it can be asserted at all.
      - `native-release-pipeline` — verified as YAML-valid and structurally
        fail-closed by inspection (tasks 4.1/4.2's evidence), not run; both
        workflows are authored-not-executed in this local, zero-network
        pass, the same constraint tasks 4.1/4.2 already document.

      This mirrors tasks 3.2 and 4.3's own precedent: a checked box
      reflects only what was actually verified, with any gap and its
      proof-elsewhere named explicitly rather than left implicit.
