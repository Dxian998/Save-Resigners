#![windows_subsystem = "windows"]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use slint::winit_030::{WinitWindowAccessor, winit, EventResult};

slint::include_modules!();

const HEADER_SIZE: usize = 40;
const MAGIC: [u8; 4] = [0xAC, 0xDB, 0xFE, 0x00];
const PT1_BASE: [u8; 16] = [ 0xAC, 0xDB, 0xFE, 0x00, 0x36, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00 ];
const BFR_KEYS: &[&str] = &["acblackflag", "acbf"];
const P40: u8 = 0x99;
const P44: u8 = 0x03;

fn format_uuid_input(raw: &str) -> String {
    let hex: String = raw
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .take(32)
        .collect();

    let mut out = String::with_capacity(36);
    for (i, c) in hex.chars().enumerate() {
        if matches!(i, 8 | 12 | 16 | 20) {
            out.push('-');
        }
        out.push(c);
    }
    out
}

fn is_valid_uuid(s: &str) -> bool {
    !s.is_empty() && uuid::Uuid::parse_str(s).is_ok()
}

fn is_valid_uuid_or_empty(s: &str) -> bool {
    s.is_empty() || uuid::Uuid::parse_str(s).is_ok()
}

fn build_key(uuid: &str, payload_size: usize) -> ([u8; 16], [u8; 16]) {
    let md5 = md5::compute(uuid.to_lowercase().as_bytes()).0;

    let mut pre = [0u8; 16];
    pre[0] = md5[0];
    pre[1] = md5[15];
    pre[2..16].copy_from_slice(&md5[1..15].iter().rev().cloned().collect::<Vec<_>>());

    let rot = payload_size % 16;
    let mut post = [0u8; 16];
    if rot == 0 {
        post.copy_from_slice(&pre);
    } else {
        post[..rot].copy_from_slice(&pre[16 - rot..]);
        post[rot..].copy_from_slice(&pre[..16 - rot]);
    }

    (pre, post)
}

fn xor_ecb(data: &[u8], key: &[u8; 16]) -> Vec<u8> {
    data.iter().enumerate().map(|(i, &b)| b ^ key[i % 16]).collect()
}

fn kpa(payload: &[u8]) -> Option<([u8; 16], [u8; 16])> {
    if payload.len() < 48 {
        return None;
    }

    let c1 = &payload[..16];
    let rot = payload.len() % 16;

    let mut post = [0u8; 16];
    for i in 0..16 {
        post[i] = c1[i] ^ PT1_BASE[i];
    }
    post[8]  = payload[40] ^ P40;
    post[12] = payload[44] ^ P44;

    let dec48 = xor_ecb(&payload[..48], &post);
    if dec48[..4] != MAGIC {
        return None;
    }
    if &dec48[36..48] != &[0x33, 0xAA, 0xFB, 0x57, 0x99, 0xFA, 0x04, 0x10, 0x03, 0x00, 0x05, 0x00] {
        return None;
    }

    let mut pre = [0u8; 16];
    if rot == 0 {
        pre.copy_from_slice(&post);
    } else {
        pre[..16 - rot].copy_from_slice(&post[rot..]);
        pre[16 - rot..].copy_from_slice(&post[..rot]);
    }

    Some((pre, post))
}

