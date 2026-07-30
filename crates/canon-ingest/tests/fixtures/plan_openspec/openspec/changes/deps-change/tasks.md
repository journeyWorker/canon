## 1. Declared dependencies

Sequencing follows design.md: **P1 lands before P2**; P2 depends on 1.1 and P3
depends on all of P1-P2. This is PHASE-level preamble prose following a heading,
so it attaches to no row and contributes no dependency to any task.

- [ ] 1.1 seed the corpus
- [ ] 1.2 wire the extractor, depends on 1.1
- [ ] 1.3 resolve every declared reference
      Blocked by 1.1/1.2 — this marker sits in a wrapped continuation line, the
      shape the live corpus actually uses.
- [ ] 1.4 an ordinary row with no dependency prose at all
- [ ] 1.5 cite a sibling row this change does not have, depends on 9.9
- [ ] 1.6 never a dependency: depends on `cel` 0.14.0, needs only P1, and gives up
      after 3 seconds
