export type SocialConnectionState =
  | "setup_required"
  | "disconnected"
  | "connecting"
  | "connected"
  | "error";

export type SocialFriendGroup = "playing" | "online" | "offline";

export interface SocialIdentity {
  id: string;
  displayName: string;
  username?: string;
  avatarUrl?: string;
}

export interface SocialFriend extends SocialIdentity {
  group: SocialFriendGroup;
  statusText: string;
  gameTitle?: string;
}

export interface SocialSession {
  gameId: number;
  gameTitle: string;
  lobbyId: string;
  partySize: number;
  partyCapacity: number;
  joinable: boolean;
}

export interface SocialSnapshot {
  connection: SocialConnectionState;
  applicationConfigured: boolean;
  sdkAvailable: boolean;
  currentUser?: SocialIdentity;
  friends: SocialFriend[];
  activeSession?: SocialSession;
  message?: string;
}
