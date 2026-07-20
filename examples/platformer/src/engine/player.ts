import type { InputSnapshot } from './input';
import type { PlayerState, LevelData, Rect } from './types';
import { resolveAxis } from './collision';
import {
  MOVE_SPEED, COYOTE_TIME, JUMP_BUFFER, JUMP_VELOCITY, GRAVITY, MAX_FALL,
  SQUASH_DURATION, SQUASH_FALL_THRESHOLD, TILE, PLAYER_HITBOX,
} from './constants';

export function updatePlayer(
  state: PlayerState, input: InputSnapshot, dt: number, level: LevelData,
  extraSolids: (Rect & { id: string })[] = [],
  pendingCarry: { dx: number; dy: number } | null = null
): string | null {
  // A moving platform's delta from this tick is applied before the
  // player's own input-driven move so the carry is exactly one tick's
  // platform motion; stepping off next tick (no overlap) drops the player
  // under ordinary gravity with no warp or clip.
  if (pendingCarry) { state.x += pendingCarry.dx; state.y += pendingCarry.dy; }

  // A held direction must not instantly clobber a knockback impulse on the
  // very next physics step -- only drive vx from input once the lockout
  // set by the hit has decayed.
  if (state.knockbackTimer > 0) state.knockbackTimer = Math.max(0, state.knockbackTimer - dt);
  else state.vx = input.left ? -MOVE_SPEED : input.right ? MOVE_SPEED : 0;
  if (input.left) state.facing = -1;
  else if (input.right) state.facing = 1;

  if (state.onGround) state.coyoteTimer = COYOTE_TIME;
  else state.coyoteTimer = Math.max(0, state.coyoteTimer - dt);

  if (input.jumpBuffered) state.jumpBufferTimer = JUMP_BUFFER;
  state.jumpBufferTimer = Math.max(0, state.jumpBufferTimer - dt);

  if (state.jumpBufferTimer > 0 && state.coyoteTimer > 0) {
    state.vy = JUMP_VELOCITY;
    state.onGround = false;
    state.jumpBufferTimer = 0;
    state.coyoteTimer = 0;
  }

  state.vy = Math.min(state.vy + GRAVITY * dt, MAX_FALL);

  const wasGrounded = state.onGround;
  const fallSpeed = state.vy;

  state.x += state.vx * dt;
  resolveAxis(state, 'x', level);
  const prevBottom = state.y + state.h;
  state.y += fallSpeed * dt;
  const landedOnId = resolveAxis(state, 'y', level, extraSolids, { prevBottom, vy: fallSpeed });

  if (!wasGrounded && state.onGround && fallSpeed > SQUASH_FALL_THRESHOLD) {
    state.squashTimer = SQUASH_DURATION;
  }
  state.squashTimer = Math.max(0, state.squashTimer - dt);

  const worldH = level.grid.length * TILE;
  if (state.y > worldH + 200) {
    return 'RESPAWN'; // simulation.ts interprets this sentinel and repositions the player
  }
  return landedOnId;
}

export function spawnPlayer(spawn: { x: number; y: number }): PlayerState {
  return {
    x: spawn.x + (TILE - PLAYER_HITBOX.w) / 2,
    y: spawn.y + (TILE - PLAYER_HITBOX.h),
    w: PLAYER_HITBOX.w, h: PLAYER_HITBOX.h,
    vx: 0, vy: 0, onGround: false, facing: 1,
    coyoteTimer: 0, jumpBufferTimer: 0, squashTimer: 0, invulnTimer: 0, knockbackTimer: 0,
  };
}
