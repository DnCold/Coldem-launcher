use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    process::Child,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use url::Url;
use tauri::{AppHandle, Emitter};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::timeout,
};

const MAX_BRIDGE_REQUEST: usize = 64 * 1024;

pub struct SocialService {
    active_session: Mutex<Option<SocialSession>>,
    game_bridges: Mutex<HashMap<String, GameBridgeState>>,
    pending_join: Mutex<Option<DiscordJoinRequest>>,
    native_snapshot: Arc<Mutex<NativeSnapshot>>,
    #[cfg(target_os = "windows")]
    native_runtime: Mutex<Option<crate::discord_native::NativeRuntime>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SocialSnapshot {
    connection: String,
    application_configured: bool,
    sdk_available: bool,
    current_user: Option<SocialIdentity>,
    friends: Vec<SocialFriend>,
    active_session: Option<SocialSession>,
    pending_join: Option<DiscordJoinRequest>,
    message: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SocialSession {
    game_id: u64,
    #[serde(skip_serializing)]
    game_slug: String,
    game_title: String,
    lobby_id: String,
    #[serde(skip_serializing)]
    join_secret: String,
    party_size: u32,
    party_capacity: u32,
    joinable: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SocialSessionReport {
    game_slug: String,
    game_title: String,
    lobby_id: String,
    join_secret: String,
    party_size: u32,
    party_capacity: u32,
    joinable: bool,
}

/// V1 carries the existing short EOS lobby code. It is intentionally not presented
/// as authorization: EOS still validates lobby state, protocol, and capacity.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiscordJoinRequest {
    pub(crate) game_slug: String,
    pub(crate) code: String,
    #[serde(skip_serializing)]
    pub(crate) payload: String,
}

#[derive(Default)]
struct GameBridgeState {
    game_id: u64,
    pending_joins: VecDeque<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SocialFriend {
    pub(crate) id: String,
    pub(crate) display_name: String,
    pub(crate) group: &'static str,
    pub(crate) status_text: String,
    pub(crate) avatar_url: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SocialIdentity {
    pub(crate) id: String,
    pub(crate) display_name: String,
    pub(crate) username: Option<String>,
    pub(crate) avatar_url: Option<String>,
}

#[derive(Clone, Default)]
pub(crate) struct NativeSnapshot {
    pub(crate) connection: String,
    pub(crate) current_user: Option<SocialIdentity>,
    pub(crate) friends: Vec<SocialFriend>,
    pub(crate) message: Option<String>,
}

pub struct GameBridgeLaunch {
    pub endpoint: String,
    pub token: String,
}

impl SocialService {
    pub fn new() -> Self {
        Self {
            active_session: Mutex::new(None),
            game_bridges: Mutex::new(HashMap::new()),
            pending_join: Mutex::new(None),
            native_snapshot: Arc::new(Mutex::new(NativeSnapshot {
                connection: "disconnected".into(),
                current_user: None,
                friends: Vec::new(),
                message: Some("Connect Discord to see friends and activity invites.".into()),
            })),
            #[cfg(target_os = "windows")]
            native_runtime: Mutex::new(None),
        }
    }

    fn snapshot(&self) -> SocialSnapshot {
        let application_configured = option_env!("COLDEM_DISCORD_APPLICATION_ID")
            .is_some_and(|value| !value.trim().is_empty());
        let sdk_available = cfg!(target_os = "windows") && discord_runtime_present();
        let native = self
            .native_snapshot
            .lock()
            .ok()
            .map(|value| value.clone())
            .unwrap_or_default();

        SocialSnapshot {
            connection: if !application_configured || !sdk_available {
                "setup_required".into()
            } else {
                native.connection
            },
            application_configured,
            sdk_available,
            current_user: native.current_user,
            friends: native.friends,
            active_session: self
                .active_session
                .lock()
                .ok()
                .and_then(|value| value.clone()),
            pending_join: self.pending_join.lock().ok().and_then(|value| value.clone()),
            message: Some(if !application_configured {
                "The Discord application ID is not configured for this build.".into()
            } else if !sdk_available {
                "Discord Social SDK runtime is not bundled with this build.".into()
            } else {
                native.message.unwrap_or_else(|| {
                    "Connect Discord to see friends and activity invites.".into()
                })
            }),
        }
    }

    #[cfg(target_os = "windows")]
    fn start_native(
        self: &Arc<Self>,
        app: &AppHandle,
        saved_credentials: Option<crate::discord_credentials::DiscordCredentials>,
    ) -> Result<(), String> {
        let application_id = option_env!("COLDEM_DISCORD_APPLICATION_ID")
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or_else(|| {
                "The Discord application ID is not configured for this build.".to_string()
            })?;
        let mut runtime = self
            .native_runtime
            .lock()
            .map_err(|_| "Could not lock the Discord Social SDK runtime".to_string())?;
        if runtime.is_some() {
            return Ok(());
        }
        let weak = Arc::downgrade(self);
        let app = app.clone();
        let emit_app = app.clone();
        let emit = Arc::new(move || {
            if let Some(service) = weak.upgrade() {
                service.emit_snapshot(&emit_app);
            }
        });
        let credentials_app = app.clone();
        let remember_credentials = Arc::new(move |credentials| {
            let _ = crate::discord_credentials::save(&credentials_app, &credentials);
        });
        let clear_app = app.clone();
        let clear_credentials = Arc::new(move || {
            let _ = crate::discord_credentials::clear(&clear_app);
        });
        let join_service = Arc::downgrade(self);
        let join_app = app.clone();
        let receive_join = Arc::new(move |payload: String| {
            if let Some(service) = join_service.upgrade() {
                if let Err(error) = service.receive_join_payload(&join_app, &payload) {
                    if let Ok(mut native) = service.native_snapshot.lock() {
                        native.message = Some(error);
                    }
                    service.emit_snapshot(&join_app);
                }
            }
        });
        *runtime = Some(crate::discord_native::NativeRuntime::start(
            application_id,
            self.native_snapshot.clone(),
            emit,
            saved_credentials,
            remember_credentials,
            clear_credentials,
            receive_join,
        )?);
        drop(runtime);
        self.sync_activity();
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    fn start_native(self: &Arc<Self>, _app: &AppHandle) -> Result<(), String> {
        Err("Discord Social SDK is currently available only in the Windows build.".into())
    }

    #[cfg(target_os = "windows")]
    pub fn restore_saved_session(self: &Arc<Self>, app: &AppHandle) -> Result<(), String> {
        let Some(credentials) = crate::discord_credentials::load(app)? else {
            return Ok(());
        };
        if let Ok(mut snapshot) = self.native_snapshot.lock() {
            snapshot.connection = "connecting".into();
            snapshot.message = Some("Restoring your saved Discord session...".into());
        }
        self.start_native(app, Some(credentials))
    }

    fn send_native_command(
        &self,
        command: crate::discord_native::NativeCommand,
    ) -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            let runtime = self
                .native_runtime
                .lock()
                .map_err(|_| "Could not lock the Discord Social SDK runtime".to_string())?;
            runtime
                .as_ref()
                .ok_or_else(|| "Discord Social SDK is not running yet.".to_string())?
                .send(command)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = command;
            Err("Discord Social SDK is currently available only in the Windows build.".into())
        }
    }

    pub async fn prepare_game_bridge(
        self: &Arc<Self>,
        app: AppHandle,
        game_id: u64,
    ) -> Result<GameBridgeLaunch, String> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|error| format!("Could not start the local social bridge: {error}"))?;
        let address = listener
            .local_addr()
            .map_err(|error| format!("Could not read the local social bridge address: {error}"))?;
        let token = random_token()?;

        self.game_bridges
            .lock()
            .map_err(|_| "Could not lock the social bridge registry".to_string())?
            .insert(
                token.clone(),
                GameBridgeState {
                    game_id,
                    pending_joins: VecDeque::new(),
                },
            );

        let service = Arc::clone(self);
        let server_token = token.clone();
        tokio::spawn(async move {
            loop {
                if !service.bridge_is_active(&server_token, game_id) {
                    break;
                }

                match timeout(Duration::from_secs(2), listener.accept()).await {
                    Ok(Ok((stream, _))) => {
                        let service = Arc::clone(&service);
                        let app = app.clone();
                        let token = server_token.clone();
                        tokio::spawn(async move {
                            let _ =
                                handle_bridge_request(stream, service, app, game_id, token).await;
                        });
                    }
                    Ok(Err(_)) => break,
                    Err(_) => continue,
                }
            }
        });

        Ok(GameBridgeLaunch {
            endpoint: format!("http://{address}"),
            token,
        })
    }

    pub fn watch_game_process<F>(
        self: &Arc<Self>,
        app: AppHandle,
        game_id: u64,
        token: String,
        mut child: Child,
        started_at: Instant,
        on_exit: F,
    ) where
        F: FnOnce(Duration) + Send + 'static,
    {
        let service = Arc::clone(self);
        std::thread::spawn(move || {
            let _ = child.wait();
            on_exit(started_at.elapsed());
            service.revoke_game_bridge(&app, game_id, &token);
        });
    }

    pub fn revoke_game_bridge(&self, app: &AppHandle, game_id: u64, token: &str) {
        if let Ok(mut bridges) = self.game_bridges.lock() {
            bridges.remove(token);
        }
        if let Ok(mut session) = self.active_session.lock() {
            if session
                .as_ref()
                .is_some_and(|value| value.game_id == game_id)
            {
                *session = None;
            }
        }
        self.clear_activity();
        self.emit_snapshot(app);
    }

    fn bridge_is_active(&self, token: &str, game_id: u64) -> bool {
        self.game_bridges
            .lock()
            .ok()
            .and_then(|bridges| bridges.get(token).map(|bridge| bridge.game_id))
            == Some(game_id)
    }

    fn sync_activity(&self) {
        let session = self
            .active_session
            .lock()
            .ok()
            .and_then(|value| value.clone());
        let Some(session) = session else {
            self.clear_activity();
            return;
        };
        let _ = self.send_native_command(crate::discord_native::NativeCommand::PublishActivity {
            game_title: session.game_title,
            lobby_id: session.lobby_id,
            join_secret: session.join_secret,
            party_size: session.party_size,
            party_capacity: session.party_capacity,
            joinable: session.joinable,
        });
    }

    fn clear_activity(&self) {
        let _ = self.send_native_command(crate::discord_native::NativeCommand::ClearActivity);
    }

    fn receive_join_payload(&self, app: &AppHandle, payload: &str) -> Result<(), String> {
        let request = parse_join_payload(payload)?;
        *self
            .pending_join
            .lock()
            .map_err(|_| "Could not store the Discord join request".to_string())? = Some(request);
        self.emit_snapshot(app);
        Ok(())
    }

    pub(crate) fn receive_deep_link(&self, app: &AppHandle, url: &Url) -> Result<(), String> {
        let secret = discord_join_secret_from_url(url)?;
        self.receive_join_payload(app, &secret)
    }

    pub(crate) fn receive_deep_link_arguments(
        &self,
        app: &AppHandle,
        arguments: impl IntoIterator<Item = String>,
    ) {
        for argument in arguments {
            let Ok(url) = Url::parse(&argument) else {
                continue;
            };
            if let Err(error) = self.receive_deep_link(app, &url) {
                if let Ok(mut native) = self.native_snapshot.lock() {
                    native.message = Some(error);
                }
                self.emit_snapshot(app);
            }
        }
    }

    pub(crate) fn queue_join_for_game(&self, game_id: u64, payload: &str) -> Result<(), String> {
        parse_join_payload(payload)?;
        let mut bridges = self
            .game_bridges
            .lock()
            .map_err(|_| "Could not access the running game bridge".to_string())?;
        let bridge = bridges
            .values_mut()
            .find(|bridge| bridge.game_id == game_id)
            .ok_or_else(|| "Robot Rock is not currently running through Coldem.".to_string())?;
        bridge.pending_joins.push_back(payload.to_owned());
        Ok(())
    }

    pub(crate) fn pending_join(&self) -> Option<DiscordJoinRequest> {
        self.pending_join.lock().ok().and_then(|value| value.clone())
    }

    pub(crate) fn dismiss_pending_join(&self, app: &AppHandle) {
        if let Ok(mut pending) = self.pending_join.lock() {
            *pending = None;
        }
        self.emit_snapshot(app);
    }

    fn emit_snapshot(&self, app: &AppHandle) {
        let _ = app.emit("launcher://social", self.snapshot());
    }
}

#[cfg(target_os = "windows")]
fn discord_runtime_present() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.parent()
                .map(|parent| parent.join("discord_partner_sdk.dll"))
        })
        .is_some_and(|path| path.exists())
}

