# canon project website — Design

- **Status:** approved by operator, 2026-07-14 (autonomy granted for execution)
- **Date:** 2026-07-14
- **Home:** `packages/website` (Bun workspace member)
- **Deploy:** Vercel, static output

## 1. Goal

A public project website for canon: a landing page that communicates the
project concept (harness knowledge substrate, musical-canon metaphor) plus a
curated set of core docs, authored in MDX, in English and Korean.

## 2. Decisions

1. **Stack:** Astro 5 + Starlight + MDX. Static build (no SSR adapter).
   Starlight chosen over a hand-rolled Astro site because EN+KR i18n,
   sidebar, full-text search (Pagefind), dark mode, and a11y come built in.
2. **Location:** `packages/website`, joining the existing `packages/*` Bun
   workspace. Site is self-contained; no imports from crates or other
   packages.
3. **i18n:** Starlight built-in locales. English at root (`defaultLocale:
   root`), Korean under `/ko/`. Sidebar labels translated; missing-page
   fallback to English is automatic.
4. **Content** (9 pages × 2 languages = 18 MDX files, hand-curated from
   README, `2026-07-10-canon-design.md`, and `canon/skills/*/SKILL.md` —
   never auto-generated):
   - `/` — landing (splash): hero, value cards, flywheel, quick start
   - `/docs/getting-started` — install (`bunx canon`), first run
   - `/docs/concepts/canon` — the metaphor; the problem canon solves
   - `/docs/concepts/trust-spine` — evidence-or-nothing gate, trust ladder
   - `/docs/concepts/tiered-storage` — git / hot / cold tiers
   - `/docs/concepts/strategy-memory` — role-scoped memory, flywheel
   - `/docs/architecture` — crate map (model/store/ingest/gate/learn/report/cli)
   - `/docs/cli` — command overview
   - `/docs/examples` — end-to-end walkthroughs (consumer-repo wiring,
     ingest → gate → report loop)
5. **Theme "Score & Voices":** musical-canon + evidence-first identity.
   - Dark default: ink black / deep navy; light mode: sepia paper.
   - Single accent: amber/gold.
   - Type: serif display headings (Newsreader), system sans body,
     JetBrains Mono code.
   - Landing hero: staggered staff-line SVG motif — five voices
     (planning / design / dev / test / review) entering in imitation.
   - Implementation: Starlight CSS custom-property overrides + custom
     splash components only. No deep component forks.
6. **Vercel:** `vercel.json` committed in `packages/website` (framework
   astro, bun install/build). Project root directory is set to
   `packages/website` in the Vercel dashboard by the operator.

## 3. Architecture

```
packages/website/
  package.json            astro, @astrojs/starlight, @fontsource/*
  astro.config.mjs        starlight: title, locales, sidebar, customCss
  vercel.json             build config for Vercel
  src/
    content/docs/         EN pages (root locale)
      index.mdx           landing (template: splash)
      getting-started.mdx
      concepts/*.mdx      canon, trust-spine, tiered-storage, strategy-memory
      architecture.mdx
      cli.mdx
      examples.mdx
    content/docs/ko/      KR mirror of the same tree
    components/           landing-only components (Hero voices motif, cards)
    styles/theme.css      Score & Voices custom properties + typography
```

## 4. Error handling / constraints

- Build must pass `astro build` with zero content-collection schema errors;
  internal links are verified against the built `dist/` (stock Starlight does
  not validate links at build time).
- No timestamps in generated output (repo convention).
- Docs content states facts only from README/design doc/skills — no invented
  CLI flags or behavior.

## 5. Testing

- `bun run build` green in `packages/website`.
- Local preview smoke in browser: landing EN/KR renders hero + cards; one
  concept page per language renders with sidebar, search index built;
  language switcher round-trips.

## 6. Out of scope

- Auto-generating docs from skills or `--help` output.
- Versioned docs, blog, changelog page.
- Custom domain / DNS (operator handles in Vercel dashboard).
