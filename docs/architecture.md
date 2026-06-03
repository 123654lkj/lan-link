# 架构设计

## 整体架构：Client-Daemon 模型

lan-link 采用 **Client-Daemon（客户端-守护进程）** 架构，所有通信基于 UDP 协议。

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Client 端                                    │
│  ┌────────────────────────────┐  ┌──────────────────────────────┐   │
│  │     lan-linkctl (CLI)      │  │   lan-link-gui (GUI)         │   │
│  │  ┌──────────────────────┐  │  │  ┌───────────────────────┐   │   │
│  │  │  clap 子命令解析器     │  │  │  │ eframe/egui UI       │   │   │
│  │  │  50+ subcommands      │  │  │  │ 快捷命令/终端/主机管理  │   │   │
│  │  └──────────┬───────────┘  │  │  └───────────┬───────────┘   │   │
│  │             │              │  │               │               │   │
│  │  ┌──────────▼───────────┐  │  │  ┌────────────▼──────────┐   │   │
│  │  │  Ctx (连接上下文)      │  │  │  │  Connection (客户端)   │   │   │
│  │  │  UDP socket / conn_id │  │  │  │  SYN→Hello→Exec       │   │   │
│  │  │  PSK / peer address   │  │  │  │  exec_streaming()     │   │   │
│  │  └──────────┬───────────┘  │  │  └────────────┬──────────┘   │   │
│  └─────────────┼──────────────┘  └───────────────┼──────────────┘   │
│                │                                  │                 │
│                └──────────┬───────────────────────┘                 │
│                           │                                         │
│              ┌────────────▼──────────────┐                          │
│              │  lan-link-protocol crate  │                          │
│              │  PacketHeader encode/decode│                         │
│              │  ChaCha20-Poly1305 encrypt │                          │
│              │  serde/bincode serialization│                         │
│              └────────────┬──────────────┘                          │
└───────────────────────────│─────────────────────────────────────────┘
                            │ UDP (端口 9876, 加密)
                            ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      Daemon 端 (lan-linkd)                          │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  main(): UDP 事件循环                                        │   │
│  │  - recv_from() → handle_packet_inner()                      │   │
│  │  - 每 100ms 轮询，每 5 秒心跳，30 秒超时清理                   │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                               │                                     │
│               ┌───────────────┼───────────────┐                     │
│               ▼               ▼               ▼                     │
│  ┌──────────────────┐ ┌──────────────┐ ┌──────────────┐            │
│  │  connection.rs   │ │ native_cmd/  │ │ discovery.rs  │            │
│  │  连接状态机       │ │ 本机命令执行   │ │ mDNS 发现     │            │
│  │  SYN→SYN-ACK     │ │ fs/system/   │ │ (TODO)       │            │
│  │  Heartbeat→Close │ │ network/     │ │              │            │
│  └──────────────────┘ │ service/     │ └──────────────┘            │
│                       └──────────────┘                              │
│               ┌───────────────┬───────────────┐                     │
│               ▼               ▼               ▼                     │
│  ┌──────────────────┐ ┌──────────────┐ ┌──────────────┐            │
│  │  shell crate     │ │ input crate  │ │ video crate  │            │
│  │  StreamingExec   │ │ uinput KVM   │ │ 视频捕获/编码  │            │
│  │  sh -c / cmd /C  │ │ SendInput    │ │ (预留)       │            │
│  └──────────────────┘ └──────────────┘ └──────────────┘            │
└─────────────────────────────────────────────────────────────────────┘
```

### 设计原则

1. **最小信任**：daemon 运行在目标机器上，客户端通过网络连接。所有通信必须经过加密认证。
2. **无状态协议**：每个连接由客户端生成的 64 位随机 `conn_id` 标识，daemon 维护连接状态的哈希表。
3. **协议先行**：所有 crate 共享 `lan-link-protocol` 中定义的帧格式、加密机制和消息类型。
4. **NativeCmd 优先**：优先使用 Rust 实现的本地命令（如通过 `std::fs` 读取文件），而非通过 shell 调用外部命令。这减少了安全风险和对目标机器工具的依赖。

---

## 协议层详解

### 帧格式

每个 UDP 包包含一个 38 字节的头部，后跟加密负载。

```
字节偏移  大小    字段          说明
───────  ────   ───────────  ───────────────────────────
  0       8     conn_id      连接标识（u64, LE）
  8       1     pkt_type     包类型（u8）
                              0=SYN  1=SYN-ACK  2=ACK
                              3=DATA 4=RST       5=HEARTBEAT
  9       1     flags        标志位（u8, bitflags）
                              bit0=RELIABLE  bit1=FRAGMENTED
                              bit2=ORDERED
 10       2     stream_id    流标识（u16, LE）
                              0=Control  1=Video  2=AudioTx
                              3=AudioRx  4=Input  5=File
 12       4     seq          序列号（u32, LE）
 16       4     ack_seq      Piggyback ACK 序列号（u32, LE）
 20       4     ack_bitmap   选择 ACK 位图（32 包窗口, u32, LE）
 24       2     payload_len  加密负载长度（u16, LE）
 26      12     nonce        ChaCha20-Poly1305 Nonce（[u8; 12]）
