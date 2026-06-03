# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
### Fixed (第二轮)

- 修复 GUI 连接锁跨 async/await 长时间持有（死锁风险）
- 修复 shell stdin close 标志静默失效
- 修复 test 文件硬编码 Windows 绝对路径，改为自动推导
- 修复 CLI PSK fallback 不读 /etc/lan-link/psk
- 修复 journal --since/--follow 参数未实现
- 修复 Docker exec 不支持交互式容器（-it）
- 统一 GUI/CLI 连接超时不一致（3s→5s）
- 修复 GUI exec 线程 panic 静默吞掉
- 修复 build.rs 条件编译写反（Linux 输出 Windows linker args）
- 修复 uinput EV_SYN 注册为无效值 0x00→0x11
- 修复 stream.rs seq 绕回后重复（saturating_add）
- 修复 top 显示 CPU/MEM 全为 0.0
- 修复 portscan 超时无网络可达提示
- 修复 Grep line_number 参数无效
- 修复 top 命令行截断
- 清理 frame.rs ExecOutput 死代码
- 修复 build_header 字段未初始化
- 修复 theme 每帧重复应用

### Changed

- Reasonix 提示词优化为外科手术级修复模式（禁止 reformat/refactor 等附带改动）

### Added

- 协议版本号 — 定义 `PROTOCOL_VERSION` 常量，Hello 握手时校验版本兼容性
- portscan 并发化，使用 `std::thread::scope` 限制最多 100 并发线程
- cmd_sed 路径安全检查，拒绝包含 `..` 的路径
- cmd_grep 递归搜索统一输出 `文件名:行号:内容`
- cmd_tail `follow`/`follow_secs` 参数传递和实现
- Windows 支持文档说明，标注当前仅支持 Linux，Windows 为未来计划
- 清理 lib.rs 测试骨架，替换为模块级 doc comment

### Security

- 移除硬编码示例 PSK，GUI 连接时引导用户自动生成随机密钥
- 应用层速率限制：SYN 限速 5/60s/IP、命令限速 30/60s/IP、连接数上限 100
- ShellExec/Exec RCE 安全警告文档，明确标注「绝不可暴露到公网」

### Fixed

- 修复 `reliable.rs` ACK 绕回逻辑：删除误标记分支，超出位图范围的 seq 保持 pending 等待重传
- 修复 daemon 连接无状态校验：Data/Heartbeat 检查 `ConnState::Established`，不同 peer 的 SYN 拒绝覆盖
- 修复 ctl 交互 shell stdin 丢换行符（`read_stdin_line` 发送时保留 `\n`）
- 修复 ctl `push` 命令 ACK 超时不重发（新增 3 次递增间隔重试）
- 修复 daemon `tail` 的 `follow_secs` 参数被忽略（用 `timeout` 包装）
- 修复 `win.rs` `SendInput` 返回值未处理（失败时 `log::warn`）
- 修复 `linux.rs` uinput EV_SYN 配置静默失败
- 修复 `linux.rs` Modifiers 修饰键信息丢失（添加 modifier 状态跟踪）
- 修复 `network.rs` portscan 多线程写 String 无锁（Mutex 保护）
- 修复 `mod.rs` Mount 命令被 `_ => debug!` 静默忽略
- 修复 Clap 命令重复 `#[command(about = ...)]` 行
- 修复 `fs.rs` `cmd_mkdir` 改用 `std::fs::create_dir_all`
- 修复 network.rs 重复函数定义导致编译错误
- 降级所有 crate edition 从 2024 到 2021（兼容性修复）
- 转换 grep_walk 从递归到迭代（避免栈溢出）
- 修复 cmd_top_snapshot 返回类型
- 修复 cmd_dmesg — 限制 /dev/kmsg 读取 64KB 并设置 2s 超时
- 移除 ReliableSender::send 多余的 conn_id 参数
- 修复 cmd_pkill signal 参数顺序（signal 放在 -f 之前）

### Changed

- `portscan` CLI 默认超时从 100ms 改为 500ms
- 移除 cmd_watch_fn 未使用的 `_interval_secs` 参数
- 移除 handle_control 未使用的 `_connections` 参数
- 移除 Connection 冗余的 psk 字段及其参数

### Documentation

- 为所有 crate 添加模块级 doc comment
- 重写 security.md — 完整安全模型
- 重写 architecture.md — 完整架构设计
- 重写 getting-started.md — 快速入门完整版
- 重写 variables.md — 变量命名和使用规范
- 新增 code-structure.md — 代码结构详细说明
- 重写 README.md — 完整项目文档

## [0.1.0] - 2025-04-06

### Added

- Initial release: lan-link source from 仙兔儿
- 基础 UDP 加密通信框架
- 自定义协议（38 字节包头、ChaCha20-Poly1305、选择性重传 ARQ）
- 50+ 原生命令（文件系统、系统信息、网络工具、服务管理、Docker）
- CLI 客户端（lan-linkctl）和守护进程（lan-linkd）
- eframe/egui 跨平台 GUI 客户端
- mDNS 服务发现（预留实现）
- 跨平台输入注入（Linux uinput）
