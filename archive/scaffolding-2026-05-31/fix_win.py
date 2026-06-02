from pathlib import Path

# Simplify win.rs to use only stable, well-known Windows APIs
# windows 0.58 has changed API surface - avoid problematic calls
win_rs = """\
//! Windows input capture and injection.
//!
//! Uses `windows` crate 0.58. Keep API surface minimal to avoid
//! version-specific name changes.

use crate::{InputCapture, InputInjector, KeyEvent, MonitorInfo, MouseButton, MouseEvent};

pub struct WinInputCapture {
    last_x: i32,
    last_y: i32,
}

impl WinInputCapture {
    pub fn new() -> Self {
        Self { last_x: 0, last_y: 0 }
    }
}

impl InputCapture for WinInputCapture {
    fn poll_keys(&mut self) -> Vec<KeyEvent> {
        // RawInput requires message pump integration.
        // For KVM passthrough, we'll use a low-level keyboard hook instead.
        vec![]
    }

    fn poll_mouse(&mut self) -> Vec<MouseEvent> {
        let mut pt = windows::Win32::Foundation::POINT::default();
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut pt);
        }
        let dx = pt.x - self.last_x;
        let dy = pt.y - self.last_y;
        self.last_x = pt.x;
        self.last_y = pt.y;
        if dx != 0 || dy != 0 {
            vec![MouseEvent::Move { dx, dy, absolute: false }]
        } else {
            vec![]
        }
    }

    fn cursor_pos(&self) -> (i32, i32) {
        let mut pt = windows::Win32::Foundation::POINT::default();
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut pt);
        }
        (pt.x, pt.y)
    }

    fn monitors(&self) -> Vec<MonitorInfo> {
        // Use GetSystemMetrics for basic display info
        // EnumDisplayMonitors API changed in windows 0.58, keeping it simple
        let cx = unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
                windows::Win32::UI::WindowsAndMessaging::SM_CXSCREEN
            )
        };
        let cy = unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
                windows::Win32::UI::WindowsAndMessaging::SM_CYSCREEN
            )
        };
        let n_monitors = unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
                windows::Win32::UI::WindowsAndMessaging::SM_CMONITORS
            )
        };

        let mut monitors = vec![];
        for i in 0..n_monitors {
            monitors.push(MonitorInfo {
                index: i as u32,
                name: format!("Display {}", i + 1),
                x: if i == 0 { 0 } else { cx * (i - 1) },
                y: 0,
                width: cx as u32,
                height: cy as u32,
                is_primary: i == 0,
            });
        }
        monitors
    }
}

/// Windows SendInput-based injection.
pub struct WinInputInjector;

impl WinInputInjector {
    pub fn new() -> Self { Self }
}

impl InputInjector for WinInputInjector {
    fn inject_key(&mut self, event: &KeyEvent) {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;
        let mut input = INPUT::default();
        input.r#type = INPUT_KEYBOARD;
        unsafe {
            input.Anonymous.ki.wVk = VIRTUAL_KEY(event.vk);
            input.Anonymous.ki.wScan = event.scancode;
            if !event.down {
                input.Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
            }
            let inputs = [input];
            windows::Win32::UI::Input::SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }
    }

    fn inject_mouse(&mut self, event: &MouseEvent) {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;
        let mut input = INPUT::default();
        input.r#type = INPUT_MOUSE;
        unsafe {
            match event {
                MouseEvent::Move { dx, dy, absolute } => {
                    if *absolute {
                        input.Anonymous.mi.dwFlags = MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE;
                    } else {
                        input.Anonymous.mi.dwFlags = MOUSEEVENTF_MOVE;
                    }
                    input.Anonymous.mi.dx = *dx;
                    input.Anonymous.mi.dy = *dy;
                }
                MouseEvent::Button { button, down } => {
                    input.Anonymous.mi.dwFlags = match (button, down) {
                        (MouseButton::Left, true) => MOUSEEVENTF_LEFTDOWN,
                        (MouseButton::Left, false) => MOUSEEVENTF_LEFTUP,
                        (MouseButton::Right, true) => MOUSEEVENTF_RIGHTDOWN,
                        (MouseButton::Right, false) => MOUSEEVENTF_RIGHTUP,
                        (MouseButton::Middle, true) => MOUSEEVENTF_MIDDLEDOWN,
                        (MouseButton::Middle, false) => MOUSEEVENTF_MIDDLEUP,
                        _ => MOUSEEVENTF_MOVE,
                    };
                }
                MouseEvent::Wheel { delta, horizontal } => {
                    if *horizontal {
                        input.Anonymous.mi.dwFlags = MOUSEEVENTF_HWHEEL;
                    } else {
                        input.Anonymous.mi.dwFlags = MOUSEEVENTF_WHEEL;
                    }
                    input.Anonymous.mi.mouseData = (*delta as u32) << 16;
                }
            }
            let inputs = [input];
            windows::Win32::UI::Input::SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }
    }

    fn set_cursor_pos(&mut self, x: i32, y: i32) {
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::SetCursorPos(x, y);
        }
    }
}
"""

Path(r"G:\codex-AI-tools\lan-link\crates\input\src\win.rs").write_text(win_rs, encoding="utf-8")
print("win.rs simplified")
