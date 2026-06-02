from pathlib import Path

cargo_dir = Path(r"G:\codex-AI-tools\lan-link\.cargo")
cargo_dir.mkdir(exist_ok=True)
(cargo_dir / "config.toml").write_text("""\
[target.x86_64-pc-windows-msvc]
linker = "rust-lld"
rustflags = ["-C", "link-arg=/NODEFAULTLIB:libcmt"]
""", encoding="utf-8")
print("config updated for MSVC")
