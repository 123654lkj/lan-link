# 代码结构说明

## 工作空间整体结构

项目是一个 Cargo 工作空间，包含 7 个 crate，组织在 `crates/` 目录下：

```
lan-link/
├── Cargo.toml              # 工作空间定义（resolver = "2"）
├── crates/
│   ├── protocol/           # 协议层 — 帧格式、加密、ARQ、流复用
│   ├── daemon/             # 服务端守护进程 — 连接管理、命令执行
│   ├── ctl/                # CLI 客户端 — 50+ 管理子命令
│   ├── gui/                # GUI 客户端 — eframe/egui 桌面应用
│   ├── shell/              # 命令执行引擎 — 同步/流式执行
│   ├── input/              # 输入注入 — Linux uinput / Windows SendInput
│   └── video/              # 视频引擎 — 屏幕捕获与编码（预留/桩）
├── deploy/                 # 部署配置（systemd）
├── tests/                  # 端到端集成测试（Python）
└── docs/                   # 文档
```

### 各 crate 职责

| Crate | 类型 | 二进制/库名 | 职责 |
|-------|------|-------------|------|
| `protocol` | 库 | `lan-link-protocol` | 定义所有 crate 共享的协议类型：帧编码（`PacketHeader`）、加密（ChaCha20-Poly1305）、可靠传输（选择性 ARQ）、流复用（`StreamMux`） |
| `daemon` | 二进制 | `lan-linkd` | UDP 服务端守护进程。管理连接状态机（SYN→SYN-ACK→Established→Heartbeat→Closed），接收 ControlMsg 并调度到 `native_cmd`/`shell`/`input` |
| `ctl` | 二进制 | `lan-linkctl` | 命令行客户端。50+ 子命令，每个映射到 `NativeCmdType` 或 `ControlMsg::Exec`，通过加密 UDP 与 daemon 通信 |
| `gui` | 二进制 | `lan-link-gui` | 跨平台桌面 GUI。使用 eframe/egui 框架，提供主机管理、快捷命令、终端输出、历史记录 |
| `shell` | 库 | `lan-link-shell` | 命令执行引擎。提供 `exec()`（同步）、`exec_with_input()`、`StreamingExec`（流式）三种模式。Unix 用 `sh -c`，Windows 用 `cmd /C` |
| `input` | 库 | `lan-link-input` | 输入注入。定义 `InputCapture`/`InputInjector` trait。Linux 通过 `evdev` 捕获，`uinput` 注入；Windows 通过 `SendInput` 注入 |
| `video` | 库 | `lan-link-video` | 视频捕获与编码（预留）。定义 `VideoCapture`/`VideoEncoder` trait。所有实现目前是桩 |

---

## 各 crate 的目录结构和文件说明

### 1. `crates/protocol/` — 协议层

```
protocol/
├── Cargo.toml
└── src/
    ├── lib.rs        # 模块声明，re-export
    ├── frame.rs      # [核心] PacketHeader / ControlMsg / NativeCmdType 定义与编解码
    ├── crypto.rs     # [核心] ChaCha20-Poly1305 加密/解密，PSK 生成，nonce 推导
    ├── reliable.rs   # 选择性重传 ARQ：ReliableSender / ReliableReceiver
    └── stream.rs     # 流多路复用：StreamMux / MuxStream
```

**文件说明：**

