use std::path::PathBuf;
use std::fs;
use std::sync::mpsc;
use std::thread;
use chrono::{DateTime, Local};
use crate::crypto::Bl4Crypto;
use crate::state::{AppState, Mode, Status};

pub struct FileProcessor;

impl FileProcessor {
    pub fn start_processing(state: &mut AppState) {
        if let Err(e) = state.validate_current_ids() {
            state.status = Status::Error(e);
            return;
        }

        let input = PathBuf::from(&state.input_dir);
        let output = state.get_final_output_path();
        let mode = state.mode.clone();

        if mode == Mode::Encrypt {
            if let Ok(files) = Self::collect_bl4_files(&input) {
                if let Some(first_file) = files.first() {
                    if let Ok(data) = fs::read(first_file) {
                        if Bl4Crypto::is_encrypted(&data) {
                            state.status = Status::EncryptionWarning(
                                input.clone(),
                                output,
                                state.get_current_id().to_string(),
                            );
                            return;
                        }
                    }
                }
            }
        }

        let (tx, rx) = mpsc::channel();
        let anonymize = state.anonymize_guids;
        state.progress_rx = Some(rx);
        state.status = Status::Processing;

        match mode {
            Mode::Decrypt => {
                let id = state.get_current_id().to_string();
                thread::spawn(move || {
                    Self::process_decrypt(input, output, id, tx);
                });
            },
            Mode::Encrypt => {
                let id = state.get_current_id().to_string();
                thread::spawn(move || {
                    Self::process_encrypt(anonymize, input, output, id, tx);
                });
            },
            Mode::Resign => {
                let old_id = state.get_old_id().to_string();
                let new_id = state.get_new_id().to_string();
                thread::spawn(move || {
                    Self::process_resign(input, output, old_id, new_id, anonymize, tx);
                });
            },
        }
    }

    fn process_decrypt(input: PathBuf, output: PathBuf, id: String, tx: mpsc::Sender<String>) {
        let result = Self::process_decrypt_impl(input, output, id);
        match result {
            Ok(msg) => { let _ = tx.send(format!("COMPLETED: {}", msg)); },
            Err(e) => {
                if e.contains("Completed with issues") {
                    let _ = tx.send(format!("WARNING: {}", e));
                } else {
                    let _ = tx.send(format!("ERROR: {}", e));
                }
            },
        }
    }

    fn process_decrypt_impl(input: PathBuf, output: PathBuf, id: String) -> Result<String, String> {
        if !input.exists() {
            return Err("Input folder doesn't exist. Please check the path.".to_string());
        }

        if input.is_file() {
            return Err("Input must be a folder, not a file.".to_string());
        }

        let files = Self::collect_bl4_files(&input)?;
        
        if files.is_empty() {
            return Err("No Borderlands 4 save files found in input directory".to_string());
        }

        let mut processed = 0;
        let mut failed = 0;
        let mut log = String::new();
        let mut output_created = false;
        
        let mut wrong_id_count = 0;
        let mut corrupted_count = 0;
        let mut permission_errors = 0;

        for file in files {
            let name = match file.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => {
                    log.push_str("❌ Failed to get file name for a file (invalid encoding)\n");
                    failed += 1;
                    continue;
                }
            };
            
            log.push_str(&format!("Decrypting {}...\n", name));
            
            let data = match fs::read(&file) {
                Ok(d) => d,
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::PermissionDenied {
                        permission_errors += 1;
                        log.push_str(&format!("❌ Permission denied: {}\n", name));
                    } else {
                        log.push_str(&format!("❌ Failed to read {}: {}\n", name, e));
                    }
                    failed += 1;
                    continue;
                }
            };
            
            let decrypted = match Bl4Crypto::decrypt_file(&data, name, &id) {
                Ok(d) => d,
                Err(e) => {
                    if e.contains("Wrong User ID") || e.contains("Steam ID may be incorrect") {
                        wrong_id_count += 1;
                    } else if e.contains("corrupted") || e.contains("damaged") {
                        corrupted_count += 1;
                    }
                    log.push_str(&format!("❌ Failed to decrypt {}: {}\n", name, e));
                    failed += 1;
                    continue;
                }
            };
            
