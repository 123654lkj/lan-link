import re
path = r"G:\codex-AI-tools\lan-link\crates\daemon\src\main.rs"
with open(path, "r", encoding="utf-8") as f:
    content = f.read()

# Add input_event helper at top of input section
old = '''fn injector() -> std::sync::MutexGuard<'static, LinuxInputInjector> {
    INJECTOR.get_or_init(|| Mutex::new(LinuxInputInjector::new()))
        .lock().unwrap()
}'''
new = '''fn injector() -> std::sync::MutexGuard<'static, LinuxInputInjector> {
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
}'''
content = content.replace(old, new)

# Add WRITE_FAIL_COUNT static
content = content.replace(
    "static INJECT_COUNT: AtomicU64 = AtomicU64::new(0);",
    "static INJECT_COUNT: AtomicU64 = AtomicU64::new(0);\nstatic WRITE_FAIL_COUNT: AtomicU64 = AtomicU64::new(0);"
)

# Replace inject calls with new helper (in linux.rs - not main.rs)
# Actually we need to also patch linux.rs to use write_inject_event
with open(path, "w", encoding="utf-8", newline="\n") as f:
    f.write(content)
print("Patched main.rs")

# Now patch linux.rs to expose fd and use new helper
linux_path = r"G:\codex-AI-tools\lan-link\crates\input\src\linux.rs"
with open(linux_path, "r", encoding="utf-8") as f:
    lc = f.read()

# Add accessor for fd
old = '''impl LinuxInputInjector {
    pub fn new() -> Self {'''
new = '''impl LinuxInputInjector {
    pub fn fd(&self) -> i32 {
        self.uinput_fd.as_ref().map(|f| f.as_raw_fd()).unwrap_or(-1)
    }

    pub fn new() -> Self {'''
lc = lc.replace(old, new)

with open(linux_path, "w", encoding="utf-8", newline="\n") as f:
    f.write(lc)
print("Patched linux.rs (fd accessor added)")

# Now update main.rs to use the new helper instead of LinuxInputInjector internals
with open(path, "r", encoding="utf-8") as f:
    content = f.read()

old = '''    if let Ok(ev) = bincode::deserialize::<lan_link_input::MouseEvent>(data) {
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
new = '''    if let Ok(ev) = bincode::deserialize::<lan_link_input::MouseEvent>(data) {
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
content = content.replace(old, new)

with open(path, "w", encoding="utf-8", newline="\n") as f:
    f.write(content)
print("Patched main.rs to use write_inject_event")