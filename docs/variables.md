# 变量命名和使用规范

## 命名规范

### Rust 代码

| 类型 | 规范 | 示例 |
|------|------|------|
| 类型/结构体 | `PascalCase` | `ReliableSender`, `PacketHeader` |
| 枚举变体 | `PascalCase` | `NativeCmdType::Ls`, `ConnState::Established` |
| 函数/方法 | `snake_case` | `run_native_cmd`, `send_control` |
| 变量 | `snake_case` | `conn_id`, `stream_id` |
| 常量 | `SCREAMING_SNAKE_CASE` | `WINDOW_SIZE`, `MAX_RETRIES` |
| 模块 | `snake_case` | `native_cmd`, `connection` |
| 泛型参数 | 单大写字母或描述性 Pascal | `T`, `E`, `Psk` |

### 缩写处理

| 缩写 | 展开 | 使用 |
|------|------|------|
| `conn` | connection | `conn_id`, `ConnState` |
| `psk` | Pre-Shared Key | `psk_hex`, `load_or_generate_psk` |
| `seq` | sequence | `seq`, `ack_seq` |
| `cmd` | command | `NativeCmd`, `ExecCmd` |
| `rx`/`tx` | receive/transmit | `chunks_rx`, `cmd_tx` |
| `mux` | multiplexer | `StreamMux` |

### 文件命名

- 每个文件一个主要类型/模块
- `mod.rs` 作为模块入口
- 测试文件：`模块名_test.rs` 或在模块内 `#[cfg(test)] mod tests`

## 变量使用规范

### conn_id

- 类型：`u64`
- 生成方式：`rand::random()` / `rand::thread_rng().next_u64()`
- 用途：唯一标识一个客户端-守护进程连接
- 传递：函数参数传递，结构体字段存储

### stream_id

- 类型：`u16`
- 取值：`StreamId` 枚举（Control=0, File=1, Input=2, Audio=3, Video=4）
- 用途：多路复用区分不同数据流

### seq (序列号)

- 类型：`u32`
- 初始值：0
- 递增方式：每个包 +1，使用 `wrapping_add` 处理回绕
- 用途：数据包排序、重传、ACK

### flags

- 类型：`Flags`（bitflags）
- 位 0：`RELIABLE` — 需要可靠传输
- 位 1-7：保留

## 错误处理规范

- 使用 `anyhow::Result` 作为主要错误类型
- 内部命令返回 `(Vec<u8>, Option<i32>)` — (输出内容, 退出码)
- `expect()` 仅用于 `bincode::serialize` 等确定不会失败的场景
- `unwrap()` 避免在外部输入路径使用

## 不安全代码

- **禁止** 使用 `unsafe` 除非有明确的安全论证
- 本项目目前零 `unsafe` 代码，保持此原则

## 线程安全

- 使用 `Arc<Mutex<T>>` 共享可变状态
- 使用 `Arc<AtomicU32/U64>` 共享原子计数器
- 使用 `tokio::sync::mpsc` 跨任务通信
- 避免 `std::mem::forget` / `ManuallyDrop`
