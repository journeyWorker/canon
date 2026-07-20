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
