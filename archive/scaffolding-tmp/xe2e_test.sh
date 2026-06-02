#!/bin/bash
set -e

# Cleanup
pkill -f 'Xvfb :99' 2>/dev/null || true
sleep 0.5

# Start Xvfb
Xvfb :99 -screen 0 1024x768x24 -ac +extension RANDR &
XVFB_PID=$!
sleep 1
echo "Xvfb started pid=$XVFB_PID on :99"

# Set DISPLAY
export DISPLAY=:99

# Get initial cursor position
INITIAL=$(xdotool getmouselocation 2>&1)
echo "Initial cursor: $INITIAL"

# Find the new lan-link-kvm event device number
echo "--- /dev/input/ devices ---"
ls -la /dev/input/event* | tail -10

# Find which event# is the new lan-link-kvm
NEW_DEV=$(for d in /sys/class/input/event*; do
  name=$(cat $d/device/name 2>/dev/null)
  if [ "$name" = "lan-link-kvm" ]; then
    echo $d | sed "s|.*/event|/dev/input/event|"
    break
  fi
done)
echo "lan-link-kvm is at: $NEW_DEV"

# Start evtest on it (will grab; daemon still writes to uinput fd fine)
evtest $NEW_DEV > /tmp/evtest.log 2>&1 &
EVTEST_PID=$!
sleep 1
echo "evtest started pid=$EVTEST_PID"

# Now ask Windows to send 50 right + 50 down (50px each)
echo "--- ready to send test input from Windows ---"
echo "Display :99, evtest listening on $NEW_DEV"