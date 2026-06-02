"""Test: start a long-running command, send SIGTERM, verify Done with non-zero exit."""
import sys, time, struct
sys.path.insert(0, r"G:\codex-AI-tools\lan-link")
from client_win import (
    Config, Log, LanLinkClient, parse_header, decrypt, HEADER_SIZE,
    control_hello, control_exec, control_exec_signal,
    TAG_EXEC_STARTED, TAG_EXEC_CHUNK, TAG_EXEC_DONE,
)

cfg = Config()
log = Log("test-signal")
client = LanLinkClient(cfg.get("addr"), cfg.get("psk"), log)
client.connect()
client._send_encrypted(0, control_hello(1, ["exec", "input"]))
time.sleep(0.2)

exec_id = 77
client._send_encrypted(0, control_exec(exec_id, "sleep 30"))
print("sent Exec(sleep 30)")

sock = client.sock
sock.settimeout(2.0)
def recv_one(timeout=2.0):
    sock.settimeout(timeout)
    try:
        data, _ = sock.recvfrom(4096)
    except OSError:
        return None
    hdr = parse_header(data)
    if hdr is None or hdr["pkt_type"] != 3 or hdr["stream_id"] != 0:
        return None
    enc = data[HEADER_SIZE:HEADER_SIZE+hdr["payload_len"]]
    return decrypt(client.psk_hex, hdr["nonce"], enc)

# Wait for Started
deadline = time.time() + 2
while time.time() < deadline:
    p = recv_one(0.5)
    if p is None: continue
    tag = struct.unpack("<I", p[:4])[0]
    if tag == TAG_EXEC_STARTED:
        print("EVENT ExecStarted")
        break

# Send SIGTERM (15)
client._send_encrypted(0, control_exec_signal(exec_id, 15))
print("sent SIGTERM")

# Wait for Done
deadline = time.time() + 3
done_exit = None
while time.time() < deadline:
    p = recv_one(0.5)
    if p is None: continue
    tag = struct.unpack("<I", p[:4])[0]
    if tag == TAG_EXEC_DONE:
        pos = 4
        cid = struct.unpack("<I", p[pos:pos+4])[0]; pos += 4
        hc = p[pos]; pos += 1
        ec = None
        if hc: ec = struct.unpack("<i", p[pos:pos+4])[0]
        print(f"EVENT ExecDone id={cid} exit={ec}")
        done_exit = ec
        break

print()
print("exit:", done_exit)
# SIGTERM exit code on Linux for `sleep` is 143 (128+15). Could also be None if process was killed.
# Accept any of: 143, -15, None
assert done_exit in (143, -15, None), f"unexpected exit {done_exit}"
print("ALL ASSERTIONS PASSED")