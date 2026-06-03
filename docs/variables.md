# 变量命名和使用规范

## Rust 命名规范

项目严格遵循 Rust 标准命名规范：

| 类别 | 规范 | 示例 |
|------|------|------|
| **类型** (struct/enum/trait) | `PascalCase` | `PacketHeader`, `ControlMsg`, `StreamingExec` |
| **枚举变体** | `PascalCase` | `ConnState::Established`, `PacketType::SynAck` |
| **函数/方法** | `snake_case` | `generate_psk()`, `run_native_cmd()`, `handle_packet_inner()` |
| **变量/参数** | `snake_case` | `conn_id`, `pkt_type`, `payload_len` |
| **常量** | `SCREAMING_SNAKE_CASE` | `HEADER_SIZE`, `MAX_PAYLOAD`, `WINDOW_SIZE`, `RTO`, `MAX_RETRIES` |
| **模块** | `snake_case` | `native_cmd`, `lan_link_protocol` |
| **类型别名** | `PascalCase` | `Psk`, `ExecMap` |
| **生命周期** | 小写单字符 | `'a` |

### 缩写词处理

- **全大写**：当缩写作为类型名的一部分时，保持大写
  - `PSK_PATH`（常量），但 `Psk`（类型别名）
  - `RTO`（常量：Retransmission Timeout）
  - `ARQ`（注释中提及）
- **全小写**：当缩写作为标识符的一部分时，保持小写
  - `psk`（变量/参数）
  - `conn_id`（connection id）
  - `pkt_type`（packet type）
- **混合**：方法名中缩写转小写
  - `build_syn_ack()`（而非 `build_SYN_ACK()`）
  - `build_encrypted_data()`（而非 `build_encrypted_DATA()`）

### 命名原则

1. **自文档化**：名称应清晰表达用途，避免缩写
   - 好：`last_activity`, `next_expected`, `window_base`
   - 避免：`la`, `ne`, `wb`
2. **一致性**：同一概念在不同 crate 中使用相同名称
   - 统一使用 `conn_id` 而非 `connection_id` 或 `cid`
   - 统一使用 `stream_id` 而非 `sid` 或 `stream`
3. **布尔变量**：使用肯定形式，以 `is_`/`has_` 前缀
   - `is_reliable`, `is_primary`, `has_keys`
   - 避免：`not_reliable`

---

## 项目专用术语表

| 术语 | 含义 | 常用字段/变量 |
|------|------|---------------|
| `conn_id` | 连接标识符（64 位随机数） | `conn_id: u64` |
| `stream_id` | 流标识符（16 位） | `stream_id: u16` |
| `seq` | 序列号（32 位） | `seq: u32`, `send_seq`, `next_seq`, `ack_seq` |
| `pkt_type` | 包类型枚举 | `pkt_type: PacketType` |
| `payload_len` | 加密负载长度 | `payload_len: u16` |
| `nonce` | ChaCha20-Poly1305 nonce（12 字节） | `nonce: [u8; 12]` |
| `psk` | 预共享密钥（32 字节） | `psk: Psk`, `Psk = [u8; 32]` |
| `conn` | Connection 实例 | `let conn = Connection::new(...)` |
| `mux` | Stream multiplexer | `mux: StreamMux` |
| `peer` | 远端 Socket 地址 | `peer: SocketAddr` |
| `flags` | 包标志位 | `flags: Flags` |
| `ack_bitmap` | 选择 ACK 位图（32 位） | `ack_bitmap: u32` |
| `slot` | 可靠传输发送槽位 | `slots: VecDeque<SendSlot>` |
| `ooo_buffer` | 乱序包缓冲区 | `ooo_buffer: VecDeque<(u32, Vec<u8>)>` |
| `RTO` | 重传超时（200ms） | `const RTO: Duration` |
| `WINDOW_SIZE` | 滑动窗口大小（32） | `const WINDOW_SIZE: u32` |
| `MAX_RETRIES` | 最大重传次数（10） | `const MAX_RETRIES: u32` |
| `scancode` | 键盘扫描码 | `scancode: u16` |
| `vk` | 虚拟键码（Windows） | `vk: u16` |
| `dx`/`dy` | 鼠标相对位移 | `dx: i16`, `dy: i16` |
| `id` | 命令/文件传输的相关 ID | `id: u32` |
| `exit_code` | 进程退出码 | `exit_code: Option<i32>` |
| `stream` | 输出流类型：0=stdout, 1=stderr | `stream: u8` |
| `chunk` | 流式输出片段 | `chunks_rx`, `StreamChunk` |

