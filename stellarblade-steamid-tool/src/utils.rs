use eframe::egui;
use std::path::PathBuf;

pub fn load_icon() -> egui::IconData {
    use ico::IconDir;
    let ico_bytes = include_bytes!("icon.ico");
    let icon_dir = IconDir::read(std::io::Cursor::new(ico_bytes)).expect("Invalid .ico data");
    let entry = &icon_dir.entries()[0];
    let image = entry.decode().expect("Failed to decode ICO image");
    let (width, height) = (image.width(), image.height());
    let pixels = image.rgba_data().to_vec();
    egui::IconData {
        rgba: pixels,
        width,
        height,
    }
}

pub fn browse_file() -> Option<(String, String)> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(userprofile) = std::env::var("USERPROFILE") {
            let path = PathBuf::from(userprofile)
                .join("AppData")
                .join("Local")
                .join("SB")
                .join("Saved")
                .join("SaveGames");
            if path.exists() {
                if let Some(selected_path) = rfd::FileDialog::new()
                    .set_directory(&path)
                    .add_filter("Save Files", &["sav"])
                    .add_filter("All Files", &["*"])
                    .pick_file()
                {
                    return Some((
                        selected_path.to_string_lossy().to_string(),
                        "File has been selected. Ready!".to_string()
                    ));
                }
            } else {
                if let Some(selected_path) = rfd::FileDialog::new()
                    .add_filter("Save Files", &["sav"])
                    .add_filter("All Files", &["*"])
                    .pick_file()
                {
                    return Some((
                        selected_path.to_string_lossy().to_string(),
                        "File has been selected. Ready!".to_string()
                    ));
                }
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Some(selected_path) = rfd::FileDialog::new()
            .add_filter("Save Files", &["sav"])
            .add_filter("All Files", &["*"])
            .pick_file()
        {
            return Some((
                selected_path.to_string_lossy().to_string(),
                "File has been selected. Ready!".to_string()
            ));
        }
    }
    None
}

pub fn create_backup(file_path: &str) -> Result<String, String> {
    let backup_path = format!("{}.bak", file_path);
    
    std::fs::copy(file_path, &backup_path)
        .map_err(|e| format!("Failed to create backup: {}", e))?;
        
    Ok(backup_path)
}

pub fn handle_demo_transfer(file_path: &str, remove_demo: bool) -> Result<(), String> {
    let path = std::path::Path::new(file_path);
    
    if let Some(filename) = path.file_name() {
        let filename_str = filename.to_string_lossy();
        
        if filename_str.contains("Demo00") {
            let new_filename = if remove_demo {
                filename_str.replace("Demo00", "00")  // For remover
            } else {
                filename_str.replace("Demo00", "")    // For replacer
            };
            
            if let Some(parent) = path.parent() {
                let new_path = parent.join(&new_filename);
                
                std::fs::rename(file_path, &new_path)
                    .map_err(|e| format!("Failed to transfer demo save: {}", e))?;
            }
        }
    }
    
    Ok(())
}

pub fn get_backup_filename(backup_path: &str) -> String {
    std::path::Path::new(backup_path)
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("backup.bak"))
        .to_string_lossy()
        .to_string()
}