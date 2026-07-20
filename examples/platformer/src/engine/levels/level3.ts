import type { LevelData } from '../types';

// Level 3 grid verbatim from the design doc (Level 3 -- hard). Three pinecone
// patrol spawns guard the elevated '###' ledges (row 2); three PPP platforms
// (row 4) are the only crossing over the wide floor chasm. Per the moving-
// platform spec ("L3 mixes: at least one vertical"), the middle platform
// rides vertically toward the row-2 ledge instead of horizontally.
export const LEVEL3: LevelData = {
  id: 2,
  grid: [
    '........................................',
    '..........A..........A..........A.......',
    '.......###........###........###........',
    '....E..........E.........E..............',
    '..PPP.......PPP......PPP.....A..........',
    '.....A..........A..........A............',
    '..A.......................A....A........',
    '###...................................##',
    '.S....................................AQ',
    '........................................',
    '####..................................##',
  ],
  enemies: [
    { x: 384, y: 64, patrolMinX: 384, patrolMaxX: 400 }, // marker (3,4) -> ledge row 2 cols (7, 9)
    { x: 912, y: 64, patrolMinX: 912, patrolMaxX: 928 }, // marker (3,15) -> ledge row 2 cols (18, 20)
    { x: 1440, y: 64, patrolMinX: 1440, patrolMaxX: 1456 }, // marker (3,25) -> ledge row 2 cols (29, 31)
  ],
  platforms: [
    { x: 96, y: 192, w: 144, h: 48, axis: 'x', min: 36, max: 156, speed: 60 },
    { x: 576, y: 192, w: 144, h: 48, axis: 'y', min: 132, max: 252, speed: 60 },
    { x: 1008, y: 192, w: 144, h: 48, axis: 'x', min: 948, max: 1068, speed: 60 },
  ],
};
