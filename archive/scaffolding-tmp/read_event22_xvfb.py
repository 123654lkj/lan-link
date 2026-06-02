#!/usr/bin/env python3
import evdev
import sys
import time
import os
import select

dev = evdev.InputDevice("/dev/input/event22")
print("Device: %s" % dev.name, flush=True)
print("Path: %s" % dev.path, flush=True)
print("Listeners on this device:", flush=True)
try:
    for fd_info in os.listdir("/proc/%d/fd" % os.getpid()):
        pass
except: pass

# Check who else has the device open via /sys/class/input/event22/holders
holders = open("/sys/class/input/event22/device/holders").read() if os.path.exists("/sys/class/input/event22/device/holders") else "(unknown)"
print("Holders: %s" % holders, flush=True)
print("Listening for 8s...", flush=True)

start = time.time()
count = 0
while time.time() - start < 8:
    r, _, _ = select.select([dev.fd], [], [], 0.3)
    if not r:
        continue
    for event in dev.read():
        count += 1
        if event.type == evdev.ecodes.EV_KEY:
            print("[%.2f] KEY code=%d value=%d" % (time.time() - start, event.code, event.value), flush=True)
        elif event.type == evdev.ecodes.EV_REL:
            print("[%.2f] REL code=%d value=%d" % (time.time() - start, event.code, event.value), flush=True)
        if count >= 200:
            break
print("Got %d events" % count, flush=True)