- **`lib.rs`** — 导出 `frame`、`crypto`、`reliable`、`stream` 四个公共模块。其他 crate 通过 `lan_link_protocol::frame::...` 等路径引用。
- **`frame.rs`** — 整个项目的"字典"文件。
  - `PacketHeader`：38 字节定长头部，包含 `conn_id`、`pkt_type`、`flags`、`stream_id`、`seq`、`ack_seq`、`ack_bitmap`、`payload_len`、`nonce`。提供 `encode()`/`decode()` 方法。
  - `PacketType`：枚举（Syn/SynAck/Ack/Data/Rst/Heartbeat）
  - `StreamId`：枚举（Control/Video/AudioTx/AudioRx/Input/File），附带 `is_reliable()` 方法
  - `Flags`：bitflags（RELIABLE/FRAGMENTED/ORDERED）
  - `ControlMsg`：控制消息枚举，bincode 序列化。包含 `Exec`、`ExecChunk`、`ExecDone`、`ExecStdin`、`NativeCmd`、`NativeSpawn`、`Hello`/`HelloAck`、`KeyEvent`、`MouseMove`、`FilePush`/`FileChunk`/`FileAck`、`VideoStart`/`VideoStop`、`AudioStart`/`AudioStop` 等。
  - `NativeCmdType`：原生命令枚举，约 50 个变体。分为 Filesystem、System、Network、Management 四大类。
  - 子动作枚举：`ServiceActionType`、`PkgActionType`、`DockerActionType`、`CrontabActionType`
- **`crypto.rs`** — 加密模块。
  - `Psk = [u8; 32]` 类型别名
  - `generate_psk()` / `encrypt()` / `decrypt()` / `make_nonce()` 四个函数
- **`reliable.rs`** — 可靠传输层。
  - `ReliableSender`：32 包滑动窗口，200ms RTO，最多 10 次重传。`send()` → `on_ack()` → `poll_retransmit()`
  - `ReliableReceiver`：乱序缓冲 + 顺序递送。`deliver()` → `ack_info()`
- **`stream.rs`** — 流复用器。
  - `MuxStream`：单流句柄，管理 `send_seq`
  - `StreamMux`：`HashMap<u16, MuxStream>`，预创建 6 个标准流（0-5）

### 2. `crates/daemon/` — 守护进程

```
daemon/
├── Cargo.toml
└── src/
    ├── main.rs            # UDP 事件循环 + 包分发 + 心跳定时器
    ├── lib.rs             # 占位
    ├── connection.rs      # 连接状态机：Connection / ConnState
    ├── discovery.rs       # mDNS 服务发现（TODO 桩）
    └── native_cmd/
        ├── mod.rs         # run_native_cmd() 分发器 — 匹配 NativeCmdType 所有变体
        ├── helper.rs      # 辅助函数：read_proc(), run_cmd(), hfmt() 等
        ├── fs.rs          # 文件系统命令实现：cmd_ls, cmd_cat, cmd_tail, cmd_head 等
        ├── system.rs      # 系统命令实现：cmd_ps, cmd_kill, cmd_free, cmd_top 等
        ├── network.rs     # 网络命令实现：cmd_netstat, cmd_portscan, cmd_dns, cmd_arp
        ├── service.rs     # 服务管理：cmd_service, cmd_journal, cmd_pkg, cmd_docker, cmd_crontab
        └── exec.rs        # Shell/批处理执行：cmd_batch_content, cmd_watch_fn, cmd_sed, cmd_shell_exec
```

**文件说明：**

- **`main.rs`** — 入口点。
  - `Args`：clap 命令行参数（`--port`、`--psk`、`--discovery`）
  - `load_or_generate_psk()`：加载或自动生成 PSK
  - `main()`：绑定 UDP socket，循环 `recv_from()` → `handle_packet_inner()`，每 100ms 轮询，每 5 秒发心跳 + 清理超时连接
  - `handle_packet_inner()`：按 `PacketType` 分发（Syn → 创建连接 → SynAck；Data → 解密 → 按 StreamId 分发到 `handle_control()` 或 `handle_input_linux()`）
  - `handle_control()`：反序列化 `ControlMsg`，按变体分发（Exec → 启动 StreamingExec；NativeCmd → 调用 run_native_cmd；NativeSpawn → 启动流式；ExecStdin/Signal → 转发）
  - `run_exec_task()`：桥接 `shell::StreamingExec` 的 `std::sync::mpsc` → tokio `mpsc`，用 `tokio::select!` 多路复用
  - `send_control()`：序列化 + 加密 + 发送
  - `ExecCmd`：内部枚举（Stdin / Signal）
  - `ExecMap`：全局 `HashMap<u32, UnboundedSender<ExecCmd>>` 跟踪运行中的 exec
