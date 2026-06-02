from pathlib import Path

Path(r"G:\codex-AI-tools\lan-link\crates\daemon\src\main.rs").write_text("""\
//! lan-linkd - LAN link daemon.
//!
//! Listens for UDP connections, manages streams, and provides:
//! - Shell execution
//! - Video streaming (screen capture + encode)
//! - Audio streaming (capture + Opus) - TODO
//! - Input injection (keyboard/mouse)

use clap::Parser;
use tokio::net::UdpSocket;
use tracing::info;

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

    let psk = if let Some(ref hex_str) = args.psk {
        let bytes = hex::decode(hex_str)?;
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
""", encoding="utf-8")
print("daemon main fixed")
