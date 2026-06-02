//! Video encoder (stub).

use crate::{EncodedPacket, VideoConfig, VideoEncoder, VideoFrame};

pub struct NvencEncoder { config: VideoConfig, frame_count: u64 }
impl NvencEncoder {
    pub fn new(config: VideoConfig) -> Self { Self { config, frame_count: 0 } }
}
impl VideoEncoder for NvencEncoder {
    fn encode(&mut self, _frame: &VideoFrame) -> Option<EncodedPacket> { None }
    fn flush(&mut self) -> Vec<EncodedPacket> { vec![] }
}

pub struct SoftwareEncoder { config: VideoConfig }
impl SoftwareEncoder {
    pub fn new(config: VideoConfig) -> Self { Self { config } }
}
impl VideoEncoder for SoftwareEncoder {
    fn encode(&mut self, _frame: &VideoFrame) -> Option<EncodedPacket> { None }
    fn flush(&mut self) -> Vec<EncodedPacket> { vec![] }
}
