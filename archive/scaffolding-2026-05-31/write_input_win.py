from pathlib import Path

# Windows input implementation
win_input = r"""//! Windows input capture and injection using Win32 APIs.

use crate::{InputCapture, InputInjector, KeyEvent, MonitorInfo, Modifiers, MouseButton, MouseEvent};
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;

/// Windows RawInput-based capture.
pub struct WinInputCapture {
    hwnd: Option<*mut std::ffi::c_void>,
    last_x: i32,
    last_y: i32,
    raw_input_registered: bool,
}

impl WinInputCapture {
    pub fn new() -> Self {
        Self {
            hwnd: None,
            last_x: 0,
            last_y: 0,
            raw_input_registered: false,
        }
    }

    /// Register for raw input messages. Requires a window handle.
    /// Call this after creating a hidden message-only window.
    pub fn register(&mut self, hwnd: *mut std::ffi::c_void) {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;

        let mut rid = [RAWINPUTDEVICE::default(); 2];

        // Mouse
        rid[0].usUsagePage = HID_USAGE_PAGE_GENERIC;
        rid[0].usUsage = HID_USAGE_GENERIC_MOUSE;
        rid[0].dwFlags = RIDEV_INPUTSINK;
        rid[0].hwndTarget = windows::Win32::Foundation::HWND(hwnd as *mut _);

        // Keyboard
        rid[1].usUsagePage = HID_USAGE_PAGE_GENERIC;
        rid[1].usUsage = HID_USAGE_GENERIC_KEYBOARD;
        rid[1].dwFlags = RIDEV_INPUTSINK;
        rid[1].hwndTarget = windows::Win32::Foundation::HWND(hwnd as *mut _);

        unsafe {
            let _ = RegisterRawInputDevices(&rid);
        }

        self.raw_input_registered = true;
        self.hwnd = Some(hwnd);
    }
}

impl InputCapture for WinInputCapture {
    fn poll_keys(&mut self) -> Vec<KeyEvent> {
        // In production, this drains from a lock-free queue filled by the window proc.
        // For now, use GetAsyncKeyState polling for a subset of keys.
        vec![]
    }

    fn poll_mouse(&mut self) -> Vec<MouseEvent> {
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
        let mut pt = windows::Win32::Foundation::POINT::default();
        let events = if unsafe { GetCursorPos(&mut pt) }.is_ok() {
            let dx = pt.x - self.last_x;
            let dy = pt.y - self.last_y;
            self.last_x = pt.x;
            self.last_y = pt.y;
            if dx != 0 || dy != 0 {
                vec![MouseEvent::Move { dx, dy, absolute: false }]
            } else {
                vec![]
            }
        } else {
            vec![]
        };
        events
    }

    fn cursor_pos(&self) -> (i32, i32) {
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
        let mut pt = windows::Win32::Foundation::POINT::default();
        if unsafe { GetCursorPos(&mut pt) }.is_ok() {
            (pt.x, pt.y)
        } else {
            (0, 0)
        }
    }

    fn monitors(&self) -> Vec<MonitorInfo> {
        use windows::Win32::Graphics::Gdi::*;
        let mut monitors = vec![];

        unsafe {
            let _ = EnumDisplayMonitors(
                HDC::default(),
                None,
                Some(monitor_enum_proc),
                windows::Win32::Foundation::LPARAM(&mut monitors as *mut _ as isize),
            );
        }
        monitors
    }
}

unsafe extern "system" fn monitor_enum_proc(
    hmon: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::BOOL {
    let monitors = &mut *(lparam.0 as *mut Vec<MonitorInfo>);

    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

    if GetMonitorInfoW(
        hmon,
        &mut info as *mut _ as *mut MONITORINFO,
    ).is_ok() {
        let name = String::from_utf16_lossy(
            &info.szDevice[..info.szDevice.iter().position(|&c| c == 0).unwrap_or(info.szDevice.len())]
        );

        let rc = info.monitorInfo.rcMonitor;
        let is_primary = (info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY) != 0;

        monitors.push(MonitorInfo {
            index: monitors.len() as u32,
            name,
            x: rc.left,
            y: rc.top,
            width: (rc.right - rc.left) as u32,
            height: (rc.bottom - rc.top) as u32,
            is_primary,
        });
    }

    windows::Win32::Foundation::BOOL::from(true)
}

/// Windows SendInput-based injection.
pub struct WinInputInjector;

impl WinInputInjector {
    pub fn new() -> Self {
        Self
    }
}

impl InputInjector for WinInputInjector {
    fn inject_key(&mut self, event: &KeyEvent) {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;
        use windows::Win32::UI::WindowsAndMessaging::SendInput;

        let mut inputs = [INPUT::default(); 1];
        inputs[0].r#type = INPUT_KEYBOARD;
        inputs[0].Anonymous.ki.wVk = windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(event.vk);
        inputs[0].Anonymous.ki.wScan = event.scancode;

        if !event.down {
            inputs[0].Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
        }

        unsafe {
            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }
    }

    fn inject_mouse(&mut self, event: &MouseEvent) {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;
        use windows::Win32::UI::WindowsAndMessaging::SendInput;

        match event {
            MouseEvent::Move { dx, dy, absolute } => {
                let mut inputs = [INPUT::default(); 1];
                inputs[0].r#type = INPUT_MOUSE;
                if *absolute {
                    inputs[0].Anonymous.mi.dwFlags = MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE;
                    // Absolute coords: 0..65535 mapped to screen
                    inputs[0].Anonymous.mi.dx = *dx;
                    inputs[0].Anonymous.mi.dy = *dy;
                } else {
                    inputs[0].Anonymous.mi.dwFlags = MOUSEEVENTF_MOVE;
                    inputs[0].Anonymous.mi.dx = *dx;
                    inputs[0].Anonymous.mi.dy = *dy;
                }
                unsafe {
                    SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
                }
            }
            MouseEvent::Button { button, down } => {
                let mut inputs = [INPUT::default(); 1];
                inputs[0].r#type = INPUT_MOUSE;
                inputs[0].Anonymous.mi.dwFlags = match button {
                    MouseButton::Left if *down => MOUSEEVENTF_LEFTDOWN,
                    MouseButton::Left => MOUSEEVENTF_LEFTUP,
                    MouseButton::Right if *down => MOUSEEVENTF_RIGHTDOWN,
                    MouseButton::Right => MOUSEEVENTF_RIGHTUP,
                    MouseButton::Middle if *down => MOUSEEVENTF_MIDDLEDOWN,
                    MouseButton::Middle => MOUSEEVENTF_MIDDLEUP,
                    MouseButton::X1 if *down => MOUSEEVENTF_XDOWN,
                    MouseButton::X1 => MOUSEEVENTF_XUP,
                    MouseButton::X2 if *down => MOUSEEVENTF_XDOWN,
                    MouseButton::X2 => MOUSEEVENTF_XUP,
                };
                unsafe {
                    SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
                }
            }
            MouseEvent::Wheel { delta, horizontal } => {
                let mut inputs = [INPUT::default(); 1];
                inputs[0].r#type = INPUT_MOUSE;
                inputs[0].Anonymous.mi.dwFlags = if *horizontal {
                    MOUSEEVENTF_HWHEEL
                } else {
                    MOUSEEVENTF_WHEEL
                };
                inputs[0].Anonymous.mi.mouseData = (*delta as u32) << 16;
                unsafe {
                    SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
                }
            }
        }
    }

    fn set_cursor_pos(&mut self, x: i32, y: i32) {
        use windows::Win32::UI::WindowsAndMessaging::SetCursorPos;
        unsafe {
            let _ = SetCursorPos(x, y);
        }
    }
}
"""

