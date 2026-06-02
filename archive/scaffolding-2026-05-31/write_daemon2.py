from pathlib import Path

daemon_main = r"""//! lan-linkd - LAN link daemon.
//!
//! Listens for UDP connections, manages streams, and provides:
//! - Shell execution
//! - Video streaming (screen capture + encode)
//! - Input injection (keyboard/mouse)

use clap::Parser;
use lan_link_protocol::frame::{PacketHeader, PacketType, Flags, StreamId, HEADER_SIZE};
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tracing::{info, warn, debug};

mod connection;
mod discovery;

use connection::{Connection, ConnState};

#[derive(Parser, Debug)]
#[command(name = "lan-linkd", version, about = "LAN link daemon")]
struct Args {
    #[arg(short, long, default_value = "9876")]
    port: u16,

    #[arg(short, long)]
    psk: Option<String>,

    #[arg(long, default_value = "true")]
    discovery: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let psk: lan_link_protocol::crypto::Psk = if let Some(ref hex_str) = args.psk {
        let bytes = hex::decode(hex_str)?;
        anyhow::ensure!(bytes.len() == 32, "PSK must be 32 bytes (64 hex chars)");
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        key
    } else {
        let key = lan_link_protocol::crypto::generate_psk();
        info!("Generated random PSK: {}", hex::encode(key));
        eprintln!("PSK={}", hex::encode(key));
        key
    };

    let addr = format!("0.0.0.0:{}", args.port);
    let socket = UdpSocket::bind(&addr).await?;
    info!("lan-linkd listening on {}", addr);

    if args.discovery {
        tokio::spawn(discovery::run(args.port));
    }

    let mut connections: HashMap<u64, Connection> = HashMap::new();
    let mut buf = vec![0u8; lan_link_protocol::frame::MAX_PACKET];

    loop {
        match socket.recv_from(&mut buf).await {
            Ok((n, peer)) => {
                debug!("Received {} bytes from {}", n, peer);
                handle_packet(&mut buf[..n], peer, &mut connections, &psk).await;
            }
            Err(e) => {
                warn!("recv error: {}", e);
            }
        }
    }
}

async fn handle_packet(
    data: &[u8],
    peer: SocketAddr,
    connections: &mut HashMap<u64, Connection>,
    psk: &lan_link_protocol::crypto::Psk,
) {
    let mut cursor = std::io::Cursor::new(data);
    let header = match PacketHeader::decode(&mut cursor) {
        Some(h) => h,
        None => {
            warn!("Failed to decode header from {}", peer);
            return;
        }
    };

    let conn_id = header.conn_id;

    match header.pkt_type {
        PacketType::Syn => {
            info!("SYN from {} (conn_id={})", peer, conn_id);
            let conn = Connection::new(conn_id, peer, *psk);
            let syn_ack = Connection::build_syn_ack(conn_id);
            connections.insert(conn_id, conn);

            if let Ok(socket) = UdpSocket::bind("0.0.0.0:0").await {
                let _ = socket.send_to(&syn_ack, peer).await;
            }
        }

        PacketType::SynAck => {
            info!("SYN-ACK from {} (conn_id={})", peer, conn_id);
            if let Some(conn) = connections.get_mut(&conn_id) {
                conn.state = ConnState::Established;
                info!("Connection {} established with {}", conn_id, peer);
            }
        }

        PacketType::Data => {
            // Decrypt payload
            let nonce = &header.nonce;
            let enc_start = HEADER_SIZE;
            let ciphertext = &data[enc_start..];
            let plaintext = lan_link_protocol::crypto::decrypt(psk, nonce, ciphertext);

            match plaintext {
                Some(data) => {
                    let stream_id = header.stream_id;
                    debug!("Data on stream {} ({} bytes)", stream_id, data.len());

                    // Route to appropriate handler
                    if stream_id == StreamId::Control as u16 {
                        handle_control(&data, conn_id, peer, connections).await;
                    }
                }
                None => {
                    warn!("Decryption failed for packet from {}", peer);
                }
            }
        }

        PacketType::Heartbeat => {
            // Respond with heartbeat
            if let Some(conn) = connections.get(&conn_id) {
                let hb = Connection::build_heartbeat(conn_id);
                if let Ok(socket) = UdpSocket::bind("0.0.0.0:0").await {
                    let _ = socket.send_to(&hb, conn.peer).await;
                }
            }
        }

        PacketType::Rst => {
            info!("RST from {} (conn_id={})", peer, conn_id);
            connections.remove(&conn_id);
        }

        _ => {
            debug!("Unhandled packet type {:?} from {}", header.pkt_type, peer);
        }
    }
}

async fn handle_control(
    data: &[u8],
    conn_id: u64,
    peer: SocketAddr,
    connections: &mut HashMap<u64, Connection>,
) {
    let msg: lan_link_protocol::frame::ControlMsg = match bincode::deserialize(data) {
        Ok(m) => m,
        Err(e) => {
            warn!("Failed to deserialize control message: {}", e);
            return;
        }
    };

    match msg {
        lan_link_protocol::frame::ControlMsg::Exec { id, cmd } => {
            info!("Exec request #{}: {}", id, cmd);

            // Split command into program + args
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            let (program, args) = if parts.is_empty() {
                ("", &[][..])
            } else {
                (parts[0], &parts[1..])
            };

            let result = lan_link_shell::exec(program, args);
            let output = match result {
                Ok(r) => {
                    if r.exit_code == 0 {
                        r.stdout.into_bytes()
                    } else {
                        format!("exit={}\nstdout:\n{}\nstderr:\n{}", r.exit_code, r.stdout, r.stderr)
                            .into_bytes()
                    }
                }
                Err(e) => format!("error: {}", e).into_bytes(),
            };

            let response = lan_link_protocol::frame::ControlMsg::ExecOutput {
                id,
                data: output,
                exit_code: None,
            };

            let payload = bincode::serialize(&response).unwrap();
            let packet = Connection::build_data(
                conn_id,
                StreamId::Control as u16,
                0, // seq managed by reliable layer
                Flags::RELIABLE,
                &payload,
            );

            if let Ok(socket) = UdpSocket::bind("0.0.0.0:0").await {
                let _ = socket.send_to(&packet, peer).await;
            }
        }

        lan_link_protocol::frame::ControlMsg::Hello { version, capabilities } => {
            info!("Hello from {}: v{} caps={:?}", peer, version, capabilities);
            let response = lan_link_protocol::frame::ControlMsg::HelloAck {
                version: 1,
                capabilities: vec!["exec".into(), "video".into(), "input".into()],
            };
            let payload = bincode::serialize(&response).unwrap();
            let packet = Connection::build_data(
                conn_id,
                StreamId::Control as u16,
                0,
                Flags::RELIABLE,
                &payload,
            );
            if let Ok(socket) = UdpSocket::bind("0.0.0.0:0").await {
                let _ = socket.send_to(&packet, peer).await;
            }
        }

        _ => {
            debug!("Unhandled control message: {:?}", msg);
        }
    }
}
"""

Path(r"G:\codex-AI-tools\lan-link\crates\daemon\src\main.rs").write_text(daemon_main, encoding="utf-8")
print("daemon main updated")