#[cfg(not(target_os = "windows"))]
fn discord_runtime_present() -> bool {
    false
}

#[tauri::command]
pub fn social_snapshot(state: tauri::State<'_, crate::AppState>) -> SocialSnapshot {
    state.social.snapshot()
}

#[tauri::command]
pub fn connect_discord(
    app: AppHandle,
    state: tauri::State<'_, crate::AppState>,
) -> Result<SocialSnapshot, String> {
    let snapshot = state.social.snapshot();
    if !snapshot.application_configured {
        return Err("The Discord application ID is not configured for this build.".into());
    }
    if !snapshot.sdk_available {
        return Err("The Discord Social SDK runtime is not bundled with this build.".into());
    }
    state.social.start_native(&app, None)?;
    state
        .social
        .send_native_command(crate::discord_native::NativeCommand::Connect)?;
    Ok(snapshot)
}

#[tauri::command]
pub fn disconnect_discord(
    app: AppHandle,
    state: tauri::State<'_, crate::AppState>,
) -> Result<SocialSnapshot, String> {
    #[cfg(target_os = "windows")]
    crate::discord_credentials::clear(&app)?;
    let _ = state
        .social
        .send_native_command(crate::discord_native::NativeCommand::Disconnect);
    Ok(state.social.snapshot())
}

