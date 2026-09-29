pub mod container;
pub mod crypto;
pub mod finder;

use crypto::Crypto;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKey {
    Online(u64),
    Offline,
}

impl TargetKey {
    pub fn to_aes_key(&self) -> [u8; 32] {
        match self {
            TargetKey::Online(steam_id) => Crypto::derive_key(*steam_id),
            TargetKey::Offline => Crypto::offline_key(),
        }
    }
}

pub fn parse_steam_id(s: &str) -> Result<u64, String> {
    let trimmed = s.trim();
    if let Some(hex_str) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
        u64::from_str_radix(hex_str, 16).map_err(|e| format!("Invalid hex SteamID '{}': {}", s, e))
    } else {
        trimmed.parse::<u64>().map_err(|e| format!("Invalid SteamID '{}': {}", s, e))
    }
}
