# lan-link — 局域网远程管理工具

lan-link 是一个通过 **UDP 加密通道** 远程管理局域网设备的工具，提供 CLI、GUI 双客户端，支持文件操作、系统管理、远程 shell、服务管理、Docker 操作等 50+ 子命令。

## 功能特性

- **🔒 端到端加密** — ChaCha20-Poly1305 AEAD 加密，PSK 预共享密钥认证
- **⚡ 低延迟 UDP 协议** — 自定义帧格式，支持可靠传输（选择性 ARQ）和流复用
- **🖥️ 双客户端** — CLI 客户端（`lan-linkctl`）和跨平台 GUI 客户端（`lan-link-gui`）
- **📂 远程文件操作** — `ls` / `cat` / `tail` / `head` / `find` / `grep` / `cp` / `mv` / `rm` / `chmod` / `chown` / `diff` / `wc` / `du` / `df` / `tree` / `stat` / `touch` / `sed` / `writefile`
- **🖥️ 远程系统管理** — `ps` / `top` / `kill` / `free` / `uptime` / `hostname` / `uname` / `whoami` / `who` / `last` / `dmesg` / `cpu` / `info` / `lsblk` / `mount`
- **🌐 远程网络工具** — `netstat` / `ip` / `portscan` / `arp` / `dns`
- **⚙️ 服务管理** — systemd 服务启停、journal 日志查询
- **📦 包管理** — apt 安装/更新/升级
- **🐳 Docker 管理** — 容器列表、日志、统计、执行
- **📁 文件传输** — `push` / `pull` 支持进度显示
- **🔧 远程 Shell** — 交互式 shell 会话（`shell`）、流式执行（`exec`/`iexec`）、批量命令（`batch`）、定时观察（`watch`）
- **🖱️ 输入注入** — Linux uinput 键盘/鼠标事件注入
- **💓 心跳保活** — 5 秒心跳，30 秒超时自动断开
- **🔍 mDNS 发现** — 自动发现局域网内的 daemon（开发中）

## 架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│                           Client 端                                  │
│  ┌─────────────────────┐  ┌─────────────────────────────────────┐   │
│  │   lan-linkctl (CLI) │  │   lan-link-gui (GUI)                │   │
│  │   clap 子命令解析    │  │   eframe/egui 桌面应用               │   │
│  └─────────┬───────────┘  └──────────────┬──────────────────────┘   │
│            │                              │                          │
│            └──────────┬───────────────────┘                          │
│                       │                                              │
│         ┌─────────────▼──────────────┐                               │
│         │   lan-link-protocol crate  │                               │
│         │   帧编码 / ChaCha20加密     │                               │
│         │   可靠传输 / 流复用         │                               │
│         └─────────────┬──────────────┘                               │
└───────────────────────│─────────────────────────────────────────────┘
                        │ UDP (端口 9876)
                        ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      Daemon 端 (lan-linkd)                          │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────────────┐   │
│  │ connection   │  │ native_cmd   │  │ discovery (mDNS)         │   │
│  │ 连接状态机    │  │ 本地命令执行  │  │ 服务发现                  │   │
│  │ SYN/SYN-ACK  │  │ fs/system/   │  │                          │   │
│  │ 心跳/超时     │  │ net/service  │  │                          │   │
│  └─────────────┘  └──────────────┘  └──────────────────────────┘   │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────────────┐   │
│  │ shell crate  │  │ input crate  │  │ video crate (预留)       │   │
│  │ 命令执行引擎  │  │ uinput注入   │  │ 视频流捕获与编码          │   │
│  │ 流式输出      │  │ SendInput    │  │                          │   │
│  └─────────────┘  └──────────────┘  └──────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

## 快速开始

### 1. 克隆

```bash
git clone <repo-url>
cd lan-link
```

### 2. 编译

```bash
cargo build --release
```

### 3. 运行

```bash
# 服务端（目标机器）
sudo ./target/release/lan-linkd
# 记下输出的 PSK=xxxx

# 客户端（控制机器）
export LAN_LINK_PSK=<上面输出的 PSK>
./target/release/lan-linkctl info
```

## 编译产物

| 二进制 | 路径 | 说明 |
|--------|------|------|
| `lan-linkd` | `target/release/lan-linkd` | 守护进程（服务端），需在目标机器上以 root 运行 |
| `lan-linkctl` | `target/release/lan-linkctl` | 命令行客户端，连接 daemon 执行管理操作 |
| `lan-link-gui` | `target/release/lan-link-gui` | 跨平台桌面 GUI 客户端 |

## 配置说明

### PSK 密钥

