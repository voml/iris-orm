//! UUID <-> BINARY(16) helpers for MySQL `uuid` columns.
//!
//! Schema `uuid` PKs must be **UUID v7** (`iris::uuid()` / VOS `uuid()`). Random v4 keys
//! cause clustered-index page splits and insert throughput collapse; v7 keeps inserts
//! append-mostly on the B-tree right edge. See `vos-language/specifications/uuid-v7.md`.

/// Parse hyphenated or 32-hex UUID text into 16 bytes.
pub fn try_parse_uuid_bytes(text: &str) -> Option<Vec<u8>> {
    let compact: String = text.chars().filter(|c| *c != '-').collect();
    if compact.len() != 32 || !compact.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = Vec::with_capacity(16);
    let bytes = compact.as_bytes();
    for i in 0..16 {
        let hi = hex_nibble(bytes[i * 2])?;
        let lo = hex_nibble(bytes[i * 2 + 1])?;
        out.push((hi << 4) | lo);
    }
    Some(out)
}

/// Format 16 raw bytes as canonical hyphenated UUID text.
pub fn uuid_bytes_to_str(bytes: &[u8]) -> Option<String> {
    if bytes.len() != 16 {
        return None;
    }
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    Some(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_hyphenated() {
        let s = "550e8400-e29b-41d4-a716-446655440000";
        let b = try_parse_uuid_bytes(s).unwrap();
        assert_eq!(b.len(), 16);
        assert_eq!(uuid_bytes_to_str(&b).as_deref(), Some(s));
    }

    #[test]
    fn roundtrip_compact() {
        let s = "550e8400e29b41d4a716446655440000";
        let b = try_parse_uuid_bytes(s).unwrap();
        assert_eq!(
            uuid_bytes_to_str(&b).as_deref(),
            Some("550e8400-e29b-41d4-a716-446655440000")
        );
    }
}
