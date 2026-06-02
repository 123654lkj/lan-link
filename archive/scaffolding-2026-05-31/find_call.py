import pathlib
p = pathlib.Path(r'G:\codex-AI-tools\lan-link\crates\daemon\src\main.rs')
t = p.read_text('utf-8')

# Find where handle_input_linux is called
idx = t.find('handle_input_linux')
while idx != -1:
    start = max(0, idx - 200)
    end = idx + 100
    print(f'--- at {idx} ---')
    print(t[start:end])
    print()
    idx = t.find('handle_input_linux', idx + 1)