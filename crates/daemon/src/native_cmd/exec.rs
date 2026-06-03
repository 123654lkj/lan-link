use super::system::read_proc;
use std::process::Command;

pub fn cmd_uptime() -> (Vec<u8>, Option<i32>) {
    let s = read_proc("/proc/uptime");

    let secs: f64 = s
        .split_whitespace()
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);

    let d = (secs / 86400.0) as u64;

    let h = ((secs % 86400.0) / 3600.0) as u64;

    let m = ((secs % 3600.0) / 60.0) as u64;

    (
        format!("up {} days, {:02}:{:02}\n", d, h, m).into_bytes(),
        Some(0),
    )
}

pub fn cmd_watch_fn(_interval_secs: u64, cmds: &[String]) -> (Vec<u8>, Option<i32>) {
    let joined = cmds.join(" ");
    match Command::new("/bin/sh")
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
