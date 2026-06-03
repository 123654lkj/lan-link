//! lan-link-protocol — 协议层核心库
//!
//! 所有 crate 共享的协议定义，包括：
//!
//! - **帧格式** ([`frame`]) — 38 字节定长 `PacketHeader`，支持 6 种包类型和 6 个逻辑流
//! - **加密** ([`crypto`]) — ChaCha20-Poly1305 AEAD 加密/解密，PSK 生成和 nonce 推导
//! - **可靠传输** ([`reliable`]) — 选择性重传 ARQ，32 包滑动窗口，200ms RTO
//! - **流复用** ([`stream`]) — 在单 UDP 连接上管理多逻辑流的序列号
//!
//! # 设计原则
//!
//! - **零依赖**：本 crate 不依赖任何项目内其他 crate，只依赖第三方库
//! - **纯数据**：不包含网络 IO 或异步运行时，仅做序列化/反序列化
//! - **错误宽容**：解析/解密失败返回 `Option`，不在本层 panic 或崩溃

pub mod frame;
pub mod crypto;
pub mod reliable;
pub mod stream;
