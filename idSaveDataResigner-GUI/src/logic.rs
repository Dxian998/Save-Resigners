#[cfg(target_os = "windows")]
use aes_gcm::{aead::{AeadCore, KeyInit, OsRng, Aead, Payload}, Aes128Gcm, Nonce, Key};
use sha2::{Sha256, Digest};
use anyhow::{Result, anyhow};

pub const NONCE_LENGTH: usize = 12;
pub const TAG_LENGTH: usize = 16;
pub const NONCE_AND_TAG_TOTAL_LENGTH: usize = NONCE_LENGTH + TAG_LENGTH;

pub struct IdCrypto;

impl IdCrypto {
    pub fn decrypt_file(
        input_data: &[u8],
        file_name: &str,
        game_code: &str,
        user_id: &str,
    ) -> Result<Vec<u8>> {
        if input_data.len() < NONCE_AND_TAG_TOTAL_LENGTH {
            return Err(anyhow!("Input data too short"));
        }

        let nonce_bytes = &input_data[..NONCE_LENGTH];
        let ciphertext_with_tag = &input_data[NONCE_LENGTH..];
        let key = Self::derive_key(user_id, game_code, file_name)?;
        let cipher = Aes128Gcm::new(&key);
        let nonce = Nonce::from_slice(nonce_bytes);
        let aad = format!("{}{}{}", user_id, game_code, file_name);
        let payload = Payload {
            msg: ciphertext_with_tag,
            aad: aad.as_bytes(),
        };
        
        cipher.decrypt(nonce, payload)
            .map_err(|e| anyhow!("Decryption failed: {}", e))
    }

    pub fn encrypt_file(
        input_data: &[u8],
        file_name: &str,
        game_code: &str,
        user_id: &str,
    ) -> Result<Vec<u8>> {
        let key = Self::derive_key(user_id, game_code, file_name)?;
        let cipher = Aes128Gcm::new(&key);
        let nonce = Aes128Gcm::generate_nonce(&mut OsRng);
        let aad = format!("{}{}{}", user_id, game_code, file_name);
        let payload = Payload {
            msg: input_data,
            aad: aad.as_bytes(),
        };
        let ciphertext = cipher.encrypt(&nonce, payload)
            .map_err(|e| anyhow!("Encryption failed: {}", e))?;
        let mut output = Vec::with_capacity(NONCE_LENGTH + ciphertext.len());
        output.extend_from_slice(&nonce);
        output.extend_from_slice(&ciphertext);
        
        Ok(output)
    }

    pub fn resign_file(
        input_data: &[u8],
        file_name: &str,
        game_code: &str,
        old_user_id: &str,
        new_user_id: &str,
    ) -> Result<Vec<u8>> {
        let decrypted = Self::decrypt_file(input_data, file_name, game_code, old_user_id)?;
        Self::encrypt_file(&decrypted, file_name, game_code, new_user_id)
    }

    fn derive_key(user_id: &str, game_code: &str, file_name: &str) -> Result<Key<Aes128Gcm>> {
        let mut hasher = Sha256::new();
        hasher.update(user_id.as_bytes());
        hasher.update(game_code.as_bytes());
        hasher.update(file_name.as_bytes());
        let hash = hasher.finalize();
        let key_bytes = &hash[..16];
        Ok(*Key::<Aes128Gcm>::from_slice(key_bytes))
    }
}
