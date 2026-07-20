import { useSyncExternalStore } from 'react';
import type { GameSimulation, HudSnapshot } from './simulation';

export function useHudSnapshot(sim: GameSimulation): HudSnapshot {
  return useSyncExternalStore(
    (listener) => sim.subscribe(listener),
    () => sim.getHudSnapshot()
  );
}
