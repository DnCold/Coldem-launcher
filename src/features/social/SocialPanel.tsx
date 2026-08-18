import {
  CircleUserRound,
  Gamepad2,
  LoaderCircle,
  LogOut,
  MessageCircle,
  Send,
  UsersRound
} from "lucide-react";
import type { useSocial } from "../../hooks/useSocial";
import type { SocialFriend, SocialFriendGroup } from "../../types/social";

type SocialState = ReturnType<typeof useSocial>;

interface SocialPanelProps {
  social: SocialState;
  nowPlaying?: string;
  sessionSeconds?: number;
}

const formatLiveSession = (seconds = 0) => {
  const minutes = Math.floor(seconds / 60);
  return minutes ? `${minutes}m in session` : "just launched";
};

const groupLabels: Record<SocialFriendGroup, string> = {
  playing: "Playing Coldem",
  online: "Online elsewhere",
  offline: "Offline"
};

const initials = (name: string) =>
  name.split(/\s+/).slice(0, 2).map((part) => part[0]).join("").toUpperCase();

function FriendRow({
  friend,
  canInvite,
  pending,
  onInvite
}: {
  friend: SocialFriend;
  canInvite: boolean;
  pending: boolean;
  onInvite: () => void;
}) {
  return (
    <li className={`social-friend social-friend--${friend.group}`}>
      <div className="social-avatar">
        {friend.avatarUrl ? <img src={friend.avatarUrl} alt="" /> : <span>{initials(friend.displayName)}</span>}
        <i aria-label={friend.group === "offline" ? "Offline" : "Online"} />
      </div>
      <div className="social-friend__identity">
        <strong>{friend.displayName}</strong>
        <small>{friend.gameTitle || friend.statusText}</small>
      </div>
      <button
        type="button"
        className="social-invite"
        onClick={onInvite}
        disabled={!canInvite || pending}
        title={canInvite ? `Invite ${friend.displayName}` : "Open an EOS lobby to send invites"}
        aria-label={`Invite ${friend.displayName}`}
      >
        {pending ? <LoaderCircle className="spin" size={13} /> : <Send size={13} />}
      </button>
    </li>
  );
}

export function SocialPanel({ social, nowPlaying, sessionSeconds }: SocialPanelProps) {
  const snapshot = social.snapshot;
  const connected = snapshot?.connection === "connected";
  const canInvite = Boolean(snapshot?.activeSession?.joinable);
  const friends = snapshot?.friends ?? [];
  const grouped = (["playing", "online", "offline"] as const)
    .map((group) => ({ group, friends: friends.filter((friend) => friend.group === group) }))
    .filter(({ friends }) => friends.length > 0);

  return (
    <section className="social-panel" aria-label="Discord friends">
      <div className="social-panel__heading">
        <div><MessageCircle size={16} /><h2>Discord</h2></div>
        {connected && <span title={`${friends.filter((friend) => friend.group !== "offline").length} online`}>{friends.length}</span>}
      </div>

      {nowPlaying && (
        <div className="social-now-playing">
          <span><i /> NOW PLAYING</span>
          <strong>{nowPlaying}</strong>
          <small>{formatLiveSession(sessionSeconds)} · invite-ready when the game opens an EOS lobby</small>
        </div>
      )}

      {!snapshot ? (
        <div className="social-panel__empty"><LoaderCircle className="spin" size={18} /> Loading social...</div>
      ) : connected ? (
        <>
          <div className={`social-session ${canInvite ? "social-session--live" : ""}`}>
            <Gamepad2 size={15} />
            <div>
              <strong>{canInvite
                ? snapshot.activeSession?.gameTitle
                : snapshot.activeSession
                  ? `${snapshot.activeSession.gameTitle} open`
                  : "No online lobby"}</strong>
              <small>{canInvite
                ? `${snapshot.activeSession?.partySize}/${snapshot.activeSession?.partyCapacity} · EOS lobby ready`
                : snapshot.activeSession
                  ? "Playing now · Discord activity is live"
                  : "Start an online lobby to invite friends"}</small>
            </div>
          </div>

          <div className="social-beacon" aria-label={canInvite ? "EOS invite relay active" : "EOS invite relay standing by"}>
            <span><i /> EOS RELAY</span>
            <b>{canInvite ? "INVITES LIVE" : "STANDING BY"}</b>
          </div>

          {grouped.length === 0 ? (
            <div className="social-panel__empty"><UsersRound size={20} /> No friends to show yet.</div>
          ) : (
            <div className="social-friends-list" aria-label="All Discord friends">
              {grouped.map(({ group, friends: groupFriends }) => (
                <div className="social-group" key={group}>
                  <p>{groupLabels[group]} <span>{groupFriends.length}</span></p>
                  <ul>
                    {groupFriends.map((friend) => (
                      <FriendRow
                        key={friend.id}
                        friend={friend}
                        canInvite={canInvite && friend.group !== "offline"}
                        pending={social.pendingFriendId === friend.id}
                        onInvite={() => void social.invite(friend.id, friend.displayName)}
                      />
                    ))}
                  </ul>
                </div>
              ))}
            </div>
          )}

          <button type="button" className="social-disconnect" onClick={() => void social.disconnect()}>
            <LogOut size={12} /> Disconnect &amp; forget Discord
          </button>
        </>
      ) : (
        <div className="social-connect">
          <CircleUserRound size={28} />
          <strong>{snapshot.connection === "setup_required" ? "Social layer coming online" : "Play with your people"}</strong>
          <p>{snapshot.message || "Connect Discord to see friends and invite them to your EOS lobby."}</p>
          {snapshot.connection !== "setup_required" && (
            <button type="button" onClick={() => void social.connect()} disabled={social.isConnecting}>
              {social.isConnecting ? <LoaderCircle className="spin" size={14} /> : <MessageCircle size={14} />}
              {social.isConnecting ? "Connecting..." : "Connect Discord"}
            </button>
          )}
        </div>
      )}

      {(social.error || social.notice) && (
        <button type="button" className={`social-feedback ${social.error ? "social-feedback--error" : ""}`}>
          {social.error || social.notice}
        </button>
      )}
    </section>
  );
}
