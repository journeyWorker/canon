export function PauseMenu({ onResume, onRestart, onQuit }: { onResume: () => void; onRestart: () => void; onQuit: () => void }) {
  return (
    <div className="overlay pause-menu">
      <h2>Paused</h2>
      <button onClick={onResume}>Resume</button>
      <button onClick={onRestart}>Restart Level</button>
      <button onClick={onQuit}>Quit to Menu</button>
    </div>
  );
}
