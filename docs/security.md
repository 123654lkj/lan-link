# 安全模型

> **平台支持说明**：当前 daemon（lan-linkd）仅支持 Linux。Windows 平台的支持为未来计划。
> 以下安全机制（uinput 注入、`/proc` 读取、iptables 建议等）均基于 Linux 实现。

## 概述

lan-link 的安全模型建立在 **预共享密钥（PSK）+ ChaCha20-Poly1305 AEAD 加密** 之上。所有控制数据、文件传输和输入事件均在 UDP 加密通道中传输，确保数据的机密性、完整性和认证性。

---

## 加密算法选择理由

### 选择 ChaCha20-Poly1305 而非 AES

| 特性 | ChaCha20-Poly1305 | AES-256-GCM |
|------|------------------|-------------|
| **软件实现性能** | ⭐ 优秀（纯 SIMD 优化） | ⭐ 优秀（AES-NI 硬件加速） |
| **无硬件加速时性能** | ⭐⭐ 显著更优 | ⬇️ 明显降级 |
| **侧信道攻击抵抗** | ⭐⭐ 设计上抵抗 | ⚠️ 需常数时间实现 |
| **实现复杂度** | 简单 | 复杂（GCM 的 GHASH 域运算） |
| **Rust 生态支持** | `chacha20poly1305` crate（纯 Rust） | `aes-gcm` crate（需 AES-NI 或软件回退） |

**选择理由**：

1. **跨平台一致性**：lan-link 需要在 x86_64 Linux、ARM Linux、Windows 等多种平台上提供一致的性能。ChaCha20-Poly1305 在所有平台上都有高效的纯软件实现，不依赖 CPU 硬件加密指令。
2. **嵌入式/低端设备友好**：树莓派等 ARM 设备可能没有 AES 硬件加速，ChaCha20 在这些平台上性能显著优于 AES 软件实现。
3. **安全边际高**：ChaCha20 是流密码，没有 AES 的代数结构弱点；Poly1305 提供一次一密的 MAC 安全性。
4. **RustCrypto 生态系统**：`chacha20poly1305` crate 是纯 Rust 实现，无 C 依赖，无潜在的内存安全问题。

### Nonce 生成方案选择

每个包使用 96 位 nonce，由 `make_nonce(conn_id: u64, seq: u32)` 生成。

```
nonce[0..8]  = conn_id.to_le_bytes()
nonce[8..12] = seq.to_le_bytes()
```

**为什么不用随机 nonce？**

| 方案 | 优点 | 缺点 |
|------|------|------|
| **确定性 nonce**（本方案） | 无需额外传输、无熵消耗、无碰撞风险 | 需保证 seq 在流内不重复 |
| **随机 nonce** | 无需管理序列号 | 需传输额外 12 字节（包变大）、熵耗尽风险、生日碰撞（2^48 包后才安全） |

**安全性论证**：

- ChaCha20-Poly1305 对 nonce 重复的要求是**同一密钥下 nonce 必须唯一**
- `conn_id` 是 64 位随机数，全局唯一
- `seq` 在每个流内单调递增，不会重复
- 因此 `(conn_id, seq)` 对在通信中全局唯一，nonce 不会重复
- 相比随机 nonce，确定性方案避免了生日碰撞问题

---

## PSK 管理

### 生成

PSK（Pre-Shared Key）是 32 字节（256 位）的随机密钥。

```rust
pub fn generate_psk() -> Psk {
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    key
}
```

- 使用操作系统提供的加密安全随机数生成器（`OsRng`）
- 输出为 hex 编码的 64 字符字符串（供人类使用）和 32 字节原始密钥

### 分发

PSK 的分发必须通过安全的带外（out-of-band）方式进行：

```
服务端 (daemon)                        客户端 (ctl/gui)
    │                                      │
    │ 1. 首次运行自动生成 PSK               │
    │    保存到 /etc/lan-link/psk           │
    │    打印到终端：PSK=xxxx...            │
    │                                      │
    │ 2. 安全通道传递                       │
    │    ● SSH 复制文件                     │
    │    ● 二维码扫描（同一局域网）           │
    │    ● 安全消息传递（Signal/Telegram）   │
    │    ● USB 介质传递                     │
    │    ● 直接在目标机器终端查看并手动输入    │
    │                                      │
    │ 3. 客户端配置                         │
    │    ● LAN_LINK_PSK 环境变量            │
    │    ● --psk 命令行参数                 │
    │    ● GUI 配置面板                     │
```

### 存储

**服务端**：
- 路径：`/etc/lan-link/psk`
- 格式：64 字符 hex 编码（不含换行）
- 权限：建议 `chmod 600`（仅 root 可读）
- 首次运行自动生成，也支持手动写入

**客户端 (CLI)**：
- 不持久化存储 PSK
- 通过环境变量 `LAN_LINK_PSK` 或 `--psk` 参数传入
- 退出后密钥从内存释放（未专门使用 `mlock` 锁定内存页）

**客户端 (GUI)**：
- PSK 保存在用户配置文件中
- Linux: `~/.config/lan-link/gui-config.json`
- ⚠️ 配置文件是 JSON 明文，不推荐 PSK 长期存储在 GUI 配置中
- Windows 支持（未来计划）: `%APPDATA%/lan-link/gui-config.json`

### 轮换建议

定期轮换 PSK 是重要的安全实践：

