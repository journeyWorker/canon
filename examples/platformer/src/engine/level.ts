import type { LevelData } from './types';
import { TILE } from './constants';
import { LEVEL1 } from './levels/level1';
import { LEVEL2 } from './levels/level2';
import { LEVEL3 } from './levels/level3';

export function tileAt(level: LevelData, col: number, row: number): string {
  if (col < 0 || col >= level.grid[0].length) return '#'; // world edges are solid walls
  if (row < 0 || row >= level.grid.length) return '.';     // above/below the level: open
  return level.grid[row][col];
}

export function findSpawn(level: LevelData, marker: string): { x: number; y: number } {
  for (let row = 0; row < level.grid.length; row++) {
    const col = level.grid[row].indexOf(marker);
    if (col !== -1) return { x: col * TILE, y: row * TILE };
  }
  return { x: 0, y: 0 };
}

export function findAllSpawns(level: LevelData, marker: string): { x: number; y: number }[] {
  const out: { x: number; y: number }[] = [];
  for (let row = 0; row < level.grid.length; row++) {
    for (let col = 0; col < level.grid[row].length; col++) {
      if (level.grid[row][col] === marker) out.push({ x: col * TILE, y: row * TILE });
    }
  }
  return out;
}

export const LEVELS: LevelData[] = [LEVEL1, LEVEL2, LEVEL3];
