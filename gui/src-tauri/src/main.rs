// No console window behind the launcher on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod game;
mod lifecycle;
mod prefs;
mod profiles;

use std::path::Path;
use tauri::{Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

/// Sent to the interface as a number: 0 info, 1 success, 2 error.
#[derive(Clone, Copy)]
pub enum NotificationLevel {
    Info = 0,
    Error = 2,
}

pub fn notify(app: &tauri::AppHandle, message: String, level: NotificationLevel) {
    if let Err(e) = app.emit_to("main", "notification", (message, level as u8)) {
        eprintln!("could not show notification: {e}");
    }
}

pub fn open_path(app: &tauri::AppHandle, path: &Path) -> Result<(), String> {
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| format!("could not open {}: {e}", path.display()))
}

#[tauri::command]
fn app_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}

fn main() {
    sc_launch::env::load_dotenv();
    // WebKitGTK's DMA-BUF renderer shows a blank window on some Linux GPU
    // drivers; the fallback renderer is fine for a launcher.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        // SAFETY: single-threaded, before GTK/WebKit start.
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }

    let app = tauri::Builder::default()
        .manage(lifecycle::Lifecycle::default())
        .manage(prefs::PrefsState::default())
        .manage(game::GameState::default())
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            lifecycle::restore(app);
        }))
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_denylist(&["splash"])
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::SIZE
                        | tauri_plugin_window_state::StateFlags::POSITION
                        | tauri_plugin_window_state::StateFlags::MAXIMIZED,
                )
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            prefs::load(app.handle());
            lifecycle::create_windows(app.handle())?;
            if let Some(window) = app.get_webview_window("main") {
                if let Err(e) = tauri::image::Image::from_bytes(include_bytes!("../icons/128x128.png"))
                    .and_then(|icon| window.set_icon(icon))
                {
                    eprintln!("could not set the window icon: {e}");
                }
            }
            lifecycle::setup(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if let Err(error) = lifecycle::close_main_window(window.app_handle()) {
                    notify(
                        window.app_handle(),
                        format!("Could not close the launcher: {error}"),
                        NotificationLevel::Error,
                    );
                }
            }
        })
        .invoke_handler(|invoke| {
            let window = invoke.message.webview_ref().label();
            if window != "main" && !(window == "splash" && invoke.message.command() == "splash_finished") {
                invoke.resolver.reject("This window cannot invoke launcher commands");
                return true;
            }
            let handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = tauri::generate_handler![
                lifecycle::main_ready,
                lifecycle::splash_finished,
                lifecycle::exit_launcher,
                prefs::get_launcher_preferences,
                prefs::update_launcher_preferences,
                game::get_launcher_snapshot,
                game::initialize_launcher,
                game::perform_launcher_action,
                game::detect_compat_tools,
                game::get_artwork,
                game::view_game_install,
                game::view_launch_log,
                profiles::list_profiles,
                profiles::save_profile,
                profiles::delete_profile,
                profiles::open_profile_folder,
                app_version,
            ];
            handler(invoke)
        });

    if let Err(error) = app.run(tauri::generate_context!()) {
        eprintln!("STAR FORGE Launcher could not start: {error}");
        std::process::exit(1);
    }
}
