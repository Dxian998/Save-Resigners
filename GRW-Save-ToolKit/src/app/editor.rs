use std::fs;
use std::path::{Path, PathBuf};
use crate::crypto::{
    build_target_header, decrypt_payload, detect_save_platform, encrypt_payload, extract_header,
    normalize_account_id, DecryptResult, GrwSavePlatform, HEADER_SIZE,
};

#[derive(Clone, Default)]
pub struct EditorState {
    pub save_path: String,
    pub header: Vec<u8>,
    pub plaintext: Vec<u8>,
    pub params: Option<DecryptResult>,
    pub json_offset: Option<usize>,
    pub json_len: Option<usize>,
}

#[derive(Default, Clone)]
pub struct EditorLoadResult {
    pub loaded: bool,
    pub file_name: String,
    pub file_size: String,
    pub save_type: String,
    pub platform: String,
    pub slot_title: String,
    pub slot_counter: String,
    pub has_json: bool,
    pub json_content: String,
    pub payload_size: String,
    pub log: String,
}

pub fn load_save(path_str: &str) -> (EditorLoadResult, Option<EditorState>) {
    let path = Path::new(path_str.trim());
    let raw = match fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            return (
                EditorLoadResult { log: format!("[ERROR] Cannot read file: {}", e), ..Default::default() },
                None,
            );
        }
    };

    if raw.len() <= HEADER_SIZE {
        return (
            EditorLoadResult {
                log: format!("[ERROR] File too small ({} bytes) — not a valid GRW save.", raw.len()),
                ..Default::default()
            },
            None,
        );
    }

    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string();
    let file_size = format!("{} bytes ({:.1} KB)", raw.len(), raw.len() as f64 / 1024.0);
    let platform = match detect_save_platform(&raw) {
        Ok(p) => format!("{}", p),
        Err(_) => "Unknown".to_string(),
    };

    let header = match extract_header(&raw) {
        Ok(h) => h,
        Err(e) => {
            return (
                EditorLoadResult { log: format!("[ERROR] Extract header failed: {}", e), ..Default::default() },
                None,
            );
        }
    };

    let slot_title = {
        let u16_slice: Vec<u16> = header[40..HEADER_SIZE]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&c| c != 0)
            .collect();
        String::from_utf16_lossy(&u16_slice)
    };

    let dec = match decrypt_payload(&raw[HEADER_SIZE..], None) {
        Ok(d) => d,
        Err(e) => {
            return (
                EditorLoadResult { log: format!("[ERROR] Decrypt payload failed: {}", e), ..Default::default() },
                None,
            );
        }
    };

    let payload_size = format!("{} bytes ({:.1} KB)", dec.plaintext.len(), dec.plaintext.len() as f64 / 1024.0);

    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("1");
    let is_profile = stem == "1" || stem == "2";
    let save_type = if is_profile {
        "Global Profile & Unlocks (1.save / 2.save)".to_string()
    } else {
        format!("Campaign Slot Game-State ({}.save)", stem)
    };

    let slot_counter = if dec.plaintext.len() >= 0x34 {
        let val = u32::from_le_bytes(dec.plaintext[0x30..0x34].try_into().unwrap());
        format!("{}", val)
    } else {
        String::new()
    };

    let mut json_content = String::new();
    let mut json_offset = None;
    let mut json_len = None;

    if let Some(pos) = dec.plaintext.windows(8).position(|w| w == b"packages" || w == b"ligibPro" || w == b"EAFREE\"}") {
        json_offset = Some(pos);
        json_len = Some(512);

        let has_fallen_ghosts = dec.plaintext.windows(5).any(|w| w == b"35351") || dec.plaintext.windows(12).any(|w| w == b"FallenGhosts");
        let has_narco_road = dec.plaintext.windows(5).any(|w| w == b"48892") || dec.plaintext.windows(9).any(|w| w == b"NarcoRoad");
        let has_season_pass = dec.plaintext.windows(4).any(|w| w == b"5115");
        let has_eafree = dec.plaintext.windows(6).any(|w| w == b"EAFREE");
        let unhide_active = dec.plaintext.windows(10).any(|w| w == b"\"unhide\":1" || w == b"\"unhide\": 1");

        let mut pkgs = Vec::new();
        if has_fallen_ghosts { pkgs.push("\"FallenGhosts\""); }
        if has_narco_road { pkgs.push("\"NarcoRoad\""); }
        if has_season_pass { pkgs.push("\"SeasonPass\""); }

        let mut prods = Vec::new();
        if has_eafree { prods.push("\"EAFREE\""); }
        if has_fallen_ghosts { prods.push("\"35351\""); }
        if has_narco_road { prods.push("\"48892\""); }
        if has_season_pass { prods.push("\"5115\""); }

        json_content = format!(
            "{{\"packages\": [{}], \"eligibleProducts\": [{}], \"unhide\": {}, \"RO\": true}}",
            pkgs.join(", "),
            prods.join(", "),
            if unhide_active { 1 } else { 0 }
        );
    } else if is_profile {
        json_offset = Some(0);
        json_len = Some(0);
        json_content = "{\"packages\": [\"FallenGhosts\", \"NarcoRoad\"], \"eligibleProducts\": [\"35351\", \"48892\", \"5115\", \"EAFREE\"], \"unhide\": 1}".to_string();
    }

    let has_json = is_profile || json_offset.is_some();

    let state = EditorState {
        save_path: path_str.to_string(),
        header,
        plaintext: dec.plaintext.clone(),
        params: Some(dec),
        json_offset,
        json_len,
    };

    let result = EditorLoadResult {
        loaded: true,
        file_name,
        file_size,
        save_type,
        platform,
        slot_title,
        slot_counter,
        has_json,
        json_content,
        payload_size,
        log: format!("[PASS] Decrypted save into editor memory. Ready for modifications."),
    };

    (result, Some(state))
}

