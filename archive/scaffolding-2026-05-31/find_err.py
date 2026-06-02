import pathlib
p = pathlib.Path(r'G:\codex-AI-tools\lan-link\crates\ctl\src\main.rs')
lines = p.read_text('utf-8').splitlines()
for i in range(395, 410):
    print(f"{i+1}: {lines[i]}")