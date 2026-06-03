//! lan-linkd — 局域网远程管理守护进程
//!
//! 运行在目标机器上的服务端二进制，通过 UDP 端口（默认 9876）监听客户端连接。
//!
//! # 职责
//!
//! 1. **连接管理** — SYN/SYN-ACK 握手，5 秒心跳保活，30 秒超时清理
//! 2. **命令执行** — 接收并执行 NativeCmd（结构化原生命令）和 Exec（shell 命令）
//! 3. **输入注入** — Linux 平台通过 uinput 将键盘/鼠标事件注入到系统输入子系统
//! 4. **服务发现** — mDNS 广播自身存在（预留实现）
//!
//! # 架构
//!
//! 主循环为单线程异步事件轮询（tokio），每 100ms 检查 UDP 套接字。
//! 所有阻塞操作（命令执行、文件 IO）通过 `spawn_blocking` 或独立线程处理。

use clap::Parser;
use lan_link_protocol::crypto::{self, Psk};
use lan_link_protocol::frame::{PacketHeader, PacketType, Flags, StreamId, HEADER_SIZE, ControlMsg, NativeCmdType, PROTOCOL_VERSION};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tracing::{info, warn, debug};
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;

mod connection;
mod native_cmd;
mod discovery;
use connection::{Connection, ConnState};

/// Command sent from handle_control to a running exec task.
#[derive(Debug)]
enum ExecCmd {
    Stdin(Vec<u8>, bool), // (data, close)
    Signal(i32),
}

/// Global map of running exec id -> command sender.
type ExecMap = Arc<AsyncMutex<HashMap<u32, tokio::sync::mpsc::UnboundedSender<ExecCmd>>>>;
fn new_exec_map() -> ExecMap { Arc::new(AsyncMutex::new(HashMap::new())) }

#[cfg(target_os = "linux")]
use lan_link_input::linux::LinuxInputInjector;

const PSK_PATH: &str = "/etc/lan-link/psk";
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const TIMEOUT: Duration = Duration::from_secs(30);

// ── 速率限制常量 ──────────────────────────────────────────────
/// 每个源 IP 每 60 秒最多允许的 SYN 握手次数
const SYN_RATE_LIMIT: u32 = 5;
/// 每个源 IP 每 60 秒最多允许的控制命令数（解密成功后计数）
const CMD_RATE_LIMIT: u32 = 30;
/// 速率限制窗口（秒）
const RATE_WINDOW_SECS: u64 = 60;
/// 单个 daemon 最大并发连接数（防连接耗尽）
const MAX_CONNECTIONS: usize = 100;

// ── 速率限制器 ──────────────────────────────────────────────
/// 简单的滑动窗口速率限制器，按源 IP 分桶。
/// 不引入外部依赖，仅用 HashMap + Instant。
struct RateLimiter {
    /// (ip, window_start) → count
    syn_counts: HashMap<std::net::IpAddr, (Instant, u32)>,
    cmd_counts: HashMap<std::net::IpAddr, (Instant, u32)>,
}

impl RateLimiter {
    fn new() -> Self {
        Self {
            syn_counts: HashMap::new(),
            cmd_counts: HashMap::new(),
        }
    }

    /// 检查并消耗一个 SYN 配额。返回 true 表示允许，false 表示限速。
    fn check_syn(&mut self, ip: std::net::IpAddr) -> bool {
        let now = Instant::now();
        let entry = self.syn_counts.entry(ip).or_insert((now, 0));
        if now.duration_since(entry.0) > Duration::from_secs(RATE_WINDOW_SECS) {
            *entry = (now, 1);
            return true;
        }
        entry.1 += 1;
        entry.1 <= SYN_RATE_LIMIT
    }

    /// 检查并消耗一个命令配额。返回 true 表示允许，false 表示限速。
    fn check_cmd(&mut self, ip: std::net::IpAddr) -> bool {
        let now = Instant::now();
        let entry = self.cmd_counts.entry(ip).or_insert((now, 0));
        if now.duration_since(entry.0) > Duration::from_secs(RATE_WINDOW_SECS) {
            *entry = (now, 1);
            return true;
        }
        entry.1 += 1;
        entry.1 <= CMD_RATE_LIMIT
    }

