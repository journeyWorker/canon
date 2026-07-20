export interface Rect { x: number; y: number; w: number; h: number; }
export interface Kinematic extends Rect { vx: number; vy: number; }

// Any kinematic entity subject to floor/ceiling collision resolution
// (player, enemies) tracks whether it is currently resting on a surface.
export type Groundable = Kinematic & { onGround: boolean };

export interface PlayerState extends Kinematic {
  onGround: boolean;
  facing: 1 | -1;
  coyoteTimer: number;
  jumpBufferTimer: number;
  squashTimer: number;
  invulnTimer: number;
  knockbackTimer: number;
}

export interface AcornState { x: number; y: number; w: number; h: number; collected: boolean; }

export interface EnemyState extends Kinematic {
  id: string;
  alive: boolean;
  onGround: boolean;
  dir: 1 | -1;
  patrolMinX: number;
  patrolMaxX: number;
}

export interface EnemySpawn { x: number; y: number; patrolMinX: number; patrolMaxX: number; }

export interface MovingPlatformState extends Rect {
  id: string;
  axis: 'x' | 'y';
  min: number;
  max: number;
  speed: number;
  dir: 1 | -1;
}

export interface PlatformSpawn {
  x: number; y: number; w: number; h: number;
  axis: 'x' | 'y'; min: number; max: number; speed: number;
}

export interface LevelData {
  id: number;
  grid: string[]; // '#' solid, '.' air, 'A' acorn spawn, 'S' player spawn, 'Q' squirrel goal, 'P'/'E' spawn markers (non-solid)
  enemies: EnemySpawn[];
  platforms: PlatformSpawn[];
}

export interface LevelRecord { bestAcorns: number; bestTimeSeconds: number; }
