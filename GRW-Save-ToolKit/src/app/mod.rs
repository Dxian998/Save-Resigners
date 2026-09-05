pub mod backup;
pub mod diagnostics;
pub mod inspector;
pub mod resigner;
pub mod settings;
pub mod workshop;
pub mod editor;

use std::cell::RefCell;
use std::rc::Rc;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use slint::winit_030::WinitWindowAccessor;
use crate::{AppWindow, BackupItem, SaveInfoData, EditorSaveData};
use self::backup::BackupManager;
use self::settings::AppSettings;

pub fn setup(app: &AppWindow) {
    let settings = AppSettings::load();
    app.set_active_tab(settings.default_tab);
    app.set_default_tab_setting(settings.default_tab);
    app.set_backup_ubi_dir(SharedString::from(&settings.ubisoft_dir));

    let backup_mgr = Rc::new(RefCell::new(BackupManager::new()));
    let editor_state = Rc::new(RefCell::new(None::<editor::EditorState>));

    {
        let handle = app.as_weak();
        app.on_switch_tab(move |idx| {
            if let Some(w) = handle.upgrade() {
                w.set_active_tab(idx);
            }
        });
    }

    {
        let handle = app.as_weak();
        app.window().on_winit_window_event(move |_, event| {
            if let slint::winit_030::winit::event::WindowEvent::DroppedFile(p) = event {
                if let Some(w) = handle.upgrade() {
                    let path_str = p.to_string_lossy().to_string();
                    let is_f = p.is_file();
                    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
                    match w.get_active_tab() {
                        0 => {
                            w.set_resign_is_folder(!is_f);
                            w.set_resign_path(SharedString::from(&path_str));
                        }
                        1 => {
                            if p.is_dir() {
                                w.set_backup_target_dir(SharedString::from(&path_str));
                            }
                        }
                        2 => {
                            if is_f {
                                w.set_inspect_path(SharedString::from(&path_str));
                                let res = inspector::inspect_save(&path_str);
                                let info = SaveInfoData {
                                    loaded: res.loaded,
                                    file_name: SharedString::from(res.file_name),
                                    file_size: SharedString::from(res.file_size),
                                    platform: SharedString::from(res.platform),
                                    slot_title: SharedString::from(res.slot_title),
                                    seed1: SharedString::from(res.seed1),
                                    seed2: SharedString::from(res.seed2),
                                    seed3: SharedString::from(res.seed3),
                                    digest: SharedString::from(res.digest),
                                    hex_preview: SharedString::from(res.hex_preview),
                                };
                                w.set_inspect_data(info);
                            }
                        }
                        3 => {
                            if ext == "bin" {
                                w.set_workshop_repack_bin(SharedString::from(&path_str));
                                let auto_hdr = p.with_extension("header");
                                if auto_hdr.exists() {
                                    w.set_workshop_repack_hdr(SharedString::from(auto_hdr.to_string_lossy().as_ref()));
                                }
                            } else if ext == "header" {
                                w.set_workshop_repack_hdr(SharedString::from(&path_str));
                            } else if ext == "save" {
                                w.set_workshop_decrypt_path(SharedString::from(&path_str));
                            }
                        }
                        4 => {
                            if is_f && ext == "save" {
                                w.set_editor_save_path(SharedString::from(&path_str));
                            }
                        }
                        5 => {
                            if is_f {
                                w.set_diag_path(SharedString::from(&path_str));
                            }
                        }
                        _ => {}
                    }
                }
                slint::winit_030::EventResult::PreventDefault
            } else {
                slint::winit_030::EventResult::Propagate
            }
        });
    }

    {
        let handle = app.as_weak();
        app.on_save_settings(move |tab_idx| {
            let ubi = handle.upgrade().map(|w| w.get_backup_ubi_dir().to_string()).unwrap_or_default();
            let s = AppSettings { default_tab: tab_idx, ubisoft_dir: ubi };
            s.save();
        });
    }

    {
        let handle = app.as_weak();
        app.on_browse_resign_path(move || {
            if let Some(w) = handle.upgrade() {
                if w.get_resign_is_folder() {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        w.set_resign_path(SharedString::from(dir.to_string_lossy().as_ref()));
                    }
                } else if let Some(file) = rfd::FileDialog::new().add_filter("Save Files", &["save"]).pick_file() {
                    w.set_resign_path(SharedString::from(file.to_string_lossy().as_ref()));
                }
            }
        });
    }

    {
        let handle = app.as_weak();
        app.on_browse_resign_out(move || {
            if let Some(w) = handle.upgrade() {
                if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                    w.set_resign_out_dir(SharedString::from(dir.to_string_lossy().as_ref()));
                }
            }
        });
    }

    {
        let handle = app.as_weak();
        app.on_run_resign(move || {
            if let Some(w) = handle.upgrade() {
                let path = w.get_resign_path().to_string();
                let is_folder = w.get_resign_is_folder();
                let target_id = w.get_resign_target_id().to_string();
                let source_id = w.get_resign_source_id().to_string();
                let platform = w.get_resign_platform();
                let out_dir = w.get_resign_out_dir().to_string();
                let log = resigner::execute_resign(&path, is_folder, &target_id, &source_id, platform, &out_dir);
                w.set_resign_log(SharedString::from(log));
            }
        });
    }

    {
        let handle = app.as_weak();
        app.on_browse_inspect_file(move || {
            if let Some(w) = handle.upgrade() {
                if let Some(file) = rfd::FileDialog::new().add_filter("Save Files", &["save"]).pick_file() {
                    w.set_inspect_path(SharedString::from(file.to_string_lossy().as_ref()));
                }
            }
        });
    }

    {
        let handle = app.as_weak();
        app.on_run_inspect(move || {
            if let Some(w) = handle.upgrade() {
                let path = w.get_inspect_path().to_string();
                let res = inspector::inspect_save(&path);
                let info = SaveInfoData {
                    loaded: res.loaded,
                    file_name: SharedString::from(res.file_name),
                    file_size: SharedString::from(res.file_size),
                    platform: SharedString::from(res.platform),
                    slot_title: SharedString::from(res.slot_title),
                    seed1: SharedString::from(res.seed1),
                    seed2: SharedString::from(res.seed2),
                    seed3: SharedString::from(res.seed3),
                    digest: SharedString::from(res.digest),
                    hex_preview: SharedString::from(res.hex_preview),
                };
                w.set_inspect_data(info);
            }
        });
    }

    {
        let handle = app.as_weak();
        app.on_browse_workshop_decrypt_file(move || {
            if let Some(w) = handle.upgrade() {
                if let Some(file) = rfd::FileDialog::new().add_filter("Save Files", &["save"]).pick_file() {
                    w.set_workshop_decrypt_path(SharedString::from(file.to_string_lossy().as_ref()));
                }
            }
        });
    }

    {
        let handle = app.as_weak();
        app.on_run_workshop_decrypt(move || {
            if let Some(w) = handle.upgrade() {
                let path = w.get_workshop_decrypt_path().to_string();
                let log = workshop::extract_payload(&path, "_OUTPUT");
                w.set_workshop_log(SharedString::from(log));
            }
        });
    }

    {
        let handle = app.as_weak();
        app.on_browse_workshop_repack_bin(move || {
            if let Some(w) = handle.upgrade() {
                if let Some(file) = rfd::FileDialog::new().add_filter("Payload Bin", &["bin"]).pick_file() {
                    w.set_workshop_repack_bin(SharedString::from(file.to_string_lossy().as_ref()));
                }
            }
        });
    }

    {
        let handle = app.as_weak();
        app.on_browse_workshop_repack_hdr(move || {
            if let Some(w) = handle.upgrade() {
                if let Some(file) = rfd::FileDialog::new().add_filter("Header File", &["header"]).pick_file() {
                    w.set_workshop_repack_hdr(SharedString::from(file.to_string_lossy().as_ref()));
                }
            }
        });
    }

    {
        let handle = app.as_weak();
        app.on_run_workshop_repack(move || {
            if let Some(w) = handle.upgrade() {
                let bin = w.get_workshop_repack_bin().to_string();
                let hdr = w.get_workshop_repack_hdr().to_string();
                let id = w.get_workshop_repack_id().to_string();
                let platform = w.get_workshop_platform();
                let log = workshop::repack_payload(&bin, &hdr, &id, platform, "_OUTPUT");
                w.set_workshop_log(SharedString::from(log));
            }
        });
    }

    {
        let handle = app.as_weak();
        app.on_browse_editor_file(move || {
            if let Some(w) = handle.upgrade() {
                if let Some(file) = rfd::FileDialog::new().add_filter("Save Files", &["save"]).pick_file() {
                    w.set_editor_save_path(SharedString::from(file.to_string_lossy().as_ref()));
                }
            }
        });
    }

    {
        let handle = app.as_weak();
        let state_ref = editor_state.clone();
        app.on_run_editor_load(move || {
            if let Some(w) = handle.upgrade() {
                let path = w.get_editor_save_path().to_string();
                let (res, maybe_state) = editor::load_save(&path);
                let info = EditorSaveData {
                    loaded: res.loaded,
                    file_name: SharedString::from(res.file_name),
                    file_size: SharedString::from(res.file_size),
                    save_type: SharedString::from(res.save_type),
                    platform: SharedString::from(res.platform),
                    slot_title: SharedString::from(res.slot_title),
                    slot_counter: SharedString::from(res.slot_counter),
                    has_json: res.has_json,
                    json_content: SharedString::from(res.json_content),
                    payload_size: SharedString::from(res.payload_size),
                };
                w.set_editor_data(info);
                w.set_editor_log(SharedString::from(res.log));
                *state_ref.borrow_mut() = maybe_state;
            }
        });
    }

    {
        let handle = app.as_weak();
        let state_ref = editor_state.clone();
        app.on_run_editor_reload(move || {
            if let Some(w) = handle.upgrade() {
                let path = w.get_editor_save_path().to_string();
                let (res, maybe_state) = editor::load_save(&path);
                let info = EditorSaveData {
                    loaded: res.loaded,
                    file_name: SharedString::from(res.file_name),
                    file_size: SharedString::from(res.file_size),
                    save_type: SharedString::from(res.save_type),
                    platform: SharedString::from(res.platform),
                    slot_title: SharedString::from(res.slot_title),
                    slot_counter: SharedString::from(res.slot_counter),
                    has_json: res.has_json,
                    json_content: SharedString::from(res.json_content),
                    payload_size: SharedString::from(res.payload_size),
                };
                w.set_editor_data(info);
                w.set_editor_log(SharedString::from("[PASS] Reloaded original save state."));
                *state_ref.borrow_mut() = maybe_state;
            }
        });
    }

    {
        let handle = app.as_weak();
        let state_ref = editor_state.clone();
        app.on_export_editor_bin(move || {
            if let Some(w) = handle.upgrade() {
                let borrowed = state_ref.borrow();
                if let Some(state) = borrowed.as_ref() {
                    let log = editor::export_decrypted_bin(state, "_OUTPUT");
                    w.set_editor_log(SharedString::from(log));
                } else {
                    w.set_editor_log(SharedString::from("[ERROR] Please load a save file first."));
                }
            }
        });
    }

    {
        let handle = app.as_weak();
        let state_ref = editor_state.clone();
        app.on_adjust_editor_playtime(move |hours| {
            if let Some(w) = handle.upgrade() {
                let mut borrowed = state_ref.borrow_mut();
                if let Some(state) = borrowed.as_mut() {
                    let (new_counter, log) = editor::adjust_playtime(state, hours);
                    let mut data = w.get_editor_data();
                    data.slot_counter = SharedString::from(new_counter);
                    w.set_editor_data(data);
                    w.set_editor_log(SharedString::from(log));
                } else {
                    w.set_editor_log(SharedString::from("[ERROR] Please load a save file first."));
                }
            }
        });
    }

    {
        let handle = app.as_weak();
        let state_ref = editor_state.clone();
        app.on_apply_editor_profile_unlocks(move |unlock_type| {
            if let Some(w) = handle.upgrade() {
                let mut borrowed = state_ref.borrow_mut();
                if let Some(state) = borrowed.as_mut() {
                    let (new_json, log) = editor::apply_profile_unlocks(state, unlock_type);
                    if !new_json.is_empty() {
                        let mut data = w.get_editor_data();
                        data.json_content = SharedString::from(new_json);
                        w.set_editor_data(data);
                    }
                    w.set_editor_log(SharedString::from(log));
                } else {
                    w.set_editor_log(SharedString::from("[ERROR] Please load a save file first."));
                }
            }
        });
    }

    {
        let handle = app.as_weak();
        app.on_apply_editor_quick_patch(move |preset| {
            if let Some(w) = handle.upgrade() {
                match preset {
                    0 => {
                        w.set_editor_hex_find(SharedString::from("00 27 10 00"));
                        w.set_editor_hex_replace(SharedString::from("00 50 20 00"));
                        w.set_editor_log(SharedString::from("Preset loaded: Search '00 27 10 00' (10,000 resources) -> Replace '00 50 20 00' (2,117,632). Click 'Patch Bytes' to apply."));
                    }
                    1 => {
                        w.set_editor_hex_find(SharedString::from("05 00 00 00"));
                        w.set_editor_hex_replace(SharedString::from("FF 00 00 00"));
                        w.set_editor_log(SharedString::from("Preset loaded: Search '05 00 00 00' (5 Skill Points) -> Replace 'FF 00 00 00' (255). Click 'Patch Bytes' to apply."));
                    }
                    _ => {}
                }
            }
        });
    }

    {
        let handle = app.as_weak();
        let state_ref = editor_state.clone();
        app.on_apply_editor_hex_patch(move || {
            if let Some(w) = handle.upgrade() {
                let find_str = w.get_editor_hex_find().to_string();
                let replace_str = w.get_editor_hex_replace().to_string();
                let mut borrowed = state_ref.borrow_mut();
                if let Some(state) = borrowed.as_mut() {
                    let log = editor::apply_hex_patch(state, &find_str, &replace_str);
                    w.set_editor_log(SharedString::from(log));
                } else {
                    w.set_editor_log(SharedString::from("[ERROR] Please load a save file first."));
                }
            }
        });
    }

    {
        let handle = app.as_weak();
        let state_ref = editor_state.clone();
        app.on_run_editor_save(move || {
            if let Some(w) = handle.upgrade() {
                let data = w.get_editor_data();
                let title = data.slot_title.to_string();
                let counter = data.slot_counter.to_string();
                let json = data.json_content.to_string();
                let target_id = w.get_editor_target_uuid().to_string();
                let platform = w.get_editor_platform();
                let borrowed = state_ref.borrow();
                if let Some(state) = borrowed.as_ref() {
                    let log = editor::save_edited_save(state, &title, &counter, &json, &target_id, platform, "_OUTPUT");
                    w.set_editor_log(SharedString::from(log));
                } else {
                    w.set_editor_log(SharedString::from("[ERROR] No save loaded in editor."));
                }
            }
        });
    }

    {
        let handle = app.as_weak();
        app.on_browse_diag_file(move || {
            if let Some(w) = handle.upgrade() {
                if let Some(file) = rfd::FileDialog::new().add_filter("Save Files", &["save"]).pick_file() {
                    w.set_diag_path(SharedString::from(file.to_string_lossy().as_ref()));
                }
            }
        });
    }

    {
        let handle = app.as_weak();
        app.on_run_diag_verify(move || {
            if let Some(w) = handle.upgrade() {
                let path = w.get_diag_path().to_string();
                let uuid = w.get_diag_uuid().to_string();
                let (pass, msg) = diagnostics::verify_ownership(&path, &uuid);
                w.set_diag_success(pass);
                w.set_diag_result(SharedString::from(msg));
            }
        });
    }

    {
        let handle = app.as_weak();
        app.on_run_diag_roundtrip(move || {
            if let Some(w) = handle.upgrade() {
                let path = w.get_diag_path().to_string();
                let (pass, msg) = diagnostics::run_roundtrip(&path);
                w.set_diag_success(pass);
                w.set_diag_result(SharedString::from(msg));
            }
        });
    }

    {
        let handle = app.as_weak();
        app.on_browse_backup_ubi_dir(move || {
            if let Some(w) = handle.upgrade() {
                if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                    let dstr = dir.to_string_lossy().to_string();
                    w.set_backup_ubi_dir(SharedString::from(&dstr));
                    let tab = w.get_default_tab_setting();
                    AppSettings { default_tab: tab, ubisoft_dir: dstr }.save();
                }
            }
        });
    }

    {
        let handle = app.as_weak();
        let mgr = backup_mgr.clone();
        app.on_detect_backup_uuids(move || {
            if let Some(w) = handle.upgrade() {
                let ubi = w.get_backup_ubi_dir().to_string();
                let found = mgr.borrow().scan_uuids(&ubi);
                if let Some(first) = found.first() {
                    w.set_backup_target_uuid(SharedString::from(first));
                    w.set_backup_status(SharedString::from(format!("Found {} UUID(s). Selected: {}", found.len(), first)));
                } else {
                    w.set_backup_status(SharedString::from(format!("No UUID folders found in '{}'.", ubi)));
                }
            }
        });
    }

    {
        let handle = app.as_weak();
        app.on_browse_backup_dir(move || {
            if let Some(w) = handle.upgrade() {
                if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                    w.set_backup_target_dir(SharedString::from(dir.to_string_lossy().as_ref()));
                }
            }
        });
    }

    {
        let handle = app.as_weak();
        let mgr = backup_mgr.clone();
        app.on_run_create_snapshot(move || {
            if let Some(w) = handle.upgrade() {
                let ubi = w.get_backup_ubi_dir().to_string();
                let uuid = w.get_backup_target_uuid().to_string();
                let dir = w.get_backup_target_dir().to_string();
                let (_, msg) = mgr.borrow().create_snapshot(&ubi, &uuid, &dir);
                w.set_backup_status(SharedString::from(msg));

                let entries = mgr.borrow().list_backups(&dir);
                let items: Vec<BackupItem> = entries.into_iter().map(|e| BackupItem {
                    name: SharedString::from(e.name),
                    date: SharedString::from(e.date),
                    size: SharedString::from(e.size),
                    full_path: SharedString::from(e.full_path),
                }).collect();
                w.set_backup_items(ModelRc::new(VecModel::from(items)));
            }
        });
    }

    {
        let handle = app.as_weak();
        let mgr = backup_mgr.clone();
        app.on_refresh_backups(move || {
            if let Some(w) = handle.upgrade() {
                let dir = w.get_backup_target_dir().to_string();
                let entries = mgr.borrow().list_backups(&dir);
                let items: Vec<BackupItem> = entries.into_iter().map(|e| BackupItem {
                    name: SharedString::from(e.name),
                    date: SharedString::from(e.date),
                    size: SharedString::from(e.size),
                    full_path: SharedString::from(e.full_path),
                }).collect();
                let count = items.len();
                w.set_backup_items(ModelRc::new(VecModel::from(items)));
                w.set_backup_status(SharedString::from(format!("Backups refreshed. Found {} backup item(s).", count)));
            }
        });
    }

    {
        let handle = app.as_weak();
        let mgr = backup_mgr.clone();
        app.on_run_restore_backup(move |backup_fp| {
            if let Some(w) = handle.upgrade() {
                let ubi = w.get_backup_ubi_dir().to_string();
                let uuid = w.get_backup_target_uuid().to_string();
                let (_, msg) = mgr.borrow().restore_backup(&backup_fp, &ubi, &uuid);
                w.set_backup_status(SharedString::from(msg));
            }
        });
    }

    {
        let mgr = backup_mgr.clone();
        let handle = app.as_weak();
        app.on_toggle_auto_backup(move |enabled, idx| {
            if let Some(w) = handle.upgrade() {
                let ubi = w.get_backup_ubi_dir().to_string();
                let uuid = w.get_backup_target_uuid().to_string();
                let dir = w.get_backup_target_dir().to_string();
                mgr.borrow_mut().toggle_auto(enabled, idx, ubi, uuid, dir);
                let status = if enabled { "Periodic auto-backup active." } else { "Periodic auto-backup stopped." };
                w.set_backup_status(SharedString::from(status));
            }
        });
    }
}
