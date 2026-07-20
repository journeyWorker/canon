export interface InputSnapshot {
  left: boolean;
  right: boolean;
  jumpBuffered: boolean;
  restartRequested: boolean;
}

const JUMP_KEYS = new Set(['Space', 'ArrowUp', 'KeyW']);

export class InputCapture {
  private left = false;
  private right = false;
  private jumpBuffered = false;
  private restartRequested = false;

  private onKeyDown = (e: KeyboardEvent) => {
    if (e.code === 'ArrowLeft' || e.code === 'KeyA') this.left = true;
    if (e.code === 'ArrowRight' || e.code === 'KeyD') this.right = true;
    if (JUMP_KEYS.has(e.code) && !e.repeat) this.jumpBuffered = true;
    if (e.code === 'KeyR' && !e.repeat) this.restartRequested = true;
  };

  private onKeyUp = (e: KeyboardEvent) => {
    if (e.code === 'ArrowLeft' || e.code === 'KeyA') this.left = false;
    if (e.code === 'ArrowRight' || e.code === 'KeyD') this.right = false;
  };

  constructor() {
    window.addEventListener('keydown', this.onKeyDown);
    window.addEventListener('keyup', this.onKeyUp);
  }

  // Edge-triggered flags are consumed (reset to false) on read.
  snapshot(): InputSnapshot {
    const snap: InputSnapshot = {
      left: this.left, right: this.right,
      jumpBuffered: this.jumpBuffered, restartRequested: this.restartRequested,
    };
    this.jumpBuffered = false;
    this.restartRequested = false;
    return snap;
  }

  dispose(): void {
    window.removeEventListener('keydown', this.onKeyDown);
    window.removeEventListener('keyup', this.onKeyUp);
  }
}
