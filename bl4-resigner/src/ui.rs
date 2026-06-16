use eframe::egui;
use crate::state::{AppState, Mode, IdType, Status};
use open;
pub struct UiComponents;

impl UiComponents {
    pub fn render_main_tab(state: &mut AppState, ui: &mut egui::Ui, should_start_processing: &mut bool, should_force_encrypt: &mut bool) {
        ui.horizontal(|ui| {
            ui.label("Mode:");
            ui.radio_value(&mut state.mode, Mode::Resign, "Resign");
            ui.radio_value(&mut state.mode, Mode::Decrypt, "Decrypt");
            ui.radio_value(&mut state.mode, Mode::Encrypt, "Encrypt");
        });

        ui.separator();

        ui.horizontal(|ui| {
            ui.label("ID Type:");
            ui.radio_value(&mut state.id_type, IdType::Steam, "Steam ID");
            ui.radio_value(&mut state.id_type, IdType::Epic, "Epic ID")
        });

        ui.separator();

        if Self::path_input_row(ui, "Input Folder:", &mut state.input_dir) {
            Self::browse_folder(state, false);
        }
        ui.add_space(5.0);
        if !state.input_dir.is_empty() {
            let out = state.get_final_output_path();
            let display = format!("→ Files will be saved to: {}", out.display());
            ui.label(egui::RichText::new(display).size(10.0).color(egui::Color32::from_rgb(100, 150, 255)));
        }

        ui.separator();

        match state.mode {
            Mode::Decrypt | Mode::Encrypt => {
                ui.horizontal(|ui| {
                    let label = match state.id_type {
                        IdType::Steam => "Steam ID (64-bit):",
                        IdType::Epic => "Epic ID (32 hex):",
                    };
                    ui.label(label);
                    ui.text_edit_singleline(state.get_current_id_mut());

                    if ui.button("🔗").clicked() {
                        Self::open_id_help_url(&state.id_type);
                    }
                });
                let example = match state.id_type {
                    IdType::Steam => "Example: 76561197960265729",
                    IdType::Epic => "Example: a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6",
                };
                ui.label(egui::RichText::new(example).size(10.0).color(egui::Color32::GRAY));
            }
            Mode::Resign => {
                ui.horizontal(|ui| {
                    let label = match state.id_type {
                        IdType::Steam => "Old Steam ID:",
                        IdType::Epic => "Old Epic ID:",
                    };
                    ui.label(label);
                    ui.text_edit_singleline(state.get_old_id_mut());
                });
                ui.horizontal(|ui| {
                    let label = match state.id_type {
                        IdType::Steam => "New Steam ID:",
                        IdType::Epic => "New Epic ID:",
                    };
                    ui.label(label);
                    ui.text_edit_singleline(state.get_new_id_mut());

                    if ui.button("🔗").clicked() {
                        Self::open_id_help_url(&state.id_type);
                    }
                });
            }
        }
        
        // Render status BEFORE the action button (had to spoonfeed myself)
        *should_force_encrypt = Self::render_status(state, ui);
        
        ui.separator();

        let can_process = state.can_process();
        let processing = state.is_processing();

        ui.add_enabled_ui(can_process && !processing, |ui| {
            let btn_text = match state.mode {
                Mode::Decrypt => "🔓 Decrypt Files",
                Mode::Encrypt => "🔒 Encrypt Files",
                Mode::Resign => "🔄 Resign Files",
            };

            if ui.button(btn_text).clicked() {
                if let Err(e) = state.validate_current_ids() {
                    state.status = Status::Error(e);
                } else {
                    *should_start_processing = true;
                }
            }
        });

        ui.separator();

        ui.collapsing("Quick Help", |ui| {
            ui.label("• Resign: Transfer save files from one ID to another");
            ui.label("• Decrypt: Convert encrypted save files to readable YAML format");
            ui.label("• Encrypt: Convert YAML files back to encrypted format");

            ui.horizontal(|ui| {
                ui.label("• Steam ID: Your 64-bit Steam ID (");
                ui.hyperlink_to("Check here", "https://steamdb.info/calculator/");
                ui.label(")");
            });
            
            ui.label("• Epic ID: Your 32-character Epic account ID (hexadecimal)");
            ui.label("• Always backup your save files before processing!");
            ui.label("• Borderlands 4 save files are usually .sav files");
            ui.label("• Processed files will include anonymized GUIDs for privacy");
        });
    }

