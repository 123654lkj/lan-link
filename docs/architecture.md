# 架构设计文档

## 概述

lan-link 是一个局域网远程管理工具，采用 **客户端-守护进程（Client-Daemon）** 架构。
所有通信基于 **UDP + 加密通道**，实现低延迟、高安全的远程管理。

## 模块关系

```
┌─────────────────────────────────────────────────────────────────┐
│                         Client (ctl/gui)                        │
│  CLI (clap) / GUI (egui) → 协议编码 → UDP Socket → 加密传输     │
└───────────────────────────────────┬─────────────────────────────┘
                                    │ UDP (加密)
                                    ▼
┌─────────────────────────────────────────────────────────────────┐
│                       Daemon (lan-linkd)                        │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────────┐   │
│  │ connection   │  │ native_cmd   │  │ discovery (mDNS)     │   │
│  │ 连接管理      │  │ 本地命令执行  │  │ 服务发现              │   │
│  └─────────────┘  └──────────────┘  └──────────────────────┘   │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────────┐   │
│  │ shell        │  │ video        │  │ input                │   │
│  │ 命令执行引擎  │  │ 视频流       │  │ 输入注入              │   │
│  └─────────────┘  └──────────────┘  └──────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

### Crate 层次

| Crate | 路径 | 职责 |
|-------|------|------|
| `lan-link-protocol` | `crates/protocol/` | 协议定义、帧编码、加密（ChaCha20-Poly1305）、可靠传输 |
| `lan-linkd` (daemon) | `crates/daemon/` | 服务端守护进程，接受客户端连接并执行命令 |
| `lan-linkctl` (ctl) | `crates/ctl/` | 命令行客户端 |
| `lan-link-gui` (gui) | `crates/gui/` | GUI 客户端（egui 跨平台） |
| `lan-link-shell` | `crates/shell/` | 命令执行引擎，支持流式输出 |
| `lan-link-video` | `crates/video/` | 视频流编码/解码 |
| `lan-link-input` | `crates/input/` | Linux 输入事件注入 |

## 数据流

### 连接建立

```
Client                    Daemon
  │                         │
  ├── SYN ────────────────► │
  │                         ├── 创建连接状态
  │◄── SYN-ACK ────────────┤
  │                         │
  ├── Hello (加密) ────────►│
  │                         ├── 验证 Hello
  │◄── HelloAck (加密) ────┤
  │                         │
  ▼                         ▼
  Established            Established
```

### 命令执行流程

```
Client                     Daemon
  │                          │
  ├── Exec { id, cmd } ────►│
  │                          ├── StreamingExec::spawn(cmd)
  │                          ├── 启动 reader 线程
  │◄── ExecStarted { id } ──┤
  │                          │
  │◄── ExecChunk { stdout } ─┤  ← 实时流式输出
  │◄── ExecChunk { stderr } ─┤
  │                          │
  │◄── ExecDone { code } ───┤  ← 进程结束
  │                          │
```

### NativeCmd 流程

```
Client                     Daemon
  │                          │
  ├── NativeCmd { id,cmd }─►│
  │                          ├── run_native_cmd(&cmd)
  │                          ├── 同步执行（Rust 实现或 Command）
  │◄── ExecChunk { output } ─┤
  │◄── ExecDone { code } ───┤
  │                          │
```

## 协议格式

### 数据包头部（38 字节）

```
Offset  Size  Field
0       8     conn_id (u64 LE)
8       1     pkt_type (SYN=0, SYN_ACK=1, DATA=3, HEARTBEAT=5, RST=6)
9       1     flags (bit 0: RELIABLE)
10      2     stream_id (u16 LE)
12      4     seq (u32 LE)
16      4     ack_seq (u32 LE)
20      4     ack_bitmap (u32 LE)
24      2     payload_len (u16 LE)
26      12    nonce (crypto nonce)
```

### 加密

- 算法：**ChaCha20-Poly1305**（通过 `chacha20poly1305` crate）
- Nonce 生成：`make_nonce(conn_id, seq)` — 12 字节，前 8 字节为 conn_id LE，后 4 字节为 seq LE
- PSK：32 字节预共享密钥

### 可靠传输

选择重传 ARQ（Selective Repeat ARQ）：
- 32 包滑动窗口
- 200ms 重传超时（RTO）
- 最大 10 次重试
- 带 piggyback ACK

## 序列图

```
┌─────────┐          ┌──────────┐          ┌─────────┐
│  Client │          │  Daemon  │          │  Shell  │
└────┬────┘          └────┬─────┘          └────┬────┘
     │  SYN               │                      │
     ├───────────────────►│                      │
     │  SYN-ACK           │                      │
     │◄───────────────────┤                      │
     │  Hello (encrypted) │                      │
     ├───────────────────►│                      │
     │  HelloAck          │                      │
     │◄───────────────────┤                      │
     │  NativeCmd / Exec  │                      │
     ├───────────────────►│                      │
     │                    ├── spawn() ──────────►│
     │                    │◄── stdout/stderr ────┤
     │  ExecChunk         │                      │
     │◄───────────────────┤                      │
     │  ExecDone          │                      │
     │◄───────────────────┤                      │
     │  Heartbeat         │                      │
     │◄───────────────────┤                      │
```
