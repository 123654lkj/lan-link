# lan-linkctl CLI 命令参考

> 版本 0.1.0 | UDP 加密远程管理工具 | 通信目标 192.168.31.244:9876



## 原生命令架构（v0.1.0+）

从 2026-06-02 起，lan-link 所有命令改为**原生执行模式**：

| 方面 | 旧方式 (shell) | 新方式 (native) |
|------|---------------|-----------------|
| 协议 | 
r! / 
r1! 宏 → 远端 sh -c | 
c! 宏 → 结构化 NativeCmdType 枚举 |
| daemon 执行 | sys_exec() → shell fork | 
un_native_cmd() → std::process::Command 或 /proc 直接读取 |
| 安全 | 依赖远端 shell，可能受到 shell 注入 | 无 shell 注入风险，参数严格类型化 |
| 性能 | 每个命令 fork 一次 shell | 直接 spawn 系统二进制或读 /proc |
| 文件操作 | mount, 
m, diff 等走 shell | 调用 /usr/bin/mount, /usr/bin/rm 等系统工具 |
| 系统信息 | 解析 shell 输出 | 直接读 /proc/meminfo, /proc/cpuinfo, /proc/stat 等 |

内核信息（uptime、free、cpu、ps、dmesg）完全用 Rust 读 /proc，不依赖任何外部命令。
文件系统操作（ls、cat、head、tail、find、grep 等）和服务管理（systemctl、docker、pkg）通过 std::process::Command 调用系统二进制，不透传 shell。
## 全局选项

所有子命令共享以下全局选项：

| 选项 | 默认值 | 说明 |
|------|--------|------|
| `--addr` / `-a` | 192.168.31.244:9876 | 目标 daemon 地址 |
| `--psk` / `-p` | (内置 32 字节 hex) | 预共享密钥 |

## 命令执行

| 命令 | 功能 | 示例 |
|------|------|------|
| `exec <cmd...>` | 远程执行命令，等待结果 | `lan-linkctl exec ls -la /root` |
| `iexec <cmd...>` | 交互式执行（支持 stdin 双向） | `lan-linkctl iexec bash` |
| `shell` | 交互式 shell 会话 | `lan-linkctl shell` |
| `batch <file>` | 批量执行文件中的命令 | `lan-linkctl batch setup.txt` |
| `watch -e <sec> <cmd...>` | 每隔 N 秒重复执行 | `lan-linkctl watch -e 5 df -h` |

## 文件操作

| 命令 | 功能 | 示例 |
|------|------|------|
| `ls [path]` | 列出目录（-l 详情 / --all 隐藏） | `lan-linkctl ls -la /var/log` |
| `cat <path>` | 显示文件内容 | `lan-linkctl cat /etc/hostname` |
| `head <path>` | 文件开头 N 行 | `lan-linkctl head -n 20 /var/log/syslog` |
| `tail <path>` | 末尾行或实时跟踪（-f） | `lan-linkctl tail -n 50 -f /var/log/syslog` |
| `find [path]` | 查找文件（--name / --type / --maxdepth） | `lan-linkctl find /etc --name *.conf` |
| `grep <pat> [path]` | 搜索内容 | `lan-linkctl grep -rn error /var/log` |
| `du [path]` | 磁盘用量 | `lan-linkctl du -sh /home` |
| `wc <paths...>` | 行数/字数统计 | `lan-linkctl wc -l /var/log/syslog` |
| `stat <path>` | 文件元信息 | `lan-linkctl stat /etc/passwd` |
| `tree [path]` | 目录树 | `lan-linkctl tree /etc --depth 3` |
| `diff <f1> <f2>` | 差异比较 | `lan-linkctl diff a.txt b.txt` |

## 文件传输

| 命令 | 功能 | 示例 |
|------|------|------|
| `push -l <local> -r <remote>` | 上传本地到远端 | `lan-linkctl push -l config.json -r /tmp/c.json` |
| `pull -r <remote> -l <local>` | 下载远端到本地 | `lan-linkctl pull -r /etc/hosts -l hosts.txt` |

