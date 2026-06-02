from pathlib import Path

# Fix 1: video Cargo.toml - add anyhow
Path(r"G:\codex-AI-tools\lan-link\crates\video\Cargo.toml").write_text("""\
[package]
name = "lan-link-video"
version = "0.1.0"
edition = "2024"

[dependencies]
lan-link-protocol = { path = "../protocol" }
tracing = "0.1"
anyhow = "1"
""", encoding="utf-8")

# Fix 2: input/win.rs - fix Windows API names for windows 0.58
win_rs = """\
//! Windows input capture and injection using Win32 APIs.
//!
//! Uses the `windows` crate 0.58. API names may differ from older versions.

use crate::{InputCapture, InputInjector, KeyEvent, MonitorInfo, MouseButton, MouseEvent};
use windows::Win32::UI::Input::KeyboardAndMouse;
use windows::Win32::UI::WindowsAndMessaging;
use windows::Win32::Graphics::Gdi;
use windows::Win32::Foundation;
use windows::core::HWND;

/// Windows RawInput-based capture (stub for now).
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
        vec![]
    }

    fn poll_mouse(&mut self) -> Vec<MouseEvent> {
        let mut pt = Foundation::POINT::default();
        let events = unsafe {
            if WindowsAndMessaging::GetCursorPos(&mut pt).is_ok() {
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
            }
        };
        events
    }

    fn cursor_pos(&self) -> (i32, i32) {
        let mut pt = Foundation::POINT::default();
        unsafe {
            let _ = WindowsAndMessaging::GetCursorPos(&mut pt);
        }
        (pt.x, pt.y)
    }

    fn monitors(&self) -> Vec<MonitorInfo> {
        let mut monitors = vec![];
        unsafe {
            let _ = Gdi::EnumDisplayMonitors(
                Gdi::HDC::default(),
                None,
                Some(monitor_enum_proc),
                Some(Foundation::LPARAM(&mut monitors as *mut _ as isize)),
            );
        }
        monitors
    }
}

unsafe extern "system" fn monitor_enum_proc(
    hmon: Gdi::HMONITOR,
    _hdc: Gdi::HDC,
    _rect: *mut Foundation::RECT,
    lparam: Foundation::LPARAM,
) -> Foundation::BOOL {
    let monitors = &mut *(lparam.0 as *mut Vec<MonitorInfo>);

    let mut info = Gdi::MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<Gdi::MONITORINFOEXW>() as u32;

    let result = Gdi::GetMonitorInfoW(
        hmon,
        &mut info as *mut _ as *mut Gdi::MONITORINFO,
    );

    if result.is_ok() {
        let name_end = info.szDevice.iter().position(|&c| c == 0).unwrap_or(info.szDevice.len());
        let name = String::from_utf16_lossy(&info.szDevice[..name_end]);

        let rc = info.monitorInfo.rcMonitor;
        let is_primary = (info.monitorInfo.dwFlags & Gdi::MONITORINFOF_PRIMARY) != 0;

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

    Foundation::BOOL::from(true)
}

/// Windows SendInput-based injection.
pub struct WinInputInjector;

impl WinInputInjector {
    pub fn new() -> Self { Self }
}

impl InputInjector for WinInputInjector {
    fn inject_key(&mut self, event: &KeyEvent) {
        let mut input = KeyboardAndMouse::INPUT::default();
        input.r#type = KeyboardAndMouse::INPUT_KEYBOARD;
        unsafe {
            input.Anonymous.ki.wVk = KeyboardAndMouse::VIRTUAL_KEY(event.vk);
            input.Anonymous.ki.wScan = event.scancode;
            if !event.down {
                input.Anonymous.ki.dwFlags = KeyboardAndMouse::KEYEVENTF_KEYUP;
            }
            let inputs = [input];
            let _ = WindowsAndMessaging::SendInput(&inputs, std::mem::size_of::<KeyboardAndMouse::INPUT>() as i32);
        }
    }

    fn inject_mouse(&mut self, event: &MouseEvent) {
        let mut input = KeyboardAndMouse::INPUT::default();
        input.r#type = KeyboardAndMouse::INPUT_MOUSE;
        unsafe {
            match event {
                MouseEvent::Move { dx, dy, absolute } => {
                    if *absolute {
                        input.Anonymous.mi.dwFlags = KeyboardAndMouse::MOUSEEVENTF_MOVE | KeyboardAndMouse::MOUSEEVENTF_ABSOLUTE;
                    } else {
                        input.Anonymous.mi.dwFlags = KeyboardAndMouse::MOUSEEVENTF_MOVE;
                    }
                    input.Anonymous.mi.dx = *dx;
                    input.Anonymous.mi.dy = *dy;
                }
                MouseEvent::Button { button, down } => {
                    input.Anonymous.mi.dwFlags = match (button, down) {
                        (MouseButton::Left, true) => KeyboardAndMouse::MOUSEEVENTF_LEFTDOWN,
                        (MouseButton::Left, false) => KeyboardAndMouse::MOUSEEVENTF_LEFTUP,
                        (MouseButton::Right, true) => KeyboardAndMouse::MOUSEEVENTF_RIGHTDOWN,
                        (MouseButton::Right, false) => KeyboardAndMouse::MOUSEEVENTF_RIGHTUP,
                        (MouseButton::Middle, true) => KeyboardAndMouse::MOUSEEVENTF_MIDDLEDOWN,
                        (MouseButton::Middle, false) => KeyboardAndMouse::MOUSEEVENTF_MIDDLEUP,
                        _ => KeyboardAndMouse::MOUSEEVENTF_MOVE, // fallback
                    };
                }
                MouseEvent::Wheel { delta, horizontal } => {
                    if *horizontal {
                        input.Anonymous.mi.dwFlags = KeyboardAndMouse::MOUSEEVENTF_HWHEEL;
                    } else {
                        input.Anonymous.mi.dwFlags = KeyboardAndMouse::MOUSEEVENTF_WHEEL;
                    }
                    input.Anonymous.mi.mouseData = (*delta as u32) << 16;
                }
            }
            let inputs = [input];
            let _ = WindowsAndMessaging::SendInput(&inputs, std::mem::size_of::<KeyboardAndMouse::INPUT>() as i32);
        }
    }

    fn set_cursor_pos(&mut self, x: i32, y: i32) {
        unsafe {
            let _ = WindowsAndMessaging::SetCursorPos(x, y);
        }
    }
}
"""

