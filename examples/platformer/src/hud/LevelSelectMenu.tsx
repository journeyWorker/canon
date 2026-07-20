import { LEVELS } from '../engine/level';
import { loadRecord } from '../engine/records';

export function LevelSelectMenu({ unlockedLevels, onSelect }: { unlockedLevels: number[]; onSelect: (index: number) => void }) {
  return (
    <div className="overlay level-select">
      <h1>Red Panda Ridge</h1>
      <div className="level-tiles">
        {LEVELS.map((level, i) => {
          const unlocked = unlockedLevels.includes(i);
          const record = loadRecord(level.id);
          return (
            <button key={level.id} disabled={!unlocked} onClick={() => onSelect(i)}>
              Level {i + 1}
              {unlocked && record && <span> &mdash; best {record.bestAcorns} acorns / {record.bestTimeSeconds.toFixed(1)}s</span>}
              {!unlocked && <span> (locked)</span>}
            </button>
          );
        })}
      </div>
    </div>
  );
}
