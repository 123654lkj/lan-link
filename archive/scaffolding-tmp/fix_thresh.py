path = r"G:\codex-AI-tools\lan-link\crates\daemon\src\main.rs"
with open(path, "r", encoding="utf-8") as f:
    c = f.read()
# Move: 3 events (REL_X, REL_Y, SYN) = 72 bytes expected
# Key: 2 events (KEY, SYN) = 48 bytes expected
# Wheel: 2 events (WHEEL, SYN) = 48 bytes expected
# Button: 2 events (BTN, SYN) = 48 bytes expected
# But isize write returns negative on error, 24*event_count on success
# So we want: warn if bytes % 24 != 0 (not a whole event) OR if bytes <= 0
c = c.replace(
    "if bytes != 48 && bytes > 0 { warn!(\"uinput mouse inject short write: {} bytes\", bytes); }",
    "if bytes <= 0 || bytes % 24 != 0 { warn!(\"uinput mouse inject bad write: {} bytes (expected multiple of 24)\", bytes); }"
)
c = c.replace(
    "if bytes != 48 && bytes > 0 { warn!(\"uinput key inject short write: {} bytes\", bytes); }",
    "if bytes <= 0 || bytes % 24 != 0 { warn!(\"uinput key inject bad write: {} bytes (expected multiple of 24)\", bytes); }"
)
with open(path, "w", encoding="utf-8", newline="\n") as f:
    f.write(c)
print("Threshold fixed")