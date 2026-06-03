//! Linux input: evdev capture + uinput injection.
use crate::{InputCapture, InputInjector, KeyEvent, MonitorInfo, MouseButton, MouseEvent};
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::unix::io::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::time::Duration;

// ===== Keyboard capture via evdev =====

pub struct LinuxInputCapture {
    kb_devices: Vec<File>,
    mouse_devices: Vec<File>,
    last_x: i32,
    last_y: i32,
}

impl LinuxInputCapture {
    pub fn new() -> Self {
        let (kb, mouse) = find_input_devices();
        Self { kb_devices: kb, mouse_devices: mouse, last_x: 0, last_y: 0 }
    }
}

/// Find keyboard and mouse event devices under /dev/input/
fn find_input_devices() -> (Vec<File>, Vec<File>) {
    let mut kb = vec![];
    let mut mouse = vec![];
    let dir = match std::fs::read_dir("/dev/input") {
        Ok(d) => d,
        Err(_) => return (kb, mouse),
    };

    for entry in dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("event") {
            continue;
        }
        let path = entry.path();

        // Check device capabilities via EVIOCGBIT ioctls
        if let Ok(f) = std::fs::File::open(&path) {
            let fd = f.as_raw_fd();
            let mut ev_bits = [0u8; 32]; // EV_MAX/8
            let mut key_bits = [0u8; 96]; // KEY_MAX/8
            let mut rel_bits = [0u8; 32]; // REL_MAX/8

            // EVIOCGBIT(0, ...) = 0x80404520 + (0 * size)
            // Simplified: use libc::ioctl
            unsafe {
                if libc::ioctl(fd, 0x80404520, ev_bits.as_mut_ptr()) >= 0 {
                    // Check EV_KEY bit (bit 1)
                    let has_keys = (ev_bits[0] & 0x02) != 0;
                    // Check EV_REL bit (bit 2)
                    let has_rel = (ev_bits[0] & 0x04) != 0;

                    if has_keys {
                        // EVIOCGBIT(EV_KEY, ...) = 0x80404521
                        unsafe {
                            libc::ioctl(fd, 0x80404521, key_bits.as_mut_ptr());
                        }
                        // Check if it has keyboard keys (KEY_A=30, KEY_ESC=1, etc.)
                        let is_kb = (key_bits[1] & 0x02) != 0 // KEY_ESC
                            || (key_bits[3] & 0xC0) != 0; // KEY_A area
                        if is_kb {
                            kb.push(f);
                            continue;
                        }
                    }
                    if has_rel {
                        // EVIOCGBIT(EV_REL, ...) = 0x80404522
                        unsafe {
                            libc::ioctl(fd, 0x80404522, rel_bits.as_mut_ptr());
                        }
                        // REL_X=0, REL_Y=1
                        let has_mouse = (rel_bits[0] & 0x03) != 0;
                        if has_mouse {
                            mouse.push(f);
                        }
                    }
                }
            }
        }
    }
    (kb, mouse)
}

impl InputCapture for LinuxInputCapture {
    fn poll_keys(&mut self) -> Vec<KeyEvent> {
        let mut events = vec![];
        let mut buf = [0u8; 24]; // input_event is 24 bytes on 64-bit

        for dev in &mut self.kb_devices {
            loop {
                match dev.read(&mut buf) {
                    Ok(24) => {
                        // Parse input_event
                        let _tv_sec = i64::from_ne_bytes(buf[0..8].try_into().unwrap());
                        let _tv_usec = i64::from_ne_bytes(buf[8..16].try_into().unwrap());
                        let ev_type = u16::from_ne_bytes(buf[16..18].try_into().unwrap());
                        let code = u16::from_ne_bytes(buf[18..20].try_into().unwrap());
                        let value = i32::from_ne_bytes(buf[20..24].try_into().unwrap());

                        if ev_type == 1 {
                            // EV_KEY
                            events.push(KeyEvent {
                                down: value != 0,
                                scancode: code,
                                vk: code,
                                modifiers: crate::Modifiers::empty(),
                            });
                        }
                    }
                    Ok(_) => break,
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => break,
                }
            }
        }
        events
    }

