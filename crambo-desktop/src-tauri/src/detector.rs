use sysinfo::System;
use tauri::command;

const MEETING_APPS: &[(&str, &str)] = &[
    ("zoom", "Zoom"),
    ("zoom.us", "Zoom"),
    ("teams", "Microsoft Teams"),
    ("webex", "Webex"),
    ("ciscocollabhost", "Webex"),
];

const BROWSER_MEETING_INDICATORS: &[(&str, &str)] = &[
    ("chrome", "Google Meet"),
    ("msedge", "Microsoft Teams"),
];

#[command]
pub fn detect_meeting_app() -> Option<String> {
    let mut sys = System::new();
    sys.refresh_processes();

    for (_pid, process) in sys.processes() {
        let proc_name = process.name().to_lowercase();

        for (pattern, app_name) in MEETING_APPS {
            if proc_name.contains(pattern) {
                return Some(app_name.to_string());
            }
        }

        for (browser_pattern, meeting_name) in BROWSER_MEETING_INDICATORS {
            if proc_name.contains(&browser_pattern.to_lowercase()) {
                if let Some(cmd) = process.cmd().first() {
                    let cmd_str = cmd.to_lowercase();
                    if cmd_str.contains("meet.google.com")
                        || cmd_str.contains("teams.microsoft.com")
                    {
                        return Some(meeting_name.to_string());
                    }
                }
            }
        }
    }

    None
}
