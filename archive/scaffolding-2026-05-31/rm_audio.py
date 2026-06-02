from pathlib import Path

Path(r"G:\codex-AI-tools\lan-link\Cargo.toml").write_text("""\
[workspace]
resolver = "2"
members = [
    "crates/protocol",
    "crates/video",
    "crates/input",
    "crates/daemon",
    "crates/ctl",
]
""", encoding="utf-8")

# Also remove audio from daemon deps
Path(r"G:\codex-AI-tools\lan-link\crates\daemon\Cargo.toml").write_text("""\
[package]
name = "lan-linkd"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "lan-linkd"
path = "src/main.rs"

[dependencies]
lan-link-protocol = { path = "../protocol" }
lan-link-video = { path = "../video" }
lan-link-input = { path = "../input" }
lan-link-shell = { path = "../shell" }
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
tracing-subscriber = "0.3"
clap = { version = "4", features = ["derive"] }
anyhow = "1"
hex = "0.4"
""", encoding="utf-8")

print("audio removed from workspace")
