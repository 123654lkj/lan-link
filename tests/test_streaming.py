#!/usr/bin/env python3
"""End-to-end test: send Exec, receive streaming chunks, verify Done.

Run from Windows after the daemon is running on the remote box.
"""
import sys, os, time, struct
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from client_win import (
    Config, Log, LanLinkClient, make_nonce, encrypt, decrypt,
    build_packet, parse_header, HEADER_SIZE, MAX_PAYLOAD,
    control_hello, control_exec, control_exec_stdin, control_exec_signal,
    ser_string, ser_vec_str,
    TAG_EXEC_STARTED, TAG_EXEC_CHUNK, TAG_EXEC_DONE,
)

# Use the same channel numbers as the client
DATA = 3
STREAM_CONTROL = 0
RELIABLE = 1
RELIABLE_ACK = 0

def main():
    cfg = Config()
    log = Log("test-streaming")
    addr = cfg.get("addr")
    psk = cfg.get("psk")
    client = LanLinkClient(addr, psk, log)
    client.connect()
    print(f"connected (conn_id={client.conn_id})")

    # Send Hello so daemon knows our caps
    client._send_encrypted(STREAM_CONTROL, control_hello(1, ["exec", "input"]))
    time.sleep(0.2)

    # Send Exec
    cmd = "for i in 1 2 3; do echo line$i; sleep 0.1; done"
    print(f"sending Exec: {cmd!r}")
    exec_id = 99
    client._send_encrypted(STREAM_CONTROL, control_exec(exec_id, cmd))

    # Read response events for up to 5 seconds
    started = False
    chunks_stdout = b""
    chunks_stderr = b""
    done_exit = None
    deadline = time.time() + 5.0
    sock = client.sock
    sock.settimeout(0.3)
    seen_event_types = []

    while time.time() < deadline:
        try:
            data, _peer = sock.recvfrom(2048)
        except (socket_timeout if False else OSError):
            continue
        hdr = parse_header(data)
        if hdr is None or hdr["pkt_type"] != DATA or hdr["stream_id"] != STREAM_CONTROL:
            continue
        if hdr["payload_len"] == 0:
            continue
        enc = data[HEADER_SIZE:HEADER_SIZE + hdr["payload_len"]]
        try:
            plain = decrypt(client.psk_hex, hdr["nonce"], enc)
        except Exception as e:
            print(f"decrypt err: {e}")
            continue
        if len(plain) < 4:
            continue
        tag = struct.unpack("<I", plain[0:4])[0]
        if tag == TAG_EXEC_STARTED:
            sid = struct.unpack("<I", plain[4:8])[0]
            print(f"EVENT ExecStarted id={sid}")
            started = True
            seen_event_types.append("Started")
        elif tag == TAG_EXEC_CHUNK:
            pos = 4
            cid = struct.unpack("<I", plain[pos:pos+4])[0]; pos += 4
            stream = plain[pos]; pos += 1
            dlen = struct.unpack("<Q", plain[pos:pos+8])[0]; pos += 8
            chunk = plain[pos:pos+dlen]
            print(f"EVENT ExecChunk id={cid} stream={stream} bytes={len(chunk)} {chunk!r}")
            if stream == 0: chunks_stdout += chunk
            else: chunks_stderr += chunk
            seen_event_types.append("Chunk")
        elif tag == TAG_EXEC_DONE:
            pos = 4
            cid = struct.unpack("<I", plain[pos:pos+4])[0]; pos += 4
            has_code = plain[pos]; pos += 1
            exit_code = None
            if has_code:
                exit_code = struct.unpack("<i", plain[pos:pos+4])[0]; pos += 4
            print(f"EVENT ExecDone id={cid} exit={exit_code}")
            done_exit = exit_code
            seen_event_types.append("Done")
            break

    print()
    print("=== summary ===")
    print(f"events:    {seen_event_types}")
    print(f"started:   {started}")
    print(f"stdout:    {chunks_stdout!r}")
    print(f"stderr:    {chunks_stderr!r}")
    print(f"exit_code: {done_exit}")

    # Assertions
    assert started, "no ExecStarted received"
    assert b"line1" in chunks_stdout, f"line1 missing from {chunks_stdout!r}"
    assert b"line2" in chunks_stdout, f"line2 missing from {chunks_stdout!r}"
    assert b"line3" in chunks_stdout, f"line3 missing from {chunks_stdout!r}"
    assert done_exit == 0, f"expected exit=0 got {done_exit}"
    print()
    print("ALL ASSERTIONS PASSED")

if __name__ == "__main__":
    main()