#[tauri::command]
pub fn invite_discord_friend(
    friend_id: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), String> {
    let session = state
        .social
        .active_session
        .lock()
        .map_err(|_| "Could not lock the active EOS session".to_string())?
        .clone()
        .ok_or_else(|| "Start a joinable EOS lobby before sending a Discord invite.".to_string())?;
    if !session.joinable {
        return Err("The current EOS lobby is not joinable.".into());
    }
    state
        .social
        .send_native_command(crate::discord_native::NativeCommand::Invite {
            friend_id,
            content: format!("Join me in {}", session.game_title),
        })
}

#[tauri::command]
pub fn pending_discord_join(state: tauri::State<'_, crate::AppState>) -> Option<DiscordJoinRequest> {
    state.social.pending_join()
}

#[tauri::command]
pub fn dismiss_discord_join(app: AppHandle, state: tauri::State<'_, crate::AppState>) {
    state.social.dismiss_pending_join(&app);
}

#[tauri::command]
pub fn queue_discord_join_for_running_game(
    game_id: u64,
    join_payload: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), String> {
    state.social.queue_join_for_game(game_id, &join_payload)
}

async fn handle_bridge_request(
    mut stream: TcpStream,
    service: Arc<SocialService>,
    app: AppHandle,
    game_id: u64,
    expected_token: String,
) -> Result<(), String> {
    let request = read_http_request(&mut stream).await?;
    let (request_line, headers, body) = split_http_request(&request)?;
    if !bridge_request_is_authorized(&service, headers, &expected_token, game_id) {
        write_http_response(&mut stream, 401, "Unauthorized").await;
        return Ok(());
    }

    if request_line == "GET /join HTTP/1.1" {
        let payload = service
            .game_bridges
            .lock()
            .ok()
            .and_then(|mut bridges| {
                bridges
                    .get_mut(&expected_token)
                    .filter(|bridge| bridge.game_id == game_id)
                    .and_then(|bridge| bridge.pending_joins.pop_front())
            });
        if let Some(payload) = payload {
            write_json_response(&mut stream, 200, &serde_json::json!({ "joinSecret": payload })).await;
        } else {
            write_http_response(&mut stream, 204, "").await;
        }
        return Ok(());
    }

    if request_line == "DELETE /session HTTP/1.1" {
        if let Ok(mut session) = service.active_session.lock() {
            if session
                .as_ref()
                .is_some_and(|value| value.game_id == game_id)
            {
                *session = None;
            }
        }
        service.clear_activity();
        service.emit_snapshot(&app);
        write_http_response(&mut stream, 204, "").await;
        return Ok(());
    }

    if request_line != "POST /session HTTP/1.1" {
        write_http_response(&mut stream, 404, "Not found").await;
        return Ok(());
    }

    let report: SocialSessionReport = serde_json::from_slice(body)
        .map_err(|error| format!("Invalid social session report: {error}"))?;
    let session = SocialSession {
        game_id,
        game_slug: report.game_slug,
        game_title: report.game_title,
        lobby_id: report.lobby_id,
        join_secret: report.join_secret,
        party_size: report.party_size,
        party_capacity: report.party_capacity,
        joinable: report.joinable,
    };
    if let Err(error) = validate_session(&session) {
        write_http_response(&mut stream, 400, &error).await;
        return Ok(());
    }

    *service
        .active_session
        .lock()
        .map_err(|_| "Could not lock the social session".to_string())? = Some(session);
    service.sync_activity();
    service.emit_snapshot(&app);
    write_http_response(&mut stream, 204, "").await;
    Ok(())
}

