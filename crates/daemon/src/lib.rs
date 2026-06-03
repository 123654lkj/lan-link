//! lan-linkd — 局域网远程管理守护进程库
//!
//! 提供运行在目标机器上的服务端核心逻辑，包括：
//!
//! - **连接管理** — UDP 连接状态机（SYN/SYN-ACK/Heartbeat/RST）
//! - **命令执行** — `native_cmd` 模块（纯 Rust 实现 50+ 本机命令）
//! - **系统管理** — systemd/docker/apt/crontab 等管理接口
//! - **输入注入** — Linux uinput 键盘/鼠标事件注入
//!
//! # 架构
//!
//! 主循环为单线程异步事件轮询（tokio），每 100ms 检查 UDP 套接字。
//! 所有阻塞操作（命令执行、文件 IO）通过 `spawn_blocking` 或独立线程处理。
//!
//! # 平台支持
//!
//! - **Linux**（当前目标平台）：完整功能，包括 uinput 输入注入
//! - **Windows/macOS**：未来计划支持，目前仅编译通过
