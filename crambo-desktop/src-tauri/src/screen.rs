use screenshots::Screen;
use std::io::Cursor;
use tauri::command;
use tempfile::NamedTempFile;
use std::io::Write;

#[command]
pub fn capture_screenshot() -> Result<String, String> {
    let screens = Screen::all().map_err(|e| format!("Failed to enumerate screens: {}", e))?;

    let screen = screens
        .first()
        .ok_or_else(|| "No screen available".to_string())?;

    let image = screen
        .capture()
        .map_err(|e| format!("Failed to capture screen: {}", e))?;

    let rgba_data = image.as_raw();
    let width = image.width();
    let height = image.height();

    let img_buffer: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> =
        image::ImageBuffer::from_raw(width, height, rgba_data.to_vec())
            .ok_or_else(|| "Failed to create image buffer".to_string())?;

    let dynamic_image = image::DynamicImage::ImageRgba8(img_buffer);

    let mut jpeg_bytes = Cursor::new(Vec::new());
    dynamic_image
        .write_to(&mut jpeg_bytes, image::ImageFormat::Jpeg)
        .map_err(|e| format!("Failed to encode JPEG: {}", e))?;

    let mut temp_file = NamedTempFile::with_suffix(".jpg")
        .map_err(|e| format!("Failed to create temp file: {}", e))?;

    temp_file
        .write_all(&jpeg_bytes.into_inner())
        .map_err(|e| format!("Failed to write screenshot: {}", e))?;

    let path = temp_file.into_temp_path();
    let final_path = path.to_string_lossy().to_string();
    let _ = path.keep();

    Ok(final_path)
}
