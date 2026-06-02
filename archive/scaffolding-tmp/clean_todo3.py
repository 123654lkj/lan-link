with open(r"G:\codex-AI-tools\todo.md", "r", encoding="utf-8") as f:
    src = f.read()
# Remove the truly non-LL item: EchoBird "新增 MiniMax-M3 LOCAL 入口" (EchoBird is user's own proxy config, not LL)
# Keep everything else
src = src.replace(
    "- [x] EchoBird 新增 MiniMax-M3 LOCAL 入口 (走星渡 Anthropic 路径) — 2026-06-01\n",
    ""
)
with open(r"G:\codex-AI-tools\todo.md", "w", encoding="utf-8", newline="\n") as f:
    f.write(src)
print("done")