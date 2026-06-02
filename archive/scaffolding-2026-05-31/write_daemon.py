from pathlib import Path

BASE = Path(r"G:\codex-AI-tools\lan-link\crates")

# ===== daemon main.rs =====
(BASE / "daemon/src").mkdir(parents=True, exist_ok=True)

main_rs = """//! lan-linkd - LAN link daemon.
//!
//! Listens for UDP connections, manages streams, and provides:
//! - Shell execution (pty)
//! - Video streaming (screen capture + encode)
//! - Audio streaming (capture + Opus)
//! - Input injection (keyboard/mouse)

use clap::Parser;
use tokio::net::UdpSocket;
use tracing::{info, warn};

mod connection;
mod discovery;

#[derive(Parser, Debug)]
#[command(name = "lan-linkd", version, about = "LAN link daemon")]
struct Args {
    /// UDP port to listen on
    #[arg(short, long, default_value = "9876")]
    port: u16,

    /// Pre-shared key (hex encoded, 64 chars for 32 bytes)
    #[arg(short, long)]
    psk: Option<String>,

    /// Enable mDNS discovery
    #[arg(long, default_value = "true")]
    discovery: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let psk = if let Some(ref hex) = args.psk {
        let bytes = hex::decode(hex)?;
        if bytes.len() != 32 {
            anyhow::bail!("PSK must be 32 bytes (64 hex chars)");
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        key
    } else {
        let key = lan_link_protocol::crypto::generate_psk();
        info!("Generated random PSK: {}", hex::encode(key));
        key
    };

    let addr = format!("0.0.0.0:{}", args.port);
    let socket = UdpSocket::bind(&addr).await?;
    info!("lan-linkd listening on {}", addr);

    if args.discovery {
        tokio::spawn(discovery::run(args.port));
    }

    let mut buf = vec![0u8; lan_link_protocol::frame::MAX_PACKET];
    loop {
        let (n, peer) = socket.recv_from(&mut buf).await?;
        info!("Received {} bytes from {}", n, peer);
        // TODO: full connection handling
    }
}
"""

(BASE / "daemon/src/main.rs").write_text(main_rs, encoding="utf-8")

# connection.rs stub
(BASE / "daemon/src/connection.rs").write_text("""\
//! Connection management.

use lan_link_protocol::crypto::Psk;
use std::net::SocketAddr;
use tokio::net::UdpSocket;

pub struct Connection {
    pub id: u64,
    pub peer: SocketAddr,
    pub psk: Psk,
}

impl Connection {
    pub fn new(id: u64, peer: SocketAddr, psk: Psk) -> Self {
        Self { id, peer, psk }
    }
}
""", encoding="utf-8")

# discovery.rs stub
(BASE / "daemon/src/discovery.rs").write_text("""\
//! mDNS-based peer discovery.
//!
//! Broadcasts _lan-link._udp service on the local network.

pub async fn run(port: u16) {
    tracing::info!("mDNS discovery started (port {})", port);
    // TODO: implement mDNS via mdns-sd or simple-mdns crate
    tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
}
""", encoding="utf-8")

# Update daemon Cargo.toml
(BASE / "daemon/Cargo.toml").write_text("""\
[package]
name = "lan-linkd"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "lan-linkd"
path = "src/main.rs"

[dependencies]
lan-link-protocol = { path = "../protocol" }
lan-link-audio = { path = "../audio" }
lan-link-video = { path = "../video" }
lan-link-input = { path = "../input" }
lan-link-shell = { path = "../shell" }
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
tracing-subscriber = "0.3"
clap = { version = "4", features = ["derive"] }
anyhow = "1"
hex = "0.4"
""", encoding="utf-8")

print("daemon files written")
