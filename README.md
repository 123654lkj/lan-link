# lan-link

A **LAN remote management tool** with encrypted UDP channels and P2P VPN mesh.

## Features

- **Encrypted UDP channel** — ChaCha20-Poly1305 encryption with PSK authentication
- **50+ native commands** — file operations, system management, Docker, firewall, and more
- **Streaming exec** — real-time stdout/stderr output with stdin forwarding
- **File transfer** — push/pull with progress bar and ACK retry
- **P2P VPN mesh** — DHT-based node discovery, relay, NAT traversal (experimental)
- **Pure CLI** — no GUI dependencies, suitable for servers and headless machines

## Quick Start

### 1. Deploy daemon

```bash
# Build from source
cargo build --release

# Generate PSK and start daemon
sudo ./target/release/lan-linkd

# Or with VPN enabled
sudo ./target/release/lan-linkd --vpn --node-name server1
```

### 2. Run commands from another machine

```bash
# Set PSK (or use --psk flag)
export LAN_LINK_PSK=your_32_byte_hex_key

# Execute a command
lan-linkctl exec "whoami"

# Streaming shell
lan-linkctl iexec "tail -f /var/log/syslog"
```

## Command Line Options

### Daemon (`lan-linkd`)

| Flag | Default | Description |
|------|---------|-------------|
| `-p`, `--port` | `9876` | Listening port |
| `--psk` | — | PSK hex string (generated if not set) |
| `--discovery` | `true` | Enable mDNS discovery |
| `--vpn-port` | `9877` | VPN relay port |
| `--node-name` | hostname | Local node name for VPN |
| `--vpn` | — | Enable VPN module |

### Client (`lan-linkctl`)

| Flag | Default | Description |
|------|---------|-------------|
| `-a`, `--addr` | `192.168.31.244:9876` | Target daemon address |
| `-p`, `--psk` | — | 32-byte PSK hex |
| `--vpn` | — | VPN routing mode |

## Architecture

```
┌──────────────┐     UDP/encrypted     ┌──────────────┐
│  lan-linkctl  │ ──────────────────▶   │  lan-linkd   │
│  (client)     │ ◀──────────────────  │  (daemon)    │
└──────────────┘                       └──────────────┘
                                                │
                                          ┌─────┴─────┐
                                          │   VPN Mesh │
                                          │ (optional) │
                                          └───────────┘
```

## Development

```bash
# Build all
cargo build

# Full check
cargo check --all-targets

# Run daemon for local testing
cargo run -p lan-linkd
```

## Project Structure

```
lan-link/
├── crates/
│   ├── protocol/     # Protocol definitions, crypto, frame serialization
│   ├── daemon/       # Server-side daemon binary
│   ├── ctl/          # Client CLI binary
│   ├── shell/        # Streaming exec implementation
│   ├── video/        # Screen capture (Linux DRM/NVENC)
│   └── vpn/          # P2P VPN mesh (DHT, relay, NAT traversal)
├── docs/             # API reference, architecture docs
├── tests/            # Integration tests
└── target/           # Build artifacts
```

## License

MIT License — see [LICENSE](LICENSE).

## Repository

[https://github.com/123654lkj/lan-link](https://github.com/123654lkj/lan-link)
