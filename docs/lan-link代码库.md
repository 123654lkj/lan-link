# lan-link 代码库

## 项目概览

- **语言**: Rust (服务端 + 协议层) + Python (Windows 客户端 + GUI)
- **构建**: Cargo (Rust) + PyInstaller (Python 打包)
- **架构**: 客户端-服务端，UDP 协议，ChaCha20-Poly1305 加密

## 目录结构

`
lan-link/
+-- Cargo.toml              # Workspace 定义
+-- client_win.py            # Python 客户端 (620 行)
+-- client_gui.py            # tkinter GUI 客户端 (916 行)
+-- lan-link-gui.spec        # PyInstaller 打包配置
+-- lan-link-gui.vbs         # VBS 启动器 (无黑框)
+-- crates/
|   +-- protocol/            # 协议层 (帧格式 + 加密 + 可靠传输)
|   +-- shell/               # Shell 引擎 (命令执行)
|   +-- input/               # 输入引擎 (键鼠捕获 + 注入)
|   +-- daemon/              # 服务端 (lan-linkd)
|   +-- ctl/                 # Rust 命令行客户端
|   +-- video/               # 视频流 (预留)
|   +-- gui/                 # eframe Rust GUI (预留)
+-- docs/                    # 设计文档
+-- tests/                   # 端到端测试
+-- deploy/                  # 部署配置 (systemd unit)
+-- archive/                 # 历史版本归档
`

## Crate 详情

### protocol (协议层)

- rame.rs (179 行): 数据包帧格式 (38 字节 header)、ControlMsg 枚举
- crypto.rs (71 行): ChaCha20-Poly1305 AEAD 加密/解密
- 
eliable.rs (192 行): 可靠传输层 (StreamMux)
- stream.rs (1298 行): 流式复用
- lib.rs (69 行): 模块导出

### shell (Shell 引擎)

- lib.rs (216 行): 
  - exec(): 一次性命令执行
  - exec_with_input(): 带 stdin 的命令执行
  - StreamingExec: 流式执行 (spawn/poll_chunk/wait/kill/write_stdin)

### input (输入引擎)

- lib.rs (86 行): 数据结构定义 (KeyEvent, MouseEvent, Modifiers, MonitorInfo, BorderWatcher)
- linux.rs (381 行): Linux evdev 捕获 + uinput 注入 + DRM 显示器枚举
- win.rs (3834 行): Windows 输入 (预留)

### daemon (服务端)

- main.rs (306 行): UDP 接收循环、SYN 处理、streaming exec 调度、heartbeat
- connection.rs (73 行): 连接状态管理、SYN-ACK 构建、加密数据包构建
- discovery.rs (328 行): mDNS 服务发现
- lib.rs (210 行): 模块导出

### ctl (Rust CLI 客户端)

- main.rs (134 行): SYN 握手、Hello、Exec/Push/Status/Video 子命令

### video (视频流 - 预留)

- lib.rs: 模块定义
- capture.rs: 捕获接口
- encoder.rs: 编码接口
- linux_capture.rs: Linux 特定捕获

## 协议格式

### 数据包 Header (38 字节)

| 偏移 | 大小 | 字段 | 类型 |
|------|------|------|------|
| 0 | 8 | conn_id | u64 LE |
| 8 | 1 | pkt_type | u8 (SYN=0, SYN-ACK=1, ACK=2, DATA=3, RST=4, HB=5) |
| 9 | 1 | flags | u8 (RELIABLE=0x01) |
| 10 | 2 | stream_id | u16 LE (Control=0, Input=4) |
| 12 | 4 | seq | u32 LE |
| 16 | 4 | ack_seq | u32 LE |
| 20 | 4 | ack_bitmap | u32 LE |
| 24 | 2 | payload_len | u16 LE |
| 26 | 12 | nonce | [u8; 12] (conn_id + seq) |

### ControlMsg 枚举

- Hello, HelloAck: 连接协商
- Exec, ExecOutput: 传统一次性执行
- ExecStarted, ExecChunk, ExecDone: 流式执行
- ExecStdin, ExecSignal: 流式 stdin/信号
- FilePush, FileChunk, FileAck: 文件传输
- KeyEvent, MouseMove, MouseButton, MouseWheel: 输入注入
- VideoStart, VideoStop: 视频控制
- AudioStart, AudioStop: 音频控制

