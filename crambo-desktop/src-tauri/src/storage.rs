use keyring::Entry;
use tauri::command;

const SERVICE_NAME: &str = "com.crambo.desktop";
const CODE_EXCHANGE_URL: &str = "https://crambo.app/api/desktop/exchange-code";

fn get_entry() -> Result<Entry, String> {
    Entry::new(SERVICE_NAME, "auth_token").map_err(|e| format!("Keyring error: {}", e))
}

#[command]
pub fn save_token(token: String) -> Result<(), String> {
    let entry = get_entry()?;
    entry
        .set_password(&token)
        .map_err(|e| format!("Failed to save token: {}", e))
}

#[command]
pub fn get_token() -> Result<Option<String>, String> {
    let entry = get_entry()?;
    match entry.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("Failed to get token: {}", e)),
    }
}

#[command]
pub fn delete_token() -> Result<(), String> {
    let entry = get_entry()?;
    match entry.delete_password() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("Failed to delete token: {}", e)),
    }
}

#[derive(serde::Deserialize)]
struct CodeExchangeResponse {
    token: String,
}

#[command]
pub async fn set_token_from_code(code: String) -> Result<(), String> {
    let code = code.trim().to_uppercase();
    if code.is_empty() {
        return Err("Code cannot be empty".to_string());
    }

    let client = reqwest::Client::new();
    let response = client
        .post(CODE_EXCHANGE_URL)
        .json(&serde_json::json!({ "code": code }))
        .send()
        .await
        .map_err(|e| format!("Failed to connect: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Invalid code ({}): {}", status, body));
    }

    let data: CodeExchangeResponse = response
        .json()
        .await
        .map_err(|e| format!("Invalid response: {}", e))?;

    let entry = get_entry()?;
    entry
        .set_password(&data.token)
        .map_err(|e| format!("Failed to save token: {}", e))
}
