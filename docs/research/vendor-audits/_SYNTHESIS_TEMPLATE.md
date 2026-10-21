# Vendor Audit — <vendor> — Synthesis

Rollup of the N per-surface audits at `docs/research/vendor-audits/<vendor>/`.
Every claim cites a source audit (`[<slug>.md §X.Y]`) or is prefixed
"Synthesis note:" with an explicit rationale.

## 0. Executive summary

Half-page. The 3–5 most consequential findings for canon. Each finding cited.

## 1. Version frame

- Effect / framework version posture (cite `util-effect.md §4` for any version-mismatch finding).
- Net implication: which surfaces are drop-in vs which need back-porting.
- Vendor license noted for the record (cite `docs-site.md §4` or equivalent). Canon's default adoption posture is **clean-room reimplementation from the audit spec**, so license is not a leverage-ranking factor here. It becomes load-bearing only at PR-author time for the rare verbatim-lift PR.

## 2. Top-10 patterns to lift (ranked by leverage × effort)

For each, in this exact shape:

- **#N <pattern name>** — what it does (1 sentence), source (`<slug>.md §3.X` + `vendors/<vendor>/<path>:<lines>`), canon target (concrete path), effort tag (trivial / moderate / significant), one-line rationale for the ranking.

Pick the strongest 10 by leverage × effort; mention runners-up in §3.

## 3. Runners-up & deferred extracts

Bulleted list of additional patterns that didn't make top-10 but are worth tracking. Each with `<slug>.md §X.Y` cite.

## 4. Critical overlaps with canon's in-flight work

Sub-sections for each major overlap. Synthesize across 2–4 audits per overlap with concrete canon learnings.

## 5. Per-canon-surface action plan

One sub-section per canon surface, listing concrete next actions ordered by leverage. Each action: `EXTRACT|INSPIRE` + concrete target path + which audit drives it + effort tag.

## 6. Risk register

Numbered. Pull from the §4 (Risks) of each audit. Cluster duplicates. Each row: risk, severity (high/medium/low), affected surfaces, mitigation. ≥6 rows.

## 7. Recommended next 3 PRs

Concrete proposal for the 3 highest-leverage PRs canon should ship from this audit. Each: title, scope (files touched), source audits, definition of done.