    fn poll_mouse(&mut self) -> Vec<MouseEvent> {
        let mut events = vec![];
        let mut buf = [0u8; 24];
        let mut dx = 0i32;
        let mut dy = 0i32;

        for dev in &mut self.mouse_devices {
            loop {
                match dev.read(&mut buf) {
                    Ok(24) => {
                        let ev_type = u16::from_ne_bytes(buf[16..18].try_into().unwrap());
                        let code = u16::from_ne_bytes(buf[18..20].try_into().unwrap());
                        let value = i32::from_ne_bytes(buf[20..24].try_into().unwrap());

                        match ev_type {
                            2 => {
                                // EV_REL
                                match code {
                                    0 => dx += value, // REL_X
                                    1 => dy += value, // REL_Y
                                    8 => events.push(MouseEvent::Wheel { delta: value as i16, horizontal: false }),
                                    _ => {}
                                }
                            }
                            1 => {
                                // EV_KEY (buttons)
                                let button = match code {
                                    0x110 => MouseButton::Left,
                                    0x111 => MouseButton::Right,
                                    0x112 => MouseButton::Middle,
                                    _ => continue,
                                };
                                events.push(MouseEvent::Button { button, down: value != 0 });
                            }
                            _ => {}
                        }
                    }
                    Ok(_) => break,
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => break,
                }
            }
        }

        if dx != 0 || dy != 0 {
            events.push(MouseEvent::Move { dx, dy, absolute: false });
        }
        events
    }

    fn cursor_pos(&self) -> (i32, i32) {
        (self.last_x, self.last_y)
    }

    fn monitors(&self) -> Vec<MonitorInfo> {
        enumerate_drm_monitors()
    }
}

// ===== DRM monitor enumeration =====
fn enumerate_drm_monitors() -> Vec<MonitorInfo> {
    let mut monitors = vec![];
    let dir = match std::fs::read_dir("/sys/class/drm") {
        Ok(d) => d,
        Err(_) => return monitors,
    };

    for entry in dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("card") || !name.contains('-') {
            continue;
        }

        let status_path = entry.path().join("status");
        let status = std::fs::read_to_string(&status_path)
            .unwrap_or_default()
            .trim()
            .to_string();

        if status != "connected" {
            continue;
        }

        // Read modes to get resolution
        let modes_path = entry.path().join("modes");
        let modes = std::fs::read_to_string(&modes_path).unwrap_or_default();
        let first_mode = modes.lines().next().unwrap_or("0x0");
        let parts: Vec<&str> = first_mode.split('x').collect();
        let width = parts.first().and_then(|s| s.parse().ok()).unwrap_or(0u32);
        let height = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0u32);

        monitors.push(MonitorInfo {
            index: monitors.len() as u32,
            name: name.clone(),
            x: 0,
            y: 0,
            width,
            height,
            is_primary: name.contains("eDP"),
        });
    }
    monitors
}

// ===== uinput injection =====
pub struct LinuxInputInjector {
    uinput_fd: Option<std::fs::File>,
}

// uinput ioctl constants
const UI_SET_EVBIT: u64 = 0x40045564;
const UI_SET_KEYBIT: u64 = 0x40045565;
const UI_SET_RELBIT: u64 = 0x40045566;
const UI_SET_ABSBIT: u64 = 0x40045567;
const UI_DEV_CREATE: u64 = 0x5501;
const UI_DEV_DESTROY: u64 = 0x5502;

#[repr(C)]
struct UinputUserDev {
    name: [u8; 80],
    id: InputId,
    ff_effects_max: u32,
    absmax: [i32; 64],
    absmin: [i32; 64],
    absfuzz: [i32; 64],
    absflat: [i32; 64],
}

#[repr(C)]
struct InputId {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
}

impl LinuxInputInjector {
    pub fn fd(&self) -> i32 {
        self.uinput_fd.as_ref().map(|f| f.as_raw_fd()).unwrap_or(-1)
    }

