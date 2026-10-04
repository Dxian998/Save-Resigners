use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};
use wdl_savecrypt::{validate_uuid, WdlSave};

#[derive(Parser)]
#[command(name = "wdl-savecrypt")]
#[command(author = "Dxian998")]
#[command(version = "0.1.0")]
#[command(about = "WDL Save Decryptor, Signer & Inspector", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Inspect a .save file (dump container header, stamp, PRNG seed, and signature)
    Inspect {
        /// Path to the .save file
        file: PathBuf,
    },

    /// Verify if a .save file is signed for a specific Ubisoft Account UUID
    Verify {
        /// Path to the .save file
        file: PathBuf,
        /// Target Ubisoft Account UUID (36 chars: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx)
        #[arg(short, long)]
        uuid: String,
    },

    /// Decrypt a .save file to raw Disrupt Engine binary payload
    Decrypt {
        /// Path to the encrypted .save file
        input: PathBuf,
        /// Path to output the raw decrypted payload
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Encrypt a raw binary payload back into a valid .save container
    Encrypt {
        /// Path to the raw binary payload
        input: PathBuf,
        /// Target Ubisoft Account UUID
        #[arg(short, long)]
        uuid: String,
        /// Path to output the encrypted .save file
        #[arg(short, long)]
        output: PathBuf,
        /// Optional 32-character ASCII hex stamp (defaults to dummy stamp)
        #[arg(short, long)]
        stamp: Option<String>,
    },

    /// Re-sign a .save file or directory of .save files to a new Ubisoft Account UUID
    Resign {
        /// Path to the source .save file or directory
        input: PathBuf,
        /// Target Ubisoft Account UUID
        #[arg(short, long)]
        uuid: String,
        /// Destination file or directory (defaults to in-place replacement with .bak backup)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Inspect { file } => {
            let save = WdlSave::from_file(&file)?;
            println!("WDL Save Info");
            println!("File:         {:?}", file);
            println!("Stamp:        {}", save.stamp_str());
            println!("Payload Size: {} bytes", save.payload.len());
            println!("Seed:         0x{:08X}", save.seed);
            println!("Tag:          0x{:02X}", save.tag);
            println!("Signature:    {}", hex_str(&save.signature));
        }

        Commands::Verify { file, uuid } => {
            let save = WdlSave::from_file(&file)?;
            let valid = save.verify_uuid(&uuid)?;
            println!("File: {:?}", file);
            println!("UUID: {}", uuid.trim());
            if valid {
                println!("Result: MATCH (Valid signature)");
            } else {
                println!("Result: MISMATCH (Signature is for another account)");
            }
        }

        Commands::Decrypt { input, output } => {
            let save = WdlSave::from_file(&input)?;
            fs::write(&output, &save.payload)
                .with_context(|| format!("Failed to write decrypted data to {:?}", output))?;
            println!("Decrypted {:?} -> {:?}", input, output);
            println!("Wrote {} bytes of raw payload.", save.payload.len());
        }

        Commands::Encrypt {
            input,
            uuid,
            output,
            stamp,
        } => {
            let raw_data = fs::read(&input)
                .with_context(|| format!("Failed to read raw input {:?}", input))?;

            let mut stamp_bytes = [0u8; 32];
            if let Some(s) = stamp {
                let clean = s.trim();
                if clean.len() != 32 {
                    bail!("Stamp must be exactly 32 characters, got {}", clean.len());
                }
                stamp_bytes.copy_from_slice(clean.as_bytes());
            } else {
                stamp_bytes.copy_from_slice(b"d3de88dca8c2f28c68a025f662e42f05");
            }

            let seed = 0x12345678u32;
            let save = WdlSave {
                stamp: stamp_bytes,
                payload: raw_data,
                signature: [0u8; 32],
                seed,
                tag: 0,
            };

            save.save_to_file(&output, &uuid)?;
            println!("Encrypted {:?} -> {:?}", input, output);
        }

        Commands::Resign {
            input,
            uuid,
            output,
        } => {
            resign_path(&input, &uuid, output.as_deref())?;
        }
    }

    Ok(())
}

fn resign_path(input: &Path, uuid: &str, output: Option<&Path>) -> Result<()> {
    let clean_uuid = validate_uuid(uuid)?;
    let saves = collect_saves(input)?;
    let is_batch = saves.len() > 1 || input.is_dir();

    if let Some(out) = output {
        if is_batch {
            fs::create_dir_all(out)?;
        }
    }

    let mut count = 0;
    for src in &saves {
        let dest = match output {
            Some(out) if is_batch || out.is_dir() => {
                let name = src.file_name().context("Invalid filename")?;
                out.join(name)
            }
            Some(out) => {
                if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
                    let _ = fs::create_dir_all(parent);
                }
                out.to_path_buf()
            }
            None => {
                let _ = fs::copy(src, src.with_extension("save.bak"));
                src.to_path_buf()
            }
        };

        match WdlSave::from_file(src).and_then(|s| s.save_to_file(&dest, &clean_uuid)) {
            Ok(_) => {
                println!("Re-signed: {:?} -> {:?}", src, dest);
                count += 1;
            }
            Err(e) if is_batch => eprintln!("Skipping {:?}: {}", src, e),
            Err(e) => return Err(e),
        }
    }

    println!("Finished: {} save(s) re-signed to UUID {}", count, clean_uuid);
    Ok(())
}

fn hex_str(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn collect_saves(input: &Path) -> Result<Vec<PathBuf>> {
    if input.is_file() {
        return Ok(vec![input.to_path_buf()]);
    }
    if input.is_dir() {
        let saves = fs::read_dir(input)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                name.ends_with(".save") && !name.ends_with("b.save") && !name.ends_with(".upload")
            })
            .collect();
        return Ok(saves);
    }
    bail!("Path not found: {:?}", input)
}