fn is_bfr_save(path: &Path, data: &[u8]) -> bool {
    if data.len() < HEADER_SIZE + 16 {
        return false;
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();
    let name_matches = BFR_KEYS.iter().any(|k| name.contains(k));
    name_matches || kpa(&data[HEADER_SIZE..]).is_some()
}

fn resign_file(path: &Path, backup_dir: &Path, current_uuid: &str, new_uuid: &str) -> Result<String, String> {
    let data = fs::read(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    if data.len() < HEADER_SIZE + 16 {
        return Err(format!("{} is too small to be a valid save.", path.display()));
    }

    let mut header = data[..HEADER_SIZE].to_vec();
    let payload = &data[HEADER_SIZE..];
    let p_size = payload.len();

    let (src_xor, prev_md5) = if !current_uuid.is_empty() {
        let md5 = md5::compute(current_uuid.to_lowercase().as_bytes()).0;
        let (_, xor) = build_key(current_uuid, p_size);
        (xor, md5)
    } else {
        let (pre, post) = kpa(payload)
            .ok_or_else(|| format!("{}: KPA failed — enter Current UUID manually.", path.display()))?;
        let mut md5 = [0u8; 16];
        md5[0] = pre[0];
        md5[15] = pre[1];
        for i in 1..15 {
            md5[i] = pre[16 - i];
        }
        (post, md5)
    };

    let decrypted = xor_ecb(payload, &src_xor);
    if decrypted[..4] != MAGIC {
        return Err(format!("{}: decryption failed. Wrong UUID or corrupted save.", path.display()));
    }

    let (_, tgt_xor) = build_key(new_uuid, p_size);
    let resigned = xor_ecb(&decrypted, &tgt_xor);

    header[8..40].fill(0);
    let mut out = header;
    out.extend_from_slice(&resigned);

    let bak = backup_dir.join(path.file_name().unwrap());
    if !bak.exists() {
        fs::copy(path, &bak)
            .map_err(|e| format!("Failed to backup {}: {}", path.display(), e))?;
    }

    if let Ok(meta) = fs::metadata(path) {
        let mut perms = meta.permissions();
        if perms.readonly() {
            perms.set_readonly(false);
            let _ = fs::set_permissions(path, perms);
        }
    }

    fs::write(path, &out).map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;

    let prev_md5_hex: String = prev_md5.iter().map(|b| format!("{:02x}", b)).collect();
    Ok(prev_md5_hex)
}

fn collect_save_files(path: &Path) -> Result<Vec<PathBuf>, String> {
    if path.is_file() {
        if let Ok(data) = fs::read(path) {
            if is_bfr_save(path, &data) {
                return Ok(vec![path.to_path_buf()]);
            }
        }
        return Err("Not a recognised AC Black Flag Resynced save.".into());
    }

    let entries = fs::read_dir(path)
        .map_err(|e| format!("Failed to read directory: {}", e))?;

    let files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .filter(|p| fs::read(p).map(|d| is_bfr_save(p, &d)).unwrap_or(false))
        .collect();

    if files.is_empty() {
        return Err("No valid AC Black Flag save files found in this directory.".into());
    }
    Ok(files)
}

fn resign_all(input: &str, current_uuid: &str, new_uuid: &str) -> Result<(), String> {
    let path = Path::new(input);
    let backup_dir = if path.is_dir() {
        path.parent().unwrap_or(path).join("Backup")
    } else {
        let parent = path.parent().unwrap_or(Path::new("."));
        parent.parent().unwrap_or(parent).join("Backup")
    };

    let files = if path.is_dir() {
        collect_save_files(path)?
    } else {
        vec![path.to_path_buf()]
    };

    fs::create_dir_all(&backup_dir)
        .map_err(|e| format!("Failed to create backup dir: {}", e))?;

    let mut info_content = format!(
        "This folder contains backups of your original Assassin's Creed Black Flag Resynced save files.\n\n\
        Source Location: {}\n\
        Target UUID: {}\n\n\
        Resigned Files:\n",
        path.display(),
        new_uuid
    );

    for file in &files {
        let prev_md5 = resign_file(file, &backup_dir, current_uuid, new_uuid)?;
        info_content.push_str(&format!(
            " - {}: Previous UUID MD5 = {}\n",
            file.file_name().unwrap().to_string_lossy(),
            prev_md5
        ));
    }

    let info_path = backup_dir.join("info.txt");
    let _ = fs::write(&info_path, info_content);

    Ok(())
}

fn set_status(ui: &AppWindow, msg: impl Into<slint::SharedString>, is_err: bool) {
    ui.set_status_text(msg.into());
    ui.set_status_is_error(is_err);
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;

    {
        let h = ui.as_weak();
        ui.on_current_uuid_edited(move |text| {
            let ui = h.unwrap();
            let fmt = format_uuid_input(&text);
            ui.set_current_uuid(fmt.clone().into());
            ui.set_current_uuid_valid(is_valid_uuid_or_empty(&fmt));
            ui.invoke_move_current_uuid_cursor_to_end();
        });
    }

    {
        let h = ui.as_weak();
        ui.on_new_uuid_edited(move |text| {
            let ui = h.unwrap();
            let fmt = format_uuid_input(&text);
            ui.set_new_uuid(fmt.clone().into());
            ui.set_new_uuid_valid(is_valid_uuid_or_empty(&fmt));
            ui.invoke_move_new_uuid_cursor_to_end();
        });
    }

    {
        let h = ui.as_weak();
        ui.on_browse_clicked(move || {
            let ui = h.unwrap();
            let Some(path) = rfd::FileDialog::new().pick_folder() else {
                return;
            };
            ui.set_input_path(path.to_string_lossy().to_string().into());
            match collect_save_files(&path) {
                Ok(files) => set_status(&ui, format!("Loaded: Found {} save files.", files.len()), false),
                Err(msg) => set_status(&ui, msg, true),
            }
        });
    }

    {
        let h = ui.as_weak();
        ui.window().on_winit_window_event(move |_, event| {
            if let winit::event::WindowEvent::DroppedFile(path) = event {
                let ui = h.unwrap();
                ui.set_input_path(path.to_string_lossy().to_string().into());
                match collect_save_files(path) {
                    Ok(files) => set_status(&ui, format!("Loaded: Found {} save files.", files.len()), false),
                    Err(msg) => set_status(&ui, msg, true),
                }
                EventResult::PreventDefault
            } else {
                EventResult::Propagate
            }
        });
    }

    {
        let h = ui.as_weak();
        ui.on_resign_clicked(move || {
            let ui = h.unwrap();

            let input_path   = ui.get_input_path().to_string();
            let current_uuid = ui.get_current_uuid().to_string();
            let new_uuid     = ui.get_new_uuid().to_string();

            if input_path.is_empty() {
                return set_status(&ui, "Input path is required.", true);
            }
            if !is_valid_uuid(&new_uuid) {
                return set_status(&ui, "New UUID is not well-formed.", true);
            }
            if !is_valid_uuid_or_empty(&current_uuid) {
                return set_status(&ui, "Current UUID is not well-formed.", true);
            }

            ui.set_busy(true);

            let weak = h.clone();
            std::thread::spawn(move || {
                let result = resign_all(&input_path, &current_uuid, &new_uuid);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = weak.upgrade() else { return };
                    ui.set_busy(false);
                    match result {
                        Ok(_)  => {
                            let msg = "Save resigned successfully!";
                            set_status(&ui, msg, false);
                            let weak2 = slint::Weak::clone(&weak);
                            std::thread::spawn(move || {
                                std::thread::sleep(Duration::from_secs(7));
                                let _ = slint::invoke_from_event_loop(move || {
                                    let Some(ui2) = weak2.upgrade() else { return };
                                    if ui2.get_status_text().as_str() == msg {
                                        set_status(&ui2, "", false);
                                    }
                                });
                            });
                        }
                        Err(e) => set_status(&ui, format!("Error: {}", e), true),
                    }
                });
            });
        });
    }

    ui.run()
}
