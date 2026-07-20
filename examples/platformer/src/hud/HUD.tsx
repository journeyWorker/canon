import { useHudSnapshot } from '../engine/store';
import type { GameSimulation } from '../engine/simulation';

function formatTime(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${s.toString().padStart(2, '0')}`;
}

export function HUD({ sim }: { sim: GameSimulation }) {
  const snap = useHudSnapshot(sim);
  return (
    <div className="hud">
      <div className="hud-hearts">
        {Array.from({ length: snap.maxHearts }, (_, i) => (
          <img key={i} src="assets/heart.png" className={i < snap.hearts ? 'heart-full' : 'heart-empty'} alt="" />
        ))}
      </div>
      <div className="hud-level">Level {snap.levelIndex + 1} &middot; {formatTime(snap.elapsed)}</div>
      <div className="hud-acorns">
        <img src="assets/acorn.png" alt="" />
        {snap.acorns} / {snap.acornsTotal}
      </div>
    </div>
  );
}
