#!/usr/bin/env python3
# lan-link desktop GUI client (Codex-style)
# Cross-platform standalone desktop app using tkinter (Python stdlib).
# Reuses LanLinkClient from client_win.py for the wire protocol.
# Run:   python client_gui.py
# Package: pyinstaller --noconfirm --noconsole --onefile -n lan-link-gui client_gui.py

import json
import os
import sys
import threading
import tkinter as tk
from tkinter import ttk

from client_win import LanLinkClient
import ctypes

def _hide_console():
    """Hide console window - called after tkinter init."""
    try:
        _hwnd = ctypes.windll.kernel32.GetConsoleWindow()
        if _hwnd:
            ctypes.windll.user32.ShowWindow(_hwnd, 0)  # SW_HIDE
    except Exception:
        pass

APP_NAME = "lan-link 远程命令"

BG          = "#13151a"
BG_PANEL    = "#1a1d23"
BG_INPUT    = "#0f1116"
BG_BTN      = "#242830"
BG_BTN_HI   = "#2f3540"
FG          = "#e8eaed"
FG_DIM      = "#8b95a5"
FG_LINK     = "#6cb4ee"
ACCENT      = "#4ec763"
ACCENT_WARN = "#e0a030"
ACCENT_ERR  = "#f06050"
BORDER      = "#3a4050"

FONT_FAMILY = ("Cascadia Mono", "Consolas", "Menlo", "DejaVu Sans Mono")
FONT_UI     = ("Segoe UI", "Microsoft YaHei UI", "PingFang SC", "Sans", 10)
FONT_MONO   = (FONT_FAMILY[0], 10)
FONT_MONO_S = (FONT_FAMILY[0], 9)


def _config_path():
    base = os.environ.get("APPDATA") or os.path.expanduser("~/.config")
    if os.name == "nt" and not os.environ.get("APPDATA"):
        base = os.path.expanduser("~")
    d = os.path.join(base, "lan-link")
    os.makedirs(d, exist_ok=True)
    return os.path.join(d, "gui-config.json")


def load_config():
    p = _config_path()
    if os.path.exists(p):
        try:
            with open(p, "r", encoding="utf-8") as f:
                return json.load(f)
        except Exception:
            pass
    return {
        "hosts": [
            {"name": "团子", "addr": "192.168.31.244:9876",
             "psk": "ca989e3c0e5f763c1ba7f3a8308a9445ca1d5b77a3e896d55e4eac86f25dfb1d"},
            {"name": "本机", "addr": "127.0.0.1:9876",
             "psk": "ca989e3c0e5f763c1ba7f3a8308a9445ca1d5b77a3e896d55e4eac86f25dfb1d"},
        ],
        "active": 0,
        "history": [],
    }


def save_config(cfg):
    p = _config_path()
    try:
        with open(p, "w", encoding="utf-8") as f:
            json.dump(cfg, f, indent=2)
    except Exception as e:
        print("[save_config err]", e, file=sys.stderr)


QUICK_COMMANDS = [
    ("系统信息", "uname -a",
     "查看内核版本与系统架构"),
    ("运行时间", "uptime",
     "显示系统已运行时长和平均负载"),
    ("当前用户", "whoami",
     "显示当前登录用户名"),
    ("网卡地址", "ip -4 addr show",
     "列出所有 IPv4 地址与网卡状态"),
    ("内存用量", "free -h",
     "查看物理内存与交换分区用量"),
    ("磁盘用量", "df -h /",
     "查看根分区剩余空间"),
    ("服务状态", "systemctl status lan-linkd --no-pager",
     "查看 lan-link 守护进程状态"),
    ("最近日志", "tail -n 50 /var/log/lan-linkd.log 2>/dev/null || journalctl -u lan-linkd -n 50 --no-pager",
     "查看 lan-link 最近 50 行日志"),
]


class ToolTip:
    def __init__(self, widget, text):
        self.widget = widget
        self.text = text
        self.tip = None
        widget.bind("<Enter>", self._show, add="+")
        widget.bind("<Leave>", self._hide, add="+")

    def _show(self, _evt=None):
        if self.tip is not None or not self.text:
            return
        x = self.widget.winfo_rootx() + 18
        y = self.widget.winfo_rooty() + self.widget.winfo_height() + 6
        self.tip = tw = tk.Toplevel(self.widget)
        tw.wm_overrideredirect(True)
        tw.wm_geometry(f"+{x}+{y}")
        lbl = tk.Label(
            tw, text=self.text, justify=tk.LEFT,
            background="#ffffe0", foreground="#222",
            relief=tk.SOLID,
            font=("Microsoft YaHei", 9) if os.name == "nt" else ("Sans", 9),
            padx=6, pady=3,
        )
        lbl.pack()

    def _hide(self, _evt=None):
        if self.tip is not None:
            self.tip.destroy()
            self.tip = None



