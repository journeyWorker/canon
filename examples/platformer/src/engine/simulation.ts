import type { AcornState, PlayerState, LevelData, EnemyState, MovingPlatformState } from './types';
import type { InputSnapshot } from './input';
import { updatePlayer, spawnPlayer } from './player';
import { updateCamera } from './camera';
import { aabbOverlap } from './collision';
import { findSpawn, findAllSpawns } from './level';
import { spawnEnemies, updateEnemies, checkPlayerHit, findOverlappingEnemy } from './enemies';
import { spawnPlatforms, updateMovingPlatforms } from './platforms';
import {
  ACORN_HITBOX, TILE, FIXED_DT, MAX_DT, MAX_HEARTS, HIT_INVULN_DURATION,
  KNOCKBACK_VX, KNOCKBACK_VY, KNOCKBACK_LOCKOUT,
} from './constants';

export interface LevelResult { levelIndex: number; acorns: number; acornsTotal: number; timeSeconds: number; }

export interface HudSnapshot {
  paused: boolean;
  hearts: number;
  maxHearts: number;
  gameOver: boolean;
  levelIndex: number;
  levelComplete: boolean;
  unlockedLevels: number[];
  acorns: number;
  acornsTotal: number;
  elapsed: number;
  levelResult: LevelResult | null;
}

export interface RenderSnapshot {
  player: PlayerState;
  acorns: AcornState[];
  enemies: EnemyState[];
  platforms: MovingPlatformState[];
  camera: { x: number };
  squirrel: { x: number; y: number; w: number; h: number };
  elapsed: number;
  levelIndex: number;
}

const EMPTY_INPUT: InputSnapshot = { left: false, right: false, jumpBuffered: false, restartRequested: false };

export class GameSimulation {
  private levels: LevelData[];
  private level: LevelData;
  private levelIndex: number;
  private player: PlayerState;
  private acorns: AcornState[];
  private enemies: EnemyState[];
  private platforms: MovingPlatformState[];
  private squirrel: { x: number; y: number; w: number; h: number };
  private camera = { x: 0 };
  private elapsed = 0;
  private accumulator = 0;
  private pendingInput: InputSnapshot = { ...EMPTY_INPUT };
  private paused = false;
  private hearts = MAX_HEARTS;
  private gameOver = false;
  private levelComplete = false;
  private levelResult: LevelResult | null = null;
  private standingPlatformId: string | null = null;
  private unlockedLevels: number[] = [0];
  private listeners = new Set<() => void>();
  private hudCache: HudSnapshot;

  constructor(levels: LevelData[], startIndex = 0, initialUnlockedLevels: number[] = [0]) {
    this.levels = levels;
    this.levelIndex = startIndex;
    this.unlockedLevels = initialUnlockedLevels.includes(startIndex) ? initialUnlockedLevels : [...initialUnlockedLevels, startIndex];
    const init = this.buildLevelState(startIndex);
    this.level = init.level;
    this.player = init.player;
    this.acorns = init.acorns;
    this.enemies = init.enemies;
    this.platforms = init.platforms;
    this.squirrel = init.squirrel;
    this.hudCache = this.computeHud();
  }

  private buildLevelState(index: number) {
    const level = this.levels[index];
    const spawn = findSpawn(level, 'S');
    const player = spawnPlayer(spawn);
    const acorns = findAllSpawns(level, 'A').map((pos) => ({
      x: pos.x + (TILE - ACORN_HITBOX) / 2, y: pos.y + (TILE - ACORN_HITBOX) / 2,
      w: ACORN_HITBOX, h: ACORN_HITBOX, collected: false,
    }));
    const goal = findSpawn(level, 'Q');
    const squirrel = { x: goal.x, y: goal.y, w: TILE, h: TILE };
    const enemies = spawnEnemies(level.enemies);
    const platforms = spawnPlatforms(level.platforms);
    return { level, player, acorns, enemies, platforms, squirrel };
  }

  private loadLevel(index: number): void {
    this.levelIndex = index;
    const next = this.buildLevelState(index);
    this.level = next.level;
    this.player = next.player;
    this.acorns = next.acorns;
    this.enemies = next.enemies;
    this.platforms = next.platforms;
    this.squirrel = next.squirrel;
    this.camera = { x: 0 };
    this.elapsed = 0;
    this.accumulator = 0;
    this.hearts = MAX_HEARTS;
    this.gameOver = false;
    this.levelComplete = false;
    this.levelResult = null;
    this.standingPlatformId = null;
  }

  setInput(input: InputSnapshot): void {
    // Merge rather than overwrite: one-shot flags fire on whichever call sets them true.
    this.pendingInput = {
      left: input.left, right: input.right,
      jumpBuffered: this.pendingInput.jumpBuffered || input.jumpBuffered,
      restartRequested: this.pendingInput.restartRequested || input.restartRequested,
    };
  }

  setPaused(paused: boolean): void { this.paused = paused; this.notify(); }

  goToLevel(index: number): void {
    if (!this.unlockedLevels.includes(index)) return;
    this.loadLevel(index);
    this.notify();
  }

