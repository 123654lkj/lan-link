# 快速入门

## 环境要求

### Rust 版本

- **最低支持**：Rust 1.78+（稳定版）
- **推荐**：latest stable（通过 `rustup update` 获取）
- **工具链**：无需 nightly 特性

### 支持的操作系统

| 角色 | 操作系统 | 状态 |
|------|---------|------|
| **daemon**（服务端） | Linux (x86_64, aarch64) | ✅ 完整支持 |
| **daemon** | macOS | ❌ 未测试 |
| **ctl**（CLI 客户端） | Linux / Windows / macOS | ✅ 完整支持 |

### 系统依赖

**Linux daemon：**
```bash

```

```bash
# 依据图形后端选择
sudo apt install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev
```

**Windows：**
- Windows 10/11 x86_64
- 若使用 `x86_64-pc-windows-gnu` 目标，需安装 MSYS2/MinGW
- 推荐使用 MSVC 工具链

---

## 编译（从源码）

### 完整编译

```bash
git clone <repo-url>
cd lan-link

# 编译所有二进制（debug 模式）
cargo build

# 或 release 模式（推荐生产使用）
cargo build --release
```

编译产物：

```
target/release/
├── lan-linkd       # 守护进程（服务端）
├── lan-linkctl     # CLI 客户端
```

### 部分编译

```bash
# 仅编译 daemon
cargo build --release -p lan-linkd

# 仅编译 CLI 客户端
cargo build --release -p lan-linkctl


# 仅编译协议库
cargo build --release -p lan-link-protocol
```

### 交叉编译（Linux → Windows）

```bash
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

---

## 安装（二进制部署）

### 1. 部署 Daemon（目标机器）

```bash
# 复制二进制
sudo cp target/release/lan-linkd /usr/local/bin/

# 创建配置目录
sudo mkdir -p /etc/lan-link

# 首次运行（自动生成 PSK）
sudo /usr/local/bin/lan-linkd
# 输出中会显示 PSK=xxxx...，请务必记录下来！
# 按 Ctrl+C 停止，后续会配置为系统服务
```

### 2. 配置 systemd 服务（可选）

```bash
sudo cp deploy/lan-linkd.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now lan-linkd
sudo systemctl status lan-linkd
```

或手动创建服务文件 `/etc/systemd/system/lan-linkd.service`：

```ini
[Unit]
Description=lan-linkd - LAN link daemon
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=/usr/local/bin/lan-linkd --port 9876
Restart=always
RestartSec=5
User=root
WorkingDirectory=/opt/lan-link
StandardOutput=journal
StandardError=journal

# 安全加固
NoNewPrivileges=false
PrivateTmp=true

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now lan-linkd
```

### 3. 部署客户端（控制机器）

```bash
# 复制 CLI 客户端
cp target/release/lan-linkctl /usr/local/bin/

```


---

## 配置 PSK

PSK（Pre-Shared Key）是 32 字节 hex 编码的预共享密钥，所有通信必须通过 PSK 加密。

### 获取 PSK

**方式一**：从 daemon 首次运行的输出中获取
```bash
# 终端输出中查找：PSK=xxxxxxxxxxxxxxxx...
sudo lan-linkd | grep PSK
```

**方式二**：读取已保存的 PSK 文件
```bash
sudo cat /etc/lan-link/psk
```

**方式三**：手动生成新 PSK
```bash
# 生成 32 字节随机密钥
openssl rand -hex 32
# 覆盖 daemon 的 PSK
echo -n "<上面输出的64位hex>" | sudo tee /etc/lan-link/psk
sudo systemctl restart lan-linkd
```

### 配置到客户端

```bash
# 方式 A：每次通过 --psk 参数
lan-linkctl --psk <64位hex字符串> info

# 方式 B：设置环境变量（推荐）
export LAN_LINK_PSK=<64位hex字符串>
lan-linkctl info

```

---

## 启动 Daemon

### 手动运行

```bash
# 默认端口 9876
sudo lan-linkd

# 自定义端口
sudo lan-linkd --port 9999

# 手动指定 PSK（覆盖文件中的密钥）
sudo lan-linkd --psk <64位hex字符串>

# 禁用 mDNS 发现
sudo lan-linkd --discovery false
```

### 作为系统服务运行

```bash
sudo systemctl start lan-linkd
sudo systemctl enable lan-linkd
sudo systemctl status lan-linkd
```

### 查看日志

```bash
# 实时查看
sudo journalctl -u lan-linkd -f

