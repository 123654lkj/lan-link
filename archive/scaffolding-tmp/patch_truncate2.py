import sys
sys.stdout.reconfigure(encoding="utf-8")

path = "/opt/dashscope_proxy.py"
with open(path, "r", encoding="utf-8") as f:
    lines = f.readlines()

start = None
end = None
for i, line in enumerate(lines):
    if line.strip().startswith("def _truncate_messages"):
        start = i
    if start is not None and i > start and line.strip().startswith("def "):
        end = i
        break

if start is None:
    print("ERROR: _truncate_messages not found")
    sys.exit(1)

print("Found _truncate_messages at lines %d-%d" % (start+1, end))

new_func = """def _has_tool_use(msg):
    content = msg.get("content", "")
    if isinstance(content, list):
        return any(b.get("type") == "tool_use" for b in content)
    return False

def _has_tool_result(msg):
    content = msg.get("content", "")
    if isinstance(content, list):
        return any(b.get("type") == "tool_result" for b in content)
    return False

def _truncate_messages(msgs, max_msgs):
    if len(msgs) <= max_msgs:
        return msgs
    for head_size in (1, 0):
        head = msgs[:head_size]
        tail_size = max_msgs - head_size
        tail = msgs[-tail_size:]
        skip = 0
        for i, m in enumerate(tail):
            if _has_tool_result(m):
                skip = i + 1
            else:
                break
        if skip > 0 and skip < len(tail):
            tail = tail[skip:]
        if head and _has_tool_use(head[-1]):
            continue
        result = head + tail
        print("[TRUNCATE] %d -> %d (head=%d, skipped=%d)" % (len(msgs), len(result), len(head), skip), flush=True)
        return result
    print("[TRUNCATE] %d -> %d (fallback)" % (len(msgs), max_msgs), flush=True)
    return msgs[-max_msgs:]

"""

lines[start:end] = [new_func]

with open(path, "w", encoding="utf-8", newline="\n") as f:
    f.writelines(lines)
print("Patched successfully")