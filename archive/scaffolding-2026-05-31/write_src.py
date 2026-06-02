import os
from pathlib import Path

BASE = Path(r"G:\codex-AI-tools\lan-link\crates")

def write(path, content):
    p = Path(path)
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(content, encoding="utf-8")

# ===== crypto.rs =====
write(BASE / "protocol/src/crypto.rs", r"""//! ChaCha20-Poly1305 AEAD encryption for UDP packets.
//!
//! Each packet is encrypted with a unique 96-bit nonce derived from the
//! connection ID + sequence number, ensuring per-packet authentication.

use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce,
};
use rand::RngCore;

/// Pre-shared key (32 bytes). Generated once, shared out-of-band.
pub type Psk = [u8; 32];

/// Generate a random 32-byte PSK.
pub fn generate_psk() -> Psk {
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    key
}

/// Encrypt plaintext, producing ciphertext + 16-byte authentication tag.
/// `nonce` must be exactly 12 bytes.
pub fn encrypt(key: &Psk, nonce: &[u8; 12], plaintext: &[u8]) -> Vec<u8> {
    let cipher = ChaCha20Poly1305::new_from_slice(key).expect("valid key size");
    let nonce = Nonce::from_slice(nonce);
    cipher.encrypt(nonce, plaintext).expect("encryption failed")
}

/// Decrypt ciphertext (which includes the 16-byte auth tag appended).
/// Returns plaintext or None if authentication fails.
pub fn decrypt(key: &Psk, nonce: &[u8; 12], ciphertext: &[u8]) -> Option<Vec<u8>> {
    let cipher = ChaCha20Poly1305::new_from_slice(key).expect("valid key size");
    let nonce = Nonce::from_slice(nonce);
    cipher.decrypt(nonce, ciphertext).ok()
}

/// Derive a per-packet nonce from conn_id + sequence number.
/// This gives us a unique nonce per packet without sending a random nonce each time.
pub fn make_nonce(conn_id: u64, seq: u32) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce[0..8].copy_from_slice(&conn_id.to_le_bytes());
    nonce[8..12].copy_from_slice(&seq.to_le_bytes());
    nonce
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let psk = generate_psk();
        let nonce = make_nonce(42, 1);
        let plain = b"hello world";
        let cipher = encrypt(&psk, &nonce, plain);
        assert_eq!(cipher.len(), plain.len() + 16);
        let dec = decrypt(&psk, &nonce, &cipher).unwrap();
        assert_eq!(&dec, plain);
    }

    #[test]
    fn tamper_detection() {
        let psk = generate_psk();
        let nonce = make_nonce(42, 1);
        let mut cipher = encrypt(&psk, &nonce, b"hello");
        cipher[0] ^= 1;
        assert!(decrypt(&psk, &nonce, &cipher).is_none());
    }
}
""")

# ===== reliable.rs =====
write(BASE / "protocol/src/reliable.rs", r"""//! Reliable transport layer over UDP.
//!
//! Implements selective repeat ARQ with piggybacked ACKs and a sliding window.
//! Used for Control, Input, and File streams. Audio/Video skip this layer.

use crate::frame::{Flags, PacketHeader, PacketType, MAX_PAYLOAD};
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
    data: BytesMut,
    sent_at: Instant,
    retries: u32,
    acked: bool,
}

/// Reliable send state for one stream.
pub struct ReliableSender {
    stream_id: u16,
    next_seq: u32,
    window_base: u32,
    slots: VecDeque<SendSlot>,
}

impl ReliableSender {
    pub fn new(stream_id: u16) -> Self {
        Self {
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
            data: BytesMut::from(payload),
            sent_at: Instant::now(),
            retries: 0,
            acked: false,
        };
        self.slots.push_back(slot);
        Some(self.encode_packet(conn_id, seq, payload))
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
    pub fn poll_retransmit(&mut self, conn_id: u64) -> Vec<BytesMut> {
        let now = Instant::now();
        let mut retrans = vec![];
        for slot in &mut self.slots {
            if slot.acked || slot.retries >= MAX_RETRIES {
                continue;
            }
            if now.duration_since(slot.sent_at) >= RTO {
                slot.retries += 1;
                slot.sent_at = now;
                retrans.push(self.encode_packet(conn_id, slot.seq, &slot.data));
            }
        }
        retrans
    }

    fn encode_packet(&self, conn_id: u64, seq: u32, payload: &[u8]) -> BytesMut {
        let mut buf = BytesMut::with_capacity(38 + payload.len());
        let header = PacketHeader {
            conn_id,
            pkt_type: PacketType::Data,
            flags: Flags::RELIABLE,
            stream_id: self.stream_id,
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
""")

# ===== stream.rs =====
write(BASE / "protocol/src/stream.rs", r"""//! Stream multiplexing over a single UDP connection.
//!
//! Maps logical stream IDs to reliable/unreliable send/receive queues.

use crate::frame::{PacketType, StreamId};
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
""")

print("protocol source files written")
