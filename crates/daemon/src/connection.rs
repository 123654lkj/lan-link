//! 连接管理
//! 连接状态枚举、SYN-ACK 握手、心跳、数据包封装。

use lan_link_protocol::crypto::Psk;
use lan_link_protocol::frame::{PacketHeader, PacketType, Flags, StreamId, HEADER_SIZE};
use lan_link_protocol::stream::StreamMux;
use bytes::{BufMut, BytesMut};
use rand::RngCore;
use std::net::SocketAddr;
use std::time::Instant;

#[derive(Debug, PartialEq, Eq)]
pub enum ConnState { Listening, SynSent, Established, Closed }

pub struct Connection {
    pub id: u64,
    pub peer: SocketAddr,
    pub psk: Psk,
    pub state: ConnState,
    pub mux: StreamMux,
    pub created: Instant,
    pub last_activity: Instant,
}

impl Connection {
    pub fn new(id: u64, peer: SocketAddr, psk: Psk) -> Self {
        Self { id, peer, psk, state: ConnState::Listening, mux: StreamMux::new(),
               created: Instant::now(), last_activity: Instant::now() }
    }

    pub fn build_syn(conn_id: u64) -> BytesMut {
        let mut buf = BytesMut::with_capacity(HEADER_SIZE);
        PacketHeader { conn_id, pkt_type: PacketType::Syn, flags: Flags::empty(),
            stream_id: StreamId::Control as u16, seq: 0, ack_seq: 0,
            ack_bitmap: 0, payload_len: 0, nonce: [0u8; 12] }.encode(&mut buf);
        buf
    }

    pub fn build_syn_ack(conn_id: u64) -> BytesMut {
        let mut buf = BytesMut::with_capacity(HEADER_SIZE);
        PacketHeader { conn_id, pkt_type: PacketType::SynAck, flags: Flags::empty(),
            stream_id: StreamId::Control as u16, seq: 0, ack_seq: 0,
            ack_bitmap: 0, payload_len: 0, nonce: [0u8; 12] }.encode(&mut buf);
        buf
    }

    pub fn build_data(conn_id: u64, stream_id: u16, seq: u32, flags: Flags, payload: &[u8]) -> BytesMut {
        let mut buf = BytesMut::with_capacity(HEADER_SIZE + payload.len());
        PacketHeader { conn_id, pkt_type: PacketType::Data, flags, stream_id, seq,
            ack_seq: 0, ack_bitmap: 0, payload_len: payload.len() as u16, nonce: [0u8; 12]
        }.encode(&mut buf);
        buf.put_slice(payload);
        buf
    }

    pub fn build_encrypted_data(conn_id: u64, stream_id: u16, seq: u32, flags: Flags,
                                 ciphertext: &[u8], nonce: [u8; 12]) -> BytesMut {
        let mut buf = BytesMut::with_capacity(HEADER_SIZE + ciphertext.len());
        PacketHeader { conn_id, pkt_type: PacketType::Data, flags, stream_id, seq,
            ack_seq: 0, ack_bitmap: 0, payload_len: ciphertext.len() as u16, nonce
        }.encode(&mut buf);
        buf.put_slice(ciphertext);
        buf
    }

    pub fn build_heartbeat(conn_id: u64) -> BytesMut {
        let mut buf = BytesMut::with_capacity(HEADER_SIZE);
        PacketHeader { conn_id, pkt_type: PacketType::Heartbeat, flags: Flags::empty(),
            stream_id: StreamId::Control as u16, seq: 0, ack_seq: 0,
            ack_bitmap: 0, payload_len: 0, nonce: [0u8; 12] }.encode(&mut buf);
        buf
    }

    pub fn generate_id() -> u64 { rand::thread_rng().next_u64() }
}
