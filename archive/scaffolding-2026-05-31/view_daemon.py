import pathlib
p = pathlib.Path(r'G:\codex-AI-tools\lan-link\crates\daemon\src\main.rs')
lines = p.read_text('utf-8').splitlines()
for i in range(25, 35):
    print(f'{i+1}: {lines[i]}')
print('---')
for i in range(180, 195):
    print(f'{i+1}: {lines[i]}')