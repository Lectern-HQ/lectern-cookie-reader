//! Test data shared by the Chromium tests.
//!
//! Vectors made outside this crate with Python's hashlib and the OpenSSL command line:
//! key = PBKDF2-HMAC-SHA1("test-password", "saltysalt", 1003, 16 bytes),
//! value = "v10" + AES-128-CBC(key, IV of 16 spaces, PKCS7)(plaintext).

pub(crate) const KEY: &str = "c0ffe4c25f07f62bfc6ab011d9efa54e";
/// plaintext = SHA-256(".example.com") + "hello-world"
pub(crate) const V24: &str = "763130c9f9d215dfe3fcea9f5f48b78c27252a61339f8511ba847e3060c0caa08c030cea6a3324ec0935447379bb81cf90ecc9";
/// plaintext = "hello-world"
pub(crate) const V23: &str = "763130e8b708e830066522ef13587430f60f95";

pub(crate) fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

pub(crate) fn key() -> [u8; 16] {
    hex(KEY).try_into().unwrap()
}
