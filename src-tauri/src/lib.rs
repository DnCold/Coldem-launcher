mod app_updates;
mod commands;
mod delivery;
#[cfg(target_os = "windows")]
mod discord_credentials;
#[cfg(target_os = "windows")]
mod discord_native;
mod social;

use app_updates::*;
use commands::*;
use delivery::DeliveryService;
use social::*;
use std::sync::Arc;
#[cfg(target_os = "windows")]
use tauri::path::BaseDirectory;
use tauri::Manager;
use tokio::sync::Mutex;

pub struct AppState {
    delivery: Arc<DeliveryService>,
    social: Arc<SocialService>,
    operation_lock: Mutex<()>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            delivery: Arc::new(DeliveryService::default()),
            social: Arc::new(SocialService::new()),
            operation_lock: Mutex::new(()),
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = AppState::default();
    #[cfg(target_os = "windows")]
    let social = state.social.clone();
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }));

        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    builder
        .plugin(tauri_plugin_shell::init())
        .setup(move |app| {
            #[cfg(target_os = "windows")]
            ensure_discord_runtime(app.handle())?;
            #[cfg(target_os = "windows")]
            let _ = social.restore_saved_session(app.handle());
            Ok(())
        })
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            initialize_launcher,
            launcher_update_status,
            check_launcher_update,
            install_launcher_update,
            social_snapshot,
            connect_discord,
            disconnect_discord,
            invite_discord_friend,
            enter_library,
            cancel_entry,
            use_local_profile,
            forget_profile,
            fetch_library,
            check_updates,
            prepare_install,
            plan_install,
            install_game,
            update_game,
            play_game,
            respond_to_prompt,
            open_external
        ])
        .run(tauri::generate_context!())
        .expect("error while running Coldem");
}

#[cfg(target_os = "windows")]
fn ensure_discord_runtime(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let executable_dir = std::env::current_exe()?
        .parent()
        .ok_or("Could not resolve the Coldem executable directory")?
        .to_path_buf();
    for filename in ["discord_partner_sdk.dll", "discord_krisp.dll"] {
        let destination = executable_dir.join(filename);
        if destination.exists() {
            continue;
        }
        let resource = app
            .path()
            .resolve(format!("discord/{filename}"), BaseDirectory::Resource)?;
        std::fs::copy(resource, destination)?;
    }
    Ok(())
}
