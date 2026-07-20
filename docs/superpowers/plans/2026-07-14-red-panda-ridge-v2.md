# Red Panda Ridge v2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the single-file `examples/platformer` canvas game to a Vite + React 18 + PixiJS v8 + TypeScript app, preserving every v1 mechanic exactly while adding pinecone enemies with hearts/invulnerability/game-over, moving platforms, a 3-level campaign with unlock progression, a React HUD/menu shell, and localStorage-persisted per-level best records.

**Architecture:** A fixed-timestep game simulation (`src/engine/*.ts`, plain TypeScript, no DOM/Pixi imports) owns all authoritative state — player physics, enemies, moving platforms, level/camera, hearts, and progression — behind a `GameSimulation` class that accumulates raw frame time and steps in fixed `FIXED_DT` increments (mirroring v1's tuned-at-60fps feel exactly). PixiJS v8 (`src/render/*.ts(x)`) is rendering-only: one `PixiStage` component owns the `Application`, drives `simulation.tick()` from its ticker, and paints sprites each frame by reading `simulation.getRenderSnapshot()` directly — it never triggers a React re-render for per-frame data. React (`src/App.tsx`, `src/hud/*.tsx`) is the shell and HUD: it renders `<PixiStage/>` plus menu/pause/game-over/level-complete overlays, and subscribes to a small `useHudSnapshot()` bridge (`useSyncExternalStore(simulation.subscribe, simulation.getHudSnapshot)`) that only re-renders when HUD-relevant fields (hearts, acorns, level, paused, gameOver, levelComplete) actually change.

**Tech Stack:** Vite, React 18, PixiJS v8, TypeScript, bun (package manager + scripts, `examples/platformer/package.json` is standalone — the repo root `package.json` only globs `workspaces: ["packages/*"]`, so this directory is never pulled into the bun workspace). No test framework; verification is manual smoke testing via `bun run dev`.

## Global Constraints

- App lives at `examples/platformer/` with its own `package.json`, `vite.config.ts`, `tsconfig.json` — never added to the root `workspaces` array.
- Existing assets stay at `examples/platformer/assets/` (`red-panda.png`, `squirrel.png`, `acorn.png`, `tile.png`, `background.png`); three new assets are added at the exact paths `examples/platformer/assets/pinecone.png`, `examples/platformer/assets/heart.png`, `examples/platformer/assets/platform.png`.
- `examples/platformer/specs/` (the S11 feature corpus) is never modified or broken by this plan.
- v1 feel constants carried forward verbatim (px/s unless noted): `GRAVITY = 2000`, `JUMP_VELOCITY = -760`, `MOVE_SPEED = 240`, `MAX_FALL = 900`, `COYOTE_TIME = 0.10` (s), `JUMP_BUFFER = 0.10` (s), `PLAYER_HITBOX = { w: 40, h: 44 }`, `ACORN_HITBOX = 28`, `MAX_DT = 0.05` (s, raw-frame clamp), `TILE = 48`, `PLAYER_DRAW = { w: 76, h: 76 }`, `SQUIRREL_DRAW = { w: 64, h: 64 }`, `ACORN_DRAW = 34`, `ACORN_ICON = 24`, `SQUASH_DURATION = 0.12` (s), `SQUASH_FALL_THRESHOLD = 200` (px/s), `BG_SCALE = 0.75`, `BG_PARALLAX = 0.12`, `HILLS_PARALLAX = 0.30`.
- v1 mechanics preserved exactly: coyote time, jump buffer, squash-on-land, acorn bob (`sin(t*3 + x*0.05)*6`), two-layer parallax background, white-background chroma-key sprite loading (threshold: R/G/B all `>= 240` → alpha 0), world-edge walls (`tileAt` returns `'#'` for out-of-bounds columns), pit respawn (`y > WORLD_H + 200` resets position only, never score/acorns).
- New mechanics (fixed feature set): pinecone patrol enemies; 3 hearts (`MAX_HEARTS = 3`); ~1s invulnerability window after a hit (`HIT_INVULN_DURATION = 1.0`); game-over screen on 0 hearts; moving platforms that carry a standing player; exactly 3 levels with a level-select menu where completing a level unlocks the next; React HUD (hearts/acorns/level) and pause menu (resume/restart/quit-to-menu); level-complete screen (acorns + time stats); localStorage best-record persistence per level.
- Controls unchanged: Arrow keys/WASD move, Space/Up/W jump (edge-triggered buffer), Escape pauses/resumes.

---

### Task 1: Vite/React/Pixi scaffold and asset pipeline

**Files:**
- Create: `examples/platformer/package.json`
- Create: `examples/platformer/vite.config.ts`
- Create: `examples/platformer/tsconfig.json`
- Create: `examples/platformer/index.html`
- Create: `examples/platformer/src/main.tsx`
- Create: `examples/platformer/src/App.tsx`
- Create: `examples/platformer/src/App.css`
- Create: `examples/platformer/src/render/assets.ts`
- Create: `examples/platformer/src/render/PixiStage.tsx`

**Interfaces:**
- Consumes: nothing (first task). Assumes all 8 sprite PNGs already exist at `examples/platformer/assets/{red-panda,squirrel,acorn,tile,background,pinecone,heart,platform}.png` (5 pre-existing, 3 new — produced by the art workstream in parallel; this task only wires the loader against those exact filenames).
- Produces:
  - `ASSET_DEFS: { name: string; chroma: boolean }[]` in `src/render/assets.ts` — the 8-entry asset table (chroma `true` for sprites on a white background: `red-panda`, `squirrel`, `acorn`, `pinecone`, `heart`; `false` for `tile`, `background`, `platform`).
  - `function chromaKeyToCanvas(img: HTMLImageElement): HTMLCanvasElement` — ported 1:1 from v1's `chromaKey()`: draws to an offscreen canvas, zeroes alpha where `r >= 240 && g >= 240 && b >= 240`.
  - `async function loadTextures(): Promise<Record<string, Texture>>` — loads each `assets/${name}.png` via `Image()`, chroma-keys it when flagged, wraps the result (canvas or raw image) in `Texture.from(...)`, resolves once all 8 are ready.
  - `<App />` mounted into `#root` by `src/main.tsx` via `createRoot`.
  - `<PixiStage />` (`src/render/PixiStage.tsx`) — owns a `pixi.Application`, calls `await app.init({ width: 960, height: 540, background: '#F6C89F', antialias: false, roundPixels: true })`, appends `app.canvas` into a `<div ref>`, calls `loadTextures()` on mount and stores the map in a ref for later tasks.

- [x] **Step 1: Write `package.json`**

```json
{
  "name": "red-panda-ridge-v2",
  "version": "0.0.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview"
  },
  "dependencies": {
    "pixi.js": "^8.6.0",
    "react": "^18.3.1",
    "react-dom": "^18.3.1"
  },
  "devDependencies": {
    "@types/react": "^18.3.12",
    "@types/react-dom": "^18.3.1",
    "@vitejs/plugin-react": "^4.3.4",
    "typescript": "^5.6.3",
    "vite": "^5.4.11"
  }
}
```

- [x] **Step 2: Install dependencies**

Run: `cd examples/platformer && bun install`
Expected: `bun.lock` and `node_modules/` created inside `examples/platformer/`; the root `bun.lock`/workspace is untouched since `examples/platformer` is outside `packages/*`.

- [x] **Step 3: Write `vite.config.ts` and `tsconfig.json`**

```typescript
// vite.config.ts
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  base: './',
});
```

```json
// tsconfig.json
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noEmit": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "skipLibCheck": true
  },
  "include": ["src"]
}
```

- [x] **Step 4: Write `index.html`**

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <title>Red Panda Ridge</title>
</head>
<body>
  <div id="root"></div>
  <script type="module" src="/src/main.tsx"></script>
</body>
</html>
```

- [x] **Step 5: Write `src/render/assets.ts`**

```typescript
import { Texture } from 'pixi.js';

export const ASSET_DEFS: { name: string; chroma: boolean }[] = [
  { name: 'red-panda', chroma: true },
  { name: 'squirrel', chroma: true },
  { name: 'acorn', chroma: true },
  { name: 'pinecone', chroma: true },
  { name: 'heart', chroma: true },
  { name: 'tile', chroma: false },
  { name: 'background', chroma: false },
  { name: 'platform', chroma: false },
];

// Zeroes alpha on near-white pixels so pixel-art sprites on a white
// background composite cleanly over the game's warm palette.
export function chromaKeyToCanvas(img: HTMLImageElement): HTMLCanvasElement {
  const off = document.createElement('canvas');
  off.width = img.width;
  off.height = img.height;
  const ctx = off.getContext('2d')!;
  ctx.drawImage(img, 0, 0);
  const frame = ctx.getImageData(0, 0, off.width, off.height);
  const d = frame.data;
  for (let i = 0; i < d.length; i += 4) {
    if (d[i] >= 240 && d[i + 1] >= 240 && d[i + 2] >= 240) d[i + 3] = 0;
  }
  ctx.putImageData(frame, 0, 0);
  return off;
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = reject;
    img.src = src;
  });
}

