import pathlib
p = pathlib.Path(r'G:\codex-AI-tools\lan-link\crates\daemon\src\main.rs')
lines = p.read_text('utf-8').splitlines()
# Line 199 is index 198 - insert cfg guard before it
lines.insert(198, '#[cfg(target_os = "linux")]')
p.write_text('\n'.join(lines), 'utf-8')
print('done')