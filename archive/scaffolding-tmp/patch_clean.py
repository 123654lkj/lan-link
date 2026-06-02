# Clean approach: expose write_input_event from lan-link-input crate, and have
# injector track write count internally.
import re
linux_path = r"G:\codex-AI-tools\lan-link\crates\input\src\linux.rs"
main_path = r"G:\codex-AI-tools\lan-link\crates\daemon\src\main.rs"

# 1. linux.rs: make write_input_event pub, return i32
with open(linux_path, "r", encoding="utf-8") as f:
    lc = f.read()
lc = lc.replace(
    "fn write_input_event(fd: i32, ev_type: u16, code: u16, value: i32) {",
    "pub fn write_input_event(fd: i32, ev_type: u16, code: u16, value: i32) -> i32 {"
)
lc = lc.replace(
    """    unsafe { libc::write(fd, buf.as_ptr() as *const libc::c_void, 24); }
}""",
    """    unsafe { libc::write(fd, buf.as_ptr() as *const libc::c_void, 24) }
}"""
)
# Remove the fd accessor we added (revert that)
lc = lc.replace(
    """impl LinuxInputInjector {
    pub fn fd(&self) -> i32 {
        self.uinput_fd.as_ref().map(|f| f.as_raw_fd()).unwrap_or(-1)
    }

    pub fn new() -> Self {""",
    """impl LinuxInputInjector {
    pub fn fd(&self) -> i32 {
        self.uinput_fd.as_ref().map(|f| f.as_raw_fd()).unwrap_or(-1)
    }

    pub fn new() -> Self {"""
)
with open(linux_path, "w", encoding="utf-8", newline="\n") as f:
    f.write(lc)
print("linux.rs updated")

# 2. main.rs: revert to use inj.inject_* but track via custom counter
with open(main_path, "r", encoding="utf-8") as f:
    mc = f.read()

# Remove the write_inject_event helper (we won't need it)
mc = mc.replace("""fn injector() -> std::sync::MutexGuard<'static, LinuxInputInjector> {
    INJECTOR.get_or_init(|| Mutex::new(LinuxInputInjector::new()))
        .lock().unwrap()
}

/// Returns number of bytes successfully written to uinput fd (or -1 on error).
/// Logs a warning every ~5s if writes are failing.
fn write_inject_event(fd: i32, ev_type: u16, code: u16, value: i32) -> i32 {
    let mut buf = [0u8; 24];
    buf[16..18].copy_from_slice(&ev_type.to_ne_bytes());
    buf[18..20].copy_from_slice(&code.to_ne_bytes());
    buf[20..24].copy_from_slice(&value.to_ne_bytes());
    unsafe {
        let ret = libc::write(fd, buf.as_ptr() as *const libc::c_void, 24);
        if ret < 0 {
            // EBADF = 9, EAGAIN = 11. Both mean uinput is broken.
            let err = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
            WRITE_FAIL_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if WRITE_FAIL_COUNT.load(std::sync::atomic::Ordering::Relaxed) == 1 {
                warn!("uinput write failed: errno={}", err);
            }
        }
        ret
    }
}""", """fn injector() -> std::sync::MutexGuard<'static, LinuxInputInjector> {
    INJECTOR.get_or_init(|| Mutex::new(LinuxInputInjector::new()))
        .lock().unwrap()
}""")

# Remove WRITE_FAIL_COUNT
mc = mc.replace(
    "static INJECT_COUNT: AtomicU64 = AtomicU64::new(0);\nstatic WRITE_FAIL_COUNT: AtomicU64 = AtomicU64::new(0);",
    "static INJECT_COUNT: AtomicU64 = AtomicU64::new(0);"
)

# Replace the verbose inject handler with simple version
old = '''    if let Ok(ev) = bincode::deserialize::<lan_link_input::MouseEvent>(data) {
        debug!("Mouse event from {}: {:?}", peer, ev);
        let mut inj = injector();
        let fd = inj.fd();
        if fd >= 0 {
            match &ev {
                lan_link_input::MouseEvent::Move { dx, dy, .. } => {
                    write_inject_event(fd, 2, 0, *dx);
                    write_inject_event(fd, 2, 1, *dy);
                }
                lan_link_input::MouseEvent::Button { button, down } => {
                    let code = match button {
                        lan_link_input::MouseButton::Left => 0x110,
                        lan_link_input::MouseButton::Right => 0x111,
                        lan_link_input::MouseButton::Middle => 0x112,
                        _ => -1,
                    };
                    if code >= 0 {
                        write_inject_event(fd, 1, code, if *down { 1 } else { 0 });
                    }
                }
                lan_link_input::MouseEvent::Wheel { delta, .. } => {
                    write_inject_event(fd, 2, 8, *delta as i32);
                }
            }
            write_inject_event(fd, 0, 0, 0);
        }
        let count = INJECT_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        if count % 50 == 0 {
            let fails = WRITE_FAIL_COUNT.load(std::sync::atomic::Ordering::Relaxed);
            info!("inject_total={} (last mouse: {:?}, write_fails={})", count, ev, fails);
        }
    } else if let Ok(ev) = bincode::deserialize::<lan_link_input::KeyEvent>(data) {
        debug!("Key event from {}: scancode={}", peer, ev.scancode);
        let mut inj = injector();
        let fd = inj.fd();
        if fd >= 0 {
            write_inject_event(fd, 1, ev.scancode, if ev.down { 1 } else { 0 });
            write_inject_event(fd, 0, 0, 0);
        }
        let count = INJECT_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        if count % 50 == 0 {
            let fails = WRITE_FAIL_COUNT.load(std::sync::atomic::Ordering::Relaxed);
            info!("inject_total={} (last key: scancode={}, write_fails={})", count, ev.scancode, fails);
        }
    } else {
        warn!("input deserialize failed from {}: {} bytes", peer, data.len());
    }'''
new = '''    if let Ok(ev) = bincode::deserialize::<lan_link_input::MouseEvent>(data) {
        debug!("Mouse event from {}: {:?}", peer, ev);
        let mut inj = injector();
        inj.inject_mouse(&ev);
        let count = INJECT_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        if count % 50 == 0 { info!("inject_total={} (last mouse: {:?})", count, ev); }
    } else if let Ok(ev) = bincode::deserialize::<lan_link_input::KeyEvent>(data) {
        debug!("Key event from {}: scancode={}", peer, ev.scancode);
        let mut inj = injector();
        inj.inject_key(&ev);
        let count = INJECT_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        if count % 50 == 0 { info!("inject_total={} (last key: scancode={})", count, ev.scancode); }
    } else {
        warn!("input deserialize failed from {}: {} bytes", peer, data.len());
    }'''
mc = mc.replace(old, new)

# Remove the unused input_injector in main loop
mc = mc.replace(
    """    #[cfg(target_os = "linux")]
    let mut input_injector = LinuxInputInjector::new();
""",
    ""
)

with open(main_path, "w", encoding="utf-8", newline="\n") as f:
    f.write(mc)
print("main.rs cleaned")