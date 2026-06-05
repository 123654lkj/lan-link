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
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::net::{UdpSocket, TcpListener, TcpStream};
use tracing::{info, warn, debug};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Mutex as AsyncMutex;

mod connection;
mod native_cmd;
mod discovery;
use connection::{Connection, ConnState};

/// TCP connection map: conn_id -> (shared stream, peer_addr)
type TcpConnMap = Arc<AsyncMutex<HashMap<u64, (Arc<tokio::sync::Mutex<TcpStream>>, SocketAddr)>>>;

/// Global file transfer state: id -> (path, file_handle, size)
static FILE_TRANSFERS: std::sync::LazyLock<std::sync::Mutex<HashMap<u32, (String, std::fs::File, u64)>>> = std::sync::LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));


/// Shared daemon state for TCP handler
struct DaemonState {
    connections: AsyncMutex<HashMap<u64, Connection>>,
    exec_map: ExecMap,
    psk: Psk,
    send_socket: Arc<UdpSocket>,
    rate_limiter: AsyncMutex<RateLimiter>,
    tcp_conns: TcpConnMap,
}

/// Command sent from handle_control to a running exec task.
#[derive(Debug)]
enum ExecCmd {
    Stdin(Vec<u8>, bool), // (data, close)
    Signal(i32),
}

/// Global map of running exec id -> command sender.
type ExecMap = Arc<AsyncMutex<HashMap<u32, tokio::sync::mpsc::UnboundedSender<ExecCmd>>>>;
fn new_exec_map() -> ExecMap { Arc::new(AsyncMutex::new(HashMap::new())) }

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
    #[arg(long, default_value_t = true, help = "启用 mDNS 发现")] discovery: bool,
    #[arg(long, default_value = "9877", help = "VPN 中继端口")] vpn_port: u16,
    #[arg(long, help = "本节点名称（启用 VPN 时必需）")] node_name: Option<String>,
    #[arg(long, help = "启用 VPN 模块")] vpn: bool,
    #[arg(long, default_value_t = true, help = "同时监听 TCP（同端口）")] tcp: bool,
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

    // -- VPN 初始化 --
    if args.vpn {
        let node_name = args.node_name.clone().unwrap_or_else(|| {
            info!("--node-name not specified, using hostname");
            std::fs::read_to_string("/etc/hostname").ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "unknown".into())
        });
        info!("VPN enabled: node_name={}, vpn_port={}", node_name, args.vpn_port);

        // 从 psk 生成节点 ID 以保证稳定性
        let id_bytes = {
            use sha2::{Sha256, Digest};
            let mut hasher = Sha256::new();
            hasher.update(node_name.as_bytes());
            hasher.update(&psk);
            let result = hasher.finalize();
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&result[..32]);
            arr
        };
        let node_id = lan_link_vpn::vpn::identity::NodeID::from_bytes(&id_bytes);
        let vpn_resolver = std::sync::Arc::new(lan_link_vpn::address::MemAddressResolver::new())
            as std::sync::Arc<dyn lan_link_vpn::address::AddressResolver + Send + Sync>;
        let relay_mgr = lan_link_vpn::vpn::relay::RelayManager::new(node_id, args.vpn_port);

        let vpn_handle = lan_link_vpn::vpn::vpn_router::VpnRouter::with_port(
            &node_name,
            node_id,
            vpn_resolver,
            None,
            relay_mgr,
            args.vpn_port,
        );
        match vpn_handle.start() {
            Ok(_) => info!("VPN started on port {}", args.vpn_port),
            Err(e) => warn!("VPN start failed: {:?}", e),
        }
    }

    let hb_socket = UdpSocket::bind("0.0.0.0:0").await?;
    let send_socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
    let mut connections: HashMap<u64, Connection> = HashMap::new();
    let mut buf = vec![0u8; lan_link_protocol::frame::MAX_PACKET];
    let exec_map: ExecMap = new_exec_map();
    let mut last_hb = Instant::now();
    let mut rate_limiter = RateLimiter::new();

    // Shared state for TCP handler
    let tcp_conns: TcpConnMap = Arc::new(AsyncMutex::new(HashMap::new()));
    let state = Arc::new(DaemonState {
        connections: AsyncMutex::new(HashMap::new()),
        exec_map: exec_map.clone(),
        psk,
        send_socket: send_socket.clone(),
        rate_limiter: AsyncMutex::new(RateLimiter::new()),
        tcp_conns: tcp_conns.clone(),
    });

    // Spawn TCP listener on the same port
    if args.tcp {
        let tcp_state = state.clone();
        let tcp_port = args.port;
        tokio::spawn(async move {
            if let Ok(listener) = TcpListener::bind(format!("0.0.0.0:{}", tcp_port)).await {
                info!("lan-linkd TCP listening on {}", tcp_port);
                loop {
                    match listener.accept().await {
                        Ok((stream, peer)) => {
                            info!("TCP connection from {}", peer);
                            let st = tcp_state.clone();
                            tokio::spawn(async move {
                                if let Err(e) = handle_tcp_client(stream, peer, st).await {
                                    debug!("TCP {} ended: {}", peer, e);
                                }
                            });
                        }
                        Err(e) => warn!("TCP accept error: {}", e),
                    }
                }
            } else {
                warn!("TCP bind failed on port {}", tcp_port);
            }
        });
    }

    // Main UDP loop
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

