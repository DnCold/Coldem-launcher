mod app_updates;
mod commands;
mod delivery;
mod social;

use app_updates::*;
use commands::*;
use delivery::DeliveryService;
use social::*;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

pub struct AppState {
    delivery: DeliveryService,
    social: Arc<SocialService>,
    operation_lock: Mutex<()>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            delivery: DeliveryService::default(),
            social: Arc::new(SocialService::default()),
            operation_lock: Mutex::new(()),
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
        .manage(AppState::default())
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
