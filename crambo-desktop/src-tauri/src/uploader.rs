use reqwest::multipart;
use serde::{Deserialize, Serialize};
use tauri::command;
use tokio::fs;

#[derive(Serialize, Deserialize)]
pub struct UploadResponse {
    pub lecture_id: String,
    pub status: String,
}

#[derive(Serialize, Deserialize)]
pub struct StatusResponse {
    pub lecture_id: String,
    pub status: String,
    pub progress: Option<f64>,
    pub error: Option<String>,
}

#[command]
pub async fn upload_session(
    audio_path: String,
    title: String,
    course: Option<String>,
    duration: Option<String>,
    captured_at: Option<String>,
    device_name: Option<String>,
    screenshot_paths: Option<Vec<String>>,
    token: String,
) -> Result<UploadResponse, String> {
    let audio_bytes = fs::read(&audio_path)
        .await
        .map_err(|e| format!("Failed to read audio file: {}", e))?;

    let (file_name, mime_type) = ("recording.wav", "audio/wav");

    let audio_part = multipart::Part::bytes(audio_bytes)
        .file_name(file_name.to_string())
        .mime_str(mime_type)
        .map_err(|e| format!("MIME error: {}", e))?;

    let mut form = multipart::Form::new()
        .part("audio", audio_part)
        .text("title", title);

    if let Some(c) = course {
        form = form.text("course", c);
    }

    if let Some(d) = duration {
        form = form.text("duration", d);
    }

    if let Some(ca) = captured_at {
        form = form.text("captured_at", ca);
    }

    if let Some(dn) = device_name {
        form = form.text("device_name", dn);
    }

    if let Some(paths) = screenshot_paths {
        for (i, path) in paths.iter().enumerate() {
            let bytes = fs::read(path)
                .await
                .map_err(|e| format!("Failed to read screenshot {}: {}", i, e))?;
            let part = multipart::Part::bytes(bytes)
                .file_name(format!("screenshot_{}.jpg", i))
                .mime_str("image/jpeg")
                .map_err(|e| format!("MIME error: {}", e))?;
            form = form.part(format!("screenshot_{}", i), part);
        }
    }

    let client = reqwest::Client::new();
    let response = client
        .post("https://app.crambo.ai/api/ingest/desktop")
        .header("Authorization", format!("Bearer {}", token))
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Upload failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Upload failed ({}): {}", status, body));
    }

    response
        .json::<UploadResponse>()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))
}

#[command]
pub async fn poll_status(lecture_id: String, token: String) -> Result<StatusResponse, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://app.crambo.ai/api/ingest/desktop/status/{}",
        lecture_id
    );

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .map_err(|e| format!("Status check failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Status check failed ({}): {}", status, body));
    }

    response
        .json::<StatusResponse>()
        .await
        .map_err(|e| format!("Failed to parse status: {}", e))
}