# 查看最近 100 条
sudo journalctl -u lan-linkd -n 100 --no-pager
```

---

## 连接测试

### 基本连接测试

```bash
# 测试连通性（发送 Hello 包并等待回复）
lan-linkctl ping
# 输出示例：
# Reply from 192.168.31.244:9876: time=1.23ms
# --- 5 ping statistics ---
# 5 packets transmitted, 5 received
# min/avg/max = 1.10/1.23/1.45 ms
```

### 查看远端系统信息

```bash
lan-linkctl info
# 输出示例：
# Hostname: server-01
# OS: Linux 6.5.0-28-generic
# Uptime: 14 days, 3 hours
# CPU: 8 cores @ 3.2GHz
# Memory: 15.6 GiB / 31.4 GiB
```

### 状态检查

```bash
lan-linkctl status
# Connected to 192.168.31.244:9876 (conn_id=123456789)
```

---

## 常用命令速查表

### 系统管理

| 命令 | 说明 |
|------|------|
| `lan-linkctl info` | 系统概要信息 |
| `lan-linkctl uname -a` | 内核版本 |
| `lan-linkctl uptime` | 运行时间 |
| `lan-linkctl whoami` | 当前用户 |
| `lan-linkctl who` | 登录用户列表 |
| `lan-linkctl last -n 20` | 最近登录记录 |
| `lan-linkctl hostname` | 主机名 |

### 进程管理

| 命令 | 说明 |
|------|------|
| `lan-linkctl ps` | 进程列表 |
| `lan-linkctl ps --full` | 完整进程信息 |
| `lan-linkctl ps --tree` | 进程树 |
| `lan-linkctl top` | 进程排名（循环显示） |
| `lan-linkctl kill --pid 1234` | 终止进程（默认 SIGTERM=15） |
| `lan-linkctl kill --pid 1234 --signal 9` | 强制终止 |
| `lan-linkctl pgrep nginx` | 按名称查找进程 |
| `lan-linkctl pkill nginx` | 按名称终止进程 |

### 文件操作

| 命令 | 说明 |
|------|------|
| `lan-linkctl ls /home` | 列出目录 |
| `lan-linkctl ls -la /home` | 详细列出（含隐藏文件） |
| `lan-linkctl cat /etc/os-release` | 查看文件内容 |
| `lan-linkctl head -n 20 /var/log/syslog` | 查看文件开头 |
| `lan-linkctl tail -n 100 /var/log/syslog` | 查看文件末尾 |
| `lan-linkctl tail -n 20 -f /var/log/syslog` | 实时跟踪文件 |
| `lan-linkctl find / -n "*.conf" --maxdepth 3` | 查找文件 |
| `lan-linkctl grep "error" /var/log/syslog -r` | 搜索文件内容 |
| `lan-linkctl du /home --summarize` | 计算磁盘用量 |
| `lan-linkctl df --human` | 磁盘分区使用情况 |
| `lan-linkctl stat /etc/hosts` | 文件元信息 |
| `lan-linkctl tree /etc --depth 2` | 目录树 |

### 文件编辑/传输

| 命令 | 说明 |
|------|------|
| `lan-linkctl cp /etc/hosts /tmp/hosts.bak` | 复制文件 |
| `lan-linkctl mv /tmp/hosts.bak /tmp/hosts.old` | 移动/重命名 |
| `lan-linkctl rm /tmp/hosts.old` | 删除文件 |
| `lan-linkctl mkdir -r /tmp/test/dir` | 创建目录 |
| `lan-linkctl chmod 755 /tmp/script.sh` | 修改权限 |
| `lan-linkctl push --local /etc/hosts --remote /tmp/hosts` | 上传文件 |
| `lan-linkctl pull --remote /var/log/syslog --local ./syslog` | 下载文件 |

### 网络工具

| 命令 | 说明 |
|------|------|
| `lan-linkctl netstat --tcp --listening` | 查看监听端口 |
| `lan-linkctl netstat --tcp --udp` | 查看所有连接 |
| `lan-linkctl ip addr` | 网络接口地址 |
| `lan-linkctl ip route` | 路由表 |
| `lan-linkctl portscan --host 127.0.0.1` | 端口扫描 |
| `lan-linkctl arp` | ARP 表 |
| `lan-linkctl dns --hostname example.com` | DNS 查询 |

### 远程执行

| 命令 | 说明 |
|------|------|
| `lan-linkctl exec "df -h"` | 执行命令并等待结果 |
| `lan-linkctl exec "ping -c 3 8.8.8.8"` | 执行带输出的命令 |
| `lan-linkctl iexec "top"` | 交互式执行（支持 stdin） |
| `lan-linkctl iexec "bash"` | 交互式 shell（伪终端） |
| `lan-linkctl shell` | 启动交互式 shell 会话 |
| `lan-linkctl batch commands.txt` | 批量执行命令文件 |
| `lan-linkctl watch -e 5 "free -m"` | 每隔 5 秒观察输出 |

### 服务管理

| 命令 | 说明 |
|------|------|
| `lan-linkctl service list --active` | 列出运行中的服务 |
| `lan-linkctl service list --failed` | 列出失败的服务 |
| `lan-linkctl service status sshd` | 查看服务状态 |
| `lan-linkctl service start nginx` | 启动服务 |
| `lan-linkctl service stop nginx` | 停止服务 |
| `lan-linkctl service restart nginx` | 重启服务 |
| `lan-linkctl service enable nginx` | 启用自启 |
| `lan-linkctl service disable nginx` | 禁用自启 |
| `lan-linkctl journal -u sshd -n 50` | 查看服务日志 |

### 包管理

| 命令 | 说明 |
|------|------|
| `lan-linkctl pkg update` | 更新包索引 |
| `lan-linkctl pkg upgrade` | 升级所有包 |
| `lan-linkctl pkg install nginx` | 安装包 |
| `lan-linkctl pkg remove nginx` | 卸载包 |
| `lan-linkctl pkg search "nginx"` | 搜索包 |
| `lan-linkctl pkg list --installed` | 列出已安装包 |

### Docker 管理

| 命令 | 说明 |
|------|------|
| `lan-linkctl docker ps` | 列出运行中的容器 |
| `lan-linkctl docker ps --all` | 列出所有容器 |
| `lan-linkctl docker logs nginx` | 查看容器日志 |
| `lan-linkctl docker stats` | 容器资源统计 |
| `lan-linkctl docker exec nginx "ls -la"` | 在容器中执行 |
| `lan-linkctl docker images` | 镜像列表 |
| `lan-linkctl docker info` | Docker 系统信息 |

---

## 故障排查

### 连接问题

| 症状 | 可能原因 | 解决方法 |
|------|---------|---------|
| `no SYN-ACK from ...` | daemon 未运行或端口不通 | 检查 daemon 状态：`systemctl status lan-linkd` |
| | 防火墙阻止 UDP 端口 | `sudo ufw allow 9876/udp` |
| | 地址错误 | 确认 `--addr` 正确：`lan-linkctl --addr 192.168.1.100:9876 info` |
| `decrypt failed` | PSK 不匹配 | 检查两端的 PSK 是否一致 |
| 连接超时 | 网络断开 | `ping <daemon_ip>` 确认网络连通性 |
| | 防火墙 drop 包 | 检查 iptables/nftables 规则 |

### 权限问题

| 症状 | 可能原因 | 解决方法 |
|------|---------|---------|
| `Permission denied`（绑定端口） | 未以 root 运行 | `sudo lan-linkd` |
| `No such file or directory`（PSK 路径） | 配置目录不存在 | `sudo mkdir -p /etc/lan-link` |

### 调试模式

```bash
# 启用 verbose 输出（显示 info 级别日志）
lan-linkctl -v info

# daemon 前台运行查看日志
sudo lan-linkd

# 查看 journal 日志
sudo journalctl -u lan-linkd -f --no-pager

# 设置 tracing 环境变量（更详细的日志）
RUST_LOG=debug sudo lan-linkd
```

### 常见错误

| 错误信息 | 解决方法 |
|---------|---------|
| `未设置 PSK` | 通过 `--psk` 或 `LAN_LINK_PSK` 环境变量提供 |
| `invalid PSK hex` | PSK 必须是 64 位 hex 字符（32 字节） |
| `Connection timed out` | 增大超时值（daemon 默认 30 秒无活动断开） |
| `Address already in use` | 端口被占用，更换端口或结束冲突进程 |
