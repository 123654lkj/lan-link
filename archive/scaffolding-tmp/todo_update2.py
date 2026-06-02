import re
content = open(r"G:\codex-AI-tools\todo.md", "r", encoding="utf-8").read()
content = re.sub(r"> 上次更新: \d{4}-\d{2}-\d{2} \d{2}:\d{2}", "> 上次更新: 2026-06-01 13:54", content)
content = content.replace(
    "- [x] lan-link: 实现 input 转发（Win 键鼠 -> Linux 注入） — 2026-06-01 (端到端链路验证: Win client -> daemon解密 -> bincode反序列化 -> uinput注入 -> 团子X server接收)",
    "- [x] lan-link: 实现 input 转发（Win 键鼠 -> Linux 注入） — 2026-06-01 (v8: bytes=72 硬验证uinput写入成功，inject_total计数器，OnceLock单实例injector)"
)
# Add new active item
content = content.replace(
    "## 待办（还没开始）",
    """## 活跃（当前在做）
- [ ] lan-link: 写 input e2e 自动测试（启动 Xvfb + xdotool getmouselocation 验证光标坐标变化）
- [ ] lan-link: daemon 加 PID 文件防多实例
- [ ] lan-link: Windows 端 ctl 二进制编译（需 MSVC Build Tools）
- [ ] lan-link: Windows 端 input hook 长驻（用 client_win.py 已实现 WH_KEYBOARD_LL/WH_MOUSE_LL）

## 待办（还没开始）"""
)
with open(r"G:\codex-AI-tools\todo.md", "w", encoding="utf-8", newline="\n") as f:
    f.write(content)
print("Updated")