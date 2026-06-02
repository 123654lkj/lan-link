#!/usr/bin/env python3
import evdev, time, select, os
dev = evdev.InputDevice("/dev/input/event22")
print("Reading %s (15s)..." % dev.path, flush=True)
start = time.time()
count = 0
while time.time() - start < 15:
    r, _, _ = select.select([dev.fd], [], [], 0.3)
    if not r:
        continue
    for event in dev.read():
        count += 1
        if event.type == evdev.ecodes.EV_REL:
            print("[%.2f] REL code=%d value=%d" % (time.time() - start, event.code, event.value), flush=True)
        elif event.type == evdev.ecodes.EV_KEY:
            print("[%.2f] KEY code=%d value=%d" % (time.time() - start, event.code, event.value), flush=True)
        if count > 500: break
print("DONE. count=%d" % count, flush=True)