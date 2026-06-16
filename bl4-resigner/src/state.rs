use std::path::PathBuf;
use std::sync::mpsc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Resign,
    Decrypt,
    Encrypt,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IdType {
    Steam,
    Epic,
}

#[derive(Debug, Clone)]
pub enum Status {
    Idle,
    Processing,
    Completed(String),
    Error(String),
    EncryptionWarning(PathBuf, PathBuf, String),
    Warning(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tab {
    Main,
    Settings,
    Help,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub output_dir: String,
    pub last_input_dir: String,
    pub preferred_id_type: String,
    pub anonymize_guids: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            output_dir: String::new(),
            last_input_dir: String::new(),
            preferred_id_type: "Steam".to_string(),
            anonymize_guids: true,
        }
    }
}

pub struct AppState {
    pub mode: Mode,
    pub id_type: IdType,
    pub input_dir: String,
    pub output_dir: String,
    pub anonymize_guids: bool,
    pub steam_id: String,
    pub old_steam_id: String,
    pub new_steam_id: String,

    pub epic_id: String,
    pub old_epic_id: String,
    pub new_epic_id: String,
    
    pub status: Status,
    pub progress_rx: Option<mpsc::Receiver<String>>,
    pub active_tab: Tab,
    pub config_file: PathBuf,
    pub config: AppConfig,
}

impl AppState {
    pub fn new() -> Self {
        let config_file = PathBuf::from("bl4resigner.ini");
        let config = Self::load_config(&config_file);
        
        println!("Config loaded: anonymize_guids = {}", config.anonymize_guids);
        
        let is_first_run = !config_file.exists();
        println!("Is first run: {}", is_first_run);
        
        let id_type = match config.preferred_id_type.as_str() {
            "Epic" => IdType::Epic,
            _ => IdType::Steam,
        };

        let mut state = Self {
            mode: Mode::Resign,
            id_type,
            input_dir: config.last_input_dir.clone(),
            output_dir: config.output_dir.clone(),
            anonymize_guids: config.anonymize_guids,
            steam_id: String::new(),
            old_steam_id: String::new(),
            new_steam_id: String::new(),
            status: Status::Idle,
            progress_rx: None,
            active_tab: Tab::Main,
            config_file,
            config,
            epic_id: String::new(),
            old_epic_id: String::new(),
            new_epic_id: String::new(),
        };
        
        println!("State anonymize_guids: {}", state.anonymize_guids);
        
        if is_first_run {
            println!("Saving config on first run");
            state.save_config();
        }        
        state
    }

    fn load_config(config_path: &PathBuf) -> AppConfig {
        if let Ok(content) = std::fs::read_to_string(config_path) {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            AppConfig::default()
        }
    }

    pub fn save_config(&mut self) {
        self.config.anonymize_guids = self.anonymize_guids;
        self.config.output_dir = self.output_dir.clone();
        self.config.last_input_dir = self.input_dir.clone();
        self.config.preferred_id_type = match self.id_type {
            IdType::Steam => "Steam".to_string(),
            IdType::Epic => "Epic".to_string(),
        };

        if let Ok(json) = serde_json::to_string_pretty(&self.config) {
            let _ = std::fs::write(&self.config_file, json);
        }
    }