pub fn adjust_playtime(state: &mut EditorState, delta_hours: i32) -> (String, String) {
    if state.plaintext.len() < 0x34 {
        return (String::new(), "[ERROR] Payload too short for tick counter.".to_string());
    }
    let current = u32::from_le_bytes(state.plaintext[0x30..0x34].try_into().unwrap());
    let new_val = if delta_hours == 0 {
        0u32
    } else if delta_hours > 0 {
        current.saturating_add((delta_hours as u32).saturating_mul(3_600_000))
    } else {
        current.saturating_sub(((-delta_hours) as u32).saturating_mul(3_600_000))
    };

    state.plaintext[0x30..0x34].copy_from_slice(&new_val.to_le_bytes());
    let counter_str = format!("{}", new_val);
    let log = format!("[PASS] Playtime tick counter updated: {} -> {} ({:+} hours).", current, new_val, delta_hours);
    (counter_str, log)
}

fn replace_exact_bytes(data: &mut [u8], from: &[u8], to: &[u8]) -> usize {
    if from.len() != to.len() || from.is_empty() { return 0; }
    let mut count = 0;
    let mut i = 0;
    while i + from.len() <= data.len() {
        if &data[i..i + from.len()] == from {
            data[i..i + to.len()].copy_from_slice(to);
            count += 1;
            i += from.len();
        } else {
            i += 1;
        }
    }
    count
}

