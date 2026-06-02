//! lan-link-gui: cross-platform native desktop client for lan-link.
//!
//! Self-contained: no WebView, no Node, no browser. Uses eframe/egui with a
//! glow backend so the same code compiles to Windows, Linux, and (with the
//! android-native-activity feature) Android APKs.

mod client;

use std::sync::Arc;

use client::{AppConfig, Connection, ExecEvent, HostConfig};
use eframe::egui;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::try_init().ok();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_min_inner_size([700.0, 500.0])
            .with_title("lan-link"),
        ..Default::default()
    };
    eframe::run_native(
        "lan-link",
        native_options,
        Box::new(|_cc| {
            let app = LanLinkApp::new();
            Ok(Box::new(app))
        }),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConnStatus { Disconnected, Connecting, Connected, Error }

struct LanLinkApp {
    config: AppConfig,
    status: ConnStatus,
    status_msg: String,
    conn: Option<Arc<Mutex<Connection>>>,
    runtime: Arc<Runtime>,
    command_input: String,
    output_lines: Vec<OutputLine>,
    auto_scroll: bool,
    history: Vec<String>,
    pending_events: Arc<std::sync::Mutex<Vec<ExecEvent>>>,
    tab_candidates: Vec<String>,
    tab_filter: String,
    history_idx: Option<usize>,
}

#[derive(Clone)]
struct OutputLine {
    stream: u8,
    text: String,
}

const QUICK_COMMANDS: &[( &str, &str )] = &[
    ("uname -a", "系统信息"),
    ("uptime", "运行时间"),
    ("whoami", "当前用户"),
    ("ip -4 addr show", "IP 地址"),
    ("free -h", "内存使用"),
    ("df -h /", "磁盘空间"),
    ("systemctl status lan-linkd --no-pager", "服务状态"),
    ("tail -n 50 /var/log/lan-linkd.log 2>/dev/null || journalctl -u lan-linkd -n 50 --no-pager", "最近日志"),
    ("top -bn1 | head -20", "进程列表"),
    ("ls -la ~", "家目录"),
];

impl LanLinkApp {
    fn new() -> Self {
        let runtime = Runtime::new().expect("tokio runtime");
        Self {
            config: AppConfig::load(),
            status: ConnStatus::Disconnected,
            status_msg: "未连接".into(),
            conn: None,
            runtime: Arc::new(runtime),
            command_input: String::new(),
            output_lines: Vec::new(),
            auto_scroll: true,
            history: Vec::new(),
            pending_events: Arc::new(std::sync::Mutex::new(Vec::new())),
            tab_candidates: Vec::new(),
            tab_filter: String::new(),
            history_idx: None,
        }
    }

    fn active_host(&self) -> HostConfig {
        let idx = self.config.active_host.min(self.config.hosts.len().saturating_sub(1));
        self.config.hosts[idx].clone()
    }

    fn push_output(&mut self, stream: u8, text: String) {
        const MAX_LINES: usize = 4000;
        if self.output_lines.len() >= MAX_LINES {
            let drop = self.output_lines.len() - MAX_LINES + 256;
            self.output_lines.drain(0..drop);
        }
        self.output_lines.push(OutputLine { stream, text });
    }

    fn connect(&mut self) {
        let host = self.active_host();
        self.status = ConnStatus::Connecting;
        self.status_msg = format!("正在连接 {} ...", host.addr);
        let runtime = self.runtime.clone();
        let (tx, rx) = std::sync::mpsc::channel::<Result<Connection, String>>();
        std::thread::spawn(move || {
            runtime.block_on(async move {
                let r = Connection::connect(&host).await.map_err(|e| e.to_string());
                let _ = tx.send(r);
            });
        });
        match rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(Ok(c)) => {
                self.conn = Some(Arc::new(Mutex::new(c)));
                self.status = ConnStatus::Connected;
                self.status_msg = format!("已连接到 {}", self.active_host().addr);
                self.push_output(2, format!("[已连接] {}", self.active_host().addr));
            }
            Ok(Err(e)) => {
                self.status = ConnStatus::Error;
                self.status_msg = format!("连接失败: {}", e);
                self.push_output(2, format!("[错误] {}", e));
            }
            Err(_) => {
                self.status = ConnStatus::Error;
                self.status_msg = "连接超时".into();
                self.push_output(2, "[错误] 连接超时".into());
            }
        }
    }

    fn run_command(&mut self, cmd: String) {
        if cmd.trim().is_empty() { return; }
        if self.conn.is_none() { self.connect(); }
        let Some(conn) = self.conn.clone() else {
            self.push_output(2, "[错误] 未连接".into());
            return;
        };
        self.push_output(2, format!("$ {}", cmd));
        self.history.push(cmd.clone());
        if self.history.len() > 200 { self.history.remove(0); }
        self.history_idx = None;
        self.command_input.clear();

        let events: Arc<std::sync::Mutex<Vec<ExecEvent>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
        self.pending_events.lock().unwrap().clear();
        let events_w = events.clone();
        let runtime = self.runtime.clone();
        std::thread::spawn(move || {
            runtime.block_on(async move {
                let mut guard = conn.lock().await;
                let events_w_err = events_w.clone();
                let on_event = move |ev: ExecEvent| {
                    events_w.lock().unwrap().push(ev);
                };
                let res = guard.exec_streaming(&cmd, None, 60, on_event).await;
                if let Err(e) = res {
                    events_w_err.lock().unwrap().push(ExecEvent::Chunk(client::ExecOutput {
                        stream: 1,
                        data: format!("\n[错误] {}\n", e).into_bytes(),
                    }));
                    events_w_err.lock().unwrap().push(ExecEvent::Done(None));
                }
            });
        });
        *self.pending_events.lock().unwrap() = (*events.lock().unwrap()).clone();
    }

    fn drain_events(&mut self) {
        let evs: Vec<ExecEvent> = std::mem::take(&mut *self.pending_events.lock().unwrap());
        for ev in evs { self.handle_event(ev); }
    }

    fn handle_event(&mut self, ev: ExecEvent) {
        match ev {
            ExecEvent::Started => { self.push_output(2, "[执行中]".into()); }
            ExecEvent::Chunk(c) => {
                let s = String::from_utf8_lossy(&c.data).into_owned();
                for line in s.split_inclusive('\n') {
                    self.push_output(c.stream, line.to_string());
                }
            }
            ExecEvent::Done(code) => {
                self.push_output(2, format!("[完成] 退出码={:?}", code));
            }
        }
    }

    fn tab_complete(&mut self) {
        let input = self.command_input.trim();
        if input.is_empty() { return; }
        if self.tab_filter != input {
            self.tab_filter = input.to_string();
            self.tab_candidates = QUICK_COMMANDS.iter()
                .map(|(cmd, _)| cmd.to_string())
                .chain(self.history.iter().cloned())
                .filter(|c| c.to_lowercase().starts_with(&input.to_lowercase()))
                .collect::<Vec<_>>();
            let mut seen = std::collections::HashSet::new();
            self.tab_candidates.retain(|c| seen.insert(c.clone()));
        }
        if !self.tab_candidates.is_empty() {
            let next = self.tab_candidates[0].clone();
            self.command_input = next;
            self.tab_candidates.remove(0);
        }
    }

    fn apply_theme(&self, ctx: &egui::Context) {
        let mut style = egui::Style::default();
        // Dark theme with soft colors
        style.visuals = egui::Visuals::dark();
        style.visuals.window_fill = egui::Color32::from_rgb(30, 30, 35);
        style.visuals.panel_fill = egui::Color32::from_rgb(30, 30, 35);
        style.visuals.extreme_bg_color = egui::Color32::from_rgb(20, 20, 25);
        style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(45, 45, 55);
        style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 75));
        style.visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6u8);
        style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(55, 55, 70);
        style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 80, 100));
        style.visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6u8);
        style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(65, 65, 85);
        style.visuals.widgets.active.corner_radius = egui::CornerRadius::same(6u8);
        style.visuals.selection.bg_fill = egui::Color32::from_rgb(60, 100, 160);
        style.visuals.hyperlink_color = egui::Color32::from_rgb(100, 150, 220);
        style.visuals.window_corner_radius = egui::CornerRadius::same(8u8);
        style.visuals.window_shadow = egui::Shadow::NONE;
        ctx.set_style(style);
    }
}

