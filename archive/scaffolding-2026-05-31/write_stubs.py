from pathlib import Path

# video stub
Path(r"G:\codex-AI-tools\lan-link\crates\video\src\lib.rs").write_text("""\
//! Video engine: screen capture + hardware encode + render.
//!
//! Windows: DXGI desktop duplication + NVENC
//! Linux: DRM/KMS + VA-API

/// Video engine placeholder.
pub struct VideoEngine;

impl VideoEngine {
    pub fn new() -> Self {
        Self
    }
}
""", encoding="utf-8")

# input stub
Path(r"G:\codex-AI-tools\lan-link\crates\input\src\lib.rs").write_text("""\
//! Input engine: keyboard/mouse capture and injection.
//!
//! Cross-platform input handling via enigo.

/// Input engine placeholder.
pub struct InputEngine;

impl InputEngine {
    pub fn new() -> Self {
        Self
    }
}
""", encoding="utf-8")

print("stubs written")
