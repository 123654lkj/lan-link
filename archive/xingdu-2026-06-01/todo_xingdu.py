content = open(r"G:\codex-AI-tools\todo.md", "r", encoding="utf-8").read()
content = content.replace(
    "- [ ] lan-link: Windows 端 input hook 长驻（用 client_win.py 已实现 WH_KEYBOARD_LL/WH_MOUSE_LL）",
    "- [x] lan-link: 验证星渡代理有效性 — 2026-06-01 (300+ req/resp 对全链路通, MiniMax Code Plan 支持 cache 命中)\n- [x] lan-link: 查 MiniMax Code Plan cache 规则 — 2026-06-01 (cache 写入 1.25x 输入价, 读取 0.1-0.2x, TTL 5 分钟, 5h 窗口按请求数, 周限额按 token 积分)\n- [ ] lan-link: Windows 端 input hook 长驻（用 client_win.py 已实现 WH_KEYBOARD_LL/WH_MOUSE_LL）"
)
with open(r"G:\codex-AI-tools\todo.md", "w", encoding="utf-8", newline="\n") as f:
    f.write(content)
print("done")