    /// 定期清理过期条目，防止内存无限增长（每 heartbeat tick 调用）。
    fn gc(&mut self) {
        let now = Instant::now();
        let window = Duration::from_secs(RATE_WINDOW_SECS * 2);
        self.syn_counts.retain(|_, v| now.duration_since(v.0) < window);
        self.cmd_counts.retain(|_, v| now.duration_since(v.0) < window);
    }
}

#[derive(Parser, Debug)]
#[command(name = "lan-linkd", version, about = "LAN link 远程管理服务端")]
struct Args {
    #[arg(short, long, default_value = "9876", help = "监听端口")] port: u16,
    #[arg(short, long, help = "手动指定 PSK hex（不指定则自动生成/读取）")] psk: Option<String>,
    #[arg(long, default_value = "true", help = "启用 mDNS 发现")] discovery: bool,
}

fn load_or_generate_psk(args: &Args) -> Psk {
    if let Some(ref hex_str) = args.psk {
        let bytes = hex::decode(hex_str).expect("invalid PSK hex");
        assert_eq!(bytes.len(), 32);
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        return key;
    }
    if let Ok(existing) = std::fs::read(PSK_PATH) {
        if existing.len() == 64 {
            if let Ok(bytes) = hex::decode(&existing[..64]) {
                if bytes.len() == 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&bytes);
                    info!("Loaded PSK from {}", PSK_PATH);
                    return key;
                }
            }
        }
    }
    let key = crypto::generate_psk();
    let dir = Path::new(PSK_PATH).parent().unwrap();
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(PSK_PATH, hex::encode(key));
    info!("Generated new PSK saved to {}", PSK_PATH);
    eprintln!("PSK={}", hex::encode(key));
    key
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();
    let psk = load_or_generate_psk(&args);

    let addr = format!("0.0.0.0:{}", args.port);
    let socket = UdpSocket::bind(&addr).await?;
    info!("lan-linkd listening on {}", addr);

    if args.discovery {
        tokio::spawn(discovery::run(args.port));
    }

    let hb_socket = UdpSocket::bind("0.0.0.0:0").await?;
    let send_socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
    let mut connections: HashMap<u64, Connection> = HashMap::new();
    let mut buf = vec![0u8; lan_link_protocol::frame::MAX_PACKET];
    let exec_map: ExecMap = new_exec_map();
    let mut last_hb = Instant::now();
    let mut rate_limiter = RateLimiter::new();


    loop {
        match tokio::time::timeout(Duration::from_millis(100), socket.recv_from(&mut buf)).await {
            Ok(Ok((n, peer))) => {
                debug!("recv {} bytes from {}", n, peer);
                handle_packet_inner(&mut buf[..n], peer, &mut connections, &psk, &exec_map, send_socket.clone(), &mut rate_limiter).await;
            }
            Ok(Err(e)) => warn!("recv error: {}", e),
            Err(_) => {}
        }

        let now = Instant::now();
        if now.duration_since(last_hb) >= HEARTBEAT_INTERVAL {
            last_hb = now;
            debug!("hb tick: {} conns", connections.len());
            rate_limiter.gc();
            for conn in connections.values() {
                if conn.state == ConnState::Established {
                    let hb = Connection::build_heartbeat(conn.id);
                    let _ = hb_socket.send_to(&hb, conn.peer).await;
                }
            }
            connections.retain(|_id, conn| {
                let alive = now.duration_since(conn.last_activity) < TIMEOUT;
                if !alive { info!("Connection {} timed out", conn.id); }
                alive
            });
        }
    }
}

