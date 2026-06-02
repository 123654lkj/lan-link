import sys
path = r"G:\codex-AI-tools\lan-link\crates\daemon\src\main.rs"
with open(path, "r", encoding="utf-8") as f:
    content = f.read()

old = '''fn handle_input_linux(data: &[u8], peer: SocketAddr) {
    use lan_link_input::InputInjector;
    let mut injector = LinuxInputInjector::new();
    if let Ok(ev) = bincode::deserialize::<lan_link_input::MouseEvent>(data) {
        debug!("Mouse event from {}: {:?}", peer, ev);
        injector.inject_mouse(&ev);
    } else if let Ok(ev) = bincode::deserialize::<lan_link_input::KeyEvent>(data) {
        debug!("Key event from {}: scancode={}", peer, ev.scancode);
        injector.inject_key(&ev);
    }
}'''

new = '''use std::sync::OnceLock;
use std::sync::Mutex;

static INJECTOR: OnceLock<Mutex<LinuxInputInjector>> = OnceLock::new();

fn injector() -> std::sync::MutexGuard<'static, LinuxInputInjector> {
    INJECTOR.get_or_init(|| Mutex::new(LinuxInputInjector::new()))
        .lock().unwrap()
}

fn handle_input_linux(data: &[u8], peer: SocketAddr) {
    use lan_link_input::InputInjector;
    if let Ok(ev) = bincode::deserialize::<lan_link_input::MouseEvent>(data) {
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
    }
}'''

content = content.replace(old, new)

# Add use std::sync::atomic
content = content.replace(
    "use std::sync::OnceLock;",
    "use std::sync::OnceLock;\nuse std::sync::atomic::AtomicU64;"
)

# Add INJECT_COUNT static
content = content.replace(
    "static INJECTOR: OnceLock<Mutex<LinuxInputInjector>> = OnceLock::new();",
    "static INJECT_COUNT: AtomicU64 = AtomicU64::new(0);\nstatic INJECTOR: OnceLock<Mutex<LinuxInputInjector>> = OnceLock::new();"
)

with open(path, "w", encoding="utf-8", newline="\n") as f:
    f.write(content)
print("Patched. Size: %d" % len(content))
if "INJECT_COUNT" in content:
    print("OK: INJECT_COUNT added")
if "OnceLock" in content:
    print("OK: OnceLock added")