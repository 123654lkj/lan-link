//! lan-linkctl — 局域网远程管理 CLI 客户端
//!
//! 通过 UDP 加密通道向 lan-linkd 守护进程发送命令。
//! 支持 50+ 管理子命令，涵盖文件操作、系统管理、网络工具、服务管理、Docker 等。
//!
//! # 工作模式
//!
//! - **NativeCmd**（推荐）— 结构化命令，在 daemon 端用 Rust 直接执行
//! - **Exec** — 流式 shell 执行，支持实时 stdout/stderr 输出
//! - **Iexec/Shell** — 交互式执行，支持 stdin 双向通道
//! - **Push/Pull** — 文件传输，块确认机制，带进度显示
//!
//! # 连接流程
//!
//! 1. 创建随机 `conn_id`，发送 SYN 包
//! 2. 等待 daemon 回复 SYN-ACK
//! 3. 发送加密 Hello 协商能力
//! 4. 发送命令并流式接收输出

use clap::{Parser, Subcommand};
use lan_link_protocol::crypto::{self, Psk};
use lan_link_protocol::frame::{PacketHeader, PacketType, Flags, StreamId, HEADER_SIZE, ControlMsg, NativeCmdType, ServiceActionType, PkgActionType, DockerActionType, CrontabActionType};
use std::io::{self, Write};
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UdpSocket;
use tracing::{info, warn};

const CHUNK_SIZE: usize = 1024;

/// Resolve PSK from --psk argument or LAN_LINK_PSK env var; exit if neither is set.
fn resolve_psk(psk_arg: &Option<String>) -> String {
    if let Some(hex) = psk_arg {
        return hex.clone();
    }
    if let Ok(hex) = std::env::var("LAN_LINK_PSK") {
        return hex;
    }
    // Fallback: try reading from default daemon PSK file
    if let Ok(hex) = std::fs::read_to_string("/etc/lan-link/psk") {
        let trimmed = hex.trim().to_string();
        if !trimmed.is_empty() {
            return trimmed;
        }
    }
    eprintln!("错误: 未设置 PSK。请通过 --psk 参数、LAN_LINK_PSK 环境变量或 /etc/lan-link/psk 文件提供 32 字节 hex 密钥");
    std::process::exit(1);
}

