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
      sprite.scale.x = (enemy.dir >= 0 ? 1 : -1) * Math.abs(sprite.scale.x);
    });
  }
  return { view, update };
}
