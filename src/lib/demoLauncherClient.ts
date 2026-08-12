import type { UnlistenFn } from "@tauri-apps/api/event";
import type { LauncherClient } from "./launcherClient";
import type {
  ButlerPrompt,
  Cave,
  Game,
  GameRecord,
  OperationEvent,
  Profile,
  Upload
} from "../types/launcher";

const profile: Profile = {
  id: 1,
  lastConnected: new Date().toISOString(),
  user: {
    id: 1,
    username: "player",
    displayName: "Coldem Player",
    developer: false,
    url: "https://github.com"
  }
};

const record: GameRecord = {
  id: 1,
  title: "Robot Rock",
  owned: true
};

const game: Game = {
  id: 1,
  url: "",
  title: "Robot Rock",
  shortText: "A loud mechanical world, ready for its first Coldem release.",
  type: "downloadable",
  classification: "game",
  userId: 1,
  user: {
    id: 1,
    username: "dancold",
    displayName: "DanCold",
    developer: true,
    url: "https://github.com/DnCold"
  }
};

const upload: Upload = {
  id: 1,
  filename: "robot-rock-0.1.0-full.pwr",
  displayName: "Windows · 0.1.0",
  size: 825_000_000,
  type: "default",
  demo: false,
  preorder: false,
  platforms: { windows: true },
  build: { version: "0.1.0" }
};

const caves: Cave[] = [];
const operationHandlers = new Set<(event: OperationEvent) => void>();
const promptHandlers = new Set<(prompt: ButlerPrompt) => void>();
const previewParams = new URLSearchParams(window.location.search);
const showLoginPreview = previewParams.has("login");
const showPromptPreview = previewParams.has("prompt");

const emitOperation = (event: OperationEvent) => {
  operationHandlers.forEach((handler) => handler(event));
};

const simulateOperation = async (kind: OperationEvent["kind"], gameId: number) => {
  emitOperation({ kind, gameId, state: "queued", progress: 0 });
  for (const progress of [0.12, 0.33, 0.58, 0.81, 1]) {
    await new Promise((resolve) => window.setTimeout(resolve, 220));
    emitOperation({
      kind,
      gameId,
      state: kind === "play" && progress > 0.5 ? "running" : "working",
      progress,
      bps: 7_800_000,
      eta: Math.max(0, (1 - progress) * 8)
    });
  }
  emitOperation({ kind, gameId, state: "finished", progress: 1 });
};

export const demoLauncherClient: LauncherClient = {
  initialize: async () => ({
    butlerVersion: "demo",
    profiles: showLoginPreview ? [] : [profile],
    catalogGameCount: 1,
    catalogRestricted: true
  }),
  beginLogin: async () => {
    await new Promise((resolve) => window.setTimeout(resolve, 350));
    return profile;
  },
  cancelLogin: async () => undefined,
  restoreProfile: async () => profile,
  forgetProfile: async () => undefined,
  fetchLibrary: async (_profileId, fresh) => {
    if (fresh) await new Promise((resolve) => window.setTimeout(resolve, 300));
    return { records: [{ ...record }], caves: [...caves], stale: false };
  },
  checkUpdates: async () => [],
  prepareInstall: async () => ({
    game,
    uploads: [upload],
    installLocationId: "demo-location"
  }),
  planInstall: async () => ({
    info: {
      upload,
      build: { version: "0.1.0" },
      type: "wharf",
      diskUsage: {
        finalDiskUsage: 1_420_000_000,
        neededFreeSpace: 2_245_000_000,
        accuracy: "estimate"
      }
    }
  }),
  install: async (_profileId, installedGame) => {
    await simulateOperation("install", installedGame.id);
    const installedAt = new Date().toISOString();
    record.installedAt = installedAt;
    caves.splice(0, caves.length, {
      id: "coldem-1",
      game,
      upload,
      build: { version: "0.1.0" },
      stats: { installedAt, lastTouchedAt: installedAt, secondsRun: 0 },
      installInfo: { installFolder: "C:/Games/robot-rock", installedSize: 1_420_000_000 }
    });
  },
  update: async (_profileId, _caveId, gameId) => simulateOperation("update", gameId),
  play: async (_profileId, _caveId, gameId) => simulateOperation("play", gameId),
  respondToPrompt: async () => undefined,
  openExternal: async (target) => {
    window.open(target, "_blank", "noopener,noreferrer");
  },
  onOperation: async (handler) => {
    operationHandlers.add(handler);
    return (() => operationHandlers.delete(handler)) as UnlistenFn;
  },
  onPrompt: async (handler) => {
    promptHandlers.add(handler);
    if (showPromptPreview) {
      window.setTimeout(() => handler({
        id: "preview-license",
        method: "AcceptLicense",
        params: {
          text: "COLD STORAGE LICENSE // Preview only\n\nKeep playing. Stay kind. Stay cold."
        }
      }), 60);
    }
    return (() => promptHandlers.delete(handler)) as UnlistenFn;
  }
};
