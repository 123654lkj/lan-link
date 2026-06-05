//! lan-linkctl TUI — 交互式远程终端面板
//!
//! 类似 cmd.exe 的交互式终端，底部输入、顶部输出，
//! 支持 tab 命令补全、历史记录。



use ratatui::{
    backend::CrosstermBackend,
    Terminal,
    widgets::{Block, Borders, Paragraph},
    layout::{Layout, Constraint, Direction},
    style::{Style, Color},
    text::Text,
    Frame,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    execute,
};
use std::io::{self, stdout};
use std::time::{Duration, Instant};

/// TUI 应用状态
struct TuiApp {
    /// 命令历史（向上/下键）
    history: Vec<String>,
    history_idx: Option<usize>,
    /// 当前输入
    input: String,
    /// 光标位置
    cursor: usize,
    /// 输出行（上屏）
    output: Vec<String>,
    /// 滚动偏移
    scroll: usize,
    /// 连接的节点列表
    nodes: Vec<NodeInfo>,
    selected_node: usize,
    /// 快速命令补全
    completions: Vec<String>,
    /// 显示补全菜单
    show_completions: bool,
    #[allow(dead_code)]
    completion_idx: usize,
}

struct NodeInfo {
    name: String,
    addr: String,
}

const NODES: &[(&str, &str)] = &[
    ("tuanzi", "192.168.31.244:9876"),
    ("rk1",    "107.174.92.188:9876"),
    ("rk2",    "23.238.57.141:9876"),
    ("lv1",    "96.9.225.57:9876"),
    ("lv2",    "173.249.199.86:9876"),
    ("lv3",    "108.171.195.161:9876"),
    ("xh",     "47.108.166.171:9876"),
];

const BUILTIN_CMDS: &[&str] = &[
    "help", "exit", "clear", "node", "exec", "push", "pull", "cat",
    "tail", "ls", "ps", "free", "df", "uptime", "hostname", "info",
    "service", "docker", "pkg",
];

impl TuiApp {
    fn new() -> Self {
        let nodes: Vec<NodeInfo> = NODES.iter().map(|(n, a)| NodeInfo {
            name: n.to_string(),
            addr: a.to_string(),
        }).collect();

        Self {
            history: Vec::new(),
            history_idx: None,
            input: String::new(),
            cursor: 0,
            output: vec![
                format!("lan-linkctl TUI — 交互式远程终端"),
                format!("{} 个节点已配置。输入 help 查看命令。", nodes.len()),
                format!("当前节点: {} ({})", nodes[0].name, nodes[0].addr),
                String::new(),
            ],
            scroll: 0,
            nodes,
            selected_node: 0,
            completions: BUILTIN_CMDS.iter().map(|s| s.to_string()).collect(),
            show_completions: false,
            completion_idx: 0,
        }
    }

    fn current_node(&self) -> &NodeInfo {
        &self.nodes[self.selected_node]
    }

    fn add_output(&mut self, line: String) {
        self.output.push(line);
        self.scroll = self.output.len().saturating_sub(1);
    }

    fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor, c);
        self.cursor += 1;
    }

    fn delete_char(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.input.remove(self.cursor);
        }
    }

    fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    fn move_right(&mut self) {
        if self.cursor < self.input.len() {
            self.cursor += 1;
        }
    }

    fn submit(&mut self) {
        let cmd = self.input.trim().to_string();
        if cmd.is_empty() {
            return;
        }
        self.history.push(cmd.clone());
        self.history_idx = None;

        self.add_output(format!("{}@{}> {}", 
            whoami(), self.current_node().name, cmd));

        let parts: Vec<&str> = cmd.split_whitespace().collect();
        match parts[0] {
            "exit" | "quit" => {
                // Handled in main loop
            }
            "clear" | "cls" => {
                self.output.clear();
            }
            "help" => {
                self.add_output(" 内置命令:".to_string());
                self.add_output("  help              — 显示此帮助".to_string());
                self.add_output("  exit/quit         — 退出 TUI".to_string());
                self.add_output("  clear/cls         — 清屏".to_string());
                self.add_output("  node <name>       — 切换节点 (tab 补全)".to_string());
                self.add_output("  exec <cmd>        — 远程执行命令".to_string());
                self.add_output("  push -l <本地> -r <远端> — 上传文件".to_string());
                self.add_output("  pull -r <远端> -l <本地> — 下载文件".to_string());
                self.add_output("  cat <path>        — 查看文件".to_string());
                self.add_output("  tail <path>       — 查看文件末尾".to_string());
                self.add_output("  ls [path]         — 列出目录".to_string());
                self.add_output("  ps                — 进程列表".to_string());
                self.add_output("  free              — 内存使用".to_string());
                self.add_output("  df                — 磁盘使用".to_string());
                self.add_output("".to_string());
                self.add_output(format!(" 节点: {}", 
                    self.nodes.iter().map(|n| &n.name[..]).collect::<Vec<_>>().join(", ")));
            }
            "node" => {
                if parts.len() < 2 {
                    self.add_output(format!("当前节点: {} ({})", 
                        self.current_node().name, self.current_node().addr));
                    self.add_output(format!("可用节点: {}", 
                        self.nodes.iter().map(|n| &n.name[..]).collect::<Vec<_>>().join(", ")));
                } else {
                    if let Some(idx) = self.nodes.iter().position(|n| n.name == parts[1]) {
                        self.selected_node = idx;
                        self.add_output(format!("切换到节点: {} ({})", 
                            self.current_node().name, self.current_node().addr));
                    } else {
                        self.add_output(format!("未知节点: {}", parts[1]));
                    }
                }
            }
            _ => {
                // 通过 LL CLI 执行远程命令
                self.add_output("(执行中...)".to_string());
            }
        }

        self.input.clear();
        self.cursor = 0;
        self.show_completions = false;
    }

    fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let idx = match self.history_idx {
            Some(i) if i > 0 => i - 1,
            None => self.history.len() - 1,
            _ => return,
        };
        self.history_idx = Some(idx);
        self.input = self.history[idx].clone();
        self.cursor = self.input.len();
    }

    fn history_down(&mut self) {
        if let Some(i) = self.history_idx {
            if i + 1 < self.history.len() {
                self.history_idx = Some(i + 1);
                self.input = self.history[i + 1].clone();
            } else {
                self.history_idx = None;
                self.input.clear();
            }
            self.cursor = self.input.len();
        }
    }

    fn tab_complete(&mut self) {
        if self.input.is_empty() {
            return;
        }
        let prefix = self.input.split_whitespace().next().unwrap_or("").to_lowercase();
        let matches: Vec<&String> = self.completions.iter()
            .filter(|c| c.starts_with(&prefix))
            .collect();
        
        if matches.len() == 1 {
            self.input = matches[0].clone();
            if self.input.chars().all(|c| c.is_alphanumeric()) {
                self.input.push(' ');
            }
            self.cursor = self.input.len();
            self.show_completions = false;
        } else if matches.len() > 1 {
            self.show_completions = !self.show_completions;
        }
    }
}

