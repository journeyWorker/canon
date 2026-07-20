# Red Panda Ridge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a playable single-level browser platformer — a red panda that runs, jumps, collects acorns, and reaches a squirrel friend to win — as a static `examples/platformer/` page with zero build step and zero dependencies.

**Architecture:** A single `index.html` boots a 960x540 `<canvas>` and loads `main.js` as a classic (non-module) script for zero-tooling compatibility. `main.js` owns one `requestAnimationFrame` game loop split into `update(dt)` and `render(ctx)`, a hardcoded 2D tile-array level, an axis-aligned-bounding-box collision pass against solid tiles, and a tiny entity list (player, acorns, squirrel). Five PNGs in `assets/` are loaded once at startup via an `Image()` preloader gate before the loop starts.

**Tech Stack:** Vanilla HTML5 Canvas 2D API, vanilla JavaScript (ES2020, no modules, no bundler), no npm dependencies, no build step. Manual smoke testing via a static file server (e.g. `python3 -m http.server`) — no test framework.

## Global Constraints

- Files live exactly at `examples/platformer/index.html`, `examples/platformer/main.js`, `examples/platformer/assets/`.
- Canvas is exactly 960x540 pixels.
- No build step: the page must run by opening `index.html` (or serving the folder statically) with no compile/bundle/transpile step.
- No dependencies: no npm packages, no CDN script tags, no external fonts/libraries.
- Player character is a red panda; sprite asset at `assets/red-panda.png`.
- End-of-level friendly NPC is a squirrel; sprite asset at `assets/squirrel.png`.
- Collectible is an acorn; sprite asset at `assets/acorn.png`; collecting increments an on-screen score.
- Solid ground/platform tiles use `assets/tile.png`; the level background uses `assets/background.png`.
- Controls: Arrow keys or WASD for left/right movement, Space (or Up/W) for jump.
- Player physics: gravity pulls the player down every frame; the player only jumps when standing on a solid tile (no double-jump/air-jump).
- Level is one hardcoded 2D tile array in `main.js` — no level file loading, no level editor.
- Win condition: player's bounding box overlaps the squirrel's bounding box; the game then shows a win message and freezes player input.

---

### Task 1: Canvas scaffold and game loop

**Files:**
- Create: `examples/platformer/index.html`
- Create: `examples/platformer/main.js`

**Interfaces:**
- Consumes: nothing (first task).
- Produces:
  - Global `const CANVAS_W = 960, CANVAS_H = 540;` in `main.js`.
  - Global `const ctx` — the 2D rendering context obtained from `document.getElementById('game')`.
  - `function loop(timestampMs)` — the `requestAnimationFrame` callback; computes `dt` in seconds and calls `update(dt)` then `render(ctx)`.
  - `function update(dt)` — stub in this task, body filled by Task 2.
  - `function render(ctx)` — stub in this task; clears canvas to a solid color.
  - `const keys = {}` — object tracking currently-held key state, populated by `keydown`/`keyup` listeners keyed on `event.code` (e.g. `"ArrowLeft"`, `"KeyA"`, `"Space"`).