PSK（Pre-Shared Key）是 32 字节的预共享密钥，用于加密所有通信。

**服务端**：首次运行自动生成并保存到 `/etc/lan-link/psk`，也可通过 `--psk` 手动指定。

```bash
# 首次运行自动生成
sudo lan-linkd

# 手动指定
sudo lan-linkd --psk <64位hex字符串>
```

**客户端**：通过 `--psk` 参数或 `LAN_LINK_PSK` 环境变量提供。

```bash
# 参数
lan-linkctl --psk <64位hex字符串> info

# 环境变量
export LAN_LINK_PSK=<64位hex字符串>
lan-linkctl info
```

### 端口

默认 UDP 端口 `9876`，可通过 `--port` 修改。

```bash
lan-linkd --port 9999
lan-linkctl --addr 192.168.1.100:9999 info
```

### systemd 服务

项目提供 systemd 单元文件 `deploy/lan-linkd.service`：

```bash
sudo cp target/release/lan-linkd /usr/local/bin/
sudo mkdir -p /etc/lan-link
sudo cp deploy/lan-linkd.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now lan-linkd
```

## 命令示例

```bash
# 系统信息
lan-linkctl info
lan-linkctl uname -a
lan-linkctl uptime

# 文件操作
lan-linkctl ls -la /home
lan-linkctl cat /etc/os-release
lan-linkctl tail -n 100 /var/log/syslog

# 进程管理
lan-linkctl ps
lan-linkctl top
lan-linkctl kill --pid 1234 --signal 9

# 网络
lan-linkctl netstat --tcp --listening
lan-linkctl dns --hostname example.com
lan-linkctl portscan --host 192.168.1.1

# 远程执行
lan-linkctl exec "df -h"
lan-linkctl iexec "top"          # 交互式
lan-linkctl shell                # 交互式 shell
lan-linkctl batch commands.txt   # 批量
lan-linkctl watch -e 5 "free -m" # 定时观察

# 服务管理
lan-linkctl service status sshd
lan-linkctl service restart nginx
lan-linkctl journal -u sshd -n 50

# Docker
lan-linkctl docker ps
lan-linkctl docker logs nginx

# 文件传输
lan-linkctl push --local /etc/hosts --remote /tmp/hosts
lan-linkctl pull --remote /var/log/syslog --local ./syslog

# 连接测试
lan-linkctl ping
```

## 目录结构

```
lan-link/
├── Cargo.toml                 # 工作空间根
├── README.md                  # 本文件
├── deploy/
│   └── lan-linkd.service      # systemd 单元文件
├── docs/
│   ├── architecture.md        # 架构设计
│   ├── getting-started.md     # 快速入门
│   ├── security.md            # 安全模型
│   ├── code-structure.md      # 代码结构
│   └── variables.md           # 变量命名规范
├── tests/                     # 端到端集成测试（Python）
├── crates/
│   ├── protocol/              # 协议 crate（帧格式、加密、ARQ、流复用）
│   ├── daemon/                # 守护进程（binary: lan-linkd）
│   ├── ctl/                   # CLI 客户端（binary: lan-linkctl）
│   ├── gui/                   # GUI 客户端（binary: lan-link-gui）
│   ├── shell/                 # 命令执行引擎
│   ├── input/                 # 输入注入（Linux uinput / Windows SendInput）
│   └── video/                 # 视频捕获与编码（预留）
```

## 依赖说明

| Crate | 关键依赖 |
|-------|---------|
| `protocol` | `chacha20poly1305`, `serde`, `bincode`, `bytes`, `bitflags` |
| `daemon` | `tokio`, `clap`, `sha2`, `md-5`, `libc` |
| `ctl` | `tokio`, `clap`, `hex` |
| `gui` | `eframe 0.34` (egui), `tokio`, `serde_json` |
| `shell` | `tokio`, `anyhow` |
| `input` | `bitflags`, `serde`, `libc` (Unix), `windows 0.58` (Windows) |
| `video` | `tracing`, `anyhow` |

## 许可证

MIT

## 贡献指南

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/my-feature`)
3. 提交变更 (`git commit -am 'Add my feature'`)
4. 推送到分支 (`git push origin feature/my-feature`)
5. 创建 Pull Request

### 开发注意事项

- Rust 稳定版即可编译，无需 nightly
- 跨平台：请确保在 Linux/Windows 上的兼容性
- `native_cmd` 中的实现应优先使用纯 Rust 文件操作（如 `std::fs`），而非调用外部命令
- 新增 `NativeCmdType` 变体时需同步更新 `ctl` 和 `daemon` 两侧的匹配逻辑
- 所有加密解密操作必须通过 `protocol::crypto` 模块
