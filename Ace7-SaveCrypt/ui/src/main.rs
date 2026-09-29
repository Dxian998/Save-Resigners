#![windows_subsystem = "windows"]

use std::fs;
use std::path::Path;
use std::time::Instant;

use ace7_savetoolkit::{
    container::EcsdSave, crypto::Crypto, finder::KeyFinder, parse_steam_id, TargetKey,
};
use iced::widget::{
    button, checkbox, column, container, row, rule, scrollable, text, text_editor, text_input,
};
use iced::{Alignment, Color, Element, Length, Task, Theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveTab {
    Resign,
    Decrypt,
    Encrypt,
}

#[derive(Debug, Clone)]
enum Message {
    TabSelected(ActiveTab),

    ResignInputChanged(String),
    ResignAutoDetectToggled(bool),
    ResignSourceOfflineToggled(bool),
    ResignSourceSteamIdChanged(String),
    ResignTargetOfflineToggled(bool),
    ResignTargetSteamIdChanged(String),
    BrowseResignInput,
    ExecuteResign,

    DecryptInputChanged(String),
    DecryptAutoDetectToggled(bool),
    DecryptOfflineToggled(bool),
    DecryptSteamIdChanged(String),
    BrowseDecryptInput,
    ExecuteDecrypt,

    EncryptInputChanged(String),
    EncryptOfflineToggled(bool),
    EncryptSteamIdChanged(String),
    BrowseEncryptInput,
    ExecuteEncrypt,

    OutputAction(text_editor::Action),
    OperationFinished(Result<String, String>),
}

struct App {
    active_tab: ActiveTab,

    resign_input: String,
    resign_auto_detect: bool,
    resign_source_offline: bool,
    resign_source_steamid: String,
    resign_target_offline: bool,
    resign_target_steamid: String,

    decrypt_input: String,
    decrypt_auto_detect: bool,
    decrypt_offline: bool,
    decrypt_steamid: String,

    encrypt_input: String,
    encrypt_offline: bool,
    encrypt_steamid: String,

    is_busy: bool,
    output_content: text_editor::Content,
}

impl Default for App {
    fn default() -> Self {
        Self {
            active_tab: ActiveTab::Resign,

            resign_input: String::new(),
            resign_auto_detect: true,
            resign_source_offline: false,
            resign_source_steamid: String::new(),
            resign_target_offline: false,
            resign_target_steamid: String::new(),

            decrypt_input: String::new(),
            decrypt_auto_detect: true,
            decrypt_offline: false,
            decrypt_steamid: String::new(),

            encrypt_input: String::new(),
            encrypt_offline: false,
            encrypt_steamid: String::new(),

            is_busy: false,
            output_content: text_editor::Content::with_text("Ready to use."),
        }
    }
}

fn create_backup(input_path: &str) -> Result<String, String> {
    let p = Path::new(input_path);
    let bak_path = p.with_extension(format!(
        "{}.bak",
        p.extension().and_then(|e| e.to_str()).unwrap_or("sav")
    ));

    if !bak_path.exists() {
        fs::copy(p, &bak_path)
            .map_err(|e| format!("Failed to create backup at {}: {}", bak_path.display(), e))?;
        Ok(format!("Backup created: {}", bak_path.display()))
    } else {
        Ok(format!("Existing backup found: {}", bak_path.display()))
    }
}

fn sanitize_steamid_input(input: &str) -> String {
    let digits: String = input.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.starts_with("7656") {
        digits.chars().take(17).collect()
    } else {
        digits.chars().take(10).collect()
    }
}

impl App {
    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TabSelected(tab) => {
                self.active_tab = tab;
                Task::none()
            }

            Message::ResignInputChanged(val) => {
                self.resign_input = val;
                Task::none()
            }
            Message::ResignAutoDetectToggled(val) => {
                self.resign_auto_detect = val;
                if val {
                    self.resign_source_offline = false;
                }
                Task::none()
            }
            Message::ResignSourceOfflineToggled(val) => {
                self.resign_source_offline = val;
                if val {
                    self.resign_auto_detect = false;
                }
                Task::none()
            }
            Message::ResignSourceSteamIdChanged(val) => {
                self.resign_source_steamid = sanitize_steamid_input(&val);
                Task::none()
            }
            Message::ResignTargetOfflineToggled(val) => {
                self.resign_target_offline = val;
                Task::none()
            }
            Message::ResignTargetSteamIdChanged(val) => {
                self.resign_target_steamid = sanitize_steamid_input(&val);
                Task::none()
            }
            Message::BrowseResignInput => {
                if let Some(p) = rfd::FileDialog::new()
                    .add_filter("Save Files (*.sav)", &["sav"])
                    .pick_file()
                {
                    self.resign_input = p.to_string_lossy().to_string();
                }
                Task::none()
            }
            Message::ExecuteResign => {
                if self.is_busy {
                    return Task::none();
                }
                let input_path = self.resign_input.clone();
                let auto_detect = self.resign_auto_detect;
                let source_offline = self.resign_source_offline;
                let source_steamid_str = self.resign_source_steamid.clone();
                let target_offline = self.resign_target_offline;
                let target_steamid_str = self.resign_target_steamid.clone();

                self.is_busy = true;
                self.output_content = text_editor::Content::with_text(&format!(
                    "Resigning save: {}\nPlease wait...",
                    input_path
                ));

                Task::perform(
                    async move {
                        let (tx, rx) = futures::channel::oneshot::channel();
                        std::thread::spawn(move || {
                            let res = (|| -> Result<String, String> {
                                let op_start = Instant::now();

                                if input_path.trim().is_empty() {
                                    return Err("Please select a save file.".to_string());
                                }

                                let target_key = if target_offline {
                                    TargetKey::Offline
                                } else {
                                    if target_steamid_str.trim().is_empty() {
                                        return Err(
                                            "Please enter a Target SteamID or tick 'Convert to Offline Save'."
                                                .to_string(),
                                        );
                                    }
                                    let id = parse_steam_id(&target_steamid_str)?;
                                    TargetKey::Online(id)
                                };

                                let backup_msg = create_backup(&input_path)?;

                                let raw = fs::read(&input_path)
                                    .map_err(|e| format!("Failed to read save file: {}", e))?;
                                let save = EcsdSave::parse(&raw)
                                    .map_err(|e| format!("Save parsing failed: {}", e))?;

                                let (source_desc, source_key) = if source_offline {
                                    ("Offline Key".to_string(), Crypto::offline_key())
                                } else if !source_steamid_str.trim().is_empty() {
                                    let id = parse_steam_id(&source_steamid_str)?;
                                    (format!("SteamID: {}", id), Crypto::derive_key(id))
                                } else if auto_detect {
                                    let detect_start = Instant::now();
                                    let block0: [u8; 16] = save
                                        .ciphertext
                                        .get(..16)
                                        .ok_or("Ciphertext too short")?
                                        .try_into()
                                        .map_err(|_| "Invalid block size")?;

                                    let found = KeyFinder::find_key(&block0, false)
                                        .ok_or("Could not auto-detect encryption key. Specify SteamID manually.")?;

                                    let detect_secs = detect_start.elapsed().as_secs_f64();
                                    let desc = if found.is_offline {
                                        format!("Detected Offline Save ({:.2}s)", detect_secs)
                                    } else {
                                        format!("Detected SteamID: {} ({:.2}s)", found.steamid64, detect_secs)
                                    };
                                    (desc, found.key)
                                } else {
                                    return Err("Please enable auto-detect, specify source SteamID, or select offline source.".to_string());
                                };

                                let decrypted = save
                                    .decrypt(&source_key)
                                    .map_err(|e| format!("Decryption failed: {}", e))?;

                                let target_key_bytes = target_key.to_aes_key();
                                let target_desc = match target_key {
                                    TargetKey::Offline => "Offline Save".to_string(),
                                    TargetKey::Online(id) => format!("SteamID: {}", id),
                                };

                                let resigned = EcsdSave::pack(&decrypted, &target_key_bytes);
                                fs::write(&input_path, &resigned)
                                    .map_err(|e| format!("Failed to write save: {}", e))?;

                                let total_secs = op_start.elapsed().as_secs_f64();

                                Ok(format!(
                                    "Resign completed successfully ({:.2}s)!\n\n  - Source: {}\n  - Target: {}\n  - {}\n  - Updated: {} ({} bytes)",
                                    total_secs, source_desc, target_desc, backup_msg, input_path, resigned.len()
                                ))
                            })();
                            let _ = tx.send(res);
                        });
                        rx.await.unwrap_or_else(|_| Err("Task was cancelled".to_string()))
                    },
                    Message::OperationFinished,
                )
            }

            Message::DecryptInputChanged(val) => {
                self.decrypt_input = val;
                Task::none()
            }
            Message::DecryptAutoDetectToggled(val) => {
                self.decrypt_auto_detect = val;
                if val {
                    self.decrypt_offline = false;
                }
                Task::none()
            }
            Message::DecryptOfflineToggled(val) => {
                self.decrypt_offline = val;
                if val {
                    self.decrypt_auto_detect = false;
                }
                Task::none()
            }
            Message::DecryptSteamIdChanged(val) => {
                self.decrypt_steamid = sanitize_steamid_input(&val);
                Task::none()
            }
            Message::BrowseDecryptInput => {
                if let Some(p) = rfd::FileDialog::new()
                    .add_filter("Save Files (*.sav)", &["sav"])
                    .pick_file()
                {
                    self.decrypt_input = p.to_string_lossy().to_string();
                }
                Task::none()
            }
            Message::ExecuteDecrypt => {
                if self.is_busy {
                    return Task::none();
                }
                let input_path = self.decrypt_input.clone();
                let auto_detect = self.decrypt_auto_detect;
                let offline = self.decrypt_offline;
                let steamid_str = self.decrypt_steamid.clone();

                self.is_busy = true;
                self.output_content = text_editor::Content::with_text(&format!(
                    "Decrypting save: {}\nPlease wait...",
                    input_path
                ));

                Task::perform(
                    async move {
                        let (tx, rx) = futures::channel::oneshot::channel();
                        std::thread::spawn(move || {
                            let res = (|| -> Result<String, String> {
                                let op_start = Instant::now();

                                if input_path.trim().is_empty() {
                                    return Err("Please select an input save file.".to_string());
                                }

                                let backup_msg = create_backup(&input_path)?;

                                let raw = fs::read(&input_path)
                                    .map_err(|e| format!("Failed to read save file: {}", e))?;
                                let save = EcsdSave::parse(&raw)
                                    .map_err(|e| format!("Save parsing failed: {}", e))?;

                                let (key_desc, key) = if offline {
                                    ("Offline Key".to_string(), Crypto::offline_key())
                                } else if !steamid_str.trim().is_empty() {
                                    let id = parse_steam_id(&steamid_str)?;
                                    (format!("SteamID: {}", id), Crypto::derive_key(id))
                                } else if auto_detect {
                                    let detect_start = Instant::now();
                                    let block0: [u8; 16] = save
                                        .ciphertext
                                        .get(..16)
                                        .ok_or("Ciphertext too short")?
                                        .try_into()
                                        .map_err(|_| "Invalid block size")?;

                                    let found = KeyFinder::find_key(&block0, false)
                                        .ok_or("Could not auto-detect encryption key. Specify SteamID manually.")?;

                                    let detect_secs = detect_start.elapsed().as_secs_f64();
                                    let desc = if found.is_offline {
                                        format!("Detected Offline Key ({:.2}s)", detect_secs)
                                    } else {
                                        format!("Detected SteamID: {} ({:.2}s)", found.steamid64, detect_secs)
                                    };
                                    (desc, found.key)
                                } else {
                                    return Err("Please enable auto-detect, select offline mode, or provide a SteamID.".to_string());
                                };

                                let decrypted = save
                                    .decrypt(&key)
                                    .map_err(|e| format!("Decryption failed: {}", e))?;

                                fs::write(&input_path, &decrypted)
                                    .map_err(|e| format!("Failed to write decrypted save: {}", e))?;

                                let total_secs = op_start.elapsed().as_secs_f64();

                                Ok(format!(
                                    "Decryption completed successfully ({:.2}s)!\n\n  - Key: {}\n  - {}\n  - Updated: {} ({} bytes uncompressed GVAS)",
                                    total_secs, key_desc, backup_msg, input_path, decrypted.len()
                                ))
                            })();
                            let _ = tx.send(res);
                        });
                        rx.await.unwrap_or_else(|_| Err("Task was cancelled".to_string()))
                    },
                    Message::OperationFinished,
                )
            }

            Message::EncryptInputChanged(val) => {
                self.encrypt_input = val;
                Task::none()
            }
            Message::EncryptOfflineToggled(val) => {
                self.encrypt_offline = val;
                Task::none()
            }
            Message::EncryptSteamIdChanged(val) => {
                self.encrypt_steamid = sanitize_steamid_input(&val);
                Task::none()
            }
            Message::BrowseEncryptInput => {
                if let Some(p) = rfd::FileDialog::new()
                    .add_filter("Save Files (*.sav)", &["sav"])
                    .pick_file()
                {
                    self.encrypt_input = p.to_string_lossy().to_string();
                }
                Task::none()
            }
            Message::ExecuteEncrypt => {
                if self.is_busy {
                    return Task::none();
                }
                let input_path = self.encrypt_input.clone();
                let offline = self.encrypt_offline;
                let steamid_str = self.encrypt_steamid.clone();

                self.is_busy = true;
                self.output_content = text_editor::Content::with_text(&format!(
                    "Encrypting save: {}\nPlease wait...",
                    input_path
                ));

                Task::perform(
                    async move {
                        let (tx, rx) = futures::channel::oneshot::channel();
                        std::thread::spawn(move || {
                            let res = (|| -> Result<String, String> {
                                let op_start = Instant::now();

                                if input_path.trim().is_empty() {
                                    return Err("Please select a plaintext GVAS save file.".to_string());
                                }

                                let backup_msg = create_backup(&input_path)?;

                                let (target_desc, key) = if offline {
                                    ("Offline Save".to_string(), Crypto::offline_key())
                                } else {
                                    if steamid_str.trim().is_empty() {
                                        return Err(
                                            "Please enter a Target SteamID or select offline mode."
                                                .to_string(),
                                        );
                                    }
                                    let id = parse_steam_id(&steamid_str)?;
                                    (format!("SteamID: {}", id), Crypto::derive_key(id))
                                };

                                let payload = fs::read(&input_path)
                                    .map_err(|e| format!("Failed to read input file: {}", e))?;

                                let container_bytes = EcsdSave::pack(&payload, &key);
                                fs::write(&input_path, &container_bytes)
                                    .map_err(|e| format!("Failed to write encrypted save: {}", e))?;

                                let total_secs = op_start.elapsed().as_secs_f64();

                                Ok(format!(
                                    "Encryption completed successfully ({:.2}s)!\n\n  - Target: {}\n  - {}\n  - Updated: {} ({} bytes ECSD container)",
                                    total_secs, target_desc, backup_msg, input_path, container_bytes.len()
                                ))
                            })();
                            let _ = tx.send(res);
                        });
                        rx.await.unwrap_or_else(|_| Err("Task was cancelled".to_string()))
                    },
                    Message::OperationFinished,
                )
            }

            Message::OutputAction(action) => {
                if !action.is_edit() {
                    self.output_content.perform(action);
                }
                Task::none()
            }

            Message::OperationFinished(res) => {
                self.is_busy = false;
                match res {
                    Ok(msg) => {
                        self.output_content = text_editor::Content::with_text(&msg);
                    }
                    Err(err) => {
                        self.output_content =
                            text_editor::Content::with_text(&format!("ERROR:\n{}", err));
                    }
                }
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let title_bar = row![
            text("ACE COMBAT 7: SKIES UNKNOWN")
                .size(17)
                .style(|_theme: &Theme| text::Style {
                    color: Some(Color::from_rgb8(110, 180, 255)),
                }),
            text("-").size(17).style(|_theme: &Theme| text::Style {
                color: Some(Color::from_rgb8(80, 90, 110)),
            }),
            text("Save Toolkit").size(17),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let nav_buttons = row![
            button(text("Resign Save"))
                .style(if self.active_tab == ActiveTab::Resign {
                    button::primary
                } else {
                    button::secondary
                })
                .on_press(Message::TabSelected(ActiveTab::Resign)),
            button(text("Decrypt Save"))
                .style(if self.active_tab == ActiveTab::Decrypt {
                    button::primary
                } else {
                    button::secondary
                })
                .on_press(Message::TabSelected(ActiveTab::Decrypt)),
            button(text("Encrypt Save"))
                .style(if self.active_tab == ActiveTab::Encrypt {
                    button::primary
                } else {
                    button::secondary
                })
                .on_press(Message::TabSelected(ActiveTab::Encrypt)),
        ]
        .spacing(8);

        let content: Element<'_, Message> = match self.active_tab {
            ActiveTab::Resign => self.view_resign(),
            ActiveTab::Decrypt => self.view_decrypt(),
            ActiveTab::Encrypt => self.view_encrypt(),
        };

        let editor_widget = text_editor(&self.output_content)
            .on_action(Message::OutputAction)
            .padding(10);

        let main_layout = column![
            title_bar,
            nav_buttons,
            rule::horizontal(1),
            content,
            rule::horizontal(1),
            text("Output:").size(15),
            editor_widget,
        ]
        .spacing(10)
        .padding(16);

        container(scrollable(main_layout))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_resign(&self) -> Element<'_, Message> {
        let input_row = row![
            text_input("Path to save file (*.sav)", &self.resign_input)
                .on_input(Message::ResignInputChanged)
                .width(Length::Fill),
            button(text("Browse...")).on_press(Message::BrowseResignInput),
        ]
        .spacing(8);

        let mut source_items = vec![
            labeled_checkbox(
                "Auto-detect encryption key",
                self.resign_auto_detect,
                Message::ResignAutoDetectToggled,
            ),
            text("Analyzing the save file to identify the encryption key may take a few seconds.")
                .size(11)
                .style(|_theme: &Theme| text::Style {
                    color: Some(Color::from_rgb8(140, 150, 165)),
                })
                .into(),
            labeled_checkbox(
                "Source is an Offline Save",
                self.resign_source_offline,
                Message::ResignSourceOfflineToggled,
            ),
        ];

        if !self.resign_auto_detect && !self.resign_source_offline {
            source_items.push(
                text_input(
                    "Source SteamID / AccountID (manual override)",
                    &self.resign_source_steamid,
                )
                .on_input(Message::ResignSourceSteamIdChanged)
                .into(),
            );
        }

        let mut target_items = vec![labeled_checkbox(
            "Convert to Offline Save",
            self.resign_target_offline,
            Message::ResignTargetOfflineToggled,
        )];

        if self.resign_target_offline {
            target_items.push(
                text("Save will be configured for offline play (without Steam).")
                    .size(11)
                    .style(|_theme: &Theme| text::Style {
                        color: Some(Color::from_rgb8(100, 200, 140)),
                    })
                    .into(),
            );
        } else {
            target_items.push(
                text_input(
                    "Target SteamID64 or AccountID (e.g. 76561197960287930)",
                    &self.resign_target_steamid,
                )
                .on_input(Message::ResignTargetSteamIdChanged)
                .into(),
            );
        }

        let action_button = if self.is_busy {
            button(text("Processing... Please wait")).style(button::secondary)
        } else {
            button(text("Resign Save"))
                .style(button::primary)
                .on_press(Message::ExecuteResign)
        };

        column![
            text("Save File:").size(15),
            input_row,
            column(source_items).spacing(5),
            rule::horizontal(1),
            text("Target:").size(15),
            column(target_items).spacing(5),
            action_button,
        ]
        .spacing(8)
        .into()
    }

    fn view_decrypt(&self) -> Element<'_, Message> {
        let input_row = row![
            text_input("Path to encrypted save (*.sav)", &self.decrypt_input)
                .on_input(Message::DecryptInputChanged)
                .width(Length::Fill),
            button(text("Browse...")).on_press(Message::BrowseDecryptInput),
        ]
        .spacing(8);

        let mut key_items = vec![
            labeled_checkbox(
                "Auto-detect encryption key",
                self.decrypt_auto_detect,
                Message::DecryptAutoDetectToggled,
            ),
            text("Analyzing the save file to identify the encryption key may take a few seconds.")
                .size(11)
                .style(|_theme: &Theme| text::Style {
                    color: Some(Color::from_rgb8(140, 150, 165)),
                })
                .into(),
            labeled_checkbox(
                "Decrypt using Offline Key",
                self.decrypt_offline,
                Message::DecryptOfflineToggled,
            ),
        ];

        if !self.decrypt_auto_detect && !self.decrypt_offline {
            key_items.push(
                text_input("Manual SteamID64 or AccountID", &self.decrypt_steamid)
                    .on_input(Message::DecryptSteamIdChanged)
                    .into(),
            );
        }

        let action_button = if self.is_busy {
            button(text("Decrypting... Please wait")).style(button::secondary)
        } else {
            button(text("Decrypt Save"))
                .style(button::primary)
                .on_press(Message::ExecuteDecrypt)
        };

        column![
            text("Save File:").size(15),
            input_row,
            column(key_items).spacing(5),
            action_button,
        ]
        .spacing(8)
        .into()
    }

    fn view_encrypt(&self) -> Element<'_, Message> {
        let input_row = row![
            text_input("Path to plaintext GVAS save (*.sav)", &self.encrypt_input)
                .on_input(Message::EncryptInputChanged)
                .width(Length::Fill),
            button(text("Browse...")).on_press(Message::BrowseEncryptInput),
        ]
        .spacing(8);

        let mut target_items = vec![labeled_checkbox(
            "Encrypt as Offline Save",
            self.encrypt_offline,
            Message::EncryptOfflineToggled,
        )];

        if self.encrypt_offline {
            target_items.push(
                text("Save will be configured for offline play (without Steam).")
                    .size(11)
                    .style(|_theme: &Theme| text::Style {
                        color: Some(Color::from_rgb8(100, 200, 140)),
                    })
                    .into(),
            );
        } else {
            target_items.push(
                text_input(
                    "Target SteamID64 or AccountID (e.g. 76561198028423168)",
                    &self.encrypt_steamid,
                )
                .on_input(Message::EncryptSteamIdChanged)
                .into(),
            );
        }

        let action_button = if self.is_busy {
            button(text("Encrypting... Please wait")).style(button::secondary)
        } else {
            button(text("Encrypt Save"))
                .style(button::primary)
                .on_press(Message::ExecuteEncrypt)
        };

        column![
            text("GVAS Save File:").size(15),
            input_row,
            column(target_items).spacing(5),
            action_button,
        ]
        .spacing(8)
        .into()
    }
}

fn labeled_checkbox<'a>(
    label: &'a str,
    is_checked: bool,
    on_toggle: impl Fn(bool) -> Message + 'static,
) -> Element<'a, Message> {
    row![checkbox(is_checked).on_toggle(on_toggle), text(label),]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
}

fn load_window_icon() -> Option<iced::window::Icon> {
    let icon_bytes = include_bytes!("../assets/icon.ico");
    let icon_dir = ico::IconDir::read(std::io::Cursor::new(icon_bytes)).ok()?;
    let entry = icon_dir
        .entries()
        .iter()
        .find(|e| e.width() == 32 && e.height() == 32)
        .or_else(|| icon_dir.entries().first())?;
    let image = entry.decode().ok()?;
    iced::window::icon::from_rgba(image.rgba_data().to_vec(), image.width(), image.height()).ok()
}

fn main() -> iced::Result {
    iced::application(App::default, App::update, App::view)
        .title("Ace Combat 7 - Save Toolkit")
        .theme(App::theme)
        .window(iced::window::Settings {
            size: iced::Size::new(720.0, 600.0),
            icon: load_window_icon(),
            resizable: false,
            ..Default::default()
        })
        .run()
}
