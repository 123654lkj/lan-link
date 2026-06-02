from pathlib import Path

Path(r"G:\codex-AI-tools\lan-link\crates\input\Cargo.toml").write_text("""\
[package]
name = "lan-link-input"
version = "0.1.0"
edition = "2024"

[dependencies]
lan-link-protocol = { path = "../protocol" }
tracing = "0.1"
bitflags = "2"

[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = [
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_UI_WindowsAndMessaging",
    "Win32_Graphics_Gdi",
    "Win32_Foundation",
] }

[target.'cfg(linux)'.dependencies]
# evdev-rs, uinput for Linux
""", encoding="utf-8")
print("input Cargo.toml updated")