---

## 错误处理规范

### 错误类型选择

| 场景 | 错误类型 | 说明 |
|------|---------|------|
| 库函数（shell/input/video） | `anyhow::Result<T>` | 使用 `anyhow` crate 简化错误处理 |
| 协议编解码（protocol） | `Option<T>` 或 `Result<T, &str>` | 包解析失败返回 `None`，调用方自行处理 |
| 二进制入口（daemon/ctl/gui） | `anyhow::Result<()>` | main 函数返回 `anyhow::Result` |

### 错误处理模式

```rust
// 1. 包解析：返回 Option，调用方 log 并跳过
fn decode(buf: &mut impl Buf) -> Option<Self> {
    if buf.remaining() < HEADER_SIZE { return None; }
    // ...
}

// 2. 库函数：使用 anyhow
fn spawn(cmd: &str) -> anyhow::Result<Self> {
    // ...
}

// 3. 加密解密：返回 Option（认证失败视为静默丢弃）
pub fn decrypt(key: &Psk, nonce: &[u8; 12], ciphertext: &[u8]) -> Option<Vec<u8>> {
    cipher.decrypt(nonce, ciphertext).ok()
}
```

### 错误传播原则

- **不要在库层** `unwrap()` / `expect()`：应传播错误到调用方
- **允许在二进制入口处** `expect()`：如配置解析、socket 绑定
- **网络层应宽容**：解析失败、解密失败、反序列化失败都应 `warn!` 并继续，而非崩溃
- **PSK 验证使用** `assert_eq!`：仅初始化时（hex 解码验证），因为这是内部而非外部输入

### 退出码约定（daemon）

- 0：正常退出
- 非 0：配置错误或无法绑定端口等致命错误

---

## 注释规范

### 注释类型

```rust
//! 模块级文档：放在文件开头，描述模块的职责和用法
//! 帧格式、加密、可靠传输、流式处理的公共类型和工具。

/// 结构体/枚举/函数文档：使用三斜杠，描述 API
pub struct PacketHeader {
    pub conn_id: u64,   /// 连接标识符
    pub pkt_type: PacketType, /// 包类型
    // ...
}

// 行内注释：解释代码逻辑，仅对不明显的实现添加
// Window is full, caller should retry
```

### 文档注释原则

1. **公共 API 必须有 `///` 文档**：所有 `pub fn`、`pub struct`、`pub enum`、`pub trait`
2. **模块文件必须有 `//!` 文档**：每个 crate 的 `lib.rs` 和主要模块文件
3. **注释解释"为什么"而不是"是什么"**：
   - 好：`// 需要 O_NONBLOCK 以避免 uinput 写入阻塞`
   - 避免：`// 打开文件`
4. **自解释的代码不需要注释**：
   - 好命名 + 简单逻辑 = 无需注释
5. **TODO 要带 Issue 编号或责任人**：
   - `// TODO(#123): 添加 mDNS 服务发现`
   - `// TODO: 支持更多编码格式`

### 示例注释模板

```rust
/// [函数简要描述]
///
/// [详细描述，包括行为说明]
///
/// # Arguments
///
/// * `key` - 32 字节预共享密钥
/// * `nonce` - 12 字节 nonce，必须唯一
/// * `plaintext` - 要加密的明文数据
///
/// # Returns
///
/// 包含 16 字节 Poly1305 认证标签的密文
///
/// # Panics
///
/// 当 key 长度不是 32 字节时 panic
pub fn encrypt(key: &Psk, nonce: &[u8; 12], plaintext: &[u8]) -> Vec<u8> {
    // ...
}
```

