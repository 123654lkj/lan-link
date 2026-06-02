#!/usr/bin/env python3
"""Non-grabbing event reader for /dev/input/event22.
X server may also be reading this device, but we can still observe events
when the kernel delivers them to multiple readers in fan-out fashion.
"""
import evdev
import sys
import time
import os

dev = evdev.InputDevice("/dev/input/event22")
print("Device: %s" % dev.name, flush=True)
print("Listeners: %d" % len(dev.active_users()) if hasattr(dev, "active_users") else "n/a", flush=True)
print("Listening for events... (timeout 5s)", flush=True)

start = time.time()
count = 0
try:
    # Use select to avoid hanging
    import select
    while time.time() - start < 5:
        r, _, _ = select.select([dev.fd], [], [], 0.5)
        if not r:
            continue
        for event in dev.read():
            count += 1
            if event.type == evdev.ecodes.EV_KEY:
                print("[%.2f] KEY code=%d value=%d" % (time.time() - start, event.code, event.value), flush=True)
            elif event.type == evdev.ecodes.EV_REL:
                print("[%.2f] REL code=%d value=%d" % (time.time() - start, event.code, event.value), flush=True)
            elif event.type == evdev.ecodes.EV_SYN:
                pass
            if count >= 200:
                break
except KeyboardInterrupt:
    pass
print("Got %d events" % count)