- **`connection.rs`** — 连接状态。
  - `ConnState`：Listening / SynSent / Established / Closed
  - `Connection`：`id`、`peer`、`state`、`mux`（StreamMux）、`created`、`last_activity`
  - 构建方法：`build_syn()`、`build_syn_ack()`、`build_data()`、`build_encrypted_data()`、`build_heartbeat()`
- **`native_cmd/mod.rs`** — 分发器。400+ 行 `match` 语句，将每个 `NativeCmdType` 变体映射到对应的实现函数。
- **`native_cmd/fs.rs`** — 纯 Rust 实现，优先使用 `std::fs` 而非调用外部命令。包含：ls、cat、tail、head、stat、grep、find、du、df、tree、mkdir、rm、mv、cp、chmod、chown、diff、wc、write_file、read_file、touch。
- **`native_cmd/system.rs`** — 系统命令。ps（读取 `/proc`）、kill（`libc::kill`）、free（读取 `/proc/meminfo`）、cpu（读取 `/proc/cpuinfo`）、uptime、hostname、uname、whoami、who、last、dmesg、lsblk、mount、info、checksum。
- **`native_cmd/network.rs`** — 网络命令。netstat（读取 `/proc/net/tcp` 等）、portscan（TCP 连接扫描）、dns、arp。
- **`native_cmd/service.rs`** — 服务管理。systemctl / journalctl / apt / docker / crontab / iptables 的命令行封装。
- **`native_cmd/exec.rs`** — 批处理和 sed 实现。
- **`native_cmd/helper.rs`** — `read_proc()` 读取 `/proc` 文件、`run_cmd()` 执行外部命令并返回输出、`hfmt()` 人类可读格式化。

### 3. `crates/ctl/` — CLI 客户端

```
ctl/
├── Cargo.toml
└── src/
    ├── main.rs     # 1700+ 行 CLI 入口：所有子命令定义 + 连接逻辑 + 输出处理
    └── lib.rs      # 占位
```

**文件说明：**

- **`main.rs`** — 单文件，包含：
  - `Cli` 结构体：clap 参数（`--addr`、`--psk`、`--verbose`）
  - `Cmd` 枚举：50+ 子命令，分为 Exec、Iexec、Shell、Batch、Watch、Push、Pull、Ls、Cat、Tail、Head、Find、Grep、Du、Df、Tree、Stat、Mkdir、Rm、Mv、Cp、Chmod、Chown、Lsblk、Mount、Diff、Wc、Ps、Kill、Pgrep、Pkill、Top、Ping、Netstat、Ip、PortScan、Arp、Dns、Info、Uname、Uptime、Hostname、Whoami、Who、Last、Free、Cpu、Dmesg、Service、Journal、Pkg、Docker、Crontab、Firewall、Ssh、Checksum、Key、Mouse、Signal、Status、Version、Video、WriteFile、Sed、Touch
  - `Ctx` 结构体：UDP socket、conn_id、psk、peer 地址，提供 `connect()`、`send_control()`、`recv_control()` 方法
  - `native_run()`：发送 NativeCmd + 等待 ExecDone
  - `cmd_push()` / `cmd_pull()`：文件传输逻辑，含进度条
  - `drain_exec()`：循环接收 ExecChunk 并打印，直到 ExecDone
  - `handle_iexec()` / `handle_shell()` / `handle_batch()`：交互式/Shell/批处理

### 4. `crates/gui/` — GUI 客户端

```
gui/
├── Cargo.toml
├── build.rs          # 设置 /SUBSYSTEM:WINDOWS 链接标志
└── src/
    ├── main.rs       # eframe/egui 应用：UI 布局 + 事件处理
    └── client.rs     # 连接逻辑（SYN→Hello→Exec 流式执行）
```

**文件说明：**