/// Handle a single TCP client connection.
/// Frame format: [4-byte BE length][ll frame data]
/// Replies are sent back on the same TCP stream.
async fn handle_tcp_client(
    stream: TcpStream,
    peer: SocketAddr,
    state: Arc<DaemonState>,
) -> anyhow::Result<()> {
    let stream = Arc::new(tokio::sync::Mutex::new(stream));
    let mut read_buf = vec![0u8; lan_link_protocol::frame::MAX_PACKET + 4];

    loop {
        // Read 4-byte length prefix
        let mut len_buf = [0u8; 4];
        {
            let mut s = stream.lock().await;
            if tokio::io::AsyncReadExt::read_exact(&mut *s, &mut len_buf).await.is_err() { break; }
        }
        let frame_len = u32::from_be_bytes(len_buf) as usize;
        if frame_len == 0 || frame_len > read_buf.len() { break; }

        // Read frame data
        {
            let mut s = stream.lock().await;
            if tokio::io::AsyncReadExt::read_exact(&mut *s, &mut read_buf[..frame_len]).await.is_err() { break; }
        }

        // Parse header to get conn_id
        let mut cursor = std::io::Cursor::new(&read_buf[..frame_len]);
        let header = match PacketHeader::decode(&mut cursor) {
            Some(h) => h,
            None => continue,
        };
        let conn_id = header.conn_id;

        debug!("TCP recv {} bytes from {} (conn={})", frame_len, peer, conn_id);

        // Register TCP connection so replies go via TCP
        {
            let mut tc = state.tcp_conns.lock().await;
            tc.entry(conn_id).or_insert_with(|| (stream.clone(), peer));
        }

        // Process the packet
        let mut rate_lim = state.rate_limiter.lock().await;
        let mut conns = state.connections.lock().await;
        let data = &mut read_buf[..frame_len];
        handle_packet_tcp(data, peer, &mut conns, &state.psk, &state.exec_map,
                          state.send_socket.clone(), &mut rate_lim, &state.tcp_conns).await;
    }

    // Cleanup
    {
        let mut tc = state.tcp_conns.lock().await;
        tc.retain(|_, (_, p)| *p != peer);
    }

    info!("TCP {} disconnected", peer);
    Ok(())
}