    pub fn render_settings_tab(state: &mut AppState, ui: &mut egui::Ui) {
        ui.heading("Output Settings");
        ui.add_space(5.0);
        ui.label("Configure where processed files will be saved:");
        ui.add_space(10.0);

        if Self::path_input_row(ui, "Output Folder:", &mut state.output_dir) {
            Self::browse_folder(state, true);
            state.save_config();
        }

        ui.add_space(10.0);

        ui.label(egui::RichText::new("Note:").strong());
        ui.label("• If no output folder is specified, files will be saved next to the input folder");
        ui.label("• Example: SaveData -> SaveData_resigned");
        ui.add_space(5.0);
        ui.horizontal(|ui| {
            if ui.button("Clear Output Folder").clicked() {
                state.output_dir.clear();
                state.save_config();
            }
            ui.label("(Will use input folder's parent directory)");
        });

        ui.separator();
        
        ui.heading("Preferences");
        ui.label("Default ID Type:");
        ui.horizontal(|ui| {
            let mut changed = false;
            changed |= ui.radio_value(&mut state.id_type, IdType::Steam, "Steam ID").changed();
            changed |= ui.radio_value(&mut state.id_type, IdType::Epic, "Epic ID").changed();
            if changed {
                state.save_config();
            }
        });

        ui.separator();

        ui.label("Privacy Settings:");
        ui.horizontal(|ui| {
            if ui.checkbox(&mut state.anonymize_guids, "Anonymize GUIDs and online preferences")
                .on_hover_text("When enabled, character/save GUIDs and online preferences will be anonymized during encryption and resigning")
                .changed() 
            {
                state.save_config();
            }
        });
        ui.label(egui::RichText::new("Recommended: Keep this enabled for privacy").size(10.0).color(egui::Color32::GRAY));
    }

    pub fn render_help_tab(_state: &mut AppState, ui: &mut egui::Ui) {
        ui.heading("Borderlands 4 Save Data Resigner");
        ui.separator();

        ui.label("This tool allows you to:");
        ui.label("• Decrypt Borderlands 4 save files to readable YAML format");
        ui.label("• Encrypt YAML files back to the game's save format");
        ui.label("• Resign save files to work with different Steam accounts");

        ui.add_space(15.0);
        ui.heading("Supported Platforms");
        ui.separator();
        ui.label("• Steam");
        ui.label("• Epic Games Store");

        ui.add_space(15.0);
        ui.heading("File Types");
        ui.separator();
        ui.label("• .sav files (profile/gameplay saves)");
        ui.label("• Other Borderlands 4 save data files");

        ui.add_space(15.0);
        ui.heading("Security & Privacy");
        ui.separator();
        ui.label("• All save data is processed locally on your computer");
        ui.label("• Character and save GUIDs are automatically anonymized");
        ui.label("• Online character preferences are cleared for privacy");

        ui.add_space(15.0);
        ui.heading("Important Notes");
        ui.separator();
        ui.colored_label(egui::Color32::from_rgb(255, 150, 50), "⚠️ Always backup your save files before processing!");
        ui.label("• Disable Steam Cloud saves before replacing files");
        ui.label("• Incorrectly modified saves may cause issues or corruption");
        ui.label("• Use at your own risk - we're not responsible for lost progress");

        ui.add_space(15.0);
        ui.heading("Getting Your ID");
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Steam ID:");
            ui.hyperlink_to("SteamDB Calculator", "https://steamdb.info/calculator/");
        });
        ui.horizontal(|ui| {
            ui.label("Epic ID:");
            ui.hyperlink_to("Epic Account Page", "https://www.epicgames.com/account/personal");
        });
        ui.label("Note: Epic ID is found in your account URL or profile settings");
    }

    fn render_status(state: &mut AppState, ui: &mut egui::Ui) -> bool {
        let mut force_encrypt = false;
        let mut clear_status = false;
        
        match &state.status {
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
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::RED, format!("❌ {}", msg));
                    if ui.button("✖").clicked() {
                        clear_status = true;
                    }
                });
            }
            Status::EncryptionWarning(_, _, _) => {
                ui.separator();
                ui.colored_label(egui::Color32::YELLOW, "⚠️ Warning: Files appear to be already encrypted!");
                ui.label("Are you sure you want to encrypt already encrypted files?");
                ui.label("This may result in corrupted save data.");
                ui.horizontal(|ui| {
                    if ui.button("Yes, Continue Anyway").clicked() {
                        force_encrypt = true;
                    }
                    if ui.button("Cancel").clicked() {
                        clear_status = true;
                    }
                });
            }
            Status::Warning(msg) => {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::from_rgb(255, 165, 0), format!("⚠️ {}", msg));
                    if ui.button("✖").clicked() {
                        clear_status = true;
                    }
                });
            }
        }
        
        if clear_status {
            state.status = Status::Idle;
        }
        
        force_encrypt
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


    fn browse_folder(state: &mut AppState, for_output: bool) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            let s = path.to_string_lossy().to_string();
            if for_output {
                state.output_dir = s;
            } else {
                state.input_dir = s;
            }
        }
    }


    fn open_id_help_url(id_type: &IdType) {
        let url = match id_type {
            IdType::Steam => "https://steamdb.info/calculator/",
            IdType::Epic => "https://www.epicgames.com/account/personal",
        };
        if let Err(e) = open::that(url) {
            eprintln!("Failed to open browser: {}", e);
        }
    }
}