fn whoami() -> String {
    "win".to_string()
}

/// 运行 TUI
pub async fn run_tui(_psk_hex: String, _default_addr: String) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = TuiApp::new();
    let tick_rate = Duration::from_millis(50);
    let mut last_tick = Instant::now();

    let res = run_app(&mut terminal, &mut app, tick_rate, &mut last_tick).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(e) = res {
        eprintln!("TUI error: {}", e);
    }

    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut TuiApp,
    tick_rate: Duration,
    last_tick: &mut Instant,
) -> anyhow::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::ZERO);

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            app.add_output("^C".to_string());
                        }
                        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            break;
                        }
                        KeyCode::Enter => {
                            if app.input.trim() == "exit" || app.input.trim() == "quit" {
                                break;
                            }
                            app.submit();
                        }
                        KeyCode::Char(c) => {
                            app.insert_char(c);
                        }
                        KeyCode::Backspace => {
                            app.delete_char();
                        }
                        KeyCode::Left => app.move_left(),
                        KeyCode::Right => app.move_right(),
                        KeyCode::Up => app.history_up(),
                        KeyCode::Down => app.history_down(),
                        KeyCode::Tab => app.tab_complete(),
                        KeyCode::PageUp => {
                            app.scroll = app.scroll.saturating_sub(10);
                        }
                        KeyCode::PageDown => {
                            app.scroll = std::cmp::min(app.scroll + 10, app.output.len().saturating_sub(1));
                        }
                        KeyCode::Home => {
                            app.scroll = 0;
                        }
                        KeyCode::End => {
                            app.scroll = app.output.len().saturating_sub(1);
                        }
                        _ => {}
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            *last_tick = Instant::now();
        }
    }

    Ok(())
}

fn ui(f: &mut Frame, app: &TuiApp) {
    let area = f.area();

    // Layout: output (top) + input (bottom)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(3),
        ])
        .split(area);

    // Top: output panel
    let output_height = chunks[0].height as usize;
    let start_line = if app.scroll + output_height > app.output.len() {
        if app.output.len() > output_height {
            app.output.len() - output_height
        } else {
            0
        }
    } else {
        app.scroll
    };

    let visible: Vec<&str> = app.output
        .iter()
        .skip(start_line)
        .take(output_height)
        .map(|s| s.as_str())
        .collect();

    let output_text = Text::from(visible.join("\n"));
    let output = Paragraph::new(output_text)
        .block(Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ({}:{}) ", app.current_node().name, 
                app.current_node().addr, app.selected_node))
            .title_alignment(ratatui::layout::Alignment::Center)
            .border_style(Style::default().fg(Color::Cyan)));
    f.render_widget(output, chunks[0]);

    // Bottom: input bar
    let prompt = "> ";
    let input_content = format!("{}{}", prompt, app.input);
    let input_para = Paragraph::new(input_content.as_str())
        .block(Block::default()
            .borders(Borders::ALL)
            .title(" Input ")
            .border_style(Style::default().fg(Color::Green)));
    f.render_widget(input_para, chunks[1]);

    // Cursor
    let cursor_x = (prompt.len() + app.cursor) as u16;
    let cursor_y = chunks[1].y + 1;
    f.set_cursor_position((cursor_x, cursor_y));
}
