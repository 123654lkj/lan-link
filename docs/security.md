# 安全模型说明

## 概述

lan-link 采用 **预共享密钥（PSK）+ ChaCha20-Poly1305 加密** 的安全模型。
所有控制数据和文件传输均在 UDP 加密通道中进行。

## 加密层

### 算法选择

- **对称加密**：ChaCha20-Poly1305（AEAD）
- **密钥长度**：256 位（32 字节）
- **Nonce**：96 位（12 字节），由 `make_nonce(conn_id, seq)` 生成
- **认证**：Poly1305 MAC，提供完整性保护和防重放

### Nonce 生成

```rust
pub fn make_nonce(conn_id: u64, seq: u32) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce[0..8].copy_from_slice(&conn_id.to_le_bytes());
    nonce[8..12].copy_from_slice(&seq.to_le_bytes());
    nonce
}
```

Nonce 由连接 ID 和序列号组合，确保同一连接下每个包的 nonce 唯一。

### 加密流程

```
原始数据 → bincode 序列化 → ChaCha20-Poly1305 加密 → 封装为 Data 包 → UDP 发送
```

## PSK 管理

### 来源优先级

| 角色 | 第一来源 | 回退来源 |
|------|---------|---------|
| **daemon** | `--psk` 命令行参数 | `/etc/lan-link/psk` 文件 → 自动生成并保存 |
| **ctl** | `--psk` 命令行参数 | `LAN_LINK_PSK` 环境变量 → 未设置则报错退出 |
| **gui** | 配置文件 `gui-config.json` | `LAN_LINK_PSK` 环境变量 → 提示用户输入 |

### 存储

- **daemon**：首次运行自动生成 32 字节随机密钥，hex 编码保存到 `/etc/lan-link/psk`
- **ctl**：不存储 PSK，每次通过参数或环境变量提供
- **gui**：PSK 保存在用户配置文件中（`~/.config/lan-link/gui-config.json` 或 `%APPDATA%/lan-link/gui-config.json`）

## 连接安全

### 握手流程

```
1. Client ── SYN ──────────► Daemon
   (明文, 包含随机 conn_id)

2. Daemon ── SYN-ACK ──────► Client
   (明文, 确认 conn_id)

3. Client ── Hello (加密) ─► Daemon
   (使用 PSK + conn_id 加密, 验证客户端持有正确密钥)

4. Daemon ── HelloAck (加密) ► Client
   (确认加密通道建立)
```

### 连接状态机

```
Listening → (收到 SYN) → SynReceived → (收到 Hello) → Established
Established → (收到 RST/超时) → Closed
```

- 超时时间：30 秒无活动自动断开
- 心跳间隔：5 秒

## 威胁模型

### 防御的威胁

| 威胁 | 防御措施 |
|------|----------|
| 网络嗅探 | 所有数据 ChaCha20-Poly1305 加密 |
| 重放攻击 | Nonce 唯一性 + 序列号检查 |
| 中间人攻击 | PSK 预共享，初次握手后加密 |
| 篡改 | Poly1305 MAC 认证 |
| 未授权访问 | PSK 认证，无 PSK 无法建立连接 |

### 未防御的威胁

| 威胁 | 说明 | 缓解措施 |
|------|------|----------|
| PSK 泄露 | PSK 被获取后可以完全仿冒客户端 | 定期更换 PSK，文件权限 600 |
| DoS 攻击 | UDP 无连接特性可能导致放大攻击 | 建议在防火墙限制源 IP |
| 本地提权 | 守护进程以 root 运行 | 监控进程行为，最小化暴露 |

## 最佳实践

1. **PSK 轮换**：定期更换 PSK，使用 `lan-linkd --psk <new_key>` 重启服务
2. **网络隔离**：在交换机/VLAN 层面限制对 daemon 端口的访问
3. **日志监控**：监控 `/var/log/lan-linkd.log` 中的异常连接
4. **最小权限**：仅开放 daemon 端口（默认 9876/udp）给受信任客户端
