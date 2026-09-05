use std::fs;
use std::path::{Path, PathBuf};
use crate::crypto::{build_target_header, convert_full_save, normalize_account_id, GrwSavePlatform};

fn try_get_save_number(path: &Path) -> Option<u32> {
    let name = path.file_name()?.to_str()?;
    if !name.to_ascii_lowercase().ends_with(".save") { return None; }
    path.file_stem()?.to_str()?.parse::<u32>().ok()
}

fn find_save_files(path: &Path, is_folder: bool) -> Vec<(u32, PathBuf)> {
    if !is_folder {
        return vec![(try_get_save_number(path).unwrap_or(1), path.to_path_buf())];
    }
    let mut list = Vec::new();
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                if let Some(slot) = try_get_save_number(&p) {
                    list.push((slot, p));
                }
            }
        }
    }
    list.sort_by_key(|k| k.0);
    list
}

pub fn execute_resign(path_str: &str, is_folder: bool, target_id: &str, source_id: &str, platform_idx: i32, out_base: &str) -> String {
    let path = Path::new(path_str.trim());
    if path_str.trim().is_empty() || !path.exists() {
        return "[ERROR] Input path does not exist.".to_string();
    }
    let norm_to = match normalize_account_id(target_id) {
        Ok(id) => id,
        Err(e) => return format!("[ERROR] Target UUID invalid: {}", e),
    };
    let norm_from = if source_id.trim().is_empty() {
        None
    } else {
        match normalize_account_id(source_id) {
            Ok(id) => Some(id),
            Err(e) => return format!("[ERROR] Source UUID invalid: {}", e),
        }
    };
    let platform = if platform_idx == 1 { GrwSavePlatform::Steam } else { GrwSavePlatform::Ubisoft };
    let files = find_save_files(path, is_folder);
    if files.is_empty() {
        return "[ERROR] No valid .save file(s) found.".to_string();
    }
    let ts = chrono::Local::now().format("%Y-%m-%d_%H%M%S");
    let out_dir = PathBuf::from(if out_base.trim().is_empty() { "_OUTPUT" } else { out_base.trim() })
        .join(format!("{}_{}", ts, norm_to));
    if let Err(e) = fs::create_dir_all(&out_dir) {
        return format!("[ERROR] Cannot create output folder: {}", e);
    }
    let mut log = format!("> RESIGN: {} file(s) -> [{}] (Platform: {})\n", files.len(), norm_to, platform);
    let mut success = 0;
    for (slot, file_path) in &files {
        let raw = match fs::read(file_path) {
            Ok(data) => data,
            Err(e) => {
                log.push_str(&format!("  [-] Slot {}: read failed ({})\n", slot, e));
                continue;
            }
        };
        let target_header = match build_target_header(&raw, platform, *slot) {
            Ok(h) => h,
            Err(e) => {
                log.push_str(&format!("  [-] Slot {}: header failed ({})\n", slot, e));
                continue;
            }
        };
        match convert_full_save(&raw, norm_from.as_deref(), &norm_to, Some(&target_header)) {
            Ok(converted) => {
                let out_file = out_dir.join(file_path.file_name().unwrap());
                if let Err(e) = fs::write(&out_file, &converted) {
                    log.push_str(&format!("  [-] Slot {}: write failed ({})\n", slot, e));
                } else {
                    log.push_str(&format!("  [PASS] Slot {} -> {} ({} bytes)\n", slot, out_file.display(), converted.len()));
                    success += 1;
                }
            }
            Err(e) => log.push_str(&format!("  [-] Slot {}: resign failed ({})\n", slot, e)),
        }
    }
    log.push_str(&format!("\nCompleted: {}/{} files resigned successfully.\nOutput: {}", success, files.len(), out_dir.display()));
    log
}
