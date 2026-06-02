import pathlib
p = pathlib.Path(r'G:\codex-AI-tools\lan-link\crates\daemon\src\main.rs')
t = p.read_text('utf-8')

# Wrap the OnceLock static and injector function with cfg(linux)
old_block = '''static INJECT_COUNT: AtomicU64 = AtomicU64::new(0);
static INJECTOR: OnceLock<Mutex<LinuxInputInjector>> = OnceLock::new();

fn injector() -> std::sync::MutexGuard<'static, LinuxInputInjector> {
    INJECTOR.get_or_init(|| Mutex::new(LinuxInputInjector::new()))
        .lock().unwrap()
}'''

new_block = '''#[cfg(target_os = "linux")]
static INJECT_COUNT: AtomicU64 = AtomicU64::new(0);
#[cfg(target_os = "linux")]
static INJECTOR: OnceLock<Mutex<LinuxInputInjector>> = OnceLock::new();

#[cfg(target_os = "linux")]
fn injector() -> std::sync::MutexGuard<'static, LinuxInputInjector> {
    INJECTOR.get_or_init(|| Mutex::new(LinuxInputInjector::new()))
        .lock().unwrap()
}'''

t = t.replace(old_block, new_block)
p.write_text(t, 'utf-8')
print('done')