use std::process::Command;

pub fn read_proc(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

pub fn run_cmd(program: &str, args: &[&str]) -> (Vec<u8>, Option<i32>) {
    match Command::new(program).args(args).output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("{} error: {}\n", program, e).into_bytes(), Some(1)),
    }
}

pub fn hfmt(s: u64) -> String {
    if s < 1024 {
        return format!("{}B", s);
    }
    let kb = s / 1024;
    if kb < 1024 {
        return format!("{}K", kb);
    }
    let mb = kb / 1024;
    if mb < 1024 {
        return format!("{}.{}M", mb, (kb % 1024) * 10 / 1024);
    }
    format!("{}.{}G", mb / 1024, (mb % 1024) * 10 / 1024)
}
