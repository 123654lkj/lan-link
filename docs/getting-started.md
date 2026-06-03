# 快速入门教程

## 安装

### 从源码编译

```bash
# 克隆仓库
git clone <repo-url>
cd lan-link

# 编译所有组件
cargo build --release

# 编译产物位于 target/release/
# - lan-linkd   : 守护进程（服务端）
# - lan-linkctl : 命令行客户端
# - lan-link-gui: GUI 客户端
```

### 部署守护进程

```bash
# 将二进制复制到系统路径
sudo cp target/release/lan-linkd /usr/local/bin/

# 创建 PSK 配置文件
sudo mkdir -p /etc/lan-link
lan-linkd   # 首次运行会自动生成 PSK 并保存到 /etc/lan-link/psk
# 注意输出中的 PSK=xxxx，这是客户端连接需要的密钥
```

### 配置守护进程为系统服务

```ini
# /etc/systemd/system/lan-linkd.service
[Unit]
Description=LAN Link Daemon
After=network.target

[Service]
ExecStart=/usr/local/bin/lan-linkd
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now lan-linkd
```

## 快速使用

### 设置 PSK

```bash
# 方式一：命令行参数
lan-linkctl --psk <32字节hex密钥> uptime

# 方式二：环境变量
export LAN_LINK_PSK=<32字节hex密钥>
lan-linkctl uptime
```

### 基本命令

```bash
# 查看系统信息
lan-linkctl info
lan-linkctl uname -a
lan-linkctl uptime

# 文件操作
lan-linkctl ls /home
lan-linkctl cat /etc/os-release
lan-linkctl head -n 5 /var/log/syslog

# 进程管理
lan-linkctl ps
lan-linkctl top
lan-linkctl kill --pid 1234

# 网络
lan-linkctl netstat --tcp
lan-linkctl dns --hostname example.com
```

### 远程执行命令

```bash
# 单条命令（等待返回）
lan-linkctl exec "df -h"

# 交互式执行（支持 stdin）
lan-linkctl iexec "bash"
lan-linkctl iexec "top"

# 流式 shell
lan-linkctl shell

# 批量执行
lan-linkctl batch commands.txt

# 持续观察
lan-linkctl watch -e 5 "free -m"
```

### 文件传输

```bash
# 上传
lan-linkctl push --local /etc/hosts --remote /tmp/hosts

# 下载
lan-linkctl pull --remote /var/log/syslog --local ./syslog
```

### 系统管理

```bash
# 服务管理
lan-linkctl service status sshd
lan-linkctl service restart network

# 包管理
lan-linkctl pkg update
lan-linkctl pkg install nginx

# Docker
lan-linkctl docker ps
lan-linkctl docker logs nginx

# 防火墙
lan-linkctl firewall
```

## GUI 客户端

```bash
# 启动 GUI
lan-link-gui

# 在设置界面添加主机：
# 1. 点击 "Settings" 按钮
# 2. 填写主机名、地址（host:port）、PSK
# 3. 点击 "保存配置"
# 4. 选择主机，点击 "连接"
```

## 安全注意事项

1. **PSK 保护**：PSK 是连接的唯一凭证，妥善保管
2. **网络隔离**：建议在受信任的局域网使用，或配合 VPN
3. **最小权限**：守护进程以 root 运行，客户端可按需限制
4. **日志审计**：所有操作记录在 `/var/log/lan-linkd.log`
