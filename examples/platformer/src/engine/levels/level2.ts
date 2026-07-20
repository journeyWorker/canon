import type { LevelData } from '../types';

// Level 2 grid verbatim from the design doc (Level 2 -- medium). Two pinecone
// patrol spawns on the elevated '###'/'####' ledges (row 3); two horizontal
// moving platforms (PPP runs at row 5) bridge the floor gaps below.
export const LEVEL2: LevelData = {
  id: 1,
  grid: [
    '........................................',
    '........................................',
    '............A..............A............',
    '.........###.........A...####...........',
    '....A.......E..............E....A.......',
    '...###....PPP....###....PPP...###.......',
    '.A.....................A................',
    '###.......A.......A..........A......A...',
    '.S..A..............................A...Q',
    '........................................',
    '#####...####....####....####....########',
  ],
  enemies: [
    { x: 496, y: 112, patrolMinX: 480, patrolMaxX: 496 }, // marker (4,12) -> ledge row 3 cols (9, 11)
    { x: 1304, y: 112, patrolMinX: 1248, patrolMaxX: 1312 }, // marker (4,27) -> ledge row 3 cols (25, 28)
  ],
  platforms: [
    { x: 480, y: 240, w: 144, h: 48, axis: 'x', min: 420, max: 540, speed: 60 },
    { x: 1152, y: 240, w: 144, h: 48, axis: 'x', min: 1092, max: 1212, speed: 60 },
  ],
};