- **`main.rs`** — eframe 应用入口。
  - `LanLinkApp`：应用状态（配置、连接、命令输入、输出行、历史、自动滚动）
  - `QUICK_COMMANDS`：10 个快捷命令按钮
  - 界面布局：顶部栏（服务器选择/连接状态）、左侧面板（快捷命令/历史/主机管理）、中央终端输出区、底部命令输入栏
  - Tab 自动补全、F5 保存配置
- **`client.rs`** — GUI 专用的连接 Client。
  - `HostConfig` / `AppConfig`：主机配置与持久化（JSON）
  - `Connection`：与 daemon 的 UDP 连接，提供 `connect()` → `send_control()` → `exec_streaming()`
  - `ExecEvent`：Started / Chunk / Done 回调模型
  - 与非交互的 exec 流程一致：SYN → SYN-ACK → Hello → Exec

### 5. `crates/shell/` — 命令执行引擎

```
shell/
├── Cargo.toml
└── src/
    └── lib.rs        # exec(), exec_with_input(), StreamingExec
```

**文件说明：**

- **`lib.rs`** — 三种执行模式：
  - `exec(cmd, args)`：同步执行，返回 `ExecResult { exit_code, stdout, stderr }`
  - `exec_with_input(cmd, args, stdin_data)`：同步执行 + 带 stdin 输入
  - `StreamingExec`：流式执行。`spawn(cmd)` 启动进程，内部三个线程（stdout reader、stderr reader、waiter）通过 `std::sync::mpsc` 通道输出
  - `StreamChunk`：`{ stream: u8, data: Vec<u8> }`
  - 跨平台：Unix 用 `sh -c`，Windows 用 `cmd /C`

### 6. `crates/input/` — 输入注入

```
input/
├── Cargo.toml
└── src/
    ├── lib.rs        # InputCapture / InputInjector trait + 数据类型
    ├── linux.rs      # Linux evdev + uinput 实现
    └── win.rs        # Windows SendInput 实现
```

**文件说明：**

- **`lib.rs`** — 类型定义：
  - `KeyEvent`：`down`、`scancode`、`vk`、`modifiers`
  - `MouseEvent`：Move / Button / Wheel
  - `MouseButton`：Left / Right / Middle / X1 / X2
  - `Modifiers`：bitflags（CTRL / ALT / SHIFT / WIN）
  - `MonitorInfo`：显示器信息
  - `InputCapture` trait：`poll_keys()`、`poll_mouse()`、`cursor_pos()`、`monitors()`
  - `InputInjector` trait：`inject_key()`、`inject_mouse()`、`set_cursor_pos()`
  - `BorderWatcher`：跨显示器边界检测
- **`linux.rs`** — Linux 实现：
  - `LinuxInputCapture`：通过 `evdev` ioctl 检测键盘/鼠标设备，解析 `input_event`（24 字节）
  - `find_input_devices()`：遍历 `/dev/input/event*`，使用 `EVIOCGBIT` ioctl 判断设备类型
  - `LinuxInputInjector`：创建 `/dev/uinput` 虚拟设备（名字 "lan-link-kvm"），写入 `input_event` 结构体
  - `enumerate_drm_monitors()`：读取 `/sys/class/drm/` 枚举显示器
- **`win.rs`** — Windows 实现（使用 `windows` crate）：
  - `WinInputCapture`：通过 `GetCursorPos` 轮询鼠标位置
  - `WinInputInjector`：通过 `SendInput` WinAPI 注入键盘/鼠标事件

### 7. `crates/video/` — 视频引擎（预留）

```
video/
├── Cargo.toml
└── src/
    ├── lib.rs              # VideoConfig / VideoFrame / VideoCapture / VideoEncoder
    ├── capture.rs          # DXGI 捕获桩（Windows）
    ├── linux_capture.rs    # DRM 捕获桩（Linux）
    └── encoder.rs          # NVENC/软件编码桩
```

---

## 关键数据结构

### `PacketHeader` — 协议头

