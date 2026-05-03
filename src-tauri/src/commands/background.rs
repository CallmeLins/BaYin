use base64::Engine;
use tauri::{AppHandle, Manager};

fn mime_from_ext(ext: &str) -> &str {
    match ext {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "gif" => "image/gif",
        _ => "image/jpeg",
    }
}

fn read_as_data_url(path: &std::path::Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("jpg");
    let mime = mime_from_ext(ext);
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{};base64,{}", mime, b64))
}

/// Copy the user-selected image to app_data_dir, return a base64 data URL.
#[tauri::command]
pub fn save_background_image(app: AppHandle, source_path: String) -> Result<String, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;

    let ext = std::path::Path::new(&source_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("jpg");
    let dest_path = data_dir.join(format!("background_image.{}", ext));

    std::fs::copy(&source_path, &dest_path).map_err(|e| e.to_string())?;

    read_as_data_url(&dest_path)
}

/// Read the cached background image and return a base64 data URL, or null.
#[tauri::command]
pub fn get_background_image_data(app: AppHandle) -> Result<Option<String>, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;

    for ext in &["jpg", "jpeg", "png", "webp", "gif", "bmp"] {
        let path = data_dir.join(format!("background_image.{}", ext));
        if path.exists() {
            return Ok(Some(read_as_data_url(&path)?));
        }
    }

    Ok(None)
}

/// Remove the background image from app data dir.
#[tauri::command]
pub fn delete_background_image(app: AppHandle) -> Result<(), String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;

    for ext in &["jpg", "jpeg", "png", "webp", "gif", "bmp"] {
        let path = data_dir.join(format!("background_image.{}", ext));
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}
