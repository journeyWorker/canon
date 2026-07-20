import type { Groundable, LevelData, Rect } from './types';
import { TILE } from './constants';
import { tileAt } from './level';

export function aabbOverlap(a: Rect, b: Rect): boolean {
  return a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y;
}

// Resolves `entity` against solid '#' tiles via AABB sweep, plus -- on the
// y-axis only -- moving-platform rects passed as extraSolids. Call once
// after an X-only move and once after a Y-only move -- resolving both axes
// in one combined pass misreads a shallow ledge-edge overlap as a wall hit
// even while the entity is deeply penetrating vertically, dropping it
// through the floor.
//
// Moving platforms are one-way-top solids: they never block from the side
// or the underside (skipped entirely on the x-axis), and only catch a
// landing when the entity was at/above the platform's top surface last
// frame and is moving downward this frame -- `oneWay` carries that
// previous-bottom/velocity context for the y-axis platform check.
export function resolveAxis(
  entity: Groundable,
  axis: 'x' | 'y',
  level: LevelData,
  extraSolids: (Rect & { id: string })[] = [],
  oneWay: { prevBottom: number; vy: number } | null = null
): string | null {
  if (axis === 'y') entity.onGround = false;
  let landedOnId: string | null = null;

  const firstCol = Math.floor(entity.x / TILE);
  const lastCol = Math.floor((entity.x + entity.w) / TILE);
  const firstRow = Math.floor(entity.y / TILE);
  const lastRow = Math.floor((entity.y + entity.h) / TILE);

  for (let row = firstRow; row <= lastRow; row++) {
    for (let col = firstCol; col <= lastCol; col++) {
      if (tileAt(level, col, row) !== '#') continue;
      landedOnId = resolveOne(entity, axis, col * TILE, row * TILE, TILE, TILE, landedOnId, null);
    }
  }
  if (axis === 'y' && oneWay) {
    for (const solid of extraSolids) {
      landedOnId = resolveTopOnly(entity, solid, oneWay.prevBottom, oneWay.vy, landedOnId);
    }
  }
  return landedOnId;
}

function resolveOne(
  entity: Groundable, axis: 'x' | 'y',
  tileX: number, tileY: number, tileW: number, tileH: number,
  landedOnId: string | null, solidId: string | null
): string | null {
  const overlapX = Math.min(entity.x + entity.w, tileX + tileW) - Math.max(entity.x, tileX);
  const overlapY = Math.min(entity.y + entity.h, tileY + tileH) - Math.max(entity.y, tileY);
  if (overlapX <= 0 || overlapY <= 0) return landedOnId;

  if (axis === 'x') {
    entity.x += entity.x < tileX ? -overlapX : overlapX;
    entity.vx = 0;
  } else {
    if (entity.y < tileY) {
      entity.y -= overlapY;
      entity.vy = 0;
      entity.onGround = true;
      landedOnId = solidId;
    } else {
      entity.y += overlapY;
      entity.vy = 0;
    }
  }
  return landedOnId;
}

// One-way-top resolution for a moving platform: lands the entity on the
// top surface only when it was at/above that surface last frame and is
// moving downward this frame; never blocks from the side or pushes the
// entity out from underneath.
function resolveTopOnly(
  entity: Groundable,
  solid: Rect & { id: string },
  prevBottom: number,
  vy: number,
  landedOnId: string | null
): string | null {
  if (vy < 0 || prevBottom > solid.y) return landedOnId;
  const overlapX = Math.min(entity.x + entity.w, solid.x + solid.w) - Math.max(entity.x, solid.x);
  const overlapY = Math.min(entity.y + entity.h, solid.y + solid.h) - Math.max(entity.y, solid.y);
  if (overlapX <= 0 || overlapY <= 0) return landedOnId;
  entity.y = solid.y - entity.h;
  entity.vy = 0;
  entity.onGround = true;
  return solid.id;
}
