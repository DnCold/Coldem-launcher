use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    process::Child,
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{AppHandle, Emitter};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    time::timeout,
};

const MAX_BRIDGE_REQUEST: usize = 64 * 1024;

#[derive(Default)]
pub struct SocialService {
    active_session: Mutex<Option<SocialSession>>,
    game_bridges: Mutex<HashMap<String, u64>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SocialSnapshot {
    connection: &'static str,
    application_configured: bool,
    sdk_available: bool,
    friends: Vec<SocialFriend>,
    active_session: Option<SocialSession>,
    message: Option<&'static str>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SocialSession {
    game_id: u64,
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
    game_title: String,
    lobby_id: String,
    join_secret: String,
    party_size: u32,
    party_capacity: u32,
    joinable: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SocialFriend {
    id: String,
    display_name: String,
    group: &'static str,
    status_text: String,
}

pub struct GameBridgeLaunch {
    pub endpoint: String,
    pub token: String,
}

impl SocialService {
    fn snapshot(&self) -> SocialSnapshot {
        let application_configured = option_env!("COLDEM_DISCORD_APPLICATION_ID")
            .is_some_and(|value| !value.trim().is_empty());

        SocialSnapshot {
            connection: "setup_required",
            application_configured,
            sdk_available: false,
            friends: Vec::new(),
            active_session: self.active_session.lock().ok().and_then(|value| value.clone()),
            message: Some(
                "Discord friends will appear here once the Coldem Social SDK build is enabled.",
            ),
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
            .insert(token.clone(), game_id);

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
                            let _ = handle_bridge_request(stream, service, app, game_id, token).await;
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

    pub fn watch_game_process(
        self: &Arc<Self>,
        app: AppHandle,
        game_id: u64,
        token: String,
        mut child: Child,
    ) {
        let service = Arc::clone(self);
        std::thread::spawn(move || {
            let _ = child.wait();
            service.revoke_game_bridge(&app, game_id, &token);
        });
    }

    pub fn revoke_game_bridge(&self, app: &AppHandle, game_id: u64, token: &str) {
        if let Ok(mut bridges) = self.game_bridges.lock() {
            bridges.remove(token);
        }
        if let Ok(mut session) = self.active_session.lock() {
            if session.as_ref().is_some_and(|value| value.game_id == game_id) {
                *session = None;
            }
        }
        self.emit_snapshot(app);
    }

    fn bridge_is_active(&self, token: &str, game_id: u64) -> bool {
        self.game_bridges
            .lock()
            .ok()
            .and_then(|bridges| bridges.get(token).copied())
            == Some(game_id)
    }

    fn emit_snapshot(&self, app: &AppHandle) {
        let _ = app.emit("launcher://social", self.snapshot());
    }
}

#[tauri::command]
pub fn social_snapshot(state: tauri::State<'_, crate::AppState>) -> SocialSnapshot {
    state.social.snapshot()
}

#[tauri::command]
pub fn connect_discord(
    state: tauri::State<'_, crate::AppState>,
) -> Result<SocialSnapshot, String> {
    let snapshot = state.social.snapshot();
    if !snapshot.application_configured {
        return Err("The Discord application ID is not configured for this build.".into());
    }
    if !snapshot.sdk_available {
        return Err("The Discord Social SDK runtime is not bundled with this build.".into());
    }
    Ok(snapshot)
}

#[tauri::command]
pub fn disconnect_discord(state: tauri::State<'_, crate::AppState>) -> SocialSnapshot {
    state.social.snapshot()
}

#[tauri::command]
pub fn invite_discord_friend(
    _friend_id: String,
    _state: tauri::State<'_, crate::AppState>,
) -> Result<(), String> {
    Err("Start a joinable EOS lobby before sending a Discord invite.".into())
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
    let supplied_token = header_value(headers, "authorization")
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default();

    if supplied_token != expected_token || !service.bridge_is_active(&expected_token, game_id) {
        write_http_response(&mut stream, 401, "Unauthorized").await;
        return Ok(());
    }

    if request_line == "DELETE /session HTTP/1.1" {
        if let Ok(mut session) = service.active_session.lock() {
            if session.as_ref().is_some_and(|value| value.game_id == game_id) {
                *session = None;
            }
        }
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

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
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

fn random_token() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|error| format!("Could not create a social bridge token: {error}"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn validate_session(session: &SocialSession) -> Result<(), String> {
    if session.game_title.trim().is_empty() || session.lobby_id.trim().is_empty() {
        return Err("The social session requires a game title and EOS lobby ID.".into());
    }
    if session.join_secret.trim().is_empty() || session.join_secret.len() > 512 {
        return Err("The social join secret is missing or too large.".into());
    }
    if session.party_capacity == 0 || session.party_size > session.party_capacity {
        return Err("The social party size is invalid.".into());
    }
    Ok(())
}
