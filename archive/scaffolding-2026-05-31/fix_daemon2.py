import pathlib
p = pathlib.Path(r'G:\codex-AI-tools\lan-link\crates\ctl\src\main.rs')
t = p.read_text('utf-8')
# Find handle_input_linux and add cfg guard before it
old = '\nfn handle_input_linux(data: &[u8], peer: SocketAddr) {'
new = '\n#[cfg(target_os = \"linux\")]\nfn handle_input_linux(data: &[u8], peer: SocketAddr) {'
# Only replace the definition one, not the call site which already has cfg
# The definition has 'fn handle_input_linux' preceded by '}'
# Find the second occurrence (the definition)
first = t.find('fn handle_input_linux')
second = t.find('fn handle_input_linux', first + 10)
if second != -1:
    # Find the newline before it
    nl = t.rfind('\n', 0, second)
    t = t[:nl] + '\n#[cfg(target_os = \"linux\")]' + t[nl:]
    p.write_text(t, 'utf-8')
    print('done')
else:
    print('not found')