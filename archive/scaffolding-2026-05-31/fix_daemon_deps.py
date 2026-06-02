from pathlib import Path

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
bytes = "1"
rand = "0.8"
bincode = "1"
""", encoding="utf-8")
print("daemon deps updated")
