import { useEffect, useRef } from 'react';
import { Application, Sprite, Texture } from 'pixi.js';
import { loadTextures } from './assets';
import { createBackgroundLayer } from './renderBackground';
import { createLevelLayer } from './renderLevel';
import { createAcornLayer } from './renderAcorns';
import { createEnemyLayer } from './renderEnemies';
import { createPlatformLayer } from './renderPlatforms';
import { createPlayerSprite, updatePlayerSprite } from './renderPlayer';
import { InputCapture } from '../engine/input';
import type { GameSimulation } from '../engine/simulation';
import { SQUIRREL_DRAW } from '../engine/constants';
import { LEVELS } from '../engine/level';

export function PixiStage({ sim }: { sim: GameSimulation }) {
  const hostRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let app: Application | null = null;
    let cancelled = false;
    let destroyed = false;
    let disposeTicker: (() => void) | null = null;
    const input = new InputCapture();

    // Single owner for teardown: whichever caller (the unmount cleanup, or
    // an async continuation that resumes after cancellation) reaches this
    // first performs the real destroy exactly once; the other is a no-op.
    // Takes the instance explicitly rather than reading the outer `app` var
    // so a cancellation that lands before `app` is assigned still gets
    // torn down once the async continuation resumes with its own reference.
    function teardown(instance: Application | null) {
      if (destroyed || !instance) return;
      destroyed = true;
      disposeTicker?.();
      input.dispose();
      instance.destroy(true, { children: true, texture: true, textureSource: true });
    }

    (async () => {
      const application = new Application();
      await application.init({ width: 960, height: 540, background: '#F6C89F', antialias: false, roundPixels: true });
      if (cancelled) { teardown(application); return; }
      app = application;
      hostRef.current?.appendChild(application.canvas);

      const textures: Record<string, Texture> = await loadTextures();
      if (cancelled) { teardown(application); return; }

      const bgLayer = createBackgroundLayer(textures);
      application.stage.addChild(bgLayer.view);

      let snap = sim.getRenderSnapshot();
      let levelLayer = createLevelLayer(LEVELS[snap.levelIndex], textures);
      let platformLayer = createPlatformLayer(snap.platforms, textures);
      let enemyLayer = createEnemyLayer(snap.enemies, textures);
      let acornLayer = createAcornLayer(snap.acorns, textures);
      const squirrelSprite = new Sprite(textures['squirrel']);
      squirrelSprite.anchor.set(0.5, 1);
      squirrelSprite.width = SQUIRREL_DRAW.w; squirrelSprite.height = SQUIRREL_DRAW.h;
      const playerSprite = createPlayerSprite(textures);

      application.stage.addChild(levelLayer.view, platformLayer.view, enemyLayer.view, acornLayer.view, squirrelSprite, playerSprite);

      let seenLevelIndex = snap.levelIndex;

      const tickerCallback = (ticker: { deltaMS: number }) => {
        sim.setInput(input.snapshot());
        sim.tick(ticker.deltaMS / 1000);
        snap = sim.getRenderSnapshot();

        if (snap.levelIndex !== seenLevelIndex) {
          seenLevelIndex = snap.levelIndex;
          application.stage.removeChild(levelLayer.view, platformLayer.view, enemyLayer.view, acornLayer.view);
          levelLayer.view.destroy({ children: true });
          platformLayer.view.destroy({ children: true });
          enemyLayer.view.destroy({ children: true });
          acornLayer.view.destroy({ children: true });
          levelLayer = createLevelLayer(LEVELS[snap.levelIndex], textures);
          platformLayer = createPlatformLayer(snap.platforms, textures);
          enemyLayer = createEnemyLayer(snap.enemies, textures);
          acornLayer = createAcornLayer(snap.acorns, textures);
          application.stage.addChildAt(levelLayer.view, application.stage.getChildIndex(squirrelSprite));
          application.stage.addChildAt(platformLayer.view, application.stage.getChildIndex(squirrelSprite));
          application.stage.addChildAt(enemyLayer.view, application.stage.getChildIndex(squirrelSprite));
          application.stage.addChildAt(acornLayer.view, application.stage.getChildIndex(squirrelSprite));
        }

        levelLayer.update(snap.camera.x);
        platformLayer.update(snap.camera.x);
        enemyLayer.update(snap.camera.x);
        acornLayer.update(snap.camera.x, snap.elapsed);
        bgLayer.update(snap.camera.x);
        squirrelSprite.x = snap.squirrel.x + snap.squirrel.w / 2 - snap.camera.x;
        squirrelSprite.y = snap.squirrel.y + snap.squirrel.h;
        updatePlayerSprite(playerSprite, snap.player, snap.camera.x, snap.elapsed);
      };
      application.ticker.add(tickerCallback);
      disposeTicker = () => application.ticker.remove(tickerCallback);
    })();

    return () => {
      cancelled = true;
      teardown(app);
    };
  }, [sim]);

  return <div ref={hostRef} className="pixi-stage" />;
}
