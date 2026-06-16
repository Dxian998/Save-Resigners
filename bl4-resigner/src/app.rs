use eframe::egui;
use crate::state::{AppState, Tab, Status};
use crate::ui::UiComponents;
use crate::processor::FileProcessor;

pub struct Bl4SaveDataApp {
    state: AppState,
}

impl Bl4SaveDataApp {
    pub fn new() -> Self {
        Self {
            state: AppState::new(),
        }
    }

    fn handle_file_drop(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                if let Some(f) = i.raw.dropped_files.first() {
                    if let Some(p) = &f.path {
                        if p.is_dir() {
                            self.state.input_dir = p.to_string_lossy().to_string();
                        } else if let Some(parent) = p.parent() {
                            self.state.input_dir = parent.to_string_lossy().to_string();
                        }
                    }
                }
            }
        });
    }

    fn render_tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.state.active_tab, Tab::Main, "Main");
            ui.selectable_value(&mut self.state.active_tab, Tab::Settings, "Settings");
            ui.selectable_value(&mut self.state.active_tab, Tab::Help, "Help");
        });
        ui.separator();
    }

    fn render_active_tab(&mut self, ui: &mut egui::Ui) {
        match self.state.active_tab {
            Tab::Main => {
                let mut should_start_processing = false;
                let mut should_force_encrypt = false;
                
                UiComponents::render_main_tab(&mut self.state, ui, &mut should_start_processing, &mut should_force_encrypt);
                
                if should_force_encrypt {
                    FileProcessor::force_encrypt(&mut self.state);
                } else if should_start_processing && !self.state.is_processing() {
                    FileProcessor::start_processing(&mut self.state);
                }
            },
            Tab::Settings => UiComponents::render_settings_tab(&mut self.state, ui),
            Tab::Help => UiComponents::render_help_tab(&mut self.state, ui),
        }
    }

    #[allow(dead_code)]
    fn should_start_processing(&self) -> bool {
        false
    }
}

impl Default for Bl4SaveDataApp {
    fn default() -> Self {
        Self::new()
    }
}

impl eframe::App for Bl4SaveDataApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_file_drop(ui.ctx());
        FileProcessor::update_progress(&mut self.state);

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.add_space(10.0);
            ui.heading("🎮 Borderlands 4 Save Data Resigner");
            ui.label("Decrypt, encrypt, and resign Borderlands 4 save files");
            ui.separator();
            self.render_tabs(ui);
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    self.render_active_tab(ui);
                });
        });

        if self.state.is_processing() {
            ui.ctx().request_repaint();
        }
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.state.save_config();
    }
}

#[allow(dead_code)]
impl Bl4SaveDataApp {
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        ctx.input_mut(|i| {
            if i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::O)) {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.state.input_dir = path.to_string_lossy().to_string();
                }
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                match &self.state.status {
                    Status::Error(_) | Status::EncryptionWarning(_, _, _) => {
                        self.state.status = Status::Idle;
                    },
                    _ => {},
                }
            }
        });
    }
}