    pub fn new() -> Self {
        let fd = OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open("/dev/uinput");

        let uinput_fd = match fd {
            Ok(f) => {
                let raw = f.as_raw_fd();
                // Configure device capabilities
                unsafe {
                    // EV_KEY, EV_REL, EV_SYN, EV_MSC
                    libc::ioctl(raw, UI_SET_EVBIT, 0x01); // EV_KEY
                    libc::ioctl(raw, UI_SET_EVBIT, 0x02); // EV_REL
                    libc::ioctl(raw, UI_SET_EVBIT, 0x00); // EV_SYN

                    // Key bits: all keys
                    for i in 0..768u16 {
                        libc::ioctl(raw, UI_SET_KEYBIT, i as u64);
                    }
                    // Rel bits: X, Y, wheel, hwheel
                    libc::ioctl(raw, UI_SET_RELBIT, 0); // REL_X
                    libc::ioctl(raw, UI_SET_RELBIT, 1); // REL_Y
                    libc::ioctl(raw, UI_SET_RELBIT, 8); // REL_WHEEL
                }

                // Create device
                let mut dev = UinputUserDev {
                    name: [0u8; 80],
                    id: InputId { bustype: 0x03, vendor: 0x1234, product: 0x5678, version: 1 },
                    ff_effects_max: 0,
                    absmax: [0i32; 64],
                    absmin: [0i32; 64],
                    absfuzz: [0i32; 64],
                    absflat: [0i32; 64],
                };
                let name_bytes = b"lan-link-kvm";
                dev.name[..name_bytes.len()].copy_from_slice(name_bytes);

                unsafe {
                    let ptr = &dev as *const UinputUserDev as *const libc::c_void;
                    libc::write(raw, ptr, std::mem::size_of::<UinputUserDev>());
                    libc::ioctl(raw, UI_DEV_CREATE, 0);
                }

                Some(f)
            }
            Err(_) => None,
        };

        Self { uinput_fd }
    }
}

impl InputInjector for LinuxInputInjector {
    fn inject_key(&mut self, event: &KeyEvent) -> isize {
        let mut total = 0;
        if let Some(ref f) = self.uinput_fd {
            let fd = f.as_raw_fd();
            total += write_input_event(fd, 1, event.scancode, if event.down { 1 } else { 0 });
            total += write_input_event(fd, 0, 0, 0);
        }
        total
    }

    fn inject_mouse(&mut self, event: &MouseEvent) -> isize {
        let mut total = 0;
        if let Some(ref f) = self.uinput_fd {
            let fd = f.as_raw_fd();
            match event {
                MouseEvent::Move { dx, dy, .. } => {
                    total += write_input_event(fd, 2, 0, *dx); // REL_X
                    total += write_input_event(fd, 2, 1, *dy); // REL_Y
                }
                MouseEvent::Button { button, down } => {
                    let code = match button {
                        MouseButton::Left => 0x110,
                        MouseButton::Right => 0x111,
                        MouseButton::Middle => 0x112,
                        _ => return total,
                    };
                    total += write_input_event(fd, 1, code, if *down { 1 } else { 0 });
                }
                MouseEvent::Wheel { delta, .. } => {
                    total += write_input_event(fd, 2, 8, *delta as i32);
                }
            }
            total += write_input_event(fd, 0, 0, 0); // EV_SYN
        }
        total
    }

    fn set_cursor_pos(&mut self, _x: i32, _y: i32) {
        // uinput doesn't support absolute cursor positioning directly.
        // For KVM we use relative mouse movement.
    }
}

pub fn write_input_event(fd: i32, ev_type: u16, code: u16, value: i32) -> isize {
    // input_event: timeval(16) + type(2) + code(2) + value(4) = 24 bytes
    let mut buf = [0u8; 24];
    buf[16..18].copy_from_slice(&ev_type.to_ne_bytes());
    buf[18..20].copy_from_slice(&code.to_ne_bytes());
    buf[20..24].copy_from_slice(&value.to_ne_bytes());
    unsafe { libc::write(fd, buf.as_ptr() as *const libc::c_void, 24) }
}


impl Drop for LinuxInputInjector {
    fn drop(&mut self) {
        if let Some(ref f) = self.uinput_fd {
            unsafe {
                libc::ioctl(f.as_raw_fd(), UI_DEV_DESTROY, 0);
            }
        }
    }
}
