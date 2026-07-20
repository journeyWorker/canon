export const CANVAS_W = 960;
export const CANVAS_H = 540;
export const TILE = 48;

export const GRAVITY = 2000;
export const JUMP_VELOCITY = -760;
export const MOVE_SPEED = 240;
export const MAX_FALL = 900;
export const COYOTE_TIME = 0.10;
export const JUMP_BUFFER = 0.10;
export const PLAYER_HITBOX = { w: 40, h: 44 };
export const ACORN_HITBOX = 28;
export const MAX_DT = 0.05;

export const PLAYER_DRAW = { w: 76, h: 76 };
export const SQUIRREL_DRAW = { w: 64, h: 64 };
export const ACORN_DRAW = 34;
export const ACORN_ICON = 24;

export const SQUASH_DURATION = 0.12;
export const SQUASH_FALL_THRESHOLD = 200;

export const BG_SCALE = 0.75;
export const BG_PARALLAX = 0.12;
export const HILLS_PARALLAX = 0.30;

// Fixed-timestep simulation: physics always advances in FIXED_DT slices
// regardless of render frame rate, matching v1's implicit 60fps tuning.
export const FIXED_DT = 1 / 60;

// --- Hearts / pinecone enemies (design doc: enemy spec) ---
export const MAX_HEARTS = 3;
export const HIT_INVULN_DURATION = 1.0;
export const PINECONE_SPEED = 90; // px/s, per design's enemy spec
export const PINECONE_HITBOX = { w: 32, h: 32 }; // per design's enemy spec
export const PINECONE_DRAW = { w: 56, h: 52 };
export const KNOCKBACK_VX = 320; // "vx away from enemy"
export const KNOCKBACK_VY = JUMP_VELOCITY / 3; // "vy = jump/3"
// Briefly locks out input-driven horizontal velocity so the knockback
// impulse actually moves the player before held-direction input can
// overwrite vx on the very next physics step.
export const KNOCKBACK_LOCKOUT = 0.15;

// --- Moving platforms (design doc: moving-platform spec) ---
export const PLATFORM_AMPLITUDE = 120; // px, total oscillation range about the anchor
export const PLATFORM_SPEED = 60; // px/s
