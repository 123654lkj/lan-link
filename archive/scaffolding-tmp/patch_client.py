# Patch client_win.py to add long-running daemon support.
# Strategy: read original, do targeted string replacements.

with open(r"G:\codex-AI-tools\lan-link\client_win.py", "r", encoding="utf-8") as f:
    src = f.read()

# 1) Add config/log/heartbeat code AFTER imports but BEFORE class definitions.
NEW_HELPERS = '''
class Config:
    """Read config from %APPDATA%\\\\lan-link\\\\config.json. Falls back to defaults."""
    DEFAULTS = {
        "addr": "192.168.31.244:9876",
        "psk": DEFAULT_PSK,
        "log_dir": "",  # empty -> use %LOCALAPPDATA%\\\\lan-link
        "log_max_bytes": 1048576,
        "heartbeat_interval": 15.0,
        "reconnect_backoff": [0.5, 1.0, 2.0, 5.0, 10.0],
    }

    def __init__(self):
        import os, json
        self.path = os.path.join(os.environ.get("APPDATA", os.path.expanduser("~")), "lan-link", "config.json")
        self.values = dict(self.DEFAULTS)
        try:
            with open(self.path, "r", encoding="utf-8") as f:
                user = json.load(f)
            for k, v in user.items():
                if k in self.DEFAULTS:
                    self.values[k] = v
        except FileNotFoundError:
            pass
        except Exception as e:
            sys.stderr.write("config read err: %s\\n" % e)

    def get(self, key):
        return self.values[key]

    def write_default(self):
        """Write default config file so user can edit."""
        import os, json
        os.makedirs(os.path.dirname(self.path), exist_ok=True)
        with open(self.path, "w", encoding="utf-8") as f:
            json.dump(self.DEFAULTS, f, indent=2)


class Log:
    """Log to file (rotating) + stderr when not in --daemon mode."""
    def __init__(self, name, max_bytes=1048576, daemon=False):
        import os
        if name:
            log_dir = os.environ.get("LOCALAPPDATA") or os.path.expanduser("~")
            self.log_dir = os.path.join(log_dir, "lan-link")
        else:
            self.log_dir = ""
        self.max_bytes = max_bytes
        self.daemon = daemon
        self.fp = None
        if self.log_dir:
            try:
                os.makedirs(self.log_dir, exist_ok=True)
                self.path = os.path.join(self.log_dir, name + ".log")
                self.fp = open(self.path, "a", encoding="utf-8", newline="\\n")
            except Exception as e:
                sys.stderr.write("log open err: %s\\n" % e)
                self.fp = None

    def write(self, level, msg):
        import time
        line = "%s [%s] %s" % (time.strftime("%Y-%m-%d %H:%M:%S"), level, msg)
        if not self.daemon:
            sys.stderr.write(line + "\\n")
            sys.stderr.flush()
        if self.fp:
            try:
                self.fp.write(line + "\\n")
                self.fp.flush()
                if self.fp.tell() > self.max_bytes:
                    self.fp.close()
                    try:
                        os.replace(self.path, self.path + ".1")
                    except OSError:
                        pass
                    self.fp = open(self.path, "a", encoding="utf-8", newline="\\n")
            except Exception:
                pass

    def info(self, msg): self.write("INFO", msg)
    def warn(self, msg): self.write("WARN", msg)
    def err(self, msg): self.write("ERR ", msg)
    def close(self):
        if self.fp:
            try: self.fp.close()
            except: pass


'''

# Insert NEW_HELPERS right after DEFAULT_PSK constant
src = src.replace(
    'DEFAULT_PSK = "ca989e3c0e5f763c1ba7f3a8308a9445ca1d5b77a3e896d55e4eac86f25dfb1d"',
    'DEFAULT_PSK = "ca989e3c0e5f763c1ba7f3a8308a9445ca1d5b77a3e896d55e4eac86f25dfb1d"\n' + NEW_HELPERS
)

# 2) Patch LanLinkClient.__init__ to take optional config + log
src = src.replace(
    '''class LanLinkClient:
    def __init__(self, addr, psk_hex):
        host, port = addr.split(":")
        self.addr = (host, int(port))
        self.psk_hex = psk_hex
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.sock.bind(("0.0.0.0", 0))
        self.sock.settimeout(2.0)
        self.conn_id = random.getrandbits(64)
        self.seq = 0''',
    '''class LanLinkClient:
    def __init__(self, addr, psk_hex, log=None, heartbeat_interval=0.0):
        host, port = addr.split(":")
        self.addr = (host, int(port))
        self.psk_hex = psk_hex
        self.log = log
        self.heartbeat_interval = heartbeat_interval
        self._hb_thread = None
        self._hb_stop = None
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.sock.bind(("0.0.0.0", 0))
        self.sock.settimeout(2.0)
        self.conn_id = random.getrandbits(64)
        self.seq = 0
        self.connected = False'''
)