Path(r"G:\codex-AI-tools\lan-link\crates\input\src\win.rs").write_text(win_rs, encoding="utf-8")

# Fix 3: video capture.rs - remove anyhow usage for now (stub)
Path(r"G:\codex-AI-tools\lan-link\crates\video\src\capture.rs").write_text("""\
//! DXGI Desktop Duplication capture (stub).

use crate::{VideoCapture, VideoFrame};

pub struct DxgiCapture {
    width: u32,
    height: u32,
    frame_count: u64,
}

impl DxgiCapture {
    pub fn new(monitor_index: u32, width: u32, height: u32) -> Self {
        let _ = monitor_index;
        Self { width, height, frame_count: 0 }
    }
}

impl VideoCapture for DxgiCapture {
    fn capture(&mut self) -> Option<VideoFrame> {
        let size = (self.width * self.height * 4) as usize;
        self.frame_count += 1;
        Some(VideoFrame {
            width: self.width,
            height: self.height,
            data: vec![0u8; size],
            format: "bgra".to_string(),
            pts: self.frame_count * 1_000_000 / 60,
        })
    }

    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}
""", encoding="utf-8")

# Fix 4: video encoder.rs
Path(r"G:\codex-AI-tools\lan-link\crates\video\src\encoder.rs").write_text("""\
//! Video encoder (stub).

use crate::{EncodedPacket, VideoConfig, VideoEncoder, VideoFrame};

pub struct NvencEncoder { config: VideoConfig, frame_count: u64 }
impl NvencEncoder {
    pub fn new(config: VideoConfig) -> Self { Self { config, frame_count: 0 } }
}
impl VideoEncoder for NvencEncoder {
    fn encode(&mut self, _frame: &VideoFrame) -> Option<EncodedPacket> { None }
    fn flush(&mut self) -> Vec<EncodedPacket> { vec![] }
}

pub struct SoftwareEncoder { config: VideoConfig }
impl SoftwareEncoder {
    pub fn new(config: VideoConfig) -> Self { Self { config } }
}
impl VideoEncoder for SoftwareEncoder {
    fn encode(&mut self, _frame: &VideoFrame) -> Option<EncodedPacket> { None }
    fn flush(&mut self) -> Vec<EncodedPacket> { vec![] }
}
""", encoding="utf-8")

print("all fixes applied")
