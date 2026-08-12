# Coldem social integration

Coldem uses two independent online layers:

- **Discord Social SDK** owns Discord account linking, the unified friends list,
  presence, and activity invitations.
- **Epic Online Services (EOS)** stays inside each game and owns authentication,
  lobbies, sessions, P2P relay, and joining the actual match.

The ordinary Discord OAuth REST API must not be used as a substitute for the
Social SDK: it does not expose a player's complete Discord friends list.

## Invite flow

1. A game creates or joins an EOS lobby.
2. Coldem starts that game with `COLDEM_SOCIAL_ENDPOINT` and a random,
   one-process `COLDEM_SOCIAL_TOKEN` in its environment.
3. The game reports a short-lived session envelope with an authenticated
   `POST /session` to that loopback endpoint. It clears the lobby with
   `DELETE /session`.
4. Coldem publishes Discord Rich Presence with party size and an opaque join
   secret, then enables **Invite** beside online friends.
5. Coldem calls `SendActivityInvite` for the selected Discord relationship.
6. When the recipient accepts, Coldem starts the correct installed game and
   passes the opaque envelope once.
7. The game validates the envelope and joins the referenced EOS lobby.

The loopback server accepts only the random bearer token passed to the game,
revokes it when that exact process exits, limits request size, and never sends
the EOS join secret to React. The envelope may identify a game and lobby, but
it must never contain EOS client secrets, Discord tokens, or other permanent
credentials. Do not persist or log it. Expire it when the lobby closes or
becomes full.

## Native adapter boundary

The React UI talks only to `src/lib/socialClient.ts`. The Tauri commands in
`src-tauri/src/social.rs` are the native boundary. The Discord C++ Social SDK
adapter will live behind those commands, so game delivery and UI state do not
depend directly on Discord headers or DLLs.

Production games must use the process-bound loopback bridge. There is no
React/Tauri command that can inject a lobby or join secret into the service.

Example request from a game after EOS creates its lobby:

```http
POST /session HTTP/1.1
Host: 127.0.0.1
Authorization: Bearer <COLDEM_SOCIAL_TOKEN>
Content-Type: application/json

{"gameTitle":"Robot Rock","lobbyId":"...","joinSecret":"...","partySize":1,"partyCapacity":4,"joinable":true}
```

Required production setup:

1. Create a Discord Developer Team and a Discord application for Coldem.
2. Enable Discord Social SDK for the application.
3. Register `http://127.0.0.1/callback` as the desktop OAuth redirect.
4. Request only the default presence scopes (`openid` and
   `sdk.social_layer_presence`) for account linking, friends, presence, and
   activity invites.
5. Download the C++ Social SDK from that application's Developer Portal page.
6. Bundle `discord_partner_sdk.dll` beside the Windows executable and compile
   the adapter with `COLDEM_DISCORD_APPLICATION_ID` set to the numeric
   Application ID.

Use separate Discord applications per game only if each game publishes its own
presence. Publisher-level account linking can share one authorization between a
launcher and child games, but Discord must configure that parent-child setup.