async fn handle_packet_inner(
    data: &mut [u8], peer: SocketAddr, connections: &mut HashMap<u64, Connection>, psk: &Psk, exec_map: &ExecMap,
    send_socket: Arc<UdpSocket>, rate_limiter: &mut RateLimiter) {
    let mut cursor = std::io::Cursor::new(&*data);
    let header = match PacketHeader::decode(&mut cursor) {
        Some(h) => h, None => { warn!("Bad header from {}", peer); return; }
    };
    let conn_id = header.conn_id;

    match header.pkt_type {
        PacketType::Syn => {
            // ── 速率限制：SYN ──
            if !rate_limiter.check_syn(peer.ip()) {
                warn!("SYN rate limit exceeded from {}, dropping", peer);
                return;
            }
            // ── 连接数限制 ──
            if connections.len() >= MAX_CONNECTIONS {
                warn!("MAX_CONNECTIONS ({}) reached, rejecting SYN from {}", MAX_CONNECTIONS, peer);
                return;
            }
            info!("SYN from {} (conn={})", peer, conn_id);
            let conn = Connection::new(conn_id, peer);
            let syn_ack = Connection::build_syn_ack(conn_id);
            connections.insert(conn_id, conn);
            let _ = send_socket.send_to(&syn_ack, peer).await;
        }
        PacketType::SynAck => {
            info!("SYN-ACK from {} (conn={})", peer, conn_id);
            if let Some(conn) = connections.get_mut(&conn_id) {
                conn.state = ConnState::Established;
                conn.last_activity = Instant::now();
            }
        }
        PacketType::Data => {
            let enc_start = HEADER_SIZE;
            if data.len() <= enc_start { return; }
            let ciphertext = &data[enc_start..];
            let plaintext = match crypto::decrypt(psk, &header.nonce, ciphertext) {
                Some(p) => p, None => { warn!("Decrypt failed from {}", peer); return; }
            };
            if let Some(conn) = connections.get_mut(&conn_id) {
                conn.last_activity = Instant::now();
            }
            let stream_id = header.stream_id;
            if stream_id == StreamId::Control as u16 {
                // ── 速率限制：控制命令 ──
                if !rate_limiter.check_cmd(peer.ip()) {
                    warn!("CMD rate limit exceeded from {}, dropping control msg", peer);
                    return;
                }
                let ctrl_seq = connections.get(&conn_id).map(|c| Arc::clone(&c.send_seq));
                if let Some(seq) = ctrl_seq {
                    handle_control(&plaintext, conn_id, peer, psk, &exec_map, send_socket.clone(), seq).await;
                }
            } else if stream_id == StreamId::Input as u16 {
                #[cfg(target_os = "linux")]
                handle_input_linux(&plaintext, peer);
                #[cfg(not(target_os = "linux"))]
                debug!("Input ignored (not Linux)");
            }
        }
        PacketType::Heartbeat => {
            if let Some(conn) = connections.get_mut(&conn_id) {
                conn.last_activity = Instant::now();
                let hb = Connection::build_heartbeat(conn_id);
                let _ = send_socket.send_to(&hb, conn.peer).await;
            }
        }
        PacketType::Rst => { connections.remove(&conn_id); }
        _ => {}
    }
}

#[cfg(target_os = "linux")]
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

#[cfg(target_os = "linux")]
static INJECT_COUNT: AtomicU64 = AtomicU64::new(0);
#[cfg(target_os = "linux")]
static INJECTOR: OnceLock<Mutex<LinuxInputInjector>> = OnceLock::new();

#[cfg(target_os = "linux")]
fn injector() -> std::sync::MutexGuard<'static, LinuxInputInjector> {
    INJECTOR.get_or_init(|| Mutex::new(LinuxInputInjector::new()))
        .lock().unwrap()
}

#[cfg(target_os = "linux")]
fn handle_input_linux(data: &[u8], peer: SocketAddr) {
    use lan_link_input::InputInjector;
    if let Ok(ev) = bincode::deserialize::<lan_link_input::MouseEvent>(data) {
        debug!("Mouse event from {}: {:?}", peer, ev);
        let mut inj = injector();
        let bytes = inj.inject_mouse(&ev);
        let count = INJECT_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        if bytes <= 0 || bytes % 24 != 0 { warn!("uinput mouse inject bad write: {} bytes (expected multiple of 24)", bytes); }
        if count % 50 == 0 || bytes < 0 { info!("inject_total={} bytes={} (last mouse: {:?})", count, bytes, ev); }
    } else if let Ok(ev) = bincode::deserialize::<lan_link_input::KeyEvent>(data) {
        debug!("Key event from {}: scancode={}", peer, ev.scancode);
        let mut inj = injector();
        let bytes = inj.inject_key(&ev);
        let count = INJECT_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        if bytes <= 0 || bytes % 24 != 0 { warn!("uinput key inject bad write: {} bytes (expected multiple of 24)", bytes); }
        if count % 50 == 0 || bytes < 0 { info!("inject_total={} bytes={} (last key: scancode={})", count, bytes, ev.scancode); }
    } else {
        warn!("input deserialize failed from {}: {} bytes", peer, data.len());
    }
}

