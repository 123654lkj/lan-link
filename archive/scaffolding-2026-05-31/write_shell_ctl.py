from pathlib import Path

BASE = Path(r"G:\codex-AI-tools\lan-link\crates")

# ===== shell lib.rs =====
shell_lib = """//! Shell engine: pty-based command execution.
//!
//! Uses portable-pty for cross-platform pseudoterminal support.
//! On Linux: creates a pty pair via openpty/forkpty.
//! On Windows: uses ConPTY (Windows 10 1809+).

use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::io::{Read, Write};
use std::sync::mpsc;
use std::thread;

/// A running shell session.
pub struct ShellSession {
    /// Channel to send commands into the pty
    tx: mpsc::Sender<Vec<u8>>,
    /// Channel to receive output from the pty
    rx: mpsc::Receiver<Vec<u8>>,
    child: Option<Box<dyn portable_pty::Child + Send>>,
}

impl ShellSession {
    /// Spawn a new shell (cmd.exe on Windows, bash on Linux).
    pub fn spawn() -> anyhow::Result<Self> {
        let sys = native_pty_system();
        let size = PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = sys.openpty(size)?;

        let cmd = if cfg!(windows) {
            CommandBuilder::new("cmd.exe")
        } else {
            CommandBuilder::new("bash")
        };

        let child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader()?;
        let mut writer = pair.master.take_writer()?;

        let (tx_in, rx_in) = mpsc::channel::<Vec<u8>>();
        let (tx_out, rx_out) = mpsc::channel::<Vec<u8>>();

        // Output reader thread
        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx_out.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // Input writer thread
        thread::spawn(move || {
            while let Ok(data) = rx_in.recv() {
                if writer.write_all(&data).is_err() {
                    break;
                }
            }
        });

        Ok(Self {
            tx: tx_in,
            rx: rx_out,
            child: Some(child),
        })
    }

    /// Send input to the shell.
    pub fn write(&self, data: &[u8]) -> anyhow::Result<()> {
        self.tx.send(data.to_vec())?;
        Ok(())
    }

    /// Try to read output (non-blocking).
    pub fn try_read(&self) -> Vec<u8> {
        let mut result = vec![];
        while let Ok(chunk) = self.rx.try_recv() {
            result.extend_from_slice(&chunk);
        }
        result
    }
}

impl Drop for ShellSession {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
"""

(BASE / "shell/src/lib.rs").write_text(shell_lib, encoding="utf-8")

# ===== ctl main.rs =====
(BASE / "ctl/src").mkdir(parents=True, exist_ok=True)

ctl_main = """//! lan-linkctl - CLI client for controlling remote lan-linkd instances.
//!
//! Commands: exec, file push/pull, status, connect.

use clap::{Parser, Subcommand};
use lan_link_protocol::crypto::Psk;
use std::net::SocketAddr;
use tokio::net::UdpSocket;

#[derive(Parser, Debug)]
#[command(name = "lan-linkctl", version, about = "LAN link control client")]
struct Cli {
    /// Remote daemon address
    #[arg(short, long, default_value = "192.168.31.244:9876")]
    addr: String,

    /// Pre-shared key (hex encoded)
    #[arg(short, long)]
    psk: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Execute a command on the remote shell
    Exec {
        cmd: Vec<String>,
    },
    /// Push a file to the remote
    Push {
        #[arg(short)]
        local: String,
        #[arg(short)]
        remote: String,
    },
    /// Check connection status
    Status,
    /// Start video streaming
    Video {
        #[arg(long, default_value = "1920")]
        width: u16,
        #[arg(long, default_value = "1080")]
        height: u16,
        #[arg(long, default_value = "60")]
        fps: u8,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    // Parse PSK
    let psk_bytes = hex::decode(&cli.psk)?;
    if psk_bytes.len() != 32 {
        anyhow::bail!("PSK must be 32 bytes (64 hex chars)");
    }
    let mut psk: Psk = [0u8; 32];
    psk.copy_from_slice(&psk_bytes);

    let remote: SocketAddr = cli.addr.parse()?;
    let local = "0.0.0.0:0".parse::<SocketAddr>()?;
    let socket = UdpSocket::bind(local).await?;

    match cli.command {
        Command::Exec { cmd } => {
            let cmd_str = cmd.join(" ");
            tracing::info!("exec: {}", cmd_str);
            // TODO: send ControlMsg::Exec, wait for response
            let _ = socket.send_to(b"TODO exec", remote).await?;
        }
        Command::Push { local: l, remote: r } => {
            tracing::info!("push {} -> {}", l, r);
            let _ = socket.send_to(b"TODO push", remote).await?;
        }
        Command::Status => {
            tracing::info!("checking status of {}", remote);
            let _ = socket.send_to(b"TODO status", remote).await?;
        }
        Command::Video { width, height, fps } => {
            tracing::info!("video: {}x{} @ {}fps", width, height, fps);
            let _ = socket.send_to(b"TODO video", remote).await?;
        }
    }

    Ok(())
}
"""

(BASE / "ctl/src/main.rs").write_text(ctl_main, encoding="utf-8")

# Update ctl Cargo.toml
(BASE / "ctl/Cargo.toml").write_text("""\
[package]
name = "lan-linkctl"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "lan-linkctl"
path = "src/main.rs"

[dependencies]
lan-link-protocol = { path = "../protocol" }
tokio = { version = "1", features = ["full"] }
clap = { version = "4", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = "0.3"
anyhow = "1"
hex = "0.4"
""", encoding="utf-8")

print("shell + ctl files written")