- [x] **Step 1: Create `index.html` with the canvas element**

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>Red Panda Ridge</title>
  <style>
    html, body { margin: 0; padding: 0; background: #1b1425; display: flex; justify-content: center; align-items: center; height: 100%; }
    canvas { image-rendering: pixelated; border: 2px solid #3a2e4a; }
  </style>
</head>
<body>
  <canvas id="game" width="960" height="540"></canvas>
  <script src="main.js"></script>
</body>
</html>
```

- [x] **Step 2: Verify the page loads with no console errors**

Run: `python3 -m http.server 8765 --directory examples/platformer` then open `http://localhost:8765/` in a browser.
Expected: a bordered 960x540 canvas renders on a dark page background; devtools console shows no errors (main.js does not exist yet, so this step is really about confirming the HTML/canvas markup is valid — expect a 404 for `main.js` at this point, which is fine).

- [x] **Step 3: Create `main.js` with constants, key tracking, and the RAF loop skeleton**

```javascript
const CANVAS_W = 960;
const CANVAS_H = 540;
const canvas = document.getElementById('game');
const ctx = canvas.getContext('2d');

const keys = {};
window.addEventListener('keydown', (e) => { keys[e.code] = true; });
window.addEventListener('keyup', (e) => { keys[e.code] = false; });

let lastTime = 0;

function update(dt) {
  // filled in by Task 2 (physics) and Task 3 (collectibles/win)
}

function render(ctx) {
  ctx.clearRect(0, 0, CANVAS_W, CANVAS_H);
  ctx.fillStyle = '#5b8fd6';
  ctx.fillRect(0, 0, CANVAS_W, CANVAS_H);
}

function loop(timestampMs) {
  const dt = lastTime ? (timestampMs - lastTime) / 1000 : 0;
  lastTime = timestampMs;
  update(dt);
  render(ctx);
  requestAnimationFrame(loop);
}

requestAnimationFrame(loop);
```

- [x] **Step 4: Smoke test the loop runs**

Run: reload `http://localhost:8765/` with devtools open, add a temporary `console.log(dt)` inside `update` if needed to confirm it fires every frame, then remove it.
Expected: canvas fills solid blue every frame; no console errors; frame callback fires continuously (visible via a quick `console.count('frame')` check, then removed before committing).

- [x] **Step 5: Commit**

```bash
git add examples/platformer/index.html examples/platformer/main.js
git commit -m "feat(platformer): scaffold canvas and game loop"
```

---

### Task 2: Player physics, input, and level collision

**Files:**
- Modify: `examples/platformer/main.js`

**Interfaces:**
- Consumes: `CANVAS_W`, `CANVAS_H`, `keys`, `update(dt)`, `render(ctx)`, `loop` from Task 1.
- Produces:
  - `const TILE = 60;` — tile size in pixels (16 columns x 9 rows fills 960x540).
  - `const level = [...]` — a 2D array of single-character strings (`'#'` solid tile, `'.'` empty, `'A'` acorn spawn marker, `'S'` squirrel spawn marker, `'P'` player spawn marker), 16 columns wide, 9 rows tall.
  - `function tileAt(col, row)` — returns the character at `level[row][col]`, or `'#'` (treated as solid) when out of bounds.
  - `const player = { x, y, w, h, vx, vy, onGround }` — player state object; `w = 40, h = 48`.
  - `const GRAVITY = 1400;` (px/s²), `const MOVE_SPEED = 220;` (px/s), `const JUMP_VELOCITY = -560;` (px/s).
  - `function resolveCollisions(entity)` — mutates `entity.x/y/vx/vy/onGround` against solid level tiles using AABB sweep, called from `update(dt)`.
  - `const images = {}` — populated by an `Image()` preloader (`assets/red-panda.png`, `assets/squirrel.png`, `assets/acorn.png`, `assets/tile.png`, `assets/background.png`) with `let assetsReady = false;` flipped true once every image's `onload` has fired; `render(ctx)` no-ops (or shows a "Loading…" text) until `assetsReady`.

- [x] **Step 1: Define the tile grid and level layout**

```javascript
const TILE = 60; // 16 cols x 9 rows = 960x540
const level = [
  '................',
  '................',
  '................',
  '....A....A......',
  '..............S.',
  '.####...####..##',
  'P....A..........',
  '#####.####.####.',
  '################',
];
```

- [x] **Step 2: Add `tileAt` and asset preloader**

```javascript
function tileAt(col, row) {
  if (row < 0 || row >= level.length || col < 0 || col >= level[0].length) return '#';
  return level[row][col];
}

const ASSET_NAMES = ['red-panda', 'squirrel', 'acorn', 'tile', 'background'];
const images = {};
let assetsLoaded = 0;
let assetsReady = false;
for (const name of ASSET_NAMES) {
  const img = new Image();
  img.onload = () => {
    assetsLoaded++;
    if (assetsLoaded === ASSET_NAMES.length) assetsReady = true;
  };
  img.src = `assets/${name}.png`;
  images[name] = img;
}
```

- [x] **Step 3: Add player state, spawn from level, and gravity/move/jump physics**

```javascript
const GRAVITY = 1400;
const MOVE_SPEED = 220;
const JUMP_VELOCITY = -560;

function findSpawn(marker) {
  for (let row = 0; row < level.length; row++) {
    const col = level[row].indexOf(marker);
    if (col !== -1) return { x: col * TILE, y: row * TILE };
  }
  return { x: 0, y: 0 };
}

const spawn = findSpawn('P');
const player = { x: spawn.x, y: spawn.y, w: 40, h: 48, vx: 0, vy: 0, onGround: false };

function updatePlayer(dt) {
  const moveLeft = keys['ArrowLeft'] || keys['KeyA'];
  const moveRight = keys['ArrowRight'] || keys['KeyD'];
  const jumpPressed = keys['Space'] || keys['ArrowUp'] || keys['KeyW'];

  player.vx = moveLeft ? -MOVE_SPEED : moveRight ? MOVE_SPEED : 0;
  if (jumpPressed && player.onGround) {
    player.vy = JUMP_VELOCITY;
    player.onGround = false;
  }

  player.vy += GRAVITY * dt;
  player.x += player.vx * dt;
  player.y += player.vy * dt;
  resolveCollisions(player);
}
```

- [x] **Step 4: Add AABB tile collision resolution**

```javascript
function resolveCollisions(entity) {
  entity.onGround = false;

  const firstCol = Math.floor(entity.x / TILE);
  const lastCol = Math.floor((entity.x + entity.w) / TILE);
  const firstRow = Math.floor(entity.y / TILE);
  const lastRow = Math.floor((entity.y + entity.h) / TILE);

  for (let row = firstRow; row <= lastRow; row++) {
    for (let col = firstCol; col <= lastCol; col++) {
      if (tileAt(col, row) !== '#') continue;
      const tileX = col * TILE, tileY = row * TILE;
      const overlapX = Math.min(entity.x + entity.w, tileX + TILE) - Math.max(entity.x, tileX);
      const overlapY = Math.min(entity.y + entity.h, tileY + TILE) - Math.max(entity.y, tileY);
      if (overlapX <= 0 || overlapY <= 0) continue;

      if (overlapX < overlapY) {
        entity.x += entity.x < tileX ? -overlapX : overlapX;
        entity.vx = 0;
      } else {
        if (entity.y < tileY) {
          entity.y -= overlapY;
          entity.vy = 0;
          entity.onGround = true;
        } else {
          entity.y += overlapY;
          entity.vy = 0;
        }
      }
    }
  }
}
```

- [x] **Step 5: Wire `updatePlayer` into `update(dt)` and draw the level + player in `render(ctx)`**

```javascript
function update(dt) {
  if (!assetsReady) return;
  updatePlayer(dt);
}

function render(ctx) {
  ctx.clearRect(0, 0, CANVAS_W, CANVAS_H);
  if (!assetsReady) {
    ctx.fillStyle = '#1b1425';
    ctx.fillRect(0, 0, CANVAS_W, CANVAS_H);
    ctx.fillStyle = '#fff';
    ctx.fillText('Loading...', CANVAS_W / 2 - 20, CANVAS_H / 2);
    return;
  }

  ctx.drawImage(images['background'], 0, 0, CANVAS_W, CANVAS_H);

  for (let row = 0; row < level.length; row++) {
    for (let col = 0; col < level[row].length; col++) {
      if (tileAt(col, row) === '#') {
        ctx.drawImage(images['tile'], col * TILE, row * TILE, TILE, TILE);
      }
    }
  }

  ctx.drawImage(images['red-panda'], player.x, player.y, player.w, player.h);
}
```

- [x] **Step 6: Smoke test movement, jumping, and collision**

Run: reload `http://localhost:8765/` in a browser, press Arrow keys / WASD and Space.
Expected: red panda sprite (or its bounding box, if the asset isn't final yet) moves left/right, falls under gravity, lands on `#` tiles without sinking through them, and jumps only while grounded (holding Space in mid-air does not trigger a second jump).

- [x] **Step 7: Commit**

```bash
git add examples/platformer/main.js
git commit -m "feat(platformer): add level grid, asset preload, and player physics"
```

---

### Task 3: Acorn collectibles, score, and squirrel win condition

**Files:**
- Modify: `examples/platformer/main.js`

**Interfaces:**
- Consumes: `level`, `findSpawn`, `TILE`, `player`, `images`, `assetsReady`, `update(dt)`, `render(ctx)` from Task 2.
- Produces:
  - `const acorns = [...]` — array of `{ x, y, w, h, collected }` built by scanning `level` for every `'A'` marker.
  - `let score = 0;` — incremented once per newly collected acorn.
  - `function aabbOverlap(a, b)` — reusable AABB overlap test used for acorn pickup and the win check.
  - `const squirrel = { x, y, w, h }` — spawned from the `'S'` marker in `level`.
  - `let gameWon = false;` — set true when the player overlaps `squirrel`; once true, `updatePlayer` no longer applies input.
  - HUD score text and a win-state overlay drawn in `render(ctx)`.

- [x] **Step 1: Spawn acorns and the squirrel from the level grid**

```javascript
function findAllSpawns(marker) {
  const out = [];
  for (let row = 0; row < level.length; row++) {
    for (let col = 0; col < level[row].length; col++) {
      if (level[row][col] === marker) out.push({ x: col * TILE, y: row * TILE });
    }
  }
  return out;
}

const ACORN_SIZE = 28;
const acorns = findAllSpawns('A').map((pos) => ({
  x: pos.x + (TILE - ACORN_SIZE) / 2,
  y: pos.y + (TILE - ACORN_SIZE) / 2,
  w: ACORN_SIZE,
  h: ACORN_SIZE,
  collected: false,
}));

const squirrelSpawn = findSpawn('S');
const squirrel = { x: squirrelSpawn.x, y: squirrelSpawn.y, w: TILE, h: TILE };

let score = 0;
let gameWon = false;
```

- [x] **Step 2: Add the shared AABB overlap helper**

```javascript
function aabbOverlap(a, b) {
  return a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y;
}
```

- [x] **Step 3: Check acorn pickup and the squirrel win condition each frame**

```javascript
function updateCollectiblesAndWin() {
  for (const acorn of acorns) {
    if (!acorn.collected && aabbOverlap(player, acorn)) {
      acorn.collected = true;
      score++;
    }
  }
  if (!gameWon && aabbOverlap(player, squirrel)) {
    gameWon = true;
  }
}
```

- [x] **Step 4: Wire the new checks into `update(dt)` and gate input once the game is won**

```javascript
function update(dt) {
  if (!assetsReady) return;
  if (!gameWon) {
    updatePlayer(dt);
    updateCollectiblesAndWin();
  }
}
```

- [x] **Step 5: Draw acorns, squirrel, HUD score, and the win overlay in `render(ctx)`**

```javascript
function render(ctx) {
  ctx.clearRect(0, 0, CANVAS_W, CANVAS_H);
  if (!assetsReady) {
    ctx.fillStyle = '#1b1425';
    ctx.fillRect(0, 0, CANVAS_W, CANVAS_H);
    ctx.fillStyle = '#fff';
    ctx.fillText('Loading...', CANVAS_W / 2 - 20, CANVAS_H / 2);
    return;
  }

  ctx.drawImage(images['background'], 0, 0, CANVAS_W, CANVAS_H);

  for (let row = 0; row < level.length; row++) {
    for (let col = 0; col < level[row].length; col++) {
      if (tileAt(col, row) === '#') {
        ctx.drawImage(images['tile'], col * TILE, row * TILE, TILE, TILE);
      }
    }
  }

  for (const acorn of acorns) {
    if (!acorn.collected) ctx.drawImage(images['acorn'], acorn.x, acorn.y, acorn.w, acorn.h);
  }

  ctx.drawImage(images['squirrel'], squirrel.x, squirrel.y, squirrel.w, squirrel.h);
  ctx.drawImage(images['red-panda'], player.x, player.y, player.w, player.h);

  ctx.fillStyle = '#fff';
  ctx.font = '20px sans-serif';
  ctx.fillText(`Acorns: ${score} / ${acorns.length}`, 16, 28);

  if (gameWon) {
    ctx.fillStyle = 'rgba(0,0,0,0.55)';
    ctx.fillRect(0, 0, CANVAS_W, CANVAS_H);
    ctx.fillStyle = '#fff';
    ctx.font = '36px sans-serif';
    ctx.fillText('You reached your squirrel friend!', CANVAS_W / 2 - 260, CANVAS_H / 2);
  }
}
```

- [x] **Step 6: Smoke test collection and the win state**

Run: reload the page, walk the panda over each acorn tile, then walk into the squirrel.
Expected: the HUD "Acorns: N / total" count increases by 1 per acorn touched and each acorn disappears once collected; touching the squirrel darkens the screen, shows the win message, and further arrow/space presses no longer move the player.

- [x] **Step 7: Commit**

```bash
git add examples/platformer/main.js
git commit -m "feat(platformer): add acorn collection, score HUD, and win condition"
```

---

### Task 4: Polish pass and full end-to-end smoke test

**Files:**
- Modify: `examples/platformer/main.js`
- Modify: `examples/platformer/index.html`

**Interfaces:**
- Consumes: everything produced by Tasks 1-3 (`update`, `render`, `player`, `acorns`, `squirrel`, `score`, `gameWon`, `images`).
- Produces:
  - `function resetGame()` — restores `player`, `score`, `acorns[*].collected`, and `gameWon` to initial spawn state; bound to an `R`/`KeyR` key press for manual replay during testing (no UI button required).
  - Final visual polish only (no new gameplay state): a start hint, a page `<title>`/`<meta>` check, and console-error-free playthrough confirmation.

- [x] **Step 1: Add a restart-on-`R` handler using a resettable game-state snapshot**

```javascript
function resetGame() {
  const p = findSpawn('P');
  player.x = p.x; player.y = p.y; player.vx = 0; player.vy = 0; player.onGround = false;
  for (const acorn of acorns) acorn.collected = false;
  score = 0;
  gameWon = false;
}

window.addEventListener('keydown', (e) => {
  if (e.code === 'KeyR') resetGame();
});
```

- [x] **Step 2: Add a small on-canvas control hint shown before the first move**

```javascript
// inside render(ctx), after drawing the HUD score, before the gameWon overlay block:
ctx.fillStyle = 'rgba(255,255,255,0.85)';
ctx.font = '14px sans-serif';
ctx.fillText('Arrows/WASD to move, Space to jump, R to restart', 16, CANVAS_H - 14);
```

- [x] **Step 3: Confirm `index.html` metadata is complete**

Verify `examples/platformer/index.html` already has `<meta charset="UTF-8">` and a non-empty `<title>Red Panda Ridge</title>` from Task 1, Step 1. No change needed if present; add them if missing.

- [x] **Step 4: Full end-to-end smoke test with devtools open**

Run: `python3 -m http.server 8765 --directory examples/platformer`, open `http://localhost:8765/`, and play a full round: move both directions, jump onto and off platforms, collect every acorn, reach the squirrel, then press `R` to confirm reset.
Expected: zero console errors/warnings throughout; every acorn is reachable via jump-and-move without an impossible gap; the win overlay appears exactly once per playthrough at the squirrel; `R` fully restores the initial state (score back to 0, all acorns visible again, win overlay cleared).

- [x] **Step 5: Verify zero-dependency, zero-build-step constraint**

Run: `grep -n "require(\|import \|<script src=\"http" examples/platformer/index.html examples/platformer/main.js`
Expected: no matches — confirms no bundler imports and no external/CDN script tags are present.

- [x] **Step 6: Commit**

```bash
git add examples/platformer/main.js examples/platformer/index.html
git commit -m "feat(platformer): add restart, control hint, and finalize smoke test"
```