---

## 日志规范

项目使用 `tracing` crate 进行日志记录，统一通过 `tracing_subscriber::fmt::init()` 初始化。

### 日志级别选择

| 级别 | 使用场景 | 示例 |
|------|---------|------|
| `error!` | 不可恢复的运行时错误 | 配置加载失败、关键路径异常 |
| `warn!` | 可恢复的异常情况 | 包解析失败、解密失败、未知控制消息 |
| `info!` | 重要生命周期事件 | 连接建立/关闭、命令开始/结束、PSK 生成 |
| `debug!` | 调试信息 | 收发字节数、连接状态变化、输入事件详情 |
| `trace!` | 高频跟踪细节 | 包的每个处理步骤（预留，当前较少使用） |

### 日志使用原则

1. **生产环境默认只输出 `info` 及以上**：`--verbose` 参数开启 `debug`
2. **每 100ms 轮询周期中的失败用 `warn!` 而非 `error!`**：网络波动是常态
3. **心跳和循环事件用 `debug!`**：避免刷屏
4. **每条日志应包含上下文信息**：如连接 ID、远端地址、命令 ID

```rust
// 好
info!("Exec #{}: {}", id, cmd);
warn!("Bad header from {}", peer);
debug!("recv {} bytes from {}", n, peer);

// 避免
info!("received data");  // 缺少上下文
warn!("error occurred"); // 缺少具体信息
```

### tracing macros 速查

```rust
use tracing::{info, warn, debug, error, trace};

// 结构化日志（支持字段）
info!(conn_id, "connection established");
warn!(%peer, "decrypt failed");  // Display 格式化

// Span（预留，当前未使用）
let span = info_span!("handle_packet", conn_id);
let _guard = span.enter();
```

---

## 公共 API 文档规范

所有公共 API 应包含以下文档元素：

1. **一句话摘要**（必需）— 描述函数/类型做什么
2. **详细说明**（可选）— 复杂行为或边界情况
3. **参数说明**（可选）— 使用 `# Arguments` 标题
4. **返回值说明**（可选）— 使用 `# Returns` 标题
5. **Panics 说明**（必需，若可 panic）— 使用 `# Panics` 标题
6. **示例**（强烈推荐）— 使用 `# Examples` 标题，包含 `assert!`

### module 文档模板（`//!`）

```rust
//! [模块名] — [一句话职责]
//!
//! [详细描述，包括模块内主要类型和用途]
//!
//! # 主要类型
//!
//! * [`PacketHeader`] — 38 字节定长协议头
//! * [`ControlMsg`] — 控制消息枚举
//!
//! # 数据流
//!
//! 序列化 → 加密 → 发送 / 接收 → 解密 → 反序列化
```

### 类型文档模板

```rust
/// [类型名] — [一句话描述]
///
/// [扩展说明：何时使用、与相关类型的关系]
#[derive(Debug, Clone)]
pub struct PacketHeader {
    /// 连接标识符，由客户端在 SYN 时生成
    pub conn_id: u64,
    // ...
}
```

### 函数文档模板

```rust
/// 加密明文数据并返回密文（含认证标签）
///
/// 使用 ChaCha20-Poly1305 AEAD 算法加密。
/// nonce 必须每个包唯一——使用 `make_nonce(conn_id, seq)` 生成。
///
/// # Arguments
///
/// * `key` - 32 字节预共享密钥
/// * `nonce` - 12 字节唯一 nonce
/// * `plaintext` - 明文数据
///
/// # Returns
///
/// 加密后的数据（末尾附加 16 字节 Poly1305 认证标签）
pub fn encrypt(key: &Psk, nonce: &[u8; 12], plaintext: &[u8]) -> Vec<u8> {
    // ...
}
```