async fn handle_control(
    data: &[u8], conn_id: u64, peer: SocketAddr,
    psk: &Psk, exec_map: &ExecMap,
    send_socket: Arc<UdpSocket>,
    ctrl_seq: Arc<AtomicU64>,
) {
    let msg: ControlMsg = match bincode::deserialize(data) {
        Ok(m) => m, Err(e) => { warn!("Bad control msg: {}", e); return; }
    };
    let next_seq = || ctrl_seq.fetch_add(1, Ordering::Relaxed);
    match msg {
        ControlMsg::Exec { id, cmd } => {
            info!("Exec #{}: {}", id, cmd);
            let se = match lan_link_shell::StreamingExec::spawn(&cmd) {
                Ok(s) => s,
                Err(e) => {
                    let _ = send_control(conn_id, peer, psk, next_seq(), &ControlMsg::ExecDone { id, exit_code: None }, &send_socket).await;
                    warn!("Exec #{} spawn failed: {}", id, e);
                    return;
                }
            };
            let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel::<ExecCmd>();
            exec_map.lock().await.insert(id, cmd_tx);
            let psk2 = *psk;
            let seq_clone = Arc::clone(&ctrl_seq);
            tokio::spawn(run_exec_task(id, se, cmd_rx, conn_id, peer, psk2, send_socket.clone(), seq_clone));
            let _ = send_control(conn_id, peer, psk, next_seq(), &ControlMsg::ExecStarted { id }, &send_socket).await;
        }
        ControlMsg::NativeSpawn { id, cmd } => {
            info!("NativeSpawn #{}: {:?}", id, cmd);
            let cmd_str = match &cmd {
                NativeCmdType::ShellExec { cmd, .. } => cmd.clone(),
                NativeCmdType::Tail { path, lines, follow: true, .. } => format!("tail -n {} -f {}", lines, path),
                _ => {
                    warn!("NativeSpawn #{}: unsupported cmd variant", id);
                    let _ = send_control(conn_id, peer, psk, next_seq(), &ControlMsg::ExecDone { id, exit_code: None }, &send_socket).await;
                    return;
                }
            };
            let se = match lan_link_shell::StreamingExec::spawn(&cmd_str) {
                Ok(s) => s,
                Err(e) => {
                    let _ = send_control(conn_id, peer, psk, next_seq(), &ControlMsg::ExecDone { id, exit_code: None }, &send_socket).await;
                    warn!("NativeSpawn #{} spawn failed: {}", id, e);
                    return;
                }
            };
            let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel::<ExecCmd>();
            exec_map.lock().await.insert(id, cmd_tx);
            let psk2 = *psk;
            let seq_clone = Arc::clone(&ctrl_seq);
            tokio::spawn(run_exec_task(id, se, cmd_rx, conn_id, peer, psk2, send_socket.clone(), seq_clone));
            let _ = send_control(conn_id, peer, psk, next_seq(), &ControlMsg::ExecStarted { id }, &send_socket).await;
        }
        ControlMsg::ExecStdin { id, data, close } => {
            let map = exec_map.lock().await;
            if let Some(tx) = map.get(&id) {
                let _ = tx.send(ExecCmd::Stdin(data, close));
            } else { warn!("ExecStdin for unknown id {}", id); }
        }
        ControlMsg::ExecSignal { id, signo } => {
            let map = exec_map.lock().await;
            if let Some(tx) = map.get(&id) {
                let _ = tx.send(ExecCmd::Signal(signo as i32));
            } else { warn!("ExecSignal for unknown id {}", id); }
        }
        ControlMsg::Hello { version, capabilities } => {
            info!("Hello v{} caps={:?}", version, capabilities);
            if version != PROTOCOL_VERSION {
                warn!("Incompatible protocol version: client v{}, daemon v{}", version, PROTOCOL_VERSION);
                return;
            }
            let _ = send_control(conn_id, peer, psk, next_seq(), &ControlMsg::HelloAck { version: PROTOCOL_VERSION, capabilities: vec!["exec".into(), "input".into()] }, &send_socket).await;
        }
        ControlMsg::NativeCmd { id, cmd } => {
            let (out, exit) = native_cmd::run_native_cmd(&cmd);
            let _ = send_control(conn_id, peer, psk, next_seq(), &ControlMsg::ExecChunk { id, stream: 0, data: out }, &send_socket).await;
            let _ = send_control(conn_id, peer, psk, next_seq(), &ControlMsg::ExecDone { id, exit_code: exit }, &send_socket).await;
        }

        _ => debug!("Unhandled control: {:?}", msg),
    }
}

