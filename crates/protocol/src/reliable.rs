//! 可靠传输层（选择性 ARQ）\n//! 32 包滑动窗口 + 重排序\n//!
//! Implements selective repeat ARQ with piggybacked ACKs and a sliding window.
//! Used for Control, Input, and File streams. Audio/Video skip this layer.

use crate::frame::{Flags, PacketHeader, PacketType};
use bytes::{BufMut, BytesMut};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Window size for reliable delivery.
const WINDOW_SIZE: u32 = 32;
/// Retransmit after this duration.
const RTO: Duration = Duration::from_millis(200);
/// Max retransmissions before giving up.
const MAX_RETRIES: u32 = 10;

/// One pending packet in the send window.
struct SendSlot {
    seq: u32,
    data: Vec<u8>,
    sent_at: Instant,
    retries: u32,
    acked: bool,
}

/// Reliable send state for one stream.
pub struct ReliableSender {
    conn_id: u64,
    stream_id: u16,
    next_seq: u32,
    window_base: u32,
    slots: VecDeque<SendSlot>,
}

impl ReliableSender {
    pub fn new(conn_id: u64, stream_id: u16) -> Self {
        Self {
            conn_id,
            stream_id,
            next_seq: 0,
            window_base: 0,
            slots: VecDeque::new(),
        }
    }

    /// Queue a payload for reliable delivery. Returns the encoded packet (without crypto).
    pub fn send(&mut self, conn_id: u64, payload: &[u8]) -> Option<BytesMut> {
        if self.next_seq - self.window_base >= WINDOW_SIZE {
            return None; // Window full, caller should retry
        }
        let seq = self.next_seq;
        self.next_seq += 1;
        let slot = SendSlot {
            seq,
            data: payload.to_vec(),
            sent_at: Instant::now(),
            retries: 0,
            acked: false,
        };
        self.slots.push_back(slot);
        Some(Self::encode_packet(conn_id, self.stream_id, seq, payload))
    }

    /// Process an incoming ACK. Returns newly-acked sequence numbers.
    pub fn on_ack(&mut self, ack_seq: u32, ack_bitmap: u32) -> Vec<u32> {
        let mut acked = vec![];
        for slot in &mut self.slots {
            if slot.acked {
                continue;
            }
            let dist = slot.seq.wrapping_sub(ack_seq);
            if dist == 0 {
                slot.acked = true;
                acked.push(slot.seq);
            } else if dist <= 32 && (ack_bitmap & (1 << (dist - 1))) != 0 {
                slot.acked = true;
                acked.push(slot.seq);
            }
        }
        // Slide window
        while self.slots.front().map_or(false, |s| s.acked) {
            self.slots.pop_front();
            self.window_base = self.window_base.wrapping_add(1);
        }
        acked
    }

    /// Check for timed-out packets that need retransmission.
    /// Returns (conn_id, stream_id, seq, payload) tuples for the caller to encode.
    pub fn poll_retransmit(&mut self) -> Vec<(u64, u16, u32, Vec<u8>)> {
        let now = Instant::now();
        let mut retrans = vec![];
        for slot in &mut self.slots {
            if slot.acked || slot.retries >= MAX_RETRIES {
                continue;
            }
            if now.duration_since(slot.sent_at) >= RTO {
                slot.retries += 1;
                slot.sent_at = now;
                retrans.push((self.conn_id, self.stream_id, slot.seq, slot.data.clone()));
            }
        }
        retrans
    }

    /// Encode a single packet into wire format.
    pub fn encode_packet(conn_id: u64, stream_id: u16, seq: u32, payload: &[u8]) -> BytesMut {
        let mut buf = BytesMut::with_capacity(38 + payload.len());
        let header = PacketHeader {
            conn_id,
            pkt_type: PacketType::Data,
            flags: Flags::RELIABLE,
            stream_id,
            seq,
            ack_seq: 0,
            ack_bitmap: 0,
            payload_len: payload.len() as u16,
            nonce: [0u8; 12], // filled by crypto layer
        };
        header.encode(&mut buf);
        buf.put_slice(payload);
        buf
    }
}

/// Receive state for one stream.
pub struct ReliableReceiver {
    #[allow(dead_code)]
    stream_id: u16,
    next_expected: u32,
    /// Out-of-order buffer: (seq, data)
    ooo_buffer: VecDeque<(u32, Vec<u8>)>,
    /// Last ACK sent
    last_ack_seq: u32,
    last_ack_bitmap: u32,
}

impl ReliableReceiver {
    pub fn new(stream_id: u16) -> Self {
        Self {
            stream_id,
            next_expected: 0,
            ooo_buffer: VecDeque::new(),
            last_ack_seq: 0,
            last_ack_bitmap: 0,
        }
    }

    /// Deliver an incoming data packet. Returns in-order payloads ready for the application.
    pub fn deliver(&mut self, seq: u32, payload: &[u8]) -> Vec<Vec<u8>> {
        let dist = seq.wrapping_sub(self.next_expected);

        if dist == 0 {
            // In order
            self.next_expected = self.next_expected.wrapping_add(1);
            let mut results = vec![payload.to_vec()];

            // Drain consecutive buffered packets
            while let Some(&(s, _)) = self.ooo_buffer.front() {
                if s == self.next_expected {
                    let (_, data) = self.ooo_buffer.pop_front().unwrap();
                    self.next_expected = self.next_expected.wrapping_add(1);
                    results.push(data);
                } else {
                    break;
                }
            }
            results
        } else if dist <= WINDOW_SIZE {
            // Out of order within window, buffer it
            let insert_pos = self.ooo_buffer.iter().position(|(s, _)| {
                seq.wrapping_sub(*s) < dist
            }).unwrap_or(self.ooo_buffer.len());
            if insert_pos < self.ooo_buffer.len()
                && self.ooo_buffer[insert_pos].0 == seq
            {
                // Duplicate, ignore
            } else {
                self.ooo_buffer.insert(insert_pos, (seq, payload.to_vec()));
            }
            vec![]
        } else {
            // Outside window, discard
            vec![]
        }
    }

    /// Build ACK info for piggybacking.
    pub fn ack_info(&self) -> (u32, u32) {
        (self.next_expected.wrapping_sub(1), self.last_ack_bitmap)
    }
}
