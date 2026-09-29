use aes::Aes256;
use cipher::{BlockCipherDecrypt, BlockCipherEncrypt, KeyInit};
use muddy::muddy;

pub const GVAS_MAGIC: u64 = 0x0000000253415647; // b"GVAS\x02\x00\x00\x00" as u64 LE

pub struct Crypto;

impl Crypto {
    pub fn salt() -> &'static [u8; 32] {
        muddy!("M$KW=w4,knvovE8u98L=~1f-MSoJ2F9r").as_bytes().first_chunk::<32>().unwrap()
    }

    pub fn key_prefix() -> &'static [u8; 24] {
        muddy!("M$KW=w4,knvovE8u00000000").as_bytes().first_chunk::<24>().unwrap()
    }

    pub fn derive_key(steam_id: u64) -> [u8; 32] {
        let account_id = (steam_id & 0xFFFFFFFF) as u32;
        let mut key = [0u8; 32];
        key[..24].copy_from_slice(Self::key_prefix());
        let hex = format!("{:08x}", account_id);
        key[24..32].copy_from_slice(hex.as_bytes());
        key
    }

    #[inline(always)]
    pub fn format_key_fast(prefix: &[u8; 24], account_id: u32, key_buf: &mut [u8; 32]) {
        key_buf[..24].copy_from_slice(prefix);
        const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
        key_buf[24] = HEX_CHARS[((account_id >> 28) & 0xF) as usize];
        key_buf[25] = HEX_CHARS[((account_id >> 24) & 0xF) as usize];
        key_buf[26] = HEX_CHARS[((account_id >> 20) & 0xF) as usize];
        key_buf[27] = HEX_CHARS[((account_id >> 16) & 0xF) as usize];
        key_buf[28] = HEX_CHARS[((account_id >> 12) & 0xF) as usize];
        key_buf[29] = HEX_CHARS[((account_id >> 8) & 0xF) as usize];
        key_buf[30] = HEX_CHARS[((account_id >> 4) & 0xF) as usize];
        key_buf[31] = HEX_CHARS[(account_id & 0xF) as usize];
    }

    pub fn offline_key() -> [u8; 32] {
        *Self::salt()
    }

    pub fn decrypt_ecb(ciphertext: &mut [u8], key: &[u8; 32]) {
        assert_eq!(ciphertext.len() % 16, 0, "Ciphertext must be 16-byte aligned");
        let cipher = Aes256::new(key.into());
        for chunk in ciphertext.chunks_exact_mut(16) {
            let block: &mut [u8; 16] = chunk.try_into().unwrap();
            cipher.decrypt_block(block.into());
        }
    }

    pub fn encrypt_ecb(plaintext: &mut [u8], key: &[u8; 32]) {
        assert_eq!(plaintext.len() % 16, 0, "Plaintext must be 16-byte aligned");
        let cipher = Aes256::new(key.into());
        for chunk in plaintext.chunks_exact_mut(16) {
            let block: &mut [u8; 16] = chunk.try_into().unwrap();
            cipher.encrypt_block(block.into());
        }
    }

    // sub_141709410
    pub fn compute_rolling_checksum(ciphertext: &[u8]) -> u8 {
        let mut seed: u8 = 26; // 0x1A
        for &b in ciphertext.iter().rev() {
            seed = seed.wrapping_mul(2) ^ b;
        }
        seed
    }

    // sub_141709450
    pub fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xFFFFFFFFu32;
        for &byte in data {
            crc ^= byte as u32;
            for _ in 0..8 {
                crc = if (crc & 1) != 0 {
                    (crc >> 1) ^ 0xEDB88320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }
}