```bash
# 1. 在目标机器上生成新 PSK
openssl rand -hex 32 | sudo tee /etc/lan-link/psk
sudo systemctl restart lan-linkd

# 2. 获取新 PSK
sudo cat /etc/lan-link/psk

# 3. 更新所有客户端
export LAN_LINK_PSK=<new_psk>
```

**轮换频率建议**：

| 使用场景 | 建议轮换频率 |
|---------|-------------|
| 家庭局域网，信任度高 | 每 6-12 个月 |
| 企业内网，中等信任 | 每月或按季度 |
| 高安全环境 | 每周或在每次使用后 |
| PSK 泄露后 | 立即轮换 |

---

## 已知安全限制

### 1. 无 PFS（前向安全性）

- **问题**：如果 PSK 泄露，攻击者可以解密所有历史通信记录
- **原因**：未使用 ECDH 密钥协商，仅依赖对称加密
- **缓解**：
  - 定期轮换 PSK
  - 控制历史日志保留期
  - 传输敏感数据时配合其他加密层（如 VPN）

### 2. 初始握手未加密

- **问题**：SYN 和 SYN-ACK 包以明文传输
- **原因**：握手阶段客户端尚无加密上下文
- **风险**：攻击者可以观察到 `conn_id` 和通信行为
- **缓解**：
  - `conn_id` 仅用于关联同一连接，不包含敏感信息
  - Hello 包之后的所有通信完全加密
  - 正确的 PSK 是建立加密通道的前提

### 3. 无 PSK 轮换协议

- **问题**：更换 PSK 需要手动更新所有客户端
- **原因**：未实现内置的密钥轮换机制
- **缓解**：通过带外方式（SSH、安全消息）分发新密钥

### 4. 无认证日志审计

- **问题**：无法区分不同用户的连接（所有客户端使用同一个 PSK）
- **原因**：PSK 是唯一的认证因子，不绑定用户身份
- **缓解**：在 daemon 日志中记录连接来源 IP 和 `conn_id` 以区分不同客户端

### 5. 内存中的 PSK 未被锁定

- **问题**：PSK 可能在进程内存转储中被泄露
- **原因**：未使用 `mlock()` 防止内存页被交换到磁盘
- **缓解**：确保核心转储被禁用（`ulimit -c 0`），或使用全盘加密

### 6. 无速率限制

- **问题**：攻击者可发送大量 UDP 包进行暴力破解尝试
- **原因**：当前未实现连接频率限制
- **缓解**：
  - 在网络层（iptables/nftables）限制源 IP 的 UDP 包速率
  - 使用 fail2ban 等工具自动封禁暴力破解 IP

### 7. 无 DoS 防护

- **问题**：UDP 协议特性可能导致放大攻击
- **原因**：UDP 无连接特性使攻击者可以伪造源 IP
- **缓解**：
  - 防火墙限制允许访问 daemon 端口的 IP 范围
  - 在企业网络中使用 VLAN 隔离
  - 考虑使用 WireGuard VPN 建立安全隧道（UDP over UDP 需要注意 MTU）

---

## 安全最佳实践

### 网络层面

```bash
# 1. 防火墙限制源 IP（仅允许管理子网）
sudo ufw allow from 192.168.1.0/24 to any port 9876 proto udp

# 2. 使用 iptables 限制连接速率
sudo iptables -A INPUT -p udp --dport 9876 -m state --state NEW \
  -m recent --set --name DDOS --rsource
sudo iptables -A INPUT -p udp --dport 9876 -m state --state NEW \
  -m recent --update --seconds 60 --hitcount 10 --name DDOS --rsource \
  -j DROP

# 3. 禁用核心转储（防止 PSK 泄露到磁盘）
ulimit -c 0

# 4. 不在公网暴露 daemon 端口，使用 VPN 隧道
```

### 系统层面

```bash
# 1. PSK 文件权限
sudo chmod 600 /etc/lan-link/psk
sudo chown root:root /etc/lan-link/psk

# 2. 配置目录权限
sudo chmod 700 /etc/lan-link

# 3. 监控异常连接尝试
sudo journalctl -u lan-linkd | grep -E "decrypt failed|Bad header|Bad control"

# 4. 禁用 uinput 设备可发现性（可选）
echo 'blacklist uinput' | sudo tee /etc/modprobe.d/blacklist-uinput.conf
```

### 开发层面

1. **所有加密解密必须通过 `protocol::crypto` 模块**，不得绕过
2. **NativeCmd 优先于 Exec**：使用结构化参数避免 shell 注入
3. **包验证**：始终验证解密成功后再反序列化
4. **日志安全**：不在日志中打印 PSK 或敏感数据
5. **最小依赖**：加密模块仅依赖经过安全审计的 `chacha20poly1305` crate
6. **Fuzzing**：建议对 `PacketHeader::decode()` 和 `crypto::decrypt()` 进行 fuzz 测试

### 审计清单

- [ ] PSK 是否使用 32 字节随机密钥？
- [ ] PSK 文件权限是否为 600？
- [ ] 是否有防火墙规则限制 UDP 端口访问？
- [ ] 是否定期轮换 PSK？
- [ ] 是否禁用核心转储？
- [ ] 是否监控 daemon 日志中的异常事件？
- [ ] 是否在受信任的局域网内使用？
- [ ] 客户端是否通过环境变量而非命令行传递 PSK？（防止被 `ps aux` 看到）
