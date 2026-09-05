use std::fs;
use std::path::Path;
use crate::crypto::{decrypt_payload, detect_save_platform, extract_header, HEADER_SIZE};

pub struct InspectionResult {
    pub loaded: bool,
    pub file_name: String,
    pub file_size: String,
    pub platform: String,
    pub slot_title: String,
    pub seed1: String,
    pub seed2: String,
    pub seed3: String,
    pub digest: String,
    pub hex_preview: String,
}

impl Default for InspectionResult {
    fn default() -> Self {
        Self {
            loaded: false,
            file_name: String::new(),
            file_size: String::new(),
            platform: String::new(),
            slot_title: String::new(),
            seed1: String::new(),
            seed2: String::new(),
            seed3: String::new(),
            digest: String::new(),
            hex_preview: String::new(),
        }
    }
}

pub fn inspect_save(path_str: &str) -> InspectionResult {
    let path = Path::new(path_str.trim());
    let raw = match fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            return InspectionResult { hex_preview: format!("Cannot read file: {}", e), ..Default::default() };
        }
    };
    if raw.len() <= HEADER_SIZE {
        return InspectionResult { hex_preview: format!("File too small ({} bytes) — not a valid GRW save.", raw.len()), ..Default::default() };
    }
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string();
    let file_size = format!("{} bytes ({:.1} KB)", raw.len(), raw.len() as f64 / 1024.0);
    let platform = match detect_save_platform(&raw) {
        Ok(p) => format!("{}", p),
        Err(_) => "Unknown".to_string(),
    };
    let slot_title = match extract_header(&raw) {
        Ok(h) => {
            let u16_slice: Vec<u16> = h[40..HEADER_SIZE].chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .take_while(|&c| c != 0)
                .collect();
            String::from_utf16_lossy(&u16_slice)
        }
        Err(_) => "None".to_string(),
    };
    match decrypt_payload(&raw[HEADER_SIZE..], None) {
        Ok(dec) => {
            let seed1 = format!("0x{:08X}", dec.seed1);
            let seed2 = format!("0x{:08X}", dec.seed2);
            let seed3 = format!("0x{:08X}", dec.seed3);
            let digest = dec.embedded_digest.iter().map(|b| format!("{:02x}", b)).collect();
            let mut hex = String::with_capacity(32768);
            hex.push_str("Offset(h)  00 01 02 03 04 05 06 07  08 09 0A 0B 0C 0D 0E 0F  Decoded Text\n");
            hex.push_str("---------  -----------------------------------------------  ----------------\n");
            let limit = dec.plaintext.len().min(4096);
            for (i, chunk) in dec.plaintext[..limit].chunks(16).enumerate() {
                let mut h1 = String::new();
                let mut h2 = String::new();
                for (ci, &b) in chunk.iter().enumerate() {
                    if ci < 8 {
                        if !h1.is_empty() { h1.push(' '); }
                        h1.push_str(&format!("{:02X}", b));
                    } else {
                        if !h2.is_empty() { h2.push(' '); }
                        h2.push_str(&format!("{:02X}", b));
                    }
                }
                let h_full = format!("{:<23}  {:<23}", h1, h2);
                let a: String = chunk.iter().map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' }).collect();
                hex.push_str(&format!("{:08X}   {:<48}  {:<16}\n", i * 16, h_full, a));
            }
            if dec.plaintext.len() > limit {
                hex.push_str(&format!("\n... [Showing first {} bytes of {} total bytes — Use Workshop tab to unpack full payload]", limit, dec.plaintext.len()));
            }
            InspectionResult {
                loaded: true,
                file_name,
                file_size,
                platform,
                slot_title,
                seed1,
                seed2,
                seed3,
                digest,
                hex_preview: hex,
            }
        }
        Err(e) => InspectionResult { hex_preview: format!("Decrypt failed: {}", e), ..Default::default() },
    }
}
