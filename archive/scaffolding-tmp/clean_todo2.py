with open(r"G:\codex-AI-tools\todo.md", "r", encoding="utf-8") as f:
    src = f.read()

# Remove explicitly non-LL lines: Codex, EchoBird (the proxy itself, not star/xingdu), WeChat, MCP
# Keep lan-link + star/xingdu
import re
lines = src.split("\n")
out = []
for line in lines:
    is_unchecked = line.lstrip().startswith("- [ ]")
    is_xingdu = ("star" in line.lower() or "xingdu" in line.lower() or "minimax" in line.lower())
    is_ll = "lan-link" in line.lower() or is_xingdu
    # Remove only if unchecked AND non-LL
    # But the unchecked MiniMax JSON parse item is "lan-link: 验证代理有效性" related? No, it's about Minimax
    # Actually "修复 MiniMax 偶发 JSON 解析错误" is about the agent's MiniMax backend, not lan-link.
    # The user said "只管 LL 的项目别做别的", so JSON parse is NOT lan-link. Remove.
    if is_unchecked and not is_ll and not is_xingdu:
        continue
    out.append(line)

with open(r"G:\codex-AI-tools\todo.md", "w", encoding="utf-8", newline="\n") as f:
    f.write("\n".join(out))
print("cleaned")