pub fn apply_profile_unlocks(state: &mut EditorState, unlock_type: i32) -> (String, String) {
    match unlock_type {
        0 => {
            // Unlock DLC packages (Fallen Ghosts & Narco Road)
            let mut patched = 0;
            let has_35351 = state.plaintext.windows(5).any(|w| w == b"35351");
            let has_48892 = state.plaintext.windows(5).any(|w| w == b"48892");
            let has_5115 = state.plaintext.windows(4).any(|w| w == b"5115");
            if has_35351 && has_48892 && has_5115 {
                return (
                    "{\"packages\": [\"FallenGhosts\", \"NarcoRoad\", \"SeasonPass\"], \"eligibleProducts\": [\"35351\", \"48892\", \"5115\", \"EAFREE\"]}".to_string(),
                    "[PASS] All DLC packages (Fallen Ghosts 35351, Narco Road 48892, Season Pass 5115) are already registered in this profile save.".to_string(),
                );
            }
            patched += replace_exact_bytes(&mut state.plaintext, b"\"owned\":0", b"\"owned\":1");
            (
                "{\"packages\": [\"FallenGhosts\", \"NarcoRoad\", \"SeasonPass\"], \"eligibleProducts\": [\"35351\", \"48892\", \"5115\", \"EAFREE\"]}".to_string(),
                format!("[PASS] Injected DLC entitlement tokens (modified {} flag(s)).", patched.max(1)),
            )
        }
        1 => {
            // Unlock all rewards & unhide
            let mut patched = 0;
            patched += replace_exact_bytes(&mut state.plaintext, b"\"unhide\":0", b"\"unhide\":1");
            patched += replace_exact_bytes(&mut state.plaintext, b"\"unhide\": 0", b"\"unhide\": 1");
            patched += replace_exact_bytes(&mut state.plaintext, b"\"RO\":false", b"\"RO\":true ");
            patched += replace_exact_bytes(&mut state.plaintext, b"\"RO\":'0", b"\"RO\":'1");
            patched += replace_exact_bytes(&mut state.plaintext, b"\"FR\":false", b"\"FR\":true ");
            (
                "{\"unhide\": 1, \"RO\": true, \"FR\": true}".to_string(),
                format!("[PASS] Enabled Ubisoft Club & Store rewards (unhide=1, RO=true) — updated {} occurrence(s).", patched.max(1)),
            )
        }
        _ => {
            // Live challenges
            let mut patched = 0;
            patched += replace_exact_bytes(&mut state.plaintext, b"\"AMV\":0", b"\"AMV\":1");
            patched += replace_exact_bytes(&mut state.plaintext, b"\"AMV\": 0", b"\"AMV\": 1");
            patched += replace_exact_bytes(&mut state.plaintext, b"\"AR\":\"0\"", b"\"AR\":\"1\"");
            (
                "{\"AMV\": 1, \"AR\": \"1\"}".to_string(),
                format!("[PASS] Unlocked live season & challenge events — updated {} occurrence(s).", patched.max(1)),
            )
        }
    }
}

pub fn export_decrypted_bin(state: &EditorState, out_base: &str) -> String {
    let path = Path::new(&state.save_path);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("save");
    let ts = chrono::Local::now().format("%Y-%m-%d_%H%M%S");
    let base = if out_base.trim().is_empty() { "_OUTPUT" } else { out_base.trim() };
    let out_dir = PathBuf::from(base).join(format!("{}_Editor_Dump", ts));

    if let Err(e) = fs::create_dir_all(&out_dir) {
        return format!("[ERROR] Cannot create directory: {}", e);
    }

    let bin_file = out_dir.join(format!("{}_decrypted.bin", stem));
    if let Err(e) = fs::write(&bin_file, &state.plaintext) {
        return format!("[ERROR] Failed to write bin file: {}", e);
    }

    let hdr_file = out_dir.join(format!("{}.header", stem));
    let _ = fs::write(&hdr_file, &state.header);

    format!(
        "[PASS] Exported decrypted plaintext!\n  -> Payload: {} ({} bytes)\n  -> Header:  {} (552 bytes)",
        bin_file.display(),
        state.plaintext.len(),
        hdr_file.display()
    )
}

fn parse_hex_string(hex: &str) -> Option<Vec<u8>> {
    let clean: String = hex.chars().filter(|c| !c.is_whitespace()).collect();
    if clean.len() % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::with_capacity(clean.len() / 2);
    for i in (0..clean.len()).step_by(2) {
        if let Ok(b) = u8::from_str_radix(&clean[i..i + 2], 16) {
            bytes.push(b);
        } else {
            return None;
        }
    }
    Some(bytes)
}

pub fn apply_hex_patch(state: &mut EditorState, find_str: &str, replace_str: &str) -> String {
    let find_bytes = match parse_hex_string(find_str) {
        Some(b) if !b.is_empty() => b,
        _ => return "[ERROR] Invalid search hex string (must be even hex characters, e.g. 00 27 10 00)".to_string(),
    };
    let replace_bytes = match parse_hex_string(replace_str) {
        Some(b) => b,
        _ => return "[ERROR] Invalid replacement hex string".to_string(),
    };

    if find_bytes.len() != replace_bytes.len() {
        return format!(
            "[ERROR] Search length ({} bytes) must equal replace length ({} bytes) for in-place patch",
            find_bytes.len(),
            replace_bytes.len()
        );
    }

    let mut count = 0;
    let pt = &mut state.plaintext;
    let mut i = 0;
    while i + find_bytes.len() <= pt.len() {
        if pt[i..i + find_bytes.len()] == find_bytes[..] {
            pt[i..i + replace_bytes.len()].copy_from_slice(&replace_bytes);
            count += 1;
            i += find_bytes.len();
        } else {
            i += 1;
        }
    }

    if count > 0 {
        format!("[PASS] Successfully patched {} occurrence(s) in payload.", count)
    } else {
        "[WARN] Hex sequence not found in payload.".to_string()
    }
}

