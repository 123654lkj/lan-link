content = open(r"G:\codex-AI-tools\todo.md", "r", encoding="utf-8").read()
content = content.replace(
    "- [ ] lan-link: Windows 端 input hook 长驻（用 client_win.py 已实现 WH_KEYBOARD_LL/WH_MOUSE_LL）",
    "- [x] lan-link: Windows 端 input hook 长驻基础设施 — 2026-06-01 (加 Config/Log/重连/心跳/--daemon 模式, 跑 input 子命令验证 daemon 端 SYN+Hello 全链路通)\n- [ ] lan-link: pyinstaller 打包成单 exe (lan-link-input.exe)\n- [ ] lan-link: VBS 启动器 + Startup 开机自启 + watchdog 重启"
)
with open(r"G:\codex-AI-tools\todo.md", "w", encoding="utf-8", newline="\n") as f:
    f.write(content)
print("done")