from pathlib import Path
Path(r"G:\codex-AI-tools\lan-link\crates\audio\Cargo.toml").write_text("""\
[package]
name = "lan-link-audio"
version = "0.1.0"
edition = "2024"

[dependencies]
lan-link-protocol = { path = "../protocol" }
cpal = "0.15"
audiopus = "0.3.0-rc.0"
tracing = "0.1"
""", encoding="utf-8")
print("fixed")
