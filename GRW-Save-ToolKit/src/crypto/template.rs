const DEFAULT_HEADER_B64: &str = "JAIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAEEAQwA0AEIARgBbADkAXQAtAHQAbQBfADIAMAAyADYAXwAwADcAXwAzADEAXwAwADQAXwAyADYAXwAzADIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

pub fn get_default_header() -> Vec<u8> {
    let bytes = DEFAULT_HEADER_B64.as_bytes();
    let mut out = Vec::with_capacity(552);
    let mut i = 0;
    while i < bytes.len() {
        let b0 = decode_char(bytes[i]);
        let b1 = decode_char(bytes[i + 1]);
        let b2 = decode_char(bytes[i + 2]);
        let b3 = decode_char(bytes[i + 3]);
        out.push((b0 << 2) | (b1 >> 4));
        if bytes[i + 2] != b'=' {
            out.push(((b1 & 0x0F) << 4) | (b2 >> 2));
        }
        if bytes[i + 3] != b'=' {
            out.push(((b2 & 0x03) << 6) | b3);
        }
        i += 4;
    }
    out
}

fn decode_char(c: u8) -> u8 {
    match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => 0,
    }
}
