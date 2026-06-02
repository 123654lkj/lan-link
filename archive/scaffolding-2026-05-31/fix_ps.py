import pathlib
p = pathlib.Path(r'G:\codex-AI-tools\lan-link\crates\ctl\src\main.rs')
t = p.read_text('utf-8')
t = t.replace('time,comm" }', 'time,comm".to_string() }')
p.write_text(t, 'utf-8')
print('done')