impl eframe::App for LanLinkApp {
    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {}

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events();
        self.apply_theme(ctx);

        // Top bar
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("服务器");
                let active = self.active_host();
                let host_names: Vec<String> = self.config.hosts.iter().map(|h| h.name.clone()).collect();
                egui::ComboBox::from_id_source("host_combo")
                    .selected_text(active.name.clone())
                    .show_ui(ui, |ui| {
                        for (i, name) in host_names.iter().enumerate() {
                            ui.selectable_value(&mut self.config.active_host, i, name.clone());
                        }
                    });
                ui.separator();
                ui.label(&active.addr);
                ui.separator();

                let dot_color = match self.status {
                    ConnStatus::Disconnected => egui::Color32::from_rgb(120, 120, 120),
                    ConnStatus::Connecting => egui::Color32::from_rgb(230, 180, 40),
                    ConnStatus::Connected => egui::Color32::from_rgb(80, 200, 120),
                    ConnStatus::Error => egui::Color32::from_rgb(220, 60, 60),
                };
                ui.colored_label(dot_color, "●");

                let (label, fill) = match self.status {
                    ConnStatus::Disconnected => ("连接", egui::Color32::from_rgb(60, 120, 200)),
                    ConnStatus::Connecting  => ("连接中...", egui::Color32::from_rgb(180, 140, 40)),
                    ConnStatus::Connected   => ("断开", egui::Color32::from_rgb(40, 140, 80)),
                    ConnStatus::Error       => ("重连", egui::Color32::from_rgb(180, 60, 60)),
                };
                let btn = egui::Button::new(label).fill(fill).min_size(egui::vec2(70.0, 24.0));
                if ui.add(btn).clicked() {
                    match self.status {
                        ConnStatus::Disconnected | ConnStatus::Error => self.connect(),
                        ConnStatus::Connected => {
                            self.conn = None;
                            self.status = ConnStatus::Disconnected;
                            self.status_msg = "已断开".into();
                        }
                        ConnStatus::Connecting => {}
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(&self.status_msg);
                });
            });
            ui.separator();
        });

        // Bottom bar
        egui::TopBottomPanel::bottom("bottom_bar").min_height(44.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("$");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.command_input)
                        .hint_text("输入命令，Tab 自动补全")
                        .desired_width(f32::INFINITY),
                );
                let enter_pressed = response.lost_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.input(|i| i.key_pressed(egui::Key::Tab)) && !self.command_input.trim().is_empty() {
                    self.tab_complete();
                    ctx.request_repaint();
                }
                if enter_pressed {
                    let cmd = self.command_input.trim().to_string();
                    if !cmd.is_empty() { self.run_command(cmd); }
                }
                if ui.button("执行").clicked() {
                    let cmd = self.command_input.trim().to_string();
                    if !cmd.is_empty() { self.run_command(cmd); }
                }
                ui.checkbox(&mut self.auto_scroll, "自动滚动");
            });
        });

        // Left panel
        egui::SidePanel::left("left_panel")
            .min_width(180.0)
            .max_width(260.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.label("快捷命令");
                ui.separator();
                for (cmd, desc) in QUICK_COMMANDS {
                    let btn = egui::Button::new(*cmd)
                        .fill(egui::Color32::from_rgb(40, 40, 50));
                    if ui.add(btn).on_hover_text(*desc).clicked() {
                        self.command_input = (*cmd).to_string();
                        self.run_command((*cmd).to_string());
                    }
                }
                ui.separator();
                ui.label("历史命令");
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(ui.available_height() * 0.4)
                    .show(ui, |ui| {
                        let mut items: Vec<(usize, &String)> = self.history.iter().enumerate().collect();
                        items.reverse();
                        for (i, h) in items.iter().take(30) {
                            if ui.selectable_label(false, h.as_str()).clicked() {
                                self.command_input = h.to_string();
                                let _ = i;
                            }
                        }
                    });
                ui.separator();
                ui.collapsing("主机管理", |ui| {
                    let n = self.config.hosts.len();
                    let mut to_remove: Option<usize> = None;
                    for i in 0..n {
                        let mut name = self.config.hosts[i].name.clone();
                        let mut addr = self.config.hosts[i].addr.clone();
                        let mut psk = self.config.hosts[i].psk_hex.clone();
                        ui.label("名称");
                        ui.text_edit_singleline(&mut name);
                        ui.label("地址");
                        ui.text_edit_singleline(&mut addr);
                        ui.label("密钥");
                        ui.add(egui::TextEdit::singleline(&mut psk).hint_text("64位十六进制"));
                        if n > 1 && ui.small_button("删除").clicked() {
                            to_remove = Some(i);
                        }
                        self.config.hosts[i].name = name;
                        self.config.hosts[i].addr = addr;
                        self.config.hosts[i].psk_hex = psk;
                        ui.separator();
                    }
                    if let Some(i) = to_remove {
                        self.config.hosts.remove(i);
                        if self.config.active_host >= self.config.hosts.len() {
                            self.config.active_host = self.config.hosts.len() - 1;
                        }
                    }
                    if ui.button("+ 添加主机").clicked() {
                        self.config.hosts.push(HostConfig::default());
                    }
                    if ui.button("保存配置").clicked() { self.config.save(); }
                });
            });

        // Center: terminal output
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(self.auto_scroll)
                .show(ui, |ui| {
                    ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);
                    for line in &self.output_lines {
                        let color = match line.stream {
                            0 => egui::Color32::from_rgb(220, 220, 225),
                            1 => egui::Color32::from_rgb(240, 130, 130),
                            _ => egui::Color32::from_rgb(120, 180, 240),
                        };
                        ui.colored_label(color, &line.text);
                    }
                });
        });

        if ctx.input(|i| i.key_pressed(egui::Key::F5)) { self.config.save(); }

        if !self.pending_events.lock().unwrap().is_empty() {
            ctx.request_repaint();
        }
    }
}
