use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use aes::Aes256;
use cipher::{BlockCipherDecrypt, KeyInit};

use crate::crypto::{Crypto, GVAS_MAGIC};

pub struct KeyFinder;

pub struct FoundKey {
    pub account_id: u32,
    pub steamid64: u64,
    pub key: [u8; 32],
    pub elapsed: Duration,
    pub is_offline: bool,
}

impl KeyFinder {
    pub fn find_key(block0: &[u8; 16], verbose: bool) -> Option<FoundKey> {
        let start = Instant::now();

        let offline_key = Crypto::offline_key();
        let cipher = Aes256::new((&offline_key).into());
        let mut test_blk = *block0;
        cipher.decrypt_block((&mut test_blk).into());
        let magic = u64::from_le_bytes(test_blk[..8].try_into().unwrap());
        if magic == GVAS_MAGIC {
            return Some(FoundKey {
                account_id: 0,
                steamid64: 0,
                key: offline_key,
                elapsed: start.elapsed(),
                is_offline: true,
            });
        }

        let prefix1: [u8; 24] = *muddy::muddy!("M$KW=w4,knvovE8u00000000").as_bytes().first_chunk::<24>().unwrap();
        let prefix2: [u8; 24] = *muddy::muddy!("M$KW=w4,knvovE8u01100001").as_bytes().first_chunk::<24>().unwrap();

        let prefixes = [
            (
                prefix1,
                "MSVC %016lx (32-bit AccountID with 8 leading zeros)",
            ),
            (
                prefix2,
                "Standard 64-bit SteamID64 (0x01100001...)",
            ),
        ];

        let available = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        let num_workers = available.saturating_sub(1).max(1);

        for (prefix, desc) in prefixes {
            if verbose {
                println!(
                    "[*] Searching space: {} (\"{}\") using {} threads...",
                    desc,
                    std::str::from_utf8(&prefix).unwrap_or("???"),
                    num_workers
                );
            }

            let found = Arc::new(AtomicBool::new(false));
            let next_chunk = Arc::new(AtomicU32::new(0));
            let result = Arc::new(std::sync::Mutex::new(None));

            let reporter = if verbose {
                let found_clone = Arc::clone(&found);
                let next_chunk_clone = Arc::clone(&next_chunk);
                Some(std::thread::spawn(move || {
                    let rep_start = Instant::now();
                    while !found_clone.load(Ordering::Relaxed) {
                        std::thread::sleep(Duration::from_millis(500));
                        let done = next_chunk_clone.load(Ordering::Relaxed) as u64;
                        let elapsed = rep_start.elapsed().as_secs_f64();
                        let speed = if elapsed > 0.0 {
                            done as f64 / elapsed / 1_000_000.0
                        } else {
                            0.0
                        };
                        print!(
                            "\r[~] Checked: {:>10} / {:>10} ({:5.2}%) | Speed: {:6.2} M keys/sec",
                            done.min(u32::MAX as u64),
                            u32::MAX,
                            (done as f64 / u32::MAX as f64) * 100.0,
                            speed
                        );
                        let _ = std::io::Write::flush(&mut std::io::stdout());
                    }
                }))
            } else {
                None
            };

            const CHUNK_SIZE: u32 = 32768;
            let mut handles = Vec::with_capacity(num_workers);

            for _ in 0..num_workers {
                let found = Arc::clone(&found);
                let next_chunk = Arc::clone(&next_chunk);
                let result = Arc::clone(&result);
                let prefix = prefix;
                let block0 = *block0;

                let handle = std::thread::spawn(move || {
                    let mut key_buf = [0u8; 32];
                    loop {
                        if found.load(Ordering::Relaxed) {
                            break;
                        }

                        let start = next_chunk.fetch_add(CHUNK_SIZE, Ordering::Relaxed);
                        let end = start.saturating_add(CHUNK_SIZE);
                        if start >= u32::MAX || start == end {
                            break;
                        }

                        for candidate_id in start..end {
                            Crypto::format_key_fast(&prefix, candidate_id, &mut key_buf);

                            let cipher = Aes256::new((&key_buf).into());
                            let mut blk = block0;
                            cipher.decrypt_block((&mut blk).into());

                            let candidate_magic = u64::from_le_bytes(blk[..8].try_into().unwrap());
                            if candidate_magic == GVAS_MAGIC {
                                found.store(true, Ordering::Relaxed);
                                let mut lock = result.lock().unwrap();
                                *lock = Some((candidate_id, key_buf));
                                return;
                            }
                        }
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                let _ = handle.join();
            }

            found.store(true, Ordering::Relaxed);
            if let Some(rep) = reporter {
                let _ = rep.join();
                println!();
            }

            let found_res = result.lock().unwrap().take();
            if let Some((acc_id, key_bytes)) = found_res {
                let steamid64 = 0x0110000100000000u64 | (acc_id as u64);
                return Some(FoundKey {
                    account_id: acc_id,
                    steamid64,
                    key: key_bytes,
                    elapsed: start.elapsed(),
                    is_offline: false,
                });
            }
        }

        None
    }
}