export async function loadTextures(): Promise<Record<string, Texture>> {
  const entries = await Promise.all(
    ASSET_DEFS.map(async (def) => {
      const img = await loadImage(`assets/${def.name}.png`);
      const source = def.chroma ? chromaKeyToCanvas(img) : img;
      return [def.name, Texture.from(source)] as const;
    })
  );
  return Object.fromEntries(entries);
}
```

- [x] **Step 6: Write `src/render/PixiStage.tsx` (mount-only skeleton)**

```tsx
import { useEffect, useRef } from 'react';
import { Application, Texture } from 'pixi.js';
import { loadTextures } from './assets';

export function PixiStage() {
  const hostRef = useRef<HTMLDivElement>(null);
  const texturesRef = useRef<Record<string, Texture> | null>(null);

  useEffect(() => {
    let app: Application | null = null;
    let cancelled = false;

    (async () => {
      const application = new Application();
      await application.init({ width: 960, height: 540, background: '#F6C89F', antialias: false, roundPixels: true });
      if (cancelled) { application.destroy(true); return; }
      app = application;
      hostRef.current?.appendChild(application.canvas);
      texturesRef.current = await loadTextures();
    })();

    return () => {
      cancelled = true;
      app?.destroy(true);
    };
  }, []);

  return <div ref={hostRef} className="pixi-stage" />;
}
```

- [x] **Step 7: Write `src/App.tsx`, `src/App.css`, `src/main.tsx`**

```tsx
// src/App.tsx
import { PixiStage } from './render/PixiStage';
import './App.css';

export function App() {
  return (
    <div className="app-shell">
      <PixiStage />
    </div>
  );
}
```

```css
/* src/App.css */
html, body { margin: 0; padding: 0; background: #1b1425; }
.app-shell { position: relative; width: 960px; height: 540px; margin: 0 auto; }
.pixi-stage canvas { image-rendering: pixelated; display: block; }
```

```tsx
// src/main.tsx
import { createRoot } from 'react-dom/client';
import { App } from './App';

createRoot(document.getElementById('root')!).render(<App />);
```

- [x] **Step 8: Smoke test the scaffold boots**

Run: `bun run dev` (from `examples/platformer/`), open the printed `localhost` URL.
Expected: a 960x540 canvas mounts with no console errors; `texturesRef.current` (inspect via devtools) resolves to an 8-key object once loading finishes; opening `red-panda.png`'s texture in isolation (e.g. temporarily `app.stage.addChild(new Sprite(texturesRef.current['red-panda']))`) shows no white bounding box around the sprite, confirming chroma-key parity with v1.

- [x] **Step 9: Commit**

```bash
git add examples/platformer/package.json examples/platformer/vite.config.ts examples/platformer/tsconfig.json examples/platformer/index.html examples/platformer/src
git commit -m "feat(platformer-v2): scaffold Vite+React+Pixi app and chroma-key asset loader"
```

---

### Task 2: Engine port — physics, level, camera parity with v1

**Files:**
- Create: `examples/platformer/src/engine/constants.ts`
- Create: `examples/platformer/src/engine/types.ts`
- Create: `examples/platformer/src/engine/input.ts`
- Create: `examples/platformer/src/engine/collision.ts`
- Create: `examples/platformer/src/engine/level.ts`
- Create: `examples/platformer/src/engine/levels/level1.ts`
- Create: `examples/platformer/src/engine/player.ts`
- Create: `examples/platformer/src/engine/camera.ts`
- Create: `examples/platformer/src/engine/simulation.ts`
- Create: `examples/platformer/src/engine/store.ts`
- Create: `examples/platformer/src/render/renderBackground.ts`
- Create: `examples/platformer/src/render/renderLevel.ts`
- Create: `examples/platformer/src/render/renderAcorns.ts`
- Create: `examples/platformer/src/render/renderPlayer.ts`
- Modify: `examples/platformer/src/render/PixiStage.tsx`

**Interfaces:**
- Consumes: `loadTextures()`, `ASSET_DEFS` (Task 1).
- Produces:
  - `Rect { x: number; y: number; w: number; h: number }`, `Kinematic extends Rect { vx: number; vy: number }`, `PlayerState extends Kinematic { onGround: boolean; facing: 1 | -1; coyoteTimer: number; jumpBufferTimer: number; squashTimer: number }`, `AcornState { x: number; y: number; w: number; h: number; collected: boolean }`, `LevelData { id: number; grid: string[]; acorns: never[] /* derived, not authored */ }` in `types.ts`.
  - `InputSnapshot { left: boolean; right: boolean; jumpBuffered: boolean; restartRequested: boolean }` and `class InputCapture { snapshot(): InputSnapshot; dispose(): void }` in `input.ts` — `jumpBuffered`/`restartRequested` are one-shot: `snapshot()` returns and clears them (edge-triggered, matches v1's `!e.repeat` keydown check).
  - `function aabbOverlap(a: Rect, b: Rect): boolean` and `function resolveAxis(entity: Kinematic, axis: 'x' | 'y', level: LevelData, extraSolids: (Rect & { id: string })[] = []): string | null` in `collision.ts` — ported from v1's `resolveCollisions`; on axis `'y'` landing atop an `extraSolids` entry returns that entry's `id`, otherwise `null`. Called once per axis, exactly as v1 comments describe (never a single combined pass).
  - `function tileAt(level: LevelData, col: number, row: number): string`, `function findSpawn(level: LevelData, marker: string): { x: number; y: number }`, `function findAllSpawns(level: LevelData, marker: string): { x: number; y: number }[]` in `level.ts`.
  - `LEVEL1: LevelData` in `levels/level1.ts` — v1's 40x11 grid, copied verbatim.
  - `function updatePlayer(state: PlayerState, input: InputSnapshot, dt: number, level: LevelData, extraSolids?: (Rect & { id: string })[]): string | null` in `player.ts` — returns the platform id (if any) the player is standing on this step, for Task 4; unused/`undefined` today.
  - `function updateCamera(camera: { x: number }, player: PlayerState, worldW: number): void` in `camera.ts`.
  - `interface HudSnapshot { paused: boolean }` (grows in later tasks) and `class GameSimulation { constructor(level: LevelData); setInput(input: InputSnapshot): void; tick(rawDt: number): void; setPaused(paused: boolean): void; getRenderSnapshot(): RenderSnapshot; getHudSnapshot(): HudSnapshot; subscribe(listener: () => void): () => void }` in `simulation.ts`. `interface RenderSnapshot { player: PlayerState; acorns: AcornState[]; camera: { x: number }; squirrel: Rect; elapsed: number }`.
  - `function useHudSnapshot(sim: GameSimulation): HudSnapshot` in `store.ts`.

- [x] **Step 1: Write `src/engine/constants.ts`**

```typescript
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
```

- [x] **Step 2: Write `src/engine/types.ts`**

```typescript
export interface Rect { x: number; y: number; w: number; h: number; }
export interface Kinematic extends Rect { vx: number; vy: number; }

export interface PlayerState extends Kinematic {
  onGround: boolean;
  facing: 1 | -1;
  coyoteTimer: number;
  jumpBufferTimer: number;
  squashTimer: number;
}

export interface AcornState { x: number; y: number; w: number; h: number; collected: boolean; }

export interface LevelData {
  id: number;
  grid: string[]; // '#' solid, '.' air, 'A' acorn spawn, 'S' player spawn, 'Q' squirrel goal
}
```

- [x] **Step 3: Write `src/engine/collision.ts`**

```typescript
import type { Kinematic, LevelData, Rect } from './types';
import { TILE } from './constants';
import { tileAt } from './level';

export function aabbOverlap(a: Rect, b: Rect): boolean {
  return a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y;
}

// Resolves `entity` against solid '#' tiles (and, once Task 4 passes them,
// moving-platform rects) via AABB sweep. Call once after an X-only move and
// once after a Y-only move — resolving both axes in one combined pass
// misreads a shallow ledge-edge overlap as a wall hit even while the entity
// is deeply penetrating vertically, dropping it through the floor.
export function resolveAxis(
  entity: Kinematic,
  axis: 'x' | 'y',
  level: LevelData,
  extraSolids: (Rect & { id: string })[] = []
): string | null {
  if (axis === 'y') (entity as any).onGround = false;
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
  for (const solid of extraSolids) {
    landedOnId = resolveOne(entity, axis, solid.x, solid.y, solid.w, solid.h, landedOnId, solid.id);
  }
  return landedOnId;
}

function resolveOne(
  entity: Kinematic, axis: 'x' | 'y',
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
      (entity as any).onGround = true;
      landedOnId = solidId;
    } else {
      entity.y += overlapY;
      entity.vy = 0;
    }
  }
  return landedOnId;
}
```

- [x] **Step 4: Write `src/engine/level.ts` and `src/engine/levels/level1.ts`**

```typescript
// level.ts
import type { LevelData } from './types';
import { TILE } from './constants';

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
```

```typescript
// levels/level1.ts
import type { LevelData } from '../types';