class GuiClient:
    def __init__(self, on_event):
        self.on_event = on_event
        self._lock = threading.Lock()
        self._client = None
        self._config = load_config()
        self._busy = False

    @property
    def active_host(self):
        idx = self._config.get("active", 0)
        if idx >= len(self._config["hosts"]):
            idx = 0
        return self._config["hosts"][idx]

    def connect(self):
        with self._lock:
            if self._client is not None:
                return
            host = self.active_host
            self._client = LanLinkClient(host["addr"], host["psk"], log=None, heartbeat_interval=0.0)
            try:
                self._client.connect()
                self.on_event({"type": "status", "text": f"已连接到 {host['addr']}"})
            except Exception as e:
                self._client = None
                self.on_event({"type": "status", "text": f"连接失败: {e}", "error": True})
                raise

    def disconnect(self):
        with self._lock:
            if self._client is not None:
                try:
                    self._client.stop()
                except Exception:
                    pass
                self._client = None
                self.on_event({"type": "status", "text": "已断开"})

    @property
    def is_connected(self):
        return self._client is not None

    def run(self, cmd):
        if self._busy:
            self.on_event({"type": "status", "text": "上一条命令还在执行中", "error": True})
            return
        with self._lock:
            if self._client is None:
                self.on_event({"type": "status", "text": "未连接", "error": True})
                return
            client = self._client
        self._busy = True
        self.on_event({"type": "started"})
        threading.Thread(target=self._run_worker, args=(client, cmd), daemon=True).start()

    def _run_worker(self, client, cmd):
        try:
            out, code = client.exec(cmd, timeout=300, on_chunk=self._on_chunk)
            self.on_event({"type": "done", "exit": code})
        except Exception as e:
            self.on_event({"type": "status", "text": f"执行出错: {e}", "error": True})
            self.on_event({"type": "done", "exit": None})
        finally:
            self._busy = False

    def _on_chunk(self, stream, data):
        self.on_event({"type": "chunk", "stream": stream, "data": data})



