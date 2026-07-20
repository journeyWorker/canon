import type { EnemyState, EnemySpawn, PlayerState, LevelData } from './types';
import { aabbOverlap, resolveAxis } from './collision';
import { PINECONE_SPEED, PINECONE_HITBOX, GRAVITY, MAX_FALL } from './constants';

export function spawnEnemies(defs: EnemySpawn[]): EnemyState[] {
  return defs.map((def, i) => ({
    id: `pinecone-${i}`,
    x: def.x, y: def.y, w: PINECONE_HITBOX.w, h: PINECONE_HITBOX.h,
    vx: PINECONE_SPEED, vy: 0,
    onGround: false,
    alive: true,
    dir: 1,
    patrolMinX: def.patrolMinX, patrolMaxX: def.patrolMaxX,
  }));
}

// Gravity-bound (sits on tiles, no jumping) horizontal patrol: walks toward
// patrolMaxX/patrolMinX and reverses at either bound, matching the design's
// "reverses one tile before the ledge ends or on hitting a wall" (the
// patrol bounds are authored inset from the real ledge edges; a genuine
// mid-ledge wall also zeroes vx via resolveAxis, which flips the enemy too).
export function updateEnemies(enemies: EnemyState[], dt: number, level: LevelData): void {
  for (const enemy of enemies) {
    if (!enemy.alive) continue;
    enemy.vx = PINECONE_SPEED * enemy.dir;
    enemy.vy = Math.min(enemy.vy + GRAVITY * dt, MAX_FALL);

    enemy.x += enemy.vx * dt;
    resolveAxis(enemy, 'x', level);
    enemy.y += enemy.vy * dt;
    resolveAxis(enemy, 'y', level);

    if (enemy.x <= enemy.patrolMinX) { enemy.x = enemy.patrolMinX; enemy.dir = 1; }
    else if (enemy.x >= enemy.patrolMaxX) { enemy.x = enemy.patrolMaxX; enemy.dir = -1; }
    else if (enemy.onGround && enemy.vx === 0) { enemy.dir = enemy.dir === 1 ? -1 : 1; } // hit a wall mid-ledge
  }
}

export function checkPlayerHit(player: PlayerState, enemies: EnemyState[]): boolean {
  if (player.invulnTimer > 0) return false;
  return enemies.some((e) => e.alive && aabbOverlap(player, e));
}

// Used only to compute a knockback direction once checkPlayerHit fires.
export function findOverlappingEnemy(player: PlayerState, enemies: EnemyState[]): EnemyState | null {
  return enemies.find((e) => e.alive && aabbOverlap(player, e)) ?? null;
}