export const LEVEL1: LevelData = {
  id: 0,
  grid: [
    '........................................',
    '........................................',
    '........................................',
    '........................................',
    '........................................',
    '........................A...............',
    '......................A.##..............',
    '.............A........##....A...........',
    '.......A....###...A.#.......#.....A.....',
    '..S.A...........................A...A.Q.',
    '#######..########..########...####.#####',
  ],
};
```

- [x] **Step 5: Write `src/engine/input.ts`**

```typescript
export interface InputSnapshot {
  left: boolean;
  right: boolean;
  jumpBuffered: boolean;
  restartRequested: boolean;
}

const JUMP_KEYS = new Set(['Space', 'ArrowUp', 'KeyW']);

export class InputCapture {
  private left = false;
  private right = false;
  private jumpBuffered = false;
  private restartRequested = false;

  private onKeyDown = (e: KeyboardEvent) => {
    if (e.code === 'ArrowLeft' || e.code === 'KeyA') this.left = true;
    if (e.code === 'ArrowRight' || e.code === 'KeyD') this.right = true;
    if (JUMP_KEYS.has(e.code) && !e.repeat) this.jumpBuffered = true;
    if (e.code === 'KeyR' && !e.repeat) this.restartRequested = true;
  };

  private onKeyUp = (e: KeyboardEvent) => {
    if (e.code === 'ArrowLeft' || e.code === 'KeyA') this.left = false;
    if (e.code === 'ArrowRight' || e.code === 'KeyD') this.right = false;
  };

  constructor() {
    window.addEventListener('keydown', this.onKeyDown);
    window.addEventListener('keyup', this.onKeyUp);
  }

  // Edge-triggered flags are consumed (reset to false) on read.
  snapshot(): InputSnapshot {
    const snap: InputSnapshot = {
      left: this.left, right: this.right,
      jumpBuffered: this.jumpBuffered, restartRequested: this.restartRequested,
    };
    this.jumpBuffered = false;
    this.restartRequested = false;
    return snap;
  }

  dispose(): void {
    window.removeEventListener('keydown', this.onKeyDown);
    window.removeEventListener('keyup', this.onKeyUp);
  }
}
```

- [x] **Step 6: Write `src/engine/player.ts`**

```typescript
import type { InputSnapshot } from './input';
import type { PlayerState, LevelData, Rect } from './types';
import { resolveAxis } from './collision';
import {
  MOVE_SPEED, COYOTE_TIME, JUMP_BUFFER, JUMP_VELOCITY, GRAVITY, MAX_FALL,
  SQUASH_DURATION, SQUASH_FALL_THRESHOLD, TILE, PLAYER_HITBOX,
} from './constants';

export function updatePlayer(
  state: PlayerState, input: InputSnapshot, dt: number, level: LevelData,
  extraSolids: (Rect & { id: string })[] = []
): string | null {
  state.vx = input.left ? -MOVE_SPEED : input.right ? MOVE_SPEED : 0;
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
  resolveAxis(state, 'x', level, extraSolids);
  state.y += fallSpeed * dt;
  const landedOnId = resolveAxis(state, 'y', level, extraSolids);

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
    coyoteTimer: 0, jumpBufferTimer: 0, squashTimer: 0,
  };
}
```

- [x] **Step 7: Write `src/engine/camera.ts`**

```typescript
import type { PlayerState } from './types';
import { CANVAS_W } from './constants';

export function updateCamera(camera: { x: number }, player: PlayerState, worldW: number): void {
  const target = player.x + player.w / 2 - CANVAS_W / 2;
  camera.x = Math.max(0, Math.min(target, worldW - CANVAS_W));
}
```

- [x] **Step 8: Write `src/engine/simulation.ts`**

```typescript
import type { AcornState, PlayerState, LevelData } from './types';
import type { InputSnapshot } from './input';
import { updatePlayer, spawnPlayer } from './player';
import { updateCamera } from './camera';
import { aabbOverlap } from './collision';
import { findSpawn, findAllSpawns } from './level';
import { ACORN_HITBOX, TILE, FIXED_DT, MAX_DT } from './constants';

export interface HudSnapshot { paused: boolean; }
export interface RenderSnapshot {
  player: PlayerState;
  acorns: AcornState[];
  camera: { x: number };
  squirrel: { x: number; y: number; w: number; h: number };
  elapsed: number;
}

export class GameSimulation {
  private level: LevelData;
  private player: PlayerState;
  private acorns: AcornState[];
  private squirrel: { x: number; y: number; w: number; h: number };
  private camera = { x: 0 };
  private elapsed = 0;
  private accumulator = 0;
  private pendingInput: InputSnapshot = { left: false, right: false, jumpBuffered: false, restartRequested: false };
  private paused = false;
  private listeners = new Set<() => void>();
  private hudCache: HudSnapshot;

