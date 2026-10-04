use sha2::{Digest, Sha256};

pub const LCG_MULT: u32 = 0x000343FD;
pub const LCG_ADD: u32 = 0x00269EC3;
pub const SEED_XOR: u32 = 0x01CCCA70;
pub const DELTA: u32 = 0x9E3779B9;
pub const HEADER_LEN: usize = 40;
pub const FOOTER_LEN: usize = 6;
pub const HASH_LEN: usize = 32;

#[inline]
pub fn decipher_block(v0: &mut u32, v1: &mut u32, key: &[u32; 4]) {
    let mut sum = DELTA.wrapping_mul(32);
    for _ in 0..32 {
        let shift_v0 = ((*v0 << 4) ^ (*v0 >> 5)).wrapping_add(*v0);
        let term_v0 = key[((sum >> 11) & 3) as usize].wrapping_add(sum);
        *v1 = v1.wrapping_sub(shift_v0 ^ term_v0);
        sum = sum.wrapping_sub(DELTA);
        let shift_v1 = ((*v1 << 4) ^ (*v1 >> 5)).wrapping_add(*v1);
        let term_v1 = key[(sum & 3) as usize].wrapping_add(sum);
        *v0 = v0.wrapping_sub(shift_v1 ^ term_v1);
    }
}

// XTEA ops
#[inline]
pub fn encipher_block(v0: &mut u32, v1: &mut u32, key: &[u32; 4]) {
    let mut sum: u32 = 0;
    for _ in 0..32 {
        let shift_v1 = ((*v1 << 4) ^ (*v1 >> 5)).wrapping_add(*v1);
        let term_v0 = key[(sum & 3) as usize].wrapping_add(sum);
        *v0 = v0.wrapping_add(shift_v1 ^ term_v0);
        sum = sum.wrapping_add(DELTA);
        let shift_v0 = ((*v0 << 4) ^ (*v0 >> 5)).wrapping_add(*v0);
        let term_v1 = key[((sum >> 11) & 3) as usize].wrapping_add(sum);
        *v1 = v1.wrapping_add(shift_v0 ^ term_v1);
    }
}

#[inline]
pub fn next_lcg(s: u32) -> u32 {
    s.wrapping_mul(LCG_MULT).wrapping_add(LCG_ADD)
}

pub fn derive_key(seed: u32) -> [u32; 4] {
    let mut state = seed;
    let mut upper = 0;
    for i in 0..6 {
        state = next_lcg(state);
        if i == 4 {
            upper = state >> 16;
        }
    }
    let mut state = (state & 0xFFFF_0000) | (upper & 0xFFFF);

    let mut key_bytes = [0u8; 16];
    for byte in key_bytes.iter_mut() {
        state = next_lcg(state);
        let val = (state >> 16) & 0xFFFF;
        let div = ((val & 0x7FFF) / 255) as u8;
        *byte = (val as u8).wrapping_add(div).wrapping_add(1);
    }

    [
        u32::from_le_bytes(key_bytes[0..4].try_into().unwrap()),
        u32::from_le_bytes(key_bytes[4..8].try_into().unwrap()),
        u32::from_le_bytes(key_bytes[8..12].try_into().unwrap()),
        u32::from_le_bytes(key_bytes[12..16].try_into().unwrap()),
    ]
}

pub fn compute_sig(payload: &[u8], uuid: &str) -> [u8; 32] {
    let clean_uuid = uuid.trim().to_lowercase();
    let mut hasher = Sha256::new();
    hasher.update(payload);
    hasher.update(clean_uuid.as_bytes());
    hasher.finalize().into()
}
