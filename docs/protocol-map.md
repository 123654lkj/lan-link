# lan-link 协议地图

## 分层架构
```
+--------------------------------------------------+
| 应用层: daemon (lan-linkd) / ctl (lan-linkctl)    |
|   - Shell执行  - 视频流  - 输入注入  - 文件传输    |
+--------------------------------------------------+
| 可靠传输层: protocol::reliable                     |
|   - 选择性重传ARQ  - 滑动窗口(32)  - 乱序缓冲      |
|   仅用于: Control(0), Input(4), File(5) 流         |
+--------------------------------------------------+
| 加密层: protocol::crypto                           |
|   - ChaCha20-Poly1305 AEAD  - 每包独立nonce        |
+--------------------------------------------------+
| 帧层: protocol::frame                              |
|   - 38字节定长头  - 流多路复用  - 最大载荷1400B    |
+--------------------------------------------------+
| 传输层: UDP                                       |
+--------------------------------------------------+
```

## 包格式 (38字节头)
```
Offset  Size  Field         说明
[0..8]   8    conn_id       u64 LE, 连接标识
[8]      1    pkt_type      SYN=0 SYN-ACK=1 ACK=2 DATA=3 RST=4 HEARTBEAT=5
[9]      1    flags         bit0=reliable bit1=fragmented bit2=ordered
[10..12] 2    stream_id     u16 LE, 0=control 1=video 2=audio_tx 3=audio_rx 4=input 5=file
[12..16] 4    seq           u32 LE, 发送序列号
[16..20] 4    ack_seq       u32 LE, 捎带确认号
[20..24] 4    ack_bitmap    u32 LE, 选择性ACK位图(32包窗口)
[24..26] 2    payload_len   u16 LE, 加密载荷长度
[26..38] 12   nonce         [u8;12], 加密随机数
[38..]   var  encrypted     加密载荷 + 16B Poly1305标签
```

## 连接握手
```
Client                           Server
  |--- SYN (conn_id) ------------>|
  |<-- SYN-ACK (conn_id) ---------|
  |--- ACK (空Data包) ----------->|   连接建立
  |--- HEARTBEAT (每5s) -------->|
```

## 流类型
| ID | 名称    | 可靠? | 方向     | 说明                  |
|----|---------|-------|----------|----------------------|
| 0  | control | yes   | 双向     | 控制消息(Hello/Exec等)|
| 1  | video   | no    | 单向     | H264/H265视频帧       |
| 2  | audio_tx| no    | 单向     | Opus音频(主机->远端)  |
| 3  | audio_rx| no    | 单向     | Opus音频(远端->主机)  |
| 4  | input   | yes   | 单向     | 键鼠事件              |
| 5  | file    | yes   | 双向     | 文件分块传输          |

## ControlMsg 类型 (流0上的bincode序列化消息)
- Hello { version, capabilities } / HelloAck
- Exec { id, cmd } / ExecOutput { id, data, exit_code }
- FilePush { id, path, size } / FileChunk / FileAck
- KeyEvent { down, scancode, vk }
- MouseMove { dx, dy } / MouseButton / MouseWheel
- VideoStart { width, height, fps, bitrate_kbps } / VideoStop
- AudioStart { sample_rate, channels } / AudioStop

## KVM 工作流程
```
Win(主控)                           Linux(被控)
  1. 枚举显示器, 选第N块作为remote ---->  VideoStart
  2. 启动DXGI捕获该显示器 ----------->  video流(H264/H265)
  3. 显示到全屏窗口
  4. BorderWatcher检测鼠标进出 ------->  input流(键鼠)
  5. Ctrl+Alt+Del 始终本地处理(不转发)
```
