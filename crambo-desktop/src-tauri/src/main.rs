#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod detector;
mod screen;
mod storage;
mod tray;
mod uploader;

use tauri::{Emitter, Listener, Manager};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            let handle = app.handle().clone();

            tray::setup_tray(&handle).expect("Failed to setup system tray");

            let device_name = hostname::get()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_else(|_| "Unknown".to_string());

            app.manage(DeviceInfo { name: device_name });

            #[cfg(any(target_os = "linux", all(debug_assertions, not(target_os = "macos"))))]
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                let _ = app.deep_link().register_all();
            }

            app.listen("deep-link://new-url", move |event| {
                let payload = event.payload();
                if let Some(token) = extract_token_from_url(payload) {
                    let _ = storage::save_token(token);
                    if let Some(window) = handle.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                        let _ = window.emit("auth-token-received", ());
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            storage::save_token,
            storage::get_token,
            storage::delete_token,
            audio::start_recording,
            audio::stop_recording,
            screen::capture_screenshot,
            detector::detect_meeting_app,
            uploader::upload_session,
            uploader::poll_status,
            get_device_name,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

struct DeviceInfo {
    name: String,
}

#[tauri::command]
fn get_device_name(state: tauri::State<DeviceInfo>) -> String {
    state.name.clone()
}

fn extract_token_from_url(url: &str) -> Option<String> {
    let url = url.trim_matches('"');
    if let Ok(parsed) = url::Url::parse(url) {
        for (key, value) in parsed.query_pairs() {
            if key == "token" {
                return Some(value.to_string());
            }
        }
    }
    None
}