Path(r"G:\codex-AI-tools\lan-link\crates\input\src\win.rs").write_text(win_input, encoding="utf-8")

# Linux input stub
Path(r"G:\codex-AI-tools\lan-link\crates\input\src\linux.rs").write_text("""\
//! Linux input using evdev + uinput (stub).

use crate::{InputCapture, InputInjector, KeyEvent, MonitorInfo, MouseEvent};

pub struct LinuxInputCapture;
impl LinuxInputCapture { pub fn new() -> Self { Self } }
impl InputCapture for LinuxInputCapture {
    fn poll_keys(&mut self) -> Vec<KeyEvent> { vec![] }
    fn poll_mouse(&mut self) -> Vec<MouseEvent> { vec![] }
    fn cursor_pos(&self) -> (i32, i32) { (0, 0) }
    fn monitors(&self) -> Vec<MonitorInfo> { vec![] }
}

pub struct LinuxInputInjector;
impl LinuxInputInjector { pub fn new() -> Self { Self } }
impl InputInjector for LinuxInputInjector {
    fn inject_key(&mut self, _event: &KeyEvent) {}
    fn inject_mouse(&mut self, _event: &MouseEvent) {}
    fn set_cursor_pos(&mut self, _x: i32, _y: i32) {}
}
""", encoding="utf-8")

print("input platform files done")
