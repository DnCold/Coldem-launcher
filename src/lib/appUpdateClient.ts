import { invoke } from "@tauri-apps/api/core";

export interface LauncherUpdateStatus {
  configured: boolean;
  currentVersion: string;
}

export interface LauncherUpdateMetadata {
  version: string;
  currentVersion: string;
}

interface AppUpdateClient {
  status(): Promise<LauncherUpdateStatus>;
  check(): Promise<LauncherUpdateMetadata | null>;
  install(): Promise<void>;
}

const nativeClient: AppUpdateClient = {
  status: () => invoke("launcher_update_status"),
  check: () => invoke("check_launcher_update"),
  install: () => invoke("install_launcher_update")
};

const showUpdatePreview = new URLSearchParams(window.location.search).has("update");

const demoClient: AppUpdateClient = {
  status: async () => ({ configured: showUpdatePreview, currentVersion: "0.2.1-preview" }),
  check: async () => showUpdatePreview
    ? { currentVersion: "0.2.1", version: "0.2.2" }
    : null,
  install: async () => undefined
};

export const appUpdateClient = window.__TAURI_INTERNALS__ ? nativeClient : demoClient;
