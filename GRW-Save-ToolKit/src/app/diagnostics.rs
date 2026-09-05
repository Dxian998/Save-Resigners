use std::fs;
use std::path::Path;
use crate::crypto::{convert_full_save, decrypt_payload, normalize_account_id, HEADER_SIZE};

pub fn verify_ownership(save_path: &str, user_id: &str) -> (bool, String) {
    let path = Path::new(save_path.trim());
    let raw = match fs::read(path) {
        Ok(d) => d,
        Err(e) => return (false, format!("[ERROR] Cannot read file: {}", e)),
    };
    if raw.len() <= HEADER_SIZE {
        return (false, "[ERROR] File is too small to be a GRW save.".to_string());
    }
    let norm_id = match normalize_account_id(user_id) {
        Ok(id) => id,
        Err(e) => return (false, format!("[ERROR] Invalid UUID format: {}", e)),
    };
    match decrypt_payload(&raw[HEADER_SIZE..], Some(&norm_id)) {
        Ok(_) => (true, format!("[PASS] SHA-256 Digest matches User ID [{}]!\nThis save was created or signed for this account.", norm_id)),
        Err(_) => (false, format!("[MISMATCH] Embedded digest does not match User ID [{}]!\nThe save belongs to a different account.", norm_id)),
    }
}

pub fn run_roundtrip(save_path: &str) -> (bool, String) {
    let path = Path::new(save_path.trim());
    let raw = match fs::read(path) {
        Ok(d) => d,
        Err(e) => return (false, format!("[ERROR] Cannot read file: {}", e)),
    };
    if raw.len() <= HEADER_SIZE {
        return (false, "[ERROR] File is too small to be a GRW save.".to_string());
    }
    let dec1 = match decrypt_payload(&raw[HEADER_SIZE..], None) {
        Ok(d) => d,
        Err(e) => return (false, format!("[FAIL] Initial decryption failed: {}", e)),
    };
    let dummy_id = "80f33a39-e682-4d1f-b693-39267e890df2";
    let converted = match convert_full_save(&raw, None, dummy_id, None) {
        Ok(c) => c,
        Err(e) => return (false, format!("[FAIL] Re-encryption failed: {}", e)),
    };
    let dec2 = match decrypt_payload(&converted[HEADER_SIZE..], Some(dummy_id)) {
        Ok(d) => d,
        Err(e) => return (false, format!("[FAIL] Verification decryption failed: {}", e)),
    };
    if dec1.plaintext == dec2.plaintext {
        (true, format!("[PASS] Roundtrip Verified 100% Bit-for-Bit!\nPlaintext payload length: {} bytes\nOriginal Seeds preserved:\n  Seed 1: 0x{:08X}\n  Seed 2: 0x{:08X}\n  Seed 3: 0x{:08X}\nZero byte deviation detected.", dec1.plaintext.len(), dec1.seed1, dec1.seed2, dec1.seed3))
    } else {
        (false, "[FAIL] Roundtrip mismatch! Decrypted payload differed from original.".to_string())
    }
}
