//! UUID v7 request-log row ids (time-ordered, matching the `id TEXT PRIMARY KEY`
//! convention of authoritative-server tables; DATABASE_SPEC §6).

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Monotonic `rand_a` counter (RFC 4122-bis v7): ids are strictly increasing per
/// process even when generated within the same millisecond, so
/// `ORDER BY id DESC` is a reliable tiebreaker for same-second rows.
static RAND_A_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generates a lowercase hyphenated UUID v7 string for `log_request.id`.
///
/// Time-ordered ids keep `log_request` inserts naturally clustered on
/// `created_at` for the common newest-first list path.
pub fn new_request_log_id() -> String {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).expect("getrandom failed");

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    // v7 layout: unix_ts_ms(48) | ver(4) | rand_a(12) | var(2) | rand_b(62).
    bytes[0..6].copy_from_slice(&now_ms.to_be_bytes()[2..8]);
    bytes[6] = (bytes[6] & 0x0F) | 0x70; // version 7
    let sequence = RAND_A_COUNTER.fetch_add(1, Ordering::Relaxed) & 0x0FFF;
    bytes[6] = (bytes[6] & 0xF0) | ((sequence >> 8) as u8);
    bytes[7] = (sequence & 0xFF) as u8;
    bytes[8] = (bytes[8] & 0x3F) | 0x80; // variant 10

    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(36);
    for (index, byte) in bytes.iter().enumerate() {
        if index == 4 || index == 6 || index == 8 || index == 10 {
            out.push('-');
        }
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0F) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_log_id_is_uuid_v7_shaped() {
        let id = new_request_log_id();
        assert_eq!(36, id.len());
        assert_eq!('7', id.as_bytes()[14] as char);
        let variant = id.as_bytes()[19] as char;
        assert!(
            matches!(variant, '8' | '9' | 'a' | 'b'),
            "variant nibble must be 10xx, got {variant}"
        );
    }

    #[test]
    fn request_log_ids_are_distinct_and_strictly_increasing() {
        let first = new_request_log_id();
        let second = new_request_log_id();
        let third = new_request_log_id();
        assert_ne!(first, second);
        assert!(first < second, "ids must be strictly increasing");
        assert!(second < third, "ids must be strictly increasing");
    }
}