```rust
// 38 字节，所有包共享
pub struct PacketHeader {
    pub conn_id: u64,         // 连接标识
    pub pkt_type: PacketType, // Syn/SynAck/Ack/Data/Rst/Heartbeat
    pub flags: Flags,         // RELIABLE | FRAGMENTED | ORDERED
    pub stream_id: u16,       // Control(0)/Video(1)/AudioTx(2)/AudioRx(3)/Input(4)/File(5)
    pub seq: u32,             // 序列号（用于可靠传输和 nonce 推导）
    pub ack_seq: u32,         // Piggyback ACK
    pub ack_bitmap: u32,      // 选择 ACK 位图
    pub payload_len: u16,     // 加密负载长度
    pub nonce: [u8; 12],      // ChaCha20-Poly1305 nonce
}
```

### `ControlMsg` — 控制消息

```rust
pub enum ControlMsg {
    // 流式 shell 执行
    Exec { id: u32, cmd: String },
    ExecOutput { id: u32, data: Vec<u8>, exit_code: Option<i32> },
    ExecStarted { id: u32 },
    ExecChunk { id: u32, stream: u8, data: Vec<u8> },
    ExecDone { id: u32, exit_code: Option<i32> },
    ExecStdin { id: u32, data: Vec<u8>, close: bool },
    ExecSignal { id: u32, signo: u32 },

    // 原生命令
    NativeCmd { id: u32, cmd: NativeCmdType },
    NativeSpawn { id: u32, cmd: NativeCmdType },

    // 连接协商
    Hello { version: u16, capabilities: Vec<String> },
    HelloAck { version: u16, capabilities: Vec<String> },

    // 文件传输
    FilePush { id: u32, path: String, size: u64 },
    FileChunk { id: u32, offset: u64, data: Vec<u8> },
    FileAck { id: u32, offset: u64 },

    // 输入事件
    KeyEvent { down: bool, scancode: u16, vk: u16 },
    MouseMove { dx: i16, dy: i16 },
    MouseButton { button: u8, down: bool },
    MouseWheel { delta: i16 },

    // 视频/音频流控制
    VideoStart { width: u16, height: u16, fps: u8, bitrate_kbps: u32 },
    VideoStop,
    AudioStart { sample_rate: u32, channels: u8 },
    AudioStop,
}
```

### `NativeCmdType` — 原生命令枚举

约 50 个变体，分为四大类：

- **Filesystem**（20+）：Ls, Cat, Tail, Head, Find, Grep, Du, Df, Tree, Stat, Mkdir, Rm, Mv, Cp, Chmod, Chown, Diff, Wc, Lsblk, Mount, WriteFile, Sed, Touch, ReadFile
- **System**（15+）：Ps, Kill, Pgrep, Pkill, Top, Uptime, Hostname, Uname, Whoami, Who, Last, Free, Cpu, Dmesg, Info
- **Network**（5）：Netstat, Ip, PortScan, Arp, Dns
- **Management**（10+）：Service, Journal, Pkg, Docker, Crontab, Firewall, Ssh, Checksum, ShellExec, BatchContent, Watch

### `Connection` — 连接状态

```rust
pub struct Connection {
    pub id: u64,
    pub peer: SocketAddr,
    pub state: ConnState,           // Listening / SynSent / Established / Closed
    pub mux: StreamMux,             // 流复用器（管理 6 个流）
    pub created: Instant,
    pub last_activity: Instant,
}
```

### `StreamingExec` — 流式执行引擎

```rust
pub struct StreamingExec {
    child: Arc<Mutex<Option<Child>>>,
    chunks_rx: Arc<Mutex<Option<mpsc::Receiver<StreamChunk>>>>,
    done_rx: Arc<Mutex<Option<mpsc::Receiver<Option<i32>>>>>,
    stdin_arc: Arc<Mutex<Option<ChildStdin>>>,
}
```

### `StreamMux` — 流多路复用

```rust
pub struct StreamMux {
    streams: HashMap<u16, MuxStream>,
}
// 预创建 6 个流：Control(0), Video(1), AudioTx(2), AudioRx(3), Input(4), File(5)
```

---

## 数据流图

### 从 ctl 发送命令到 daemon 执行的完整路径