#[derive(Parser, Debug)]
#[command(name = "lan-linkctl", version, about = "LAN link 远程管理客户端")]
struct Cli {
    #[arg(short, long, global = true, help = "show INFO level logs")]
    verbose: bool,
    #[arg(short, long, default_value = "192.168.31.244:9876", help = "目标 daemon 地址 host:port")]
    addr: String,
    #[arg(short, long, help = "32 字节 PSK hex 字符串（默认从 LAN_LINK_PSK 环境变量读取）")]
    psk: Option<String>,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    #[command(about = "远程执行命令，等待完成后输出")]
    Exec {
        #[arg(allow_hyphen_values = true, num_args = 1.., help = "要执行的命令")]
        cmd: Vec<String>,
        #[arg(long, default_value = "120")] timeout: u64,
    },
    #[command(about = "交互式执行，支持 stdin 双向通道")]
    Iexec {
        #[arg(allow_hyphen_values = true, num_args = 1.., help = "要执行的命令")]
        cmd: Vec<String>,
        #[arg(long, default_value = "30")] timeout: u64,
    },
    #[command(about = "启动交互式 shell 会话")]
    Shell { #[arg(long, default_value = "300")] timeout: u64 },
    #[command(about = "从文件批量执行命令，支持超时和错误统计")]
    Batch {
        file: String,
        #[arg(long, default_value = "120")] timeout: u64,
        #[arg(short, long, help = "run commands in parallel")]
        parallel: bool,
        #[arg(short = 'j', long, default_value = "4", help = "parallel jobs count")]
        jobs: u32,
    },
    #[command(about = "每隔 N 秒重复执行指定命令")]
    Watch {
        #[arg(short = 'e', long = "every", default_value = "2")] interval_secs: u64,
        #[arg(allow_hyphen_values = true, num_args = 1..)] cmd: Vec<String>,
    },
    #[command(about = "上传本地文件到远端服务器")]
    Push { #[arg(short)] local: String, #[arg(short)] remote: String },
    #[command(about = "从远端服务器下载文件到本地")]
    Pull {
        #[arg(short)] remote: String,
        #[arg(short)] local: String,
        #[arg(long, default_value = "300", help = "timeout in seconds")]
        timeout: u64,
    },
    #[command(about = "列出远端目录内容")]
    Ls { #[arg(default_value = ".")] path: String, #[arg(short, long)] long: bool, #[arg(long)] all: bool },
    #[command(about = "显示远端文件内容")]
    Cat { path: String },
    #[command(about = "显示文件末尾 N 行或实时跟踪")]
    Tail { path: String, #[arg(short = 'n', long, default_value = "20")] lines: u32, #[arg(short, long)] follow: bool, #[arg(long)] follow_secs: u64 },
    #[command(about = "显示文件开头 N 行")]
    Head { path: String, #[arg(short = 'n', default_value = "10")] lines: u32 },
    #[command(about = "在远端查找文件")]
    Find { #[arg(default_value = ".")] path: String, #[arg(short = 'n', long)] name: Option<String>, #[arg(long)] type_: Option<String>, #[arg(long, default_value = "3")] maxdepth: u32 },
    #[command(about = "在远端搜索文件内容")]
    Grep { pattern: String, #[arg(default_value = ".")] path: String, #[arg(short, long)] recursive: bool, #[arg(long)] line_number: bool, #[arg(long)] count: bool },
    #[command(about = "计算远端目录磁盘用量")]
    Du { #[arg(default_value = ".")] path: String, #[arg(short, long)] summarize: bool, #[arg(long, default_value = "1")] maxdepth: u32 },
    #[command(about = "显示远端磁盘分区使用情况")]
    Df { #[arg(long)] human: bool, #[arg(long)] all: bool },
    #[command(about = "显示远端目录树（需安装 tree）")]
    Tree { #[arg(default_value = ".")] path: String, #[arg(long, default_value = "2")] depth: u32, #[arg(long)] dirs_only: bool },
    #[command(about = "显示远端文件元信息")]
    Stat { path: String },
    #[command(about = "在远端创建目录")]
    Mkdir { #[arg(short)] recursive: bool, paths: Vec<String> },
    #[command(about = "删除远端文件或目录")]
    Rm { #[arg(short)] recursive: bool, #[arg(short)] force: bool, paths: Vec<String> },
    #[command(about = "移动或重命名远端文件")]
    Mv { src: String, dest: String },
    #[command(about = "复制远端文件或目录")]
    Cp { #[arg(short)] recursive: bool, src: String, dest: String },
    #[command(about = "修改远端文件权限")]
    Chmod { mode: String, paths: Vec<String> },
    #[command(about = "修改远端文件所有者")]
    Chown { owner: String, paths: Vec<String> },
    #[command(about = "列出远端块设备")]
    Lsblk,
    #[command(about = "显示远端挂载点")]
    Mount,
    #[command(about = "比较两个远端文件差异")]
    Diff { file1: String, file2: String },
    #[command(about = "统计远端文件行数或字数")]
    Wc { #[arg(short, long)] lines: bool, #[arg(short, long)] words: bool, paths: Vec<String> },
    #[command(about = "列出远端进程")]
    Ps { #[arg(short, long)] full: bool, #[arg(long)] user: Option<String>, #[arg(long)] tree: bool },
    #[command(about = "终止远端进程")]
    Kill { pid: u32, #[arg(short, default_value = "15")] signal: u32 },
    #[command(about = "按名称查找远端进程")]
    Pgrep { name: String, #[arg(long)] full: bool, #[arg(long)] count: bool },
    #[command(about = "按名称终止远端进程")]
    Pkill { name: String, #[arg(short, default_value = "15")] signal: u32 },
    #[command(about = "循环显示远端进程排名")]
    Top { #[arg(long, default_value = "2")] interval_secs: u64, #[arg(long, default_value = "10")] iterations: u32 },
    #[command(about = "测试到 daemon 的 RTT 延迟")]
    Ping { #[arg(short, long, default_value = "5")] count: u32 },
    #[command(about = "查看远端网络连接")]
    Netstat { #[arg(short, long)] tcp: bool, #[arg(short, long)] udp: bool, #[arg(long)] numeric: bool, #[arg(long)] listening: bool },
    #[command(about = "查看远端网络接口")]
    Ip { #[arg(long)] addr: bool, #[arg(long)] route: bool, #[arg(long)] link: bool },
    #[command(about = "扫描远端 TCP 端口")]
    PortScan { #[arg(default_value = "127.0.0.1")] host: String, #[arg(long, default_value = "1")] start_port: u16, #[arg(long, default_value = "1024")] end_port: u16, #[arg(long, default_value = "500")] timeout_ms: u64 },
    #[command(about = "查看远端 ARP 表")]
    Arp,
    #[command(about = "解析远端 DNS 查询")]
    Dns { hostname: String, #[arg(long)] type_: Option<String> },
    #[command(about = "显示远端系统概要信息")]
    Info,
    #[command(about = "显示远端内核信息")]
    Uname { #[arg(short, long)] all: bool, #[arg(short = 'r', long)] release: bool, #[arg(short = 'm', long)] machine: bool },
    #[command(about = "显示远端系统运行时间")]
    Uptime,
    #[command(about = "显示远端主机名")]
    Hostname,
    #[command(about = "显示远端当前用户")]
    Whoami,
    #[command(about = "显示远端登录用户")]
    Who,
    #[command(about = "显示远端最近登录记录")]
    Last { #[arg(long, default_value = "20")] lines: u32 },
    #[command(about = "显示远端内存使用")]
    Free { #[arg(short, long)] human: bool },
    #[command(about = "显示远端 CPU 详细信息")]
    Cpu,
    #[command(about = "查看远端内核日志")]
    Dmesg { #[arg(long, default_value = "50")] lines: u32, #[arg(long)] level: Option<String> },
    #[command(about = "管理远端 systemd 服务")]
    Service { #[command(subcommand)] action: ServiceAction },
    #[command(about = "查询远端 systemd 日志")]
    Journal { #[arg(short, long)] unit: Option<String>, #[arg(short, long)] follow: bool, #[arg(long, default_value = "50")] lines: u32, #[arg(long)] priority: Option<String>, #[arg(long)] since: Option<String> },
    #[command(about = "管理远端 apt 软件包")]
    Pkg { #[command(subcommand)] action: PkgAction },
    #[command(about = "管理远端 Docker 容器")]
    Docker { #[command(subcommand)] action: DockerAction },
    #[command(about = "管理远端 crontab")]
    Crontab { #[command(subcommand)] action: CrontabAction },
    #[command(about = "查看远端防火墙规则")]
    Firewall { #[arg(long, default_value = "iptables")] backend: String },
    #[command(about = "检查远端 SSH 服务状态")]
    Ssh,
    #[command(about = "计算远端文件校验和")]
    Checksum { path: String, #[arg(long, default_value = "sha256")] algorithm: String },
    #[command(about = "向远端发送键盘事件（已弃用）")]
    Key { #[arg(long)] scancode: u16, #[arg(long)] vk: u16, #[arg(long)] release: bool },
    #[command(about = "向远端发送鼠标事件（已弃用）")]
    Mouse { #[command(subcommand)] action: MouseAction },
    #[command(about = "给正在运行的 exec 发送信号")]
    Signal { id: u32, #[arg(short, default_value = "15")] signal: u32 },
    #[command(about = "测试与 daemon 的连接状态")]
    Status,
    #[command(about = "显示本客户端版本号")]
    Version,
    #[command(about = "远端视频流控制（已弃用）")]
    Video { #[arg(long, default_value = "1920")] width: u16, #[arg(long, default_value = "1080")] height: u16, #[arg(long, default_value = "30")] fps: u8, #[arg(long)] stop: bool },
    #[command(about = "Write file to remote (supports append)")]
    WriteFile { path: String, data: String, #[arg(long)] append: bool },
    #[command(about = "Sed stream editor on remote")]
    Sed { path: String, pattern: String, replacement: String, #[arg(long)] global: bool, #[arg(long)] regex: bool },
    #[command(about = "Touch file/directory on remote")]
    Touch { path: String },

}

#[derive(Subcommand, Debug)]
enum ServiceAction {
    #[command(about = "列出服务（--active 运行中 / --failed 失败）")]
    List { #[arg(long)] active: bool, #[arg(long)] failed: bool },
    #[command(about = "查看指定服务状态")]
    Status { name: String },
    #[command(about = "启动指定服务")]
    Start { name: String },
    #[command(about = "停止指定服务")]
    Stop { name: String },
    #[command(about = "重启指定服务")]
    Restart { name: String },
    #[command(about = "重载指定服务配置")]
    Reload { name: String },
    #[command(about = "启用指定服务自启")]
    Enable { name: String },
    #[command(about = "禁用指定服务自启")]
    Disable { name: String },
}

#[derive(Subcommand, Debug)]
enum PkgAction {
    #[command(about = "列出已安装包")]
    List { #[arg(long)] installed: bool },
    #[command(about = "搜索软件包")]
    Search { query: String },
    #[command(about = "安装软件包")]
    Install { name: String },
    #[command(about = "卸载软件包")]
    Remove { name: String },
    #[command(about = "更新包索引")]
    Update,
    #[command(about = "升级所有已安装包")]
    Upgrade,
}

#[derive(Subcommand, Debug)]
enum DockerAction {
    #[command(about = "列出容器")]
    Ps { #[arg(short, long)] all: bool, #[arg(long)] running: bool },
    #[command(about = "查看容器日志")]
    Logs { name: String, #[arg(long, default_value = "100")] tail: u32, #[arg(short, long)] follow: bool },
    #[command(about = "容器资源统计")]
    Stats { #[arg(long, default_value = "2")] interval_secs: u64 },
    #[command(about = "在容器内执行命令")]
    Exec { container: String, #[arg(short, long)] interactive: bool, #[arg(allow_hyphen_values = true, num_args = 1..)] cmd: Vec<String> },
    #[command(about = "Docker 系统信息")]
    Info,
    #[command(about = "镜像列表")]
    Images,
    #[command(about = "删除容器")]
    Rm { container: String, #[arg(short, long)] force: bool },
    #[command(about = "控制容器（start/stop/pause 等）")]
    Control { container: String, action: String },
}

#[derive(Subcommand, Debug)]
enum CrontabAction {
    #[command(about = "查看 crontab")]
    List,
    #[command(about = "编辑 crontab")]
    Edit,
    #[command(about = "移除 crontab")]
    Remove,
}

#[derive(Subcommand, Debug)]
enum MouseAction {
    #[command(about = "移动鼠标（相对于当前位置，已弃用）")]
    Move { dx: i32, dy: i32 },
    #[command(about = "点击鼠标按键")]
    Click { #[arg(long, default_value = "left")] button: String, #[arg(long)] release: bool },
    #[command(about = "滚动鼠标滚轮")]
    Wheel { #[arg(default_value_t = 1)] delta: i16, #[arg(long)] horizontal: bool },
}

fn encode_control(conn_id: u64, psk: &Psk, seq: u32, msg: &ControlMsg) -> Vec<u8> {
    let payload = bincode::serialize(msg).expect("serialize");
    let nonce = crypto::make_nonce(conn_id, seq);
    let encrypted = crypto::encrypt(psk, &nonce, &payload);
    encode_packet(conn_id, PacketType::Data, StreamId::Control as u16, seq, Flags::RELIABLE, &encrypted)
}

fn encode_packet(conn_id: u64, pkt_type: PacketType, stream_id: u16, seq: u32, flags: Flags, payload: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(HEADER_SIZE + payload.len());
    PacketHeader { conn_id, pkt_type, flags, stream_id, seq, ack_seq: 0, ack_bitmap: 0, payload_len: payload.len() as u16, nonce: [0u8; 12] }.encode(&mut buf);
    buf.extend_from_slice(payload);
    buf
}

fn parse_packet(data: &[u8]) -> Option<(PacketHeader, &[u8])> {
    let mut cursor = std::io::Cursor::new(data);
    let hdr = PacketHeader::decode(&mut cursor)?;
    Some((hdr, &data[HEADER_SIZE..]))
}

struct Ctx {
    socket: UdpSocket,
    remote: SocketAddr,
    psk: Psk,
    conn_id: u64,
    seq: u32,
}

impl Ctx {
    async fn new(addr: &str, psk_hex: &str) -> anyhow::Result<Self> {
        let remote: SocketAddr = addr.parse()?;
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        let psk_bytes = hex::decode(psk_hex)?;
        anyhow::ensure!(psk_bytes.len() == 32, "PSK must be 32 bytes");
        let mut psk: Psk = [0u8; 32];
        psk.copy_from_slice(&psk_bytes);
        let conn_id: u64 = rand::random();
        let mut ctx = Self { socket, remote, psk, conn_id, seq: 0 };

        let syn = encode_packet(conn_id, PacketType::Syn, StreamId::Control as u16, 0, Flags::empty(), &[]);
        ctx.socket.send_to(&syn, remote).await?;
        info!("Sent SYN (conn={})", conn_id);

        let (hdr, _) = ctx.recv_parsed(Duration::from_secs(5)).await?;
        if hdr.pkt_type != PacketType::SynAck || hdr.conn_id != conn_id {
            anyhow::bail!("Expected SYN-ACK");
        }

        ctx.seq += 1;
        let hello = encode_control(conn_id, &ctx.psk, ctx.seq, &ControlMsg::Hello {
            version: lan_link_protocol::frame::PROTOCOL_VERSION, capabilities: vec!["exec".into(), "input".into()],
        });
        ctx.socket.send_to(&hello, remote).await?;

        if let Some(msg) = ctx.recv_control(Duration::from_secs(5)).await? {
            if let ControlMsg::HelloAck { version: v, capabilities: caps } = msg {
                info!("HelloAck v={} caps={:?}", v, caps);
            }
        }
        info!("Connected to {} (conn={})", remote, conn_id);
        Ok(ctx)
    }

    async fn send_control(&mut self, msg: &ControlMsg) -> anyhow::Result<()> {
        self.seq += 1;
        let pkt = encode_control(self.conn_id, &self.psk, self.seq, msg);
        self.socket.send_to(&pkt, self.remote).await?;
        Ok(())
    }

    async fn recv_parsed(&mut self, timeout: Duration) -> anyhow::Result<(PacketHeader, Vec<u8>)> {
        let mut buf = vec![0u8; lan_link_protocol::frame::MAX_PACKET];
        let (n, _) = tokio::time::timeout(timeout, self.socket.recv_from(&mut buf)).await??;
        let (hdr, ct) = parse_packet(&buf[..n]).ok_or_else(|| anyhow::anyhow!("Bad packet"))?;
        let end = std::cmp::min(hdr.payload_len as usize, ct.len());
        Ok((hdr, ct[..end].to_vec()))
    }

    async fn recv_control(&mut self, timeout: Duration) -> anyhow::Result<Option<ControlMsg>> {
        let (hdr, data) = self.recv_parsed(timeout).await?;
        if hdr.pkt_type != PacketType::Data || data.is_empty() { return Ok(None); }
        if let Some(plain) = crypto::decrypt(&self.psk, &hdr.nonce, &data) {
            return Ok(bincode::deserialize(&plain).ok());
        }
        Ok(None)
    }
}

async fn drain_exec(ctx: &mut Ctx, exec_id: u32, timeout_secs: u64) -> anyhow::Result<Option<i32>> {
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            warn!("Exec #{} timed out", exec_id);
            let _ = ctx.send_control(&ControlMsg::ExecSignal { id: exec_id, signo: 15 }).await;
            tokio::time::sleep(Duration::from_millis(500)).await;
            let _ = ctx.send_control(&ControlMsg::ExecSignal { id: exec_id, signo: 9 }).await;
            return Ok(None);
        }
        match ctx.recv_control(remaining.min(Duration::from_secs(2))).await {
            Ok(Some(msg)) => match msg {
                ControlMsg::ExecChunk { id, stream, data: chunk } if id == exec_id => {
                    let out: &mut dyn Write = if stream == 1 { &mut io::stderr() } else { &mut io::stdout() };
                    out.write_all(&chunk)?; out.flush()?;
                }
                ControlMsg::ExecDone { id, exit_code: code } if id == exec_id => { return Ok(code); }
                _ => {}
            }
            _ => {}
        }
    }
}

async fn drain_exec_interactive(ctx: &mut Ctx, exec_id: u32, timeout_secs: u64) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            warn!("Exec #{} timed out", exec_id);
            let _ = ctx.send_control(&ControlMsg::ExecSignal { id: exec_id, signo: 15 }).await;
            return Ok(());
        }
        tokio::select! {
            result = ctx.recv_control(remaining.min(Duration::from_secs(1))) => {
                if let Ok(Some(msg)) = result {
                    match msg {
                        ControlMsg::ExecChunk { id, stream, data: chunk } if id == exec_id => {
                            let out: &mut dyn Write = if stream == 1 { &mut io::stderr() } else { &mut io::stdout() };
                            out.write_all(&chunk)?; out.flush()?;
                        }
                        ControlMsg::ExecDone { id, exit_code: code } if id == exec_id => {
                            if let Some(c) = code { eprintln!(" [exit {}]", c); }
                            return Ok(());
                        }
                        _ => {}
                    }
                }
            }
            line_result = read_stdin_line() => {
                if let Ok(line) = line_result {
                    ctx.send_control(&ControlMsg::ExecStdin { id: exec_id, data: line.into_bytes(), close: false }).await?;
                } else {
                    ctx.send_control(&ControlMsg::ExecStdin { id: exec_id, data: vec![], close: true }).await?;
                }
            }
            _ = tokio::time::sleep(remaining) => { return Ok(()); }
        }
    }
}

async fn read_stdin_line() -> io::Result<String> {
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();
    let n = reader.read_line(&mut line).await?;
    if n == 0 { return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "EOF")); }
    // Keep trailing newline so the remote shell executes the command
    // Trim only \r (Windows line endings), preserve \n
    if line.ends_with('\r') {
        line.truncate(line.len() - 1);
    }
    Ok(line)
}



#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    use tracing_subscriber::EnvFilter;
    let filter = if cli.verbose { EnvFilter::new("info") } else { EnvFilter::new("error") };
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let resolved_psk = resolve_psk(&cli.psk);
    macro_rules! new_ctx { () => { Ctx::new(&cli.addr, &resolved_psk).await? }; }
    macro_rules! nc { ($ctx:expr, $cmd:expr) => { native_run($ctx, 1, $cmd, 30).await? }; ($ctx:expr, $cmd:expr, $to:expr) => { native_run($ctx, 1, $cmd, $to).await? }; }

    match cli.command {
        Cmd::Exec { cmd, timeout } => {
            let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::ShellExec { cmd: cmd.join(" "), timeout_secs: timeout as u32 });
        }
        Cmd::Iexec { cmd, timeout } => {
            let mut ctx = new_ctx!();
            ctx.send_control(&ControlMsg::NativeSpawn { id: 1, cmd: NativeCmdType::ShellExec { cmd: cmd.join(" "), timeout_secs: timeout as u32 } }).await?;
            drain_exec_interactive(&mut ctx, 1, timeout).await?;
        }
        Cmd::Shell { timeout } => {
            let mut ctx = new_ctx!();
            ctx.send_control(&ControlMsg::NativeSpawn { id: 1, cmd: NativeCmdType::ShellExec { cmd: "bash".into(), timeout_secs: timeout as u32 } }).await?;
            drain_exec_interactive(&mut ctx, 1, timeout).await?;
        }
        Cmd::Batch { file, timeout, parallel, jobs } => {
            if parallel && jobs > 1 {
                let content = std::fs::read_to_string(&file)?;
                let lines: Vec<String> = content.lines()
                    .filter(|l| !l.trim().is_empty() && !l.trim().starts_with("#"))
                    .map(|l| l.to_string())
                    .collect();
                let total_cmds = lines.len();
                info!("Parallel batch: {} commands, {} jobs", total_cmds, jobs);
                let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(jobs as usize));
                let mut handles = Vec::new();
                let addr = cli.addr.clone();
                let psk_hex = resolved_psk.clone();
                for i in 0..total_cmds {
                    let line = lines[i].clone();
                    let permit = sem.clone().acquire_owned().await.expect("sem");
                    let addr = addr.clone();
                    let psk_hex = psk_hex.clone();
                    handles.push(tokio::spawn(async move {
                        let _permit = permit;
                        match Ctx::new(&addr, &psk_hex).await {
                            Ok(mut ctx) => {
                                println!("--- [{}/{}] {} ---", i + 1, total_cmds, line);
                                let _ = native_run(&mut ctx, (i + 1) as u32, NativeCmdType::ShellExec { cmd: line, timeout_secs: timeout as u32 }, timeout).await;
                            }
                            Err(e) => eprintln!("Parallel batch #{} error: {}", i + 1, e),
                        }
                    }));
                }
                for h in handles { let _ = h.await; }
            } else {
                let mut ctx = new_ctx!();
                let content = std::fs::read_to_string(&file)?;
                let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty() && !l.trim().starts_with("#")).collect();
                info!("Batch: {} commands from {}", lines.len(), file);
                let mut ok = 0u32;
                for i in 0..lines.len() {
                    let line = lines[i].clone();
                    println!("--- [{}/{}] {} ---", i + 1, lines.len(), line);
                    native_run(&mut ctx, (i + 1) as u32, NativeCmdType::ShellExec { cmd: line.to_string(), timeout_secs: timeout as u32 }, timeout).await?;
                    ok += 1;
                }
                println!("Batch complete: {} ok", ok);
            }
        }
        Cmd::Watch { interval_secs, cmd } => {
            let mut ctx = new_ctx!();
            let joined = cmd.join(" ");
            let mut n = 0u32;
            loop {
                n += 1;
                println!("=== [{}] {} ===", n, now_str());
                nc!(&mut ctx, NativeCmdType::ShellExec { cmd: joined.clone(), timeout_secs: 30 });
                tokio::time::sleep(Duration::from_secs(interval_secs)).await;
            }
        }
        Cmd::Push { local, remote } => { let mut ctx = new_ctx!(); cmd_push(&mut ctx, &local, &remote).await?; }
        Cmd::Pull { remote, local, timeout } => {
            let mut ctx = new_ctx!();
            let id: u32 = rand::random();
            ctx.send_control(&ControlMsg::NativeCmd { id, cmd: NativeCmdType::ReadFile { path: remote.clone() } }).await?;
            let mut buf = Vec::new();
            let start = Instant::now();
            let deadline = start + Duration::from_secs(timeout);
            loop {
                let rem = deadline.saturating_duration_since(Instant::now());
                if rem.is_zero() { break; }
                match ctx.recv_control(rem.min(Duration::from_secs(2))).await {
                    Ok(Some(msg)) => match msg {
                        ControlMsg::ExecChunk { id: eid, stream: 0, data } if eid == id => { buf.extend_from_slice(&data); }
                        ControlMsg::ExecDone { id: eid, .. } if eid == id => {
                            if let Some(parent) = std::path::Path::new(&local).parent() { let _ = std::fs::create_dir_all(parent); }
                            let dlen = buf.len();
                            std::fs::write(&local, &buf)?;
                            let elapsed = start.elapsed().as_secs();
                            let speed = if elapsed > 0 { dlen as u64 / elapsed } else { 0 };
                            eprintln!("Pulled {} bytes -> {} ({}/s)", dlen, local, human_speed(speed));
                            return Ok(());
                        }
                        _ => {}
                    }
                    _ => {}
                }
            }
        }
        Cmd::Ls { path, long, all } => {
            let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Ls { path: path.clone(), long: long, all: all });
        }
        Cmd::Cat { path } => { let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Cat { path: path.clone() }); }
        Cmd::Tail { path, lines, follow, follow_secs } => {
            let mut ctx = new_ctx!();
            if follow {
                let fsecs = if follow_secs > 0 { follow_secs } else { 2 };
                ctx.send_control(&ControlMsg::NativeSpawn { id: 1, cmd: NativeCmdType::Tail { path: path.clone(), lines: lines, follow: true, follow_secs: fsecs } }).await?;
                drain_exec(&mut ctx, 1, std::cmp::max(fsecs * 2, 3600)).await?;
            } else {
                nc!(&mut ctx, NativeCmdType::Tail { path: path.clone(), lines: lines, follow: false, follow_secs: 0 });
            }
        }
        Cmd::Head { path, lines } => { let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Head { path: path.clone(), lines: lines }); }
        Cmd::Find { path, name, type_, maxdepth } => {
            let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Find { path: path.clone(), name: name.clone(), type_: type_.clone(), maxdepth: maxdepth });
        }
        Cmd::Grep { pattern, path, recursive, line_number, count } => {
            let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Grep { pattern: pattern.clone(), path: path.clone(), recursive: recursive, line_number: line_number, count: count });
        }
        Cmd::Du { path, summarize, maxdepth } => {
            let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Du { path: path.clone(), summarize: summarize, maxdepth: maxdepth });
        }
        Cmd::Df { human, all } => {
            let mut ctx = new_ctx!();
            nc!(&mut ctx, NativeCmdType::Df { human: human, all: all });
        }
        Cmd::Tree { path, depth, dirs_only } => {
            let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Tree { path: path.clone(), depth: depth, dirs_only: dirs_only });
        }
        Cmd::Stat { path } => { let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Stat { path: path.clone() }); }
        Cmd::Mkdir { recursive, paths } => {
            let mut ctx = new_ctx!();
            nc!(&mut ctx, NativeCmdType::Mkdir { recursive: recursive, paths: paths.clone() });
        }
        Cmd::Rm { recursive, force, paths } => {
            let mut ctx = new_ctx!();
            nc!(&mut ctx, NativeCmdType::Rm { recursive: recursive, force: force, paths: paths.clone() });
        }
        Cmd::Mv { src, dest } => { let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Mv { src: src.clone(), dest: dest.clone() }); }
        Cmd::Cp { recursive, src, dest } => {
            let mut ctx = new_ctx!();
            nc!(&mut ctx, NativeCmdType::Cp { recursive: recursive, src: src.clone(), dest: dest.clone() });
        }
        Cmd::Chmod { mode, paths } => { let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Chmod { mode: mode.clone(), paths: paths.clone() }); }
        Cmd::Chown { owner, paths } => { let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Chown { owner: owner.clone(), paths: paths.clone() }); }
        Cmd::Lsblk => { let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Lsblk); }
        Cmd::Mount => { let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Mount); }
        Cmd::Diff { file1, file2 } => { let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Diff { file1: file1.clone(), file2: file2.clone() }); }
        Cmd::Wc { lines, words, paths } => {
            let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Wc { lines: lines, words: words, paths: paths.clone() });
        }
        Cmd::Ps { full, user, tree } => {
            let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Ps { full: full, user: user.clone(), tree: tree });
        }
        Cmd::Kill { pid, signal } => { let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Kill { pid: pid, signal: signal }); }
        Cmd::Pgrep { name, full, count } => {
            let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Pgrep { name: name.clone(), full: full, count: count });
        }
        Cmd::Pkill { name, signal } => { let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Pkill { name: name.clone(), signal: signal }); }
        Cmd::Top { interval_secs, iterations } => {
            let mut ctx = new_ctx!();
            for i in 0..iterations {
                println!("=== [{}] {} ===", i + 1, now_str());
                nc!(&mut ctx, NativeCmdType::Top { interval_secs: interval_secs, iterations: 1 });
                if i < iterations - 1 { tokio::time::sleep(Duration::from_secs(interval_secs)).await; }
            }
        }
        Cmd::Ping { count } => {
            let mut ctx = new_ctx!();
            info!("Ping {} times...", count);
            let mut lat = Vec::new();
            for i in 1..=count {
                let start = Instant::now();
                ctx.send_control(&ControlMsg::Hello { version: lan_link_protocol::frame::PROTOCOL_VERSION, capabilities: vec!["ping".into()] }).await?;
                ctx.recv_control(Duration::from_secs(3)).await?;
                let e = start.elapsed(); lat.push(e);
                println!("Reply from {}: time={:.2}ms", ctx.remote, e.as_secs_f64() * 1000.0);
                if i < count { tokio::time::sleep(Duration::from_millis(500)).await; }
            }
            let avg = lat.iter().map(|d| d.as_micros()).sum::<u128>() as f64 / lat.len() as f64;
            let min = lat.iter().map(|d| d.as_micros()).min().unwrap_or(0) as f64 / 1000.0;
            let max = lat.iter().map(|d| d.as_micros()).max().unwrap_or(0) as f64 / 1000.0;
            println!("--- {} ping statistics ---", count);
            println!("{} packets transmitted, {} received", count, lat.len());
            println!("min/avg/max = {:.2}/{:.2}/{:.2} ms", min, avg / 1000.0, max);
        }
        Cmd::Netstat { tcp, udp, numeric, listening } => {
            let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Netstat { tcp: tcp, udp: udp, numeric: numeric, listening: listening });
        }
        Cmd::Ip { addr, route, link } => {
            let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Ip { addr: addr, route: route, link: link });
        }
        Cmd::PortScan { host, start_port, end_port, timeout_ms } => {
            let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::PortScan { host: host.clone(), start_port: start_port, end_port: end_port, timeout_ms: timeout_ms });
        }
        Cmd::Arp => { let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Arp); }
        Cmd::Dns { hostname, type_ } => {
            let mut ctx = new_ctx!();
            nc!(&mut ctx, NativeCmdType::Dns { hostname: hostname.clone(), type_: type_.clone() });
        }
        Cmd::Info => {
            let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Info);
        }
        Cmd::Uname { all, release, machine } => {
            let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Uname { all: all, release: release, machine: machine });
        }
        Cmd::Uptime => { let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Uptime); }
        Cmd::Hostname => { let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Hostname); }
        Cmd::Whoami => { let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Whoami); }
        Cmd::Who => { let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Who); }
        Cmd::Last { lines } => { let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Last { lines: lines }); }
        Cmd::Free { human } => { let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Free { human: human }); }
        Cmd::Cpu => { let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Cpu); }
        Cmd::Dmesg { lines, level } => {
            let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Dmesg { lines: lines, level: level.clone() });
        }
        Cmd::Service { action } => {
            let mut ctx = new_ctx!();
            let svc = match action {
                ServiceAction::List { active, failed } => ServiceActionType::List { active: active, failed: failed },
                ServiceAction::Status { name } => ServiceActionType::Status { name: name.clone() },
                ServiceAction::Start { name } => ServiceActionType::Start { name: name.clone() },
                ServiceAction::Stop { name } => ServiceActionType::Stop { name: name.clone() },
                ServiceAction::Restart { name } => ServiceActionType::Restart { name: name.clone() },
                ServiceAction::Reload { name } => ServiceActionType::Reload { name: name.clone() },
                ServiceAction::Enable { name } => ServiceActionType::Enable { name: name.clone() },
                ServiceAction::Disable { name } => ServiceActionType::Disable { name: name.clone() },
            };
            nc!(&mut ctx, NativeCmdType::Service { action: svc });
        }
        Cmd::Journal { unit, follow, lines, priority, since } => {
            let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Journal { unit: unit.clone(), follow: follow, lines: lines, priority: priority.clone(), since: since.clone() });
        }
        Cmd::Pkg { action } => {
            let mut ctx = new_ctx!();
            let pkg_action = match action {
                PkgAction::List { installed } => PkgActionType::List { installed: installed },
                PkgAction::Search { query } => PkgActionType::Search { query: query.clone() },
                PkgAction::Install { name } => PkgActionType::Install { name: name.clone() },
                PkgAction::Remove { name } => PkgActionType::Remove { name: name.clone() },
                PkgAction::Update => PkgActionType::Update,
                PkgAction::Upgrade => PkgActionType::Upgrade,
            };
            nc!(&mut ctx, NativeCmdType::Pkg { action: pkg_action });
        }
        Cmd::Docker { action } => {
            let mut ctx = new_ctx!();
            let docker_action = match action {
                DockerAction::Ps { all, .. } => DockerActionType::Ps { all: all, running: false },
                DockerAction::Logs { name, tail, follow } => DockerActionType::Logs { name: name.clone(), tail: tail, follow: follow },
                DockerAction::Stats { .. } => DockerActionType::Stats { interval_secs: 0 },
                DockerAction::Exec { container, interactive, cmd } => DockerActionType::Exec { container: container.clone(), interactive, cmd: cmd.clone() },
                DockerAction::Info => DockerActionType::Info,
                DockerAction::Images => DockerActionType::Images,
                DockerAction::Rm { container, force } => DockerActionType::Rm { container: container.clone(), force: force },
                DockerAction::Control { container, action } => DockerActionType::Control { container: container.clone(), action: action.clone() },
            };
            nc!(&mut ctx, NativeCmdType::Docker { action: docker_action });
        }
        Cmd::Crontab { action } => {
            let mut ctx = new_ctx!();
            let c_action = match action {
                CrontabAction::List => CrontabActionType::List,
                CrontabAction::Edit => CrontabActionType::Edit,
                CrontabAction::Remove => CrontabActionType::Remove,
            };
            nc!(&mut ctx, NativeCmdType::Crontab { action: c_action });
        }
        Cmd::Firewall { backend } => {
            let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Firewall { backend: backend.clone() });
        }
        Cmd::Ssh => { let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Ssh); }
        Cmd::Checksum { path, algorithm } => {
            let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Checksum { path: path.clone(), algorithm: algorithm.clone() });
        }
        Cmd::Key { scancode, vk, release } => {
            let mut ctx = new_ctx!();
            ctx.send_control(&ControlMsg::KeyEvent { down: true, scancode, vk }).await?;
            if release { tokio::time::sleep(Duration::from_millis(50)).await; ctx.send_control(&ControlMsg::KeyEvent { down: false, scancode, vk }).await?; }
        }
        Cmd::Mouse { action } => {
            let mut ctx = new_ctx!();
            match &action {
                MouseAction::Move { dx, dy } => { ctx.send_control(&ControlMsg::MouseMove { dx: *dx as i16, dy: *dy as i16 }).await?; }
                MouseAction::Click { button, release } => {
                    let btn = match button.as_str() { "left" => 0, "right" => 1, "middle" => 2, _ => 0 };
                    ctx.send_control(&ControlMsg::MouseButton { button: btn, down: true }).await?;
                    if !release { tokio::time::sleep(Duration::from_millis(50)).await; ctx.send_control(&ControlMsg::MouseButton { button: btn, down: false }).await?; }
                }
                MouseAction::Wheel { delta, .. } => { ctx.send_control(&ControlMsg::MouseWheel { delta: *delta }).await?; }
            }
        }
        Cmd::Signal { id, signal } => { let mut ctx = new_ctx!(); ctx.send_control(&ControlMsg::ExecSignal { id, signo: signal }).await?; }
        Cmd::Status => { let ctx = new_ctx!(); println!("Connected to {} (conn_id={})", ctx.remote, ctx.conn_id); }
        Cmd::Version => { println!("lan-linkctl {}", env!("CARGO_PKG_VERSION")); }
        Cmd::Video { width, height, fps, stop } => {
            let mut ctx = new_ctx!();
            if stop { ctx.send_control(&ControlMsg::VideoStop).await?; }
            else { ctx.send_control(&ControlMsg::VideoStart { width, height, fps, bitrate_kbps: 5000 }).await?; }
        }
        Cmd::WriteFile { path, data, append } => { let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::WriteFile { path: path.clone(), data: data.as_bytes().to_vec(), append }); }
        Cmd::Sed { path, pattern, replacement, global, regex } => { let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Sed { path: path.clone(), pattern: pattern.clone(), replacement: replacement.clone(), global, regex }); }
        Cmd::Touch { path } => { let mut ctx = new_ctx!(); nc!(&mut ctx, NativeCmdType::Touch { path: path.clone() }); }
    }
    Ok(())
}

