export function GameOverScreen({ onReturnToMenu }: { onReturnToMenu: () => void }) {
  return (
    <div className="overlay game-over">
      <h2>Game Over</h2>
      <button onClick={onReturnToMenu}>Return to Menu</button>
    </div>
  );
}
