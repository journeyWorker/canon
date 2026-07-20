# s34 scope-publish-naming — tasks

## 1. Rename (load-bearing)

- [x] 1.1 `packages/cli/package.json`: name → `@journeykit/canon`,
      optionalDependencies → `@journeykit/canon-core-<platform>`.
- [x] 1.2 `packages/core-{darwin-arm64,linux-x64}/package.json`:
      name → `@journeykit/canon-core-<platform>`.
- [x] 1.3 `packages/cli/src/index.ts`: resolve scoped
      `@journeykit/canon-core-<platform>` (scoped node_modules:
      sibling / nested / hoisted), monorepo-dev `packages/<dir>` path
      unchanged, error-message name updated.
- [x] 1.4 `.github/workflows/publish.yml` matrix `package_name` +
      step labels + header naming comment.
- [x] 1.5 `bun install` regenerate `bun.lock`; `bun run build` the
      launcher.

## 2. Living docs

- [x] 2.1 README, getting-started + examples + architecture + index +
      canon concept (EN/KO), repo-scaffold skill (3 copies), design-doc
      Distribution line, presentation, site copy →
      `bunx @journeykit/canon` / `@journeykit/canon-core-*`. Archived
      openspec docs untouched.

## 3. Verification

- [x] 3.1 `check-release-workflow-safety.py` green (names coherent).
- [x] 3.2 Launcher resolution smoke: built `dist/index.js` execs a
      staged `@journeykit/canon-core-<platform>` sibling binary via the
      scoped `node_modules/@journeykit/` path, passing args + exit code.
- [ ] 3.3 Operator actions (out of repo): own the `@journeykit` npm
      scope with trusted publisher / token; push a `vX.Y.Z` tag to
      trigger publish; verify `bunx @journeykit/canon --version` on a
      clean machine.
