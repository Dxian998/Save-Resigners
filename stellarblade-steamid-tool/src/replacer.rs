use crate::utils;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};

#[derive(Default)]
pub struct ReplacerTab {
    pub file_path: String,
    pub new_steamid: String,
    pub current_steamid: String,
    pub status: String,
    pub backup_filename: String,
    pub transfer_demo_save: bool,
    pub show_demo_error: bool,
}

impl ReplacerTab {
    pub fn read_current_steamid(&mut self) {
        self.current_steamid.clear();
        
        if self.file_path.is_empty() {
            return;
        }
        
        if let Ok(mut file) = File::open(&self.file_path) {
            let mut data = Vec::new();
            if file.read_to_end(&mut data).is_ok() {
                for i in 0..=data.len().saturating_sub(17) {
                    if self.is_steamid_at_position(&data, i) {
                        if let Ok(steamid) = String::from_utf8(data[i..i+17].to_vec()) {
                            self.current_steamid = steamid;
                            break;
                        }
                    }
                }
            }
        }
    }

    pub fn handle_replace(&mut self) {
        if self.file_path.is_empty() {
            self.status = "(!) Please select a file first".to_string();
            self.backup_filename.clear();
        } else if self.new_steamid.is_empty() {
            self.status = "(!) Please enter a new SteamID".to_string();
            self.backup_filename.clear();
        } else if !self.current_steamid.is_empty() && self.current_steamid == self.new_steamid {
            self.status = "(!) Error: New SteamID is the same as current SteamID".to_string();
            self.backup_filename.clear();
        } else if self.transfer_demo_save && !self.file_path.contains("Demo00") {
            self.status = "❌ Error: File does not contain 'Demo00'. Cannot transfer demo save.".to_string();
            self.backup_filename.clear();
        } else {
            match self.replace_steamid() {
                Ok((count, backup_name)) => {
                    if count > 0 {
                        self.status = format!("✅ Successfully replaced SteamID!");
                        self.backup_filename = backup_name;
                        self.current_steamid = self.new_steamid.clone();
                    } else {
                        self.status = "⚠️ No valid SteamIDs found in file".to_string();
                        self.backup_filename.clear();
                    }
                }
                Err(e) => {
                    self.status = format!("❌ Error: {}", e);
                    self.backup_filename.clear();
                }
            }
        }
    }

    fn replace_steamid(&self) -> Result<(usize, String), String> {
        if !self.is_valid_steamid(&self.new_steamid) {
            return Err("Invalid SteamID format. Must be 17 digits starting with 7656".to_string());
        }

        let backup_path = utils::create_backup(&self.file_path)?;
        let backup_filename = utils::get_backup_filename(&backup_path);

        let mut file = File::open(&self.file_path)
            .map_err(|e| format!("Failed to open file: {}", e))?;

        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|e| format!("Failed to read file: {}", e))?;

        let mut replacements = 0;
        let mut i = 0;

        while i <= data.len().saturating_sub(17) {
            if self.is_steamid_at_position(&data, i) {
                for (j, &byte) in self.new_steamid.as_bytes().iter().enumerate() {
                    data[i + j] = byte;
                }
                replacements += 1;
                i += 17; 
            } else {
                i += 1;
            }
        }

        if replacements > 0 {
            let mut file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&self.file_path)
                .map_err(|e| {
                    let _ = std::fs::copy(&backup_path, &self.file_path);
                    format!("Failed to write to file: {}", e)
                })?;

            file.write_all(&data)
                .map_err(|e| {
                    let _ = std::fs::copy(&backup_path, &self.file_path);
                    format!("Failed to write to file: {}", e)
                })?;

            if self.transfer_demo_save {
                utils::handle_demo_transfer(&self.file_path, false)?;
            }
        }

        Ok((replacements, backup_filename))
    }

    fn is_valid_steamid(&self, steamid: &str) -> bool {
        steamid.len() == 17 
            && steamid.starts_with("7656") 
            && steamid.chars().all(|c| c.is_ascii_digit())
    }

    fn is_steamid_at_position(&self, data: &[u8], pos: usize) -> bool {
        if pos + 17 > data.len() {
            return false;
        }

        if &data[pos..pos + 4] != b"7656" {
            return false;
        }

        for i in pos..pos + 17 {
            if !data[i].is_ascii_digit() {
                return false;
            }
        }

        true
    }
}