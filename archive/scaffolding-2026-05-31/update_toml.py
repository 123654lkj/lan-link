from pathlib import Path

# protocol Cargo.toml
Path(r"G:\codex-AI-tools\lan-link\crates\protocol\Cargo.toml").write_text("""\
[package]
name = "lan-link-protocol"
version = "0.1.0"
edition = "2024"

[dependencies]
chacha20poly1305 = "0.10"
rand = "0.8"
thiserror = "2"
bytes = "1"
tracing = "0.1"
bincode = "1"
serde = { version = "1", features = ["derive"] }
bitflags = "2"
""", encoding="utf-8")

# daemon Cargo.toml
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
lan-link-audio = { path = "../audio" }
lan-link-video = { path = "../video" }
lan-link-input = { path = "../input" }
lan-link-shell = { path = "../shell" }
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
tracing-subscriber = "0.3"
clap = { version = "4", features = ["derive"] }
""", encoding="utf-8")

# ctl Cargo.toml
Path(r"G:\codex-AI-tools\lan-link\crates\ctl\Cargo.toml").write_text("""\
[package]
name = "lan-linkctl"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "lan-linkctl"
path = "src/main.rs"

[dependencies]
lan-link-protocol = { path = "../protocol" }
tokio = { version = "1", features = ["full"] }
clap = { version = "4", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = "0.3"
""", encoding="utf-8")

# audio Cargo.toml
Path(r"G:\codex-AI-tools\lan-link\crates\audio\Cargo.toml").write_text("""\
[package]
name = "lan-link-audio"
version = "0.1.0"
edition = "2024"

[dependencies]
lan-link-protocol = { path = "../protocol" }
cpal = "0.15"
audiopus = "0.3"
tracing = "0.1"
""", encoding="utf-8")

# video Cargo.toml
Path(r"G:\codex-AI-tools\lan-link\crates\video\Cargo.toml").write_text("""\
[package]
name = "lan-link-video"
version = "0.1.0"
edition = "2024"

[dependencies]
lan-link-protocol = { path = "../protocol" }
tracing = "0.1"
""", encoding="utf-8")

# input Cargo.toml
Path(r"G:\codex-AI-tools\lan-link\crates\input\Cargo.toml").write_text("""\
[package]
name = "lan-link-input"
version = "0.1.0"
edition = "2024"

[dependencies]
lan-link-protocol = { path = "../protocol" }
enigo = { version = "0.3", default-features = false }
tracing = "0.1"
""", encoding="utf-8")

# shell Cargo.toml
Path(r"G:\codex-AI-tools\lan-link\crates\shell\Cargo.toml").write_text("""\
[package]
name = "lan-link-shell"
version = "0.1.0"
edition = "2024"

[dependencies]
lan-link-protocol = { path = "../protocol" }
tokio = { version = "1", features = ["full"] }
portable-pty = "0.8"
tracing = "0.1"
""", encoding="utf-8")

print("all Cargo.toml files updated")
