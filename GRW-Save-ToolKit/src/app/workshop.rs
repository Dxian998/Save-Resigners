use std::fs;
use std::path::{Path, PathBuf};
use crate::crypto::{build_target_header, decrypt_payload, encrypt_payload, extract_header, get_default_header, normalize_account_id, DecryptResult, GrwSavePlatform, HEADER_SIZE};

pub fn extract_payload(save_path_str: &str, out_base: &str) -> String {
    let path = Path::new(save_path_str.trim());
    let raw = match fs::read(path) {
        Ok(d) => d,
        Err(e) => return format!("[ERROR] Cannot read save: {}", e),
    };
    if raw.len() <= HEADER_SIZE {
        return "[ERROR] Save file is too small.".to_string();
    }
    let header = match extract_header(&raw) {
        Ok(h) => h,
        Err(e) => return format!("[ERROR] Failed to extract header: {}", e),
    };
    let dec = match decrypt_payload(&raw[HEADER_SIZE..], None) {
        Ok(d) => d,
        Err(e) => return format!("[ERROR] Decryption failed: {}", e),
    };
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("1");
    let ts = chrono::Local::now().format("%Y-%m-%d_%H%M%S");
    let out_dir = PathBuf::from(if out_base.trim().is_empty() { "_OUTPUT" } else { out_base.trim() })
        .join(format!("{}_Workshop_{}", ts, stem));
    if let Err(e) = fs::create_dir_all(&out_dir) {
        return format!("[ERROR] Cannot create output directory: {}", e);
    }
    let bin_path = out_dir.join(format!("{}.bin", stem));
    let hdr_path = out_dir.join(format!("{}.header", stem));
    if let Err(e) = fs::write(&bin_path, &dec.plaintext) {
        return format!("[ERROR] Cannot write .bin: {}", e);
    }
    if let Err(e) = fs::write(&hdr_path, &header) {
        return format!("[ERROR] Cannot write .header: {}", e);
    }
    format!("[PASS] Extracted successfully!\n  -> Payload: {} ({} bytes)\n  -> Header:  {} ({} bytes)\nOutput: {}", bin_path.display(), dec.plaintext.len(), hdr_path.display(), header.len(), out_dir.display())
}

pub fn repack_payload(bin_path_str: &str, hdr_path_str: &str, target_id: &str, platform_idx: i32, out_base: &str) -> String {
    let bin_path = Path::new(bin_path_str.trim());
    let raw_bin = match fs::read(bin_path) {
        Ok(d) => d,
        Err(e) => return format!("[ERROR] Cannot read payload .bin: {}", e),
    };
    let norm_id = match normalize_account_id(target_id) {
        Ok(id) => id,
        Err(e) => return format!("[ERROR] Target UUID invalid: {}", e),
    };
    let platform = if platform_idx == 1 { GrwSavePlatform::Steam } else { GrwSavePlatform::Ubisoft };
    let base_header = if !hdr_path_str.trim().is_empty() && Path::new(hdr_path_str.trim()).exists() {
        fs::read(hdr_path_str.trim()).unwrap_or_else(|_| get_default_header())
    } else {
        let auto_hdr = bin_path.with_extension("header");
        if auto_hdr.exists() {
            fs::read(auto_hdr).unwrap_or_else(|_| get_default_header())
        } else {
            get_default_header()
        }
    };
    let slot = bin_path.file_stem().and_then(|s| s.to_str()).and_then(|s| s.parse::<u32>().ok()).unwrap_or(1);
    let target_header = match build_target_header(&base_header, platform, slot) {
        Ok(h) => h,
        Err(e) => return format!("[ERROR] Header build failed: {}", e),
    };
    let default_params = DecryptResult {
        plaintext: raw_bin.clone(),
        seed1: 0x12345678,
        seed2: 0x23456789,
        seed3: 0x3456789A,
        shuffle_padding_bytes: Vec::new(),
        tea_padding_bytes: Vec::new(),
        embedded_digest: [0u8; 32],
    };
    let encrypted = match encrypt_payload(&raw_bin, &norm_id, &default_params) {
        Ok(enc) => enc,
        Err(e) => return format!("[ERROR] Encryption failed: {}", e),
    };
    let mut out_data = Vec::with_capacity(HEADER_SIZE + encrypted.len());
    out_data.extend_from_slice(&target_header);
    out_data.extend_from_slice(&encrypted);

    let ts = chrono::Local::now().format("%Y-%m-%d_%H%M%S");
    let out_dir = PathBuf::from(if out_base.trim().is_empty() { "_OUTPUT" } else { out_base.trim() })
        .join(format!("{}_Repack_{}", ts, norm_id));
    if let Err(e) = fs::create_dir_all(&out_dir) {
        return format!("[ERROR] Cannot create directory: {}", e);
    }
    let stem = bin_path.file_stem().and_then(|s| s.to_str()).unwrap_or("1");
    let out_save = out_dir.join(format!("{}.save", stem));
    if let Err(e) = fs::write(&out_save, &out_data) {
        return format!("[ERROR] Cannot write .save: {}", e);
    }
    format!("[PASS] Repacked save successfully!\n  -> File: {} ({} bytes)\n  -> Platform: {}\n  -> Target ID: [{}]\nOutput: {}", out_save.display(), out_data.len(), platform, norm_id, out_dir.display())
}
