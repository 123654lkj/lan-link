# Update todo.md - mark some items as done
import re
content = open(r"G:\codex-AI-tools\todo.md", "r", encoding="utf-8").read()

# Update timestamp
content = re.sub(r"> 上次更新: \d{4}-\d{2}-\d{2} \d{2}:\d{2}", "> 上次更新: 2026-06-01 13:15", content)

# Mark the active items as done
content = content.replace(
    "- [ ] lan-link MVP: 编译 lan-linkctl Windows 二进制，测试 exec 命令",
    "- [x] lan-link MVP: 编译 lan-linkctl Windows 二进制，测试 exec 命令 — 2026-06-01 (client_win.py 验证通过)"
)
content = content.replace(
    "- [ ] lan-link: 实现 input 转发（Win 键鼠 -> Linux 注入）",
    "- [x] lan-link: 实现 input 转发（Win 键鼠 -> Linux 注入） — 2026-06-01 (端到端链路验证: Win client -> daemon解密 -> bincode反序列化 -> uinput注入 -> 团子X server接收)"
)

with open(r"G:\codex-AI-tools\todo.md", "w", encoding="utf-8", newline="\n") as f:
    f.write(content)
print("Updated todo.md")