async fn read_http_request(stream: &mut TcpStream) -> Result<Vec<u8>, String> {
    let mut request = Vec::with_capacity(2048);
    let mut buffer = [0_u8; 2048];
    loop {
        let read = timeout(Duration::from_secs(3), stream.read(&mut buffer))
            .await
            .map_err(|_| "The social bridge request timed out".to_string())?
            .map_err(|error| format!("Could not read the social bridge request: {error}"))?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        if request.len() > MAX_BRIDGE_REQUEST {
            return Err("The social bridge request is too large".into());
        }
        if let Some(header_end) = find_bytes(&request, b"\r\n\r\n") {
            let headers = &request[..header_end];
            let content_length = std::str::from_utf8(headers)
                .ok()
                .and_then(|value| header_value(value, "content-length"))
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(0);
            if request.len() >= header_end + 4 + content_length {
                break;
            }
        }
    }
    Ok(request)
}

fn split_http_request(request: &[u8]) -> Result<(&str, &str, &[u8]), String> {
    let header_end = find_bytes(request, b"\r\n\r\n")
        .ok_or_else(|| "The social bridge request has no headers".to_string())?;
    let headers = std::str::from_utf8(&request[..header_end])
        .map_err(|_| "The social bridge headers are not valid UTF-8".to_string())?;
    let request_line = headers
        .lines()
        .next()
        .ok_or_else(|| "The social bridge request line is missing".to_string())?;
    Ok((request_line, headers, &request[header_end + 4..]))
}

