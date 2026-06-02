import pathlib
p = pathlib.Path(r'G:\codex-AI-tools\lan-link\crates\ctl\src\main.rs')
t = p.read_text('utf-8')
# Fix the ps chain: make all branches String
old = 'let c = if tree { \"ps auxf\" } else if full { \"ps aux\" }\n                else if let Some(u) = user { format!(\"ps -u {}\", esc(\u0026u)) }\n                else { \"ps -eo pid,user,%cpu,%mem,vsz,rss,tty,stat,start,time,comm\".to_string() }'
new = 'let c = if tree { \"ps auxf\".to_string() } else if full { \"ps aux\".to_string() }\n                else if let Some(u) = user { format!(\"ps -u {}\", esc(\u0026u)) }\n                else { \"ps -eo pid,user,%cpu,%mem,vsz,rss,tty,stat,start,time,comm\".to_string() }'
t = t.replace(old, new)
p.write_text(t, 'utf-8')
print('done')