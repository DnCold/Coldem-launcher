import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { SocialSnapshot } from "../types/social";
import { demoSocialClient } from "./demoSocialClient";

export interface SocialClient {
  snapshot(): Promise<SocialSnapshot>;
  connect(): Promise<SocialSnapshot>;
  disconnect(): Promise<SocialSnapshot>;
  invite(friendId: string): Promise<void>;
  onUpdate(handler: (snapshot: SocialSnapshot) => void): Promise<UnlistenFn>;
}

const tauriSocialClient: SocialClient = {
  snapshot: () => invoke("social_snapshot"),
  connect: () => invoke("connect_discord"),
  disconnect: () => invoke("disconnect_discord"),
  invite: (friendId) => invoke("invite_discord_friend", { friendId }),
  onUpdate: async (handler) =>
    listen<SocialSnapshot>("launcher://social", ({ payload }) => handler(payload))
};

const useDemoClient = !window.__TAURI_INTERNALS__ && import.meta.env.DEV;

export const socialClient: SocialClient = useDemoClient
  ? demoSocialClient
  : tauriSocialClient;
