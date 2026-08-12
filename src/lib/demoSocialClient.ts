import type { UnlistenFn } from "@tauri-apps/api/event";
import type { SocialClient } from "./socialClient";
import type { SocialSnapshot } from "../types/social";

const previewParams = new URLSearchParams(window.location.search);
const updateHandlers = new Set<(snapshot: SocialSnapshot) => void>();

const disconnected: SocialSnapshot = {
  connection: "disconnected",
  applicationConfigured: true,
  sdkAvailable: true,
  friends: []
};

const connected: SocialSnapshot = {
  connection: "connected",
  applicationConfigured: true,
  sdkAvailable: true,
  currentUser: {
    id: "101",
    displayName: "DanCold",
    username: "dancold"
  },
  friends: [
    {
      id: "201",
      displayName: "MoraByte",
      username: "morabyte",
      group: "playing",
      statusText: "In a lobby",
      gameTitle: "Robot Rock"
    },
    {
      id: "202",
      displayName: "Nico.exe",
      username: "nico.exe",
      group: "online",
      statusText: "Online"
    },
    {
      id: "203",
      displayName: "Cami",
      username: "cami",
      group: "online",
      statusText: "Playing elsewhere"
    },
    {
      id: "204",
      displayName: "FrostBite",
      username: "frostbite",
      group: "offline",
      statusText: "Offline"
    }
  ],
  activeSession: previewParams.has("social-lobby")
    ? {
        gameId: 1,
        gameTitle: "Robot Rock",
        lobbyId: "eos-demo-lobby",
        partySize: 1,
        partyCapacity: 4,
        joinable: true
      }
    : undefined
};

let snapshot = previewParams.has("social-off") ? disconnected : connected;

const publish = () => updateHandlers.forEach((handler) => handler(snapshot));

export const demoSocialClient: SocialClient = {
  snapshot: async () => snapshot,
  connect: async () => {
    await new Promise((resolve) => window.setTimeout(resolve, 450));
    snapshot = connected;
    publish();
    return snapshot;
  },
  disconnect: async () => {
    snapshot = disconnected;
    publish();
    return snapshot;
  },
  invite: async (friendId) => {
    if (!snapshot.activeSession?.joinable) {
      throw new Error("Open an online lobby before inviting a friend.");
    }
    if (!snapshot.friends.some((friend) => friend.id === friendId)) {
      throw new Error("That Discord friend is no longer available.");
    }
    await new Promise((resolve) => window.setTimeout(resolve, 300));
  },
  onUpdate: async (handler) => {
    updateHandlers.add(handler);
    return (() => updateHandlers.delete(handler)) as UnlistenFn;
  }
};
