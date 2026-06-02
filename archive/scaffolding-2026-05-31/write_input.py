from pathlib import Path

# ===== INPUT ENGINE =====
# Windows RawInput capture + SendInput injection + border detection

input_lib = """\
//! Input engine: keyboard/mouse capture and injection.
//!
//! Windows implementation using RawInput (capture) and SendInput (injection).
//! Linux implementation using evdev (capture) and uinput (injection).
//!
//! KVM-specific features:
//! - Multi-monitor border detection: determines when cursor enters/exits
//!   the "remote screen" monitor
//! - Passthrough mode: capture on one machine, inject on the other
//! - Escape key combo: Ctrl+Alt+Del always handled locally (high priority)

#[cfg(target_os = "windows")]
mod win;
#[cfg(target_os = "linux")]
mod linux;

/// A keyboard event.
#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    pub down: bool,
    /// Hardware scancode (set 1)
    pub scancode: u16,
    /// Virtual key code (Windows VK or Linux keycode)
    pub vk: u16,
    /// Modifier flags at time of event
    pub modifiers: Modifiers,
}

/// A mouse event.
#[derive(Debug, Clone, Copy)]
pub enum MouseEvent {
    Move { dx: i32, dy: i32, absolute: bool },
    Button { button: MouseButton, down: bool },
    Wheel { delta: i16, horizontal: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Modifiers: u8 {
        const CTRL  = 0x01;
        const ALT   = 0x02;
        const SHIFT = 0x04;
        const WIN   = 0x08;
    }
}

/// Information about a display monitor.
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub index: u32,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
}

/// Input capture interface.
pub trait InputCapture: Send {
    /// Poll for keyboard events (non-blocking).
    fn poll_keys(&mut self) -> Vec<KeyEvent>;
    /// Poll for mouse events (non-blocking).
    fn poll_mouse(&mut self) -> Vec<MouseEvent>;
    /// Get current cursor position.
    fn cursor_pos(&self) -> (i32, i32);
    /// Get monitor layout info.
    fn monitors(&self) -> Vec<MonitorInfo>;
}

/// Input injection interface.
pub trait InputInjector: Send {
    /// Inject a keyboard event.
    fn inject_key(&mut self, event: &KeyEvent);
    /// Inject a mouse event.
    fn inject_mouse(&mut self, event: &MouseEvent);
    /// Set cursor position (absolute).
    fn set_cursor_pos(&mut self, x: i32, y: i32);
}

/// Border watcher: determines when cursor crosses between local/remote monitors.
pub struct BorderWatcher {
    /// Index of the monitor that's designated as "remote" (the KVM display).
    remote_monitor: u32,
    /// Whether the cursor is currently inside the remote monitor.
    cursor_on_remote: bool,
}

impl BorderWatcher {
    pub fn new(remote_monitor: u32) -> Self {
        Self { remote_monitor, cursor_on_remote: false }
    }

    /// Check if cursor has entered or exited the remote monitor.
    /// Returns Some(true) when entering remote, Some(false) when leaving, None if no change.
    pub fn check(&mut self, x: i32, y: i32, monitors: &[MonitorInfo]) -> Option<bool> {
        let remote = monitors.iter().find(|m| m.index == self.remote_monitor)?;
        let inside = x >= remote.x
            && x < remote.x + remote.width as i32
            && y >= remote.y
            && y < remote.y + remote.height as i32;

        if inside != self.cursor_on_remote {
            self.cursor_on_remote = inside;
            Some(inside)
        } else {
            None
        }
    }

    pub fn is_on_remote(&self) -> bool {
        self.cursor_on_remote
    }
}
"""

Path(r"G:\codex-AI-tools\lan-link\crates\input\src\lib.rs").write_text(input_lib, encoding="utf-8")
print("input lib.rs done")
