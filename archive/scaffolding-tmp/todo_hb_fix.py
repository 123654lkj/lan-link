content = open(r"G:\codex-AI-tools\todo.md", "r", encoding="utf-8").read()
content = content.replace(
    "- [x] lan-link: VBS 启动器 + Startup + watchdog — 2026-06-01 (VBS 放 C:\\Users\\26063\\AppData\\...\\Startup\\, Register-ScheduledTask 用户级任务每 2min 检查拉起)",
    "- [x] lan-link: VBS 启动器 + Startup + watchdog — 2026-06-01 (VBS 放 C:\\Users\\26063\\AppData\\...\\Startup\\, Register-ScheduledTask 用户级任务每 2min 检查拉起)\n- [x] lan-link: 前台 + --daemon input 长驻 70s 稳定验证 — 2026-06-01 (Python 进程 T+5s~T+70s 持续 alive, daemon 端 SYN+Hello 收到无 timeout, 30s timeout 在 client 死 20-30s 后正确触发)"
)
with open(r"G:\codex-AI-tools\todo.md", "w", encoding="utf-8", newline="\n") as f:
    f.write(content)
print("done")