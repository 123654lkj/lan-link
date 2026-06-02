import pathlib
p = pathlib.Path(r'G:\codex-AI-tools\lan-link\crates\daemon\src\main.rs')
lines = p.read_text('utf-8').splitlines()
for i, line in enumerate(lines):
    if 'handle_input_linux' in line:
        print(f'{i+1}: {line}')