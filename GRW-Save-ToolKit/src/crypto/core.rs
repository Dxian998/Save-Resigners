use anyhow::{bail, Result};
use sha2::{Digest, Sha256};
use super::rng::GrwRng;

pub const HEADER_SIZE: usize = 552;
pub const TEA_DELTA: u32 = 2654435769; // 0x9E3779B9
pub const SEED_OBFUSCATION: [u8; 4] = [68, 237, 124, 177]; // [0x44, 0xED, 0x7C, 0xB1]
pub const STEAM_TOKEN_ODD: &[u8; 32] = b"2cc22de8944a492722b727e7ab72ed35";
pub const STEAM_TOKEN_EVEN: &[u8; 32] = b"88c98aa342db87c13901ff44ec0b8e22";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrwSavePlatform {
    Ubisoft,
    Steam,
}

impl std::fmt::Display for GrwSavePlatform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GrwSavePlatform::Ubisoft => write!(f, "Ubisoft"),
            GrwSavePlatform::Steam => write!(f, "Steam"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DecryptResult {
    pub plaintext: Vec<u8>,
    pub seed1: u32,
    pub seed2: u32,
    pub seed3: u32,
    pub shuffle_padding_bytes: Vec<u8>,
    pub tea_padding_bytes: Vec<u8>,
    pub embedded_digest: [u8; 32],
}

#[derive(Debug, Clone)]
struct StageSeed {
    data: Vec<u8>,
    seed: u32,
}

#[derive(Debug, Clone)]
struct PadShuffleResult {
    stage1: Vec<u8>,
    seed: u32,
    padding_bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
struct HashTeaResult {
    stage2: Vec<u8>,
    tea_padding_bytes: Vec<u8>,
    embedded_digest: [u8; 32],
}

pub fn normalize_account_id(account_id: &str) -> Result<String> {
    let text = account_id.trim().to_ascii_lowercase();
    let parts: Vec<&str> = text.split('-').collect();
    if parts.len() != 5 || parts[0].len() != 8 || parts[1].len() != 4 || parts[2].len() != 4 || parts[3].len() != 4 || parts[4].len() != 12 || !parts.iter().all(|p| p.chars().all(|c| c.is_ascii_hexdigit())) {
        bail!("Enter a valid 36-character Ubisoft user ID (UUID format, e.g. xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx).");
    }
    Ok(text)
}

fn is_ascii_hex(b: u8) -> bool {
    b.is_ascii_hexdigit()
}

pub fn detect_save_platform(full_save: &[u8]) -> Result<GrwSavePlatform> {
    if full_save.len() <= HEADER_SIZE {
        bail!("The save file is invalid (too short).");
    }
    let mut is_ubi = true;
    let mut is_steam = true;
    for &b in &full_save[8..40] {
        if b != 0 { is_ubi = false; }
        if !is_ascii_hex(b) { is_steam = false; }
    }

    if is_ubi {
        Ok(GrwSavePlatform::Ubisoft)
    } else if is_steam {
        Ok(GrwSavePlatform::Steam)
    } else {
        bail!("The save platform header is not recognized as Ubisoft or Steam.");
    }
}

pub fn extract_header(full_save: &[u8]) -> Result<Vec<u8>> {
    if full_save.len() <= HEADER_SIZE {
        bail!("The save file is invalid.");
    }
    Ok(full_save[..HEADER_SIZE].to_vec())
}

pub fn compute_digest(stage2: &[u8], account_id: &str) -> Result<[u8; 32]> {
    let normalized = normalize_account_id(account_id)?;
    let mut hasher = Sha256::new();
    hasher.update(stage2);
    hasher.update(normalized.as_bytes());
    let result = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    Ok(out)
}

pub fn tea_encrypt_zero_key(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() % 8 != 0 {
        bail!("Invalid TEA input length.");
    }
    let mut out = data.to_vec();
    for chunk in out.chunks_exact_mut(8) {
        let mut v0 = u32::from_le_bytes(chunk[0..4].try_into().unwrap());
        let mut v1 = u32::from_le_bytes(chunk[4..8].try_into().unwrap());
        let mut sum = 0u32;
        for _ in 0..32 {
            sum = sum.wrapping_add(TEA_DELTA);
            v0 = v0.wrapping_add(((v1 << 4) ^ (v1 >> 5)) ^ v1.wrapping_add(sum));
            v1 = v1.wrapping_add(((v0 << 4) ^ (v0 >> 5)) ^ v0.wrapping_add(sum));
        }
        chunk[0..4].copy_from_slice(&v0.to_le_bytes());
        chunk[4..8].copy_from_slice(&v1.to_le_bytes());
    }
    Ok(out)
}

pub fn tea_decrypt_zero_key(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() % 8 != 0 { bail!("Invalid TEA input length."); }

    let mut out = data.to_vec();
    for chunk in out.chunks_exact_mut(8) {
        let mut v0 = u32::from_le_bytes(chunk[0..4].try_into().unwrap());
        let mut v1 = u32::from_le_bytes(chunk[4..8].try_into().unwrap());
        let mut sum = 3337565984u32; // 32 * TEA_DELTA
        for _ in 0..32 {
            v1 = v1.wrapping_sub(((v0 << 4) ^ (v0 >> 5)) ^ v0.wrapping_add(sum));
            v0 = v0.wrapping_sub(((v1 << 4) ^ (v1 >> 5)) ^ v1.wrapping_add(sum));
            sum = sum.wrapping_sub(TEA_DELTA);
        }
        chunk[0..4].copy_from_slice(&v0.to_le_bytes());
        chunk[4..8].copy_from_slice(&v1.to_le_bytes());
    }

    Ok(out)
}

fn seed_obfuscation_decode(data: &[u8]) -> Result<StageSeed> {
    if data.len() < 4 { bail!("Invalid seed stage."); }
    let mut array = data.to_vec();
    let num = array.len();
    let num2 = num >> 2;
    let num3 = num2 >> 1;
    let indices = [num3, num3 + num2, num3 + 2 * num2, num3 + 3 * num2];

    for i in 0..4 {
        let num4 = num - 1 - i;
        array[indices[i]] ^= SEED_OBFUSCATION[i];
        array.swap(indices[i], num4);
    }

    let seed = u32::from_le_bytes(array[num - 4..num].try_into().unwrap());
    let payload = array[..num - 4].to_vec();

    Ok(StageSeed { data: payload, seed })
}

fn seed_obfuscation_encode(data: &[u8], seed: u32) -> Vec<u8> {
    let mut array = Vec::with_capacity(data.len() + 4);
    array.extend_from_slice(data);
    array.extend_from_slice(&seed.to_le_bytes());

    let num = array.len();
    let num2 = num >> 2;
    let num3 = num2 >> 1;
    let indices = [num3, num3 + num2, num3 + 2 * num2, num3 + 3 * num2];

    for i in 0..4 {
        let num4 = num - 1 - i;
        array.swap(indices[i], num4);
        array[indices[i]] ^= SEED_OBFUSCATION[i];
    }

    array
}

fn hash_tea_decode(data: &[u8], account_id: Option<&str>) -> Result<HashTeaResult> {
    let decrypted = tea_decrypt_zero_key(data)?;
    if decrypted.len() < 40 {
        bail!("Invalid encrypted payload (too short)");
    }
    let num = decrypted.len() - 40;
    let pad_len = u64::from_le_bytes(decrypted[num..num + 8].try_into().unwrap());
    if pad_len >= 8 || pad_len > num as u64 {
        bail!("Invalid TEA padding size.");
    }
    let num3 = pad_len as usize;
    let num4 = num - num3;

    let stage2 = decrypted[..num4].to_vec();
    let tea_padding_bytes = if num3 == 0 {
        Vec::new()
    } else {
        decrypted[num4..num].to_vec()
    };
    let mut embedded_digest = [0u8; 32];
    embedded_digest.copy_from_slice(&decrypted[decrypted.len() - 32..]);

    if let Some(id) = account_id {
        let computed_digest = compute_digest(&stage2, id)?;
        if embedded_digest != computed_digest {
            bail!("The save file does not match the specified Current UUID.");
        }
    }

    Ok(HashTeaResult {
        stage2,
        tea_padding_bytes,
        embedded_digest,
    })
}

fn hash_tea_encode(stage2: &[u8], account_id: &str, padding_bytes: &[u8]) -> Result<Vec<u8>> {
    let num = (-(stage2.len() as isize) & 7) as usize;
    let pad_bytes: Vec<u8> = if padding_bytes.is_empty() && num > 0 {
        vec![0u8; num]
    } else if padding_bytes.len() == num {
        padding_bytes.to_vec()
    } else {
        bail!("Invalid TEA padding.");
    };

    let digest = compute_digest(stage2, account_id)?;
    let mut array = Vec::with_capacity(stage2.len() + num + 8 + 32);
    array.extend_from_slice(stage2);
    array.extend_from_slice(&pad_bytes);
    array.extend_from_slice(&(num as u64).to_le_bytes());
    array.extend_from_slice(&digest);

    tea_encrypt_zero_key(&array)
}

fn pad_shuffle_decode(data: &[u8]) -> Result<PadShuffleResult> {
    if data.len() < 12 {
        bail!("Invalid shuffle stage.");
    }
    let num = data.len() - 12;
    let pad_len = u32::from_le_bytes(data[num..num + 4].try_into().unwrap()) as usize;
    let block_size = u32::from_le_bytes(data[num + 4..num + 8].try_into().unwrap()) as usize;
    let seed = u32::from_le_bytes(data[num + 8..num + 12].try_into().unwrap());

    if block_size == 0 || block_size > i32::MAX as usize {
        bail!("Invalid shuffle block size.");
    }

    let mut array = data[..num].to_vec();
    if array.len() % block_size != 0 || pad_len >= block_size {
        bail!("Invalid shuffle trailer.");
    }

    let mut rng = GrwRng::new(seed);
    for chunk in array.chunks_exact_mut(block_size) {
        let mut swap_indices = vec![0usize; block_size - 1];
        for j in (1..block_size).rev() {
            let next_val = rng.next() as usize;
            swap_indices[j - 1] = next_val % (j + 1);
        }
        for k in 1..block_size {
            let swap_idx = swap_indices[k - 1];
            chunk.swap(k, swap_idx);
        }
    }

    if pad_len > array.len() {
        bail!("Invalid shuffle padding size.");
    }

    let actual_len = array.len() - pad_len;
    let padding_bytes = if pad_len == 0 {
        Vec::new()
    } else {
        array[actual_len..].to_vec()
    };
    let stage1 = array[..actual_len].to_vec();

    Ok(PadShuffleResult {
        stage1,
        seed,
        padding_bytes,
    })
}

fn pad_shuffle_encode(data: &[u8], seed: u32, padding_bytes: &[u8]) -> Result<Vec<u8>> {
    let num = (-(data.len() as isize) & 7) as usize;
    let pad_bytes: Vec<u8> = if padding_bytes.is_empty() && num > 0 {
        vec![0u8; num]
    } else if padding_bytes.len() == num {
        padding_bytes.to_vec()
    } else {
        bail!("Invalid shuffle padding.");
    };

    let mut array = Vec::with_capacity(data.len() + num + 12);
    array.extend_from_slice(data);
    array.extend_from_slice(&pad_bytes);

    let num2 = data.len() + num;
    let mut rng = GrwRng::new(seed);

    for chunk in array[..num2].chunks_exact_mut(8) {
        for j in (1..=7).rev() {
            let next_val = rng.next() as usize;
            let swap_idx = next_val % (j + 1);
            chunk.swap(j, swap_idx);
        }
    }

    array.extend_from_slice(&(num as u32).to_le_bytes());
    array.extend_from_slice(&8u32.to_le_bytes());
    array.extend_from_slice(&seed.to_le_bytes());

    Ok(array)
}

fn xor_stream_decode(data: &[u8]) -> Result<StageSeed> {
    if data.len() < 4 {
        bail!("Invalid XOR stream stage.");
    }
    let seed = u32::from_le_bytes(data[data.len() - 4..].try_into().unwrap());
    let mut array = data[..data.len() - 4].to_vec();

    let mut rng = GrwRng::new(seed);
    let mut key = 0i32;

    for i in 0..array.len() {
        if i % 4 == 0 { key = rng.next(); }
        let shift = 8 * (i % 4);
        let mask = ((key >> shift) & 0xFF) as u8;
        array[i] ^= mask;
    }

    Ok(StageSeed { data: array, seed })
}

fn xor_stream_encode(data: &[u8], seed: u32) -> Vec<u8> {
    let mut array = Vec::with_capacity(data.len() + 4);
    array.extend_from_slice(data);

    let mut rng = GrwRng::new(seed);
    let mut key = 0i32;

    for i in 0..data.len() {
        if i % 4 == 0 { key = rng.next(); }
        let shift = 8 * (i % 4);
        let mask = ((key >> shift) & 0xFF) as u8;
        array[i] ^= mask;
    }

    array.extend_from_slice(&seed.to_le_bytes());
    array
}

pub fn decrypt_payload(ciphertext: &[u8], account_id: Option<&str>) -> Result<DecryptResult> {
    let stage_seed3 = seed_obfuscation_decode(ciphertext)?;
    let hash_tea = hash_tea_decode(&stage_seed3.data, account_id)?;
    let pad_shuffle = pad_shuffle_decode(&hash_tea.stage2)?;
    let stage_seed1 = xor_stream_decode(&pad_shuffle.stage1)?;

    Ok(DecryptResult {
        plaintext: stage_seed1.data,
        seed1: stage_seed1.seed,
        seed2: pad_shuffle.seed,
        seed3: stage_seed3.seed,
        shuffle_padding_bytes: pad_shuffle.padding_bytes,
        tea_padding_bytes: hash_tea.tea_padding_bytes,
        embedded_digest: hash_tea.embedded_digest,
    })
}

pub fn encrypt_payload(plaintext: &[u8], account_id: &str, parameters: &DecryptResult, ) -> Result<Vec<u8>> {
    let xor_encoded     = xor_stream_encode(plaintext, parameters.seed1);
    let shuffle_encoded = pad_shuffle_encode(&xor_encoded, parameters.seed2, &parameters.shuffle_padding_bytes)?;
    let tea_encoded     = hash_tea_encode(&shuffle_encoded, account_id, &parameters.tea_padding_bytes)?;
    let obfuscated      = seed_obfuscation_encode(&tea_encoded, parameters.seed3);
    Ok(obfuscated)
}

pub fn build_target_header( source_header: &[u8], target_platform: GrwSavePlatform, save_slot: u32, ) -> Result<Vec<u8>> {
    let mut header = if source_header.len() >= HEADER_SIZE {
        source_header[..HEADER_SIZE].to_vec()
    } else {
        super::template::get_default_header()
    };

    match target_platform {
        GrwSavePlatform::Ubisoft => {
            header[8..40].fill(0);
        }
        GrwSavePlatform::Steam => {
            let token = if save_slot % 2 == 1 {
                STEAM_TOKEN_ODD
            } else {
                STEAM_TOKEN_EVEN
            };
            header[8..40].copy_from_slice(token);
        }
    }
    Ok(header)
}

pub fn convert_full_save(full_save: &[u8], source_account_id: Option<&str>, target_account_id: &str, target_header: Option<&[u8]>) -> Result<Vec<u8>> {
    if full_save.len() <= HEADER_SIZE { bail!("The save file is invalid (too short)."); }

    let header = match target_header {
        Some(h) => {
            if h.len() != HEADER_SIZE {
                bail!("Target header must be exactly {} bytes.", HEADER_SIZE);
            }
            h.to_vec()
        }
        None => extract_header(full_save)?,
    };

    let ciphertext = &full_save[HEADER_SIZE..];
    let decrypted = decrypt_payload(ciphertext, source_account_id)?;
    let re_encrypted = encrypt_payload(&decrypted.plaintext, target_account_id, &decrypted)?;
    let verify = decrypt_payload(&re_encrypted, Some(target_account_id))?;

    if verify.plaintext != decrypted.plaintext { bail!("Converted save verification failed."); }

    let mut result = Vec::with_capacity(HEADER_SIZE + re_encrypted.len());
    result.extend_from_slice(&header);
    result.extend_from_slice(&re_encrypted);
    Ok(result)
}
