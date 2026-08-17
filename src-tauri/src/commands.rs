use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, State};

use crate::{delivery::player_profile, AppState};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapResult {
    butler_version: String,
    profiles: Value,
    catalog_game_count: usize,
    catalog_restricted: bool,
    channel: String,
}

#[tauri::command]
pub async fn initialize_launcher(
    app: AppHandle,
    state: State<'_, AppState>,
    channel: Option<String>,
) -> Result<BootstrapResult, String> {
    if let Some(channel) = channel.as_deref() {
        state.delivery.set_channel(channel).await?;
    }
    let channel = state.delivery.channel().await;
    let (butler_version, catalog) = tokio::try_join!(
        state.delivery.butler_version(&app),
        state.delivery.load_catalog(&app, true)
    )?;
    let catalog_game_count = catalog.games.len();
    Ok(BootstrapResult {
        butler_version,
        profiles: json!([player_profile()]),
        catalog_game_count,
        catalog_restricted: catalog_game_count > 0,
        channel,
    })
}

#[tauri::command]
pub async fn set_release_channel(
    channel: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.delivery.set_channel(&channel).await
}

#[tauri::command]
pub async fn enter_library() -> Result<Value, String> {
    Ok(player_profile())
}

#[tauri::command]
pub async fn cancel_entry() -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn use_local_profile(_profile_id: u64) -> Result<Value, String> {
    Ok(player_profile())
}

#[tauri::command]
pub async fn forget_profile(_profile_id: u64) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn fetch_library(
    app: AppHandle,
    _profile_id: u64,
    fresh: bool,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    state.delivery.library(&app, fresh).await
}

#[tauri::command]
pub async fn check_updates(app: AppHandle, state: State<'_, AppState>) -> Result<Value, String> {
    state.delivery.updates(&app).await
}

#[tauri::command]
pub async fn prepare_install(
    app: AppHandle,
    game_id: u64,
    _profile_id: u64,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    state.delivery.install_options(&app, game_id).await
}

#[tauri::command]
pub async fn plan_install(
    app: AppHandle,
    upload_id: u64,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    state.delivery.install_plan(&app, upload_id).await
}

#[tauri::command]
pub async fn install_game(
    app: AppHandle,
    _profile_id: u64,
    game: Value,
    _upload: Value,
    _install_location_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let game_id = game
        .get("id")
        .and_then(Value::as_u64)
        .ok_or_else(|| "Install request did not include a game ID".to_string())?;
    let _guard = state.operation_lock.lock().await;
    state.delivery.install(&app, game_id).await
}

#[tauri::command]
pub async fn update_game(
    app: AppHandle,
    _profile_id: u64,
    _cave_id: String,
    game_id: u64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let _guard = state.operation_lock.lock().await;
    state.delivery.update(&app, game_id).await
}

#[tauri::command]
pub async fn play_game(
    app: AppHandle,
    _profile_id: u64,
    _cave_id: String,
    game_id: u64,
    join_payload: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let social = state.social.clone();
    let delivery = state.delivery.clone();
    if let Some(payload) = join_payload.as_deref() {
        let request = crate::social::parse_join_payload(payload)?;
        let invited_game_id = delivery.game_id_for_slug(&app, &request.game_slug).await?;
        if invited_game_id != game_id {
            return Err("This Discord invite does not match the game you selected.".into());
        }
    }
    let bridge = social.prepare_game_bridge(app.clone(), game_id).await?;
    let started_at = std::time::Instant::now();
    match delivery.play(&app, game_id, &bridge, join_payload.as_deref()) {
        Ok(child) => {
            let token = bridge.token;
            let tracking_app = app.clone();
            social.watch_game_process(app, game_id, token, child, started_at, move |elapsed| {
                let _ = delivery.record_play_finished(&tracking_app, game_id, elapsed);
            });
            Ok(())
        }
        Err(error) => {
            social.revoke_game_bridge(&app, game_id, &bridge.token);
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn respond_to_prompt(_id: Value, _result: Value) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn open_external(target: String) -> Result<(), String> {
    if !(target.starts_with("https://") || target.starts_with("http://")) {
        return Err("Only HTTP and HTTPS links can be opened".into());
    }
    open::that(target).map_err(|error| format!("Could not open link: {error}"))
}
