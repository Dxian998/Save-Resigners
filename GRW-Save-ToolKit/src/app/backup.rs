use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use std::thread;
use std::time::Duration;

#[derive(Clone)]
pub struct BackupEntry {
    pub name: String,
    pub date: String,
    pub size: String,
    pub full_path: String,
}

pub struct BackupManager {
    auto_stop_flag: Option<Arc<AtomicBool>>,
}

impl BackupManager {
    pub fn new() -> Self {
        Self { auto_stop_flag: None }
    }

    pub fn scan_uuids(&self, ubi_dir: &str) -> Vec<String> {
        let p = Path::new(ubi_dir.trim());
        let mut uuids = Vec::new();
        if let Ok(entries) = fs::read_dir(p) {
            for entry in entries.flatten() {
                if entry.file_type().map_or(false, |ft| ft.is_dir()) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.len() >= 32 {
                        uuids.push(name);
                    }
                }
            }
        }
        uuids.sort();
        uuids
    }

    pub fn resolve_uuid_dir(&self, ubi_dir: &str, target_uuid: &str) -> Option<PathBuf> {
        let base = Path::new(ubi_dir.trim());
        let uuid_trim = target_uuid.trim();
        if uuid_trim.is_empty() { return None; }
        let direct = base.join(uuid_trim);
        if direct.exists() && direct.is_dir() {
            return Some(direct);
        }
        let clean = uuid_trim.replace('-', "").to_ascii_lowercase();
        if let Ok(entries) = fs::read_dir(base) {
            for entry in entries.flatten() {
                if entry.file_type().map_or(false, |ft| ft.is_dir()) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.replace('-', "").to_ascii_lowercase() == clean {
                        return Some(entry.path());
                    }
                }
            }
        }
        None
    }

    pub fn create_snapshot(&self, ubi_dir: &str, target_uuid: &str, target_dir_str: &str) -> (bool, String) {
        let src_dir = match self.resolve_uuid_dir(ubi_dir, target_uuid) {
            Some(d) => d,
            None => return (false, format!("[ERROR] UUID folder '{}' not found in '{}'", target_uuid.trim(), ubi_dir.trim())),
        };
        let out_dir_base = PathBuf::from(if target_dir_str.trim().is_empty() { "_BACKUPS" } else { target_dir_str.trim() });
        let uuid_name = src_dir.file_name().and_then(|n| n.to_str()).unwrap_or("uuid");
        let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let bak_folder = out_dir_base.join(format!("{}_{}.bak", uuid_name, ts));
        if let Err(e) = fs::create_dir_all(&bak_folder) {
            return (false, format!("[ERROR] Cannot create backup directory: {}", e));
        }
        let mut count = 0;
        if let Err(e) = copy_dir_all(&src_dir, &bak_folder, &mut count) {
            return (false, format!("[ERROR] Backup failed: {}", e));
        }
        (true, format!("[PASS] Snapshot saved! Copied {} file(s) to:\n{}", count, bak_folder.display()))
    }

    pub fn create_snapshot_file(&self, src_path: &str, target_dir_str: &str) -> (bool, String) {
        let src = Path::new(src_path.trim());
        if !src.exists() {
            return (false, format!("[ERROR] Source path does not exist: {}", src_path.trim()));
        }
        let out_dir_base = PathBuf::from(if target_dir_str.trim().is_empty() { "_BACKUPS" } else { target_dir_str.trim() });
        let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
        if src.is_dir() {
            let folder_name = src.file_name().and_then(|n| n.to_str()).unwrap_or("save");
            let bak_folder = out_dir_base.join(format!("{}_{}.bak", folder_name, ts));
            if let Err(e) = fs::create_dir_all(&bak_folder) {
                return (false, format!("[ERROR] Cannot create backup directory: {}", e));
            }
            let mut count = 0;
            if let Err(e) = copy_dir_all(src, &bak_folder, &mut count) {
                return (false, format!("[ERROR] Backup failed: {}", e));
            }
            (true, format!("[PASS] Snapshot saved! Copied {} file(s) to:\n{}", count, bak_folder.display()))
        } else {
            if let Err(e) = fs::create_dir_all(&out_dir_base) {
                return (false, format!("[ERROR] Cannot create backup directory: {}", e));
            }
            let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("save");
            let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("save");
            let bak_name = format!("{}_{}.{}", stem, ts, ext);
            let dest = out_dir_base.join(&bak_name);
            if let Err(e) = fs::copy(src, &dest) {
                return (false, format!("[ERROR] Backup failed: {}", e));
            }
            (true, format!("[PASS] Snapshot saved to:\n{}", dest.display()))
        }
    }

    pub fn list_backups(&self, target_dir_str: &str) -> Vec<BackupEntry> {
        let out_dir = PathBuf::from(if target_dir_str.trim().is_empty() { "_BACKUPS" } else { target_dir_str.trim() });
        let mut list = Vec::new();
        if let Ok(entries) = fs::read_dir(out_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                let is_bak = p.file_name().and_then(|n| n.to_str()).map_or(false, |s| s.ends_with(".bak"));
                if is_bak {
                    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                    let meta = entry.metadata().ok();
                    let size = if p.is_dir() { calc_dir_size(&p) } else { meta.as_ref().map_or(0, |m| m.len()) };
                    let size_str = format!("{:.1} KB", size as f64 / 1024.0);
                    let date_str = meta.and_then(|m| m.modified().ok())
                        .map(|t| {
                            let dt: chrono::DateTime<chrono::Local> = t.into();
                            dt.format("%Y-%m-%d %H:%M:%S").to_string()
                        })
                        .unwrap_or_else(|| "Unknown".to_string());
                    list.push(BackupEntry {
                        name,
                        date: date_str,
                        size: size_str,
                        full_path: p.to_string_lossy().to_string(),
                    });
                }
            }
        }
        list.sort_by(|a, b| b.date.cmp(&a.date));
        list
    }

    pub fn restore_backup(&self, backup_path_str: &str, ubi_dir: &str, target_uuid: &str) -> (bool, String) {
        let bak = Path::new(backup_path_str.trim());
        if !bak.exists() {
            return (false, "[ERROR] Backup path does not exist.".to_string());
        }
        let dest_dir = match self.resolve_uuid_dir(ubi_dir, target_uuid) {
            Some(d) => d,
            None => {
                let base = Path::new(ubi_dir.trim());
                if !base.exists() {
                    return (false, format!("[ERROR] Ubisoft savegames folder does not exist: {}", ubi_dir.trim()));
                }
                let fallback = base.join(target_uuid.trim());
                fs::create_dir_all(&fallback).ok();
                fallback
            }
        };
        let mut count = 0;
        if bak.is_dir() {
            if let Err(e) = copy_dir_all(bak, &dest_dir, &mut count) {
                return (false, format!("[ERROR] Restore failed: {}", e));
            }
        } else if let Err(e) = fs::copy(bak, dest_dir.join(bak.file_name().unwrap_or_default())) {
            return (false, format!("[ERROR] Single-file restore failed: {}", e));
        } else {
            count = 1;
        }
        (true, format!("[PASS] Restored {} file(s) to:\n{}", count, dest_dir.display()))
    }

    pub fn restore_backup_file(&self, backup_path_str: &str, dest_path: &str) -> (bool, String) {
        let bak = Path::new(backup_path_str.trim());
        if !bak.exists() {
            return (false, "[ERROR] Backup path does not exist.".to_string());
        }
        let dest = Path::new(dest_path.trim());
        if bak.is_dir() {
            let mut count = 0;
            if let Err(e) = fs::create_dir_all(dest) {
                return (false, format!("[ERROR] Cannot create destination: {}", e));
            }
            if let Err(e) = copy_dir_all(bak, dest, &mut count) {
                return (false, format!("[ERROR] Restore failed: {}", e));
            }
            (true, format!("[PASS] Restored {} file(s) to:\n{}", count, dest.display()))
        } else {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).ok();
            }
            match fs::copy(bak, dest) {
                Ok(_) => (true, format!("[PASS] Restored to:\n{}", dest.display())),
                Err(e) => (false, format!("[ERROR] Restore failed: {}", e)),
            }
        }
    }

    pub fn toggle_auto(&mut self, enabled: bool, interval_idx: i32, ubi_dir: String, target_uuid: String, target_dir: String) {
        if let Some(flag) = self.auto_stop_flag.take() {
            flag.store(true, Ordering::SeqCst);
        }
        if !enabled { return; }
        let mins = match interval_idx {
            0 => 5,
            1 => 10,
            2 => 15,
            3 => 30,
            _ => 60,
        };
        let stop_flag = Arc::new(AtomicBool::new(false));
        self.auto_stop_flag = Some(stop_flag.clone());
        thread::spawn(move || {
            while !stop_flag.load(Ordering::SeqCst) {
                let base = std::path::Path::new(&ubi_dir);
                let uuid_trim = target_uuid.trim();
                let src_dir = base.join(uuid_trim);
                if src_dir.exists() && src_dir.is_dir() {
                    let out_dir_base = PathBuf::from(if target_dir.trim().is_empty() { "_BACKUPS" } else { target_dir.trim() });
                    let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
                    let bak_folder = out_dir_base.join(format!("{}_{}_auto.bak", uuid_trim, ts));
                    let mut count = 0;
                    copy_dir_all(&src_dir, &bak_folder, &mut count).ok();
                }
                for _ in 0..(mins * 12) {
                    if stop_flag.load(Ordering::SeqCst) { break; }
                    thread::sleep(Duration::from_secs(5));
                }
            }
        });
    }
}

fn calc_dir_size(dir: &Path) -> u64 {
    let mut total = 0;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            } else if p.is_dir() {
                total += calc_dir_size(&p);
            }
        }
    }
    total
}

fn copy_dir_all(src: &Path, dst: &Path, count: &mut usize) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest_path, count)?;
        } else {
            fs::copy(entry.path(), &dest_path)?;
            *count += 1;
        }
    }
    Ok(())
}
