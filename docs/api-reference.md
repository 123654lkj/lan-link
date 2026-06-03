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
