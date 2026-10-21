# Surface — <slug>

**Vendor paths**: `vendors/<vendor>/<path1>`, `vendors/<vendor>/<path2>`
**Scale**: <file count> files / <approx LOC>
**Primary language(s)**: TypeScript / Go / etc.
**Canon analog (if any)**: `<canon path>` — one line on relationship.

## 1. Overview

3–5 sentences. What this surface IS. Why it exists. Who uses it. Is it standalone or coupled?

## 2. Entry points & architecture

- **Entry file(s)**: `vendors/<vendor>/<path>:<lines>` — what bootstraps this surface.
- **Core abstractions**: name + 1-line role for the top 3–7 types/classes/Effect services.
- **Cross-package dependencies**: list of vendor packages this surface imports from (e.g. `packages/core`, `packages/llm`).
- **External runtime dependencies**: notable libraries (Effect version, AI SDK version, Solid/React, etc.).

## 3. Pattern catalog

Numbered list of 3–7 patterns worth lifting. For each:

### 3.<n>. <Pattern name>

- **Where**: `vendors/<vendor>/<path>:<line-range>`
- **What it does**: 2–3 sentences. Be concrete — name the abstraction, the data shape, the lifecycle.
- **Why it's interesting**: what problem it solves that canon (or agents using canon) also face.
- **Canon applicability**: which canon path/feature this maps to (concrete: `canon/skills/SKILL.src.md`, `canon/skills/reference/role-*.md`, a `canon <command>`, `crates/canon-cli/src/<file>.rs`, the AGENTS block in `crates/canon-cli/src/init.rs`).
- **Effort to lift**: trivial / moderate / significant.

## 4. Risks & incompatibilities

- **Effect version** (if applicable): note any mismatch with canon's toolchain.
- **Framework**: Solid/React/Vue/etc. — note framework mismatch for UI surfaces.
- **Heavy deps**: anything pulling in massive transitive deps.
- **Coupling**: does this surface only make sense with another surface?
- **TOS-violation patterns** (license-independent): flag anything that impersonates Anthropic / Google / OpenAI clients, forges request fingerprints, or evades provider rate limits — those are SKIP regardless of audit score.
- **License footnote**: canon's default adoption is clean-room reimplementation from the audit spec, so vendor license is not a routing axis here. Note non-MIT / non-Apache files only if they sit in a path a future verbatim-lift PR might target; that PR's author handles attribution.

## 5. Recommended actions

Numbered checklist. Each item is `EXTRACT` / `INSPIRE` / `SKIP`:

1. **EXTRACT** `<thing>` → canon `<path>` — one-line rationale.
2. **INSPIRE** `<pattern>` for canon `<feature>` — one-line rationale.
3. **SKIP** `<thing>` — one-line rationale (incompatible / already solved / out of scope).

## 6. Citations

Bullet list of the 5–10 most informative files/snippets in this surface, with `path:line` ranges. The synthesis pass uses these as the deepest pointers.
