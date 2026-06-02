import pathlib
p = pathlib.Path(r'G:\codex-AI-tools\lan-link\crates\daemon\src\main.rs')
t = p.read_text('utf-8')
# Find handle_input_linux definition
idx = t.find('fn handle_input_linux')
print(t[idx-50:idx+200])