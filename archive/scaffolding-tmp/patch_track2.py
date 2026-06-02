path = r"G:\codex-AI-tools\lan-link\crates\daemon\src\main.rs"
with open(path, "r", encoding="utf-8") as f:
    c = f.read()

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
        let bytes = inj.inject_mouse(&ev);
        let count = INJECT_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        if bytes != 48 && bytes > 0 { warn!("uinput mouse inject short write: {} bytes", bytes); }
        if count % 50 == 0 || bytes < 0 { info!("inject_total={} bytes={} (last mouse: {:?})", count, bytes, ev); }
    } else if let Ok(ev) = bincode::deserialize::<lan_link_input::KeyEvent>(data) {
        debug!("Key event from {}: scancode={}", peer, ev.scancode);
        let mut inj = injector();
        let bytes = inj.inject_key(&ev);
        let count = INJECT_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        if bytes != 48 && bytes > 0 { warn!("uinput key inject short write: {} bytes", bytes); }
        if count % 50 == 0 || bytes < 0 { info!("inject_total={} bytes={} (last key: scancode={})", count, bytes, ev.scancode); }
    } else {
        warn!("input deserialize failed from {}: {} bytes", peer, data.len());
    }'''
c = c.replace(old, new)
with open(path, "w", encoding="utf-8", newline="\n") as f:
    f.write(c)
print("daemon now tracks bytes written")