  constructor(level: LevelData) {
    this.level = level;
    const spawn = findSpawn(level, 'S');
    this.player = spawnPlayer(spawn);
    this.acorns = findAllSpawns(level, 'A').map((pos) => ({ x: pos.x, y: pos.y, w: TILE, h: TILE, collected: false }));
    const goal = findSpawn(level, 'Q');
    this.squirrel = { x: goal.x, y: goal.y, w: TILE, h: TILE };
    this.hudCache = this.computeHud();
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

  tick(rawDt: number): void {
    if (this.paused) return;
    this.accumulator += Math.min(rawDt, MAX_DT);
    let firstStep = true;
    while (this.accumulator >= FIXED_DT) {
      const stepInput = firstStep ? this.pendingInput : { ...this.pendingInput, jumpBuffered: false, restartRequested: false };
      this.stepOnce(FIXED_DT, stepInput);
      this.accumulator -= FIXED_DT;
      firstStep = false;
    }
    this.pendingInput = { ...this.pendingInput, jumpBuffered: false, restartRequested: false };
    this.notify();
  }

  private stepOnce(dt: number, input: InputSnapshot): void {
    this.elapsed += dt;
    const worldW = this.level.grid[0].length * TILE;
    const result = updatePlayer(this.player, input, dt, this.level);
    if (result === 'RESPAWN') {
      const spawn = findSpawn(this.level, 'S');
      const fresh = spawnPlayer(spawn);
      Object.assign(this.player, fresh);
    }
    for (const acorn of this.acorns) {
      if (!acorn.collected && aabbOverlap(this.player, acorn)) acorn.collected = true;
    }
    updateCamera(this.camera, this.player, worldW);
  }

  private computeHud(): HudSnapshot {
    return { paused: this.paused };
  }

  getHudSnapshot(): HudSnapshot {
    const next = this.computeHud();
    const changed = (Object.keys(next) as (keyof HudSnapshot)[]).some((k) => next[k] !== this.hudCache[k]);
    if (changed) this.hudCache = next;
    return this.hudCache;
  }

  getRenderSnapshot(): RenderSnapshot {
    return { player: this.player, acorns: this.acorns, camera: this.camera, squirrel: this.squirrel, elapsed: this.elapsed };
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private notify(): void { this.listeners.forEach((l) => l()); }
}
```

- [x] **Step 9: Write `src/engine/store.ts`**

```typescript
import { useSyncExternalStore } from 'react';
import type { GameSimulation, HudSnapshot } from './simulation';

export function useHudSnapshot(sim: GameSimulation): HudSnapshot {
  return useSyncExternalStore(
    (listener) => sim.subscribe(listener),
    () => sim.getHudSnapshot()
  );
}
```

- [x] **Step 10: Write the render helpers (`renderBackground.ts`, `renderLevel.ts`, `renderAcorns.ts`, `renderPlayer.ts`)**

```typescript
// render/renderBackground.ts
import { Container, Sprite, Texture, Graphics } from 'pixi.js';
import { CANVAS_W, CANVAS_H, BG_SCALE, BG_PARALLAX, HILLS_PARALLAX } from '../engine/constants';

export function createBackgroundLayer(textures: Record<string, Texture>): { view: Container; update: (cameraX: number) => void } {
  const view = new Container();
  const sky = new Sprite(Texture.WHITE);
  sky.tint = 0xF6C89F; sky.width = CANVAS_W; sky.height = CANVAS_H;
  const bg = new Sprite(textures['background']);
  bg.width = textures['background'].width * BG_SCALE;
  bg.height = textures['background'].height * BG_SCALE;
  bg.y = -(bg.height - CANVAS_H) * 0.35;
  const hills = new Graphics();
  view.addChild(sky, bg, hills);
  const panMax = bg.width - CANVAS_W;

  function update(cameraX: number) {
    bg.x = -Math.max(0, Math.min(panMax, cameraX * BG_PARALLAX));
    const hillsOffset = cameraX * HILLS_PARALLAX;
    hills.clear();
    hills.moveTo(0, CANVAS_H);
    const step = 40;
    for (let x = -step; x <= CANVAS_W + step; x += step) {
      const worldX = x + hillsOffset;
      hills.lineTo(x, 380 + Math.sin(worldX * 0.01) * 18);
    }
    hills.lineTo(CANVAS_W, CANVAS_H);
    hills.closePath();
    hills.fill({ color: 0xE8A15C, alpha: 0.6 });
  }

  return { view, update };
}
```

```typescript
// render/renderLevel.ts
import { Container, TilingSprite, Texture } from 'pixi.js';
import type { LevelData } from '../engine/types';
import { TILE } from '../engine/constants';
import { tileAt } from '../engine/level';

export function createLevelLayer(level: LevelData, textures: Record<string, Texture>): { view: Container; update: (cameraX: number) => void } {
  const view = new Container();
  const tiles: { col: number; row: number; sprite: TilingSprite }[] = [];
  for (let row = 0; row < level.grid.length; row++) {
    for (let col = 0; col < level.grid[row].length; col++) {
      if (tileAt(level, col, row) !== '#') continue;
      const sprite = new TilingSprite({ texture: textures['tile'], width: TILE, height: TILE });
      sprite.y = row * TILE;
      view.addChild(sprite);
      tiles.push({ col, row, sprite });
    }
  }
  function update(cameraX: number) {
    for (const t of tiles) t.sprite.x = t.col * TILE - cameraX;
  }
  return { view, update };
}
```

```typescript
// render/renderAcorns.ts
import { Container, Sprite, Texture } from 'pixi.js';
import type { AcornState } from '../engine/types';
import { ACORN_DRAW } from '../engine/constants';

export function createAcornLayer(acorns: AcornState[], textures: Record<string, Texture>): { view: Container; update: (cameraX: number, elapsed: number) => void } {
  const view = new Container();
  const sprites = acorns.map(() => {
    const s = new Sprite(textures['acorn']);
    s.anchor.set(0.5); s.width = ACORN_DRAW; s.height = ACORN_DRAW;
    view.addChild(s);
    return s;
  });
  function update(cameraX: number, elapsed: number) {
    acorns.forEach((acorn, i) => {
      const sprite = sprites[i];
      sprite.visible = !acorn.collected;
      const bob = Math.sin(elapsed * 3 + acorn.x * 0.05) * 6;
      sprite.x = acorn.x + acorn.w / 2 - cameraX;
      sprite.y = acorn.y + acorn.h / 2 + bob;
    });
  }
  return { view, update };
}
```

```typescript
// render/renderPlayer.ts
import { Sprite, Texture } from 'pixi.js';
import type { PlayerState } from '../engine/types';
import { PLAYER_DRAW, SQUASH_DURATION } from '../engine/constants';

function lerp(a: number, b: number, t: number) { return a + (b - a) * t; }

export function createPlayerSprite(textures: Record<string, Texture>): Sprite {
  const sprite = new Sprite(textures['red-panda']);
  sprite.anchor.set(0.5, 1);
  sprite.width = PLAYER_DRAW.w; sprite.height = PLAYER_DRAW.h;
  return sprite;
}

export function updatePlayerSprite(sprite: Sprite, player: PlayerState, cameraX: number): void {
  const t = 1 - player.squashTimer / SQUASH_DURATION;
  const eased = 1 - Math.pow(1 - t, 3);
  const scaleX = player.squashTimer > 0 ? lerp(1.15, 1, eased) : 1;
  const scaleY = player.squashTimer > 0 ? lerp(0.85, 1, eased) : 1;
  sprite.x = player.x + player.w / 2 - cameraX;
  sprite.y = player.y + player.h;
  sprite.scale.set(player.facing * scaleX, scaleY);
}
```

- [x] **Step 11: Wire everything into `PixiStage.tsx`**

Replace the Task 1 skeleton's `useEffect` body: after `loadTextures()` resolves, construct `new GameSimulation(LEVEL1)` and an `InputCapture`, build the background/level/acorn layers plus one player `Sprite`, add them to `app.stage` in back-to-front order (background, level, acorns, squirrel sprite, player sprite), and register `app.ticker.add((ticker) => { sim.setInput(input.snapshot()); sim.tick(ticker.deltaMS / 1000); const snap = sim.getRenderSnapshot(); levelLayer.update(snap.camera.x); acornLayer.update(snap.camera.x, snap.elapsed); bgLayer.update(snap.camera.x); updatePlayerSprite(playerSprite, snap.player, snap.camera.x); })`. Dispose the `InputCapture` alongside `app.destroy(true)` in the cleanup function.

- [x] **Step 12: Smoke test parity against v1**

Run: `bun run dev`, open the app alongside the original `examples/platformer/index.html` (e.g. `python3 -m http.server 8765 --directory examples/platformer`) side by side.
Expected: movement speed, jump height/arc, coyote time, jump buffering (press jump just before landing), squash-on-land, acorn bob, parallax pan, world-edge walls, and pit respawn (walk off the right edge past `WORLD_W`) all feel identical between the two builds; devtools console shows no errors in the v2 build.

- [x] **Step 13: Commit**

```bash
git add examples/platformer/src
git commit -m "feat(platformer-v2): port physics/level/camera engine with fixed-timestep simulation"
```

---

### Task 3: Pinecone enemies, hearts, and game over

**Files:**
- Create: `examples/platformer/src/engine/enemies.ts`
- Create: `examples/platformer/src/render/renderEnemies.ts`
- Modify: `examples/platformer/src/engine/types.ts` (add `EnemyState`)
- Modify: `examples/platformer/src/engine/constants.ts` (add hearts/enemy constants)
- Modify: `examples/platformer/src/engine/levels/level1.ts` (add one pinecone patrol spawn)
- Modify: `examples/platformer/src/engine/simulation.ts` (integrate enemies, hearts, invulnerability, game over)
- Modify: `examples/platformer/src/render/PixiStage.tsx` (render enemies)

**Interfaces:**
- Consumes: `PlayerState`, `LevelData`, `aabbOverlap`, `GameSimulation`, `RenderSnapshot`, `HudSnapshot` (Task 2).
- Produces:
  - `constants.ts` additions: `MAX_HEARTS = 3`, `HIT_INVULN_DURATION = 1.0` (s), `PINECONE_SPEED = 80` (px/s), `PINECONE_HITBOX = { w: 36, h: 32 }`, `PINECONE_DRAW = { w: 56, h: 52 }`.
  - `EnemyState extends Kinematic { id: string; alive: boolean; patrolMinX: number; patrolMaxX: number }` and `PlayerState` gains `invulnTimer: number` in `types.ts`.
  - `interface EnemySpawn { x: number; y: number; patrolMinX: number; patrolMaxX: number }` and `LevelData` gains `enemies: EnemySpawn[]` in `types.ts`.
  - `function spawnEnemies(defs: EnemySpawn[]): EnemyState[]` and `function updateEnemies(enemies: EnemyState[], dt: number): void` in `enemies.ts` — each enemy walks at `PINECONE_SPEED` toward `patrolMaxX`, reverses at either bound.
  - `function checkPlayerHit(player: PlayerState, enemies: EnemyState[]): boolean` in `enemies.ts` — returns `true` (and does not mutate hearts itself) the first tick a live enemy overlaps the player while `player.invulnTimer <= 0`.
  - `GameSimulation`'s `HudSnapshot` gains `hearts: number; maxHearts: number; gameOver: boolean`. `RenderSnapshot` gains `enemies: EnemyState[]`.

- [x] **Step 1: Add constants**

```typescript
// append to constants.ts
export const MAX_HEARTS = 3;
export const HIT_INVULN_DURATION = 1.0;
export const PINECONE_SPEED = 80;
export const PINECONE_HITBOX = { w: 36, h: 32 };
export const PINECONE_DRAW = { w: 56, h: 52 };
```

- [x] **Step 2: Extend `types.ts`**

```typescript
// add to types.ts
export interface EnemyState extends Kinematic {
  id: string;
  alive: boolean;
  patrolMinX: number;
  patrolMaxX: number;
}

export interface EnemySpawn { x: number; y: number; patrolMinX: number; patrolMaxX: number; }

// PlayerState gains:
//   invulnTimer: number;
// LevelData gains:
//   enemies: EnemySpawn[];
```

- [x] **Step 3: Add a pinecone to `level1.ts`**

```typescript
// levels/level1.ts — add alongside `grid`:
enemies: [
  { x: 528, y: 336, patrolMinX: 480, patrolMaxX: 720 }, // patrols the ledge near cols 10-15, row 6-7
],
```

- [x] **Step 4: Write `src/engine/enemies.ts`**

```typescript
import type { EnemyState, EnemySpawn, PlayerState } from './types';
import { aabbOverlap } from './collision';
import { PINECONE_SPEED, PINECONE_HITBOX } from './constants';

export function spawnEnemies(defs: EnemySpawn[]): EnemyState[] {
  return defs.map((def, i) => ({
    id: `pinecone-${i}`,
    x: def.x, y: def.y, w: PINECONE_HITBOX.w, h: PINECONE_HITBOX.h,
    vx: PINECONE_SPEED, vy: 0,
    alive: true,
    patrolMinX: def.patrolMinX, patrolMaxX: def.patrolMaxX,
  }));
}

export function updateEnemies(enemies: EnemyState[], dt: number): void {
  for (const enemy of enemies) {
    if (!enemy.alive) continue;
    enemy.x += enemy.vx * dt;
    if (enemy.x <= enemy.patrolMinX) { enemy.x = enemy.patrolMinX; enemy.vx = Math.abs(enemy.vx); }
    else if (enemy.x + enemy.w >= enemy.patrolMaxX) { enemy.x = enemy.patrolMaxX - enemy.w; enemy.vx = -Math.abs(enemy.vx); }
  }
}

export function checkPlayerHit(player: PlayerState, enemies: EnemyState[]): boolean {
  if (player.invulnTimer > 0) return false;
  return enemies.some((e) => e.alive && aabbOverlap(player, e));
}
```

- [x] **Step 5: Integrate into `simulation.ts`**

Modify `spawnPlayer` usage sites to also set `invulnTimer: 0` (update `player.ts`'s `spawnPlayer` return object to include it). In `simulation.ts`: add `private enemies: EnemyState[]`, `private hearts = MAX_HEARTS`, `private gameOver = false` fields, initialize `this.enemies = spawnEnemies(level.enemies)` in the constructor. In `stepOnce(dt, input)`, after the existing player/acorn/camera updates and before returning: guard the whole body with `if (this.gameOver) return;`, call `updateEnemies(this.enemies, dt)`, decrement `this.player.invulnTimer = Math.max(0, this.player.invulnTimer - dt)`, then `if (checkPlayerHit(this.player, this.enemies)) { this.hearts -= 1; this.player.invulnTimer = HIT_INVULN_DURATION; if (this.hearts <= 0) this.gameOver = true; }`. Extend `computeHud()` to return `{ paused: this.paused, hearts: this.hearts, maxHearts: MAX_HEARTS, gameOver: this.gameOver }` and `getRenderSnapshot()` to include `enemies: this.enemies`.

- [x] **Step 6: Write `src/render/renderEnemies.ts`**

```typescript
import { Container, Sprite, Texture } from 'pixi.js';
import type { EnemyState } from '../engine/types';
import { PINECONE_DRAW } from '../engine/constants';

export function createEnemyLayer(enemies: EnemyState[], textures: Record<string, Texture>): { view: Container; update: (cameraX: number) => void } {
  const view = new Container();
  const sprites = enemies.map(() => {
    const s = new Sprite(textures['pinecone']);
    s.anchor.set(0.5, 1); s.width = PINECONE_DRAW.w; s.height = PINECONE_DRAW.h;
    view.addChild(s);
    return s;
  });
  function update(cameraX: number) {
    enemies.forEach((enemy, i) => {
      const sprite = sprites[i];
      sprite.visible = enemy.alive;
      sprite.x = enemy.x + enemy.w / 2 - cameraX;
      sprite.y = enemy.y + enemy.h;
      sprite.scale.x = Math.sign(enemy.vx) * PINECONE_DRAW.w / textures['pinecone'].width * Math.abs(sprite.scale.x) || sprite.scale.x;
    });
  }
  return { view, update };
}
```

- [x] **Step 7: Wire the enemy layer into `PixiStage.tsx`**

Add `createEnemyLayer(sim.getRenderSnapshot().enemies, textures)` to the stage's child list (between the level and acorn layers) and call its `update(snap.camera.x)` inside the ticker callback alongside the other layer updates.

- [x] **Step 8: Smoke test contact damage and game over**

Run: `bun run dev`, walk the player into the pinecone.
Expected: first contact drops hearts from 3 to 2 (inspect via `sim.getHudSnapshot().hearts` in devtools) and the player is untouchable for about a second (repeated overlap during that window does not decrement further); after the invuln window a second contact drops to 1, a third to 0; at 0 hearts `sim.getHudSnapshot().gameOver === true` and the simulation stops advancing (player stops responding to input).

- [x] **Step 9: Commit**

```bash
git add examples/platformer/src
git commit -m "feat(platformer-v2): add pinecone patrol enemies, hearts, and game over"
```

---

### Task 4: Moving platforms, 3-level data, and progression

**Files:**
- Create: `examples/platformer/src/engine/platforms.ts`
- Create: `examples/platformer/src/engine/levels/level2.ts`
- Create: `examples/platformer/src/engine/levels/level3.ts`
- Create: `examples/platformer/src/render/renderPlatforms.ts`
- Modify: `examples/platformer/src/engine/types.ts` (add `MovingPlatformState`, `PlatformSpawn`, `LevelData.platforms`)
- Modify: `examples/platformer/src/engine/level.ts` (add `LEVELS: LevelData[]`)
- Modify: `examples/platformer/src/engine/player.ts` (carry the player on a moving platform)
- Modify: `examples/platformer/src/engine/simulation.ts` (multi-level state, level-complete/progression)
- Modify: `examples/platformer/src/render/PixiStage.tsx` (render platforms; react to level switches)

**Interfaces:**
- Consumes: `resolveAxis`'s `extraSolids` parameter, `updatePlayer`'s `extraSolids`/return value, `GameSimulation`, `HudSnapshot`, `RenderSnapshot` (Tasks 2-3).
- Produces:
  - `MovingPlatformState extends Rect { id: string; axis: 'x' | 'y'; min: number; max: number; speed: number; dir: 1 | -1 }` and `PlatformSpawn { x: number; y: number; w: number; h: number; axis: 'x' | 'y'; min: number; max: number; speed: number }` in `types.ts`; `LevelData` gains `platforms: PlatformSpawn[]`.
  - `function spawnPlatforms(defs: PlatformSpawn[]): MovingPlatformState[]` and `function updateMovingPlatforms(platforms: MovingPlatformState[], dt: number): { id: string; dx: number; dy: number }[]` in `platforms.ts` — moves each platform along its axis between `min`/`max`, reversing `dir` at the bounds, and returns this tick's per-platform delta (consumed by `player.ts` for carrying).
  - `LEVELS: LevelData[]` (3 entries, `id` 0/1/2) exported from `level.ts`; `LEVEL2`, `LEVEL3` exported from their own files.
  - `updatePlayer`'s existing `extraSolids` param is now populated (`platforms.map(p => ({...p}))`) by `simulation.ts`; when the returned landed-on id matches a platform, `player.ts` applies that platform's `dx`/`dy` (from `updateMovingPlatforms`'s return) to the player's position **before** that tick's own input-driven move, so the carry is exactly one tick's platform motion and stepping off the platform's rect next tick (no overlap → `resolveAxis` returns `null`) drops the player with ordinary gravity — no warp, no clip.
  - `GameSimulation` gains `goToLevel(index: number): void`, `restartLevel(): void`; `HudSnapshot` gains `levelIndex: number; levelComplete: boolean; unlockedLevels: number[]; acorns: number; acornsTotal: number; elapsed: number; levelResult: { levelIndex: number; acorns: number; acornsTotal: number; timeSeconds: number } | null`; `RenderSnapshot` gains `platforms: MovingPlatformState[]`.

- [x] **Step 1: Extend `types.ts`**

```typescript
// add to types.ts
export interface MovingPlatformState extends Rect {
  id: string;
  axis: 'x' | 'y';
  min: number;
  max: number;
  speed: number;
  dir: 1 | -1;
}

export interface PlatformSpawn {
  x: number; y: number; w: number; h: number;
  axis: 'x' | 'y'; min: number; max: number; speed: number;
}

// LevelData gains:
//   platforms: PlatformSpawn[];
```

- [x] **Step 2: Write `src/engine/platforms.ts`**

```typescript
import type { MovingPlatformState, PlatformSpawn } from './types';

export function spawnPlatforms(defs: PlatformSpawn[]): MovingPlatformState[] {
  return defs.map((def, i) => ({
    id: `platform-${i}`,
    x: def.x, y: def.y, w: def.w, h: def.h,
    axis: def.axis, min: def.min, max: def.max, speed: def.speed, dir: 1,
  }));
}

export function updateMovingPlatforms(platforms: MovingPlatformState[], dt: number): { id: string; dx: number; dy: number }[] {
  return platforms.map((p) => {
    const delta = p.speed * p.dir * dt;
    const before = p.axis === 'x' ? p.x : p.y;
    let next = before + delta;
    if (next <= p.min) { next = p.min; p.dir = 1; }
    else if (next >= p.max) { next = p.max; p.dir = -1; }
    const actualDelta = next - before;
    if (p.axis === 'x') p.x = next; else p.y = next;
    return { id: p.id, dx: p.axis === 'x' ? actualDelta : 0, dy: p.axis === 'y' ? actualDelta : 0 };
  });
}
```

- [x] **Step 3: Write `levels/level2.ts` and `levels/level3.ts`**

```typescript
// levels/level2.ts — a pit crossed by a horizontal platform, a vertical
// platform to a high acorn alcove, and two pinecone ledges.
import type { LevelData } from '../types';

export const LEVEL2: LevelData = {
  id: 1,
  grid: [
    '........................................',
    '........................................',
    '..................A......A..............',
    '........................................',
    '......########............########......',
    '........................................',
    '........................................',
    '.......A......................A.........',
    '..S..................................Q..',
    '########........................########',
    '########........................########',
  ],
  enemies: [
    { x: 288, y: 160, patrolMinX: 288, patrolMaxX: 588 },   // ledge A, cols 6-13
    { x: 1248, y: 160, patrolMinX: 1248, patrolMaxX: 1548 }, // ledge B, cols 26-33
  ],
  platforms: [
    { x: 432, y: 432, w: 96, h: 48, axis: 'x', min: 432, max: 1344, speed: 90 },  // bridges the floor pit
    { x: 1008, y: 380, w: 96, h: 48, axis: 'y', min: 96, max: 380, speed: 70 },   // lifts to the row-2 acorn alcove
  ],
};
```

```typescript
// levels/level3.ts — three floor islands separated by two pits (each
// crossed by its own platform) plus three patrol ledges.
import type { LevelData } from '../types';

export const LEVEL3: LevelData = {
  id: 2,
  grid: [
    '........................................',
    '....................A...................',
    '........................................',
    '....######....................######....',
    '........................................',
    '................########................',
    '.........A.........A.........A..........',
    '........................................',
    '..S...................................Q.',
    '######......####........####......######',
    '######......####........####......######',
  ],
  enemies: [
    { x: 192, y: 112, patrolMinX: 192, patrolMaxX: 432 },    // ledge near spawn, cols 4-9
    { x: 768, y: 208, patrolMinX: 768, patrolMaxX: 1104 },   // mid ledge, cols 16-23
    { x: 1440, y: 112, patrolMinX: 1440, patrolMaxX: 1680 }, // ledge near goal, cols 30-35
  ],
  platforms: [
    { x: 288, y: 432, w: 96, h: 48, axis: 'x', min: 288, max: 480, speed: 90 },   // pit 1, cols 6-11
    { x: 768, y: 432, w: 96, h: 48, axis: 'x', min: 768, max: 1056, speed: 110 }, // pit 2, cols 16-23
  ],
};
```

- [x] **Step 4: Export `LEVELS` from `level.ts`**

```typescript
// add to level.ts
import { LEVEL1 } from './levels/level1';
import { LEVEL2 } from './levels/level2';
import { LEVEL3 } from './levels/level3';

export const LEVELS: LevelData[] = [LEVEL1, LEVEL2, LEVEL3];
```

- [x] **Step 5: Carry the player in `player.ts`**

```typescript
// player.ts — updatePlayer signature and body gain a carry step. Add a
// `pendingCarry: { dx: number; dy: number } | null` parameter (the delta
// returned by this level's `updateMovingPlatforms` call for the platform
// the player landed on *last* tick), applied once before the player's own
// input-driven move:
export function updatePlayer(
  state: PlayerState, input: InputSnapshot, dt: number, level: LevelData,
  extraSolids: (Rect & { id: string })[] = [],
  pendingCarry: { dx: number; dy: number } | null = null
): string | null {
  if (pendingCarry) { state.x += pendingCarry.dx; state.y += pendingCarry.dy; }
  // ...unchanged body from Task 2 below this point...
}
```

- [x] **Step 6: Integrate multi-level state and progression into `simulation.ts`**

Add fields: `private platforms: MovingPlatformState[]`, `private levelIndex: number`, `private unlockedLevels: number[] = [0]`, `private levelComplete = false`, `private levelResult: HudSnapshot['levelResult'] = null`, `private standingPlatformId: string | null = null`. Change the constructor to `constructor(private levels: LevelData[], startIndex = 0)` and factor level setup (spawning player/acorns/enemies/platforms, resetting `elapsed`/`hearts`/`gameOver`/`levelComplete`) into a private `loadLevel(index: number)` called from the constructor and from the new public methods:

```typescript
goToLevel(index: number): void {
  if (!this.unlockedLevels.includes(index)) return;
  this.levelIndex = index;
  this.loadLevel(index);
}

restartLevel(): void { this.loadLevel(this.levelIndex); }
```

In `stepOnce`, before `updatePlayer`: `const platformDeltas = updateMovingPlatforms(this.platforms, dt); const carry = this.standingPlatformId ? platformDeltas.find(d => d.id === this.standingPlatformId) ?? null : null;`. Call `updatePlayer(this.player, input, dt, this.level, this.platforms, carry ? { dx: carry.dx, dy: carry.dy } : null)` and store its return in `this.standingPlatformId` (only when it's a platform id, i.e. present in `this.platforms`; otherwise `null`). After the existing squirrel-overlap check, replace the old placeholder win logic with: `if (!this.levelComplete && aabbOverlap(this.player, this.squirrel)) { this.levelComplete = true; const acornsCollected = this.acorns.filter(a => a.collected).length; this.levelResult = { levelIndex: this.levelIndex, acorns: acornsCollected, acornsTotal: this.acorns.length, timeSeconds: this.elapsed }; if (this.levelIndex + 1 < this.levels.length && !this.unlockedLevels.includes(this.levelIndex + 1)) this.unlockedLevels = [...this.unlockedLevels, this.levelIndex + 1]; }`. Extend `computeHud()` and `getRenderSnapshot()` with the new fields listed in this task's Interfaces block.

- [x] **Step 7: Write `src/render/renderPlatforms.ts`**

```typescript
import { Container, TilingSprite, Texture } from 'pixi.js';
import type { MovingPlatformState } from '../engine/types';

export function createPlatformLayer(platforms: MovingPlatformState[], textures: Record<string, Texture>): { view: Container; update: (cameraX: number) => void } {
  const view = new Container();
  const sprites = platforms.map((p) => {
    const s = new TilingSprite({ texture: textures['platform'], width: p.w, height: p.h });
    view.addChild(s);
    return s;
  });
  function update(cameraX: number) {
    platforms.forEach((p, i) => { sprites[i].x = p.x - cameraX; sprites[i].y = p.y; });
  }
  return { view, update };
}
```

- [x] **Step 8: Wire platform rendering and `LEVELS` into `PixiStage.tsx`**

Construct `new GameSimulation(LEVELS, 0)` (replacing the Task 2 `new GameSimulation(LEVEL1)` call), add `createPlatformLayer(sim.getRenderSnapshot().platforms, textures)` to the stage's children, and call its `update(snap.camera.x)` in the ticker. Because `loadLevel` swaps arrays by reference, rebuild all per-entity layers (acorns/enemies/platforms) whenever `snap.levelIndex` differs from the previously seen value inside the ticker callback.

- [x] **Step 9: Smoke test carry, edge-drop, and progression**

Run: `bun run dev`, navigate to level 2 (temporarily call `sim.goToLevel(1)` from devtools since the menu doesn't exist until Task 5).
Expected: standing on the horizontal platform while it crosses the pit keeps the player's x moving with it without sinking through the floor at either end; walking to the edge of the platform mid-crossing and continuing past its width causes the player to fall straight down (no horizontal warp, no clipping into the pit walls) exactly as if stepping off a static ledge. Reaching the squirrel in level 1 sets `sim.getHudSnapshot().levelComplete === true` and `unlockedLevels` becomes `[0, 1]`.

- [x] **Step 10: Commit**

```bash
git add examples/platformer/src
git commit -m "feat(platformer-v2): add moving platforms, 3-level data, and unlock progression"
```

---

### Task 5: React HUD, menus, and localStorage records

**Files:**
- Create: `examples/platformer/src/engine/records.ts`
- Create: `examples/platformer/src/hud/HUD.tsx`
- Create: `examples/platformer/src/hud/PauseMenu.tsx`
- Create: `examples/platformer/src/hud/GameOverScreen.tsx`
- Create: `examples/platformer/src/hud/LevelCompleteScreen.tsx`
- Create: `examples/platformer/src/hud/LevelSelectMenu.tsx`
- Modify: `examples/platformer/src/App.tsx`
- Modify: `examples/platformer/src/App.css`
- Modify: `examples/platformer/src/render/PixiStage.tsx` (accept a `sim`/`paused` prop instead of owning the simulation)

**Interfaces:**
- Consumes: `GameSimulation` (constructor, `setPaused`, `restartLevel`, `goToLevel`), `useHudSnapshot`, `HudSnapshot` (all fields), `LEVELS` (Tasks 2-4).
- Produces:
  - `interface LevelRecord { bestAcorns: number; bestTimeSeconds: number }`, `function loadRecord(levelId: number): LevelRecord | null`, `function saveRecordIfBetter(levelId: number, result: { acorns: number; timeSeconds: number }): LevelRecord`, `function loadUnlockedLevels(): number[]`, `function saveUnlockedLevels(levels: number[]): void` in `records.ts`. localStorage keys: `red-panda-ridge:record:<levelId>`, `red-panda-ridge:unlocked`.
  - `type Screen = 'menu' | 'playing' | 'gameover' | 'levelComplete'` and the top-level `<App/>` owning a single `useRef<GameSimulation>` plus `screen`/`paused` React state, created once via `useMemo(() => new GameSimulation(LEVELS, 0), [])`.
  - `<HUD sim={sim} />`, `<PauseMenu onResume={...} onRestart={...} onQuit={...} />`, `<GameOverScreen onReturnToMenu={...} />`, `<LevelCompleteScreen result={...} record={...} hasNext={...} onNext={...} onMenu={...} />`, `<LevelSelectMenu unlockedLevels={...} onSelect={(index: number) => void} />`.

- [x] **Step 1: Write `src/engine/records.ts`**

```typescript
import type { LevelRecord } from './types';

const RECORD_KEY = (levelId: number) => `red-panda-ridge:record:${levelId}`;
const UNLOCKED_KEY = 'red-panda-ridge:unlocked';

export function loadRecord(levelId: number): LevelRecord | null {
  const raw = localStorage.getItem(RECORD_KEY(levelId));
  return raw ? (JSON.parse(raw) as LevelRecord) : null;
}

export function saveRecordIfBetter(levelId: number, result: { acorns: number; timeSeconds: number }): LevelRecord {
  const current = loadRecord(levelId);
  const isBetter = !current || result.acorns > current.bestAcorns
    || (result.acorns === current.bestAcorns && result.timeSeconds < current.bestTimeSeconds);
  const next: LevelRecord = isBetter
    ? { bestAcorns: result.acorns, bestTimeSeconds: result.timeSeconds }
    : current;
  localStorage.setItem(RECORD_KEY(levelId), JSON.stringify(next));
  return next;
}

export function loadUnlockedLevels(): number[] {
  const raw = localStorage.getItem(UNLOCKED_KEY);
  return raw ? (JSON.parse(raw) as number[]) : [0];
}

export function saveUnlockedLevels(levels: number[]): void {
  localStorage.setItem(UNLOCKED_KEY, JSON.stringify(levels));
}
```

Add `LevelRecord` to `types.ts`: `export interface LevelRecord { bestAcorns: number; bestTimeSeconds: number; }`.

- [x] **Step 2: Write `src/hud/HUD.tsx`**

```tsx
import { useHudSnapshot } from '../engine/store';
import type { GameSimulation } from '../engine/simulation';

export function HUD({ sim }: { sim: GameSimulation }) {
  const snap = useHudSnapshot(sim);
  return (
    <div className="hud">
      <div className="hud-hearts">
        {Array.from({ length: snap.maxHearts }, (_, i) => (
          <img key={i} src="assets/heart.png" className={i < snap.hearts ? 'heart-full' : 'heart-empty'} alt="" />
        ))}
      </div>
      <div className="hud-acorns">Acorns: {snap.acorns} / {snap.acornsTotal}</div>
      <div className="hud-level">Level {snap.levelIndex + 1}</div>
    </div>
  );
}
```

- [x] **Step 3: Write `src/hud/PauseMenu.tsx` and `src/hud/GameOverScreen.tsx`**

```tsx
// hud/PauseMenu.tsx
export function PauseMenu({ onResume, onRestart, onQuit }: { onResume: () => void; onRestart: () => void; onQuit: () => void }) {
  return (
    <div className="overlay pause-menu">
      <h2>Paused</h2>
      <button onClick={onResume}>Resume</button>
      <button onClick={onRestart}>Restart Level</button>
      <button onClick={onQuit}>Quit to Menu</button>
    </div>
  );
}
```

```tsx
// hud/GameOverScreen.tsx
export function GameOverScreen({ onReturnToMenu }: { onReturnToMenu: () => void }) {
  return (
    <div className="overlay game-over">
      <h2>Game Over</h2>
      <button onClick={onReturnToMenu}>Return to Menu</button>
    </div>
  );
}
```

- [x] **Step 4: Write `src/hud/LevelCompleteScreen.tsx`**

```tsx
import type { LevelRecord } from '../engine/types';

export function LevelCompleteScreen({
  result, record, hasNext, onNext, onMenu,
}: {
  result: { acorns: number; acornsTotal: number; timeSeconds: number };
  record: LevelRecord;
  hasNext: boolean;
  onNext: () => void;
  onMenu: () => void;
}) {
  return (
    <div className="overlay level-complete">
      <h2>Level Complete!</h2>
      <p>Acorns: {result.acorns} / {result.acornsTotal} (best: {record.bestAcorns})</p>
      <p>Time: {result.timeSeconds.toFixed(1)}s (best: {record.bestTimeSeconds.toFixed(1)}s)</p>
      {hasNext && <button onClick={onNext}>Next Level</button>}
      <button onClick={onMenu}>Level Select</button>
    </div>
  );
}
```

- [x] **Step 5: Write `src/hud/LevelSelectMenu.tsx`**

```tsx
import { LEVELS } from '../engine/level';
import { loadRecord } from '../engine/records';

export function LevelSelectMenu({ unlockedLevels, onSelect }: { unlockedLevels: number[]; onSelect: (index: number) => void }) {
  return (
    <div className="overlay level-select">
      <h1>Red Panda Ridge</h1>
      <div className="level-tiles">
        {LEVELS.map((level, i) => {
          const unlocked = unlockedLevels.includes(i);
          const record = loadRecord(level.id);
          return (
            <button key={level.id} disabled={!unlocked} onClick={() => onSelect(i)}>
              Level {i + 1}
              {unlocked && record && <span> — best {record.bestAcorns} acorns / {record.bestTimeSeconds.toFixed(1)}s</span>}
              {!unlocked && <span> (locked)</span>}
            </button>
          );
        })}
      </div>
    </div>
  );
}
```

- [x] **Step 6: Rewrite `src/App.tsx` as the screen state machine**

```tsx
import { useEffect, useMemo, useState } from 'react';
import { GameSimulation } from './engine/simulation';
import { LEVELS } from './engine/level';
import { loadUnlockedLevels, saveUnlockedLevels, loadRecord, saveRecordIfBetter } from './engine/records';
import { useHudSnapshot } from './engine/store';
import { PixiStage } from './render/PixiStage';
import { HUD } from './hud/HUD';
import { PauseMenu } from './hud/PauseMenu';
import { GameOverScreen } from './hud/GameOverScreen';
import { LevelCompleteScreen } from './hud/LevelCompleteScreen';
import { LevelSelectMenu } from './hud/LevelSelectMenu';
import './App.css';

type Screen = 'menu' | 'playing' | 'gameover' | 'levelComplete';

export function App() {
  const sim = useMemo(() => new GameSimulation(LEVELS, 0), []);
  const [screen, setScreen] = useState<Screen>('menu');
  const [paused, setPaused] = useState(false);
  const [unlockedLevels, setUnlockedLevels] = useState<number[]>(() => loadUnlockedLevels());
  const snap = useHudSnapshot(sim);

  useEffect(() => { if (screen === 'playing' && snap.gameOver) setScreen('gameover'); }, [snap.gameOver, screen]);
  useEffect(() => {
    if (screen === 'playing' && snap.levelComplete && snap.levelResult) {
      saveRecordIfBetter(LEVELS[snap.levelResult.levelIndex].id, snap.levelResult);
      if (snap.unlockedLevels.length > unlockedLevels.length) {
        setUnlockedLevels(snap.unlockedLevels);
        saveUnlockedLevels(snap.unlockedLevels);
      }
      setScreen('levelComplete');
    }
  }, [snap.levelComplete, screen]);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.code === 'Escape' && screen === 'playing') { setPaused((p) => { sim.setPaused(!p); return !p; }); }
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [screen, sim]);

