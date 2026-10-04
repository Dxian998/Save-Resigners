use anyhow::{bail, Result};

pub mod crypto;
pub mod save;

pub use crypto::*;
pub use save::WdlSave;

pub fn validate_uuid(uuid_str: &str) -> Result<String> {
    let clean = uuid_str.trim().to_lowercase();
    if clean.len() != 36 {
        bail!("Invalid UUID length: expected 36 characters, got {}", clean.len());
    }
    let bytes = clean.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if i == 8 || i == 13 || i == 18 || i == 23 {
            if b != b'-' {
                bail!("Invalid UUID format: expected dash at position {}", i);
            }
        } else if !b.is_ascii_hexdigit() {
            bail!("Invalid UUID format: character '{}' at position {} is not hex", b as char, i);
        }
    }
    Ok(clean)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xtea_block_roundtrip() {
        let key = derive_key(0x12345678);
        let orig_v0 = 0xDEADBEEFu32;
        let orig_v1 = 0xCAFEBABEu32;
        let mut v0 = orig_v0;
        let mut v1 = orig_v1;

        encipher_block(&mut v0, &mut v1, &key);
        assert_ne!((v0, v1), (orig_v0, orig_v1));
        decipher_block(&mut v0, &mut v1, &key);
        assert_eq!((v0, v1), (orig_v0, orig_v1));
    }

    #[test]
    fn test_save_container_roundtrip() {
        let orig_payload = b"DisruptEngineSaveGamePayloadSample1234567890!@#$%^&*()_+";
        let uuid_a = "12345678-1234-1234-1234-123456789abc";
        let uuid_b = "87654321-4321-4321-4321-cba987654321";

        let save = WdlSave {
            stamp: *b"d3de88dca8c2f28c68a025f662e42f05",
            payload: orig_payload.to_vec(),
            signature: [0u8; 32],
            seed: 0x99887766,
            tag: 0,
        };

        let encoded = save.to_bytes_with_uuid(uuid_a, None).expect("encoding failed");
        let parsed = WdlSave::from_bytes(&encoded).expect("decoding failed");

        assert_eq!(parsed.payload, orig_payload);
        assert_eq!(parsed.stamp, *b"d3de88dca8c2f28c68a025f662e42f05");
        assert_eq!(parsed.seed, 0x99887766);

        assert!(parsed.verify_uuid(uuid_a).unwrap());
        assert!(!parsed.verify_uuid(uuid_b).unwrap());

        let resigned_bytes = parsed.to_bytes_with_uuid(uuid_b, None).expect("resigning failed");
        let parsed_b = WdlSave::from_bytes(&resigned_bytes).expect("decoding resigned failed");

        assert_eq!(parsed_b.payload, orig_payload);
        assert!(!parsed_b.verify_uuid(uuid_a).unwrap());
        assert!(parsed_b.verify_uuid(uuid_b).unwrap());
    }
}
