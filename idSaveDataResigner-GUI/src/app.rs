#[cfg(target_os = "windows")]
use eframe::egui;
use std::path::PathBuf;
use std::fs;
use std::thread;
use std::sync::mpsc;
use chrono::{DateTime, Local};
use crate::logic::IdCrypto;

#[derive(Debug, Clone, PartialEq)]
enum Mode {
    Resign,
    Decrypt,
    Encrypt,
}

#[derive(Debug, Clone)]
enum Status {
    Idle,
    Processing,
    Completed(String),
    Error(String),
    EncryptionWarning(PathBuf, PathBuf, String, String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct AppConfig {
    output_dir: String,
}

#[derive(Debug, Clone)]
struct GameInfo {
    name: &'static str,
    code: &'static str,
    platforms: &'static [Platform],
}

#[derive(Debug, Clone, PartialEq)]
enum Platform {
    Steam,
    GOG,
}

const GAMES: &[GameInfo] = &[
    GameInfo { 
        name: "DOOM Eternal", 
        code: "MANCUBUS",
        platforms: &[Platform::Steam, Platform::GOG],
    },
    GameInfo { 
        name: "DOOM: The Dark Ages", 
        code: "MANCUBUS",
        platforms: &[Platform::Steam],
        // platforms: &[Platform::Steam, Platform::GOG],
    },
    GameInfo { 
        name: "Indiana Jones and the Great Circle", 
        code: "SUKHOTHAI",
        platforms: &[Platform::Steam, Platform::GOG],
    },
];

pub struct SaveDataApp {
    mode: Mode,
    game_idx: usize,
    input_dir: String,
    output_dir: String,
    steam_id: String,
    old_id: String,
    new_id: String,
    status: Status,
    progress_rx: Option<mpsc::Receiver<String>>,
    active_tab: Tab,
    config_file: PathBuf,
    platform: Platform,
}

#[derive(Debug, Clone, PartialEq)]
enum Tab {
    Main,
    Settings,
}

impl SaveDataApp {

    pub fn new() -> Self {
        let config_file = PathBuf::from("resigner_config");
        let config = Self::load_config(&config_file);

        Self {
            mode: Mode::Resign,
            game_idx: 0,
            input_dir: String::new(),
            output_dir: config.output_dir,
            steam_id: String::new(),
            old_id: String::new(),
            new_id: String::new(),
            status: Status::Idle,
            progress_rx: None,
            active_tab: Tab::Main,
            config_file: config_file,
            platform: Platform::Steam,
        }
    }

    fn load_config(config_path: &PathBuf) -> AppConfig {
        if let Ok(content) = fs::read_to_string(config_path) {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            AppConfig::default()
        }
    }

    fn save_config(&self) {
        let config = AppConfig {
            output_dir: self.output_dir.clone(),
        };

        if let Ok(json) = serde_json::to_string_pretty(&config) {
            let _ = fs::write(&self.config_file, json);
        }
    }

    fn get_suffix(&self) -> &'static str {
        match self.mode {
            Mode::Resign => "_resigned",
            Mode::Decrypt => "_decrypted",
            Mode::Encrypt => "_encrypted",
        }
    }

    fn get_game_code(&self) -> String {
        match (self.game_idx, &self.platform) {
            (0, Platform::GOG) => "ARACHNOTRON".to_string(),
            // (1, Platform::GOG) => "not_on_gog".to_string(), // for future
            (2, Platform::GOG) => "PAINELEMENTAL".to_string(),
            _ => GAMES[self.game_idx].code.to_string(),
        }
    }

    fn get_id_label(&self) -> &'static str {
        match &self.platform {
            Platform::GOG => "Galaxy ID",
            Platform::Steam => "SteamID",
        }
    }

    fn get_final_output_path(&self) -> PathBuf {
        let base = if self.output_dir.is_empty() {
            let input = PathBuf::from(&self.input_dir);
            input.parent().unwrap_or(&input).to_path_buf()
        } else {
            PathBuf::from(&self.output_dir)
        };
        
        let input = PathBuf::from(&self.input_dir);
        let name = input.file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("output"))
            .to_string_lossy();
        
