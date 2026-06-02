//! Video engine: screen capture + hardware encoding.
//!
//! Windows: DXGI Desktop Duplication API + NVENC (via nvidia-video-codec crate or ffmpeg CLI).
//! Linux: DRM/KMS framebuffer capture + VA-API encode.
//!
//! For initial implementation, we use DXGI Output Duplication to capture
//! a specific monitor's content, then encode with NVENC hardware encoder.

#[cfg(target_os = "windows")]
mod capture;
#[cfg(target_os = "linux")]
mod linux_capture;

mod encoder;

/// Configuration for video streaming.
#[derive(Debug, Clone)]
pub struct VideoConfig {
    /// Target monitor index (0-based)
    pub monitor_index: u32,
    /// Capture resolution width
    pub width: u32,
    /// Capture resolution height
    pub height: u32,
    /// Target frames per second
    pub fps: u32,
    /// Target bitrate in kbps
    pub bitrate_kbps: u32,
    /// Codec: "h264" or "hevc"
    pub codec: String,
}

/// A captured video frame (raw BGRA or NV12).
#[derive(Debug)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
    /// Pixel format: "bgra", "nv12", "yuv420p"
    pub format: String,
    /// Timestamp in microseconds
    pub pts: u64,
}

/// Encoded video packet.
#[derive(Debug)]
pub struct EncodedPacket {
    pub data: Vec<u8>,
    /// Whether this is a keyframe (IDR)
    pub keyframe: bool,
    pub pts: u64,
    pub dts: u64,
}

/// Video capture trait.
pub trait VideoCapture: Send {
    /// Capture the next frame. Returns None if no new frame available.
    fn capture(&mut self) -> Option<VideoFrame>;
    /// Get monitor dimensions.
    fn dimensions(&self) -> (u32, u32);
}

/// Video encoder trait.
pub trait VideoEncoder: Send {
    /// Encode a raw frame into a compressed packet.
    fn encode(&mut self, frame: &VideoFrame) -> Option<EncodedPacket>;
    /// Flush any buffered frames (call at end of stream).
    fn flush(&mut self) -> Vec<EncodedPacket>;
}
