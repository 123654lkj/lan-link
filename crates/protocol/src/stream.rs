//! 流多路复用\n//! 在单 UDP 连接上管理多逻辑流\n//!
//! Maps logical stream IDs to reliable/unreliable send/receive queues.

use crate::frame::StreamId;
use std::collections::HashMap;

/// A muxed stream handle.
pub struct MuxStream {
    pub id: u16,
    pub is_reliable: bool,
    send_seq: u32,
}

impl MuxStream {
    pub fn new(id: u16) -> Self {
        let is_reliable = StreamId::from_u16(id)
            .map(|s| s.is_reliable())
            .unwrap_or(false);
        Self { id, is_reliable, send_seq: 0 }
    }

    pub fn next_seq(&mut self) -> u32 {
        let seq = self.send_seq;
        self.send_seq = self.send_seq.wrapping_add(1);
        seq
    }
}

/// Manages all streams for a connection.
pub struct StreamMux {
    streams: HashMap<u16, MuxStream>,
}

impl StreamMux {
    pub fn new() -> Self {
        let mut streams = HashMap::new();
        // Pre-create standard streams
        for id in [0u16, 1, 2, 3, 4, 5] {
            streams.insert(id, MuxStream::new(id));
        }
        Self { streams }
    }

    pub fn get(&self, id: u16) -> Option<&MuxStream> {
        self.streams.get(&id)
    }

    pub fn get_mut(&mut self, id: u16) -> Option<&mut MuxStream> {
        self.streams.get_mut(&id)
    }
}