            let rel = match file.strip_prefix(&input) {
                Ok(r) => r,
                Err(_) => {
                    log.push_str(&format!("❌ Path error for {}\n", name));
                    failed += 1;
                    continue;
                }
            };
            
            let out = output.join(rel);
            
            if let Some(parent) = out.parent() {
                if let Err(e) = fs::create_dir_all(parent) {
                    if e.kind() == std::io::ErrorKind::PermissionDenied {
                        permission_errors += 1;
                        log.push_str(&format!("❌ No permission to create folder for {}\n", name));
                    } else {
                        log.push_str(&format!("❌ Failed to create directory for {}: {}\n", name, e));
                    }
                    failed += 1;
                    continue;
                }
            }
            output_created = true;
            
            let out_yaml = out.with_extension("yaml");
            if let Err(e) = fs::write(&out_yaml, decrypted) {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    permission_errors += 1;
                    log.push_str(&format!("❌ Permission denied writing {}\n", name));
                } else {
                    log.push_str(&format!("❌ Failed to write {}: {}\n", name, e));
                }
                failed += 1;
                continue;
            }
            
            processed += 1;
            log.push_str(&format!("✅ Successfully decrypted {}\n", name));
        }

        if output_created {
            let ts: DateTime<Local> = Local::now();
            let mut info = format!("Decryption completed at: {}\n\n", ts.format("%Y-%m-%d %H:%M:%S"));
            info.push_str(&format!("Successfully decrypted {} files for User ID: {}\n", processed, id));
            
            if failed > 0 {
                info.push_str(&format!("\n⚠️ {} files failed:\n", failed));
                if wrong_id_count > 0 {
                    info.push_str(&format!("  • {} files: Wrong User ID or corrupted\n", wrong_id_count));
                }
                if corrupted_count > 0 {
                    info.push_str(&format!("  • {} files: File corruption detected\n", corrupted_count));
                }
                if permission_errors > 0 {
                    info.push_str(&format!("  • {} files: Permission denied\n", permission_errors));
                }
            }
            
            info.push_str("\nFiles have been saved as .yaml format for readability.\n");
            info.push_str("⚠️ WARNING: Decrypted files contain original GUIDs and online preferences.\n");
            info.push_str("Share with caution as they may contain personal information.\n\n");
            info.push_str("Detailed Log:\n");
            info.push_str(&log);
            
            if let Err(e) = fs::write(output.join("DECRYPTION_INFO.txt"), info) {
                log.push_str(&format!("⚠️ Warning: Failed to write info file: {}\n", e));
            }
        }

        if processed == 0 {
            if wrong_id_count > 0 {
                Err("No files could be decrypted. Check that your User ID is correct.".to_string())
            } else if permission_errors > 0 {
                Err("No files could be decrypted. Check file and folder permissions.".to_string())
            } else {
                Err("Failed to decrypt any files. Check that files are valid Borderlands 4 saves.".to_string())
            }
        } else if failed > 0 {

            let mut error_details = format!("Completed with issues: {} successful, {} failed.\n", processed, failed);
            if wrong_id_count > 0 {
                error_details.push_str(&format!("• {} files had wrong User ID\n", wrong_id_count));
            }
            if corrupted_count > 0 {
                error_details.push_str(&format!("• {} files were corrupted\n", corrupted_count));
            }
            if permission_errors > 0 {
                error_details.push_str(&format!("• {} files had permission errors\n", permission_errors));
            }
            error_details.push_str("Check DECRYPTION_INFO.txt for details.");
            Err(error_details)
        } else {
            Ok(format!("Success! Decrypted {} files.", processed))
        }
    }

    fn process_encrypt(anonymize: bool, input: PathBuf, output: PathBuf, id: String, tx: mpsc::Sender<String>) {
        let result = Self::process_encrypt_impl(input, output, id, anonymize);
        match result {
            Ok(msg) => { let _ = tx.send(format!("COMPLETED: {}", msg)); },
            Err(e) => {
                if e.contains("Completed with issues") {
                    let _ = tx.send(format!("WARNING: {}", e));
                } else {
                    let _ = tx.send(format!("ERROR: {}", e));
                }
            },
        }
    }

    fn process_encrypt_impl(input: PathBuf, output: PathBuf, id: String, anonymize: bool) -> Result<String, String> {
        if !input.exists() {
            return Err("Input folder doesn't exist. Please check the path.".to_string());
        }

        if input.is_file() {
            return Err("Input must be a folder, not a file.".to_string());
        }

        let files = Self::collect_yaml_and_save_files(&input)?;
        
        if let Err(e) = fs::create_dir_all(&output) {
            return Err(format!("Failed to create output directory: {}", e));
        }
        
        if files.is_empty() {
            return Err("No YAML or save files found in input directory".to_string());
        }

        let mut processed = 0;
        let mut failed = 0;
        let mut log = String::new();
        
        let mut anonymize_errors = 0;
        let mut encrypt_errors = 0;
        let mut permission_errors = 0;

        for file in files {
            let name = match file.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => {
                    log.push_str("❌ Failed to get file name for a file (invalid encoding)\n");
                    failed += 1;
                    continue;
                }
            };
            
            log.push_str(&format!("Encrypting {}...\n", name));
            
            let data = match fs::read(&file) {
                Ok(d) => d,
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::PermissionDenied {
                        permission_errors += 1;
                        log.push_str(&format!("❌ Permission denied: {}\n", name));
                    } else {
                        log.push_str(&format!("❌ Failed to read {}: {}\n", name, e));
                    }
                    failed += 1;
                    continue;
                }
            };

            let data_to_encrypt = if anonymize {
                let filename_without_ext = name
                    .trim_end_matches(".yaml")
                    .trim_end_matches(".yml")
                    .trim_end_matches(".sav")
                    .trim_end_matches(".dat")
                    .trim_end_matches(".bin");
                
                match Bl4Crypto::anonymize_save_data(&data, filename_without_ext) {
                    Ok(d) => d,
                    Err(e) => {
                        anonymize_errors += 1;
                        log.push_str(&format!("❌ Failed to anonymize {}: {}\n", name, e));
                        failed += 1;
                        continue;
                    }
                }
            } else {
                data
            };
            
            let encrypted = match Bl4Crypto::encrypt_file(&data_to_encrypt, name, &id) {
                Ok(e) => e,
                Err(e) => {
                    encrypt_errors += 1;
                    log.push_str(&format!("❌ Failed to encrypt {}: {}\n", name, e));
                    failed += 1;
                    continue;
                }
            };
            
            let rel = match file.strip_prefix(&input) {
                Ok(r) => r,
                Err(_) => {
                    log.push_str(&format!("❌ Path error for {}\n", name));
                    failed += 1;
                    continue;
                }
            };
            
            let mut out = output.join(rel);
            
            if out.extension().and_then(|s| s.to_str()) == Some("yaml") 
                || out.extension().and_then(|s| s.to_str()) == Some("yml") {
                out.set_extension("sav");
            }
            
            if let Some(parent) = out.parent() {
                if let Err(e) = fs::create_dir_all(parent) {
                    if e.kind() == std::io::ErrorKind::PermissionDenied {
                        permission_errors += 1;
                        log.push_str(&format!("❌ No permission to create folder for {}\n", name));
                    } else {
                        log.push_str(&format!("❌ Failed to create directory for {}: {}\n", name, e));
                    }
                    failed += 1;
                    continue;
                }
            }
            
            if let Err(e) = fs::write(&out, encrypted) {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    permission_errors += 1;
                    log.push_str(&format!("❌ Permission denied writing {}\n", name));
                } else {
                    log.push_str(&format!("❌ Failed to write {}: {}\n", name, e));
                }
                failed += 1;
                continue;
            }
            
            processed += 1;
            log.push_str(&format!("✅ Successfully encrypted {}\n", name));
        }

        let ts: DateTime<Local> = Local::now();
        let mut info = format!("Encryption completed at: {}\n\n", ts.format("%Y-%m-%d %H:%M:%S"));
        info.push_str(&format!("Successfully encrypted {} files for User ID: {}\n", processed, id));
        
        if failed > 0 {
            info.push_str(&format!("\n⚠️ {} files failed:\n", failed));
            if anonymize_errors > 0 {
                info.push_str(&format!("  • {} files: Anonymization failed\n", anonymize_errors));
            }
            if encrypt_errors > 0 {
                info.push_str(&format!("  • {} files: Encryption failed\n", encrypt_errors));
            }
            if permission_errors > 0 {
                info.push_str(&format!("  • {} files: Permission denied\n", permission_errors));
            }
        }
        
        if anonymize {
            info.push_str("\nGUIDs and online preferences have been anonymized for privacy.\n\n");
        } else {
            info.push_str("\nGUIDs and online preferences weren't anonymized. Share with caution.\n\n");
        }
        
        info.push_str("Detailed Log:\n");
        info.push_str(&log);
        
        if let Err(e) = fs::write(output.join("ENCRYPTION_INFO.txt"), info) {
            log.push_str(&format!("⚠️ Warning: Failed to write info file: {}\n", e));
        }

        if processed == 0 {
            if permission_errors > 0 {
                Err("No files could be encrypted. Check file and folder permissions.".to_string())
            } else {
                Err("Failed to encrypt any files. Check that files are valid YAML or save files.".to_string())
            }
        } else if failed > 0 {
            let mut error_details = format!("Completed with issues: {} successful, {} failed.\n", processed, failed);
            if anonymize_errors > 0 {
                error_details.push_str(&format!("• {} files had anonymization errors\n", anonymize_errors));
            }
            if encrypt_errors > 0 {
                error_details.push_str(&format!("• {} files had encryption errors\n", encrypt_errors));
            }
            if permission_errors > 0 {
                error_details.push_str(&format!("• {} files had permission errors\n", permission_errors));
            }
            error_details.push_str("Check ENCRYPTION_INFO.txt for details.");
            Err(error_details)
        } else {
            Ok(format!("Success! Encrypted {} files.", processed))
        }
    }

    fn process_resign(input: PathBuf, output: PathBuf, old_id: String, new_id: String, anonymize: bool, tx: mpsc::Sender<String>) {
        let result = Self::process_resign_impl(input, output, old_id, new_id, anonymize);
        match result {
            Ok(msg) => { let _ = tx.send(format!("COMPLETED: {}", msg)); },
            Err(e) => {
                if e.contains("Completed with issues") {
                    let _ = tx.send(format!("WARNING: {}", e));
                } else {
                    let _ = tx.send(format!("ERROR: {}", e));
                }
            },
        }
    }

    fn process_resign_impl(input: PathBuf, output: PathBuf, old_id: String, new_id: String, anonymize: bool) -> Result<String, String> {
        if !input.exists() {
            return Err("Input folder doesn't exist. Please check the path.".to_string());
        }

        if input.is_file() {
            return Err("Input must be a folder, not a file.".to_string());
        }

        let files = Self::collect_bl4_files(&input)?;
        
        if let Err(e) = fs::create_dir_all(&output) {
            return Err(format!("Failed to create output directory: {}", e));
        }
        
        if files.is_empty() {
            return Err("No Borderlands 4 save files found in input directory".to_string());
        }

        let mut processed = 0;
        let mut skipped = 0;
        let mut failed = 0;
        let mut log = String::new();

        let mut wrong_id_count = 0;
        let mut corrupted_count = 0;
        let mut permission_errors = 0;

        for file in files {
            let name = match file.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => {
                    log.push_str("❌ Failed to get file name for a file (invalid encoding)\n");
                    failed += 1;
                    continue;
                }
            };
            
            log.push_str(&format!("Resigning {}...\n", name));
            
            let data = match fs::read(&file) {
                Ok(d) => d,
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::PermissionDenied {
                        permission_errors += 1;
                        log.push_str(&format!("❌ Permission denied: {}\n", name));
                    } else {
                        log.push_str(&format!("❌ Failed to read {}: {}\n", name, e));
                    }
                    failed += 1;
                    continue;
                }
            };

            if !name.to_lowercase().ends_with(".yaml") && !name.to_lowercase().ends_with(".yml") {
                if !Bl4Crypto::is_encrypted(&data) {
                    log.push_str(&format!("⚠️ Skipping already un-encrypted file: {}\n", name));
                    skipped += 1;
                    continue;
                }
            }
            
            let resigned = match Bl4Crypto::resign_file(&data, name, &old_id, &new_id) {
                Ok(r) => r,
                Err(e) => {
                    if e.contains("Wrong User ID") || e.contains("Steam ID may be incorrect") {
                        wrong_id_count += 1;
                    } else if e.contains("corrupted") || e.contains("damaged") {
                        corrupted_count += 1;
                    }
                    log.push_str(&format!("❌ Failed to resign {}: {}\n", name, e));
                    failed += 1;
                    continue;
                }
            };
            
            let rel = match file.strip_prefix(&input) {
                Ok(r) => r,
                Err(_) => {
                    log.push_str(&format!("❌ Path error for {}\n", name));
                    failed += 1;
                    continue;
                }
            };
            
            let out = output.join(rel);
            
            if let Some(parent) = out.parent() {
                if let Err(e) = fs::create_dir_all(parent) {
                    if e.kind() == std::io::ErrorKind::PermissionDenied {
                        permission_errors += 1;
                        log.push_str(&format!("❌ No permission to create folder for {}\n", name));
                    } else {
                        log.push_str(&format!("❌ Failed to create directory for {}: {}\n", name, e));
                    }
                    failed += 1;
                    continue;
                }
            }
            
            if let Err(e) = fs::write(&out, resigned) {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    permission_errors += 1;
                    log.push_str(&format!("❌ Permission denied writing {}\n", name));
                } else {
                    log.push_str(&format!("❌ Failed to write resigned file {}: {}\n", name, e));
                }
                failed += 1;
                continue;
            }
            
            processed += 1;
            log.push_str(&format!("✅ Successfully resigned {}\n", name));
        }

        let ts: DateTime<Local> = Local::now();
        let mut info = format!("Resigning completed at: {}\n\n", ts.format("%Y-%m-%d %H:%M:%S"));
        info.push_str(&format!("Successfully resigned {} files from User ID {} to User ID {}\n", processed, old_id, new_id));
        if skipped > 0 {
            info.push_str(&format!("Skipped {} files (already unencrypted)\n", skipped));
        }
        if failed > 0 {
            info.push_str(&format!("\n⚠️ {} files failed:\n", failed));
            if wrong_id_count > 0 {
                info.push_str(&format!("  • {} files: Wrong User ID or corrupted\n", wrong_id_count));
            }
            if corrupted_count > 0 {
                info.push_str(&format!("  • {} files: File corruption detected\n", corrupted_count));
            }
            if permission_errors > 0 {
                info.push_str(&format!("  • {} files: Permission denied\n", permission_errors));
            }
        }
        if anonymize {
            info.push_str("\nGUIDs and online preferences have been anonymized for privacy.\n\n");
        } else {
            info.push_str("\nGUIDs and online preferences weren't anonymized. Share with caution.\n\n");
        }
        info.push_str("Detailed Log:\n");
        info.push_str(&log);
        
        if let Err(e) = fs::write(output.join("RESIGNING_INFO.txt"), info) {
            log.push_str(&format!("⚠️ Warning: Failed to write info file: {}\n", e));
        }

        if processed == 0 {
            if wrong_id_count > 0 {
                Err("No files could be resigned. Check that your User IDs are correct.".to_string())
            } else if permission_errors > 0 {
                Err("No files could be resigned. Check file and folder permissions.".to_string())
            } else {
                Err(format!("Failed to resign any files. {} errors, {} skipped. Check logs.", failed, skipped))
            }
        } else if failed > 0 {
            let mut error_details = format!("Completed with issues: {} successful, {} failed, {} skipped.\n", processed, failed, skipped);
            if wrong_id_count > 0 {
                error_details.push_str(&format!("• {} files had wrong User ID\n", wrong_id_count));
            }
            if corrupted_count > 0 {
                error_details.push_str(&format!("• {} files were corrupted\n", corrupted_count));
            }
            if permission_errors > 0 {
                error_details.push_str(&format!("• {} files had permission errors\n", permission_errors));
            }
            error_details.push_str("Check RESIGNING_INFO.txt for details.");
            Err(error_details)
        } else {
            Ok(format!("Success! Resigned {} files, {} skipped.", processed, skipped))
        }
    }

    fn collect_bl4_files(path: &PathBuf) -> Result<Vec<PathBuf>, String> {
        let mut files = Vec::new();
        Self::walk_directory(path, &mut files, Self::is_bl4_save_file)?;
        if files.is_empty() {
            Err("No Borderlands 4 save files (.sav) found in the directory".to_string())
        } else {
            Ok(files)
        }
    }

    fn collect_yaml_and_save_files(path: &PathBuf) -> Result<Vec<PathBuf>, String> {
        let mut files = Vec::new();
        Self::walk_directory(path, &mut files, |p| {
            Self::is_bl4_save_file(p) || Self::is_yaml_file(p)
        })?;
        if files.is_empty() {
            Err("No YAML or save files found in the directory".to_string())
        } else {
            Ok(files)
        }
    }

    fn walk_directory<F>(path: &PathBuf, files: &mut Vec<PathBuf>, predicate: F) -> Result<(), String>
    where
        F: Fn(&PathBuf) -> bool + Copy,
    {
        if path.is_file() {
            if predicate(path) {
                files.push(path.clone());
            }
        } else if path.is_dir() {
            let entries = fs::read_dir(path)
                .map_err(|e| format!("Failed to read directory {}: {}", path.display(), e))?;
            for entry in entries {
                let entry = entry
                    .map_err(|e| format!("Failed to read directory entry: {}", e))?;
                let entry_path = entry.path();
                if entry_path.is_file() {
                    if predicate(&entry_path) {
                        files.push(entry_path);
                    }
                } else if entry_path.is_dir() {
                    Self::walk_directory(&entry_path, files, predicate)?;
                }
            }
        }
        Ok(())
    }

    fn is_bl4_save_file(path: &PathBuf) -> bool {
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            let lower = name.to_lowercase();
            return lower.ends_with(".sav") || 
                   lower.ends_with(".dat") || 
                   lower.ends_with(".bin") ||
                   lower.ends_with(".profile") ||
                   lower.ends_with(".details");
        }
        false
    }

    fn is_yaml_file(path: &PathBuf) -> bool {
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            let lower = name.to_lowercase();
            return lower.ends_with(".yaml") || lower.ends_with(".yml");
        }
        false
    }

    pub fn update_progress(state: &mut AppState) {
        if let Some(rx) = &state.progress_rx {
            if let Ok(msg) = rx.try_recv() {
                if msg.starts_with("COMPLETED:") {
                    state.status = Status::Completed(msg[10..].to_string());
                    state.progress_rx = None;
                } else if msg.starts_with("WARNING:") {
                    state.status = Status::Warning(msg[8..].to_string());
                    state.progress_rx = None;
                } else if msg.starts_with("ERROR:") {
                    state.status = Status::Error(msg[6..].to_string());
                    state.progress_rx = None;
                }
            }
        }
    }

    pub fn force_encrypt(state: &mut AppState) {
        if let Status::EncryptionWarning(input, output, id) = &state.status.clone() {
            let (tx, rx) = mpsc::channel();
            let anonymize = state.anonymize_guids;
            state.progress_rx = Some(rx);
            state.status = Status::Processing;

            let input = input.clone();
            let output = output.clone();
            let id = id.clone();
            
            thread::spawn(move || {
                Self::process_encrypt(anonymize, input, output, id, tx);
            });
        }
    }
}