//! lan-link client core for the GUI.
//!
//! Mirrors the streaming exec path used by client_win.py: SYN -> SYN-ACK ->
//! encrypted Hello, then dispatch Exec/ExecStdin/ExecSignal and receive
//! ExecStarted/ExecChunk/ExecDone. Reuses the protocol crate's frame + crypto,
//! keeping the wire format identical to the Python client.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use bincode::serialize;
use lan_link_protocol::crypto::{self, Psk};
use lan_link_protocol::frame::{
    ControlMsg, Flags, HEADER_SIZE, PacketHeader, PacketType, StreamId,
};
use rand::RngCore;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio::time::timeout;

pub const STREAM_CONTROL: u16 = 0;

const PKT_SYN: u8 = 0;
const PKT_SYN_ACK: u8 = 1;
const PKT_DATA: u8 = 3;
const PKT_HEARTBEAT: u8 = 5;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HostConfig {
    pub name: String,
    pub addr: String,
    pub psk_hex: String,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            name: "tuanzi".into(),
            addr: "192.168.31.244:9876".into(),
            psk_hex: String::new(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    pub hosts: Vec<HostConfig>,
    pub active_host: usize,
    pub timeout_secs: u64,
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut hosts = vec![HostConfig::default()];
        hosts.push(HostConfig {
            name: "localhost".into(),
            addr: "127.0.0.1:9876".into(),
            psk_hex: String::new(),
        });
        Self { hosts, active_host: 0, timeout_secs: 30 }
    }
}

