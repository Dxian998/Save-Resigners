use crate::utils;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};

#[derive(Default)]
pub struct RemoverTab {
    pub file_path: String,
    pub current_steamid: String,
    pub status: String,
    pub backup_filename: String,
    pub transfer_demo_save: bool,
}

#[derive(Debug, Clone)]
struct SteamIdInfo {
    start_pos: usize,
    steamid_length: usize,
    total_length: usize,
}

impl RemoverTab {
    pub fn read_current_steamid(&mut self) {
        self.current_steamid.clear();
        
        if self.file_path.is_empty() {
            return;
        }
        
        if let Ok(mut file) = File::open(&self.file_path) {
            let mut data = Vec::new();
            if file.read_to_end(&mut data).is_ok() {
                let search_start = data.len().saturating_sub(1024);
                
                for i in search_start..data.len().saturating_sub(50) {
                    if let Some(steamid_info) = self.find_steamid_at(&data, i) {
                        if let Some(steamid) = self.extract_steamid_from_pattern(&data, &steamid_info) {
                            self.current_steamid = steamid;
                            break;
                        }
                    }
                }
            }
        }
    }

    pub fn handle_remove(&mut self) {
        if self.file_path.is_empty() {
            self.status = "(!) Please select a file first".to_string();
            self.backup_filename.clear();
        } else if self.transfer_demo_save && !self.file_path.contains("Demo00") {
            self.status = "❌ Error: File does not contain 'Demo00'. Cannot transfer demo save.".to_string();
            self.backup_filename.clear();
        } else {
            match self.remove_steamid() {
                Ok(backup_name) => {
                    if self.has_steamids().unwrap_or(false) {
                        self.status = "(!) No SteamIDs found - file is already universal".to_string();
                        self.backup_filename.clear();
                    } else {
                        self.status = "✅ Successfully removed SteamID! Save is now universal.".to_string();
                        self.backup_filename = backup_name;
                        self.current_steamid.clear();
                    }
                }
                Err(e) => {
                    self.status = format!("❌ Error: {}", e);
                    self.backup_filename.clear();
                }
            }
        }
    }

    fn has_steamids(&self) -> Result<bool, String> {
        let mut file = File::open(&self.file_path)
            .map_err(|e| format!("Failed to open file: {}", e))?;

        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|e| format!("Failed to read file: {}", e))?;

        let search_start = data.len().saturating_sub(1024);
        
        for i in search_start..data.len().saturating_sub(50) {
            if let Some(_steamid_info) = self.find_steamid_at(&data, i) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn remove_steamid(&self) -> Result<String, String> {
        if !self.has_steamids()? {
            return Err("No SteamIDs found - file is already universal".to_string());
        }

        let backup_path = utils::create_backup(&self.file_path)?;
        let backup_filename = utils::get_backup_filename(&backup_path);

        let mut file = File::open(&self.file_path)
            .map_err(|e| format!("Failed to open file: {}", e))?;

        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|e| format!("Failed to read file: {}", e))?;

        let mut modifications = Vec::new();
        
        let search_start = data.len().saturating_sub(1024);
        let mut i = search_start;

        while i < data.len().saturating_sub(50) {
            if let Some(steamid_info) = self.find_steamid_at(&data, i) {
                modifications.push(steamid_info.clone());
                i += steamid_info.total_length;
            } else {
                i += 1;
            }
        }

        // Apply modifications in reverse order to maintain positions
        modifications.reverse();
        for steamid_info in modifications {
            self.replace_steamid_pattern(&mut data, steamid_info);
        }

        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.file_path)
            .map_err(|e| format!("Failed to write to file: {}", e))?;

        file.write_all(&data)
            .map_err(|e| format!("Failed to write to file: {}", e))?;

        if self.transfer_demo_save {
            utils::handle_demo_transfer(&self.file_path, true)?;
        }

        Ok(backup_filename)
    }

    fn find_steamid_at(&self, data: &[u8], pos: usize) -> Option<SteamIdInfo> {
        let str_property = b"StrProperty";
        if pos + 11 > data.len() || data[pos..pos + 11] != *str_property {
            return None;
        }

        let search_start = pos + 11;
        let none_pattern = b"None";
        
        for i in search_start..data.len().saturating_sub(4).min(search_start + 100) {
            if i + 4 <= data.len() && data[i..i + 4] == *none_pattern {
                if i >= 5 && data[i-5..i] == [0x00, 0x05, 0x00, 0x00, 0x00] {
                    let gap_data = &data[search_start..i-5];
                    
                    if self.is_steamid_data(&gap_data) {
                        return Some(SteamIdInfo {
                            start_pos: search_start,
                            steamid_length: (i - 5) - search_start,
                            total_length: (i + 4) - pos,
                        });
                    }
                }
            }
        }

        None
    }

    fn is_steamid_data(&self, data: &[u8]) -> bool {
        if data.len() < 20 {
            return false;
        }

        if data.len() >= 14 + 17 {
            if data[0] == 0x00 && data[1] == 0x16 && data[2] == 0x00 && data[3] == 0x00 && data[4] == 0x00 {
                for i in 5..std::cmp::min(data.len().saturating_sub(21), 15) {
                    if i + 4 < data.len() && 
                       data[i] == 0x00 && data[i+1] == 0x12 && data[i+2] == 0x00 && 
                       data[i+3] == 0x00 && data[i+4] == 0x00 {
                        
                        let steamid_start = i + 5;
                        if steamid_start + 17 <= data.len() {
                            let potential_steamid = &data[steamid_start..steamid_start + 17];
                            
                            if potential_steamid.iter().all(|&b| b >= b'0' && b <= b'9') {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    fn extract_steamid_from_pattern(&self, data: &[u8], steamid_info: &SteamIdInfo) -> Option<String> {
        let gap_data = &data[steamid_info.start_pos..steamid_info.start_pos + steamid_info.steamid_length];
        
        if gap_data.len() >= 14 + 17 {
            if gap_data[0] == 0x00 && gap_data[1] == 0x16 && gap_data[2] == 0x00 && gap_data[3] == 0x00 && gap_data[4] == 0x00 {
                for i in 5..std::cmp::min(gap_data.len().saturating_sub(21), 15) {
                    if i + 4 < gap_data.len() && 
                       gap_data[i] == 0x00 && gap_data[i+1] == 0x12 && gap_data[i+2] == 0x00 && 
                       gap_data[i+3] == 0x00 && gap_data[i+4] == 0x00 {
                        
                        let steamid_start = i + 5;
                        if steamid_start + 17 <= gap_data.len() {
                            let potential_steamid = &gap_data[steamid_start..steamid_start + 17];
                            
                            if potential_steamid.iter().all(|&b| b >= b'0' && b <= b'9') {
                                return String::from_utf8(potential_steamid.to_vec()).ok();
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn replace_steamid_pattern(&self, data: &mut Vec<u8>, steamid_info: SteamIdInfo) {
        let gap_pattern = [
            0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05, 
            0x00, 0x00, 0x00
        ];
        
        let gap_start = steamid_info.start_pos;
        let gap_end = gap_start + steamid_info.steamid_length;
        
        if gap_end <= data.len() {
            data.splice(gap_start..gap_end, gap_pattern.iter().cloned());
        }
    }
}