from pathlib import Path

cargo_dir = Path(r"G:\codex-AI-tools\lan-link\.cargo")
(cargo_dir / "config.toml").write_text("""\
[target.x86_64-pc-windows-gnu]
linker = "rust-lld"
rustflags = ["-C", "link-arg=--no-rosegment"]
""", encoding="utf-8")
print("config set for GNU + rust-lld")