/// Handle a packet received over TCP. Same logic as handle_packet_inner
/// but replies go back via TCP stream instead of UDP.
async fn handle_packet_tcp(
    data: &mut [u8], peer: SocketAddr,
    connections: &mut HashMap<u64, Connection>, psk: &Psk, exec_map: &ExecMap,
    send_socket: Arc<UdpSocket>, rate_limiter: &mut RateLimiter,
    tcp_conns: &TcpConnMap,
) {
    let mut cursor = std::io::Cursor::new(&*data);
    let header = match PacketHeader::decode(&mut cursor) {
        Some(h) => h, None => { warn!("Bad header from TCP {}", peer); return; }
    };
    let conn_id = header.conn_id;

    match header.pkt_type {
        PacketType::Syn => {
            if !rate_limiter.check_syn(peer.ip()) {
                warn!("SYN rate limit from TCP {}", peer);
                return;
            }
            if let Some(existing) = connections.get(&conn_id) {
                if existing.peer != peer {
                    warn!("SYN for existing conn {} from different TCP peer", conn_id);
                    return;
                }
                // Refresh
                let syn_ack = Connection::build_syn_ack(conn_id);
                let _ = tcp_send(tcp_conns, conn_id, &syn_ack).await;
                return;
            }
            info!("SYN from TCP {} (conn={})", peer, conn_id);
            let mut conn = Connection::new(conn_id, peer);
            conn.state = ConnState::Established;
            let syn_ack = Connection::build_syn_ack(conn_id);
            connections.insert(conn_id, conn);
            let _ = tcp_send(tcp_conns, conn_id, &syn_ack).await;
        }
        PacketType::Data => {
            match connections.get(&conn_id) {
                Some(conn) if conn.state == ConnState::Established => {}
                _ => { warn!("Data on non-established TCP conn {}", conn_id); return; }
            }
            let enc_start = HEADER_SIZE;
            if data.len() <= enc_start { return; }
            let ciphertext = &data[enc_start..];
            let plaintext = match crypto::decrypt(psk, &header.nonce, ciphertext) {
                Some(p) => p, None => { warn!("Decrypt failed from TCP {}", peer); return; }
            };
            if let Some(conn) = connections.get_mut(&conn_id) {
                conn.last_activity = Instant::now();
            }
            let stream_id = header.stream_id;
            if stream_id == StreamId::Control as u16 {
                if !rate_limiter.check_cmd(peer.ip()) { return; }
                let ctrl_seq = connections.get(&conn_id).map(|c| Arc::clone(&c.send_seq));
                if let Some(seq) = ctrl_seq {
                    handle_control_tcp(&plaintext, conn_id, peer, psk, exec_map,
                                       send_socket, seq, tcp_conns).await;
                }
            }
        }
        PacketType::Heartbeat => {
            if let Some(conn) = connections.get_mut(&conn_id) {
                if conn.state == ConnState::Established {
                    conn.last_activity = Instant::now();
                }
            }
        }
        PacketType::Rst => { connections.remove(&conn_id); }
        _ => {}
    }
}

/// Send a framed message back via TCP stream.
async fn tcp_send(tcp_conns: &TcpConnMap, conn_id: u64, data: &[u8]) -> anyhow::Result<()> {
    let tc = tcp_conns.lock().await;
    if let Some((stream, _peer)) = tc.get(&conn_id) {
        let mut s = stream.lock().await;
        let len_bytes = (data.len() as u32).to_be_bytes();
        tokio::io::AsyncWriteExt::write_all(&mut *s, &len_bytes).await?;
        tokio::io::AsyncWriteExt::write_all(&mut *s, data).await?;
        Ok(())
    } else {
        Err(anyhow::anyhow!("no TCP conn for {}", conn_id))
    }
}

