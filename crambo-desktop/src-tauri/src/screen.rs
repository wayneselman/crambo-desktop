use screenshots::Screen;
use std::io::Cursor;
use tauri::command;

#[command]
pub fn capture_screenshot() -> Result<Vec<u8>, String> {
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

    Ok(jpeg_bytes.into_inner())
}