class App(tk.Tk):
    def __init__(self):
        super().__init__()
        _hide_console()
        self.title(APP_NAME)
        self.geometry("1040x720")
        self.minsize(680, 420)
        self.configure(bg=BG)
        self._apply_theme()
        self.gui_client = GuiClient(on_event=self._on_event)
        self._build_layout()
        self._update_status()
        self.bind("<Configure>", self._on_resize)
        self._reflow_scheduled = False
        self.after(500, self._tick)

    def _apply_theme(self):
        style = ttk.Style(self)
        style.theme_use("alt")
        s = style
        # Global dark background
        s.configure(".", background=BG, foreground=FG, fieldbackground=BG_INPUT, bordercolor=BORDER)
        s.configure("TFrame", background=BG)
        s.configure("Panel.TFrame", background=BG_PANEL)
        s.configure("TLabel", background=BG, foreground=FG, font="TkFixedFont")
        s.configure("Panel.TLabel", background=BG_PANEL, foreground=FG, font="TkFixedFont")
        s.configure("Dim.TLabel", background=BG, foreground=FG_DIM, font="TkFixedFont")
        s.configure("Header.TLabel", background=BG, foreground=FG, font=(FONT_UI[0], 10, "bold"))
        s.configure("PanelHeader.TLabel", background=BG_PANEL, foreground=FG, font=(FONT_UI[0], 9, "bold"))
        s.configure("Status.TLabel", background=BG, foreground=FG_DIM, font="TkFixedFont")
        # Quick-cmd buttons — use a named style AND explicitly pass foreground.
        # clam theme strips foreground from the base style, so we rely on the map.
        s.configure("Quick.TButton", background=BG_BTN, font="TkFixedFont",
                    padding=(10, 8), borderwidth=0, relief=tk.FLAT)
        s.map("Quick.TButton",
              background=[("active", BG_BTN_HI), ("pressed", BG_BTN_HI), ("disabled", BG)],
              foreground=[("active", FG), ("pressed", FG), ("!disabled", FG), ("disabled", FG_DIM)])
        # Action / Accent buttons
        s.configure("Accent.TButton", background=ACCENT, foreground="#0e1116",
                    font=(FONT_UI[0], 10, "bold"), padding=(14, 8))
        s.map("Accent.TButton", background=[("active", "#5dd875"), ("pressed", "#3ab055")])
        s.configure("Danger.TButton", background=ACCENT_ERR, foreground="#fff",
                    font="TkFixedFont", padding=(10, 6))
        s.map("Danger.TButton", background=[("active", "#ff6259")])
        s.configure("Ghost.TButton", background=BG_PANEL, foreground=FG,
                    font="TkFixedFont", padding=(6, 4))
        s.map("Ghost.TButton", background=[("active", BG_BTN_HI)])
        s.configure("Clear.TButton", background=BG_BTN, foreground=FG_DIM,
                    font="TkFixedFont", padding=(6, 4))
        s.map("Clear.TButton", background=[("active", BG_BTN_HI)])
        # Entry (use tk.Entry in layout for reliability)
        s.configure("TEntry", fieldbackground=BG_INPUT, foreground=FG,
                    bordercolor=BORDER, lightcolor=BORDER, darkcolor=BORDER, padding=(10, 7))
        # Combobox
        s.configure("TCombobox", fieldbackground=BG_INPUT, foreground=FG,
                    background=BG_BTN, arrowcolor=FG, bordercolor=BORDER, padding=(6, 4))
        s.map("TCombobox",
              fieldbackground=[("readonly", BG_INPUT)],
              foreground=[("readonly", FG)],
              selectbackground=[("readonly", BG_INPUT)],
              selectforeground=[("readonly", FG)])
        # Checkbox
        s.configure("TCheckbutton", background=BG, foreground=FG_DIM, font="TkFixedFont",
                    focuscolor=BG, indicatorcolor=BG_BTN)
        s.map("TCheckbutton", background=[("active", BG)],
              indicatorcolor=[("selected", ACCENT), ("!selected", BG_BTN)])
        # Paned / Scrollbar
        s.configure("TPanedwindow", background=BG)
        s.configure("Sash", background=BG, sashthickness=4)
        s.configure("Vertical.TScrollbar", background=BG, troughcolor=BG,
                    bordercolor=BG, arrowcolor=FG_DIM, width=10)
        s.configure("Horizontal.TScrollbar", background=BG, troughcolor=BG,
                    bordercolor=BG, arrowcolor=FG_DIM, width=10)
        # Listbox (classic tk)
        s.configure("TListbox", background=BG_INPUT, foreground=FG,
                    selectbackground=BG_BTN_HI, selectforeground=FG,
                    borderwidth=0, highlightthickness=0, font=FONT_MONO_S)
        # Notebook
        s.configure("TNotebook", background=BG, bordercolor=BORDER)
        s.configure("TNotebook.Tab", background=BG_PANEL, foreground=FG,
                    padding=(12, 6), font="TkFixedFont")
        s.map("TNotebook.Tab",
              background=[("selected", BG_BTN_HI)],
              foreground=[("selected", FG)])



    def _build_layout(self):
        # ---- Top bar -----------------------------------------------------------
        top = ttk.Frame(self, padding=(14, 10, 14, 10))
        top.pack(side=tk.TOP, fill=tk.X)
        ttk.Label(top, text=APP_NAME, style="Header.TLabel").pack(side=tk.LEFT)
        # status dot
        self.status_dot = tk.Canvas(top, width=8, height=8, bg=BG, highlightthickness=0)
        self.status_dot.pack(side=tk.LEFT, padx=(12, 6))
        self.status_dot_id = self.status_dot.create_oval(0, 0, 8, 8, fill=ACCENT_WARN, outline="")
        self.status_text = ttk.Label(top, text="未连接", style="Status.TLabel")
        self.status_text.pack(side=tk.LEFT)
        # spacer
        ttk.Frame(top).pack(side=tk.LEFT, fill=tk.X, expand=True)
        # host selector
        ttk.Label(top, text="主机", style="Dim.TLabel").pack(side=tk.LEFT, padx=(0, 6))
        self.host_var = tk.StringVar()
        self.host_combo = ttk.Combobox(
            top, textvariable=self.host_var, state="readonly", width=14,
            values=self._host_names(),
        )
        self.host_combo.pack(side=tk.LEFT, padx=(0, 10))
        self.host_combo.bind("<<ComboboxSelected>>", self._on_host_change)
        if self.gui_client._config["hosts"]:
            self.host_var.set(
                self.gui_client._config["hosts"][
                    self.gui_client._config.get("active", 0)
                ]["name"]
            )
        # connect button
        self.connect_btn = ttk.Button(top, text="连接", style="Accent.TButton",
                                      command=self._toggle_connection)
        self.connect_btn.pack(side=tk.LEFT)
        ttk.Separator(self, orient=tk.HORIZONTAL).pack(side=tk.TOP, fill=tk.X)

        # ---- Main split (left + center) ----------------------------------------
        self._main_paned = ttk.PanedWindow(self, orient=tk.HORIZONTAL)
        main = self._main_paned

        # ---- Bottom command bar (pack BEFORE main so it gets space) ---------------
        bottom = tk.Frame(self, bg=BG, bd=0)
        bottom.pack(side=tk.BOTTOM, fill=tk.X, padx=14, pady=(10, 14))
        bottom.columnconfigure(1, weight=1)

        tk.Label(bottom, text="$", bg=BG, fg=ACCENT,
                 font=(FONT_FAMILY[0], 12, "bold")).grid(row=0, column=0, padx=(0, 8))
        self.cmd_var = tk.StringVar()
        self.cmd_entry = tk.Entry(
            bottom, textvariable=self.cmd_var,
            background=BG_INPUT, foreground=FG, insertbackground=FG,
            selectbackground=BG_BTN_HI, selectforeground=FG,
            borderwidth=1, relief=tk.SOLID, highlightthickness=1,
            highlightbackground=BORDER, highlightcolor=ACCENT,
            font=FONT_MONO, width=40,
        )
        self.cmd_entry.grid(row=0, column=1, sticky="ew", padx=(0, 10))
        self.cmd_entry.bind("<Return>", self._on_run)
        self.cmd_entry.bind("<Tab>", self._on_tab_complete)
        self.cmd_entry.bind("<Up>", self._on_history_up)
        self.cmd_entry.bind("<Down>", self._on_history_down)
        self.cmd_entry.focus_set()
        self.cmd_entry.bind("<KeyRelease>", self._on_cmd_type)
        # Clear tab state when user types printable characters
        self.cmd_entry.bind("<KeyPress>", self._on_key_press)

        btn_frame = ttk.Frame(bottom, style="TFrame")
        btn_frame.grid(row=0, column=2, sticky="e")
        btn_frame.columnconfigure(0, weight=1)

        self.run_btn = tk.Button(btn_frame, text="执行", bg=ACCENT, fg="#0e1116",
                                 activebackground="#4ec763", font=(FONT_UI[0], 10, "bold"),
                                 borderwidth=0, cursor="hand2",
                                 command=self._on_run)
        self.run_btn.pack(side=tk.LEFT, padx=(0, 6))

        self.autoscroll_var = tk.BooleanVar(value=True)
        tk.Checkbutton(btn_frame, text="自动滚动", variable=self.autoscroll_var,
                       bg=BG, fg=FG_DIM, selectcolor=ACCENT,
                       activebackground=BG, activeforeground=FG_DIM,
                       font="TkFixedFont", borderwidth=0).pack(side=tk.LEFT, padx=(4, 0))

        tk.Button(btn_frame, text="清屏", bg=BG_BTN, fg=FG_DIM,
                  activebackground=BG_BTN_HI, font="TkFixedFont",
                  borderwidth=0, cursor="hand2",
                  command=self._clear_output).pack(side=tk.LEFT, padx=(6, 0))

        # Status bar
        sep = ttk.Separator(self, orient=tk.HORIZONTAL)
        sep.pack(side=tk.BOTTOM, fill=tk.X)
        status_bar = ttk.Frame(self, padding=(12, 4))
        status_bar.pack(side=tk.BOTTOM, fill=tk.X)
        self.addr_status = ttk.Label(status_bar, text="", style="Status.TLabel")
        self.addr_status.pack(side=tk.LEFT)
        ttk.Frame(status_bar).pack(side=tk.LEFT, fill=tk.X, expand=True)
        self.hint = ttk.Label(status_bar,
            text="Enter 执行 · ↑↓ 历史 · Ctrl+L 清屏 · F5 保存",
            style="Status.TLabel")
        self.hint.pack(side=tk.RIGHT)
        self.bind_all("<Control-l>", lambda e: self._clear_output())
        self.bind_all("<F5>", lambda e: self._save_config())
        # Set initial sash position
        self.after(100, self._set_initial_sash)

        # Main area (packs last, fills remaining space)
        main.pack(side=tk.TOP, fill=tk.BOTH, expand=True)

        # Left panel
        left = ttk.Frame(main, style="Panel.TFrame", padding=(14, 14, 14, 14))
        main.add(left, weight=1)
        left.columnconfigure(0, weight=1)

        # --- Quick commands ---
        ttk.Label(left, text="常用命令", style="PanelHeader.TLabel").grid(
            row=0, column=0, sticky=tk.W, pady=(0, 8))
        self.quick_container = ttk.Frame(left, style="Panel.TFrame")
        self.quick_container.grid(row=1, column=0, sticky="ew")
        self.quick_container.columnconfigure(0, weight=1)
        self._quick_cols = 1
        self._render_quick_commands()

        # --- Separator ---
        ttk.Separator(left, orient=tk.HORIZONTAL).grid(
            row=2, column=0, sticky="ew", pady=(14, 10))

        # --- History ---
        hist_hdr = ttk.Frame(left, style="Panel.TFrame")
        hist_hdr.grid(row=3, column=0, sticky="ew", pady=(0, 4))
        hist_hdr.columnconfigure(0, weight=1)
        ttk.Label(hist_hdr, text="历史", style="PanelHeader.TLabel").grid(row=0, column=0, sticky=tk.W)
        ttk.Button(hist_hdr, text="清空", style="Ghost.TButton",
                   command=self._clear_history).grid(row=0, column=1, sticky=tk.E)
        self.history_list = tk.Listbox(
            left, height=10, exportselection=False, activestyle="none",
            background=BG_INPUT, foreground=FG,
            selectbackground=BG_BTN_HI, selectforeground=FG,
            borderwidth=1, highlightthickness=0,
            relief=tk.SOLID,
            font=FONT_MONO_S,
        )
        self.history_list.grid(row=4, column=0, sticky="nsew")
        left.rowconfigure(4, weight=1)
        self.history_list.bind("<<ListboxSelect>>", self._on_history_pick)
        self.history_list.config(borderwidth=1, highlightthickness=0,
                                 relief=tk.SOLID)

        # --- Separator ---
        ttk.Separator(left, orient=tk.HORIZONTAL).grid(
            row=5, column=0, sticky="ew", pady=(14, 10))

        # --- Hosts editor ---
        hosts_hdr = ttk.Frame(left, style="Panel.TFrame")
        hosts_hdr.grid(row=6, column=0, sticky="ew", pady=(0, 4))
        hosts_hdr.columnconfigure(0, weight=1)
        ttk.Label(hosts_hdr, text="主机", style="PanelHeader.TLabel").grid(row=0, column=0, sticky=tk.W)
        ttk.Button(hosts_hdr, text="+ 添加", style="Ghost.TButton",
                   command=self._add_host).grid(row=0, column=1, sticky=tk.E)
        self.hosts_box = ttk.Frame(left, style="Panel.TFrame")
        self.hosts_box.grid(row=7, column=0, sticky="ew")
        self.hosts_box.columnconfigure(0, weight=1)
        self._render_hosts()

        # ---- Center terminal ---------------------------------------------------
        # Center terminal frame
        center = ttk.Frame(main, style="TFrame")
        main.add(center, weight=4)
        # term_frame fills the entire PanedWindow pane
        term_frame = ttk.Frame(center, style="TFrame")
        term_frame.pack(fill=tk.BOTH, expand=True)

        self.output = tk.Text(
            term_frame, wrap=tk.NONE,
            background=BG_INPUT, foreground=FG,
            insertbackground=FG, selectbackground=BG_BTN_HI, selectforeground=FG,
            font=FONT_MONO, padx=14, pady=10, relief=tk.FLAT,
            borderwidth=1, highlightthickness=1,
            highlightbackground=BORDER, highlightcolor=BORDER,
            undo=False, takefocus=False,
        )
        self.output.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)
        self.output.tag_configure("stdout", foreground=FG)
        self.output.tag_configure("stderr", foreground=ACCENT_ERR)
        self.output.tag_configure("system", foreground=FG_LINK)
        self.output.tag_configure("prompt", foreground=ACCENT)
        self.output.tag_configure("dim",    foreground=FG_DIM)
        self.output.tag_configure("hi",     foreground=FG, background="#1a212b")
        # scrollbars — dark-themed, placed alongside the Text widget
        ysb = ttk.Scrollbar(term_frame, orient=tk.VERTICAL, command=self.output.yview)
        ysb.pack(side=tk.RIGHT, fill=tk.Y)
        xsb = ttk.Scrollbar(term_frame, orient=tk.HORIZONTAL, command=self.output.xview)
        xsb.pack(side=tk.BOTTOM, fill=tk.X)
        self.output.configure(yscrollcommand=ysb.set, xscrollcommand=xsb.set)
        self.output.bind("<Key>", self._block_text_input)
        self.output.bind("<Control-c>", self._on_copy)
        self.output.bind("<Control-C>", self._on_copy)
        self.output.bind("<Control-a>", self._on_select_all)
        self.output.bind("<Control-A>", self._on_select_all)
        # Right-click context menu
        self.output.bind("<Button-3>", self._show_context_menu)
        self.output.configure(state=tk.DISABLED)

        



    # ---- render helpers ----
    def _render_quick_commands(self):
        for w in self.quick_container.winfo_children():
            w.destroy()
        for i, (label, cmd, tip) in enumerate(QUICK_COMMANDS):
            r, c = divmod(i, self._quick_cols)
            b = ttk.Button(
                self.quick_container, text=label,
                style="Quick.TButton",
                command=lambda v=cmd: self._fill_command(v),
            )
            b.grid(row=r, column=c, sticky="ew",
                   padx=(0 if c == 0 else 4, 0), pady=2, ipadx=6, ipady=4)
            ToolTip(b, tip)

    def _reflow_quick(self, cols):
        if cols == self._quick_cols:
            return
        self._quick_cols = cols
        for w in self.quick_container.winfo_children():
            w.destroy()
        for c in range(cols):
            self.quick_container.columnconfigure(c, weight=1)
        for i, (label, cmd, tip) in enumerate(QUICK_COMMANDS):
            r, c = divmod(i, cols)
            b = ttk.Button(
                self.quick_container, text=label,
                style="Quick.TButton",
                command=lambda v=cmd: self._fill_command(v),
            )
            b.grid(row=r, column=c, sticky="ew",
                   padx=(0 if c == 0 else 4, 0), pady=2, ipadx=6, ipady=4)
            ToolTip(b, tip)

    def _render_hosts(self):
        for w in self.hosts_box.winfo_children():
            w.destroy()
        hosts = self.gui_client._config["hosts"]
        for i, h in enumerate(hosts):
            row = ttk.Frame(self.hosts_box, style="Panel.TFrame")
            row.pack(fill=tk.X, pady=(0, 8))
            row.columnconfigure(1, weight=1)
            ttk.Label(row, text="名字", style="Panel.TLabel").grid(row=0, column=0, sticky=tk.W, pady=(0, 1))
            ttk.Label(row, text="地址", style="Panel.TLabel").grid(row=1, column=0, sticky=tk.W, pady=(0, 1))
            ttk.Label(row, text="PSK ", style="Panel.TLabel").grid(row=2, column=0, sticky=tk.W, pady=(0, 1))
            nv = tk.StringVar(value=h["name"])
            av = tk.StringVar(value=h["addr"])
            pv = tk.StringVar(value=h["psk"])
            tk.Entry(row, textvariable=nv,
                     background=BG_INPUT, foreground=FG, insertbackground=FG,
                     borderwidth=1, relief=tk.SOLID, highlightthickness=1,
                     highlightbackground=BORDER, highlightcolor=ACCENT,
                     font=FONT_MONO_S).grid(row=0, column=1, sticky="ew", padx=(8, 0), pady=(0, 1))
            tk.Entry(row, textvariable=av,
                     background=BG_INPUT, foreground=FG, insertbackground=FG,
                     borderwidth=1, relief=tk.SOLID, highlightthickness=1,
                     highlightbackground=BORDER, highlightcolor=ACCENT,
                     font=FONT_MONO_S).grid(row=1, column=1, sticky="ew", padx=(8, 0), pady=(0, 1))
            tk.Entry(row, textvariable=pv, show="*",
                     background=BG_INPUT, foreground=FG, insertbackground=FG,
                     borderwidth=1, relief=tk.SOLID, highlightthickness=1,
                     highlightbackground=BORDER, highlightcolor=ACCENT,
                     font=FONT_MONO_S).grid(row=2, column=1, sticky="ew", padx=(8, 0), pady=(0, 1))
            btns = ttk.Frame(row, style="Panel.TFrame")
            btns.grid(row=3, column=0, columnspan=2, sticky="ew", pady=(4, 0))
            ttk.Button(btns, text="保存", style="Ghost.TButton",
                       command=lambda i=i, n=nv, a=av, p=pv: self._save_host(i, n, a, p)).pack(side=tk.LEFT)
            if len(hosts) > 1:
                ttk.Button(btns, text="删除", style="Ghost.TButton",
                           command=lambda i=i: self._remove_host(i)).pack(side=tk.LEFT, padx=(6, 0))

    # ---- callbacks ----
    def _host_names(self):
        return [h["name"] for h in self.gui_client._config["hosts"]]

    def _on_host_change(self, _evt=None):
        name = self.host_var.get()
        for i, h in enumerate(self.gui_client._config["hosts"]):
            if h["name"] == name:
                self.gui_client._config["active"] = i
                self._save_config()
                break
        self._update_status()

    def _toggle_connection(self):
        if self.gui_client.is_connected:
            self.gui_client.disconnect()
        else:
            try:
                self.gui_client.connect()
            except Exception:
                pass
        self._update_status()

    def _update_status(self):
        host = self.gui_client.active_host
        self.addr_status.configure(text=f"  {host['name']}  ·  {host['addr']}")
        if self.gui_client.is_connected:
            self.status_dot.itemconfigure(self.status_dot_id, fill=ACCENT)
            self.status_text.configure(text="已连接", foreground=ACCENT)
            self.connect_btn.configure(text="断开", style="Danger.TButton")
        else:
            self.status_dot.itemconfigure(self.status_dot_id, fill=ACCENT_WARN)
            self.status_text.configure(text="未连接", foreground=ACCENT_WARN)
            self.connect_btn.configure(text="连接", style="Accent.TButton")

    def _fill_command(self, cmd):
        self.cmd_var.set(cmd)
        self.cmd_entry.focus_set()
        self.cmd_entry.bind("<KeyRelease>", self._on_cmd_type)

    def _on_history_pick(self, _evt=None):
        sel = self.history_list.curselection()
        if sel:
            self._fill_command(self.history_list.get(sel[0]))

    def _clear_history(self):
        self.gui_client._config["history"] = []
        self._refresh_history()
        self._save_config()

    def _on_tab_complete(self, _evt=None):
        """Tab key completion: show command with Chinese hint in status bar"""
        current = self.cmd_var.get().strip().lower()
        if not current:
            return "break"
        # Build matches with Chinese descriptions
        quick_map = {cmd: label for label, cmd, _ in QUICK_COMMANDS}
        quick_cmds = [(cmd, label) for label, cmd, _ in QUICK_COMMANDS]
        # Deduplicate
        seen = set()
        unique = []
        for cmd, label in quick_cmds:
            if cmd not in seen:
                seen.add(cmd)
                unique.append((cmd, label))
        # Add history items
        history = self.gui_client._config.get("history", [])
        for cmd in history:
            if cmd not in seen:
                seen.add(cmd)
                unique.append((cmd, ""))
        # Filter
        tab_filter = self.gui_client._config.get("_tab_filter", "")
        if tab_filter != current:
            matches = [(cmd, label) for cmd, label in unique if cmd.lower().startswith(current)]
            self.gui_client._config["_tab_matches"] = matches
            self.gui_client._config["_tab_idx"] = 0
        else:
            matches = self.gui_client._config.get("_tab_matches", [])
        tab_idx = self.gui_client._config.get("_tab_idx", 0)
        if matches:
            idx = tab_idx % len(matches)
            cmd, label = matches[idx]
            # Only set the command text (clean, no Chinese)
            self.cmd_var.set(cmd)
            self.cmd_entry.icursor(tk.END)
            # Show Chinese hint in status bar
            if label:
                self.hint.configure(text=f"Tab: {cmd}  —  {label}  ·  Enter 执行")
            else:
                self.hint.configure(text=f"Tab: {cmd}  ·  Enter 执行")
            self.gui_client._config["_tab_idx"] = idx + 1
            self.gui_client._config["_tab_filter"] = current
        return "break"


    def _on_history_up(self, _evt=None):
        h = self.gui_client._config.get("history", [])
        if not h:
            return "break"
        idx = self.gui_client._config.get("_hist_idx")
        if idx is None:
            idx = len(h)
        idx = max(0, idx - 1)
        self.gui_client._config["_hist_idx"] = idx
        self.cmd_var.set(h[idx])
        return "break"

    def _on_history_down(self, _evt=None):
        h = self.gui_client._config.get("history", [])
        idx = self.gui_client._config.get("_hist_idx")
        if idx is None:
            return "break"
        idx = min(len(h), idx + 1)
        if idx >= len(h):
            self.gui_client._config["_hist_idx"] = None
            self.cmd_var.set("")
        else:
            self.gui_client._config["_hist_idx"] = idx
            self.cmd_var.set(h[idx])
        self.gui_client._config["_tab_idx"] = None
        self.gui_client._config["_tab_filter"] = ""
        return "break"


    def _on_key_press(self, _evt=None):
        """Clear tab state when user types a printable character"""
        if _evt and _evt.char and _evt.char.isprintable():
            self.gui_client._config["_tab_idx"] = 0
            self.gui_client._config["_tab_filter"] = ""

    def _on_cmd_type(self, _evt=None):
        """Reset hint when user types (but not during tab completion)"""
        # Don't reset if we're in the middle of tab cycling
        if self.gui_client._config.get("_tab_idx", 0) > 0:
            return
        self.hint.configure(text="Enter 执行 · ↑↓ 历史 · Ctrl+L 清屏 · F5 保存")

    def _on_run(self, _evt=None):
        self.hint.configure(text="Enter 执行 · ↑↓ 历史 · Ctrl+L 清屏 · F5 保存")
        cmd = self.cmd_var.get().strip()
        if not cmd:
            return "break"
        self.cmd_var.set("")
        if not self.gui_client.is_connected:
            self._append("system", "[未连接] ", dim=True)
        history = self.gui_client._config.setdefault("history", [])
        if not history or history[-1] != cmd:
            history.append(cmd)
            if len(history) > 200:
                del history[: len(history) - 200]
            self._refresh_history()
            save_config(self.gui_client._config)
        self.gui_client._config["_hist_idx"] = None
        self.gui_client._config["_tab_idx"] = None
        self.gui_client._config["_tab_filter"] = ""
        self._append("prompt", f"$ {cmd}\n")
        if self.gui_client.is_connected:
            self.gui_client.run(cmd)
        return "break"

    def _refresh_history(self):
        self.history_list.delete(0, tk.END)
        for h in self.gui_client._config.get("history", []):
            self.history_list.insert(tk.END, h)

    def _save_host(self, i, n, a, p):
        h = self.gui_client._config["hosts"][i]
        h["name"] = n.get().strip() or h["name"]
        h["addr"] = a.get().strip() or h["addr"]
        h["psk"] = p.get().strip() or h["psk"]
        self.host_combo.configure(values=self._host_names())
        if self.host_var.get() == h["name"]:
            self._update_status()
        save_config(self.gui_client._config)

    def _remove_host(self, i):
        hosts = self.gui_client._config["hosts"]
        if len(hosts) <= 1:
            return
        del hosts[i]
        active = self.gui_client._config.get("active", 0)
        if active >= len(hosts):
            self.gui_client._config["active"] = len(hosts) - 1
        self.host_combo.configure(values=self._host_names())
        if hosts:
            self.host_var.set(hosts[self.gui_client._config["active"]]["name"])
        self._render_hosts()
        self._save_config()

    def _add_host(self):
        n = len(self.gui_client._config["hosts"]) + 1
        self.gui_client._config["hosts"].append({
            "name": f"主机{n}",
            "addr": "127.0.0.1:9876",
            "psk": "ca989e3c0e5f763c1ba7f3a8308a9445ca1d5b77a3e896d55e4eac86f25dfb1d",
        })
        self.host_combo.configure(values=self._host_names())
        self._render_hosts()
        self._save_config()

    def _save_config(self):
        save_config(self.gui_client._config)
        self.addr_status.configure(text=f"  已保存配置  ·  {_config_path()}")

    def _clear_output(self):
        self.output.configure(state=tk.NORMAL)
        self.output.delete("1.0", tk.END)
        self.output.configure(state=tk.DISABLED)

    def _on_copy(self, _evt=None):
        """Copy selected text from the output terminal"""
        try:
            sel = self.output.tag_ranges("sel")
            if sel:
                text = self.output.get(sel[0], sel[1])
                self.clipboard_clear()
                self.clipboard_append(text)
        except tk.TclError:
            pass
        return "break"

    def _on_select_all(self, _evt=None):
        """Select all text in the output terminal"""
        self.output.configure(state=tk.NORMAL)
        self.output.tag_add("sel", "1.0", tk.END)
        self.output.configure(state=tk.DISABLED)
        return "break"

    def _show_context_menu(self, event):
        """Show right-click menu on output terminal"""
        self.output.configure(state=tk.NORMAL)
        menu = tk.Menu(self, tearoff=0, bg=BG_PANEL, fg=FG, activebackground=BG_BTN_HI)
        # Check if there is a selection
        try:
            sel = self.output.tag_ranges("sel")
            if sel:
                menu.add_command(label="复制 (Ctrl+C)", command=self._on_copy)
                menu.add_separator()
            menu.add_command(label="全选 (Ctrl+A)", command=self._on_select_all)
        except tk.TclError:
            menu.add_command(label="全选 (Ctrl+A)", command=self._on_select_all)
        menu.tk_popup(event.x_root, event.y_root)
        self.output.configure(state=tk.DISABLED)
        return "break"

    def _set_initial_sash(self):
        """Set initial sash position for better layout"""
        try:
            w = self.winfo_width()
            # Left panel gets about 25% of width
            left_w = max(200, int(w * 0.25))
            main.sashpos(0, left_w)
        except Exception:
            pass

    def _update_output_height(self, total_h):
        """Adjust the terminal output area to fit better"""
        # Set a reasonable minimum visible area for output
        # Show at least 20 lines, but don't exceed available space
        min_lines = max(20, int(total_h / 16))  # ~16px per line
        max_lines = min(min_lines, 100)  # Cap at 100 lines
        # Update the output widget configuration
        self.output.configure(height=max_lines)

    def _block_text_input(self, _evt):
        return "break"

    def _on_event(self, ev):
        self.after(0, lambda: self._handle_event(ev))

    def _handle_event(self, ev):
        t = ev.get("type")
        if t == "chunk":
            data = ev["data"]
            try:
                text = data.decode("utf-8", errors="replace")
            except Exception:
                text = str(data)
            tag = "stderr" if ev.get("stream") == 1 else "stdout"
            self._append(tag, text)
        elif t == "started":
            self._append("system", "[started]\n")
        elif t == "done":
            exit_code = ev.get("exit")
            self._append("system", f"[done] 退出码 = {exit_code!r}\n", hi=True)
        elif t == "status":
            self._append("system", f"[{ev.get('text','')}]\n")
            if "error" in ev and ev.get("error"):
                self.status_text.configure(text=ev.get("text", ""), foreground=ACCENT_ERR)
        self._update_status()

    def _append(self, tag, text, dim=False, hi=False):
        if dim:
            tag = "dim"
        self.output.configure(state=tk.NORMAL)
        if hi:
            end_nl = ""
            if text.endswith("\n"):
                end_nl = "\n"
                text = text[:-1]
            self.output.insert(tk.END, text, ("hi",))
            self.output.insert(tk.END, end_nl, ())
        else:
            self.output.insert(tk.END, text, (tag,))
        self.output.configure(state=tk.DISABLED)
        if self.autoscroll_var.get():
            self.output.see(tk.END)

    def _tick(self):
        self._update_status()
        self.after(500, self._tick)

    def _on_resize(self, _evt=None):
        if self._reflow_scheduled:
            return
        self._reflow_scheduled = True
        self.after(120, self._reflow)

    def _reflow(self):
        self._reflow_scheduled = False
        try:
            side_w = self.quick_container.winfo_width()
            total_h = self.winfo_height()
        except tk.TclError:
            return
        if side_w <= 0:
            cols = 1
        else:
            cols = max(1, side_w // 130)
            cols = min(cols, len(QUICK_COMMANDS))
        self._reflow_quick(cols)
        # Set terminal output to show more lines by default
        if total_h > 0:
            self._update_output_height(total_h)


def main():
    app = App()
    app.mainloop()
    return 0


if __name__ == "__main__":
    sys.exit(main())

