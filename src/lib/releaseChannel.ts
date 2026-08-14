import type { ReleaseChannel } from "../types/launcher";

export const RELEASE_CHANNEL_KEY = "coldem-release-channel-v1";

export const readReleaseChannel = (): ReleaseChannel => {
  try {
    return window.localStorage.getItem(RELEASE_CHANNEL_KEY) === "test" ? "test" : "stable";
  } catch {
    return "stable";
  }
};

export const saveReleaseChannel = (channel: ReleaseChannel) => {
  try {
    window.localStorage.setItem(RELEASE_CHANNEL_KEY, channel);
  } catch {
    // The launcher can still use the selected channel for this session.
  }
};