# 3) Patch connect() to set self.connected and start heartbeat
src = src.replace(
    '''    def connect(self, caps=("exec", "input")):
        pkt = build_packet(self.conn_id, SYN, STREAM_CONTROL, 0, 0, b"\\x00" * 12)
        self.send_raw(pkt)
        data = self.recv(3.0)
        if not data:
            raise RuntimeError("No SYN-ACK")
        hdr = parse_header(data)
        if hdr["pkt_type"] != SYN_ACK or hdr["conn_id"] != self.conn_id:
            raise RuntimeError("Bad SYN-ACK")
        self._send_encrypted(STREAM_CONTROL, control_hello(1, list(caps)))''',
    '''    def connect(self, caps=("exec", "input"), retries=1):
        """SYN -> SYN-ACK -> HELLO with simple retry. Raises on hard failure."""
        last_err = None
        for attempt in range(retries + 1):
            try:
                # Reset seq on reconnect so server-side does not see stale seq
                self.seq = 0
                pkt = build_packet(self.conn_id, SYN, STREAM_CONTROL, 0, 0, b"\\x00" * 12)
                self.send_raw(pkt)
                data = self.recv(3.0)
                if not data:
                    raise RuntimeError("No SYN-ACK")
                hdr = parse_header(data)
                if hdr["pkt_type"] != SYN_ACK or hdr["conn_id"] != self.conn_id:
                    raise RuntimeError("Bad SYN-ACK")
                self._send_encrypted(STREAM_CONTROL, control_hello(1, list(caps)))
                self.connected = True
                if self.log:
                    self.log.info("connected conn_id=%d" % self.conn_id)
                self._start_heartbeat()
                return
            except Exception as e:
                last_err = e
                if self.log:
                    self.log.warn("connect attempt %d failed: %s" % (attempt + 1, e))
        raise last_err

    def _start_heartbeat(self):
        import threading
        if self.heartbeat_interval <= 0:
            return
        if self._hb_thread and self._hb_thread.is_alive():
            return
        self._hb_stop = threading.Event()

        def _hb_loop():
            while not self._hb_stop.is_set():
                self._hb_stop.wait(self.heartbeat_interval)
                if self._hb_stop.is_set():
                    break
                try:
                    pkt = build_packet(self.conn_id, HEARTBEAT, 0, 0, 0, b"\\x00" * 12)
                    self.send_raw(pkt)
                except Exception as e:
                    if self.log:
                        self.log.warn("heartbeat err: %s" % e)

        self._hb_thread = threading.Thread(target=_hb_loop, daemon=True)
        self._hb_thread.start()

    def stop(self):
        """Tear down heartbeat + close socket."""
        if self._hb_stop:
            self._hb_stop.set()
        self.connected = False
        try:
            self.sock.close()
        except Exception:
            pass'''
)

# 4) Patch run_input_capture: log start, wrap main loop with try/except
src = src.replace(
    '''    kb_hook = user32.SetWindowsHookExW(WH_KEYBOARD_LL, HOOKPROC(kb_proc), None, 0)
    mouse_hook = user32.SetWindowsHookExW(WH_MOUSE_LL, HOOKPROC(mouse_proc), None, 0)
    if not kb_hook or not mouse_hook:
        raise RuntimeError("SetWindowsHookExW failed")
    print("Input capture started. Ctrl+C to stop.", flush=True)''',
    '''    kb_hook = user32.SetWindowsHookExW(WH_KEYBOARD_LL, HOOKPROC(kb_proc), None, 0)
    mouse_hook = user32.SetWindowsHookExW(WH_MOUSE_LL, HOOKPROC(mouse_proc), None, 0)
    if not kb_hook or not mouse_hook:
        raise RuntimeError("SetWindowsHookExW failed")
    if hasattr(client, "log") and client.log:
        client.log.info("Input capture started")
    else:
        print("Input capture started. Ctrl+C to stop.", flush=True)'''
)

