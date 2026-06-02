with open(r"G:\codex-AI-tools\lan-link\client_win.py", "r", encoding="utf-8") as f:
    src = f.read()

# Enable HB for any long-running mode (input, --daemon, --auto-reconnect, test modes).
# Only disable HB for one-shot exec commands.
old = "    hb_interval = cfg.get(\"heartbeat_interval\") if args.daemon else 0.0"
new = "    # Enable HB for any long-running mode (input, --daemon, --auto-reconnect)\n" \
      "    # Disable only for one-shot exec commands.\n" \
      "    hb_interval = cfg.get(\"heartbeat_interval\") if (args.daemon or args.cmd == \"input\" or args.auto_reconnect) else 0.0"
src = src.replace(old, new)

# Also: in --daemon mode redirect to nul -- but if HB writes to log it is fine.
# Reduce default HB interval to 10s (under 30s timeout) and increase safety margin.
src = src.replace(
    '"heartbeat_interval": 15.0,',
    '"heartbeat_interval": 10.0,'
)

with open(r"G:\codex-AI-tools\lan-link\client_win.py", "w", encoding="utf-8", newline="\n") as f:
    f.write(src)
print("done")