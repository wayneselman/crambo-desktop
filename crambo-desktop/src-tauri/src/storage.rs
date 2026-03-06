use keyring::Entry;
use tauri::command;

const SERVICE_NAME: &str = "com.crambo.desktop";

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
