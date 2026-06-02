from pathlib import Path

base = Path('G:/codex-AI-tools/lan-link')

# Workspace Cargo.toml
(base / 'Cargo.toml').write_text(
    '[workspace]\n'
    'resolver = "2"\n'
    'members = [\n'
    '    "crates/protocol",\n'
    '    "crates/audio",\n'
    '    "crates/video",\n'
    '    "crates/input",\n'
    '    "crates/daemon",\n'
    '    "crates/ctl",\n'
    ']\n',
    encoding='utf-8'
)

# protocol crate
(base / 'crates/protocol/Cargo.toml').write_text(
    '[package]\n'
    'name = "lan-link-protocol"\n'
    'version = "0.1.0"\n'
    'edition = "2024"\n'
    '\n'
    '[dependencies]\n'
    'chacha20poly1305 = "0.10"\n'
    'rand = "0.8"\n'
    'thiserror = "2"\n'
    'bytes = "1"\n'
    'tracing = "0.1"\n'
    'bincode = "1"\n'
    'serde = { version = "1", features = ["derive"] }\n',
    encoding='utf-8'
)

print('all done')