───────  ────   ───────────  ───────────────────────────
 38      n      encrypted    加密负载（含 16 字节 Poly1305 认证标签）

总开销 = 38（头部）+ 16（认证标签）= 54 字节
最大负载 = 1400 字节（适配 MTU=1500）
```

### 加密层

#### 算法选择

| 项目 | 选择 |
|------|------|
| **加密算法** | ChaCha20-Poly1305（AEAD） |
| **密钥长度** | 256 位（32 字节） |
| **Nonce** | 96 位（12 字节） |
| **认证标签** | Poly1305（16 字节） |
| **底层实现** | `chacha20poly1305` crate（RustCrypto） |

#### Nonce 生成策略

```rust
fn make_nonce(conn_id: u64, seq: u32) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce[0..8].copy_from_slice(&conn_id.to_le_bytes());  // 连接标识
    nonce[8..12].copy_from_slice(&seq.to_le_bytes());     // 序列号
    nonce
}
```

- **唯一性保证**：`(conn_id, seq)` 对在通信中是全局唯一的
  - `conn_id` 是 64 位随机数，碰撞概率极低
  - `seq` 在单个流内单调递增，不会重复
- **无需额外随机数**：减少包大小和熵消耗
- **无需再加密**：非对称 over-the-wire 传输 nonce，直接由收发双方独立计算

#### 加密流程

```
控制消息（ControlMsg）
    │
    ▼
bincode::serialize()       ← 序列化为二进制
    │
    ▼
crypto::encrypt(psk, nonce, plaintext)
    │
    ├── ChaCha20 流加密
    ├── Poly1305 MAC 认证标签（16 字节追加在末尾）
    │
    ▼
PacketHeader::encode()     ← 构造 38 字节头部
    │
    ▼
UDP 发送
```

#### 解密流程

```
UDP 接收
    │
    ▼
PacketHeader::decode()     ← 解析 38 字节头部
    │
    ▼
crypto::decrypt(psk, nonce, ciphertext)
    │
    ├── Poly1305 验证 MAC（篡改检测）
    ├── ChaCha20 流解密
    │
    ▼
bincode::deserialize()     ← 反序列化为 ControlMsg
```

### ARQ 可靠传输层

用于 Control、Input、File 流。Video、Audio 流不使用可靠传输。

```
协议：选择性重传 ARQ (Selective Repeat ARQ)
窗口：32 包
RTO：200ms（重传超时）
最大重试：10 次
ACK：Piggyback ACK + 选择 ACK 位图
```

#### 发送端状态机

```
                    ┌──────────┐
                    │  空闲     │
                    └────┬─────┘
                         │ send(payload)
                         ▼
               ┌──────────────────┐
          ┌───│  窗口未满？        │──── 否 ───→ 返回 None（重试）
          │   └────────┬─────────┘
          │            │ 是
          │            ▼
          │   ┌──────────────────┐
          │   │  分配 seq         │
          │   │  存入 SendSlot    │
          │   │  发送数据包        │
          │   └────────┬─────────┘
          │            │
          │       ┌────▼─────┐
          │   ┌───│  等待 ACK  │
          │   │   └────┬─────┘
          │   │   ┌────┴──────┐
          │   │   │ 超时？     │── 是 → 重传（最多 10 次）
          │   │   └────┬──────┘
          │   │   ┌────┴──────┐
          │   │   │ 收到 ACK？ │── 是 → 标记 acked，滑动窗口
          │   │   └───────────┘
          │   │
          │   │  on_ack(ack_seq, ack_bitmap)
          │   │  → 跳过已 acked 的 slot
          │   │  → 滑动窗口 base
          │   │
          └───┤
              │  poll_retransmit()（每轮事件循环调用）
              │  → 检查超时 slot
              │  → 重传
              └──
