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
