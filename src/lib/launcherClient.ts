import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  BootstrapResult,
  ButlerPrompt,
  Game,
  GameUpdate,
  InstallOptions,
  InstallPlan,
  LibrarySnapshot,
  OperationEvent,
  Profile,
  ReleaseChannel,
  Upload
} from "../types/launcher";
import { demoLauncherClient } from "./demoLauncherClient";

export interface LauncherClient {
  initialize(channel: ReleaseChannel): Promise<BootstrapResult>;
  setChannel(channel: ReleaseChannel): Promise<void>;
  beginLogin(): Promise<Profile | null>;
  cancelLogin(): Promise<void>;
  restoreProfile(profileId: number): Promise<Profile>;
  forgetProfile(profileId: number): Promise<void>;
  fetchLibrary(profileId: number, fresh: boolean): Promise<LibrarySnapshot>;
  checkUpdates(): Promise<GameUpdate[]>;
  prepareInstall(gameId: number, profileId: number): Promise<InstallOptions>;
  planInstall(uploadId: number): Promise<InstallPlan>;
  install(
    profileId: number,
    game: Game,
    upload: Upload,
    installLocationId: string
  ): Promise<void>;
  update(profileId: number, caveId: string, gameId: number): Promise<void>;
  play(profileId: number, caveId: string, gameId: number): Promise<void>;
  respondToPrompt(id: number | string, result: Record<string, unknown>): Promise<void>;
  openExternal(target: string): Promise<void>;
  onOperation(handler: (event: OperationEvent) => void): Promise<UnlistenFn>;
  onPrompt(handler: (prompt: ButlerPrompt) => void): Promise<UnlistenFn>;
}

const tauriLauncherClient: LauncherClient = {
  initialize: (channel) => invoke("initialize_launcher", { channel }),
  setChannel: (channel) => invoke("set_release_channel", { channel }),
  beginLogin: () => invoke<Profile>("enter_library"),
  cancelLogin: () => invoke("cancel_entry"),
  restoreProfile: (profileId) => invoke("use_local_profile", { profileId }),
  forgetProfile: (profileId) => invoke("forget_profile", { profileId }),
  fetchLibrary: (profileId, fresh) =>
    invoke("fetch_library", { profileId, fresh }),
  checkUpdates: () => invoke("check_updates"),
  prepareInstall: (gameId, profileId) =>
    invoke("prepare_install", { gameId, profileId }),
  planInstall: (uploadId) => invoke("plan_install", { uploadId }),
  install: (profileId, game, upload, installLocationId) =>
    invoke("install_game", {
      profileId,
      game,
      upload,
      installLocationId
    }),
  update: (profileId, caveId, gameId) =>
    invoke("update_game", { profileId, caveId, gameId }),
  play: (profileId, caveId, gameId) =>
    invoke("play_game", { profileId, caveId, gameId }),
  respondToPrompt: (id, result) =>
    invoke("respond_to_prompt", { id, result }),
  openExternal: (target) => invoke("open_external", { target }),
  onOperation: async (handler) =>
    listen<OperationEvent>("launcher://operation", ({ payload }) => handler(payload)),
  onPrompt: async (handler) =>
    listen<ButlerPrompt>("launcher://prompt", ({ payload }) => handler(payload))
};

const useDemoClient = !window.__TAURI_INTERNALS__ && import.meta.env.DEV;

export const launcherClient: LauncherClient = useDemoClient
  ? demoLauncherClient
  : tauriLauncherClient;

export const isDemoMode = useDemoClient;