```

#### 接收端状态机

```
接收数据包
    │
    ▼
deliver(seq, payload)
    │
    ├── seq == next_expected？── 是 → 递送 + 检查乱序缓冲区
    │                               → 递送连续包
    │
    ├── seq 在窗口内（dist <= 32）？── 是 → 存入乱序缓冲区
    │
    └── seq 在窗口外？── 丢弃
    │
    ▼
返回 ack_info() → (ack_seq, ack_bitmap)
```

### 流多路复用

单条 UDP 连接（由一个 `conn_id` 标识）可以承载 6 个逻辑流：

| StreamId | 名称 | 可靠 | 用途 |
|----------|------|------|------|
| 0 | Control | ✅ | 控制消息（命令、结果、协商） |
| 1 | Video | ❌ | 视频帧数据 |
| 2 | AudioTx | ❌ | 音频发送 |
| 3 | AudioRx | ❌ | 音频接收 |
| 4 | Input | ✅ | 输入事件（键盘、鼠标） |
| 5 | File | ✅ | 文件传输 |

每个流维护独立的 `send_seq` 计数器，互不干扰。可靠流使用 `ReliableSender`/`ReliableReceiver`，不可靠流直接发送。

---

## 连接生命周期

### 状态机

```
                  ┌────────────┐
                  │  Listening  │  ← Connection::new()
                  └──────┬─────┘
                         │ 收到 SYN
                         ▼
                  ┌────────────┐
                  │ Established │  ← 发送 SYN-ACK
                  └──────┬─────┘
                         │
              ┌──────────┼──────────┐
              │          │          │
              ▼          ▼          ▼
          ┌────────┐ ┌────────┐ ┌────────┐
          │  Data   │ │Heartbeat│ │  RST   │
          │ 收发    │ │ 刷新    │ │  断开  │
          └────────┘ │ 时间戳  │ └────────┘
                     └────────┘
              │          │          │
              └──────────┼──────────┘
                         │ 超时 30 秒
                         ▼
                  ┌────────────┐
                  │   Closed   │
                  └────────────┘
```

### 握手流程

```
Client                           Daemon
  │                                │
  │  1. SYN                        │
  │  (conn_id: 随机 u64)           │
  │  pkt_type=Syn, stream_id=0     │
  ├──────────────────────────────► │
  │                                │ 创建 Connection { state: Listening }
  │                                │
  │  2. SYN-ACK                    │
  │  (同一 conn_id)                │
  │  pkt_type=SynAck               │
  │◄──────────────────────────────┤
  │                                │ conn.state = Established
  │                                │
  │  3. Hello (加密)               │
  │  ControlMsg::Hello {           │
  │    version: 1,                 │
  │    capabilities: ["exec"]      │
  │  }                             │
  ├──────────────────────────────► │
  │                                │ 验证 Hello
  │  4. HelloAck (加密)            │
  │  ControlMsg::HelloAck {        │
  │    version: 1,                 │
  │    capabilities: ["exec","input"]│
  │  }                             │
  │◄──────────────────────────────┤
  │                                │
  │  5. NativeCmd / Exec           │
  ├──────────────────────────────► │
  │      ...                       │
  │  6. ExecChunk / ExecDone       │
  │◄──────────────────────────────┤
  │                                │
  │  7. Heartbeat (每 5 秒)        │
  │◄──────────────────────────────┤
  │                                │ 刷新 last_activity
  │                                │
  │  8. RST                        │
  │  (或 30 秒无活动超时)           │
  ├──────────────────────────────► │ 移除连接
  │  (或)                          │
  │        超时                    │
  │◄─────── 移除连接 ─────────────┤
