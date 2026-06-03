# LL Project Design + Code Map

> 2026-06-01 cleanup. ASCII to avoid encoding issues; cross-refs to existing
> files in this repo and `G:\codex-AI-tools\tmp\` (xingdu proxy) and
> `G:\codex-AI-tools\lan-link\docs\protocol-map.md` (older GBK Chinese).

## What "LL" means here

Three sub-projects, all serving the same goal: let Codex-style LLM agents
control a remote Linux box (tuanzi 192.168.31.244) with low latency from a
Windows host (192.168.31.169).

1. **lan-link** - protocol + transport + Win/Linux endpoints
2. **xingdu (star-crossing) proxy** - OpenAI <-> Anthropic translator
   that lets Codex clients reach MiniMax / Qwen / Kimi backends
3. **MiniMax cache research** - one-shot doc scrape, kept for reference

## Sub-project 1: lan-link

### Goal
Low-latency UDP KVM (keyboard, video, mouse) + shell exec between
Windows (controller) and Linux (controlled). The interesting design
constraint is that Codex itself is the controller: its `exec_command`
tool output eventually lands in the agent's context, and `input` events
(keystrokes, mouse moves) are the only way the agent can drive a remote
desktop.

### Protocol layering (top-down)

```
+-------------------- application ----------------------+
| daemon (lan-linkd)         ctl (lan-linkctl)          |
|   - exec (Command::spawn)   - exec / status / video   |
|   - video (DXGI/DRM+VAAPI)  - file push               |
|   - input (Win kbd/mouse)                             |
|   - file (chunked)                                   |
+------------------- reliable (selective ARQ) -----------+
| protocol::reliable - 32-packet window, reorder buf  |
|   reliable for Control(0) / Input(4) / File(5)      |
+------------------- crypto (ChaCha20-Poly1305) --------+
| protocol::crypto - per-packet 96-bit nonce           |
+------------------- frame (38B header) ----------------+
| protocol::frame - conn_id / pkt_type / stream / seq  |
+------------------- transport: UDP --------------------+
```

Frame is 38-byte fixed header + up to 1400-byte encrypted payload
(+16-byte Poly1305 tag = 1454 max packet, fits MTU=1500). See
`docs/protocol-map.md` for the byte-level layout (that file is older
GBK Chinese, but the offset table is still valid).

### Workspace layout

```
lan-link/
  Cargo.toml                       # workspace
  client_win.py                    # Windows client (Python, stdlib+cryptography)
  .cargo/config.toml               # linker = rust-lld for windows-gnu
  build/                           # PyInstaller staging (regenerable)
  dist/
    lan-link-input.exe             # 11.3 MB, single-file, --daemon input
  deploy/
    lan-linkd.service              # systemd unit (Linux)
  docs/
    api-reference.md               # crate API surface (older GBK)
    protocol-map.md                # frame layout (older GBK)
    ll-design.md                   # this file
  crates/
    protocol/                      # frame, crypto, reliable, stream
    shell/                         # cross-platform exec (used by daemon)
    input/                         # WinInputCapture, LinuxInputInjector, uinput
    video/                         # VideoConfig, VideoCapture/Encoder traits
    daemon/                        # lan-linkd (main.rs, connection.rs, discovery.rs)
    ctl/                           # lan-linkctl CLI (main.rs only)
    audio/                         # STUB - default lib.rs (not wired in)
