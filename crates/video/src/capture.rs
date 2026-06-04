//! DXGI Desktop Duplication capture (stub).

use crate::{VideoCapture, VideoFrame};

#[allow(dead_code)]
pub struct DxgiCapture {
    width: u32,
    height: u32,
    frame_count: u64,
}

#[allow(dead_code)]
impl DxgiCapture {
    pub fn new(monitor_index: u32, width: u32, height: u32) -> Self {
        let _ = monitor_index;
        Self { width, height, frame_count: 0 }
    }
}

impl VideoCapture for DxgiCapture {
    fn capture(&mut self) -> Option<VideoFrame> {
        let size = (self.width * self.height * 4) as usize;
        self.frame_count += 1;
        Some(VideoFrame {
            width: self.width,
            height: self.height,
            data: vec![0u8; size],
            format: "bgra".to_string(),
            pts: self.frame_count * 1_000_000 / 60,
        })
    }

    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}