```

### 心跳与超时

- **心跳间隔**：daemon 每 5 秒向所有 Established 连接发送 Heartbeat 包
- **超时阈值**：30 秒无活动（未收到任何包），连接被自动清理
- **客户端**：收到 Heartbeat 后刷新 `last_activity`，无需回复
- **重连**：客户端发现超时后应重新发起 SYN 握手

---

## NativeCmdType 设计理念

### 设计动机

传统的远程管理工具（如 SSH）通过在远端启动 shell 进程来执行命令。这种方式存在以下问题：

1. **启动开销**：每条命令都需要 fork+exec 一个新进程
2. **依赖远端工具**：目标机器必须安装 ls、cat、ps 等工具
3. **解析成本**：CLI 输出需要解析才能结构化使用
4. **安全问题**：shell 注入风险，特殊字符可能导致意外执行

### NativeCmdType 的解决方案

`NativeCmdType` 是一个包含约 50 个变体的 Rust 枚举，每个变体描述一个结构化命令。

```rust
// 传统 SSH：ssh user@host "ls -la /home"  → 远端启动 /bin/ls
// NativeCmd：NativeCmdType::Ls { path: "/home", long: true, all: true }
```

**优势：**

1. **远端正向执行**：daemon 内部通过 `std::fs` 等 Rust API 直接执行，无需外部命令
2. **无 shell 注入风险**：参数是结构化的，不会出现 `$(rm -rf /)` 这样的注入
3. **无解析成本**：结果直接以 `ExecChunk + ExecDone` 流式返回
4. **跨平台一致**：同一命令在 Linux 和 Windows 上行为一致（即使底层实现不同）
5. **类型安全**：编译器确保所有参数类型正确

### 使用模式

**ctl 端**：每个 CLI 子命令映射到对应的 `NativeCmdType` 变体

```
用户输入: lan-linkctl ls -la /home
    │
    ▼
clap 解析 → Cmd::Ls { path: "/home", long: true, all: true }
    │
    ▼
映射 → NativeCmdType::Ls { path: "/home", long: true, all: true }
    │
    ▼
ControlMsg::NativeCmd { id: N, cmd: ... }
    │
    ▼
加密 → 发送 → daemon 解密 → run_native_cmd()
    │
    ▼
fs::cmd_ls("/home", true, true)  ← 纯 Rust 实现
    │
    ▼
