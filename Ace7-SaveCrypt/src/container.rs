use anyhow::{bail, ensure, Result};
use crate::crypto::{Crypto, GVAS_MAGIC};

pub struct EcsdSave {
    pub rolling_checksum: u8,
    pub unencrypted_len: u32,
    pub ciphertext: Vec<u8>,
    pub crc32: u32,
}

impl EcsdSave {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() >= 13,
            "File is too small to be an ECSD save (got {} bytes, minimum 13)",
            bytes.len()
        );
        ensure!(
            &bytes[..4] == b"ECSD",
            "Invalid file magic: expected 'ECSD', found {:?}",
            &bytes[..4]
        );

        let rolling_checksum = bytes[4];
        let unencrypted_len = u32::from_le_bytes(bytes[5..9].try_into()?);
        let ciphertext = bytes[9..bytes.len() - 4].to_vec();
        let crc32 = u32::from_be_bytes(bytes[bytes.len() - 4..].try_into()?);

        ensure!(
            ciphertext.len() % 16 == 0,
            "Ciphertext length ({}) is not a multiple of 16",
            ciphertext.len()
        );

        Ok(Self {
            rolling_checksum,
            unencrypted_len,
            ciphertext,
            crc32,
        })
    }

    pub fn verify_integrity(&self, raw_bytes: &[u8]) -> Result<()> {
        let computed_crc = Crypto::crc32(&raw_bytes[..raw_bytes.len() - 4]);
        ensure!(
            computed_crc == self.crc32,
            "CRC-32 checksum mismatch: computed 0x{:08X}, expected 0x{:08X}",
            computed_crc,
            self.crc32
        );

        let computed_rolling = Crypto::compute_rolling_checksum(&raw_bytes[5..raw_bytes.len() - 4]);
        ensure!(
            computed_rolling == self.rolling_checksum,
            "Rolling XOR checksum mismatch: computed 0x{:02X}, expected 0x{:02X}",
            computed_rolling,
            self.rolling_checksum
        );

        Ok(())
    }

    pub fn decrypt(&self, key: &[u8; 32]) -> Result<Vec<u8>> {
        let mut decrypted = self.ciphertext.clone();
        Crypto::decrypt_ecb(&mut decrypted, key);

        if decrypted.len() < 8 {
            bail!("Decrypted payload too short");
        }

        let magic = u64::from_le_bytes(decrypted[..8].try_into().unwrap());
        if magic != GVAS_MAGIC {
            bail!(
                "Decryption failed: expected GVAS header, got {:?}",
                &decrypted[..4]
            );
        }

        decrypted.truncate(self.unencrypted_len as usize);
        Ok(decrypted)
    }

    pub fn pack(gvas_payload: &[u8], key: &[u8; 32]) -> Vec<u8> {
        let unencrypted_len = gvas_payload.len() as u32;

        let mut padded = gvas_payload.to_vec();
        let remainder = padded.len() % 16;
        if remainder != 0 {
            padded.resize(padded.len() + (16 - remainder), 0u8);
        }

        Crypto::encrypt_ecb(&mut padded, key);

        let mut check_slice = Vec::with_capacity(4 + padded.len());
        check_slice.extend_from_slice(&unencrypted_len.to_le_bytes());
        check_slice.extend_from_slice(&padded);
        let rolling_checksum = Crypto::compute_rolling_checksum(&check_slice);

        let mut container = Vec::with_capacity(9 + padded.len() + 4);
        container.extend_from_slice(b"ECSD");
        container.push(rolling_checksum);
        container.extend_from_slice(&unencrypted_len.to_le_bytes());
        container.extend_from_slice(&padded);

        let crc = Crypto::crc32(&container);
        container.extend_from_slice(&crc.to_be_bytes());

        container
    }
}
