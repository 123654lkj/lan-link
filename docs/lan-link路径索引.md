# lan-link 路径索引

## 核心源码

| 文件 | 绝对路径 | 说明 |
|------|----------|------|
| 客户端 Python | G:\codex-AI-tools\lan-link\client_win.py | 协议客户端 + 输入捕获 |
| GUI 客户端 | G:\codex-AI-tools\lan-link\client_gui.py | tkinter GUI |
| Protocol | G:\codex-AI-tools\lan-link\crates\protocol\src\frame.rs | 帧格式 + ControlMsg |
| Crypto | G:\codex-AI-tools\lan-link\crates\protocol\src\crypto.rs | ChaCha20-Poly1305 |
| Reliable | G:\codex-AI-tools\lan-link\crates\protocol\src\reliable.rs | 可靠传输 |
| Daemon | G:\codex-AI-tools\lan-link\crates\daemon\src\main.rs | 服务端主循环 |
| Connection | G:\codex-AI-tools\lan-link\crates\daemon\src\connection.rs | 连接管理 |
| Shell | G:\codex-AI-tools\lan-link\crates\shell\src\lib.rs | 命令执行 |
| Input Linux | G:\codex-AI-tools\lan-link\crates\input\src\linux.rs | Linux 输入注入 |
| Input Lib | G:\codex-AI-tools\lan-link\crates\input\src\lib.rs | 输入数据结构 |
| CTL | G:\codex-AI-tools\lan-link\crates\ctl\src\main.rs | Rust CLI 客户端 |
| GUI Rust | G:\codex-AI-tools\lan-link\crates\gui\src\main.rs | eframe GUI (预留) |

## 构建配置

| 文件 | 绝对路径 | 说明 |
|------|----------|------|
| Workspace | G:\codex-AI-tools\lan-link\Cargo.toml | Rust workspace |
| PyInstaller | G:\codex-AI-tools\lan-link\lan-link-gui.spec | GUI 打包 |
| VBS 启动器 | G:\codex-AI-tools\lan-link\lan-link-gui.vbs | 无黑框启动 |

## 部署

| 文件 | 绝对路径 | 说明 |
|------|----------|------|
| systemd unit | G:\codex-AI-tools\lan-link\deploy\lan-linkd.service | 团子服务配置 |
| PSK | /etc/lan-link/psk (团子) | 预共享密钥 |
| 二进制 | /opt/lan-link/target/release/lan-linkd (团子) | 服务端二进制 |

## 测试

| 文件 | 绝对路径 | 说明 |
|------|----------|------|
| Streaming | G:\codex-AI-tools\lan-link\tests\test_streaming.py | 流式 exec 测试 |
| Stdin | G:\codex-AI-tools\lan-link\tests\test_stdin.py | stdin 测试 |
| Signal | G:\codex-AI-tools\lan-link\tests\test_signal.py | SIGTERM 测试 |

## 文档

| 文件 | 绝对路径 | 说明 |
|------|----------|------|
| 设计文档 | G:\codex-AI-tools\lan-link\docs\ll-design.md | 架构设计 |
| 协议地图 | G:\codex-AI-tools\lan-link\docs\protocol-map.md | 协议说明 |
| API 参考 | G:\codex-AI-tools\lan-link\docs\api-reference.md | API 文档 |
| GUI 文档 | G:\codex-AI-tools\lan-link\docs\client-gui.md | GUI 说明 |

## 可执行文件

| 文件 | 绝对路径 | 说明 |
|------|----------|------|
| GUI exe | G:\codex-AI-tools\lan-link\dist\lan-link-gui.exe | 打包后的 GUI |