返回 (output_bytes, exit_code)
```

**daemon 端**：`run_native_cmd()` 通过 match 分发到具体实现

```rust
pub fn run_native_cmd(cmd: &NativeCmdType) -> (Vec<u8>, Option<i32>) {
    match cmd {
        NativeCmdType::Ls { path, long, all } => fs::cmd_ls(path, *long, *all),
        NativeCmdType::Cat { path } => fs::cmd_cat(path),
        NativeCmdType::Ps { full, user, tree } => system::cmd_ps(*full, user.clone(), *tree),
        NativeCmdType::Service { action } => service::cmd_service(action),
        // ... 50+ 变体
    }
}
```

### 变体分类

| 类别 | 实现方式 | 示例 |
|------|---------|------|
| Filesystem | `std::fs` Rust API | `ls`, `cat`, `cp`, `rm`, `chmod` |
| System | `std::fs` + `/proc` 读取 | `ps`, `free`, `cpu`, `uptime` |
| Network | `std::net` + TCP 连接 | `portscan`, `dns`, `netstat` |
| Management | `std::process::Command` | `service`, `pkg`, `docker`, `journal` |
| ShellExec | `sh -c`（回退方案） | `ShellExec`（用于非结构化命令） |

### 何时使用 NativeCmd vs Exec

| 场景 | 推荐方式 | 理由 |
|------|---------|------|
| 文件操作 | `NativeCmd` | 纯 Rust 实现，无 shell 开销 |
| 进程管理 | `NativeCmd` | 读取 `/proc` 更高效 |
| 系统信息 | `NativeCmd` | 结构化和格式化更方便 |
| 安装软件 | `NativeCmd::Pkg` | 通过 `apt` 命令但结构化参数 |
| 任意 shell 命令 | `Exec` | 需要 shell 管道、重定向等 |
| 交互式程序 | `Exec` (iexec/shell) | 需要 stdin/stdout 双向流 |
| 文件传输 | `FilePush`/`FilePull` | 专用协议，分块+ACK |

---

## 输入注入机制

### Linux (evdev + uinput)

```
客户端                              daemon
  │                                  │
  ├── ControlMsg::KeyEvent {         │
  │     down: true,                  │
  │     scancode: 30, // KEY_A      │
  │     vk: 65                       │
  │   }                              │
  │                                  │
  ├── (加密) ──── Data 包 ────────►  │
  │                                  │
  │                                  ├── bincode::deserialize
  │                                  │    → lan_link_input::KeyEvent
  │                                  │
  │                                  ├── injector().lock()
  │                                  │    → LinuxInputInjector
  │                                  │
  │                                  ├── fd 写入 input_event 结构
  │                                  │    struct input_event {
  │                                  │        timeval tv;  // 16 bytes
  │                                  │        u16 type;    // EV_KEY=1
  │                                  │        u16 code;    // scancode
  │                                  │        i32 value;   // 0/1
  │                                  │    } // total 24 bytes
  │                                  │
  │                                  ├── write(fd, &event, 24)
  │                                  ├── write(fd, &syn, 24) // EV_SYN
  │                                  │
  │                                  ▼
  │                           /dev/uinput 虚拟设备
  │                           → Linux 输入子系统
  │                           → 应用程序接收按键事件
```

#### uinput 设备创建

1. 打开 `/dev/uinput`（需 root 或 `uinput` 组成员）
2. ioctl 设置设备能力：`UI_SET_EVBIT(EV_KEY)`、`UI_SET_EVBIT(EV_REL)`、`UI_SET_KEYBIT(...)`、`UI_SET_RELBIT(...)`
3. 写入 `UinputUserDev` 结构体（设备名：`lan-link-kvm`）
4. ioctl `UI_DEV_CREATE` 创建设备
5. 之后向 fd 写入 `input_event` 结构体即可注入输入

#### evdev 捕获

1. 遍历 `/dev/input/event*` 设备文件
2. 使用 `EVIOCGBIT` ioctl 查询设备能力
3. 识别键盘设备（支持 EV_KEY 且包含 KEY_A 等按键码）
4. 识别鼠标设备（支持 EV_REL 且包含 REL_X/REL_Y）
5. 以非阻塞方式读取 24 字节的 `input_event` 结构

### Windows (SendInput)

```
客户端                              daemon (Windows)
  │                                  │
  ├── ControlMsg::KeyEvent {         │
  │     down: true,                  │
  │     scancode: 0x1E,             │
  │     vk: 0x41                     │
  │   }                              │
  │                                  │
  ├── (加密) ──── Data 包 ────────►  │
  │                                  │
  │                                  ├── WinInputInjector
  │                                  │
  │                                  ├── SendInput(INPUT)
  │                                  │    struct INPUT {
  │                                  │        type: INPUT_KEYBOARD,
  │                                  │        ki: {
  │                                  │            wVk: VK_A,
  │                                  │            wScan: 0x1E,
  │                                  │            dwFlags: 0 / KEYEVENTF_KEYUP
  │                                  │        }
  │                                  │    }
  │                                  │
  │                                  ▼
  │                           Windows 输入系统
  │                           → 应用程序接收按键事件
```

---

## 安全模型

详见 [security.md](security.md) 文档。

### 核心要点

1. **端到端加密**：所有控制数据使用 ChaCha20-Poly1305 AEAD 加密
2. **PSK 预共享密钥**：32 字节随机密钥，所有通信的认证基础
3. **每个包唯一 nonce**：`(conn_id, seq)` 确保 nonce 不重复
4. **Poly1305 认证标签**：检测数据篡改
5. **无 shell 注入**：NativeCmd 使用结构化参数
6. **连接超时**：30 秒无活动自动断开
7. **心跳保活**：5 秒心跳维持连接
