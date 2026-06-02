#!/bin/bash
set -e

# Start Xvfb (no X server grabs, daemon is the only writer)
Xvfb :99 -screen 0 1024x768x24 -ac +extension RANDR &
XVFB_PID=$!
sleep 1.5
echo "Xvfb started pid=$XVFB_PID"

export DISPLAY=:99

# Wait for X server to be fully up
xset -display :99 q 2>/dev/null || sleep 1

# Now use python-evdev to listen on event22 (no grab) — Xvfb is a fresh X server
# that has NOT yet opened the device (unlike gnome-shell which grabs it).
python3 /tmp/read_event22_xvfb.py > /tmp/xvfb_event.log 2>&1 &
READER_PID=$!
sleep 0.5
echo "Event reader started pid=$READER_PID"

# Initial cursor position
echo "--- before input ---"
xdotool getmouselocation

# Signal ready
echo "READY" > /tmp/xvfb_event.done