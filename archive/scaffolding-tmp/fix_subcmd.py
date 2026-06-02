with open(r"G:\codex-AI-tools\lan-link\client_win.py", "r", encoding="utf-8") as f:
    src = f.read()

# Make cmd optional, default None
src = src.replace(
    'sub = ap.add_subparsers(dest="cmd", required=True)',
    'sub = ap.add_subparsers(dest="cmd", required=False)'
)

with open(r"G:\codex-AI-tools\lan-link\client_win.py", "w", encoding="utf-8", newline="\n") as f:
    f.write(src)
print("done")