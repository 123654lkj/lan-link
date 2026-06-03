//! lan-linkd - 局域网远程管理 daemon

use clap::Parser;
use lan_link_protocol::crypto::{self, Psk};
use lan_link_protocol::frame::{PacketHeader, PacketType, Flags, StreamId, HEADER_SIZE, ControlMsg, NativeCmdType};
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
    let mut connections: HashMap<u64, Connection> = HashMap::new();
    let mut buf = vec![0u8; lan_link_protocol::frame::MAX_PACKET];
    let exec_map: ExecMap = new_exec_map();
    let mut last_hb = Instant::now();


    loop {
        match tokio::time::timeout(Duration::from_millis(100), socket.recv_from(&mut buf)).await {
            Ok(Ok((n, peer))) => {
                debug!("recv {} bytes from {}", n, peer);
                handle_packet_inner(&mut buf[..n], peer, &mut connections, &psk, &exec_map).await;
            }
            Ok(Err(e)) => warn!("recv error: {}", e),
            Err(_) => {}
        }

        let now = Instant::now();
        if now.duration_since(last_hb) >= HEARTBEAT_INTERVAL {
            last_hb = now;
            debug!("hb tick: {} conns", connections.len());
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
    data: &mut [u8], peer: SocketAddr, connections: &mut HashMap<u64, Connection>, psk: &Psk, exec_map: &ExecMap) {
    let mut cursor = std::io::Cursor::new(&*data);
    let header = match PacketHeader::decode(&mut cursor) {
        Some(h) => h, None => { warn!("Bad header from {}", peer); return; }
    };
    let conn_id = header.conn_id;

    match header.pkt_type {
        PacketType::Syn => {
            info!("SYN from {} (conn={})", peer, conn_id);
            let conn = Connection::new(conn_id, peer, *psk);
            let syn_ack = Connection::build_syn_ack(conn_id);
            connections.insert(conn_id, conn);
            if let Ok(sock) = UdpSocket::bind("0.0.0.0:0").await {
                let _ = sock.send_to(&syn_ack, peer).await;
            }
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
                handle_control(&plaintext, conn_id, peer, connections, psk, &exec_map).await;
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
                if let Ok(sock) = UdpSocket::bind("0.0.0.0:0").await {
                    let _ = sock.send_to(&hb, conn.peer).await;
                }
            }
        }
        PacketType::Rst => { connections.remove(&conn_id); }
        _ => {}
    }
}

#[cfg(target_os = "linux")]
use std::sync::OnceLock;
use std::sync::atomic::AtomicU64;
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
    _connections: &mut HashMap<u64, Connection>, psk: &Psk, exec_map: &ExecMap,
) {
    let msg: ControlMsg = match bincode::deserialize(data) {
        Ok(m) => m, Err(e) => { warn!("Bad control msg: {}", e); return; }
    };
    match msg {
        ControlMsg::Exec { id, cmd } => {
            info!("Exec #{}: {}", id, cmd);
            let se = match lan_link_shell::StreamingExec::spawn(&cmd) {
                Ok(s) => s,
                Err(e) => {
                    let _ = send_control(conn_id, peer, psk, &ControlMsg::ExecDone { id, exit_code: None }).await;
                    warn!("Exec #{} spawn failed: {}", id, e);
                    return;
                }
            };
            let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel::<ExecCmd>();
            exec_map.lock().await.insert(id, cmd_tx);
            let psk2 = *psk;
            tokio::spawn(run_exec_task(id, se, cmd_rx, conn_id, peer, psk2));
            let _ = send_control(conn_id, peer, psk, &ControlMsg::ExecStarted { id }).await;
        }
        ControlMsg::NativeSpawn { id, cmd } => {
            info!("NativeSpawn #{}: {:?}", id, cmd);
            let cmd_str = match &cmd {
                NativeCmdType::ShellExec { cmd, .. } => cmd.clone(),
                NativeCmdType::Tail { path, lines, follow: true, .. } => format!("tail -n {} -f {}", lines, path),
                _ => {
                    warn!("NativeSpawn #{}: unsupported cmd variant", id);
                    let _ = send_control(conn_id, peer, psk, &ControlMsg::ExecDone { id, exit_code: None }).await;
                    return;
                }
            };
            let se = match lan_link_shell::StreamingExec::spawn(&cmd_str) {
                Ok(s) => s,
                Err(e) => {
                    let _ = send_control(conn_id, peer, psk, &ControlMsg::ExecDone { id, exit_code: None }).await;
                    warn!("NativeSpawn #{} spawn failed: {}", id, e);
                    return;
                }
            };
            let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel::<ExecCmd>();
            exec_map.lock().await.insert(id, cmd_tx);
            let psk2 = *psk;
            tokio::spawn(run_exec_task(id, se, cmd_rx, conn_id, peer, psk2));
            let _ = send_control(conn_id, peer, psk, &ControlMsg::ExecStarted { id }).await;
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
            send_control(conn_id, peer, psk, &ControlMsg::HelloAck { version: 1, capabilities: vec!["exec".into(), "input".into()] }).await;
        }
        ControlMsg::NativeCmd { id, cmd } => {
            let (out, exit) = native_cmd::run_native_cmd(&cmd);
            send_control(conn_id, peer, psk, &ControlMsg::ExecChunk { id, stream: 0, data: out }).await;
            send_control(conn_id, peer, psk, &ControlMsg::ExecDone { id, exit_code: exit }).await;
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
) {
    use std::time::Duration;
    let poll_interval = Duration::from_millis(5);
    loop {
        // Drain any pending chunks first
        while let Some(c) = se.try_poll_chunk() {
            let msg = ControlMsg::ExecChunk { id, stream: c.stream, data: c.data };
            send_control(conn_id, peer, &psk, &msg).await;
        }
        // Process one cmd if available (non-blocking)
        match cmd_rx.try_recv() {
            Ok(ExecCmd::Stdin(data, close)) => { let _ = se.write_stdin(&data, close); }
            Ok(ExecCmd::Signal(_s)) => { let _ = se.kill(); }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {}
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => { let _ = se.kill(); break; }
        }
        // Check if exec finished
        if let Some(exit_code) = se.try_wait() {
            let _ = send_control(conn_id, peer, &psk, &ControlMsg::ExecDone { id, exit_code }).await;
            break;
        }
        tokio::time::sleep(poll_interval).await;
    }
}

async fn send_control(conn_id: u64, peer: SocketAddr, psk: &Psk, msg: &ControlMsg) {
    let payload = bincode::serialize(msg).unwrap();
    let nonce = crypto::make_nonce(conn_id, 0);
    let encrypted = crypto::encrypt(psk, &nonce, &payload);
    let packet = Connection::build_encrypted_data(conn_id, StreamId::Control as u16, 0, Flags::RELIABLE, &encrypted, nonce);
    if let Ok(sock) = UdpSocket::bind("0.0.0.0:0").await {
        let _ = sock.send_to(&packet, peer).await;
    }
}
