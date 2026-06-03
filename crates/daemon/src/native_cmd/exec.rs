// exec module

pub fn cmd_watch_fn(cmds: &[String]) -> (Vec<u8>, Option<i32>) {
    let joined = cmds.join(" ");
    match std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(&joined)
        .output()
    {
        Ok(o) => {
            let mut buf = o.stdout;
            if !o.stderr.is_empty() {
                if !buf.is_empty() {
                    buf.push(b'\n');
                }
                buf.extend_from_slice(&o.stderr);
            }
            (buf, o.status.code())
        }
        Err(e) => (format!("Watch error: {}\n", e).into_bytes(), Some(-1)),
    }
}

pub fn cmd_batch_content(lines: &[String]) -> (Vec<u8>, Option<i32>) {
    (lines.join("\n").into_bytes(), Some(0))
}

pub fn cmd_sed(path: &str, pattern: &str, replacement: &str) -> (Vec<u8>, Option<i32>) {
    // 路径安全检查：拒绝包含 .. 的路径
    if path.contains("..") {
        return (b"sed: path must not contain '..'\n".to_vec(), Some(1));
    }

    let new_content = match std::fs::read_to_string(path) {
        Ok(c) => c.replace(pattern, replacement),
        Err(e) => return (format!("read error: {}\n", e).into_bytes(), Some(1)),
    };
    match std::fs::write(path, &new_content) {
        Ok(_) => (b"ok\n".to_vec(), Some(0)),
        Err(e) => (format!("write error: {}\n", e).into_bytes(), Some(1)),
    }
}

pub fn cmd_pkill(name: &str, signal: u32) -> (Vec<u8>, Option<i32>) {
    match std::process::Command::new("/usr/bin/pkill")
        .arg(format!("-{}", signal)).arg("-f").arg(name)
        .output()
    {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("pkill error: {}\n", e).into_bytes(), Some(1)),
    }
}

pub fn cmd_shell_exec(cmd: &str) -> (Vec<u8>, Option<i32>) {
    match std::process::Command::new("/bin/sh")
        .arg("-c").arg(cmd)
        .output()
    {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("shell error: {}\n", e).into_bytes(), Some(1)),
    }
}
