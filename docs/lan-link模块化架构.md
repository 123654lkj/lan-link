# lan-link 模块化架构

## 架构总览

`
+------------------+          UDP (ChaCha20-Poly1305)          +------------------+
|   Windows Client | <--------------------------------------> |   Linux Daemon   |
|                  |                                          |                  |
| +--------------+ |          +------------------+            | +--------------+ |
| | client_win.py| |          |   protocol       |            | |   daemon     | |
| | (Python)     | | <------> |   (Rust crate)   | <--------> | |   (Rust)     | |
| |              | |          |   - frame        |            | |   - main     | |
| | +---------+  | |          |   - crypto       |            | |   - conn     | |
| | | GUI     |  | |          |   - reliable     |            | |              | |
| | | (tkinter)| |          |   - stream       |            | +--------------+ |
| +--------------+ |          +------------------+            | +--------------+ |
| +--------------+ |                                          | | shell crate  | |
| | lan-linkctl |  |                                          | | (command exec)|
| | (Rust CLI)  | |                                          | +--------------+ |
| +--------------+ |                                          | +--------------+ |
+------------------+                                          | | input crate  | |
                                                              | | (key/mouse)  | |
                                                              | +--------------+ |
                                                              +------------------+
`

## 模块职责

### protocol (协议层)

**职责**: 定义所有通信协议相关的类型和编解码。

**对外接口**:
- PacketHeader: 帧 header 编解码
- ControlMsg: 控制消息枚举
- crypto::encrypt/decrypt: 加密解密
- crypto::make_nonce: Nonce 派生
- crypto::generate_psk: PSK 生成
- 
eliable::StreamMux: 流式复用器

**被引用**: daemon, ctl, gui, client_win.py

### shell (Shell 引擎)

**职责**: 跨平台命令执行。

**对外接口**:
- exec(cmd, args) -> ExecResult: 一次性执行
- exec_with_input(cmd, args, stdin_data) -> ExecResult: 带输入执行
- StreamingExec::spawn(cmd): 创建流式执行
- StreamingExec::try_poll_chunk() -> Option<StreamChunk>: 轮询输出
- StreamingExec::try_wait() -> Option<Option<i32>>: 非阻塞等待
- StreamingExec::write_stdin(data, close): 写入 stdin
- StreamingExec::kill(): 终止进程

**被引用**: daemon

### input (输入引擎)

**职责**: 跨平台键鼠捕获和注入。

**对外接口**:
- KeyEvent, MouseEvent, Modifiers: 数据结构
- InputCapture trait: 输入捕获接口
- InputInjector trait: 输入注入接口
- BorderWatcher: 屏幕边界检测
- MonitorInfo: 显示器信息

**平台实现**:
- Linux: evdev 捕获 + uinput 注入 + DRM 显示器
- Windows: (预留)

**被引用**: daemon

### daemon (服务端)

**职责**: UDP 服务端，处理连接、命令执行、输入注入。

**主要流程**:
1. UDP 接收循环 (tokio timeout 100ms)
2. SYN -> 创建 Connection -> 回复 SYN-ACK
3. DATA (Hello) -> 建立连接
4. DATA (ExecStarted) -> 调用 shell crate -> 流式转发 chunk
5. Heartbeat -> 超时清理连接

**被引用**: 无 (顶层二进制)

### client_win.py (Python 客户端)

**职责**: Windows 端协议客户端。

**主要功能**:
- LanLinkClient: 连接、exec、键鼠发送
- 
un_input_capture(): Windows 全局键鼠钩子
- Config: 配置文件管理
- Log: 文件日志 (rotating)

**被引用**: client_gui.py

### client_gui.py (tkinter GUI)

**职责**: 桌面 GUI 客户端。

**功能**: 连接管理、命令输入、输出显示、Tab 补全

**引用**: client_win.py (LanLinkClient)