    pub fn get_suffix(&self) -> &'static str {
        match self.mode {
            Mode::Resign => "_resigned",
            Mode::Decrypt => "_decrypted", 
            Mode::Encrypt => "_encrypted",
        }
    }

    pub fn get_final_output_path(&self) -> PathBuf {
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

    pub fn get_current_id(&self) -> &str {
        match self.id_type {
            IdType::Steam => &self.steam_id,
            IdType::Epic => &self.epic_id,
        }
    }

    pub fn get_current_id_mut(&mut self) -> &mut String {
        match self.id_type {
            IdType::Steam => &mut self.steam_id,
            IdType::Epic => &mut self.epic_id,
        }
    }

    pub fn get_old_id(&self) -> &str {
        match self.id_type {
            IdType::Steam => &self.old_steam_id,
            IdType::Epic => &self.old_epic_id,
        }
    }

    pub fn get_old_id_mut(&mut self) -> &mut String {
        match self.id_type {
            IdType::Steam => &mut self.old_steam_id,
            IdType::Epic => &mut self.old_epic_id,
        }
    }

    pub fn get_new_id(&self) -> &str {
        match self.id_type {
            IdType::Steam => &self.new_steam_id,
            IdType::Epic => &self.new_epic_id,
        }
    }

    pub fn get_new_id_mut(&mut self) -> &mut String {
        match self.id_type {
            IdType::Steam => &mut self.new_steam_id,
            IdType::Epic => &mut self.new_epic_id,
        }
    }

    pub fn can_process(&self) -> bool {
        match self.mode {
            Mode::Decrypt | Mode::Encrypt => {
                !self.input_dir.is_empty() && !self.get_current_id().is_empty()
            }
            Mode::Resign => {
                !self.input_dir.is_empty() && 
                !self.get_old_id().is_empty() && 
                !self.get_new_id().is_empty()
            }
        }
    }

    pub fn is_processing(&self) -> bool {
        matches!(self.status, Status::Processing)
    }

    pub fn validate_steam_id(id: &str) -> Result<(), String> {
        if id.is_empty() {
            return Err("Steam ID cannot be empty".to_string());
        }
        
        if id.len() != 17 {
            return Err("Steam ID must be exactly 17 digits long".to_string());
        }
        
        if !id.chars().all(|c| c.is_ascii_digit()) {
            return Err("Steam ID must contain only numbers".to_string());
        }
        
        let num: u64 = id.parse().map_err(|_| "Invalid Steam ID format")?;
        
        if !id.starts_with("7656119") {
            return Err("Steam ID must start with 7656119 (Steam64 format)".to_string());
        }
        
        if num < 76561197960265728 {
            return Err("Steam ID appears to be invalid (too small for Steam64 format)".to_string());
        }
        
        if num > 76561999999999999 {
            return Err("Steam ID appears to be invalid (too large for Steam64 format)".to_string());
        }
        
        Ok(())
    }

    pub fn validate_epic_id(id: &str) -> Result<(), String> {
        let id = id.trim();

        if id.is_empty() {
            return Err("Epic ID cannot be empty".to_string());
        }
        
        if id.len() != 32 {
            return Err("Epic ID must be exactly 32 characters long".to_string());
        }
        
        if !id.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err("Epic ID must contain only hexadecimal characters (0-9, A-F)".to_string());
        }
        
        Ok(())
    }

    pub fn validate_current_ids(&self) -> Result<(), String> {
        match self.mode {
            Mode::Decrypt | Mode::Encrypt => {
                let current_id = self.get_current_id();
                match self.id_type {
                    IdType::Steam => Self::validate_steam_id(current_id)?,
                    IdType::Epic => Self::validate_epic_id(current_id)?,
                }
            },
            Mode::Resign => {
                let old_id = self.get_old_id();
                let new_id = self.get_new_id();
                
                match self.id_type {
                    IdType::Steam => {
                        Self::validate_steam_id(old_id)
                            .map_err(|e| format!("Invalid Old Steam ID: {}", e))?;
                        Self::validate_steam_id(new_id)
                            .map_err(|e| format!("Invalid New Steam ID: {}", e))?;
                    },

                    IdType::Epic => {
                        Self::validate_epic_id(old_id)
                            .map_err(|e| format!("Invalid Old Epic ID: {}", e))?;
                        Self::validate_epic_id(new_id)
                            .map_err(|e| format!("Invalid New Epic ID: {}", e))?;
                    },
                }
                
                if old_id == new_id {
                    return Err("Old and New IDs cannot be the same".to_string());
                }
            },
        }
        Ok(())
    }
}