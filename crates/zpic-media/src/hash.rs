//! Content hashing (blake3) for deduplication and template variables.

use blake3::Hasher;

/// Compute the full blake3 hash of the given bytes.
pub fn content_hash(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

/// Return the hex-encoded full hash.
pub fn content_hash_hex(bytes: &[u8]) -> String {
    hex::encode(content_hash(bytes))
}

/// Return the first `n` hex characters of the content hash. Used for the
/// `{hash8}` template variable.
pub fn short_hash(bytes: &[u8], n: usize) -> String {
    let hex = content_hash_hex(bytes);
    hex.chars().take(n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_deterministic() {
        let a = content_hash_hex(b"hello");
        let b = content_hash_hex(b"hello");
        assert_eq!(a, b);
    }

    #[test]
    fn short_hash_truncates() {
        let h = short_hash(b"hello", 8);
        assert_eq!(h.len(), 8);
    }
}
