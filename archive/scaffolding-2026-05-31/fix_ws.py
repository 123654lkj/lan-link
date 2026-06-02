from pathlib import Path

Path(r"G:\codex-AI-tools\lan-link\Cargo.toml").write_text("""\
[workspace]
resolver = "2"
members = [
    "crates/protocol",
    "crates/shell",
    "crates/video",
    "crates/input",
    "crates/daemon",
    "crates/ctl",
]
""", encoding="utf-8")
print("shell added back")
