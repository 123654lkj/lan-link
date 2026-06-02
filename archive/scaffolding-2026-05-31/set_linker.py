from pathlib import Path

# Create .cargo/config.toml to use rust-lld
cargo_dir = Path(r"G:\codex-AI-tools\lan-link\.cargo")
cargo_dir.mkdir(exist_ok=True)
(cargo_dir / "config.toml").write_text("""\
[target.x86_64-pc-windows-gnu]
linker = "rust-lld"
""", encoding="utf-8")
print("config written")
