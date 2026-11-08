//! Stable identifier hashing.

use sha2::{Digest, Sha256};

/// Returns the first 12 hexadecimal characters of a SHA-256 digest.
#[must_use]
pub fn candidate_id(stable_key: &str) -> String {
    sha256(stable_key)[..12].to_owned()
}

/// Returns a full SHA-256 digest encoded as lowercase hexadecimal.
#[must_use]
pub fn sha256(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    format!("{digest:x}")
}

#[cfg(test)]
mod tests {
    use super::{candidate_id, sha256};

    #[test]
    fn uses_the_documented_sha256_prefix() {
        assert_eq!(
            candidate_id("src/shared.ts#Shared#typeAlias"),
            "6bc37e02088c"
        );
    }

    #[test]
    fn produces_full_declaration_hashes() {
        assert_eq!(
            sha256("src/a.ts#interface#Alpha#0"),
            "45d242a3ea5a88a300b173ee9ba8d79b2ef8ac0889d2df6d171a7588df1b4859"
        );
    }
}
