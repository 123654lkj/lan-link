# lan-link 依赖清单

## Rust Workspace

`	oml
# Cargo.toml (workspace)
[workspace]
resolver = "2"
members = [
    "crates/protocol",
    "crates/shell",
    "crates/video",
    "crates/input",
    "crates/daemon",
    "crates/ctl",
    "crates/gui",
]
`

## 各 Crate 依赖

### protocol

| 依赖 | 版本 | 用途 |
|------|------|------|
| chacha20poly1305 | latest | ChaCha20-Poly1305 AEAD 加密 |
| rand | latest | PSK 生成 |
| serde | latest | ControlMsg 序列化 |
| bytes | latest | 帧编解码 |
| bitflags | latest | 标志位 |

### shell

| 依赖 | 版本 | 用途 |
|------|------|------|
| anyhow | latest | 错误处理 |

### input

| 依赖 | 版本 | 用途 |
|------|------|------|
| serde | latest | 事件序列化 |
| bitflags | latest | Modifiers 标志位 |
| evdev | latest (Linux) | 输入设备捕获 |
| uinput | latest (Linux) | 虚拟输入注入 |
| drm | latest (Linux) | 显示器枚举 |

### daemon

| 依赖 | 版本 | 用途 |
|------|------|------|
| lan-link-protocol | path | 协议层 |
| lan-link-input | path | 输入引擎 |
| lan-link-shell | path | Shell 引擎 |
| tokio | full | 异步运行时 |
| tracing | latest | 日志 |
| tracing-subscriber | latest | 日志格式化 |
| clap | derive | 命令行解析 |
| anyhow | latest | 错误处理 |
| hex | latest | PSK hex 解析 |
| bytes | latest | 帧处理 |
| rand | latest | conn_id 生成 |
| bincode | latest | ControlMsg 序列化 |
| serde | derive | 消息序列化 |

### ctl

| 依赖 | 版本 | 用途 |
|------|------|------|
| lan-link-protocol | path | 协议层 |
| tokio | full | 异步运行时 |
| tracing | latest | 日志 |
| tracing-subscriber | latest | 日志格式化 |
| clap | derive | 命令行解析 |
| anyhow | latest | 错误处理 |
| hex | latest | PSK hex 解析 |
| bincode | latest | 消息序列化 |
| rand | latest | conn_id 生成 |

### gui (预留)

| 依赖 | 版本 | 用途 |
|------|------|------|
| eframe | latest | GUI 框架 |
| lan-link-protocol | path | 协议层 |

## Python 客户端依赖

| 依赖 | 安装方式 | 用途 |
|------|----------|------|
| cryptography | pip install cryptography | ChaCha20-Poly1305 |
| pystray | pip install pystray (可选) | 系统托盘 |

## 打包工具

| 工具 | 安装方式 | 用途 |
|------|----------|------|
| PyInstaller | pip install pyinstaller | Python 打包 exe |
| Rust Toolchain | rustup | Rust 编译 |
| MSVC Build Tools | Visual Studio Installer | Windows Rust 编译 |

