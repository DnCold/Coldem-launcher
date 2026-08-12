use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::{Updater, UpdaterExt};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherUpdateStatus {
    configured: bool,
    current_version: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherUpdateMetadata {
    version: String,
    current_version: String,
}

#[derive(Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
enum LauncherUpdateEvent {
    Downloading { downloaded: u64, total: Option<u64> },
    Installed,
}

fn configured_updater(app: &AppHandle) -> Result<Updater, String> {
    app.updater().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn launcher_update_status(app: AppHandle) -> LauncherUpdateStatus {
    LauncherUpdateStatus {
        configured: true,
        current_version: app.package_info().version.to_string(),
    }
}

#[tauri::command]
pub async fn check_launcher_update(
    app: AppHandle,
) -> Result<Option<LauncherUpdateMetadata>, String> {
    let update = configured_updater(&app)?
        .check()
        .await
        .map_err(|error| error.to_string())?;

    Ok(update.map(|update| LauncherUpdateMetadata {
        version: update.version,
        current_version: update.current_version,
    }))
}

#[tauri::command]
pub async fn install_launcher_update(app: AppHandle) -> Result<(), String> {
    let update = configured_updater(&app)?
        .check()
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Coldem is already up to date".to_owned())?;

    let progress_app = app.clone();
    let finished_app = app.clone();
    let mut downloaded = 0_u64;
    update
        .download_and_install(
            move |chunk_length, total| {
                downloaded += chunk_length as u64;
                let _ = progress_app.emit(
                    "launcher://self-update",
                    LauncherUpdateEvent::Downloading { downloaded, total },
                );
            },
            move || {
                let _ = finished_app.emit("launcher://self-update", LauncherUpdateEvent::Installed);
            },
        )
        .await
        .map_err(|error| error.to_string())?;

    app.restart()
}
