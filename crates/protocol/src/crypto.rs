//! ChaCha20-Poly1305 AEAD 加密\n//! 96-bit nonce（conn_id || seq），认证加密防重放\n//!
//! Each packet is encrypted with a unique 96-bit nonce derived from the
//! connection ID + sequence number, ensuring per-packet authentication.

use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce,
};
use rand::RngCore;

/// Pre-shared key (32 bytes). Generated once, shared out-of-band.
pub type Psk = [u8; 32];

/// Generate a random 32-byte PSK.
pub fn generate_psk() -> Psk {
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    key
}

/// Encrypt plaintext, producing ciphertext + 16-byte authentication tag.
/// `nonce` must be exactly 12 bytes.
pub fn encrypt(key: &Psk, nonce: &[u8; 12], plaintext: &[u8]) -> Vec<u8> {
    let cipher = ChaCha20Poly1305::new_from_slice(key).expect("valid key size");
    let nonce = Nonce::from_slice(nonce);
    cipher.encrypt(nonce, plaintext).expect("encryption failed")
}

/// Decrypt ciphertext (which includes the 16-byte auth tag appended).
/// Returns plaintext or None if authentication fails.
pub fn decrypt(key: &Psk, nonce: &[u8; 12], ciphertext: &[u8]) -> Option<Vec<u8>> {
    let cipher = ChaCha20Poly1305::new_from_slice(key).expect("valid key size");
    let nonce = Nonce::from_slice(nonce);
    cipher.decrypt(nonce, ciphertext).ok()
}

/// Derive a per-packet nonce from conn_id + sequence number.
/// This gives us a unique nonce per packet without sending a random nonce each time.
pub fn make_nonce(conn_id: u64, seq: u32) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce[0..8].copy_from_slice(&conn_id.to_le_bytes());
    nonce[8..12].copy_from_slice(&seq.to_le_bytes());
    nonce
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let psk = generate_psk();
        let nonce = make_nonce(42, 1);
        let plain = b"hello world";
        let cipher = encrypt(&psk, &nonce, plain);
        assert_eq!(cipher.len(), plain.len() + 16);
        let dec = decrypt(&psk, &nonce, &cipher).unwrap();
        assert_eq!(&dec, plain);
    }

    #[test]
    fn tamper_detection() {
        let psk = generate_psk();
        let nonce = make_nonce(42, 1);
        let mut cipher = encrypt(&psk, &nonce, b"hello");
        cipher[0] ^= 1;
        assert!(decrypt(&psk, &nonce, &cipher).is_none());
    }
}
