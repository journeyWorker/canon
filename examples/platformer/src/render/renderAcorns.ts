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
