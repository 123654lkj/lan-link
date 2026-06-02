from pathlib import Path

doc_dir = Path(r"G:\codex-AI-tools\lan-link\docs")
doc_dir.mkdir(exist_ok=True)

# ===== PROTOCOL MAP =====
protocol_map = """\
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
"""

(doc_dir / "protocol-map.md").write_text(protocol_map, encoding="utf-8")

# ===== API REFERENCE =====
api_ref = """\
# lan-link API 参考

## Crate 依赖关系
```
daemon ──┬── protocol (frame, crypto, reliable, stream)
         ├── shell   (exec, ExecResult)
         ├── video   (VideoCapture, VideoEncoder, DxgiCapture, NvencEncoder)
         ├── input   (InputCapture, InputInjector, BorderWatcher, MonitorInfo)
         └── discovery (mDNS stub)

ctl ─────┬── protocol
         └── (独立CLI, 不依赖其他crate)
```

## protocol crate 导出
```
lan_link_protocol::
  frame::
    HEADER_SIZE: usize = 38
    MAX_PAYLOAD: usize = 1400
    MAX_PACKET: usize = 1454
    PacketType { Syn, SynAck, Ack, Data, Rst, Heartbeat }
    StreamId { Control=0, Video=1, AudioTx=2, AudioRx=3, Input=4, File=5 }
    Flags { RELIABLE, FRAGMENTED, ORDERED }
    PacketHeader { conn_id, pkt_type, flags, stream_id, seq, ack_seq, ack_bitmap, payload_len, nonce }
      ::encode(buf) ::decode(buf) -> Option<Self>
    ControlMsg enum (serde):
      Exec{id:u32, cmd:String}
      ExecOutput{id:u32, data:Vec<u8>, exit_code:Option<i32>}
      FilePush{id, path, size} / FileChunk{id, offset, data} / FileAck{id, offset}
      KeyEvent{down:bool, scancode:u16, vk:u16}
      MouseMove{dx:i16, dy:i16} / MouseButton{button:u8, down:bool} / MouseWheel{delta:i16}
      VideoStart{width, height, fps, bitrate_kbps} / VideoStop
      AudioStart{sample_rate, channels} / AudioStop
      Hello{version:u16, capabilities:Vec<String>} / HelloAck
  crypto::
    Psk = [u8; 32]
    generate_psk() -> Psk
    encrypt(key, nonce, plaintext) -> Vec<u8>
    decrypt(key, nonce, ciphertext) -> Option<Vec<u8>>
    make_nonce(conn_id:u64, seq:u32) -> [u8;12]
  reliable::
    ReliableSender::new(stream_id) -> Self
      ::send(conn_id, payload) -> Option<BytesMut>  // 队列满返回None
      ::on_ack(ack_seq, ack_bitmap) -> Vec<u32>      // 返回新确认的seq
      ::poll_retransmit() -> Vec<(u64,u16,u32,Vec<u8>)>
    ReliableReceiver::new(stream_id) -> Self
      ::deliver(seq, payload) -> Vec<Vec<u8>>        // 返回有序载荷
      ::ack_info() -> (u32, u32)
  stream::
    StreamMux::new() -> Self  // 预创建流0-5
      ::get(id) -> Option<&MuxStream>
      ::get_mut(id) -> Option<&mut MuxStream>
    MuxStream { id:u16, is_reliable:bool }
      ::next_seq() -> u32
```

## shell crate 导出
```
lan_link_shell::
  exec(cmd:&str, args:&[&str]) -> anyhow::Result<ExecResult>
  exec_with_input(cmd, args, stdin_data) -> anyhow::Result<ExecResult>
  ExecResult { exit_code:i32, stdout:String, stderr:String }
```

## input crate 导出
```
lan_link_input::
  KeyEvent { down:bool, scancode:u16, vk:u16, modifiers:Modifiers }
  MouseEvent::Move{dx,dy,absolute} | Button{button,down} | Wheel{delta,horizontal}
  MouseButton::Left|Right|Middle|X1|X2
  Modifiers { CTRL, ALT, SHIFT, WIN }
  MonitorInfo { index, name, x, y, width, height, is_primary }
  InputCapture trait: poll_keys(), poll_mouse(), cursor_pos(), monitors()
  InputInjector trait: inject_key(), inject_mouse(), set_cursor_pos()
  BorderWatcher::new(remote_monitor:u32)
    ::check(x, y, monitors) -> Option<bool>  // Some(true)=进入remote, Some(false)=离开
    ::is_on_remote() -> bool
  // Windows:
  WinInputCapture::new() ::register(hwnd)
  WinInputInjector::new()
  // Linux (stub):
  LinuxInputCapture / LinuxInputInjector
```

## video crate 导出
```
lan_link_video::
  VideoConfig { monitor_index, width, height, fps, bitrate_kbps, codec }
  VideoFrame { width, height, data, format, pts }
  EncodedPacket { data, keyframe, pts, dts }
  VideoCapture trait: capture() -> Option<VideoFrame>, dimensions()
  VideoEncoder trait: encode(frame) -> Option<EncodedPacket>, flush()
  DxgiCapture::new(monitor_index, width, height)
  NvencEncoder::new(config) / SoftwareEncoder::new(config)
```

## daemon crate 内部结构
```
lan-linkd (bin):
  main.rs:
    Args { port:u16, psk:Option<String>, discovery:bool }
    handle_packet(data, peer, connections, psk)  // 包分发
    handle_control(data, conn_id, peer, connections)  // 控制消息处理
  connection.rs:
    Connection { id:u64, peer:SocketAddr, psk:Psk, state:ConnState, mux:StreamMux, ... }
      ::build_syn(conn_id) / build_syn_ack(conn_id)
      ::build_data(conn_id, stream_id, seq, flags, payload)
      ::build_heartbeat(conn_id)
      ::generate_id() -> u64
    ConnState { Listening, SynSent, Established, Closed }
  discovery.rs: mDNS stub
```

## ctl crate 内部结构
```
lan-linkctl (bin):
  Cli { addr, psk, command }
  Command enum: Exec{cmd}, Push{local,remote}, Status, Video{width,height,fps}
```

## 编译依赖 (Cargo.toml)
```
workspace members: protocol, shell, video, input, daemon, ctl
(音频crate暂移除，因cpal→cmake→dlltool依赖问题)

关键外部依赖:
  protocol: chacha20poly1305, rand, bytes, serde, bincode, bitflags
  daemon:   tokio, clap, anyhow, hex
  ctl:      tokio, clap, anyhow, hex
  shell:    tokio, anyhow
  input:    bitflags, windows (Win32 UI/GDI)
  video:    (待加DXGI/NVENC依赖)
```
"""

(doc_dir / "api-reference.md").write_text(api_ref, encoding="utf-8")

print("docs created")
