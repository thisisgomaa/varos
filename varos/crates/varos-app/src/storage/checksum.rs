//! In-house CRC-32 (IEEE 802.3, reflected, poly `0xEDB88320`) and unique nonces.
//! No new dependency: the table is built at compile time.
use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const fn make_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
}

static TABLE: [u32; 256] = make_table();

/// CRC-32/IEEE of `bytes` (the zip/PNG checksum). Detects damaged snapshots; not a security hash.
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in bytes {
        c = TABLE[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

static NONCE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// 128 bits as 32 lowercase hex chars: two independently keyed std hashes of (time, pid, counter).
/// Unique across writes and processes for naming temp files / recovery sessions; **not** a secret.
pub fn new_nonce() -> String {
    let n = NONCE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let t = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let pid = std::process::id();
    let half = |salt: u8| RandomState::new().hash_one((salt, t, pid, n));
    format!("{:016x}{:016x}", half(0), half(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn crc32_known_vector() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"The quick brown fox jumps over the lazy dog"), 0x414F_A339);
    }

    #[test]
    fn nonces_are_unique_across_10k() {
        let set: HashSet<String> = (0..10_000).map(|_| new_nonce()).collect();
        assert_eq!(set.len(), 10_000);
        assert!(set.iter().all(|s| s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit())));
    }
}
