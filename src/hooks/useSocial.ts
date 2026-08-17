import { useCallback, useEffect, useState } from "react";
import { socialClient } from "../lib/socialClient";
import type { SocialSnapshot } from "../types/social";

export function useSocial() {
  const [snapshot, setSnapshot] = useState<SocialSnapshot | null>(null);
  const [isConnecting, setIsConnecting] = useState(false);
  const [pendingFriendId, setPendingFriendId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    void socialClient.onUpdate((next) => {
      if (active) setSnapshot(next);
    }).then((cleanup) => {
      if (active) unlisten = cleanup;
      else cleanup();
    });

    void socialClient.snapshot()
      .then((next) => active && setSnapshot(next))
      .catch((reason) => active && setError(String(reason)));

    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  const connect = useCallback(async () => {
    setIsConnecting(true);
    setError(null);
    setNotice(null);
    try {
      setSnapshot(await socialClient.connect());
    } catch (reason) {
      setError(String(reason));
    } finally {
      setIsConnecting(false);
    }
  }, []);

  const disconnect = useCallback(async () => {
    setError(null);
    setNotice(null);
    try {
      setSnapshot(await socialClient.disconnect());
    } catch (reason) {
      setError(String(reason));
    }
  }, []);

  const invite = useCallback(async (friendId: string, displayName: string) => {
    setPendingFriendId(friendId);
    setError(null);
    setNotice(null);
    try {
      await socialClient.invite(friendId);
      setNotice(`Invite sent to ${displayName}.`);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setPendingFriendId(null);
    }
  }, []);

  const dismissJoin = useCallback(async () => {
    setError(null);
    try {
      await socialClient.dismissJoin();
      setSnapshot(await socialClient.snapshot());
    } catch (reason) {
      setError(String(reason));
    }
  }, []);

  const queueJoinForRunningGame = useCallback(async (gameId: number, joinPayload: string) => {
    setError(null);
    try {
      await socialClient.queueJoinForRunningGame(gameId, joinPayload);
      await socialClient.dismissJoin();
      setNotice("Invite queued. Robot Rock will join when it is safe to leave its current session.");
    } catch (reason) {
      setError(String(reason));
    }
  }, []);

  return {
    snapshot,
    isConnecting,
    pendingFriendId,
    error,
    notice,
    connect,
    disconnect,
    invite,
    dismissJoin,
    queueJoinForRunningGame
  };
}
