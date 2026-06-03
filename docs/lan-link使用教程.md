# lan-link 使用教程

> 局域网远程管理工具 | Windows -> Linux | UDP 加密传输

## 架构概述

```
Windows (客户端)              Linux (服务端 - 团子 192.168.31.244)
+-------------+              +------------------+
| Python CLI  |--- UDP 9876->| lan-linkd daemon |
| Rust CLI    |   ChaCha20   | - shell exec     |
|             |   Poly1305   | - file I/O       |
+-------------+              +------------------+
```

## 快速开始

### 1. 确认服务端在线

```bash
# 在团子上
systemctl status lan-linkd
# 应显示 active (running)，监听 UDP 9876
```

### 2. Python 客户端（零编译）

```powershell
cd G:\codex-AI-tools\lan-link

# 执行远程命令
python client_win.py exec ls -la

# 带管道 stdin
echo hello | python client_win.py exec cat

# 生成配置文件
python client_win.py --write-config
```

### 3. Rust CLI（功能最全）

需要 MSVC 编译，或使用预编译的 `lan-linkctl.exe`。

```bash
lan-linkctl exec ls -la
lan-linkctl info
lan-linkctl status
```

## 命令行使用详解

### 命令执行

```bash
# 简单执行，等待完成
lan-linkctl exec 'ls -la /root'

# 交互式执行 -- 支持 stdin 双向（退出码会显示）
lan-linkctl iexec bash

# 交互式 shell 会话
lan-linkctl shell

# 批量执行文件中的命令（每行一条，# 为注释）
echo 'hostname' > /tmp/cmds.txt
echo 'uname -a' >> /tmp/cmds.txt
lan-linkctl batch /tmp/cmds.txt
```

### 文件传输

```bash
# 上传本地文件到远端
lan-linkctl push -l myfile.txt -r /tmp/myfile.txt

# 下载远端文件到本地
lan-linkctl pull -r /var/log/syslog -l syslog.txt
```

### 文件操作

```bash
lan-linkctl ls -la /root
lan-linkctl cat /etc/hostname
lan-linkctl head -n 20 /var/log/syslog
lan-linkctl tail -n 50 -f /var/log/syslog   # 实时跟踪
lan-linkctl find /etc --name *.conf
lan-linkctl grep -rn error /var/log
lan-linkctl du -sh /home
```

### 进程管理

```bash
lan-linkctl ps --tree
lan-linkctl top --interval 2 --iterations 10
lan-linkctl kill 1234 -s 9
lan-linkctl pgrep nginx
```

### 系统信息

```bash
lan-linkctl info        # 一键系统概览
lan-linkctl free -h     # 内存
lan-linkctl df -h       # 磁盘
lan-linkctl uname -a    # 内核
lan-linkctl uptime      # 运行时间
lan-linkctl dmesg -n 30 # 内核日志
```

### 网络

```bash
lan-linkctl ping -c 5
lan-linkctl netstat -tln
lan-linkctl ip --addr
lan-linkctl portscan 127.0.0.1 --start-port 1 --end-port 1024
lan-linkctl dns google.com
```

### 服务管理

```bash
lan-linkctl service list --failed
lan-linkctl service status nginx
lan-linkctl service restart nginx
lan-linkctl journal -u nginx -n 50 --priority err
```

### 包管理

```bash
lan-linkctl pkg update
lan-linkctl pkg install nginx
lan-linkctl pkg search python
```

### Docker

```bash
lan-linkctl docker ps -a
lan-linkctl docker logs webapp -f
lan-linkctl docker exec webapp ls -la
```

## 配置

| 文件/路径 | 说明 |
|-----------|------|
| %APPDATA%\lan-link\config.json | Python 客户端配置 |
| /etc/lan-link/psk | 服务端 PSK（自动生成） |

默认 PSK: `ca989e3c0e5f763c1ba7f3a8308a9445ca1d5b77a3e896d55e4eac86f25dfb1d`

## 故障排查

**No SYN-ACK**
```bash
systemctl restart lan-linkd
journalctl -u lan-linkd -n 50
```

**超时**
- 检查 PSK 是否正确
- 检查防火墙：`ufw status` 或 `iptables -L -n`，确认 UDP 9876 已放行
- Windows 防火墙也要放行 UDP 出站

**执行失败**
- 检查 daemon 日志：`journalctl -u lan-linkd -n 50`
- 检查 RUST_LOG=info,lan_linkd=debug 级别

**中文乱码**
```bash
# 确保远端语言环境
export LANG=zh_CN.UTF-8
```

## 相关文档

- `docs/cli-reference.md` — 完整命令参考
- `docs/protocol-map.md` — 协议细节
- `docs/ll-design.md` — 项目设计
