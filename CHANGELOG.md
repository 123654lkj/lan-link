# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- 协议版本号 — 定义 `PROTOCOL_VERSION` 常量，Hello 握手时校验版本兼容性
- portscan 并发化，使用 `std::thread::scope` 限制最多 100 并发线程
- cmd_sed 路径安全检查，拒绝包含 `..` 的路径
- cmd_grep 递归搜索统一输出 `文件名:行号:内容`
- cmd_tail `follow`/`follow_secs` 参数传递和实现
- Windows 支持文档说明，标注当前仅支持 Linux，Windows 为未来计划
- 清理 lib.rs 测试骨架，替换为模块级 doc comment

### Fixed

- 修复 network.rs 重复函数定义导致编译错误
- 降级所有 crate edition 从 2024 到 2021（兼容性修复）
- 转换 grep_walk 从递归到迭代（避免栈溢出）
- 修复 cmd_top_snapshot 返回类型
- 修复 cmd_dmesg — 限制 /dev/kmsg 读取 64KB 并设置 2s 超时
- 移除 ReliableSender::send 多余的 conn_id 参数
- 修复 cmd_pkill signal 参数顺序（signal 放在 -f 之前）
- P0 严重 bug + P1 性能优化 + P2 重构 + P3 文档

### Changed

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
