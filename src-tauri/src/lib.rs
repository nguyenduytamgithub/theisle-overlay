//! TheIsle Overlay v2 — tauri application shell.
//!
//! Position flows ONE way: clipboard.rs -> tracker -> both windows.
//! See win/mod.rs for the anti-cheat safety boundary this app must never
//! cross.

pub mod clipboard;
pub mod commands;
pub mod events;
pub mod fetch;
pub mod hotkeys;
pub mod minimap;
pub mod pipeline;
pub mod replay;
pub mod settings;
pub mod state;
pub mod store;
pub mod win;

use std::path::PathBuf;

use tauri::Manager;

use crate::state::AppState;

pub fn run(replay_file: Option<PathBuf>) {
    let builder = tauri::Builder::default()
        // Must be the FIRST plugin: RegisterHotKey is system-exclusive, so a
        // second instance would silently lose half its hotkeys. The old app
        // used a named mutex for the same reason.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(main) = app.get_webview_window("main") {
                let _ = main.show();
                let _ = main.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Folder {
                        path: settings::local_dir().join("logs"),
                        file_name: None,
                    }),
                ])
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::patch_settings,
            commands::list_waypoints,
            commands::list_waypoints_px,
            commands::add_waypoint_at_pixel,
            commands::add_waypoint_here,
            commands::rename_waypoint,
            commands::delete_waypoint,
            commands::get_previous_trail,
            commands::get_current_trail,
            commands::data_status,
            commands::get_basemap_paths,
            commands::get_pois,
            commands::get_pois_render,
            commands::nearest_waypoint,
            commands::get_fullscreen_mode,
            commands::get_map_info,
            commands::check_hotkey_available,
            commands::apply_hotkeys,
            commands::open_trails_folder,
            commands::fetch_data,
            #[cfg(debug_assertions)]
            commands::simulate_position,
        ]);

    builder
        .setup(move |app| {
            settings::ensure_dirs()?;
            // Upgrade pois_gateway.json in place (offline, from cache) when
            // an app update added new layers.
            fetch::ensure_pois_current();
            minimap::create(app.handle())?;
            clipboard::spawn(app.handle().clone());
            {
                let state = app.state::<AppState>();
                state.hotkeys.restart(app.handle().clone());
            }
            if let Some(path) = replay_file {
                replay::spawn(app.handle().clone(), path);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