## 目录/文件管理

| 命令 | 功能 | 示例 |
|------|------|------|
| `mkdir <paths...>` | 创建目录（-p 递归） | `lan-linkctl mkdir -p /tmp/a/b` |
| `rm <paths...>` | 删除（-r 递归 / -f 强制） | `lan-linkctl rm -rf /tmp/cache` |
| `mv <src> <dest>` | 移动/重命名 | `lan-linkctl mv /tmp/a /tmp/b` |
| `cp <src> <dest>` | 复制（-r 递归） | `lan-linkctl cp -r /a /b` |
| `chmod <mode> <paths...>` | 修改权限 | `lan-linkctl chmod 644 /etc/hosts` |
| `chown <owner> <paths...>` | 修改所有者 | `lan-linkctl chown root:root /etc/hosts` |

## 进程管理

| 命令 | 功能 | 示例 |
|------|------|------|
| `ps` | 列出进程（-f 全格式 / --tree / --user） | `lan-linkctl ps --tree` |
| `kill <pid>` | 终止进程（-s 信号号，默认 15） | `lan-linkctl kill 1234 -s 9` |
| `pgrep <name>` | 按名称查找进程 | `lan-linkctl pgrep nginx` |
| `pkill <name>` | 按名称杀进程 | `lan-linkctl pkill nginx -s 9` |
| `top` | 循环显示进程排名 | `lan-linkctl top --interval 3 --iterations 5` |
| `signal <id> <signo>` | 给运行中的 exec 发信号 | `lan-linkctl signal 1 9` |

## 系统信息

| 命令 | 功能 | 示例 |
|------|------|------|
| `info` | 系统概要（CPU/内存/磁盘/运行时间） | `lan-linkctl info` |
| `uname` | 内核信息（-a / -r / -m） | `lan-linkctl uname -a` |
| `uptime` | 运行时间 | `lan-linkctl uptime` |
| `hostname` | 主机名 | `lan-linkctl hostname` |
| `whoami` | 当前用户 | `lan-linkctl whoami` |
| `who` | 登录用户 | `lan-linkctl who` |
| `last` | 最近登录 | `lan-linkctl last --lines 10` |
| `free` | 内存使用（-h 可读格式） | `lan-linkctl free -h` |
| `cpu` | CPU 详细信息 | `lan-linkctl cpu` |
| `dmesg` | 内核日志 | `lan-linkctl dmesg --lines 30` |
| `df` | 磁盘分区（--human / --all） | `lan-linkctl df -h` |
| `lsblk` | 块设备 | `lan-linkctl lsblk` |
| `mount` | 挂载点 | `lan-linkctl mount` |

## 网络

| 命令 | 功能 | 示例 |
|------|------|------|
| `ping` | RTT 延迟测试 | `lan-linkctl ping --count 10` |
| `netstat` | 网络连接（-t TCP / -u UDP / -l 监听） | `lan-linkctl netstat -tln` |
| `ip` | 网络接口（--addr / --route / --link） | `lan-linkctl ip --addr` |
| `portscan <host>` | TCP 端口扫描 | `lan-linkctl portscan 127.0.0.1 --start-port 1 --end-port 1000` |
| `arp` | ARP 表 | `lan-linkctl arp` |
| `dns <hostname>` | DNS 解析 | `lan-linkctl dns google.com --type A` |
| `ssh` | 检查 SSH 监听 | `lan-linkctl ssh` |

## 服务管理（systemd）

| 命令 | 功能 | 示例 |
|------|------|------|
| `service list` | 列出服务（--active / --failed） | `lan-linkctl service list --failed` |
| `service status <name>` | 服务状态 | `lan-linkctl service status nginx` |
| `service start/stop/restart/reload <name>` | 控制服务 | `lan-linkctl service restart nginx` |
| `service enable/disable <name>` | 自启 | `lan-linkctl service enable nginx` |
| `journal` | 日志查询（-u unit / -p priority / -f 跟踪） | `lan-linkctl journal -u nginx -n 50 --priority err` |