/// handle_control variant that sends replies via TCP.
async fn handle_control_tcp(
    data: &[u8], conn_id: u64, peer: SocketAddr,
    psk: &Psk, exec_map: &ExecMap,
    _send_socket: Arc<UdpSocket>,
    ctrl_seq: Arc<AtomicU64>,
    tcp_conns: &TcpConnMap,
) {
    let msg: ControlMsg = match bincode::deserialize(data) {
        Ok(m) => m, Err(e) => { warn!("Bad control msg from TCP: {}", e); return; }
    };
    let next_seq = || ctrl_seq.fetch_add(1, Ordering::Relaxed);

    match msg {
        ControlMsg::Hello { version, capabilities } => {
            info!("TCP Hello v{} caps={:?}", version, capabilities);
            if version != PROTOCOL_VERSION { return; }
            let pkt = build_control_packet_tcp(conn_id, psk, next_seq(),
                &ControlMsg::HelloAck { version: PROTOCOL_VERSION, capabilities: vec!["exec".into(), "input".into()] });
            let _ = tcp_send(tcp_conns, conn_id, &pkt).await;
        }
        ControlMsg::NativeCmd { id, ref cmd } => {
            let psk2 = *psk;
            let tc2 = tcp_conns.clone();
            let cmd_clone = cmd.clone();
            let (out, exit) = tokio::task::spawn_blocking(move || native_cmd::run_native_cmd(&cmd_clone)).await
                .unwrap_or((b"NativeCmd: spawn_blocking failed\n".to_vec(), Some(1)));
            let pkt = build_control_packet_tcp(conn_id, &psk2, next_seq(), &ControlMsg::ExecChunk { id, stream: 0, data: out });
            let _ = tcp_send(&tc2, conn_id, &pkt).await;
            let pkt = build_control_packet_tcp(conn_id, &psk2, next_seq(), &ControlMsg::ExecDone { id, exit_code: exit });
            let _ = tcp_send(&tc2, conn_id, &pkt).await;
        }
        ControlMsg::NativeSpawn { id, ref cmd } => {
            let cmd_str = match cmd {
                NativeCmdType::ShellExec { cmd, .. } => cmd.clone(),
                NativeCmdType::Tail { path, lines, follow: true, follow_secs } => {
                    if *follow_secs > 0 { format!("timeout {} tail -n {} -f {}", follow_secs, lines, path) }
                    else { format!("tail -n {} -f {}", lines, path) }
                }
                _ => {
                    // For other NativeCmdType variants, use the NativeCmd path instead
                    let psk2 = *psk;
                    let tc2 = tcp_conns.clone();
                    let cmd_clone = cmd.clone();
                    let (out, exit) = tokio::task::spawn_blocking(move || native_cmd::run_native_cmd(&cmd_clone)).await
                        .unwrap_or((b"spawn_blocking failed\n".to_vec(), Some(1)));
                    let pkt = build_control_packet_tcp(conn_id, &psk2, next_seq(), &ControlMsg::ExecChunk { id, stream: 0, data: out });
                    let _ = tcp_send(&tc2, conn_id, &pkt).await;
                    let pkt = build_control_packet_tcp(conn_id, &psk2, next_seq(), &ControlMsg::ExecDone { id, exit_code: exit });
                    let _ = tcp_send(&tc2, conn_id, &pkt).await;
                    return;
                }
            };
            info!("TCP NativeSpawn #{}: {}", id, cmd_str);
            let se = match lan_link_shell::StreamingExec::spawn(&cmd_str) {
                Ok(s) => s,
                Err(e) => {
                    let pkt = build_control_packet_tcp(conn_id, psk, next_seq(), &ControlMsg::ExecDone { id, exit_code: None });
                    let _ = tcp_send(tcp_conns, conn_id, &pkt).await;
                    warn!("TCP NativeSpawn #{} spawn failed: {}", id, e);
                    return;
                }
            };
            let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel::<ExecCmd>();
            exec_map.lock().await.insert(id, cmd_tx);
            let psk2 = *psk;
            let seq_clone = Arc::clone(&ctrl_seq);
            let tcp_conns2 = tcp_conns.clone();
            tokio::spawn(run_exec_task_tcp(id, se, cmd_rx, conn_id, peer, psk2, tcp_conns2, seq_clone));
            let pkt = build_control_packet_tcp(conn_id, psk, next_seq(), &ControlMsg::ExecStarted { id });
            let _ = tcp_send(tcp_conns, conn_id, &pkt).await;
        }
        ControlMsg::Exec { id, ref cmd } => {
            info!("TCP Exec #{}: {}", id, cmd);
            let se = match lan_link_shell::StreamingExec::spawn(cmd) {
                Ok(s) => s,
                Err(_e) => {
                    let pkt = build_control_packet_tcp(conn_id, psk, next_seq(), &ControlMsg::ExecDone { id, exit_code: None });
                    let _ = tcp_send(tcp_conns, conn_id, &pkt).await;
                    return;
                }
            };
            let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel::<ExecCmd>();
            exec_map.lock().await.insert(id, cmd_tx);
            let psk2 = *psk;
            let seq_clone = Arc::clone(&ctrl_seq);
            let tcp_conns2 = tcp_conns.clone();
            tokio::spawn(run_exec_task_tcp(id, se, cmd_rx, conn_id, peer, psk2, tcp_conns2, seq_clone));
            let pkt = build_control_packet_tcp(conn_id, psk, next_seq(), &ControlMsg::ExecStarted { id });
            let _ = tcp_send(tcp_conns, conn_id, &pkt).await;
        }
        ControlMsg::ExecStdin { id, data, close } => {
            let map = exec_map.lock().await;
            if let Some(tx) = map.get(&id) {
                let _ = tx.send(ExecCmd::Stdin(data, close));
            }
        }
        ControlMsg::ExecSignal { id, signo } => {
            let map = exec_map.lock().await;
            if let Some(tx) = map.get(&id) {
                let _ = tx.send(ExecCmd::Signal(signo as i32));
            }
        }
        ControlMsg::FilePush { id, path, size } => {
            info!("TCP FilePush #{}: {} ({} bytes)", id, path, size);
            match std::fs::OpenOptions::new().write(true).create(true).truncate(true).open(&path) {
                Ok(file) => {
                    FILE_TRANSFERS.lock().unwrap().insert(id, (path.clone(), file, size));
                    let pkt = build_control_packet_tcp(conn_id, psk, next_seq(), &ControlMsg::FileAck { id, offset: 0 });
                    let _ = tcp_send(tcp_conns, conn_id, &pkt).await;
                }
                Err(e) => { warn!("TCP FilePush #{} create failed: {}", id, e); }
            }
        }
        ControlMsg::FileChunk { id, offset, data: chunk } => {
            let ack_off = {
                let mut ft = FILE_TRANSFERS.lock().unwrap();
                if let Some((_path, file, _size)) = ft.get_mut(&id) {
                    let _ = file.seek(SeekFrom::Start(offset));
                    if let Err(e) = file.write_all(&chunk) {
                        warn!("TCP FileChunk #{} write error: {}", id, e);
                        return;
                    }
                    let ack = offset + chunk.len() as u64;
                    let done = ack >= *_size;
                    if done {
                        info!("TCP FilePush #{} complete: {} bytes", id, ack);
                        ft.remove(&id);
                    }
                    ack
                } else {
                    warn!("TCP FileChunk #{}: no active transfer", id);
                    return;
                }
            };
            let pkt = build_control_packet_tcp(conn_id, psk, next_seq(), &ControlMsg::FileAck { id, offset: ack_off });
            let _ = tcp_send(tcp_conns, conn_id, &pkt).await;
        }
        _ => debug!("Unhandled TCP control: {:?}", msg),
    }
}