# 5) Patch main(): add --daemon flag, --write-config, --auto-reconnect
src = src.replace(
    '''def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--addr", default="192.168.31.244:9876")
    ap.add_argument("--psk", default=DEFAULT_PSK)
    sub = ap.add_subparsers(dest="cmd", required=True)''',
    '''def main():
    import signal
    ap = argparse.ArgumentParser()
    ap.add_argument("--addr", default=None, help="daemon addr host:port (default from config)")
    ap.add_argument("--psk", default=None, help="PSK hex (default from config)")
    ap.add_argument("--daemon", action="store_true", help="run as long-lived service with file logging + auto-reconnect")
    ap.add_argument("--write-config", action="store_true", help="write default config.json and exit")
    ap.add_argument("--show-config", action="store_true", help="print resolved config and exit")
    ap.add_argument("--auto-reconnect", action="store_true", help="auto-reconnect on disconnect (default in --daemon)")
    sub = ap.add_subparsers(dest="cmd", required=True)'''
)

# 6) Insert config handling at top of main() body
src = src.replace(
    '''    args = ap.parse_args()
    client = LanLinkClient(args.addr, args.psk)
    print("Connecting to %s..." % args.addr, flush=True)
    client.connect()
    print("Connected (conn_id=%d)." % client.conn_id, flush=True)''',
    '''    args = ap.parse_args()

    cfg = Config()
    addr = args.addr or cfg.get("addr")
    psk = args.psk or cfg.get("psk")

    if args.write_config:
        cfg.write_default()
        print("Config written to %s" % cfg.path)
        return
    if args.show_config:
        print("Config file: %s" % cfg.path)
        for k, v in cfg.values.items():
            print("  %s = %s" % (k, v))
        return

    log = Log("daemon" if args.daemon else "", daemon=args.daemon)
    log.info("lan-link client start (daemon=%s, addr=%s)" % (args.daemon, addr))

    if args.daemon:
        # Detach from console window so this process can run without a visible cmd
        try:
            import ctypes
            whnd = ctypes.windll.kernel32.GetConsoleWindow()
            if whnd:
                ctypes.windll.user32.ShowWindow(whnd, 0)  # SW_HIDE
        except Exception:
            pass
        # Also try to free stdout/stderr in case parent is a terminal
        try:
            sys.stdout = open("nul", "w")
            sys.stderr = open("nul", "w")
        except Exception:
            pass

    auto_reconnect = args.daemon or args.auto_reconnect
    hb_interval = cfg.get("heartbeat_interval") if args.daemon else 0.0

    def _build_client():
        return LanLinkClient(addr, psk, log=log, heartbeat_interval=hb_interval)

    def _connect_with_retry(c):
        if not args.daemon:
            print("Connecting to %s..." % addr, flush=True)
        backoff = list(cfg.get("reconnect_backoff"))
        idx = 0
        while True:
            try:
                c.connect()
                if not args.daemon:
                    print("Connected (conn_id=%d)." % c.conn_id, flush=True)
                return
            except Exception as e:
                wait = backoff[min(idx, len(backoff) - 1)]
                idx += 1
                log.err("connect failed: %s; retry in %.1fs" % (e, wait))
                if not args.daemon:
                    print("connect failed: %s; retry in %.1fs" % (e, wait), file=sys.stderr, flush=True)
                if not auto_reconnect:
                    raise
                time.sleep(wait)
                # rebuild conn_id so server treats as fresh
                c.conn_id = random.getrandbits(64)

    client = _build_client()
    _connect_with_retry(client)

    # Handle graceful shutdown via SIGINT/SIGTERM
    stop_event = None
    if args.daemon:
        stop_event = [False]
        def _handler(sig, frame):
            log.info("got signal %d, stopping" % sig)
            stop_event[0] = True
        try:
            signal.signal(signal.SIGINT, _handler)
            signal.signal(signal.SIGTERM, _handler)
        except Exception:
            pass'''
)

# 7) Patch input branch to pass stop_event (so it can exit cleanly on signal)
src = src.replace(
    '''    elif args.cmd == "input":
        run_input_capture(client)''',
    '''    elif args.cmd == "input":
        # Re-implement minimal wrapper: pass log; we keep the existing run_input_capture
        # but we can post a WM_QUIT to break the GetMessageW loop.
        if args.daemon and stop_event is not None:
            # Background thread: watch stop_event, then PostQuitMessage
            import threading
            def _watcher():
                while not stop_event[0]:
                    time.sleep(0.5)
                log.info("PostQuitMessage(0)")
                user32.PostQuitMessage(0)
            threading.Thread(target=_watcher, daemon=True).start()
        run_input_capture(client)'''
)

with open(r"G:\codex-AI-tools\lan-link\client_win.py", "w", encoding="utf-8", newline="\n") as f:
    f.write(src)
print("patched; new size:", len(src))