async fn native_run(ctx: &mut Ctx, id: u32, cmd: NativeCmdType, timeout: u64) -> anyhow::Result<()> {
    ctx.send_control(&ControlMsg::NativeCmd { id, cmd }).await?;
    let code = drain_exec(ctx, id, timeout).await?;
    if let Some(c) = code { eprintln!("[exit {}]", c); }
    Ok(())
}

async fn cmd_push(ctx: &mut Ctx, local: &str, remote: &str) -> anyhow::Result<()> {
    let data = std::fs::read(local)?;
    let file_id: u32 = rand::random();
    info!("Push #{}: {} -> {} ({} bytes)", file_id, local, remote, data.len());
    ctx.send_control(&ControlMsg::FilePush { id: file_id, path: remote.into(), size: data.len() as u64 }).await?;
    let start = std::time::Instant::now();
    let total = data.len();
    let mut offset: u64 = 0;
    let mut last_pct: usize = 0;
    while offset < total as u64 {
        let end = ((offset as usize) + CHUNK_SIZE).min(total);
        // Send chunk with retry (up to 3 retries, increasing intervals)
        let mut chunk_acked = false;
        for retry in 0..4 {
            if retry > 0 {
                info!("Push #{}: retry {}/3 for offset {}", file_id, retry, offset);
            }
            ctx.send_control(&ControlMsg::FileChunk { id: file_id, offset, data: data[offset as usize..end].to_vec() }).await?;
            let timeout = std::time::Duration::from_secs(1 + retry as u64 * 2); // 1s, 3s, 5s
            let ack_deadline = std::time::Instant::now() + timeout;
            while !chunk_acked && std::time::Instant::now() < ack_deadline {
                if let Ok(Some(msg)) = ctx.recv_control(std::time::Duration::from_millis(500)).await {
                    if let ControlMsg::FileAck { id, offset: ack_off } = msg {
                        if id == file_id && ack_off >= end as u64 { chunk_acked = true; }
                    }
                }
            }
            if chunk_acked { break; }
        }
        if !chunk_acked {
            anyhow::bail!("Push #{}: chunk at offset {} not ACKed after 3 retries", file_id, offset);
        }
        offset = end as u64;
        // Progress bar - uses \r as escape sequence (two chars: \ + r)
        let pct = (offset as usize * 100) / total;
        if pct > last_pct + 5 || offset as usize == total {
            last_pct = pct;
            let bar_w = 40;
            let filled = (pct * bar_w) / 100;
            let bar = format!("{}{}", "=".repeat(filled), " ".repeat(bar_w - filled));
            let elapsed = start.elapsed().as_secs();
            let speed = if elapsed > 0 { offset / elapsed } else { 0 };
            eprint!("\r[{}] {}%  {}/s   ", bar, pct, human_speed(speed));
        }
    }
    eprintln!();
    info!("Push complete: {} bytes in {:?}", total, start.elapsed());
    Ok(())
}

fn human_speed(bytes_per_sec: u64) -> String {
    if bytes_per_sec < 1024 {
        return format!("{}B", bytes_per_sec);
    }
    let kb = bytes_per_sec / 1024;
    if kb < 1024 {
        return format!("{}KB", kb);
    }
    format!("{:.1}MB", kb as f64 / 1024.0)
}

fn now_str() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
    let secs = now.as_secs();
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}
