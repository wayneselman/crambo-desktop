# Crambo Desktop

## Overview
Tauri v2 desktop application that wraps the Crambo web UI and adds native system audio capture for Mac and Windows. Includes deep link scheme (`crambo://`), device name detection, multipart upload, and cross-platform build pipeline via GitHub Actions.

## Project Structure

```
crambo-desktop/
├── package.json                    # Node/npm config with Tauri CLI scripts
├── src/
│   └── index.html                  # Frontend placeholder UI
├── src-tauri/
│   ├── Cargo.toml                  # Rust dependencies
│   ├── tauri.conf.json             # Tauri v2 configuration
│   ├── Entitlements.plist          # macOS entitlements (mic, camera, screen)
│   ├── build.rs                    # Tauri build script
│   ├── icons/                      # App icons (placeholder)
│   └── src/
│       ├── main.rs                 # Entry point, command registration, deep link handler
│       ├── storage.rs              # Secure token storage via keyring
│       ├── audio.rs                # System audio capture via cpal (CoreAudio/WASAPI)
│       ├── screen.rs               # Screenshot capture via screenshots crate
│       ├── detector.rs             # Meeting app detection via sysinfo
│       ├── tray.rs                 # System tray setup (idle/recording states)
│       └── uploader.rs             # Multipart upload to /api/ingest/desktop
.github/
└── workflows/
    └── desktop-build.yml           # Cross-platform CI (macOS + Windows)
```

## Key Dependencies (Rust)

| Crate | Version | Purpose |
|-------|---------|---------|
| tauri | 2 | App framework (tray-icon feature) |
| tauri-plugin-shell | 2 | Shell/open URLs |
| tauri-plugin-deep-link | 2 | crambo:// scheme handler |
| cpal | 0.15 | Cross-platform audio capture |
| opus | 0.3 | Audio encoding |
| screenshots | 0.8 | Screen capture |
| keyring | 2 | Secure credential storage |
| sysinfo | 0.30 | Process detection (meeting apps) |
| hostname | 0.4 | Device name |
| reqwest | 0.12 | HTTP multipart upload |
| tokio | 1 | Async runtime |

## Tauri Commands (Exposed to Frontend)

- `save_token(token)` / `get_token()` / `delete_token()` — Secure auth token storage
- `start_recording(mode)` / `stop_recording()` — Audio capture (returns file path)
- `capture_screenshot()` — Returns JPEG bytes
- `detect_meeting_app()` — Polls processes for Zoom/Meet/Teams/Webex
- `upload_session(...)` — Multipart upload with exact field names: audio, screenshot_0..N, title, course, duration, captured_at, device_name
- `poll_status(lecture_id, token)` — Check upload processing status
- `get_device_name()` — Returns hostname

## Deep Link

Scheme: `crambo://auth?token=<jwt>` — stores token via keyring and shows main window.

## Upload Field Names (POST /api/ingest/desktop)

- `audio` (required) — WebM/Opus file
- `video` (optional)
- `screenshot_0`, `screenshot_1`, ... `screenshot_N` (optional)
- `title` (required), `course`, `duration`, `captured_at`, `device_name` (optional text fields)
- Authorization: `Bearer <token>` header

## GitHub Actions (desktop-build.yml)

Matrix: macOS (universal-apple-darwin) + Windows (x86_64-pc-windows-msvc)

### Required GitHub Secrets (add when available)

| Secret | Description |
|--------|-------------|
| `TAURI_SIGNING_PRIVATE_KEY` | Tauri update signing key |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Password for signing key |
| `APPLE_CERTIFICATE` | Base64-encoded .p12 certificate |
| `APPLE_CERTIFICATE_PASSWORD` | Certificate password |
| `APPLE_SIGNING_IDENTITY` | e.g. "Developer ID Application: Company (TEAMID)" |
| `APPLE_ID` | Apple ID email for notarization |
| `APPLE_PASSWORD` | App-specific password |
| `APPLE_TEAM_ID` | Apple Team ID |

## Development

This is a native desktop application. It cannot run as a web preview in Replit.

**Local development:**
```bash
cd crambo-desktop
npm install
npm run dev     # Starts Tauri dev mode
npm run build   # Produces distributable binaries
```

## System Dependencies (Linux/Replit)

Installed via Nix: pkg-config, glib, gtk3, webkitgtk_4_1, libsoup_3, alsa-lib, openssl, libayatana-appindicator, dbus, libopus