```
lan-linkctl                      UDP                       lan-linkd
──────────                       ───                       ────────

1. 用户输入: `lan-linkctl ls /home`
        │
2. clap 解析为 Cmd::Ls { path: "/home", long: false, all: false }
        │
3. main() 创建 Ctx { socket, conn_id, psk, peer }
        │
4. Ctx::connect() ─── SYN ──────────────────────────► Connection::new()
        │                                              │ 创建连接状态
        │◄─────── SYN-ACK ────────────────────────────  conn.state = Listening
        │
5. conn.state = Established
   (对于新连接，ctl 会发送 Hello 协商)
        │
6. 映射为 NativeCmdType::Ls { path, ... }
   nc!(&mut ctx, cmd) 宏展开为 native_run(ctx, id, cmd, timeout)
        │
7. bincode::serialize → ControlMsg::NativeCmd { id, cmd }
   crypto::encrypt(psk, nonce, &payload)
   PacketHeader { type: Data, stream: Control }
        │
8. ─── Data 包 (加密) ─────────────────────────────► handle_packet_inner()
                                                       │
9.                                                    PacketHeader::decode()
                                                       crypto::decrypt()
                                                       │
10.                                                   stream_id == Control?
                                                       │
11.                                                   handle_control()
                                                       bincode::deserialize → ControlMsg::NativeCmd
                                                       │
12.                                                   native_cmd::run_native_cmd(&cmd)
                                                       │  → fs::cmd_ls(path, long, all)
                                                       │  → 读取目录 + 格式化输出
                                                       │  → 返回 (output_bytes, exit_code)
                                                       │
13. ◄──── ExecChunk { id, stream:0, data } ────────── send_control()
    ◄──── ExecDone { id, exit_code } ──────────────── send_control()
        │
14. drain_exec() 循环接收 ExecChunk
    打印到 stdout
        │
15. 完成，返回
```

### Shell 流式执行路径（`lan-linkctl exec "ping 8.8.8.8"`）

```
ctl                              daemon                         shell crate
───                              ──────                         ───────────

Exec { id:1, cmd:"ping 8.8.8.8" } ──────────►
                                    │
                                    StreamingExec::spawn("ping 8.8.8.8")
                                    │  → sh -c "ping 8.8.8.8"
                                    │  → 启动 3 个线程：
                                    │     ll-shell-stdout  (stdout reader)
                                    │     ll-shell-stderr  (stderr reader)
                                    │     ll-shell-wait    (waiter)
                                    │
ExecStarted { id:1 } ◄────────────
                                    │
ExecChunk { id:1, stream:0, data } ◄────────── std::sync::mpsc → tokio::mpsc
ExecChunk { id:1, stream:0, data } ◄────────── (tokio::select! 驱动)
    ...                              │
                                    │  (进程结束)
ExecDone { id:1, exit_code:0 } ◄────── waiter 线程发送 exit code
```

---

## 模块间依赖关系

```
lan-linkctl (ctl) ◄──── protocol ────► lan-linkd (daemon)
       │                                        │
       │                                        ├── native_cmd/ (fs/system/network/service/exec)
       │                                        │       └── helper (read_proc, run_cmd)
       │                                        │
       │                                        ├── shell crate (StreamingExec)
       │                                        │       └── protocol (仅类型)
       │                                        │
       │                                        ├── input crate (Linux uinput)
       │                                        │       └── protocol (仅类型)
       │                                        │
       │                                        └── video crate (预留)
       │
       └── gui crate ◄──── protocol
               │
               └── shell crate (通过 daemon 间接)

依赖层次（从底层到上层）：
protocol  ← 无依赖（只有第三方库）
shell     ← protocol
input     ← protocol
video     ← protocol
daemon    ← protocol + shell + input + video
ctl       ← protocol
gui       ← protocol + shell (间接)
```

依赖仅限 `protocol` crate 方向：所有其他 crate 都依赖 `protocol`，但 `protocol` 不依赖任何项目内 crate。这确保了协议层是最稳定、最可复用的组件。
