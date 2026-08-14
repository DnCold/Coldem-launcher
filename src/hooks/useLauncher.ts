import { useCallback, useEffect, useRef, useState } from "react";
import { launcherClient } from "../lib/launcherClient";
import type {
  ActiveOperation,
  BootstrapResult,
  ButlerPrompt,
  Game,
  GameUpdate,
  InstallOptions,
  InstallPlan,
  LibrarySnapshot,
  Profile,
  ReleaseChannel,
  Upload
} from "../types/launcher";
import { readReleaseChannel, saveReleaseChannel } from "../lib/releaseChannel";

type AppStatus = "booting" | "login" | "ready" | "error";

export function useLauncher() {
  const [status, setStatus] = useState<AppStatus>("booting");
  const [bootstrap, setBootstrap] = useState<BootstrapResult | null>(null);
  const [channel, setChannelState] = useState<ReleaseChannel>(readReleaseChannel);
  const [profile, setProfile] = useState<Profile | null>(null);
  const [library, setLibrary] = useState<LibrarySnapshot | null>(null);
  const [libraryError, setLibraryError] = useState<string | null>(null);
  const [updates, setUpdates] = useState<GameUpdate[]>([]);
  const [operations, setOperations] = useState<Record<number, ActiveOperation>>({});
  const [prompt, setPrompt] = useState<ButlerPrompt | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [authError, setAuthError] = useState<string | null>(null);
  const [isAuthenticating, setIsAuthenticating] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const profileRef = useRef<Profile | null>(null);

  useEffect(() => {
    profileRef.current = profile;
  }, [profile]);

  const loadLibrary = useCallback(async (activeProfile: Profile, fresh = false) => {
    if (fresh) setIsRefreshing(true);
    setLibraryError(null);
    try {
      const snapshot = await launcherClient.fetchLibrary(activeProfile.id, fresh);
      setLibrary(snapshot);
      if ((fresh || snapshot.stale) && snapshot.caves.length > 0) {
        try {
          const foundUpdates = await launcherClient.checkUpdates();
          setUpdates(foundUpdates);
        } catch (reason) {
          setLibraryError(`Games loaded, but updates could not be checked: ${String(reason)}`);
        }
      }
      return snapshot;
    } catch (reason) {
      setLibraryError(String(reason));
      throw reason;
    } finally {
      if (fresh) setIsRefreshing(false);
    }
  }, []);

  const activateProfile = useCallback(
    async (nextProfile: Profile) => {
      setProfile(nextProfile);
      profileRef.current = nextProfile;
      setStatus("ready");
      setError(null);
      setAuthError(null);
      setIsAuthenticating(false);
      try {
        await loadLibrary(nextProfile, false);
        void loadLibrary(nextProfile, true).catch((reason) => {
          console.warn("Fresh library sync failed", reason);
        });
      } catch (reason) {
        console.warn("Initial library load failed", reason);
      }
    },
    [loadLibrary]
  );

  useEffect(() => {
    let active = true;
    const unlisteners: Array<() => void> = [];

    void launcherClient.onOperation((event) => {
      if (!active) return;
      setOperations((current) => ({
        ...current,
        [event.gameId]: {
          ...event,
          startedAt: current[event.gameId]?.startedAt ?? Date.now()
        }
      }));

      if (event.state === "finished" && profileRef.current) {
        void loadLibrary(profileRef.current, true);
      }
    }).then((unlisten) => unlisteners.push(unlisten));

    void launcherClient.onPrompt((nextPrompt) => {
      if (active) setPrompt(nextPrompt);
    }).then((unlisten) => unlisteners.push(unlisten));

    void (async () => {
      try {
        const initialChannel = readReleaseChannel();
        setChannelState(initialChannel);
        const result = await launcherClient.initialize(initialChannel);
        if (!active) return;
        setBootstrap(result);
        setChannelState(result.channel);
        saveReleaseChannel(result.channel);

        const remembered = [...result.profiles].sort(
          (a, b) => Date.parse(b.lastConnected) - Date.parse(a.lastConnected)
        );
        if (remembered[0]) {
          try {
            const restored = await launcherClient.restoreProfile(remembered[0].id);
            if (active) await activateProfile(restored);
            return;
          } catch {
            // Saved credentials can expire; fall through to login.
          }
        }
        setStatus("login");
      } catch (reason) {
        if (!active) return;
        setError(String(reason));
        setStatus("error");
      }
    })();

    return () => {
      active = false;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [activateProfile, loadLibrary]);

  const login = useCallback(async () => {
    if (isAuthenticating) return;
    setIsAuthenticating(true);
    setAuthError(null);
    try {
      const immediateProfile = await launcherClient.beginLogin();
      if (immediateProfile) await activateProfile(immediateProfile);
    } catch (reason) {
      setAuthError(String(reason));
      setIsAuthenticating(false);
    }
  }, [activateProfile, isAuthenticating]);

  const cancelLogin = useCallback(async () => {
    await launcherClient.cancelLogin();
    setIsAuthenticating(false);
    setAuthError(null);
  }, []);

  const logout = useCallback(async () => {
    if (profile) await launcherClient.forgetProfile(profile.id);
    setProfile(null);
    setLibrary(null);
    setLibraryError(null);
    setUpdates([]);
    setOperations({});
    setAuthError(null);
    setIsAuthenticating(false);
    setStatus("login");
  }, [profile]);

  const setChannel = useCallback(
    async (nextChannel: ReleaseChannel) => {
      if (nextChannel === channel) return;
      const previousChannel = channel;
      setChannelState(nextChannel);
      saveReleaseChannel(nextChannel);
      setLibraryError(null);
      setUpdates([]);
      setIsRefreshing(true);
      try {
        await launcherClient.setChannel(nextChannel);
        if (profileRef.current) {
          await loadLibrary(profileRef.current, true);
        }
      } catch (reason) {
        try {
          await launcherClient.setChannel(previousChannel);
        } catch {
          // Keep the original error visible; the next bootstrap will restore the persisted channel.
        }
        setChannelState(previousChannel);
        saveReleaseChannel(previousChannel);
        setLibraryError(String(reason));
        throw reason;
      } finally {
        setIsRefreshing(false);
      }
    },
    [channel, loadLibrary]
  );

  const prepareInstall = useCallback(
    (gameId: number): Promise<InstallOptions> => {
      if (!profile) throw new Error("No active profile");
      return launcherClient.prepareInstall(gameId, profile.id);
    },
    [profile]
  );

  const planInstall = useCallback(
    (uploadId: number): Promise<InstallPlan> => launcherClient.planInstall(uploadId),
    []
  );

  const install = useCallback(
    async (
      game: Game,
      upload: Upload,
      installLocationId: string
    ) => {
      if (!profile) return;
      await launcherClient.install(profile.id, game, upload, installLocationId);
    },
    [profile]
  );

  const update = useCallback(
    async (caveId: string, gameId: number) => {
      if (!profile) return;
      await launcherClient.update(profile.id, caveId, gameId);
    },
    [profile]
  );

  const play = useCallback(
    async (caveId: string, gameId: number) => {
      if (!profile) return;
      await launcherClient.play(profile.id, caveId, gameId);
    },
    [profile]
  );

  const respondToPrompt = useCallback(
    async (result: Record<string, unknown>) => {
      if (!prompt) return;
      const current = prompt;
      setPrompt(null);
      await launcherClient.respondToPrompt(current.id, result);
    },
    [prompt]
  );

  return {
    status,
    bootstrap,
    channel,
    profile,
    library,
    libraryError,
    updates,
    operations,
    prompt,
    error,
    authError,
    isAuthenticating,
    isRefreshing,
    login,
    cancelLogin,
    logout,
    refresh: () => profile && loadLibrary(profile, true),
    setChannel,
    prepareInstall,
    planInstall,
    install,
    update,
    play,
    respondToPrompt,
    openExternal: launcherClient.openExternal
  };
}
