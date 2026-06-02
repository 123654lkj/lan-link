import re
with open(r"G:\codex-AI-tools\todo.md", "r", encoding="utf-8") as f:
    src = f.read()

# Remove all non-LL items from the todo. Keep only lan-link and star (xingdu).
# Remove lines starting with "- [ ] " that are NOT lan-link and NOT xingdu-related
non_ll_keywords = ["MCP", "Codex", "EchoBird", "WeChat", "wxauto", "WeChatFerry", "json", "JSON", "Qwen", "Anthropic", "O-A-O", "OAO"]
# Actually for xingdu, only "star" or "xingdu" matches.
# The "MiniMax 偶发 JSON" item, "EchoBird", "Codex", "WeChat" all need to go.

lines = src.split("\n")
out = []
for line in lines:
    # Always keep headers
    if line.startswith("#") or not line.strip():
        out.append(line)
        continue
    # Remove non-LL lines: starts with - [ ] (unchecked items)
    is_unchecked = line.lstrip().startswith("- [ ]")
    is_ll = "lan-link" in line.lower() or "star" in line.lower() or "xingdu" in line.lower() or "minimax" in line.lower()
    if is_unchecked and not is_ll:
        continue
    out.append(line)

with open(r"G:\codex-AI-tools\todo.md", "w", encoding="utf-8", newline="\n") as f:
    f.write("\n".join(out))
print("cleaned")