/// Build an encrypted control packet (same as send_control but returns bytes instead of sending).
fn build_control_packet_tcp(conn_id: u64, psk: &Psk, seq: u64, msg: &ControlMsg) -> Vec<u8> {
    let payload = bincode::serialize(msg).unwrap();
    let seq32 = seq as u32;
    let nonce = crypto::make_nonce(conn_id, seq32);
    let encrypted = match crypto::encrypt(psk, &nonce, &payload) {
        Ok(e) => e,
        Err(_) => { warn!("encrypt failed for conn {}", conn_id); return vec![]; }
    };
    let mut buf = Vec::with_capacity(HEADER_SIZE + encrypted.len());
    PacketHeader { conn_id, pkt_type: PacketType::Data, flags: Flags::RELIABLE,
        stream_id: StreamId::Control as u16, seq: seq32,
        ack_seq: 0, ack_bitmap: 0, payload_len: encrypted.len() as u16, nonce
    }.encode(&mut buf);
    buf.extend_from_slice(&encrypted);
    buf
}

/// Exec task variant that sends output via TCP.
async fn run_exec_task_tcp(
    id: u32,
    se: lan_link_shell::StreamingExec,
    mut cmd_rx: tokio::sync::mpsc::UnboundedReceiver<ExecCmd>,
    conn_id: u64,
    _peer: SocketAddr,
    psk: Psk,
    tcp_conns: TcpConnMap,
    ctrl_seq: Arc<AtomicU64>,
) {
    let (chunk_tx, mut chunk_rx) = tokio::sync::mpsc::unbounded_channel::<lan_link_shell::StreamChunk>();
    let (done_tx, mut done_rx) = tokio::sync::oneshot::channel::<Option<i32>>();

    if let Some(std_chunks) = se.take_chunks_rx() {
        tokio::task::spawn_blocking(move || {
            loop {
                match std_chunks.recv() {
                    Ok(chunk) => { if chunk_tx.send(chunk).is_err() { break; } }
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
                let pkt = build_control_packet_tcp(conn_id, &psk, next_seq(), &msg);
                let _ = tcp_send(&tcp_conns, conn_id, &pkt).await;
            }
            Ok(exit_code) = &mut done_rx => {
                let pkt = build_control_packet_tcp(conn_id, &psk, next_seq(),
                    &ControlMsg::ExecDone { id, exit_code });
                let _ = tcp_send(&tcp_conns, conn_id, &pkt).await;
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
            // Check if conn_id already exists from a different peer
            if let Some(existing) = connections.get(&conn_id) {
                if existing.peer != peer {
                    warn!("SYN for existing conn {} from different peer {}, ignoring", conn_id, peer);
                    return;
                }
                // Same peer: refresh
                info!("SYN refresh from {} (conn={})", peer, conn_id);
                let syn_ack = Connection::build_syn_ack(conn_id);
                let _ = send_socket.send_to(&syn_ack, peer).await;
                return;
            }
            info!("SYN from {} (conn={})", peer, conn_id);
            let mut conn = Connection::new(conn_id, peer);
            conn.state = ConnState::Established;
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
            // Verify source IP matches connection
            match connections.get(&conn_id) {
                Some(conn) if conn.state == ConnState::Established && conn.peer == peer => {}
                Some(conn) if conn.state == ConnState::Established && conn.peer != peer => {
                    warn!("Data from wrong peer {} for conn {} (expected {}), dropping", peer, conn_id, conn.peer);
                    return;
                }
                _ => { warn!("Data on non-established conn {} from {}", conn_id, peer); return; }
            }
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
        }
            }

        PacketType::Heartbeat => {
            if let Some(conn) = connections.get_mut(&conn_id) {
                if conn.state != ConnState::Established || conn.peer != peer {
                    return;
                }
                conn.last_activity = Instant::now();
                let hb = Connection::build_heartbeat(conn_id);
                let _ = send_socket.send_to(&hb, conn.peer).await;
            }
        }
        PacketType::Rst => { connections.remove(&conn_id); }
        _ => {}
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
                NativeCmdType::Tail { path, lines, follow: true, follow_secs } => {
                    if *follow_secs > 0 {
                        format!("timeout {} tail -n {} -f {}", follow_secs, lines, path)
                    } else {
                        format!("tail -n {} -f {}", lines, path)
                    }
                }
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
            let psk2 = *psk;
            let sock2 = send_socket.clone();
            let peer2 = peer;
            // Run in blocking thread to not stall the async event loop
            let (out, exit) = tokio::task::spawn_blocking(move || {
                native_cmd::run_native_cmd(&cmd)
            }).await.unwrap_or((b"NativeCmd: spawn_blocking failed\n".to_vec(), Some(1)));
            let _ = send_control(conn_id, peer2, &psk2, next_seq(), &ControlMsg::ExecChunk { id, stream: 0, data: out }, &sock2).await;
            let _ = send_control(conn_id, peer2, &psk2, next_seq(), &ControlMsg::ExecDone { id, exit_code: exit }, &sock2).await;
        }

        ControlMsg::FilePush { id, path, size } => {
            info!("FilePush #{}: {} ({} bytes)", id, path, size);
            match std::fs::OpenOptions::new().write(true).create(true).truncate(true).open(&path) {
                Ok(file) => {
                    let _ft = FILE_TRANSFERS.lock().unwrap().insert(id, (path.clone(), file, size));
                    let _ = send_control(conn_id, peer, psk, next_seq(), &ControlMsg::FileAck { id, offset: 0 }, &send_socket).await;
                }
                Err(e) => {
                    warn!("FilePush #{} create failed: {}", id, e);
                }
            }
        }
        ControlMsg::FileChunk { id, offset, data: chunk } => {
            let mut ft = FILE_TRANSFERS.lock().unwrap();
            if let Some((_path, file, _size)) = ft.get_mut(&id) {
                let _ = file.seek(SeekFrom::Start(offset));
                if let Err(e) = file.write_all(&chunk) {
                    warn!("FileChunk #{} write error: {}", id, e);
                    drop(ft);
                    return;
                }
                let ack_off = offset + chunk.len() as u64;
                // Check if transfer complete
                let done = ack_off >= *_size;
                if done {
                    info!("FilePush #{} complete: {} bytes", id, ack_off);
                    ft.remove(&id);
                }
                drop(ft);
                let _ = send_control(conn_id, peer, psk, next_seq(), &ControlMsg::FileAck { id, offset: ack_off }, &send_socket).await;
            } else {
                warn!("FileChunk #{}: no active transfer", id);
            }
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
    let encrypted = match crypto::encrypt(psk, &nonce, &payload) {
        Ok(e) => e,
        Err(_) => { warn!("encrypt failed for conn {}", conn_id); return; }
    };
    let packet = Connection::build_encrypted_data(conn_id, StreamId::Control as u16, seq32, Flags::RELIABLE, &encrypted, nonce);
    let _ = send_socket.send_to(&packet, peer).await;
}
