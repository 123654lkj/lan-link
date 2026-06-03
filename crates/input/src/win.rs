//! Windows input capture and injection.

use crate::{InputCapture, InputInjector, KeyEvent, MonitorInfo, MouseButton, MouseEvent};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub struct WinInputCapture { last_x: i32, last_y: i32 }

impl WinInputCapture {
    pub fn new() -> Self { Self { last_x: 0, last_y: 0 } }
}

impl InputCapture for WinInputCapture {
    fn poll_keys(&mut self) -> Vec<KeyEvent> { vec![] }

    fn poll_mouse(&mut self) -> Vec<MouseEvent> {
        let mut pt = windows::Win32::Foundation::POINT::default();
        unsafe { let _ = GetCursorPos(&mut pt); }
        let dx = pt.x - self.last_x;
        let dy = pt.y - self.last_y;
        self.last_x = pt.x; self.last_y = pt.y;
        if dx != 0 || dy != 0 {
            vec![MouseEvent::Move { dx, dy, absolute: false }]
        } else { vec![] }
    }

    fn cursor_pos(&self) -> (i32, i32) {
        let mut pt = windows::Win32::Foundation::POINT::default();
        unsafe { let _ = GetCursorPos(&mut pt); }
        (pt.x, pt.y)
    }

    fn monitors(&self) -> Vec<MonitorInfo> {
        let cx = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let cy = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        let n = unsafe { GetSystemMetrics(SM_CMONITORS) };
        (0..n).map(|i| MonitorInfo {
            index: i as u32, name: format!("Display {}", i + 1),
            x: if i == 0 { 0 } else { cx * (i - 1) }, y: 0,
            width: cx as u32, height: cy as u32, is_primary: i == 0,
        }).collect()
    }
}

pub struct WinInputInjector;

impl WinInputInjector {
    pub fn new() -> Self { Self }
}

impl InputInjector for WinInputInjector {
    fn inject_key(&mut self, event: &KeyEvent) -> isize {
        let mut input = INPUT::default();
        input.r#type = INPUT_KEYBOARD;
        unsafe {
            input.Anonymous.ki.wVk = VIRTUAL_KEY(event.vk);
            input.Anonymous.ki.wScan = event.scancode;
            if !event.down { input.Anonymous.ki.dwFlags = KEYEVENTF_KEYUP; }
            let inputs = [input];
            let rc = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
            if rc == 0 {
                log::warn!("SendInput (key) returned 0, injection may have failed");
            }
        }
        0
    }

    fn inject_mouse(&mut self, event: &MouseEvent) -> isize {
        let mut input = INPUT::default();
        input.r#type = INPUT_MOUSE;
        unsafe {
            match event {
                MouseEvent::Move { dx, dy, absolute } => {
                    input.Anonymous.mi.dwFlags = if *absolute { MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE } else { MOUSEEVENTF_MOVE };
                    input.Anonymous.mi.dx = *dx; input.Anonymous.mi.dy = *dy;
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
                    input.Anonymous.mi.dwFlags = if *horizontal { MOUSEEVENTF_HWHEEL } else { MOUSEEVENTF_WHEEL };
                    input.Anonymous.mi.mouseData = (*delta as u32) << 16;
                }
            }
            let inputs = [input];
            let rc = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
            if rc == 0 {
                log::warn!("SendInput (mouse) returned 0, injection may have failed");
            }
        }
        0
    }

    fn set_cursor_pos(&mut self, x: i32, y: i32) {
        unsafe { let _ = SetCursorPos(x, y); }
    }
}