```

### Daemon (Rust) - the receiving side

- `crates/daemon/src/main.rs` (9.7 KB, last edit 2026-06-01 13:52)
  - tokio UDP listener on `0.0.0.0:9876` (default; override `-p`)
  - 100 ms recv timeout, processes packet, then every 5 s:
    - sends HEARTBEAT to each `Established` conn via a fresh bound socket
    - runs `connections.retain(|_| now - last_activity < 30 s)` - timeout
  - match on `pkt_type`:
    - Syn -> insert Connection, reply SynAck
    - SynAck -> mark Established, bump last_activity
    - Data -> decrypt via ChaCha20-Poly1305, dispatch on `stream_id`
      - Control(0) -> handle_control (Hello, Exec, FilePush, VideoStart, ...)
      - Input(4) -> `handle_input_linux` (cfg-gated; calls
        `LinuxInputInjector` defined in `crates/input/src/linux.rs`)
    - Heartbeat -> bump last_activity, reply Heartbeat
    - Rst -> drop conn
- `crates/daemon/src/connection.rs` (2.9 KB)
  - `ConnState` enum, `Connection` struct
  - `build_syn`, `build_syn_ack`, `build_data`, `build_encrypted_data`,
    `build_heartbeat` helpers
- `crates/daemon/src/discovery.rs` (0.3 KB)
  - mDNS stub (not wired in)

Constants (in `main.rs`):
- `HEARTBEAT_INTERVAL = 5 s`
- `TIMEOUT = 30 s` (retain kicks in only on the 5 s tick)
- PSK path: `/etc/lan-link/psk`

### input crate (Rust)

- `crates/input/src/lib.rs` (2.3 KB)
  - `KeyEvent { down, scancode, vk, modifiers }`
  - `MouseEvent` enum (Move / Button / Wheel)
  - `MouseButton` enum, `Modifiers` bitflags
  - `MonitorInfo` struct
  - `InputCapture` / `InputInjector` traits
  - `BorderWatcher` - tracks when cursor enters/exits the remote monitor
- `crates/input/src/linux.rs` (13.2 KB)
  - `LinuxInputInjector` - opens `/dev/uinput`, registers key/mouse bits,
    writes EV_KEY / EV_REL / EV_SYN events; uses `OnceLock<Mutex<...>>`
    so a single uinput fd is shared across all event paths
- `crates/input/src/win.rs` (3.8 KB)
  - `WinInputCapture` - GetCursorPos, returns `MouseEvent::Move { dx, dy }`
  - `WinInputInjector` - SendInput for keyboard and mouse
  - **NOTE**: this is the trait surface, NOT what client_win.py uses
    (the Python client is the active Windows path; Rust win.rs is only
    here for when `lan-linkctl` or a future Rust client ships)

### protocol crate

- `crates/protocol/src/frame.rs` (5.1 KB)
  - `PacketType` enum (Syn=0..Heartbeat=5)
  - `StreamId` enum (Control=0..File=5)
  - `Flags` bitflags (RELIABLE, FRAGMENTED, ORDERED)
  - `PacketHeader` - encode/decode, 38 bytes
  - `ControlMsg` enum (serde) - the inner bincode message
- `crates/protocol/src/crypto.rs` (2.4 KB)
  - `Psk = [u8; 32]`, `generate_psk`, `encrypt`, `decrypt`, `make_nonce`
- `crates/protocol/src/reliable.rs` (6.3 KB)
  - `ReliableSender` / `ReliableReceiver` with 32-packet SACK window
- `crates/protocol/src/stream.rs` (1.3 KB)

### ctl crate (client, Rust)

- `crates/ctl/src/main.rs` (5.5 KB) - **not yet compiled for Windows**
  - clap CLI: `lan-linkctl exec --addr 192.168.31.244:9876 --psk <hex> <cmd>`
  - also `push`, `status`, `video` subcommands
  - sends SYN, awaits SynAck, sends encrypted `ControlMsg::Hello { caps: ["exec"] }`
  - **status**: this is the one missing binary in `target/`. MSVC toolchain
    was never installed on Windows; MinGW would be the next move.

### Windows client (Python, active)

- `client_win.py` (21.3 KB, last edit 2026-06-01 15:05)
  - **stdlib + `cryptography`** only; PyInstaller-bundled into
    `dist/lan-link-input.exe` (11.3 MB)
  - Class layout (in file order):
    - `Config` - reads `%APPDATA%\lan-link\config.json`, falls back to
      `DEFAULTS` (addr, psk, log_dir, log_max_bytes, heartbeat_interval,
      reconnect_backoff)
    - `Log` - rotating file logger (`%LOCALAPPDATA%\lan-link\*.log`)
    - packet helpers: `make_nonce`, `encrypt`, `decrypt`, `build_packet`,
      `parse_header`, `ser_string`, `ser_vec_str`
    - control message packers: `control_hello`, `control_exec`
    - input event packers: `input_key`, `input_mouse_move`,
      `input_mouse_button`, `input_mouse_wheel` (bincode layout, enum
      tag u32, struct fields little-endian)
    - `LanLinkClient`:
      - `__init__` - bind UDP socket, random conn_id, settimeout 2 s
      - `send_raw`, `recv`, `connect` (SYN/SynAck/Hello dance)
      - `_start_heartbeat` - 10 s thread, sends HEARTBEAT pkt
      - `--daemon input` enables HB; one-shot exec does not
      - `main()` - argparse (`exec` / `input` / `show-config` /
        `write-config`), runs the right path
    - `run_input_capture`:
      - `SetWindowsHookExW(WH_KEYBOARD_LL, kb_proc, ...)` +
        `SetWindowsHookExW(WH_MOUSE_LL, mouse_proc, ...)`
      - inner callbacks `kb_proc` / `mouse_proc` are `WINFUNCTYPE`
        and each builds the `KeyEvent` / `MouseEvent` payload and calls
        `client.send_raw`
      - main loop is `while GetMessageW(...) > 0: TranslateMessage;
        DispatchMessageW` - GetMessageW is the natural yield point

### Build / deploy

- `dist/lan-link-input.exe` is the binary actually running
- Deployment on Windows:
  - VBS launcher in `%APPDATA%\...\Start Menu\Programs\Startup\lan-link-input.vbs`
    (runs `lan-link-input.exe --daemon input` with `0, False` window style)
  - User-level scheduled task `lan-link-input-watchdog` every 2 min
    (checks `Get-Process lan-link-input`, restarts if missing)
- Deployment on Linux (tuanzi):
  - `deploy/lan-linkd.service` (systemd) runs `lan-linkd` as root with
    PSK at `/etc/lan-link/psk`

## Sub-project 2: xingdu (star-crossing) proxy

Path: `G:\codex-AI-tools\tmp\` (NOT inside `lan-link/`).

- `dashscope_proxy_v4.py` (21.9 KB, 2026-06-01 13:07) - **active**
- `dashscope_proxy_v3.py` (21.9 KB, 2026-06-01 13:01) - predecessor
- `dashscope_proxy_v2.py` (21.8 KB, 2026-06-01 12:31) - predecessor
- `xingdu_current.py` (22.3 KB) - old `current` (kept for diff)
- `check_xingdu.py`, `parse_minimax.py`, `todo_xingdu.py` - ad-hoc
  tools, can be archived

### What it does
- Sits between Codex client and an LLM backend
- Accepts OpenAI `/v1/chat/completions`, converts request to Anthropic
  format, forwards to upstream (DashScope / MiniMax / Kimi / etc.),
  converts SSE back to OpenAI streaming format
- Routes by model name: `dashscope` block, Anthropic path, etc.
- v2 / v3 / v4 changes are listed in `todo.md` - v4 is the one running

### Active file's structure (`dashscope_proxy_v4.py`)

Functions (in file order):
- `get_backend(model)` - model -> backend config
- `_oai_image_to_anthropic(block)` - image_url -> Anthropic image source
- `_oai_content_to_anthropic(content)` - text/image_url/input_image -> blocks
- `_merge_consecutive_roles(msgs)` - **v2 feature**: collapse same-role turns
- `_truncate_messages(msgs, max_msgs)` - **v2 feature**: keep last N
- `_has_intent_text(text)` - **v2 feature**: detect "I'll do X" lazy tool calls
- `@app.route('/v1/chat/completions')` - main entry
  - `_is_lazy_response(anth_result)` - check for "describe but don't call" anti-pattern
  - `_anth_to_oai(ar, model)` - SSE event -> OpenAI chunk
  - `_do_stream_with_retry(...)` - **v2 feature**: lazy-tooling retry
  - `_stream_event(evt, model)` - format conversion
- `@app.route('/v1/models')` - list models
- `@app.route('/health')` - health check
- `app.run(host='0.0.0.0', port=9999)` - **port 9999** is where the
  proxy listens; Echobird forwards Codex traffic from 53682 to 9999

### Known follow-up
- "修复 MiniMax 偶发 JSON 解析错误" - observed in Codex client logs
  but not reproducible in the dump files I checked. Likely Codex-side.
  Should leave alone until it shows up in a captured SSE stream.

## Sub-project 3: MiniMax cache research

One-shot scrape of `platform.MiniMax.com/docs` and `MiniMax Code Plan`
pricing, kept in `tmp/`:
- `minimax_cache_doc.html` (1.9 MB) - raw HTML
- `minimax_cache_full.txt` (34 KB) - extracted text
- `minimax_llms.md` (161 KB) - llms.txt from the docs
- `minimax_token_plan.txt` (20 KB) - token-plan table

These are reference material only, not executable code. Safe to keep.

## Cleanup plan (next step)

Files in `lan-link/` root that are 2026-05-31 throwaway scripts and can
be moved to `lan-link/archive/scaffolding-2026-05-31/`:

- `fix_audio.py`, `fix_all.py`, `fix_config.py`, `fix_conn.py`,
  `fix_daemon_deps.py`, `fix_daemon_main.py`, `fix_proto.py`,
  `fix_shell.py`, `fix_win.py`, `fix_win2.py`, `fix_ws.py`
- `write_conn.py`, `write_daemon.py`, `write_daemon2.py`,
  `write_input.py`, `write_input_win.py`, `write_shell_ctl.py`,
  `write_src.py`, `write_stubs.py`, `write_video.py`
- `set_linker.py`, `set_lld.py`, `setup_cargo.py`
- `update_input_toml.py`, `update_toml.py`
- `gen_docs.py` (the older GBK Chinese docs it produced are still in
  docs/, this script is no longer needed)
- `rm_audio.py` (no-op now; audio stub crate can stay - documented above)
- `client_win.py.bak`

`crates/audio/` and the `audio` line in `Cargo.toml` are scaffold
leftovers; either wire audio up or drop the crate entirely.

`target/` (~1 GB of cargo build artifacts) and `build/` (~PyInstaller)
can be `.gitignore`d - they regenerate from source.

`docs/protocol-map.md` and `docs/api-reference.md` are older GBK
Chinese. Worth keeping but worth a UTF-8 pass when there's time.

`tmp/` files that can be archived:
- `dashscope_proxy_v2.py`, `dashscope_proxy_v3.py`, `xingdu_current.py`
  (only `dashscope_proxy_v4.py` is active)
- `check_xingdu.py`, `parse_minimax.py`, `todo_xingdu.py` (one-shot)
- `star_save*.json` (already-sent star memories; keep the latest as
  evidence, drop the rest)