impl AppConfig {
    pub fn config_path() -> std::path::PathBuf {
        let base = std::env::var_os("APPDATA")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(std::path::PathBuf::from))
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        base.join("lan-link").join("gui-config.json")
    }

    pub fn load() -> Self {
        let p = Self::config_path();
        if let Ok(s) = std::fs::read_to_string(&p) {
            if let Ok(c) = serde_json::from_str::<AppConfig>(&s) {
                return c;
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let p = Self::config_path();
        if let Some(parent) = p.parent() { let _ = std::fs::create_dir_all(parent); }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, s);
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExecOutput {
    pub stream: u8, // 0=stdout, 1=stderr
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum ExecEvent {
    Started,
    Chunk(ExecOutput),
    Done(Option<i32>),
}

pub struct Connection {
    pub socket: Arc<UdpSocket>,
    pub peer: SocketAddr,
    pub psk: Psk,
    pub conn_id: u64,
    pub seq: Arc<AtomicU32>,
    pub next_id: Arc<AtomicU32>,
}

impl Connection {
    pub async fn connect(host: &HostConfig) -> anyhow::Result<Self> {
        let psk_hex = if host.psk_hex.trim().is_empty() {
            std::env::var("LAN_LINK_PSK")
                .map_err(|_| anyhow::anyhow!("PSK 未设置：请在配置中填写 PSK 或设置 LAN_LINK_PSK 环境变量"))?
        } else {
            host.psk_hex.clone()
        };
        let psk_bytes = hex::decode(psk_hex.trim())?;
        anyhow::ensure!(psk_bytes.len() == 32, "PSK must be 32 bytes hex");
        let mut psk: Psk = [0u8; 32];
        psk.copy_from_slice(&psk_bytes);

        let peer: SocketAddr = host.addr.parse()?;
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(peer).await?;

        let mut rng = rand::thread_rng();
        let conn_id = rng.next_u64();

        // SYN
        let syn = build_header(conn_id, PKT_SYN, 0, 0, 0, &mut [0u8; 12], 0);
        socket.send(&syn).await?;

        // Wait SYN-ACK
        let mut buf = vec![0u8; 2048];
        let mut got_syn_ack = false;
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let remain = deadline.saturating_duration_since(Instant::now());
            match timeout(remain, socket.recv(&mut buf)).await {
                Ok(Ok(n)) => {
                    if n < HEADER_SIZE { continue; }
                    if let Some(h) = PacketHeader::decode(&mut std::io::Cursor::new(&buf[..n])) {
                        if h.conn_id == conn_id && h.pkt_type as u8 == PKT_SYN_ACK {
                            got_syn_ack = true;
                            break;
                        }
                    }
                }
                _ => continue,
            }
        }
        anyhow::ensure!(got_syn_ack, "no SYN-ACK from {}", peer);

        // Send encrypted Hello
        let hello = ControlMsg::Hello { version: lan_link_protocol::frame::PROTOCOL_VERSION, capabilities: vec!["exec".into(), "input".into()] };
        let mut conn = Connection {
            socket: Arc::new(socket),
            peer, psk, conn_id,
            seq: Arc::new(AtomicU32::new(0)),
            next_id: Arc::new(AtomicU32::new(1)),
        };
        conn.send_control(&hello).await?;
        Ok(conn)
    }

    pub async fn send_control(&mut self, msg: &ControlMsg) -> anyhow::Result<()> {
        let payload = serialize(msg)?;
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        let nonce = crypto::make_nonce(self.conn_id, seq);
        let ciphertext = crypto::encrypt(&self.psk, &nonce, &payload).map_err(|e| anyhow::anyhow!("encrypt: {}", e))?;
        let pkt = build_encrypted(self.conn_id, STREAM_CONTROL, seq, &nonce, &ciphertext);
        self.socket.send(&pkt).await?;
        Ok(())
    }

    /// Run an exec and stream events back. stdin_bytes and close_stdin are
    /// optional. Caller can observe events via the on_event callback invoked
    /// from a background task; the future returned completes when the exec is
    /// done.
    pub async fn exec_streaming(
        &mut self,
        cmd: &str,
        stdin_bytes: Option<Vec<u8>>,
        timeout_secs: u64,
        mut on_event: impl FnMut(ExecEvent) + Send + 'static,
    ) -> anyhow::Result<Option<i32>> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.send_control(&ControlMsg::Exec { id, cmd: cmd.to_string() }).await?;
        if let Some(data) = stdin_bytes {
            self.send_control(&ControlMsg::ExecStdin { id, data, close: true }).await?;
        }

        let mut buf = vec![0u8; 2048];
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        let mut exit_code: Option<i32> = None;
        let mut done = false;
        on_event(ExecEvent::Started);
        while Instant::now() < deadline && !done {
            let remain = deadline.saturating_duration_since(Instant::now());
            let recv = match timeout(remain, self.socket.recv(&mut buf)).await {
                Ok(Ok(n)) => n,
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => break, // timeout
            };
            if recv < HEADER_SIZE { continue; }
            let header = match PacketHeader::decode(&mut std::io::Cursor::new(&buf[..recv])) {
                Some(h) => h,
                None => continue,
            };
            if header.pkt_type as u8 != PKT_DATA { continue; }
            if header.stream_id != STREAM_CONTROL { continue; }
            let enc_start = HEADER_SIZE;
            if recv <= enc_start { continue; }
            let plaintext = match crypto::decrypt(&self.psk, &header.nonce, &buf[enc_start..recv]) {
                Some(p) => p,
                None => continue,
            };
            let msg: ControlMsg = match bincode::deserialize(&plaintext) {
                Ok(m) => m,
                Err(_) => continue,
            };
            match msg {
                ControlMsg::ExecChunk { id: mid, stream, data } if mid == id => {
                    on_event(ExecEvent::Chunk(ExecOutput { stream, data }));
                }
                ControlMsg::ExecDone { id: mid, exit_code: code } if mid == id => {
                    exit_code = code;
                    on_event(ExecEvent::Done(code));
                    done = true;
                }
                _ => {}
            }
        }
        if !done {
            on_event(ExecEvent::Done(None));
        }
        Ok(exit_code)
    }
}

fn build_header(
    conn_id: u64, pkt_type: u8, stream_id: u16, seq: u32, flags: u8,
    nonce: &mut [u8; 12], payload_len: u16,
) -> [u8; HEADER_SIZE] {
    let mut buf = [0u8; HEADER_SIZE];
    buf[0..8].copy_from_slice(&conn_id.to_le_bytes());
    buf[8] = pkt_type;
    buf[9] = flags;
    buf[10..12].copy_from_slice(&stream_id.to_le_bytes());
    buf[12..16].copy_from_slice(&seq.to_le_bytes());
    buf[16..20].copy_from_slice(&0u32.to_le_bytes());
    buf[20..24].copy_from_slice(&0u32.to_le_bytes());
    buf[24..26].copy_from_slice(&payload_len.to_le_bytes());
    buf[26..38].copy_from_slice(nonce);
    buf
}

fn build_encrypted(
    conn_id: u64, stream_id: u16, seq: u32,
    nonce: &[u8; 12], ciphertext: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_SIZE + ciphertext.len());
    out.extend_from_slice(&conn_id.to_le_bytes());
    out.push(PKT_DATA);
    out.push(Flags::RELIABLE.bits());
    out.extend_from_slice(&stream_id.to_le_bytes());
    out.extend_from_slice(&seq.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // ack_seq
    out.extend_from_slice(&0u32.to_le_bytes()); // ack_bitmap
    out.extend_from_slice(&(ciphertext.len() as u16).to_le_bytes());
    out.extend_from_slice(nonce);
    out.extend_from_slice(ciphertext);
    out
}

// Suppress unused import warnings if features get stripped.
#[allow(dead_code)]
fn _force_use(_: StreamId) {}
