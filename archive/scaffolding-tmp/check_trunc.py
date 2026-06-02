import json, sys

with open("/tmp/xingdu_dumps/1780290354_anth_req.json") as f:
    data = json.load(f)

msgs = data.get("messages", [])
print("Total messages:", len(msgs))

for i, m in enumerate(msgs[:5]):
    role = m.get("role", "?")
    content = m.get("content", "")
    if isinstance(content, list):
        types = [b.get("type","?") for b in content]
        print("  [%d] %s %s" % (i, role, types))
    else:
        print("  [%d] %s %s" % (i, role, repr(content)[:80]))
print("  ...")
for i in range(max(5, len(msgs)-5), len(msgs)):
    m = msgs[i]
    role = m.get("role", "?")
    content = m.get("content", "")
    if isinstance(content, list):
        types = [b.get("type","?") for b in content]
        print("  [%d] %s %s" % (i, role, types))
    else:
        print("  [%d] %s %s" % (i, role, repr(content)[:80]))

# Find orphan tool_result at boundary
print("\n--- Boundary analysis ---")
for i in range(len(msgs)):
    m = msgs[i]
    role = m.get("role", "?")
    content = m.get("content", "")
    has_tool_result = False
    has_tool_use = False
    if isinstance(content, list):
        for b in content:
            if b.get("type") == "tool_result":
                has_tool_result = True
            if b.get("type") == "tool_use":
                has_tool_use = True
    if i < 5 or i >= len(msgs) - 5:
        marker = " <<< HEAD" if i < 2 else " <<< TAIL START"
    else:
        marker = ""
    if has_tool_result or has_tool_use:
        print("  [%d] %s tool_result=%s tool_use=%s%s" % (i, role, has_tool_result, has_tool_use, marker))