fn header_value<'a>(headers: &'a str, name: &str) -> Option<&'a str> {
    headers.lines().skip(1).find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.eq_ignore_ascii_case(name).then(|| value.trim())
    })
}

fn bridge_request_is_authorized(
    service: &SocialService,
    headers: &str,
    expected_token: &str,
    game_id: u64,
) -> bool {
    header_value(headers, "authorization")
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|token| token == expected_token)
        && service.bridge_is_active(expected_token, game_id)
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

async fn write_http_response(stream: &mut TcpStream, status: u16, body: &str) {
    let label = match status {
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        _ => "OK",
    };
    let response = format!(
        "HTTP/1.1 {status} {label}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

async fn write_json_response(stream: &mut TcpStream, status: u16, body: &serde_json::Value) {
    let encoded = body.to_string();
    let label = match status {
        200 => "OK",
        _ => "OK",
    };
    let response = format!(
        "HTTP/1.1 {status} {label}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{encoded}",
        encoded.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

fn random_token() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|error| format!("Could not create a social bridge token: {error}"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn validate_session(session: &SocialSession) -> Result<(), String> {
    if session.game_slug != "robot-rock-reborn" || session.game_title.trim().is_empty() {
        return Err("The social session requires a supported game title.".into());
    }
    if session.joinable {
        if session.lobby_id.trim().is_empty() {
            return Err("A joinable social session requires an EOS lobby ID.".into());
        }
        parse_join_payload(&session.join_secret)?;
        if session.party_capacity == 0 || session.party_size > session.party_capacity {
            return Err("The social party size is invalid.".into());
        }
    }
    Ok(())
}

pub(crate) fn parse_join_payload(payload: &str) -> Result<DiscordJoinRequest, String> {
    let mut pieces = payload.split(':');
    let (Some(prefix), Some(version), Some(game_slug), Some(code), None) = (
        pieces.next(),
        pieces.next(),
        pieces.next(),
        pieces.next(),
        pieces.next(),
    ) else {
        return Err("The Discord join payload has an unsupported format.".into());
    };
    if prefix != "coldem" || version != "v1" || game_slug != "robot-rock-reborn" {
        return Err("This Discord invite targets an unsupported Coldem game.".into());
    }
    let normalized = code.trim().to_ascii_uppercase();
    const ALPHABET: &str = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    if normalized.len() != 6 || !normalized.chars().all(|value| ALPHABET.contains(value)) {
        return Err("The Robot Rock lobby code in this invite is invalid.".into());
    }
    Ok(DiscordJoinRequest {
        game_slug: game_slug.into(),
        code: normalized,
        payload: payload.into(),
    })
}

fn discord_join_secret_from_url(url: &Url) -> Result<String, String> {
    if url.scheme() != "coldem" || url.host_str() != Some("discord") || url.path() != "/join" {
        return Err("This Coldem link is not a Discord game invite.".into());
    }
    url.query_pairs()
        .find_map(|(key, value)| (key == "secret").then(|| value.into_owned()))
        .ok_or_else(|| "The Discord invite link did not include a join payload.".to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        bridge_request_is_authorized, discord_join_secret_from_url, parse_join_payload,
        GameBridgeState, SocialService,
    };
    use std::collections::VecDeque;
    use url::Url;

    #[test]
    fn parses_robot_rock_join_payload() {
        let request = parse_join_payload("coldem:v1:robot-rock-reborn:AB2CD3").unwrap();
        assert_eq!(request.game_slug, "robot-rock-reborn");
        assert_eq!(request.code, "AB2CD3");
    }

    #[test]
    fn rejects_untrusted_join_payload() {
        assert!(parse_join_payload("coldem:v1:other-game:AB2CD3").is_err());
        assert!(parse_join_payload("coldem:v1:robot-rock-reborn:OOOOOO").is_err());
        assert!(parse_join_payload("coldem:v1:robot-rock-reborn:AB2CD3:extra").is_err());
    }

    #[test]
    fn parses_only_the_expected_discord_deep_link() {
        let accepted = Url::parse("coldem://discord/join?secret=coldem%3Av1%3Arobot-rock-reborn%3AAB2CD3")
            .unwrap();
        assert_eq!(
            discord_join_secret_from_url(&accepted).unwrap(),
            "coldem:v1:robot-rock-reborn:AB2CD3"
        );

        let wrong_path = Url::parse("coldem://discord/not-a-join?secret=value").unwrap();
        assert!(discord_join_secret_from_url(&wrong_path).is_err());
        let missing_secret = Url::parse("coldem://discord/join").unwrap();
        assert!(discord_join_secret_from_url(&missing_secret).is_err());
    }

    #[test]
    fn queued_join_is_consumed_once() {
        let mut bridge = GameBridgeState {
            game_id: 7,
            pending_joins: VecDeque::from(["coldem:v1:robot-rock-reborn:AB2CD3".to_string()]),
        };
        assert_eq!(
            bridge.pending_joins.pop_front().as_deref(),
            Some("coldem:v1:robot-rock-reborn:AB2CD3")
        );
        assert!(bridge.pending_joins.pop_front().is_none());
    }

    #[test]
    fn bridge_rejects_missing_or_wrong_tokens() {
        let service = SocialService::new();
        service.game_bridges.lock().unwrap().insert(
            "expected-token".to_string(),
            GameBridgeState {
                game_id: 9,
                pending_joins: VecDeque::new(),
            },
        );
        assert!(bridge_request_is_authorized(
            &service,
            "GET /join HTTP/1.1\r\nAuthorization: Bearer expected-token",
            "expected-token",
            9
        ));
        assert!(!bridge_request_is_authorized(
            &service,
            "GET /join HTTP/1.1\r\nAuthorization: Bearer wrong-token",
            "expected-token",
            9
        ));
        assert!(!bridge_request_is_authorized(
            &service,
            "GET /join HTTP/1.1",
            "expected-token",
            9
        ));
    }
}
