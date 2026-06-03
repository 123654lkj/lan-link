"""Test: send a cat-like command (no args -> reads stdin until EOF),
write to stdin, close, verify chunks and Done."""
import sys, os, time, struct
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from client_win import (
    Config, Log, LanLinkClient, parse_header, decrypt, HEADER_SIZE,
    control_hello, control_exec, control_exec_stdin,
    TAG_EXEC_STARTED, TAG_EXEC_CHUNK, TAG_EXEC_DONE,
)

cfg = Config()
log = Log("test-stdin")
client = LanLinkClient(cfg.get("addr"), cfg.get("psk"), log)
client.connect()
client._send_encrypted(0, control_hello(1, ["exec", "input"]))
time.sleep(0.2)

# Use `cat` which reads stdin and echoes to stdout
exec_id = 42
client._send_encrypted(0, control_exec(exec_id, "cat"))
print("sent Exec(cat)")

sock = client.sock
sock.settimeout(2.0)
events = []
stdout = b""
done_exit = None
waiting_for_chunks = True

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

# Wait for ExecStarted
deadline = time.time() + 2
while time.time() < deadline:
    p = recv_one(0.5)
    if p is None: continue
    tag = struct.unpack("<I", p[:4])[0]
    if tag == TAG_EXEC_STARTED:
        print("EVENT ExecStarted")
        break

# Send some stdin (in 2 chunks), then close
client._send_encrypted(0, control_exec_stdin(exec_id, b"hello ", False))
client._send_encrypted(0, control_exec_stdin(exec_id, b"world\n", False))
client._send_encrypted(0, control_exec_stdin(exec_id, b"", True))  # close
print("sent stdin + close")

# Now wait for chunks + Done
deadline = time.time() + 3
while time.time() < deadline:
    p = recv_one(0.5)
    if p is None: continue
    tag = struct.unpack("<I", p[:4])[0]
    if tag == TAG_EXEC_CHUNK:
        pos = 4
        cid = struct.unpack("<I", p[pos:pos+4])[0]; pos += 4
        stream = p[pos]; pos += 1
        dlen = struct.unpack("<Q", p[pos:pos+8])[0]; pos += 8
        chunk = p[pos:pos+dlen]
        print(f"EVENT ExecChunk id={cid} stream={stream} {chunk!r}")
        if stream == 0: stdout += chunk
    elif tag == TAG_EXEC_DONE:
        pos = 4
        cid = struct.unpack("<I", p[pos:pos+4])[0]; pos += 4
        hc = p[pos]; pos += 1
        ec = None
        if hc: ec = struct.unpack("<i", p[pos:pos+4])[0]
        print(f"EVENT ExecDone id={cid} exit={ec}")
        done_exit = ec
        break

print()
print("stdout:", repr(stdout))
print("exit:", done_exit)
assert stdout == b"hello world\n", f"expected b'hello world\\n', got {stdout!r}"
assert done_exit == 0
print("ALL ASSERTIONS PASSED")