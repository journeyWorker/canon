---
version: 1
slug: "src-pages-index-astro"
primary_target: "src/pages/index.astro"
related_targets: ["src/pages/ko/index.astro"]
---

# Landing surface brief

Scope: the canon site landing (`/` and `/ko/`), Persuade mode. The docs pages (Read mode) re-themed into the same world.

Audience: leads and engineers on teams shipping with AI coding agents, judging whether canon fits and how to install it.
Job: understand that canon gates "done" on recorded evidence, see proof on canon's own corpus, copy the install command or open the docs.
Proof: Fig. 1 is generated at build time from canon's own specs, git history, evidence ledger, and a real `canon gate check` run (`src/data/corpus-figure.json`, `bun run figure:data`).
Constraints: comp words verbatim; no invented taglines or claims; other-tool claims only with a linked primary source; experimental features labeled; EN and KO.
Approved comp: `.impeccable/comps/landing.png` (Grid Manual, 1536x1024).

## Direction contract

THESIS: canon's contract set as an engineering standard: every claim flush-left on a strict twelve-column grid, every proof a numbered figure the reader can check. Refuses the dev-tool hero with a glowing terminal and a feature-card grid.

OWN-WORLD: bright white ground #f3f3ef, ink #111, one signal-orange field #ff4a1c that owns a whole region, grey hairline column rules always visible. Spline Sans bold at poster scale, Spline Sans regular for text, Cousine for commands, captions, and data. Square corners, black command bar, underlined links, numbered Fig. captions, one black rule carrying the stations.

STORY: the visitor reads "Agents say done. canon makes them prove it.", sees canon's own 246 scenarios with 0 violations, copies the install command, then follows the loop (Fig. 2), sees where canon sits beside process and runtime tools (Fig. 3), and enters the docs.

FIRST VIEWPORT: left seven columns: headline at 111px, the line "A verification gate for agent-written work.", black command field with Copy, guide link. Right five columns: orange Fig. 1 panel, caption, one hairline per scenario on a time axis, "0 violations" huge at its foot. Full-width black rule at the bottom with six stations on column lines. Primary action: the command field.

FORM: Grid Manual, International Typographic Style engineering manual; the assigned direction (kind assigned, first on the ordered list). Seed key: not recorded in this checkout; the build started from the approved comp with `build-phase start --comp`.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

## Memorable moment

Fig. 1: canon's real corpus drawn as hairlines, nine of them carrying a white gap where evidence came later, ending in the gate's real count.

## Unresolved

None.
