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
pub mod islepilot;
pub mod minimap;
pub mod pipeline;
pub mod replay;
pub mod settings;
pub mod state;
pub mod store;
pub mod translate;
pub mod tray;
pub mod webview_mem;
pub mod win;

use std::path::PathBuf;

use tauri::Manager;

use crate::state::{AppState, LockExt};

pub fn run(replay_file: Option<PathBuf>) {
    let builder = tauri::Builder::default()
        // Must be the FIRST plugin: RegisterHotKey is system-exclusive, so a
        // second instance would silently lose half its hotkeys. The old app
        // used a named mutex for the same reason.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main(app);
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
        .on_window_event(|window, event| match event {
            // X hides to the tray, Steam/Discord-style. Quit lives in the
            // tray menu; app.exit bypasses CloseRequested so it cannot be
            // trapped here. The login window keeps its own close handling.
            tauri::WindowEvent::CloseRequested { api, .. }
                if window.label() == "main" && !tray::is_quitting() =>
            {
                api.prevent_close();
                log::info!("main window: hide (X to tray)");
                let _ = window.hide();
                if let Some(webview) = window.app_handle().get_webview_window("main") {
                    webview_mem::on_hidden(&webview);
                }
            }
            tauri::WindowEvent::Destroyed => win::vis::unregister(window.label()),
            _ => {}
        })
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::patch_settings,
            commands::get_current_position,
            commands::list_waypoints,
            commands::list_waypoints_px,
            commands::add_waypoint_at_pixel,
            commands::add_waypoint_here,
            commands::rename_waypoint,
            commands::set_waypoint_color,
            commands::delete_waypoint,
            commands::resolve_coordinates,
            commands::get_previous_trail,
            commands::get_current_trail,
            commands::clear_trail,
            commands::data_status,
            commands::get_basemap_paths,
            commands::set_basemap_source,
            commands::get_pois,
            commands::get_pois_render,
            commands::nearest_waypoint,
            commands::get_fullscreen_mode,
            commands::get_map_info,
            commands::check_hotkey_available,
            commands::apply_hotkeys,
            commands::open_trails_folder,
            commands::fetch_data,
            commands::islepilot_login,
            commands::islepilot_set_cookie,
            commands::islepilot_cancel_login,
            commands::islepilot_token_login,
            commands::islepilot_set_token,
            commands::islepilot_logout,
            commands::islepilot_apply,
            commands::islepilot_state,
            #[cfg(debug_assertions)]
            commands::simulate_position,
        ]);

    builder
        .setup(move |app| {
            settings::ensure_dirs()?;
            // Upgrade pois_gateway.json in place (offline, from cache) when
            // an app update added new layers.
            fetch::ensure_pois_current();
            // ...and quietly fetch sources an update added that the offline
            // path cannot produce (islemaps animal sightings).
            fetch::spawn_topup(app.handle());
            // Heal settings that point at deleted islemaps imagery (LOCALDATA
            // wiped, roaming settings kept) BEFORE any window exists, so
            // every later path/calibration resolve can trust the settings
            // without per-call file checks.
            {
                let state = app.state::<AppState>();
                let source = state.active_source();
                if let Some(variant) = fetch::IslemapsVariant::for_source(source) {
                    if !variant.dest().exists() {
                        log::warn!(
                            "selected basemap {} missing on disk - reverting to vulnona",
                            source.key()
                        );
                        // Direct settings write, not apply_settings_patch —
                        // there are no windows to broadcast to yet.
                        let mut s = state.settings.lock_safe();
                        *s = settings::merge(
                            &s,
                            &serde_json::json!({ "map": { "basemap": "vulnona" } }),
                        );
                        drop(s);
                        state.request_settings_save();
                    }
                }
            }
            if let Some(main) = app.get_webview_window("main") {
                if let Ok(hwnd) = main.hwnd() {
                    win::vis::register("main", hwnd.0 as isize);
                }
            }
            minimap::create(app.handle())?;
            tray::create(app.handle())?;
            clipboard::spawn(app.handle().clone());
            webview_mem::spawn_watchdog(app.handle().clone());
            {
                let state = app.state::<AppState>();
                state.hotkeys.restart(app.handle().clone());
            }
            islepilot::restart_poller(app.handle());
            if let Some(path) = replay_file {
                replay::spawn(app.handle().clone(), path);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
