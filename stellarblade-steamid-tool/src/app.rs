use crate::{replacer::ReplacerTab, remover::RemoverTab, utils};
use eframe::egui;

#[derive(Default)]
pub struct SteamIDApp {
    icon_texture: Option<egui::TextureHandle>,
    current_tab: AppTab,
    show_about: bool,
    show_help: bool,
    drag_hover: bool,
    pub replacer: ReplacerTab,
    pub remover: RemoverTab,
}

#[derive(Default, PartialEq)]
enum AppTab {
    #[default]
    Replacer,
    Remover,
}

impl eframe::App for SteamIDApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_keyboard_input(ctx);
        self.load_icon_if_needed(ctx);
        self.handle_drag_and_drop(ctx);
        self.show_menu_bar(ctx);
        self.show_main_content(ctx);
        self.show_dialogs(ctx);
    }
}

impl SteamIDApp {
    fn handle_keyboard_input(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Escape) {
                if self.show_about {
                    self.show_about = false;
                } else if self.show_help {
                    self.show_help = false;
                }
            }
        });
    }

    fn load_icon_if_needed(&mut self, ctx: &egui::Context) {
        if self.icon_texture.is_none() {
            let icon_data = utils::load_icon();
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [icon_data.width as usize, icon_data.height as usize],
                &icon_data.rgba,
            );
            self.icon_texture = Some(ctx.load_texture("icon", image, Default::default()));
        }
    }

    fn handle_drag_and_drop(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            if !i.raw.hovered_files.is_empty() {
                self.drag_hover = true;
            } else {
                self.drag_hover = false;
            }
            if !i.raw.dropped_files.is_empty() {
                if let Some(file) = i.raw.dropped_files.first() {
                    if let Some(path) = &file.path {
                        let file_path = path.to_string_lossy().to_string();
                        match self.current_tab {
                            AppTab::Replacer => {
                                self.replacer.file_path = file_path;
                                self.replacer.status = "File loaded via drag & drop".to_string();
                                self.replacer.backup_filename.clear();
                                self.replacer.read_current_steamid();
                            }
                            AppTab::Remover => {
                                self.remover.file_path = file_path;
                                self.remover.status = "File loaded via drag & drop".to_string();
                                self.remover.backup_filename.clear();
                                self.remover.read_current_steamid();
                            }
                        }
                    }
                }
            }
        });
    }

    fn show_menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("🗁 Open File").clicked() {
                        if let Some((file_path, status)) = utils::browse_file() {
                            match self.current_tab {
                                AppTab::Replacer => {
                                    self.replacer.file_path = file_path;
                                    self.replacer.status = status;
                                    self.replacer.backup_filename.clear();
                                    self.replacer.read_current_steamid();
                                }
                                AppTab::Remover => {
                                    self.remover.file_path = file_path;
                                    self.remover.status = status;
                                    self.remover.backup_filename.clear();
                                    self.remover.read_current_steamid();
                                }
                            }
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("❌ Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("❓ Help").clicked() {
                        self.show_help = true;
                        ui.close_menu();
                    }
                    if ui.button("(i) About").clicked() {
                        self.show_about = true;
                        ui.close_menu();
                    }
                });
            });
        });
    }

    fn show_main_content(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.show_header(ui);
            self.show_tab_selector(ui);
            
            match self.current_tab {
                AppTab::Replacer => self.show_replacer_tab(ui),
                AppTab::Remover => self.show_remover_tab(ui),
            }
        });
    }

    fn show_header(&self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(15.0);
            ui.horizontal(|ui| {
                ui.add_space(ui.available_width() / 2.0 - 140.0);
                if let Some(tex) = &self.icon_texture {
                    ui.image((tex.id(), egui::vec2(24.0, 24.0)));
                    ui.add_space(8.0);
                }
                ui.heading("Stellar Blade SteamID Tool");
            });
            ui.add_space(8.0);
            ui.label("Replace or remove SteamID in Stellar Blade save files");
            ui.add_space(15.0);
        });
    }

    fn show_tab_selector(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.current_tab, AppTab::Replacer, "🔄 Replacer");
            ui.selectable_value(&mut self.current_tab, AppTab::Remover, "❌ Remover");
        });
        ui.add_space(10.0);
    }

    fn show_replacer_tab(&mut self, ui: &mut egui::Ui) {
        self.show_file_selection(ui, true);
        ui.add_space(8.0);
        self.show_new_steamid_input(ui);
        ui.add_space(8.0);
        self.show_demo_transfer_option(ui, true);
        ui.add_space(12.0);
        self.show_replace_button(ui);
        ui.add_space(12.0);
        self.show_status_section(ui, &self.replacer.status, &self.replacer.backup_filename);
    }

    fn show_remover_tab(&mut self, ui: &mut egui::Ui) {
        self.show_file_selection(ui, false);
        ui.add_space(8.0);
        self.show_remover_info(ui);
        ui.add_space(8.0);
        self.show_demo_transfer_option(ui, false);
        ui.add_space(12.0);
        self.show_remove_button(ui);
        ui.add_space(12.0);
        self.show_status_section(ui, &self.remover.status, &self.remover.backup_filename);
    }

    fn show_file_selection(&mut self, ui: &mut egui::Ui, is_replacer: bool) {
        ui.group(|ui| {
            ui.vertical(|ui| {
                ui.label("📁 File Selection");
                ui.horizontal(|ui| {
                    ui.label("File Path:");
                    let text_width = ui.available_width() - 80.0;
                    
                    let file_path = if is_replacer {
                        &mut self.replacer.file_path
                    } else {
                        &mut self.remover.file_path
                    };
                    
                    let response = ui.add_sized(
                        [text_width, 20.0],
                        egui::TextEdit::singleline(file_path)
                            .hint_text("Drag & drop a file or click Browse..."),
                    );
                    
                    if response.changed() {
                        if is_replacer {
                            self.replacer.status.clear();
                            self.replacer.backup_filename.clear();
                            self.replacer.read_current_steamid();
                        } else {
                            self.remover.status.clear();
                            self.remover.backup_filename.clear();
                            self.remover.read_current_steamid();
                        }
                    }
                    
                    if ui.button("📂 Browse").clicked() {
                        if let Some((file_path_new, status)) = utils::browse_file() {
                            if is_replacer {
                                self.replacer.file_path = file_path_new;
                                self.replacer.status = status;
                                self.replacer.backup_filename.clear();
                                self.replacer.read_current_steamid();
                            } else {
                                self.remover.file_path = file_path_new;
                                self.remover.status = status;
                                self.remover.backup_filename.clear();
                                self.remover.read_current_steamid();
                            }
                        }
                    }
                });
                
                if self.drag_hover {
                    ui.colored_label(egui::Color32::LIGHT_GRAY, "📤 Drop your file here");
                }
                
                ui.horizontal(|ui| {
                    ui.label("Current SteamID:");
                    let current_steamid = if is_replacer {
                        &self.replacer.current_steamid
                    } else {
                        &self.remover.current_steamid
                    };
                    
                    if current_steamid.is_empty() {
                        let label = if is_replacer {
                            "No valid file selected"
                        } else {
                            "No SteamID found or universal save"
                        };
                        ui.colored_label(egui::Color32::LIGHT_GRAY, label);
                    } else {
                        ui.colored_label(egui::Color32::LIGHT_BLUE, current_steamid);
                    }
                });
            });
        });
    }

    fn show_new_steamid_input(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.vertical(|ui| {
                ui.label("🔢 New SteamID");
                ui.horizontal(|ui| {
                    ui.label("SteamID:");
                    let text_width = ui.available_width();
                    let response = ui.add_sized(
                        [text_width, 20.0],
                        egui::TextEdit::singleline(&mut self.replacer.new_steamid)
                            .hint_text("Enter 17-digit SteamID (e.g., 76561198123456789)"),
                    );
                    if response.changed() {
                        self.replacer.status.clear();
                        self.replacer.backup_filename.clear();
                    }
                });
                ui.label("💡 SteamID must be 17 digits starting with '7656'");
            });
        });
    }

    fn show_remover_info(&self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.vertical(|ui| {
                ui.label("(i) Universal Save Creation");
                ui.label("This will remove SteamID from the save file, making it work for any Steam account.");
                ui.add_space(5.0);
                ui.label("(!) Note: Once removed, the SteamID cannot be recovered from the save file.");
            });
        });
    }

    fn show_demo_transfer_option(&mut self, ui: &mut egui::Ui, is_replacer: bool) {
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                let transfer_demo_save = if is_replacer {
                    &mut self.replacer.transfer_demo_save
                } else {
                    &mut self.remover.transfer_demo_save
                };
                
                let response = ui.checkbox(transfer_demo_save, "📋 Transfer Demo Save (removes 'Demo00' from filename)");
                
                if is_replacer && response.changed() && *transfer_demo_save {
                    if !self.replacer.file_path.contains("Demo00") {
                        self.replacer.status = "❌ Error: File does not contain 'Demo00'. Cannot transfer demo save.".to_string();
                        self.replacer.show_demo_error = true;
                    } else {
                        self.replacer.show_demo_error = false;
                    }
                }
            });
        });
    }

    fn show_replace_button(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            let button_enabled = !self.replacer.file_path.is_empty()
                && !self.replacer.new_steamid.is_empty()
                && (!self.replacer.transfer_demo_save || self.replacer.file_path.contains("Demo00"));
            let button = egui::Button::new("🔄 Replace SteamID")
                .min_size(egui::vec2(160.0, 32.0));
            let response = ui.add_enabled(button_enabled, button);
            if response.clicked() {
                self.replacer.handle_replace();
            }
        });
    }

    fn show_remove_button(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            let button_enabled = !self.remover.file_path.is_empty()
                && (!self.remover.transfer_demo_save || self.remover.file_path.contains("Demo00"));
            let button = egui::Button::new("❌ Remove SteamID")
                .min_size(egui::vec2(160.0, 32.0));
            let response = ui.add_enabled(button_enabled, button);
            if response.clicked() {
                self.remover.handle_remove();
            }
        });
    }

    fn show_status_section(&self, ui: &mut egui::Ui, status: &str, backup_filename: &str) {
        ui.group(|ui| {
            ui.set_min_height(60.0);
            ui.set_width(ui.available_width());
            ui.vertical_centered(|ui| {
                ui.heading("🍕 Status");
                if status.is_empty() {
                    if !backup_filename.is_empty() {
                        ui.colored_label(
                            egui::Color32::LIGHT_GRAY,
                            format!("Backup saved as: {}", backup_filename),
                        );
                    }
                } else {
                    if status.contains("Error") || status.contains("❌") {
                        ui.colored_label(egui::Color32::from_rgb(220, 80, 80), status);
                    } else if status.contains("Successfully") || status.contains("✅") {
                        ui.colored_label(egui::Color32::from_rgb(80, 200, 120), status);
                    } else if status.contains("(!)") {
                        ui.colored_label(egui::Color32::from_rgb(255, 165, 0), status);
                    } else {
                        ui.colored_label(egui::Color32::LIGHT_BLUE, status);
                    }
                    if !backup_filename.is_empty() {
                        ui.colored_label(
                            egui::Color32::LIGHT_GRAY,
                            format!("Backup saved as: {}", backup_filename),
                        );
                    }
                }
            });
        });
    }

    fn show_dialogs(&mut self, ctx: &egui::Context) {
        if self.show_about {
            self.show_about_dialog(ctx);
        }

        if self.show_help {
            self.show_help_dialog(ctx);
        }
    }

    fn show_about_dialog(&mut self, ctx: &egui::Context) {
        egui::Window::new("About")
            .collapsible(false)
            .resizable(false)
            .default_width(370.0)
            .max_width(400.0)
            .max_height(320.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    ui.heading("Stellar Blade SteamID Tool");
                    ui.add_space(10.0);
                    ui.label("Version 1.1");
                    ui.add_space(5.0);
                    ui.label("A tool for replacing or removing SteamID from Stellar Blade save files");
                    ui.label("Made by Dxian998 on NexusMods");
                    ui.add_space(15.0);
                    
                    ui.separator();
                    ui.add_space(10.0);
                    
                    ui.label("Features:");
                    ui.label("• Replace SteamID in Stellar Blade saves");
                    ui.label("• Remove SteamID (universal saves)");
                    ui.label("• Show current SteamID in file");
                    ui.label("• Demo save transfer support");
                    ui.label("• Automatic backup creation");
                    ui.label("• Drag & drop support");
                    ui.label("• Safe file operations");
                    
                    ui.add_space(15.0);
                    
                    ui.horizontal(|ui| {
                        if ui.button("✅ Close").clicked() {
                            self.show_about = false;
                        }
                    });
                });
            });
    }

    fn show_help_dialog(&mut self, ctx: &egui::Context) {
        egui::Window::new("Help")
            .collapsible(false)
            .resizable(false)
            .default_width(480.0)
            .max_width(520.0)
            .max_height(450.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    egui::ScrollArea::vertical()
                        .max_height(360.0)
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.heading("How to use Stellar Blade SteamID Tool");
                            });
                            ui.add_space(10.0);

                            ui.label("🔄 Replacer Tab:");
                            ui.label("Replace SteamID with a different one");
                            ui.add_space(5.0);
                            
                            ui.label("❌ Remover Tab:");
                            ui.label("Remove SteamID entirely (creates universal saves)");
                            ui.add_space(10.0);

                            ui.label("📋 Step-by-step guide:");
                            ui.add_space(5.0);

                            ui.label("1. 📁 Select your Stellar Blade save file:");
                            ui.label("   • Click 'Browse' button, or");
                            ui.label("   • Drag & drop the file into the window");
                            ui.add_space(5.0);

                            ui.label("2. 🔍 View current SteamID (if found):");
                            ui.label("   • The app will show the existing SteamID in the file");
                            ui.add_space(5.0);

                            ui.label("3a. For Replacer:");
                            ui.label("   • Enter the new 17-digit SteamID");
                            ui.label("   • Must start with '7656'");
                            ui.label("   • Example: 76561198123456789");
                            ui.add_space(5.0);

                            ui.label("3b. For Remover:");
                            ui.label("   • No additional input needed");
                            ui.label("   • Creates universal saves that work for anyone");
                            ui.add_space(5.0);

                            ui.label("4. 📋 Transfer Demo Save (Optional):");
                            ui.label("   • Check this for demo save files");
                            ui.label("   • Removes 'Demo00' from filename");
                            ui.label("   • Keeps backup as demo file");
                            ui.add_space(5.0);

                            ui.label("5. 🔄 Click 'Replace SteamID' or 'Remove SteamID'");
                            ui.label("   • A backup (.bak) will be created automatically");
                            ui.add_space(10.0);

                            ui.separator();
                            ui.label("🔍 Finding your SteamID:");
                            ui.label("• Visit steamid.io or steamidfinder.com");
                            ui.label("• Enter your Steam profile URL");
                            ui.label("• Copy the 'SteamID64' number");

                            ui.add_space(5.0);

                            ui.separator();
                            ui.colored_label(egui::Color32::from_rgb(80, 200, 120), "✅ Safety:");
                            ui.label("• Automatic backup creation (.bak extension)");
                            ui.label("• Original file is preserved before changes");
                            ui.label("• Prevents replacing with same SteamID");
                            ui.add_space(5.0);
                        });

                    ui.add_space(10.0);
                    ui.horizontal_centered(|ui| {
                        if ui.button("✅ Close").clicked() {
                            self.show_help = false;
                        }
                    });
                });
            });
    }
}