use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

use crate::crypto::{
    compute_sig, derive_key, decipher_block, encipher_block, 
    FOOTER_LEN, HASH_LEN, HEADER_LEN, SEED_XOR,
};
use crate::validate_uuid;

#[derive(Debug, Clone)]
pub struct WdlSave {
    pub stamp: [u8; 32],
    pub payload: Vec<u8>,
    pub signature: [u8; 32],
    pub seed: u32,
    pub tag: u8,
}

impl WdlSave {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file_bytes = fs::read(path.as_ref())
            .with_context(|| format!("Failed to read save file {:?}", path.as_ref()))?;
        Self::from_bytes(&file_bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < HEADER_LEN + FOOTER_LEN + HASH_LEN {
            bail!(
                "File too small: expected at least {} bytes, got {}",
                HEADER_LEN + FOOTER_LEN + HASH_LEN,
                bytes.len()
            );
        }

        let hdr_prefix = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        if hdr_prefix != 36 {
            bail!("Invalid save container header: expected 36 (0x24), got {}", hdr_prefix);
        }

        let mut stamp = [0u8; 32];
        stamp.copy_from_slice(&bytes[8..40]);

        let mut payload = bytes[HEADER_LEN..].to_vec();
        let n = payload.len();

        let end_marker = payload[n - 1];
        if end_marker != 1 {
            bail!("Invalid container end marker: expected 0x01, got 0x{:02X}", end_marker);
        }
        let tag = payload[n - 2];
        let body_len = n - FOOTER_LEN;

        if body_len >= 4 {
            let step = body_len >> 2;
            let offs = body_len >> 3;
            let indices = [offs, step + offs, 2 * step + offs, 3 * step + offs];

            for (idx, &shift) in indices.iter().enumerate() {
                payload.swap(shift, body_len + idx);
            }
        }

        let obf_seed = u32::from_le_bytes(payload[body_len..body_len + 4].try_into().unwrap());
        let seed = obf_seed ^ SEED_XOR;

        let key = derive_key(seed);

        let decrypt_len = body_len & !7;
        let mut decrypted = payload[..decrypt_len].to_vec();

        for chunk in decrypted.chunks_exact_mut(8) {
            let mut v0 = u32::from_le_bytes(chunk[0..4].try_into().unwrap());
            let mut v1 = u32::from_le_bytes(chunk[4..8].try_into().unwrap());
            decipher_block(&mut v0, &mut v1, &key);
            chunk[0..4].copy_from_slice(&v0.to_le_bytes());
            chunk[4..8].copy_from_slice(&v1.to_le_bytes());
        }

        if decrypted.len() < HASH_LEN {
            bail!("Decrypted payload too small to contain signature");
        }

        let sig_start = decrypted.len() - HASH_LEN;
        let mut signature = [0u8; 32];
        signature.copy_from_slice(&decrypted[sig_start..]);
        let actual_payload = decrypted[..sig_start].to_vec();

        Ok(Self {
            stamp,
            payload: actual_payload,
            signature,
            seed,
            tag,
        })
    }

    pub fn verify_uuid(&self, uuid_str: &str) -> Result<bool> {
        let clean_uuid = validate_uuid(uuid_str)?;
        let expected = compute_sig(&self.payload, &clean_uuid);
        Ok(self.signature == expected)
    }

    pub fn to_bytes_with_uuid(&self, target_uuid_str: &str, custom_seed: Option<u32>) -> Result<Vec<u8>> {
        let clean_uuid = validate_uuid(target_uuid_str)?;
        let new_signature = compute_sig(&self.payload, &clean_uuid);

        let seed = custom_seed.unwrap_or(self.seed);
        let key = derive_key(seed);

        let mut block = Vec::with_capacity(self.payload.len() + HASH_LEN + 8);
        block.extend_from_slice(&self.payload);
        block.extend_from_slice(&new_signature);

        let rem = block.len() % 8;
        if rem != 0 {
            block.resize(block.len() + (8 - rem), 0u8);
        }

        let body_len = block.len();

        for chunk in block.chunks_exact_mut(8) {
            let mut v0 = u32::from_le_bytes(chunk[0..4].try_into().unwrap());
            let mut v1 = u32::from_le_bytes(chunk[4..8].try_into().unwrap());
            encipher_block(&mut v0, &mut v1, &key);
            chunk[0..4].copy_from_slice(&v0.to_le_bytes());
            chunk[4..8].copy_from_slice(&v1.to_le_bytes());
        }

        let obf_seed = seed ^ SEED_XOR;
        block.extend_from_slice(&obf_seed.to_le_bytes());

        if body_len >= 4 {
            let step = body_len >> 2;
            let offs = body_len >> 3;
            let indices = [offs, step + offs, 2 * step + offs, 3 * step + offs];

            for (idx, &shift) in indices.iter().enumerate() {
                block.swap(shift, body_len + idx);
            }
        }

        block.push(self.tag);
        block.push(1);

        let mut output = Vec::with_capacity(HEADER_LEN + block.len());
        output.extend_from_slice(&36u32.to_le_bytes());
        output.extend_from_slice(&0u32.to_le_bytes());
        output.extend_from_slice(&self.stamp);
        output.extend_from_slice(&block);

        Ok(output)
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P, target_uuid_str: &str) -> Result<()> {
        let bytes = self.to_bytes_with_uuid(target_uuid_str, None)?;
        fs::write(path.as_ref(), bytes)
            .with_context(|| format!("Failed to write re-signed save to {:?}", path.as_ref()))?;
        Ok(())
    }

    pub fn stamp_str(&self) -> String {
        String::from_utf8_lossy(&self.stamp).into_owned()
    }
}
