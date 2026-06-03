//! UDP 数据包帧格式\n//! 固定 38 字节 Header + 加密负载\n//!
//! Fixed 38-byte header + encrypted payload.
//!
//! Layout (all little-endian):
//! `	ext
//! [ 0.. 8] conn_id:      u64    Connection identifier
//! [ 8    ] pkt_type:     u8     SYN=0 ACK=1 DATA=2 RST=3 HEARTBEAT=4
//! [ 9    ] flags:        u8     bit0=reliable bit1=fragmented bit2=ordered
//! [10..12] stream_id:    u16    0=control 1=video 2=audio_tx 3=audio_rx 4=input 5=file
//! [12..16] seq:          u32    Sequence number
//! [16..20] ack_seq:      u32    Piggyback ACK
//! [20..24] ack_bitmap:   u32    Selective ACK bitmap (32 packets window)
//! [24..26] payload_len:  u16    Encrypted payload length
//! [26..38] nonce:        [u8;12] 96-bit nonce for ChaCha20-Poly1305
//! [38.. ]  encrypted:    [u8]   Encrypted payload (includes 16B Poly1305 tag)
//! `
//!
//! Total overhead: 38 (header) + 16 (auth tag) = 54 bytes
//! Max payload: 1400 bytes (fits in typical MTU=1500)

use bytes::{Buf, BufMut};

/// 当前协议版本，Hello 握手时协商使用
pub const PROTOCOL_VERSION: u16 = 1;

pub const HEADER_SIZE: usize = 38;
pub const MAX_PAYLOAD: usize = 1400;
pub const MAX_PACKET: usize = HEADER_SIZE + MAX_PAYLOAD + 16; // +16 auth tag

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PacketType {
    Syn = 0,
    SynAck = 1,
    Ack = 2,
    Data = 3,
    Rst = 4,
    Heartbeat = 5,
}

impl PacketType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Syn),
            1 => Some(Self::SynAck),
            2 => Some(Self::Ack),
            3 => Some(Self::Data),
            4 => Some(Self::Rst),
            5 => Some(Self::Heartbeat),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum StreamId {
    Control = 0,
    Video = 1,
    AudioTx = 2,
    AudioRx = 3,
    Input = 4,
    File = 5,
}

impl StreamId {
    pub fn from_u16(v: u16) -> Option<Self> {
        match v {
            0 => Some(Self::Control),
            1 => Some(Self::Video),
            2 => Some(Self::AudioTx),
            3 => Some(Self::AudioRx),
            4 => Some(Self::Input),
            5 => Some(Self::File),
            _ => None,
        }
    }

    /// Whether this stream uses reliable delivery.
    pub fn is_reliable(self) -> bool {
        matches!(self, Self::Control | Self::Input | Self::File)
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Flags: u8 {
        const RELIABLE   = 0b001;
        const FRAGMENTED = 0b010;
        const ORDERED    = 0b100;
    }
}

/// Decoded packet header (plaintext fields only).
#[derive(Debug, Clone)]
pub struct PacketHeader {
    pub conn_id: u64,
    pub pkt_type: PacketType,
    pub flags: Flags,
    pub stream_id: u16,
    pub seq: u32,
    pub ack_seq: u32,
    pub ack_bitmap: u32,
    pub payload_len: u16,
    pub nonce: [u8; 12],
}

impl PacketHeader {
    pub fn encode(&self, buf: &mut impl BufMut) {
        buf.put_u64_le(self.conn_id);
        buf.put_u8(self.pkt_type as u8);
        buf.put_u8(self.flags.bits());
        buf.put_u16_le(self.stream_id);
        buf.put_u32_le(self.seq);
        buf.put_u32_le(self.ack_seq);
        buf.put_u32_le(self.ack_bitmap);
        buf.put_u16_le(self.payload_len);
        buf.put_slice(&self.nonce);
    }

    pub fn decode(buf: &mut impl Buf) -> Option<Self> {
        if buf.remaining() < HEADER_SIZE {
            return None;
        }
        let conn_id = buf.get_u64_le();
        let pkt_type = PacketType::from_u8(buf.get_u8())?;
        let flags = Flags::from_bits_retain(buf.get_u8());
        let stream_id = buf.get_u16_le();
        let seq = buf.get_u32_le();
        let ack_seq = buf.get_u32_le();
        let ack_bitmap = buf.get_u32_le();
        let payload_len = buf.get_u16_le();
        let mut nonce = [0u8; 12];
        buf.copy_to_slice(&mut nonce);
        Some(Self { conn_id, pkt_type, flags, stream_id, seq, ack_seq, ack_bitmap, payload_len, nonce })
    }
}

/// Control messages carried on stream 0.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ControlMsg {
    /// Shell: execute command, return output (legacy one-shot)
    Exec { id: u32, cmd: String },
    ExecOutput { id: u32, data: Vec<u8>, exit_code: Option<i32> },


    /// File transfer
    FilePush { id: u32, path: String, size: u64 },
    FileChunk { id: u32, offset: u64, data: Vec<u8> },
    FileAck { id: u32, offset: u64 },

    /// Input injection (keyboard/mouse events)
    KeyEvent { down: bool, scancode: u16, vk: u16 },
    MouseMove { dx: i16, dy: i16 },
    MouseButton { button: u8, down: bool },
    MouseWheel { delta: i16 },

