//! Linux DRM/KMS capture (stub).

use crate::{VideoCapture, VideoFrame};

pub struct DrmCapture;
impl DrmCapture { pub fn new() -> anyhow::Result<Self> { Ok(Self) } }
impl VideoCapture for DrmCapture {
    fn capture(&mut self) -> Option<VideoFrame> { None }
    fn dimensions(&self) -> (u32, u32) { (0, 0) }
}