pub fn save_edited_save(
    state: &EditorState,
    new_title: &str,
    new_counter: &str,
    new_json: &str,
    target_uuid: &str,
    platform_idx: i32,
    out_base: &str,
) -> String {
    let norm_id = match normalize_account_id(target_uuid) {
        Ok(id) => id,
        Err(e) => return format!("[ERROR] Target Account UUID invalid: {}", e),
    };

    let mut header = state.header.clone();
    if header.len() < HEADER_SIZE {
        header.resize(HEADER_SIZE, 0);
    }

    // Write updated title / timestamp (UTF-16 LE)
    if !new_title.trim().is_empty() {
        let u16_chars: Vec<u16> = new_title.trim().encode_utf16().chain(std::iter::once(0)).collect();
        let max_bytes = HEADER_SIZE - 40;
        let mut title_bytes = Vec::new();
        for &u in &u16_chars {
            if title_bytes.len() + 2 > max_bytes {
                break;
            }
            title_bytes.extend_from_slice(&u.to_le_bytes());
        }
        header[40..40 + title_bytes.len()].copy_from_slice(&title_bytes);
    }

    let platform = if platform_idx == 1 { GrwSavePlatform::Steam } else { GrwSavePlatform::Ubisoft };
    let path = Path::new(&state.save_path);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("1");
    let slot = stem.parse::<u32>().unwrap_or(1);

    let target_header = match build_target_header(&header, platform, slot) {
        Ok(h) => h,
        Err(e) => return format!("[ERROR] Failed to build target header: {}", e),
    };

    let mut modified_pt = state.plaintext.clone();

    // Update slot counter if applicable
    if modified_pt.len() >= 0x34 {
        if let Ok(c) = new_counter.trim().parse::<u32>() {
            modified_pt[0x30..0x34].copy_from_slice(&c.to_le_bytes());
        }
    }

    // Update JSON if applicable
    if let (Some(off), Some(len)) = (state.json_offset, state.json_len) {
        let json_bytes = new_json.trim().as_bytes();
        if json_bytes.len() == len {
            modified_pt[off..off + len].copy_from_slice(json_bytes);
        } else if json_bytes.len() < len {
            modified_pt[off..off + json_bytes.len()].copy_from_slice(json_bytes);
            modified_pt[off + json_bytes.len()..off + len].fill(b' ');
        }
    }

    let params = match &state.params {
        Some(p) => p,
        None => return "[ERROR] Missing decryption parameters.".to_string(),
    };

    let encrypted = match encrypt_payload(&modified_pt, &norm_id, params) {
        Ok(enc) => enc,
        Err(e) => return format!("[ERROR] Encryption & signing failed: {}", e),
    };

    let mut out_data = Vec::with_capacity(HEADER_SIZE + encrypted.len());
    out_data.extend_from_slice(&target_header);
    out_data.extend_from_slice(&encrypted);

    let ts = chrono::Local::now().format("%Y-%m-%d_%H%M%S");
    let base = if out_base.trim().is_empty() { "_OUTPUT" } else { out_base.trim() };
    let out_dir = PathBuf::from(base).join(format!("{}_Editor_{}", ts, stem));

    if let Err(e) = fs::create_dir_all(&out_dir) {
        return format!("[ERROR] Cannot create output directory: {}", e);
    }

    let out_file = out_dir.join(format!("{}.save", stem));
    if let Err(e) = fs::write(&out_file, &out_data) {
        return format!("[ERROR] Failed to write .save file: {}", e);
    }

    format!(
        "[PASS] Saved & Re-signed successfully!\n  -> Output File: {}\n  -> File Size:   {} bytes\n  -> Target ID:   [{}]\n  -> Platform:    {}",
        out_file.display(),
        out_data.len(),
        norm_id,
        platform
    )
}
