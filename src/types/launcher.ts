export interface User {
  id: number;
  username: string;
  displayName: string;
  developer: boolean;
  url: string;
  coverUrl?: string;
  stillCoverUrl?: string;
}

export interface Profile {
  id: number;
  lastConnected: string;
  user: User;
}

export interface Game {
  id: number;
  url: string;
  title: string;
  shortText?: string;
  type: string;
  classification: string;
  coverUrl?: string;
  stillCoverUrl?: string;
  user?: User;
  userId: number;
}

export interface GameRecord {
  id: number;
  slug?: string;
  title: string;
  url?: string;
  cover?: string;
  owned?: boolean;
  installedAt?: string;
}

export interface Upload {
  id: number;
  filename: string;
  displayName: string;
  size: number;
  channelName?: string;
  type: string;
  demo: boolean;
  preorder: boolean;
  platforms: Record<string, boolean>;
  build?: unknown;
}

export interface Cave {
  id: string;
  game: Game;
  upload: Upload;
  build?: unknown;
  stats: {
    installedAt?: string;
    lastTouchedAt?: string;
    secondsRun: number;
  };
  interaction?: {
    userId: number;
    gameId: number;
    secondsRun: number;
    lastRunAt?: string;
    syncedAt?: string;
  };
  installInfo: {
    installFolder: string;
    installedSize: number;
  };
}

export interface GameUpdateChoice {
  upload: Upload;
  build?: unknown;
  confidence: number;
}

export interface GameUpdate {
  caveId: string;
  game: Game;
  direct: boolean;
  choices: GameUpdateChoice[];
}

export interface LibrarySnapshot {
  records: GameRecord[];
  caves: Cave[];
  stale: boolean;
  warning?: string;
}

export type ReleaseChannel = "stable" | "test";

export interface BootstrapResult {
  butlerVersion: string;
  profiles: Profile[];
  catalogGameCount: number;
  catalogRestricted: boolean;
  channel: ReleaseChannel;
}

export interface InstallOptions {
  game: Game;
  uploads: Upload[];
  incompatibleUploads?: Upload[];
  installLocationId: string;
}

export interface InstallPlan {
  info: {
    upload: Upload;
    build?: unknown;
    type: string;
    diskUsage?: {
      finalDiskUsage: number;
      neededFreeSpace: number;
      accuracy: string;
    };
    error?: string;
    errorMessage?: string;
  };
}

export type OperationKind = "install" | "update" | "play";
export type OperationState =
  | "queued"
  | "working"
  | "running"
  | "finished"
  | "failed";

export interface OperationEvent {
  kind: OperationKind;
  gameId: number;
  state: OperationState;
  progress?: number;
  eta?: number;
  bps?: number;
  message?: string;
}

export interface ButlerPrompt {
  id: number | string;
  method: string;
  params: Record<string, unknown>;
}

export interface ActiveOperation extends OperationEvent {
  startedAt: number;
}
