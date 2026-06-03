//! Input engine: keyboard/mouse capture and injection.

#[cfg(target_os = "windows")]
pub mod win;
#[cfg(target_os = "linux")]
pub mod linux;

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct KeyEvent {
    pub down: bool,
    pub scancode: u16,
    pub vk: u16,
    pub modifiers: Modifiers,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MouseEvent {
    Move { dx: i32, dy: i32, absolute: bool },
    Button { button: MouseButton, down: bool },
    Wheel { delta: i16, horizontal: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Modifiers: u8 {
        const CTRL  = 0x01;
        const ALT   = 0x02;
        const SHIFT = 0x04;
        const WIN   = 0x08;
    }
}

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

pub trait InputCapture: Send {
    fn poll_keys(&mut self) -> Vec<KeyEvent>;
    fn poll_mouse(&mut self) -> Vec<MouseEvent>;
    fn cursor_pos(&self) -> (i32, i32);
    fn monitors(&self) -> Vec<MonitorInfo>;
}

pub trait InputInjector: Send {
    fn inject_key(&mut self, event: &KeyEvent) -> isize;
    fn inject_mouse(&mut self, event: &MouseEvent) -> isize;
    fn set_cursor_pos(&mut self, x: i32, y: i32);
}

pub struct BorderWatcher {
    remote_monitor: u32,
    cursor_on_remote: bool,
}

impl BorderWatcher {
    pub fn new(remote_monitor: u32) -> Self { Self { remote_monitor, cursor_on_remote: false } }

    pub fn check(&mut self, x: i32, y: i32, monitors: &[MonitorInfo]) -> Option<bool> {
        let remote = monitors.iter().find(|m| m.index == self.remote_monitor)?;
        let inside = x >= remote.x && x < remote.x + remote.width as i32
            && y >= remote.y && y < remote.y + remote.height as i32;
        if inside != self.cursor_on_remote { self.cursor_on_remote = inside; Some(inside) }
        else { None }
    }

    pub fn is_on_remote(&self) -> bool { self.cursor_on_remote }
}