        base.join(format!("{}{}", name, self.get_suffix()))
    }

    fn validate_steam_id(id: &str) -> Result<(), String> {
        if id.is_empty() {
            return Err("SteamID cannot be empty".to_string());
        }
        
        if id.len() != 17 {
            return Err("SteamID must be exactly 17 digits long".to_string());
        }
        
        if !id.chars().all(|c| c.is_ascii_digit()) {
            return Err("SteamID must contain only numbers".to_string());
        }
        
        let num: u64 = id.parse().map_err(|_| "Invalid SteamID format")?;
        
        if !id.starts_with("7656119") {
            return Err("SteamID must start with 7656119 (Steam64 format)".to_string());
        }
        
        if num < 76561197960265728 {
            return Err("SteamID appears to be invalid (too small for Steam64 format)".to_string());
        }
        
        if num > 76561999999999999 {
            return Err("SteamID appears to be invalid (too large for Steam64 format)".to_string());
        }
        
        Ok(())
    }

    fn is_file_encrypted(data: &[u8]) -> bool {
        if data.len() < 16 {
            return false;
        }
        
        let text_start = std::str::from_utf8(&data[..16.min(data.len())]).is_ok();
        if text_start {
            return false;
        }
        
        let mut byte_counts = [0u32; 256];
        for &byte in data.iter().take(1024) {
            byte_counts[byte as usize] += 1;
        }
        
        let len = data.len().min(1024) as f64;
        let entropy: f64 = byte_counts.iter()
            .filter(|&&count| count > 0)
            .map(|&count| {
                let p = count as f64 / len;
                -p * p.log2()
            })
            .sum();
        
        entropy > 6.0
    }

    fn process_files(&mut self) {
        match self.mode {
            Mode::Decrypt | Mode::Encrypt => {
                if self.platform == Platform::Steam {
                    if let Err(e) = Self::validate_steam_id(&self.steam_id) {
                        self.status = Status::Error(format!("Invalid SteamID: {}", e));
                        return;
                    }
                }
            },
            Mode::Resign => {
                if self.platform == Platform::Steam {
                    if let Err(e) = Self::validate_steam_id(&self.old_id) {
                        self.status = Status::Error(format!("Invalid Old SteamID: {}", e));
                        return;
                    }
                    if let Err(e) = Self::validate_steam_id(&self.new_id) {
                        self.status = Status::Error(format!("Invalid New SteamID: {}", e));
                        return;
                    }
                }
                if self.old_id == self.new_id {
                    self.status = Status::Error("Old and New IDs cannot be the same".to_string());
                    return;
                }
            },
        }

        let input = PathBuf::from(&self.input_dir);
        let output = self.get_final_output_path();
        let code = self.get_game_code();
        
        if self.mode == Mode::Encrypt {
            if let Ok(files) = Self::collect_files(&input) {
                if let Some(first_file) = files.first() {
                    if let Ok(data) = fs::read(first_file) {
                        if Self::is_file_encrypted(&data) {
                            self.status = Status::EncryptionWarning(
                                input.clone(),
                                output,
                                code.clone(),
                                self.steam_id.clone()
                            );
                            return;
                        }
                    }
                }
            }
        }

        self.start_processing();
    }

    fn start_processing(&mut self) {
        let input = PathBuf::from(&self.input_dir);
        let output = self.get_final_output_path();
        let code = self.get_game_code();
        let mode = self.mode.clone();
        
        let (tx, rx) = mpsc::channel();
        self.progress_rx = Some(rx);
        self.status = Status::Processing;

        match mode {
            Mode::Decrypt => {
                let id = self.steam_id.clone();
                thread::spawn(move || {
                    Self::process_decrypt(input, output, code, id, tx);
                });
            },
            Mode::Encrypt => {
                let id = self.steam_id.clone();
                thread::spawn(move || {
                    Self::process_encrypt(input, output, code, id, tx);
                });
            },
            Mode::Resign => {
                let old = self.old_id.clone();
                let new = self.new_id.clone();
                thread::spawn(move || {
                    Self::process_resign(input, output, code, old, new, tx);
                });
            },
        }
    }

    fn process_decrypt(input: PathBuf, output: PathBuf, code: String, id: String, tx: mpsc::Sender<String>) {
        let result = (|| -> Result<String, Box<dyn std::error::Error>> {
            if !input.exists() {
                return Err("Input path does not exist".into());
            }

            if input.is_file() {
                return Err("Input path must be a directory, not a file".into());
            }

            let files = Self::collect_files(&input)?;
            fs::create_dir_all(&output).map_err(|e| format!("Failed to create output directory: {}", e))?;
            
            if files.is_empty() {
                return Err("No files found in input directory".into());
            }

            let mut processed = 0;
            let mut log = String::new();

            for file in files {
                let name = file.file_name()
                    .ok_or("Invalid file name")?
                    .to_str()
                    .ok_or("Invalid file name encoding")?;
                
                log.push_str(&format!("Decrypting {}...\n", name));
                
                let data = fs::read(&file).map_err(|e| format!("Failed to read file {}: {}", name, e))?;
                let decrypted = IdCrypto::decrypt_file(&data, name, &code, &id)
                    .map_err(|_| format!("Failed to decrypt {}: Check if SteamID is correct", name))?;
                
                let rel = file.strip_prefix(&input)?;
                let out = output.join(rel);
                if let Some(parent) = out.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&out, decrypted)
                    .map_err(|e| format!("Failed to write decrypted file {}: {}", name, e))?;
                
                processed += 1;
            }

            let ts: DateTime<Local> = Local::now();
            let mut info = format!("Processing completed at: {}\n\n", ts.format("%Y-%m-%d %H:%M:%S"));
            info.push_str(&format!("Decrypted {} files from SteamID {}\n\n", processed, id));
            info.push_str(&log);
            
            fs::write(output.join("INFO.txt"), info)?;
            Ok(format!("Successfully decrypted {} files", processed))
        })();

        match result {
            Ok(msg) => tx.send(format!("COMPLETED: {}", msg)).ok(),
            Err(e) => tx.send(format!("ERROR: {}", e)).ok(),
        };
    }

    fn process_encrypt(input: PathBuf, output: PathBuf, code: String, id: String, tx: mpsc::Sender<String>) {
        let result = (|| -> Result<String, Box<dyn std::error::Error>> {
            if !input.exists() {
                return Err("Input path does not exist".into());
            }

            if input.is_file() {
                return Err("Input path must be a directory, not a file".into());
            }

            let files = Self::collect_files(&input)?;
            fs::create_dir_all(&output).map_err(|e| format!("Failed to create output directory: {}", e))?;
            
            if files.is_empty() {
                return Err("No files found in input directory".into());
            }

            let mut processed = 0;
            let mut log = String::new();

            for file in files {
                let name = file.file_name()
                    .ok_or("Invalid file name")?
                    .to_str()
                    .ok_or("Invalid file name encoding")?;
                
                log.push_str(&format!("Encrypting {}...\n", name));
                
                let data = fs::read(&file).map_err(|e| format!("Failed to read file {}: {}", name, e))?;
                let encrypted = IdCrypto::encrypt_file(&data, name, &code, &id)
                    .map_err(|_| format!("Failed to encrypt {}: Check if SteamID is correct", name))?;
                
                let rel = file.strip_prefix(&input)?;
                let out = output.join(rel);
                if let Some(parent) = out.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&out, encrypted)
                    .map_err(|e| format!("Failed to write encrypted file {}: {}", name, e))?;
                
                processed += 1;
            }

            let ts: DateTime<Local> = Local::now();
            let mut info = format!("Processing completed at: {}\n\n", ts.format("%Y-%m-%d %H:%M:%S"));
            info.push_str(&format!("Encrypted {} files for SteamID {}\n\n", processed, id));
            info.push_str(&log);
            
            fs::write(output.join("INFO.txt"), info)?;
            Ok(format!("Successfully encrypted {} files", processed))
        })();

        match result {
            Ok(msg) => tx.send(format!("COMPLETED: {}", msg)).ok(),
            Err(e) => tx.send(format!("ERROR: {}", e)).ok(),
        };
    }

    fn process_resign(input: PathBuf, output: PathBuf, code: String, old: String, new: String, tx: mpsc::Sender<String>) {
        let result = (|| -> Result<String, Box<dyn std::error::Error>> {
            if !input.exists() {
                return Err("Input path does not exist".into());
            }

            if input.is_file() {
                return Err("Input path must be a directory, not a file".into());
            }

            let files = Self::collect_files(&input)?;
            fs::create_dir_all(&output).map_err(|e| format!("Failed to create output directory: {}", e))?;
            
            if files.is_empty() {
                return Err("No files found in input directory".into());
            }

            let mut processed = 0;
            let mut log = String::new();

            for file in files {
                let name = file.file_name()
                    .ok_or("Invalid file name")?
                    .to_str()
                    .ok_or("Invalid file name encoding")?;
                
                log.push_str(&format!("Resigning {}...\n", name));
                
                let data = fs::read(&file).map_err(|e| format!("Failed to read file {}: {}", name, e))?;
                let resigned = IdCrypto::resign_file(&data, name, &code, &old, &new)
                    .map_err(|_| format!("Failed to resign {}: Check if Old SteamID is correct", name))?;
                
                let rel = file.strip_prefix(&input)?;
                let out = output.join(rel);
                if let Some(parent) = out.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&out, resigned)
                    .map_err(|e| format!("Failed to write resigned file {}: {}", name, e))?;
                
                processed += 1;
            }

            let ts: DateTime<Local> = Local::now();
            let mut info = format!("Processing completed at: {}\n\n", ts.format("%Y-%m-%d %H:%M:%S"));
            info.push_str(&format!("Resigned {} files from SteamID {} to SteamID {}\n\n", processed, old, new));
            info.push_str(&log);
            
            fs::write(output.join("INFO.txt"), info)?;
            Ok(format!("Successfully resigned {} files", processed))
        })();

        match result {
            Ok(msg) => tx.send(format!("COMPLETED: {}", msg)).ok(),
            Err(e) => tx.send(format!("ERROR: {}", e)).ok(),
        };
    }

    fn is_save(path: &PathBuf) -> bool {
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            let lower = name.to_lowercase();
            return lower.ends_with(".bin") || 
                lower.ends_with(".dat") || 
                lower.ends_with(".details") ||
                lower.ends_with(".details-backup") ||
                lower.ends_with(".dat-backup");
        }
        false
    }

    fn collect_files(path: &PathBuf) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
        let mut files = Vec::new();
        let mut bad = Vec::new();
        
        if path.is_file() {
            if Self::is_save(path) {
                files.push(path.clone());
            } else {
                bad.push(path.clone());
            }
        } else if path.is_dir() {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let p = entry.path();
                if p.is_file() {
                    if Self::is_save(&p) {
                        files.push(p);
                    } else {
                        bad.push(p);
                    }
                } else if p.is_dir() {
                    let (sub, sub_bad) = Self::walk_dir(&p)?;
                    files.extend(sub);
                    bad.extend(sub_bad);
                }
            }
        }
        
        if !bad.is_empty() {
            let _names: Vec<String> = bad
                .iter()
                .filter_map(|p| p.file_name().and_then(|n| n.to_str().map(|s| s.to_string())))
                .collect();
        }
        
        if files.is_empty() {
            return Err("No supported save files (.bin / .dat / .details / .details-backup / .dat-backup) found in the directory".into());
        }
        
        Ok(files)
    }

    fn walk_dir(path: &PathBuf) -> Result<(Vec<PathBuf>, Vec<PathBuf>), Box<dyn std::error::Error>> {
        let mut files = Vec::new();
        let mut bad = Vec::new();
        
        if path.is_file() {
            if Self::is_save(path) {
                files.push(path.clone());
            } else {
                bad.push(path.clone());
            }
        } else if path.is_dir() {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let p = entry.path();
                if p.is_file() {
                    if Self::is_save(&p) {
                        files.push(p);
                    } else {
                        bad.push(p);
                    }
                } else if p.is_dir() {
                    let (sub, sub_bad) = Self::walk_dir(&p)?;
                    files.extend(sub);
                    bad.extend(sub_bad);
                }
            }
        }
        
        Ok((files, bad))
    }

    fn browse_folder(&mut self, for_output: bool) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            let s = path.to_string_lossy().to_string();
            if for_output {
                self.output_dir = s;
            } else {
                self.input_dir = s;
            }
        }
    }

    fn path_input_row(ui: &mut egui::Ui, label: &str, path: &mut String) -> bool {
        let mut clicked = false;
        ui.horizontal(|ui| {
            ui.label(label);
            ui.text_edit_singleline(path);
            if ui.button("Browse").clicked() {
                clicked = true;
            }
        });
        
        ui.ctx().input(|i| {
            if !i.raw.dropped_files.is_empty() {
                if let Some(f) = i.raw.dropped_files.first() {
                    if let Some(p) = &f.path {
                        *path = p.to_string_lossy().to_string();
                    }
                }
            }
        });
        
        clicked
    }

    fn main_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Mode:");
            ui.radio_value(&mut self.mode, Mode::Resign, "Resign");
            ui.radio_value(&mut self.mode, Mode::Decrypt, "Decrypt");
            ui.radio_value(&mut self.mode, Mode::Encrypt, "Encrypt");
        });

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Game:");
            egui::ComboBox::from_label("")
                .selected_text(GAMES[self.game_idx].name)
                .show_ui(ui, |ui| {
                    for (i, game) in GAMES.iter().enumerate() {
                        ui.selectable_value(&mut self.game_idx, i, game.name);
                    }
                });
        });

        if GAMES[self.game_idx].platforms.len() > 1 {
            ui.horizontal(|ui| {
                ui.label("Platform:");
                ui.radio_value(&mut self.platform, Platform::Steam, "Steam");
                ui.radio_value(&mut self.platform, Platform::GOG, "GOG");
            });
        }

        ui.separator();
        if Self::path_input_row(ui, "Input Folder:", &mut self.input_dir) {
            self.browse_folder(false);
        }
        ui.add_space(5.0);
        if !self.input_dir.is_empty() {
            let out = self.get_final_output_path();
            let display = format!("→ Files will be saved to: {}", out.display());
            ui.label(egui::RichText::new(display).size(10.0).color(egui::Color32::from_rgb(100, 150, 255)));
        }

        ui.separator();

        match self.mode {
            Mode::Decrypt | Mode::Encrypt => {
                ui.horizontal(|ui| {
                    ui.label(format!("{}:", self.get_id_label()));
                    ui.text_edit_singleline(&mut self.steam_id);
                });
            }
            Mode::Resign => {
                ui.horizontal(|ui| {
                    ui.label(format!("Old {}:", self.get_id_label()));
                    ui.text_edit_singleline(&mut self.old_id);
                });
                ui.horizontal(|ui| {
                    ui.label(format!("New {}:", self.get_id_label()));
                    ui.text_edit_singleline(&mut self.new_id);
                });
            }
        }

        ui.separator();

        let can_process = match self.mode {
            Mode::Decrypt | Mode::Encrypt => {
                !self.input_dir.is_empty() && !self.steam_id.is_empty()
            }
            Mode::Resign => {
                !self.input_dir.is_empty() && 
                !self.old_id.is_empty() && !self.new_id.is_empty()
            }
        };
        let processing = matches!(self.status, Status::Processing);
        
        ui.add_enabled_ui(can_process && !processing, |ui| {
            let btn_text = match self.mode {
                Mode::Decrypt => "🔓 Decrypt Files",
                Mode::Encrypt => "🔒 Encrypt Files",
                Mode::Resign => "✍ Resign Files",
            };
            
            if ui.button(btn_text).clicked() {
                self.process_files();
            }
        });

        let mut new_status = None;
        let mut start_encrypt = None;
        
        match &self.status {
            Status::Idle => {},
            Status::Processing => {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Processing...");
                });
            }
            Status::Completed(msg) => {
                ui.separator();
                ui.colored_label(egui::Color32::GREEN, format!("✅ {}", msg));
            }
            Status::Error(msg) => {
                ui.separator();
                ui.colored_label(egui::Color32::RED, format!("❌ {}", msg));
            }
            Status::EncryptionWarning(input, output, code, id) => {
                ui.separator();
                ui.colored_label(egui::Color32::YELLOW, "⚠️ Warning: Files appear to be already encrypted!");
                ui.label("Are you sure you want to encrypt already encrypted files?");
                ui.horizontal(|ui| {
                    if ui.button("Yes, Continue").clicked() {
                        start_encrypt = Some((input.clone(), output.clone(), code.clone(), id.clone()));
                        new_status = Some(Status::Processing);
                    }
                    if ui.button("Cancel").clicked() {
                        new_status = Some(Status::Idle);
                    }
                });
            }
        }
        
        if let Some(status) = new_status {
            self.status = status;
        }
        
        if let Some((input, output, code, id)) = start_encrypt {
            let (tx, rx) = mpsc::channel();
            self.progress_rx = Some(rx);
            
            thread::spawn(move || {
                Self::process_encrypt(input, output, code, id, tx);
            });
        }

        ui.separator();
        ui.collapsing("Help", |ui| {
            ui.label("• Resign: Transfer save files from a SteamID to another");
            ui.label("• Decrypt: Convert encrypted save files to readable format");
            ui.label("• Encrypt: Convert readable files back to encrypted format");
            ui.horizontal(|ui| {
                ui.label("• SteamID: Your 64-bit Steam ID");
                ui.hyperlink_to("Click me to check", "https://steamdb.info/calculator/");
                ui.label(")");
            });
            ui.label("• Always backup your save files before processing!");
            ui.label("• WEEEEEEEEEEEEE");
        });
    }

    fn settings_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("Output Settings");
        ui.separator();

        ui.label("Configure where processed files will be saved:");
        ui.add_space(10.0);

        if Self::path_input_row(ui, "Output Folder:", &mut self.output_dir) {
            self.browse_folder(true);
            self.save_config();
        }

        ui.add_space(10.0);

        ui.label(egui::RichText::new("Note:").strong());
        ui.label("• If no output folder is specified, files will be saved next to the input folder");
        ui.label("• Example: GAME-AUTOSAVE1 → GAME-AUTOSAVE1_resigned");
        ui.add_space(20.0);
        ui.separator();

        ui.horizontal(|ui| {
            if ui.button("Clear Output Folder").clicked() {
                self.output_dir.clear();
                self.save_config();
            }
            ui.label("(Will use input folder's parent directory)");
        });
    }
}

impl eframe::App for SaveDataApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(rx) = &self.progress_rx {
            if let Ok(msg) = rx.try_recv() {
                if msg.starts_with("COMPLETED:") {
                    self.status = Status::Completed(msg[10..].to_string());
                    self.progress_rx = None;
                } else if msg.starts_with("ERROR:") {
                    self.status = Status::Error(msg[6..].to_string());
                    self.progress_rx = None;
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, Tab::Main, "Main");
                ui.selectable_value(&mut self.active_tab, Tab::Settings, "Settings");
            });
            
            ui.separator();

            match self.active_tab {
                Tab::Main => self.main_tab(ui),
                Tab::Settings => self.settings_tab(ui),
            }
        });

        let processing = matches!(self.status, Status::Processing);
        if processing {
            ctx.request_repaint();
        }
    }
}