  restartLevel(): void {
    this.loadLevel(this.levelIndex);
    this.notify();
  }

  tick(rawDt: number): void {
    if (this.paused) return;
    if (this.pendingInput.restartRequested) {
      this.pendingInput = { ...this.pendingInput, restartRequested: false };
      this.restartLevel();
      return;
    }
    this.accumulator += Math.min(rawDt, MAX_DT);
    let stepped = false;
    while (this.accumulator >= FIXED_DT) {
      const stepInput = stepped ? { ...this.pendingInput, jumpBuffered: false, restartRequested: false } : this.pendingInput;
      this.stepOnce(FIXED_DT, stepInput);
      this.accumulator -= FIXED_DT;
      stepped = true;
    }
    // Only consume the one-shot flags once they've actually reached a physics
    // step: a render-rate tick() call that accumulates less than one FIXED_DT
    // slice (e.g. a display refresh rate above 60Hz) must leave a buffered
    // jump/restart pending for the next call instead of silently dropping it.
    if (stepped) {
      this.pendingInput = { ...this.pendingInput, jumpBuffered: false, restartRequested: false };
    }
    this.notify();
  }

  private stepOnce(dt: number, input: InputSnapshot): void {
    // Mirrors v1's `if (!gameWon) { ... }` guard: a finished level or a
    // game-over freezes the whole simulation, including the elapsed timer.
    if (this.gameOver || this.levelComplete) return;
    this.elapsed += dt;
    const worldW = this.level.grid[0].length * TILE;

    const platformDeltas = updateMovingPlatforms(this.platforms, dt);
    const carry = this.standingPlatformId
      ? (platformDeltas.find((d) => d.id === this.standingPlatformId) ?? null)
      : null;

    const result = updatePlayer(
      this.player, input, dt, this.level, this.platforms,
      carry ? { dx: carry.dx, dy: carry.dy } : null
    );
    if (result === 'RESPAWN') {
      const spawn = findSpawn(this.level, 'S');
      const fresh = spawnPlayer(spawn);
      Object.assign(this.player, fresh);
      this.standingPlatformId = null;
    } else {
      this.standingPlatformId = result && this.platforms.some((p) => p.id === result) ? result : null;
    }

    for (const acorn of this.acorns) {
      if (!acorn.collected && aabbOverlap(this.player, acorn)) acorn.collected = true;
    }

    updateEnemies(this.enemies, dt, this.level);
    this.player.invulnTimer = Math.max(0, this.player.invulnTimer - dt);
    if (checkPlayerHit(this.player, this.enemies)) {
      const enemy = findOverlappingEnemy(this.player, this.enemies);
      this.hearts = Math.max(0, this.hearts - 1);
      this.player.invulnTimer = HIT_INVULN_DURATION;
      if (enemy) {
        const playerCx = this.player.x + this.player.w / 2;
        const enemyCx = enemy.x + enemy.w / 2;
        const away = Math.sign(playerCx - enemyCx) || (this.player.facing === 1 ? -1 : 1);
        this.player.vx = away * KNOCKBACK_VX;
        this.player.knockbackTimer = KNOCKBACK_LOCKOUT;
        this.player.vy = KNOCKBACK_VY;
        this.player.onGround = false;
      }
      if (this.hearts <= 0) this.gameOver = true;
    }

    updateCamera(this.camera, this.player, worldW);

    if (!this.levelComplete && !this.gameOver && aabbOverlap(this.player, this.squirrel)) {
      this.levelComplete = true;
      const acornsCollected = this.acorns.filter((a) => a.collected).length;
      this.levelResult = {
        levelIndex: this.levelIndex, acorns: acornsCollected,
        acornsTotal: this.acorns.length, timeSeconds: this.elapsed,
      };
      if (this.levelIndex + 1 < this.levels.length && !this.unlockedLevels.includes(this.levelIndex + 1)) {
        this.unlockedLevels = [...this.unlockedLevels, this.levelIndex + 1];
      }
    }
  }

  private computeHud(): HudSnapshot {
    return {
      paused: this.paused,
      hearts: this.hearts,
      maxHearts: MAX_HEARTS,
      gameOver: this.gameOver,
      levelIndex: this.levelIndex,
      levelComplete: this.levelComplete,
      unlockedLevels: this.unlockedLevels,
      acorns: this.acorns.filter((a) => a.collected).length,
      acornsTotal: this.acorns.length,
      elapsed: this.elapsed,
      levelResult: this.levelResult,
    };
  }

  getHudSnapshot(): HudSnapshot {
    const next = this.computeHud();
    const changed = (Object.keys(next) as (keyof HudSnapshot)[]).some((k) => next[k] !== this.hudCache[k]);
    if (changed) this.hudCache = next;
    return this.hudCache;
  }

  getRenderSnapshot(): RenderSnapshot {
    return {
      player: this.player, acorns: this.acorns, enemies: this.enemies, platforms: this.platforms,
      camera: this.camera, squirrel: this.squirrel, elapsed: this.elapsed, levelIndex: this.levelIndex,
    };
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private notify(): void { this.listeners.forEach((l) => l()); }
}