    /// Video stream control
    VideoStart { width: u16, height: u16, fps: u8, bitrate_kbps: u32 },
    VideoStop,

    /// Audio stream control
    AudioStart { sample_rate: u32, channels: u8 },
    AudioStop,

    /// Connection negotiation
    Hello { version: u16, capabilities: Vec<String> },
    HelloAck { version: u16, capabilities: Vec<String> },

    /// Shell: streaming execution (id is correlation id, same as Exec.id).
    /// Spawn fires off ExecStarted, then stdout/stderr chunks stream as
    /// ExecChunk, and a final ExecDone with the exit code.
    ExecStarted { id: u32 },
    ExecChunk { id: u32, stream: u8, data: Vec<u8> }, // stream: 0=stdout, 1=stderr
    ExecDone { id: u32, exit_code: Option<i32> },
    /// Stdin input to a running exec (id, optional flag to close stdin).
    ExecStdin { id: u32, data: Vec<u8>, close: bool },
    /// Signal the running exec (signal number, e.g. 15=SIGTERM, 9=SIGKILL).
    ExecSignal { id: u32, signo: u32 },
    /// Native command — executed in-process on daemon without shell.
    /// Results are streamed back via ExecChunk/ExecDone (reuses client drain_exec).
    NativeCmd { id: u32, cmd: NativeCmdType },
    /// Native spawn — like Exec but uses NativeCmdType, supports stdin/signal/streaming.
    NativeSpawn { id: u32, cmd: NativeCmdType },
}


/// Native command types — executed in-process on the daemon.
/// Each variant describes a structured command with typed parameters.
/// Results are returned via ExecChunk (as formatted text) + ExecDone.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum NativeCmdType {
    // -- Filesystem --
    Ls { path: String, long: bool, all: bool },
    Cat { path: String },
    Tail { path: String, lines: u32, follow: bool, follow_secs: u64 },
    Head { path: String, lines: u32 },
    Find { path: String, name: Option<String>, type_: Option<String>, maxdepth: u32 },
    Grep { pattern: String, path: String, recursive: bool, line_number: bool, count: bool },
    Du { path: String, summarize: bool, maxdepth: u32 },
    Df { human: bool, all: bool },
    Tree { path: String, depth: u32, dirs_only: bool },
    Stat { path: String },
    Mkdir { recursive: bool, paths: Vec<String> },
    Rm { recursive: bool, force: bool, paths: Vec<String> },
    Mv { src: String, dest: String },
    Cp { recursive: bool, src: String, dest: String },
    Chmod { mode: String, paths: Vec<String> },
    Chown { owner: String, paths: Vec<String> },
    Diff { file1: String, file2: String },
    Wc { lines: bool, words: bool, paths: Vec<String> },
    Lsblk,
    Mount,

    // -- System --
    Ps { full: bool, user: Option<String>, tree: bool },
    Kill { pid: u32, signal: u32 },
    Pgrep { name: String, full: bool, count: bool },
    Pkill { name: String, signal: u32 },
    Top { interval_secs: u64, iterations: u32 },
    Uptime,
    Hostname,
    Uname { all: bool, release: bool, machine: bool },
    Whoami,
    Who,
    Last { lines: u32 },
    Free { human: bool },
    Cpu,
    Dmesg { lines: u32, level: Option<String> },
    Info,

    // -- Network --
    Netstat { tcp: bool, udp: bool, numeric: bool, listening: bool },
    Ip { addr: bool, route: bool, link: bool },
    PortScan { host: String, start_port: u16, end_port: u16, timeout_ms: u64 },
    Arp,
    Dns { hostname: String, type_: Option<String> },

    // -- Management (structured protocol, may use std::process::Command internally) --
    Service { action: ServiceActionType },
    Journal { unit: Option<String>, follow: bool, lines: u32, priority: Option<String>, since: Option<String> },
    Pkg { action: PkgActionType },
    Docker { action: DockerActionType },
    Crontab { action: CrontabActionType },
    Firewall { backend: String },
    Ssh,
    Checksum { path: String, algorithm: String },

    // -- Generic shell execution (runs sh -c on daemon) --
    ShellExec { cmd: String, timeout_secs: u32 },
    ReadFile { path: String },

    // -- Batch / Watch --
    BatchContent { lines: Vec<String>, timeout: u64 },
    Watch { interval_secs: u64, cmd: Vec<String> },

    // -- File editing --
    WriteFile { path: String, data: Vec<u8>, append: bool },
    Sed { path: String, pattern: String, replacement: String, global: bool, regex: bool },
    Touch { path: String },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ServiceActionType {
    List { active: bool, failed: bool },
    Status { name: String },
    Start { name: String },
    Stop { name: String },
    Restart { name: String },
    Reload { name: String },
    Enable { name: String },
    Disable { name: String },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PkgActionType {
    List { installed: bool },
    Search { query: String },
    Install { name: String },
    Remove { name: String },
    Update,
    Upgrade,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum DockerActionType {
    Ps { all: bool, running: bool },
    Logs { name: String, tail: u32, follow: bool },
    Stats { interval_secs: u64 },
    Exec { container: String, cmd: Vec<String> },
    Info,
    Images,
    Rm { container: String, force: bool },
    Control { container: String, action: String },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum CrontabActionType {
    List,
    Edit,
    Remove,
}
