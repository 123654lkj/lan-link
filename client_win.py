#!/usr/bin/env python3
"""lan-link Windows client: exec + input capture/injection.
Uses stdlib + cryptography. Talks to lan-linkd daemon.
"""
import socket
import struct
import sys
import time
import argparse
import random
import ctypes
import ctypes.wintypes as wt

from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305

HEADER_SIZE = 38
MAX_PAYLOAD = 1400
DEFAULT_PSK = ""

class Config:
    """Read config from %APPDATA%\\lan-link\\config.json. Falls back to defaults."""
    DEFAULTS = {
        "addr": "192.168.31.244:9876",
        "psk": DEFAULT_PSK,
        "log_dir": "",  # empty -> use %LOCALAPPDATA%\\lan-link
        "log_max_bytes": 1048576,
        "heartbeat_interval": 10.0,
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
            sys.stderr.write("config read err: %s\n" % e)

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
                self.fp = open(self.path, "a", encoding="utf-8", newline="\n")
            except Exception as e:
                sys.stderr.write("log open err: %s\n" % e)
                self.fp = None

    def write(self, level, msg):
        import time
        line = "%s [%s] %s" % (time.strftime("%Y-%m-%d %H:%M:%S"), level, msg)
        if not self.daemon:
            sys.stderr.write(line + "\n")
            sys.stderr.flush()
        if self.fp:
            try:
                self.fp.write(line + "\n")
                self.fp.flush()
                if self.fp.tell() > self.max_bytes:
                    self.fp.close()
                    try:
                        os.replace(self.path, self.path + ".1")
                    except OSError:
                        pass
                    self.fp = open(self.path, "a", encoding="utf-8", newline="\n")
            except Exception:
                pass

    def info(self, msg): self.write("INFO", msg)
    def warn(self, msg): self.write("WARN", msg)
    def err(self, msg): self.write("ERR ", msg)
    def close(self):
        if self.fp:
            try: self.fp.close()
            except: pass




SYN, SYN_ACK, ACK, DATA, RST, HEARTBEAT = 0, 1, 2, 3, 4, 5
STREAM_CONTROL, STREAM_INPUT = 0, 4

TAG_EXEC, TAG_EXEC_OUTPUT, TAG_KEY_EVENT, TAG_MOUSE_MOVE, TAG_MOUSE_BUTTON, \
    TAG_MOUSE_WHEEL, TAG_HELLO = 0, 1, 5, 6, 7, 12, 13
TAG_EXEC_STARTED, TAG_EXEC_CHUNK, TAG_EXEC_DONE, TAG_EXEC_STDIN, TAG_EXEC_SIGNAL = 15, 16, 17, 18, 19

# lan-link-input::MouseButton enum tags
MOUSE_BTN_LEFT, MOUSE_BTN_RIGHT, MOUSE_BTN_MIDDLE, MOUSE_BTN_X1, MOUSE_BTN_X2 = 0, 1, 2, 3, 4
# lan-link-input::MouseEvent enum tags
MOUSE_EV_MOVE, MOUSE_EV_BUTTON, MOUSE_EV_WHEEL = 0, 1, 2


def make_nonce(conn_id, seq):
    return struct.pack("<QI", conn_id & 0xFFFFFFFFFFFFFFFF, seq & 0xFFFFFFFF)


def encrypt(psk_hex, nonce, plaintext):
    return ChaCha20Poly1305(bytes.fromhex(psk_hex)).encrypt(nonce, plaintext, None)


def decrypt(psk_hex, nonce, ciphertext):
    return ChaCha20Poly1305(bytes.fromhex(psk_hex)).decrypt(nonce, ciphertext, None)


def build_packet(conn_id, pkt_type, stream_id, seq, flags, nonce, payload=b""):
    hdr = struct.pack("<Q", conn_id)
    hdr += struct.pack("<B", pkt_type)
    hdr += struct.pack("<B", flags)
    hdr += struct.pack("<H", stream_id)
    hdr += struct.pack("<I", seq)
    hdr += struct.pack("<I", 0)  # ack_seq
    hdr += struct.pack("<I", 0)  # ack_bitmap
    hdr += struct.pack("<H", len(payload))
    hdr += nonce
    assert len(hdr) == HEADER_SIZE
    return hdr + payload


def parse_header(data):
    if len(data) < HEADER_SIZE:
        return None
    return dict(
        conn_id=struct.unpack("<Q", data[0:8])[0],
        pkt_type=data[8], flags=data[9],
        stream_id=struct.unpack("<H", data[10:12])[0],
        seq=struct.unpack("<I", data[12:16])[0],
        payload_len=struct.unpack("<H", data[24:26])[0],
        nonce=data[26:38],
    )


def ser_string(s):
    data = s.encode("utf-8") if s else b""
    return struct.pack("<Q", len(data)) + data


def ser_vec_str(items):
    out = struct.pack("<Q", len(items))
    for s in items:
        out += ser_string(s)
    return out


def ser_vec_u8(items):
    n = len(items)
    return struct.pack("<Q", n) + bytes(items)


def control_hello(version, caps):
    return struct.pack("<I", TAG_HELLO) + struct.pack("<H", version) + ser_vec_str(caps)


def control_exec(id, cmd):
    return struct.pack("<I", TAG_EXEC) + struct.pack("<I", id) + ser_string(cmd)


def control_exec_stdin(id, data, close=False):
    return struct.pack("<I", TAG_EXEC_STDIN) + struct.pack("<I", id) + ser_vec_u8(data) + struct.pack("<B", 1 if close else 0)

def control_exec_signal(id, signo):
    return struct.pack("<I", TAG_EXEC_SIGNAL) + struct.pack("<I", id) + struct.pack("<I", signo)

# lan-link-input crate formats (not ControlMsg):
# KeyEvent: bool down (1B) + u16 scancode + u16 vk + u8 modifiers = 6 bytes
# MouseEvent: enum tag (u32 or u8) + payload
# bincode default for enum: u32 little-endian tag

def input_key(down, scancode, vk, modifiers=0):
    # bool is 1 byte in bincode (default config)
    return struct.pack("<BHHB", 1 if down else 0, scancode, vk, modifiers)


def input_mouse_move(dx, dy):
    # enum tag u32 + struct { i32 dx, i32 dy, bool absolute }
    return struct.pack("<I", MOUSE_EV_MOVE) + struct.pack("<iiB", dx, dy, 0) + b"\x00\x00\x00"  # 4B padding to align to 4


def input_mouse_button(button, down):
    # enum tag u32 + struct { MouseButton(u8), bool(u8) } + padding
    return struct.pack("<I", MOUSE_EV_BUTTON) + struct.pack("<BB", button, 1 if down else 0) + b"\x00\x00"


def input_mouse_wheel(delta, horizontal=False):
    return struct.pack("<I", MOUSE_EV_WHEEL) + struct.pack("<hB", delta, 1 if horizontal else 0) + b"\x00"


class LanLinkClient:
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
        self.connected = False

    def send_raw(self, data):
        self.sock.sendto(data, self.addr)

    def recv(self, timeout=3.0):
        self.sock.settimeout(timeout)
        try:
            data, _ = self.sock.recvfrom(4096)
            return data
        except socket.timeout:
            return None

    def connect(self, caps=("exec", "input"), retries=1):
        """SYN -> SYN-ACK -> HELLO with simple retry. Raises on hard failure."""
        last_err = None
        for attempt in range(retries + 1):
            try:
                # Reset seq on reconnect so server-side does not see stale seq
                self.seq = 0
                pkt = build_packet(self.conn_id, SYN, STREAM_CONTROL, 0, 0, b"\x00" * 12)
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
                    pkt = build_packet(self.conn_id, HEARTBEAT, 0, 0, 0, b"\x00" * 12)
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
            pass

    def _next_seq(self):
        self.seq += 1
        return self.seq

    def _send_encrypted(self, stream, msg):
        seq = self._next_seq()
        nonce = make_nonce(self.conn_id, seq)
        enc = encrypt(self.psk_hex, nonce, msg)
        self.send_raw(build_packet(self.conn_id, DATA, stream, seq, 1, nonce, enc))

    def exec(self, cmd, timeout=30, stdin_bytes=None, on_chunk=None):
        """Streaming exec: send Exec, then drain chunks until Done.

        stdin_bytes: optional bytes to write to the process's stdin then close.
        on_chunk: optional callback(stream, bytes) called for each chunk.
                     Default writes stdout to sys.stdout, stderr to sys.stderr.
        Returns (combined_stdout_text, exit_code) or (None, None) on timeout.
        """
        import sys as _sys
        if on_chunk is None:
            def on_chunk(stream, data):
                sink = _sys.stderr if stream == 1 else _sys.stdout
                sink.write(data.decode("utf-8", errors="replace"))
                sink.flush()
        self._send_encrypted(STREAM_CONTROL, control_exec(1, cmd))
        if stdin_bytes is not None:
            self._send_encrypted(STREAM_CONTROL, control_exec_stdin(1, stdin_bytes, True))
        deadline = time.time() + timeout
        out = b""
        exit_code = None
        got_done = False
        saw_started = False
        while time.time() < deadline and not got_done:
            data = self.recv(min(2.0, deadline - time.time()))
            if not data:
                continue
            hdr = parse_header(data)
            if hdr is None or hdr["pkt_type"] != DATA or hdr["stream_id"] != STREAM_CONTROL:
                continue
            if hdr["payload_len"] == 0:
                continue
            enc = data[HEADER_SIZE:HEADER_SIZE + hdr["payload_len"]]
            try:
                plain = decrypt(self.psk_hex, hdr["nonce"], enc)
            except Exception:
                continue
            if len(plain) < 4:
                continue
            tag = struct.unpack("<I", plain[0:4])[0]
            if tag == TAG_EXEC_STARTED:
                saw_started = True
            elif tag == TAG_EXEC_CHUNK:
                pos = 4
                _id = struct.unpack("<I", plain[pos:pos+4])[0]; pos += 4
                stream = plain[pos]; pos += 1
                dlen = struct.unpack("<Q", plain[pos:pos+8])[0]; pos += 8
                chunk = plain[pos:pos+dlen]
                if stream == 0: out += chunk
                on_chunk(stream, chunk)
            elif tag == TAG_EXEC_DONE:
                pos = 4
                _id = struct.unpack("<I", plain[pos:pos+4])[0]; pos += 4
                has_code = plain[pos]; pos += 1
                if has_code: exit_code = struct.unpack("<i", plain[pos:pos+4])[0]
                got_done = True
                break
        if not got_done:
            return None, None
        return out.decode("utf-8", errors="replace"), exit_code

    def send_key(self, down, scancode, vk, modifiers=0):
        self._send_encrypted(STREAM_INPUT, input_key(down, scancode, vk, modifiers))

    def send_mouse_move(self, dx, dy):
        if dx or dy:
            self._send_encrypted(STREAM_INPUT, input_mouse_move(dx, dy))

    def send_mouse_button(self, button, down):
        self._send_encrypted(STREAM_INPUT, input_mouse_button(button, down))

    def send_mouse_wheel(self, delta, horizontal=False):
        self._send_encrypted(STREAM_INPUT, input_mouse_wheel(delta, horizontal))


# --- Windows input capture ---

WH_KEYBOARD_LL = 13
WH_MOUSE_LL = 14
user32 = ctypes.WinDLL("user32", use_last_error=True)


def run_input_capture(client):
    class KBDLLHOOKSTRUCT(ctypes.Structure):
        _fields_ = [("vkCode", wt.DWORD), ("scanCode", wt.DWORD),
                    ("flags", wt.DWORD), ("time", wt.DWORD),
                    ("dwExtraInfo", ctypes.c_void_p)]

    class MSLLHOOKSTRUCT(ctypes.Structure):
        _fields_ = [("pt", wt.POINT), ("mouseData", wt.DWORD),
                    ("flags", wt.DWORD), ("time", wt.DWORD),
                    ("dwExtraInfo", ctypes.c_void_p)]

    HOOKPROC = ctypes.WINFUNCTYPE(ctypes.c_int, ctypes.c_int, wt.WPARAM, ctypes.c_void_p)

    pt = wt.POINT()
    user32.GetCursorPos(ctypes.byref(pt))
    last_x, last_y = [pt.x], [pt.y]

    def kb_proc(nCode, wParam, lParam):
        if nCode >= 0:
            kb = ctypes.cast(lParam, ctypes.POINTER(KBDLLHOOKSTRUCT))[0]
            down = (wParam == 0x0100)
            try:
                client.send_key(down, kb.scanCode, kb.vkCode)
            except Exception as e:
                sys.stderr.write("key err: %s\n" % e)
        return user32.CallNextHookEx(None, nCode, wParam, lParam)

    def mouse_proc(nCode, wParam, lParam):
        if nCode >= 0:
            m = ctypes.cast(lParam, ctypes.POINTER(MSLLHOOKSTRUCT))[0]
            x, y = m.pt.x, m.pt.y
            dx, dy = x - last_x[0], y - last_y[0]
            last_x[0], last_y[0] = x, y
            try:
                if dx or dy:
                    client.send_mouse_move(dx, dy)
                if wParam == 0x0201: client.send_mouse_button(MOUSE_BTN_LEFT, True)
                elif wParam == 0x0202: client.send_mouse_button(MOUSE_BTN_LEFT, False)
                elif wParam == 0x0204: client.send_mouse_button(MOUSE_BTN_RIGHT, True)
                elif wParam == 0x0205: client.send_mouse_button(MOUSE_BTN_RIGHT, False)
                elif wParam == 0x0207: client.send_mouse_button(MOUSE_BTN_MIDDLE, True)
                elif wParam == 0x0208: client.send_mouse_button(MOUSE_BTN_MIDDLE, False)
                elif wParam == 0x020A:
                    delta = ctypes.c_short(m.mouseData >> 16).value
                    client.send_mouse_wheel(delta)
            except Exception as e:
                sys.stderr.write("mouse err: %s\n" % e)
        return user32.CallNextHookEx(None, nCode, wParam, lParam)

    kb_hook = user32.SetWindowsHookExW(WH_KEYBOARD_LL, HOOKPROC(kb_proc), None, 0)
    mouse_hook = user32.SetWindowsHookExW(WH_MOUSE_LL, HOOKPROC(mouse_proc), None, 0)
    if not kb_hook or not mouse_hook:
        raise RuntimeError("SetWindowsHookExW failed")
    if hasattr(client, "log") and client.log:
        client.log.info("Input capture started")
    else:
        print("Input capture started. Ctrl+C to stop.", flush=True)

    class MSG(ctypes.Structure):
        _fields_ = [("hwnd", wt.HWND), ("message", wt.UINT), ("wParam", wt.WPARAM),
                    ("lParam", wt.LPARAM), ("time", wt.DWORD), ("pt", wt.POINT)]

    msg = MSG()
    while True:
        ret = user32.GetMessageW(ctypes.byref(msg), None, 0, 0)
        if ret <= 0:
            break
        user32.TranslateMessage(ctypes.byref(msg))
        user32.DispatchMessageW(ctypes.byref(msg))

    user32.UnhookWindowsHookEx(kb_hook)
    user32.UnhookWindowsHookEx(mouse_hook)


def main():
    import signal
    ap = argparse.ArgumentParser(description="lan-link Windows 远程命令执行工具")
    ap.add_argument("--addr", default=None, help="daemon addr host:port (default from config)")
    ap.add_argument("--psk", default=None, help="PSK hex (default from config)")
    ap.add_argument("--daemon", action="store_true", help="run as long-lived service with file logging + auto-reconnect")
    ap.add_argument("--write-config", action="store_true", help="write default config.json and exit")
    ap.add_argument("--show-config", action="store_true", help="print resolved config and exit")
    ap.add_argument("--auto-reconnect", action="store_true", help="auto-reconnect on disconnect (default in --daemon)")
    sub = ap.add_subparsers(dest="cmd", required=False)

    p_exec = sub.add_parser("exec")
    p_exec.add_argument("command", nargs=argparse.REMAINDER)
    sub.add_parser("input")

    p_k = sub.add_parser("test-keystroke")
    p_k.add_argument("scancode", type=int)
    p_k.add_argument("vk", type=int)
    p_k.add_argument("--count", type=int, default=1)

    p_m = sub.add_parser("test-mousemove")
    p_m.add_argument("dx", type=int)
    p_m.add_argument("dy", type=int)
    p_m.add_argument("--count", type=int, default=10)
    p_m.add_argument("--interval", type=float, default=0.05)

    args = ap.parse_args()

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
    # Enable HB for any long-running mode (input, --daemon, --auto-reconnect)
    # Disable only for one-shot exec commands.
    hb_interval = cfg.get("heartbeat_interval") if (args.daemon or args.cmd == "input" or args.auto_reconnect) else 0.0

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
            pass

    if args.cmd == "exec":
        cmd = " ".join(args.command) if args.command else "echo hello"
        # If stdin is not a tty (piped/redirected), forward it to remote then close
        stdin_data = None
        if not sys.stdin.isatty():
            try:
                stdin_data = sys.stdin.buffer.read()
            except Exception:
                stdin_data = None
        out, code = client.exec(cmd, stdin_bytes=stdin_data, timeout=300.0)
        if out is None and code is None:
            print("No response / timeout", file=sys.stderr)
            sys.exit(124)  # same as `timeout` cmd
        if code is not None:
            sys.exit(code)
        sys.exit(0)
    elif args.cmd == "input":
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
        run_input_capture(client)
    elif args.cmd == "test-keystroke":
        for _ in range(args.count):
            client.send_key(True, args.scancode, args.vk)
            time.sleep(0.05)
            client.send_key(False, args.scancode, args.vk)
            time.sleep(0.05)
        print("Sent %d keystrokes" % args.count, flush=True)
    elif args.cmd == "test-mousemove":
        for i in range(args.count):
            client.send_mouse_move(args.dx, args.dy)
            time.sleep(args.interval)
        print("Sent %d mouse moves (dx=%d, dy=%d)" % (args.count, args.dx, args.dy), flush=True)


if __name__ == "__main__":
    main()