## 包管理（apt）

| 命令 | 功能 | 示例 |
|------|------|------|
| `pkg list` | 已安装包 | `lan-linkctl pkg list` |
| `pkg search <query>` | 搜索 | `lan-linkctl pkg search nginx` |
| `pkg install/remove <name>` | 安装/卸载 | `lan-linkctl pkg install nginx` |
| `pkg update/upgrade` | 更新/升级 | `lan-linkctl pkg upgrade` |

## Docker

| 命令 | 功能 | 示例 |
|------|------|------|
| `docker ps` | 容器列表（-a 含停止） | `lan-linkctl docker ps -a` |
| `docker logs <name>` | 容器日志（-f 跟踪） | `lan-linkctl docker logs webapp --tail 200` |
| `docker stats` | 资源统计 | `lan-linkctl docker stats` |
| `docker exec <container> <cmd...>` | 容器内执行 | `lan-linkctl docker exec webapp ls -la` |
| `docker info` | Docker 系统信息 | `lan-linkctl docker info` |
| `docker images` | 镜像列表 | `lan-linkctl docker images` |
| `docker rm <container>` | 删除容器（-f 强制） | `lan-linkctl docker rm webapp -f` |
| `docker control <container> <action>` | 通用操作（start/stop/pause） | `lan-linkctl docker control webapp restart` |

## 杂项

| 命令 | 功能 | 示例 |
|------|------|------|
| `crontab list/remove` | crontab 操作 | `lan-linkctl crontab list` |
| `firewall` | 防火墙规则 | `lan-linkctl firewall --backend ufw` |
| `checksum <path>` | 文件校验和 | `lan-linkctl checksum /etc/hosts --algorithm sha256` |
| `status` | 测试连接 | `lan-linkctl status` |
| `version` | 版本号 | `lan-linkctl version` |

## Python 客户端

### 基本用法

```powershell
python client_win.py exec ls -la
echo hello | python client_win.py exec cat
python client_win.py --write-config
python client_win.py --show-config
```

### 配置文件

位置: %%APPDATA%%\lan-link\config.json

```json
{
  "addr": "192.168.31.244:9876",
  "psk": "ca989e3c0e5f763c1ba7f3a8308a9445ca1d5b77a3e896d55e4eac86f25dfb1d"
}
```

### Python vs Rust 选择

| 场景 | 推荐 | 原因 |
|------|------|------|
| 简单远程命令 | Python | 无需编译，即改即用 |
| 文件传输 | Rust ctl | push/pull 内置 |
| 交互式 shell | Rust ctl | iexec/shell 支持 stdin 双向 |
| 批量执行 | Rust ctl | batch + 超时 + 错误统计 |
| 系统管理 | Rust ctl | 40+ 子命令覆盖最全 |

## 快速参考

```bash
lan-linkctl info                          # 系统概览
lan-linkctl top --interval 2              # 实时进程监控
lan-linkctl tail -f /var/log/syslog       # 日志跟踪
lan-linkctl pull -r access.log -l log     # 下载文件
lan-linkctl push -l config -r /tmp/cfg    # 上传文件
lan-linkctl batch setup.sh                # 批量执行
lan-linkctl watch -e 5 'df -h'           # 重复监控
lan-linkctl ping -c 10                    # 延迟测试
lan-linkctl docker logs webapp -f         # Docker 日志
```

## 故障排查

| 问题 | 检查项 |
|------|--------|
| No SYN-ACK | daemon 在线？systemctl restart lan-linkd |
| 超时 | PSK 匹配？防火墙 UDP 9876 放行？ |
| 执行失败 | daemon 日志：journalctl -u lan-linkd -n 50 |
| 乱码 | 远端 UTF-8，中文需 LANG=zh_CN.UTF-8 |
