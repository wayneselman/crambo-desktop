use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIcon, TrayIconBuilder},
    AppHandle, Emitter, Manager,
};
use std::sync::Mutex;

struct TrayState {
    tray: TrayIcon,
    start_item: MenuItem<tauri::Wry>,
    stop_item: MenuItem<tauri::Wry>,
}

static TRAY_STATE: std::sync::OnceLock<Mutex<Option<TrayState>>> = std::sync::OnceLock::new();

fn get_tray_state() -> &'static Mutex<Option<TrayState>> {
    TRAY_STATE.get_or_init(|| Mutex::new(None))
}

pub fn set_recording_state(recording: bool) {
    if let Ok(guard) = get_tray_state().lock() {
        if let Some(state) = guard.as_ref() {
            let _ = state.start_item.set_enabled(!recording);
            let _ = state.stop_item.set_enabled(recording);
            let tooltip = if recording {
                "Crambo — Recording"
            } else {
                "Crambo"
            };
            let _ = state.tray.set_tooltip(Some(tooltip));
        }
    }
}

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let start_item = MenuItem::with_id(app, "start", "Start Recording", true, None::<&str>)?;
    let stop_item = MenuItem::with_id(app, "stop", "Stop Recording", false, None::<&str>)?;
    let open_item = MenuItem::with_id(app, "open", "Open Crambo", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&start_item, &stop_item, &open_item, &quit_item])?;

    let tray = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("Crambo")
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "start" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.eval("window.__tauriStartRecording && window.__tauriStartRecording()");
                }
                let _ = app.emit("tray-start-recording", ());
            }
            "stop" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.eval("window.__tauriStopRecording && window.__tauriStopRecording()");
                }
                let _ = app.emit("tray-stop-recording", ());
            }
            "open" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    let state = TrayState {
        tray,
        start_item,
        stop_item,
    };

    if let Ok(mut guard) = get_tray_state().lock() {
        *guard = Some(state);
    }

    Ok(())
}
