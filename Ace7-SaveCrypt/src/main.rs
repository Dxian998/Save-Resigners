use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

use ace7_savecrypt::container::EcsdSave;
use ace7_savecrypt::crypto::Crypto;
use ace7_savecrypt::finder::KeyFinder;


#[derive(Parser)]
#[command(
    name = "ace7-savetoolkit",
    author = "PRS",
    version = "1.0.0",
    about = "Ace Combat 7: Skies Unknown - Save Decryptor, Resigner, Encryptor & Key Finder Toolkit"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

fn parse_steam_id(s: &str) -> Result<u64, String> {
    let trimmed = s.trim();
    if let Some(hex_str) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
        u64::from_str_radix(hex_str, 16).map_err(|e| format!("Invalid hex SteamID '{}': {}", s, e))
    } else {
        trimmed.parse::<u64>().map_err(|e| format!("Invalid SteamID '{}': {}", s, e))
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Inspect an encrypted ECSD save file and verify checksums
    Inspect {
        /// Path to the save file (e.g. ACE7Save.sav)
        input: PathBuf,
    },

    /// Brute-force or recover the SteamID / AccountID and AES key from an encrypted save
    Identify {
        /// Path to the encrypted save file
        input: PathBuf,
    },

    /// Decrypt an ECSD save file to raw Unreal Engine 4 GVAS format
    Decrypt {
        /// Path to input encrypted save file
        input: PathBuf,

        /// Path to output decrypted file
        output: PathBuf,

        /// SteamID64 or 32-bit AccountID in decimal or 0x hex (auto-detected if omitted)
        #[arg(short, long, value_parser = parse_steam_id)]
        steamid: Option<u64>,

        /// Use the offline fallback key
        #[arg(long)]
        offline: bool,
    },

    /// Encrypt a raw UE4 GVAS save file into a valid ECSD container
    Encrypt {
        /// Path to input unencrypted GVAS save file
        input: PathBuf,

        /// Path to output encrypted save file
        output: PathBuf,

        /// Target SteamID64 or 32-bit AccountID in decimal or 0x hex
        #[arg(short, long, value_parser = parse_steam_id)]
        steamid: Option<u64>,

        /// Encrypt with the offline fallback key
        #[arg(long)]
        offline: bool,
    },

    /// Resign an encrypted save from one Steam account to another
    Resign {
        /// Path to input encrypted save file
        input: PathBuf,

        /// Path to output resigned save file
        output: PathBuf,

        /// Target SteamID64 or 32-bit AccountID in decimal or 0x hex
        #[arg(short, long, value_parser = parse_steam_id)]
        target_steamid: Option<u64>,

        /// Source SteamID64 in decimal or 0x hex (auto-detected if omitted)
        #[arg(long, value_parser = parse_steam_id)]
        source_steamid: Option<u64>,

        /// Resign to the offline fallback key
        #[arg(long)]
        offline: bool,
    },
}