/// Async loop for a single streaming exec. Polls the shell crate's
/// StreamingExec (which is thread-based) and forwards events to the
/// client. Uses tokio::select! to interleave chunk forwarding, stdin/
/// signal commands, and the done signal.
async fn run_exec_task(
    id: u32,
    se: lan_link_shell::StreamingExec,
    mut cmd_rx: tokio::sync::mpsc::UnboundedReceiver<ExecCmd>,
    conn_id: u64,
    peer: SocketAddr,
    psk: Psk,
    send_socket: Arc<UdpSocket>,
    ctrl_seq: Arc<AtomicU64>,
) {
    // Bridge the std::sync::mpsc channels from StreamingExec to tokio channels,
    // so we can use tokio::select! instead of busy-polling.
    let (chunk_tx, mut chunk_rx) = tokio::sync::mpsc::unbounded_channel::<lan_link_shell::StreamChunk>();
    let (done_tx, mut done_rx) = tokio::sync::oneshot::channel::<Option<i32>>();

    // Take the internal receivers and forward to tokio channels in blocking threads.
    if let Some(std_chunks) = se.take_chunks_rx() {
        tokio::task::spawn_blocking(move || {
            loop {
                match std_chunks.recv() {
                    Ok(chunk) => {
                        if chunk_tx.send(chunk).is_err() { break; }
                    }
                    Err(_) => break,
                }
            }
        });
    }
    if let Some(std_done) = se.take_done_rx() {
        tokio::task::spawn_blocking(move || {
            match std_done.recv() {
                Ok(code) => { let _ = done_tx.send(code); }
                Err(_) => { let _ = done_tx.send(None); }
            }
        });
    }

    let next_seq = || ctrl_seq.fetch_add(1, Ordering::Relaxed);

    loop {
        tokio::select! {
            Some(chunk) = chunk_rx.recv() => {
                let msg = ControlMsg::ExecChunk { id, stream: chunk.stream, data: chunk.data };
                send_control(conn_id, peer, &psk, next_seq(), &msg, &send_socket).await;
            }
            Ok(exit_code) = &mut done_rx => {
                let _ = send_control(conn_id, peer, &psk, next_seq(), &ControlMsg::ExecDone { id, exit_code }, &send_socket).await;
                break;
            }
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(ExecCmd::Stdin(data, close)) => { let _ = se.write_stdin(&data, close); }
                    Some(ExecCmd::Signal(_s)) => { let _ = se.kill(); }
                    None => { let _ = se.kill(); break; }
                }
            }
        }
    }
}

async fn send_control(conn_id: u64, peer: SocketAddr, psk: &Psk, seq: u64, msg: &ControlMsg, send_socket: &UdpSocket) {
    let payload = bincode::serialize(msg).unwrap();
    let seq32 = seq as u32;
    let nonce = crypto::make_nonce(conn_id, seq32);
    let encrypted = crypto::encrypt(psk, &nonce, &payload);
    let packet = Connection::build_encrypted_data(conn_id, StreamId::Control as u16, seq32, Flags::RELIABLE, &encrypted, nonce);
    let _ = send_socket.send_to(&packet, peer).await;
}
