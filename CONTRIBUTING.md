# Contributing to lan-link

感谢您对本项目的关注！我们欢迎各种形式的贡献——报告问题、提交功能请求、改进文档、提交代码修复或新功能。

## 目录

- [开发环境要求](#开发环境要求)
- [如何构建](#如何构建)
- [代码规范](#代码规范)
- [提交规范](#提交规范)
- [PR 流程](#pr-流程)
- [Issue 模板说明](#issue-模板说明)

## 开发环境要求

- **Rust**：edition 2021，toolchain stable（最低支持 MSRV 1.70）
- **系统**：目前仅支持 Linux（daemon 和输入注入），CLI 和 GUI 客户端跨平台
- **额外依赖**：
  - `libxdo-dev`（GUI 客户端）
  - `libevdev-dev`、`libudev-dev`（输入注入）
  - `pkg-config`、`cmake`（部分构建脚本）

## 如何构建

```bash
# 完整构建（所有 crate）
cargo build

# 发布模式
cargo build --release

# 仅构建特定 crate
cargo build -p lan-link-protocol
cargo build -p lan-linkd
cargo build -p lan-linkctl

# 运行测试
cargo test --workspace

# 运行 clippy
cargo clippy --workspace -- -D warnings

# 格式化代码
cargo fmt
```

### 项目结构

```
lan-link/
├── crates/
│   ├── protocol/    # 协议核心（帧格式、加密、可靠传输、流复用）
│   ├── daemon/      # 守护进程（lan-linkd）
│   ├── ctl/         # CLI 客户端（lan-linkctl）
│   ├── gui/         # GUI 客户端（lan-link-gui）
│   ├── shell/       # 命令执行引擎
│   ├── input/       # 输入事件捕获与注入
│   └── video/       # 视频捕获与编码（预留）
└── docs/            # 项目文档
```

## 代码规范

- **Rust edition**：2021
- **格式化**：使用 `rustfmt`（运行 `cargo fmt`）
- **Lint**：通过 `cargo clippy` 检查，不允许 warnings
- **模块文档**：每个 crate 和公开模块顶部必须包含 `//!` 文档注释
- **命名约定**：
  - 类型/枚举/结构体/特征：PascalCase
  - 函数/方法/变量：snake_case
  - 常量：SCREAMING_SNAKE_CASE
- **错误处理**：优先使用 `anyhow::Result`，库 crate 使用自定义错误类型
- **导入顺序**：`std` → 外部 crate → `crate`/`self`/`super`

## 提交规范

本项目遵循 [Conventional Commits](https://www.conventionalcommits.org/) 规范：

```
<type>: <description>

[optional body]
[optional footer(s)]
```

### 类型

| 类型 | 说明 |
|------|------|
| `feat` | 新功能 |
| `fix` | Bug 修复 |
| `docs` | 文档变更 |
| `style` | 代码格式（不影响功能） |
| `refactor` | 重构（既不修复 bug 也不添加功能） |
| `perf` | 性能优化 |
| `test` | 测试相关 |
| `build` | 构建系统/依赖变更 |
| `ci` | CI 配置变更 |
| `chore` | 杂项（维护任务） |

### 示例

```
feat: 添加 mDNS 服务发现功能
fix: 修复连接超时未正确清理的问题
docs: 更新 API 参考文档
refactor: 重构连接状态机
```

## PR 流程

1. **Fork** 本仓库并创建您的分支：`git checkout -b feat/my-feature`
2. **提交** 符合规范的 commit
3. **运行检查**：
   ```bash
   cargo build --workspace
   cargo test --workspace
   cargo clippy --workspace -- -D warnings
   cargo fmt --check
   ```
4. **创建 PR** 到 `main` 分支，填写 PR 模板
5. 等待审核，根据反馈进行修改
6. 合并后删除分支

### PR 指南

- 每个 PR 应聚焦一个变更，避免混合多个不相关的修改
- 包含必要的测试覆盖
- 更新相关的文档
- 如果 PR 修复了一个 issue，请在描述中引用（`Closes #123`）

## Issue 模板说明

本项目提供两种 Issue 模板：

1. **Bug Report**：用于报告错误或异常行为
   - 描述问题、复现步骤、期望行为和实际行为
   - 附上环境信息和相关日志

2. **Feature Request**：用于请求新功能或改进
   - 描述当前痛点、期望的解决方案
   - 提供备选方案（可选）

请选择合适的模板提交 Issue，以便我们更快地理解并处理您的问题。