  function startLevel(index: number) {
    sim.goToLevel(index);
    setPaused(false);
    setScreen('playing');
  }

  return (
    <div className="app-shell">
      <PixiStage sim={sim} />
      {screen === 'playing' && <HUD sim={sim} />}
      {screen === 'playing' && paused && (
        <PauseMenu
          onResume={() => { sim.setPaused(false); setPaused(false); }}
          onRestart={() => { sim.restartLevel(); sim.setPaused(false); setPaused(false); }}
          onQuit={() => { sim.setPaused(false); setPaused(false); setScreen('menu'); }}
        />
      )}
      {screen === 'gameover' && <GameOverScreen onReturnToMenu={() => setScreen('menu')} />}
      {screen === 'levelComplete' && snap.levelResult && (
        <LevelCompleteScreen
          result={snap.levelResult}
          record={loadRecord(LEVELS[snap.levelResult.levelIndex].id)!}
          hasNext={snap.levelResult.levelIndex + 1 < LEVELS.length}
          onNext={() => startLevel(snap.levelResult!.levelIndex + 1)}
          onMenu={() => setScreen('menu')}
        />
      )}
      {screen === 'menu' && <LevelSelectMenu unlockedLevels={unlockedLevels} onSelect={startLevel} />}
    </div>
  );
}
```

- [x] **Step 7: Update `PixiStage.tsx` to take `sim` as a prop**

Change the component signature to `export function PixiStage({ sim }: { sim: GameSimulation })`, remove the `new GameSimulation(...)` construction from inside its `useEffect` (the simulation now lives in `App.tsx` and outlives level switches), and keep everything else (texture loading, layer creation, the ticker callback reading `sim.getRenderSnapshot()`) unchanged.

- [x] **Step 8: Style the overlays in `App.css`**

```css
/* append to App.css */
.overlay {
  position: absolute; inset: 0; display: flex; flex-direction: column;
  align-items: center; justify-content: center; gap: 12px;
  background: rgba(58, 42, 30, 0.72); color: #F3E6D0; font-family: sans-serif;
}
.hud { position: absolute; top: 12px; left: 12px; display: flex; gap: 16px; color: #3A2A1E; font-weight: bold; }
.hud-hearts img { width: 28px; height: 28px; }
.hud-hearts .heart-empty { opacity: 0.25; }
.level-tiles { display: flex; flex-direction: column; gap: 8px; }
```

- [x] **Step 9: Smoke test the full user journey**

Run: `bun run dev`. Play from the level-select menu: start level 1 → pause (Escape) confirms the player/enemies freeze in place and resume continues them → let a pinecone reduce hearts to 0 → confirm the game-over screen appears and "Return to Menu" goes back to `LevelSelectMenu` → start level 1 again, collect all acorns, reach the squirrel → confirm `LevelCompleteScreen` shows correct acorns/time and level 2 is now unlocked and selectable. Reload the page: confirm level 2 is still unlocked and the level 1 tile still shows its best record (both read from `localStorage`).

- [x] **Step 10: Commit**

```bash
git add examples/platformer/src
git commit -m "feat(platformer-v2): add React HUD, pause/game-over/level-complete menus, and localStorage records"
```

---

### Task 6: End-to-end smoke test

**Files:**
- Modify: `examples/platformer/README.md` (document the new dev workflow, replacing the v1 static-file-server instructions)

**Interfaces:**
- Consumes: the fully assembled app from Tasks 1-5. Produces nothing new — this task only verifies.

- [x] **Step 1: Fresh install and dev boot**

Run: `cd examples/platformer && rm -rf node_modules && bun install && bun run dev`
Expected: install completes with no errors; the dev server starts and the level-select menu renders with no console errors.

- [x] **Step 2: Verify `platformer.enemy.01` and `platformer.enemy.02`**

Walk into a pinecone: confirm a heart is lost and the player is briefly invulnerable (repeated contact within ~1s does not cost a second heart). Let all 3 hearts run out: confirm the game-over screen appears and "Return to Menu" lands back on `LevelSelectMenu`.

- [x] **Step 3: Verify `platformer.levels.01` and `platformer.levels.02`**

Complete level 1 (reach the squirrel): confirm the level-complete screen appears and level 2's tile becomes selectable (was locked before). Collect a specific number of acorns and note the completion time, then reload the browser page: confirm the level 1 tile in `LevelSelectMenu` still shows that best-acorns/best-time record and level 2 is still unlocked.

- [x] **Step 4: Verify `platformer.moving.01` and `platformer.moving.02`**

In level 2, ride the horizontal platform across the pit: confirm the player's x moves with the platform without sinking or stalling. Walk to the platform's trailing edge mid-crossing until past its width: confirm a clean fall (no warp to a stale position, no clipping into a wall) followed by normal gravity and pit respawn if it falls past the world bottom.

- [x] **Step 5: Verify `platformer.hud.01` and `platformer.hud.02`**

Press Escape mid-level: confirm the player, enemies, and platforms all visibly freeze (no position drift while paused) and the pause menu's Resume/Restart/Quit-to-Menu buttons all behave correctly. While unpaused, take damage and collect acorns: confirm the HUD's heart icons and acorn counter update within the same frame as the underlying state change (no stale display).

- [x] **Step 6: Production build check**

Run: `bun run build`
Expected: `tsc -b` reports zero type errors and `vite build` emits `examples/platformer/dist/` with no build warnings about missing assets (`pinecone.png`, `heart.png`, `platform.png` all resolve).

- [x] **Step 7: Confirm workspace isolation**

Run: `cat package.json` (repo root) and confirm `workspaces` is still exactly `["packages/*"]`; confirm `examples/platformer/node_modules` and `examples/platformer/bun.lock` exist independently of the root's.

- [x] **Step 8: Update `README.md` and commit**

Replace the v1 "open `index.html`" instructions with: `cd examples/platformer && bun install && bun run dev`, and note the production build command `bun run build`.

```bash
git add examples/platformer/README.md
git commit -m "docs(platformer-v2): document Vite dev/build workflow; final v2 smoke test pass"
```
