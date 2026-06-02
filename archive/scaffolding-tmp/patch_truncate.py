import sys
sys.stdout.reconfigure(encoding="utf-8")

path = r"G:\codex-AI-tools\tmp\dashscope_proxy_v3.py"
with open(path, "r", encoding="utf-8") as f:
    content = f.read()

old = '''def _truncate_messages(msgs, max_msgs):
    """Keep recent messages within limit. Preserve first few for context."""
    if len(msgs) <= max_msgs:
        return msgs
    head = msgs[:2]
    tail = msgs[-(max_msgs - 2):]
    print('[TRUNCATE] %d -> %d messages (kept first 2 + last %d)' % (len(msgs), len(head) + len(tail), len(tail)), flush=True)
    return head + tail'''

new = '''def _has_tool_use(msg):
    """Check if a message contains tool_use blocks."""
    content = msg.get('content', '')
    if isinstance(content, list):
        return any(b.get('type') == 'tool_use' for b in content)
    return False

def _has_tool_result(msg):
    """Check if a message contains tool_result blocks."""
    content = msg.get('content', '')
    if isinstance(content, list):
        return any(b.get('type') == 'tool_result' for b in content)
    return False

def _truncate_messages(msgs, max_msgs):
    """Keep recent messages within limit. Preserve first few for context.
    Ensures no orphan tool_use or tool_result at truncation boundary."""
    if len(msgs) <= max_msgs:
        return msgs
    # Try head=1 first (just the initial user message), then head=0
    for head_size in (1, 0):
        head = msgs[:head_size]
        tail_size = max_msgs - head_size
        tail = msgs[-tail_size:]
        # Check: tail must not start with a tool_result (orphaned)
        # If it does, skip forward until we find a non-tool_result message
        skip = 0
        for i, m in enumerate(tail):
            if _has_tool_result(m) or (m.get('role') == 'user' and _has_tool_result(m)):
                skip = i + 1
            else:
                break
        if skip > 0 and skip < len(tail):
            tail = tail[skip:]
        # Check: head must not end with tool_use (orphaned result)
        if head and _has_tool_use(head[-1]):
            continue  # Try smaller head
        result = head + tail
        if len(result) <= max_msgs:
            print('[TRUNCATE] %d -> %d messages (head=%d, tail=%d, skipped=%d)' % (
                len(msgs), len(result), len(head), len(tail), skip), flush=True)
            return result
    # Fallback: just take last max_msgs messages
    print('[TRUNCATE] %d -> %d messages (fallback)' % (len(msgs), max_msgs), flush=True)
    return msgs[-max_msgs:]'''

content = content.replace(old, new)

outpath = r"G:\codex-AI-tools\tmp\dashscope_proxy_v4.py"
with open(outpath, "w", encoding="utf-8", newline="\n") as f:
    f.write(content)
print("Patched. Size: %d bytes" % len(content))
if new[:50] in content:
    print("OK: truncate patched")
else:
    print("FAIL: truncate not patched")