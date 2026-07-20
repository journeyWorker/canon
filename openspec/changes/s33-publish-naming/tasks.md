# s33 publish-naming — tasks

## 1. Rename (load-bearing)

- [x] 1.1 `packages/cli/package.json`: name → `canoncli`,
      optionalDependencies → `canoncli-core-<platform>`, repository →
      journeyWorker/canon.
- [x] 1.2 `packages/core-{darwin-arm64,linux-x64}/package.json`:
      name → `canoncli-core-<platform>`, repository → journeyWorker.
- [x] 1.3 `packages/cli/src/index.ts`: resolve unscoped
      `canoncli-core-<platform>` (flat node_modules), monorepo-dev
      `packages/<dir>` path + error-message names updated.
- [x] 1.4 `.github/workflows/publish.yml` matrix `package_name` +
      step labels + header naming comment.
- [x] 1.5 `Cargo.toml` repository URL → journeyWorker/canon.
- [x] 1.6 `bun install` regenerate `bun.lock`; `bun run build` the
      launcher.

## 2. Living docs

- [x] 2.1 README, getting-started EN/KR, repo-scaffold skill (+
      rematerialize), design-doc Distribution line, presentation,
      site copy → `bunx canoncli` / `canoncli-core-*`. Archived
      openspec docs untouched.

## 3. Verification

- [x] 3.1 `check-release-workflow-safety.py` green (names coherent).
- [x] 3.2 Launcher resolution smoke: built `dist/index.js` execs a
      staged platform binary via the unscoped/monorepo path
      (`canon 0.1.0`).
- [ ] 3.3 Operator actions (out of repo): set `NPM_TOKEN` secret;
      push `v0.1.0` tag to trigger the first publish; verify
      `bunx canoncli --version` on a clean machine.
