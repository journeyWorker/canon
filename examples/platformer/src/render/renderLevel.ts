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