fn resolve_key_or_find(
    save: &EcsdSave,
    steamid: Option<u64>,
    offline: bool,
) -> Result<[u8; 32]> {
    if offline {
        println!("[*] Using offline fallback key.");
        return Ok(Crypto::offline_key());
    }

    if let Some(id) = steamid {
        let key = Crypto::derive_key(id);
        println!(
            "[*] Using provided SteamID: {} -> Key: \"{}\"",
            id,
            std::str::from_utf8(&key).unwrap_or("???")
        );
        return Ok(key);
    }

    println!("[*] No SteamID provided. Running automated key finder...");
    let block0: [u8; 16] = save.ciphertext[..16].try_into()?;
    if let Some(found) = KeyFinder::find_key(&block0, true) {
        if found.is_offline {
            println!("[+] Save was encrypted with OFFLINE FALLBACK KEY!");
        } else {
            println!("\n========================================================");
            println!("[+] SOURCE KEY IDENTIFIED in {:.2?}!", found.elapsed);
            println!("[+] AccountID:  {} (0x{:08x})", found.account_id, found.account_id);
            println!("[+] SteamID64:  {}", found.steamid64);
            println!(
                "[+] AES-256 Key: \"{}\"",
                std::str::from_utf8(&found.key).unwrap_or("???")
            );
            println!("========================================================\n");
        }
        Ok(found.key)
    } else {
        bail!("Failed to automatically identify save key. Please specify --steamid manually.");
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Inspect { input } => {
            println!("[*] Inspecting file: {:?}", input);
            let raw = fs::read(&input).with_context(|| format!("Failed to read {:?}", input))?;
            let save = EcsdSave::parse(&raw)?;

            println!("\n================ Container Header ================");
            println!("Magic:              ECSD");
            println!("Rolling XOR sum:    0x{:02X}", save.rolling_checksum);
            println!("Plaintext size:     {} bytes", save.unencrypted_len);
            println!(
                "Ciphertext size:    {} bytes ({} blocks)",
                save.ciphertext.len(),
                save.ciphertext.len() / 16
            );
            println!("CRC-32 trailer:     0x{:08X}", save.crc32);

            let computed_crc = Crypto::crc32(&raw[..raw.len() - 4]);
            let computed_xor = Crypto::compute_rolling_checksum(&raw[5..raw.len() - 4]);

            println!("\n================ Integrity Checks ================");
            println!("CRC-32:             0x{:08X} (Computed: 0x{:08X}) -> {}", save.crc32, computed_crc, if computed_crc == save.crc32 { "VALID [OK]" } else { "MISMATCH [FAIL]" });
            println!("Rolling XOR:        0x{:02X}       (Computed: 0x{:02X})       -> {}", save.rolling_checksum, computed_xor, if computed_xor == save.rolling_checksum { "VALID [OK]" } else { "MISMATCH [FAIL]" });
            match save.verify_integrity(&raw) {
                Ok(()) => println!("Overall Status:     ALL CHECKS PASSED [OK]"),
                Err(e) => println!("Overall Status:     FAILED: {}", e),
            }
            println!("==================================================\n");
        }

        Commands::Identify { input } => {
            println!("[*] Loading save file: {:?}", input);
            let raw = fs::read(&input).with_context(|| format!("Failed to read {:?}", input))?;
            let save = EcsdSave::parse(&raw)?;

            let block0: [u8; 16] = save.ciphertext[..16].try_into()?;
            if let Some(found) = KeyFinder::find_key(&block0, true) {
                if found.is_offline {
                    println!("\n[+] Save was encrypted with the OFFLINE FALLBACK KEY!");
                    println!(
                        "[+] Key: \"{}\"",
                        std::str::from_utf8(&found.key).unwrap_or("???")
                    );
                } else {
                    println!("\n========================================================");
                    println!("[+] KEY FOUND in {:.2?}!", found.elapsed);
                    println!("[+] AccountID:  {} (0x{:08x})", found.account_id, found.account_id);
                    println!("[+] SteamID64:  {}", found.steamid64);
                    println!(
                        "[+] AES-256 Key: \"{}\"",
                        std::str::from_utf8(&found.key).unwrap_or("???")
                    );
                    println!("========================================================\n");
                }
            } else {
                println!("[-] Search finished across all patterns. No matching key found.");
            }
        }

        Commands::Decrypt {
            input,
            output,
            steamid,
            offline,
        } => {
            println!("[*] Reading input save: {:?}", input);
            let raw = fs::read(&input).with_context(|| format!("Failed to read {:?}", input))?;
            let save = EcsdSave::parse(&raw)?;
            let key = resolve_key_or_find(&save, steamid, offline)?;

            println!("[*] Decrypting payload...");
            let decrypted = save.decrypt(&key)?;
            fs::write(&output, &decrypted)
                .with_context(|| format!("Failed to write {:?}", output))?;

            println!(
                "[+] Successfully decrypted {} bytes to: {:?}",
                decrypted.len(),
                output
            );
        }

        Commands::Encrypt {
            input,
            output,
            steamid,
            offline,
        } => {
            if !offline && steamid.is_none() {
                bail!("Must provide --steamid or specify --offline for encryption.");
            }

            let key = if offline {
                println!("[*] Encrypting with offline fallback key.");
                Crypto::offline_key()
            } else {
                let id = steamid.unwrap();
                let k = Crypto::derive_key(id);
                println!(
                    "[*] Encrypting for SteamID: {} -> Key: \"{}\"",
                    id,
                    std::str::from_utf8(&k).unwrap_or("???")
                );
                k
            };

            let gvas_payload = fs::read(&input)
                .with_context(|| format!("Failed to read {:?}", input))?;

            println!("[*] Packing into ECSD container...");
            let container_bytes = EcsdSave::pack(&gvas_payload, &key);
            fs::write(&output, &container_bytes)
                .with_context(|| format!("Failed to write {:?}", output))?;

            println!(
                "[+] Successfully encrypted {} bytes to: {:?}",
                container_bytes.len(),
                output
            );
        }

        Commands::Resign {
            input,
            output,
            target_steamid,
            source_steamid,
            offline,
        } => {
            if !offline && target_steamid.is_none() {
                bail!("Must provide --target-steamid or specify --offline for resigning.");
            }

            let target_key = if offline {
                Crypto::offline_key()
            } else {
                Crypto::derive_key(target_steamid.unwrap())
            };

            println!("[*] Reading input save: {:?}", input);
            let raw = fs::read(&input).with_context(|| format!("Failed to read {:?}", input))?;
            let save = EcsdSave::parse(&raw)?;

            let source_key = resolve_key_or_find(&save, source_steamid, false)?;

            println!("[*] Decrypting source save...");
            let decrypted = save.decrypt(&source_key)?;

            println!(
                "[*] Re-encrypting for target key (\"{}\")...",
                std::str::from_utf8(&target_key).unwrap_or("???")
            );
            let resigned_bytes = EcsdSave::pack(&decrypted, &target_key);
            fs::write(&output, &resigned_bytes)
                .with_context(|| format!("Failed to write {:?}", output))?;

            println!(
                "[+] Successfully resigned save ({} bytes) to: {:?}",
                resigned_bytes.len(),
                output
            );
        }
    }

    Ok(())
}
