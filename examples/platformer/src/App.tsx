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

declare global {
  interface Window { __sim?: GameSimulation; }
}

type Screen = 'menu' | 'playing' | 'gameover' | 'levelComplete';

export function App() {
  const [screen, setScreen] = useState<Screen>('menu');
  const [paused, setPaused] = useState(false);
  const [unlockedLevels, setUnlockedLevels] = useState<number[]>(() => loadUnlockedLevels());
  const sim = useMemo(() => new GameSimulation(LEVELS, 0, unlockedLevels), []);
  const snap = useHudSnapshot(sim);

  useEffect(() => {
    if (import.meta.env.DEV) window.__sim = sim;
  }, [sim]);

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

  // The R restart shortcut is captured globally by InputCapture (mounted once
  // for the app's lifetime in PixiStage) and resets the simulation regardless
  // of which screen is showing. Sync 'screen' back to 'playing' once the sim
  // actually leaves its complete/game-over state (e.g. via that shortcut)
  // instead of forcing it eagerly on keydown -- the restart only takes effect
  // on the next tick, and an eager set would race the win/game-over detection
  // effects above, which re-fire off the still-stale snapshot and flip the
  // screen right back, leaving no HUD/overlay visible at all.
  useEffect(() => {
    if (screen === 'levelComplete' && !snap.levelComplete) setScreen('playing');
    if (screen === 'gameover' && !snap.gameOver) setScreen('playing');
  }, [snap.levelComplete, snap.gameOver, screen]);

  // Notify the simulation of pause changes from an effect (post-commit),
  // never from inside a setState updater -- sim.setPaused() synchronously
  // notifies useSyncExternalStore subscribers (HUD), and doing that while
  // App is still applying its own state update reproducibly triggers
  // React's "Cannot update a component while rendering a different
  // component" warning on every Escape/P pause toggle.
  useEffect(() => { sim.setPaused(paused); }, [paused, sim]);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.repeat) return;
      if ((e.code === 'Escape' || e.code === 'KeyP') && screen === 'playing') {
        setPaused((p) => !p);
      }
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [screen]);

  function startLevel(index: number) {
    sim.goToLevel(index);
    setPaused(false);
    setScreen('playing');
  }

  return (
    <div className="app-shell">
      <PixiStage sim={sim} />
      {(screen === 'playing' || screen === 'gameover') && <HUD sim={sim} />}
      {screen === 'playing' && paused && (
        <PauseMenu
          onResume={() => setPaused(false)}
          onRestart={() => { sim.restartLevel(); setPaused(false); }}
          onQuit={() => { setPaused(false); setScreen('menu'); }}
        />
      )}
      {screen === 'gameover' && <GameOverScreen onReturnToMenu={() => setScreen('menu')} />}
      {screen === 'levelComplete' && snap.levelResult && (
        <LevelCompleteScreen
          result={snap.levelResult}
          record={loadRecord(LEVELS[snap.levelResult.levelIndex].id) ?? { bestAcorns: snap.levelResult.acorns, bestTimeSeconds: snap.levelResult.timeSeconds }}
          hasNext={snap.levelResult.levelIndex + 1 < LEVELS.length}
          onNext={() => startLevel(snap.levelResult!.levelIndex + 1)}
          onMenu={() => setScreen('menu')}
        />
      )}
      {screen === 'menu' && <LevelSelectMenu unlockedLevels={unlockedLevels} onSelect={startLevel} />}
    </div>
  );
}
