use crate::native_cmd::helper::run_cmd;
use std::io::Write;

pub fn cmd_ls(path: &str, long: bool, all: bool) -> (Vec<u8>, Option<i32>) {
    let dir = match std::fs::read_dir(path) {
        Ok(d) => d,
        Err(e) => return (format!("ls: {}: {}\n", path, e).into_bytes(), Some(2)),
    };

    let mut entries: Vec<_> = dir.filter_map(|e| e.ok()).collect();

    entries.sort_by_key(|e| e.file_name());

    let mut out = Vec::new();

    for e in &entries {
        let name = e.file_name().to_string_lossy().to_string();

        if !all && name.starts_with('.') {
            continue;
        }

        if long {
            #[cfg(target_os = "linux")]
            {
                use std::os::unix::fs::MetadataExt;

                if let Ok(m) = e.metadata() {
                    let ft = if m.is_dir() {
                        'd'
                    } else if m.is_symlink() {
                        'l'
                    } else {
                        '-'
                    };

                    let _ = writeln!(
                        out,
                        "{}{:04o} {:>3} {:>8} {:>8} {:>8} {} {}",
                        ft,
                        m.mode() & 0o7777,
                        m.nlink(),
                        m.uid(),
                        m.gid(),
                        m.size(),
                        "",
                        name
                    );
                } else {
                    let _ = writeln!(out, "{}", name);
                }
            }

            #[cfg(not(target_os = "linux"))]
            {
                let _ = writeln!(out, "{}", name);
            }
        } else {
            let _ = writeln!(out, "{}", name);
        }
    }

    (out, Some(0))
}

pub fn cmd_cat(path: &str) -> (Vec<u8>, Option<i32>) {
    match std::fs::read(path) {
        Ok(d) => (d, Some(0)),
        Err(e) => (format!("cat: {}: {}\n", path, e).into_bytes(), Some(1)),
    }
}

pub fn cmd_head(path: &str, lines: u32) -> (Vec<u8>, Option<i32>) {
    let c = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => return (format!("head: {}: {}\n", path, e).into_bytes(), Some(1)),
    };

    (
        c.lines()
            .take(lines as usize)
            .flat_map(|l| [l, "\n"])
            .collect::<String>()
            .into_bytes(),
        Some(0),
    )
}

pub fn cmd_tail(path: &str, n: u32) -> (Vec<u8>, Option<i32>) {
    let c = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => return (format!("tail: {}: {}\n", path, e).into_bytes(), Some(1)),
    };

    let l: Vec<&str> = c.lines().collect();

    let start = if (n as usize) >= l.len() {
        0
    } else {
        l.len() - n as usize
    };

    (
        l[start..]
            .iter()
            .flat_map(|l| [*l, "\n"])
            .collect::<String>()
            .into_bytes(),
        Some(0),
    )
}

pub fn cmd_stat(path: &str) -> (Vec<u8>, Option<i32>) {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;

        if let Ok(m) = std::fs::metadata(path) {
            let mut out = format!("  File: {}\n", path);

            out += &format!(
                "  Size: {}    Blocks: {}    IO Block: {}\n",
                m.len(),
                m.blocks(),
                m.blksize()
            );

            out += &format!(
                "Device: {}/{}    Inode: {}    Links: {}\n",
                m.dev(),
                m.rdev(),
                m.ino(),
                m.nlink()
            );

            out += &format!(
                "Access: ({:04o}/{:?})  Uid: {}  Gid: {}\n",
                m.mode() & 0o7777,
                m.permissions(),
                m.uid(),
                m.gid()
            );

            return (out.into_bytes(), Some(0));
        }
    }

    (
        format!(
            "  File: {}\n  (metadata unavailable on this platform)\n",
            path
        )
        .into_bytes(),
        Some(1),
    )
}

pub fn cmd_grep(
    pattern: &str,
    path: &str,
    recursive: bool,
    ln: bool,
    cnt: bool,
) -> (Vec<u8>, Option<i32>) {
    if !recursive {
        let c = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => return (format!("grep: {}: {}\n", path, e).into_bytes(), Some(2)),
        };

        let mut out = String::new();
        let mut m = 0usize;

        for (i, l) in c.lines().enumerate() {
            if l.contains(pattern) {
                m += 1;
                if !cnt {
                    out += &if ln {
                        format!("{}:{}:{}\n", path, i + 1, l)
                    } else {
                        format!("{}\n", l)
                    };
                }
            }
        }

        return (
            if cnt {
                format!("{}:{}\n", path, m).into_bytes()
            } else {
                out.into_bytes()
            },
            Some(if m > 0 { 0 } else { 1 }),
        );
    }

    let mut out = String::new();
    let mut total = 0usize;

    grep_walk(
        std::path::Path::new(path),
        pattern,
        &mut out,
        &mut total,
        ln,
        cnt,
    );

    (
        if cnt {
            format!("{}\n", total).into_bytes()
        } else {
            out.into_bytes()
        },
        Some(if total > 0 { 0 } else { 1 }),
    )
}

pub fn grep_walk(p: &std::path::Path, pat: &str, out: &mut String, t: &mut usize, ln: bool, cnt: bool) {
    let mut stack = vec![p.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if let Ok(c) = std::fs::read_to_string(&path) {
                    let mut fm = 0usize;
                    for (i, l) in c.lines().enumerate() {
                        if l.contains(pat) {
                            fm += 1;
                            if !cnt {
                                let _line = if ln {
                                    format!("{}:{}:{}\n", path.display(), i + 1, l)
                                } else {
                                    format!("{}:{}\n", path.display(), l)
                                };
                                out.push_str(&_line);
                            }
                        }
                    }
                    *t += fm;
                }
            }
        }
    }
}

pub fn cmd_df(human: bool) -> (Vec<u8>, Option<i32>) {
    run_cmd("/usr/bin/df", if human { &["-h"] } else { &[] })
}

pub fn cmd_find(path: &str, name: &Option<String>, type_: &Option<String>, maxdepth: u32) -> (Vec<u8>, Option<i32>) {
    let mut args: Vec<String> = vec![path.to_string()];
    if maxdepth > 0 {
        args.push("-maxdepth".to_string());
        args.push(format!("{}", maxdepth));
    }
    if let Some(n) = name {
        args.push("-name".to_string());
        args.push(n.clone());
    }
    if let Some(t) = type_ {
        args.push("-type".to_string());
        args.push(t.clone());
    }
    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_cmd("/usr/bin/find", &args_ref)
}

pub fn cmd_tree(path: &str, depth: u32, dirs_only: bool) -> (Vec<u8>, Option<i32>) {
    let mut args: Vec<String> = vec![];
    if depth > 0 {
        args.push("-L".to_string());
        args.push(format!("{}", depth));
    }
    if dirs_only { args.push("-d".to_string()); }
    args.push(path.to_string());
    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_cmd("/usr/bin/tree", &args_ref)
}

pub fn cmd_du(path: &str, summarize: bool, maxdepth: u32) -> (Vec<u8>, Option<i32>) {
    let mut args: Vec<String> = vec!["-h".to_string()];
    if summarize { args.push("-s".to_string()); }
    if maxdepth > 0 {
        args.push("--max-depth".to_string());
        args.push(format!("{}", maxdepth));
    }
    args.push(path.to_string());
    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_cmd("/usr/bin/du", &args_ref)
}

pub fn cmd_wc(lines: bool, words: bool, paths: &[String]) -> (Vec<u8>, Option<i32>) {
    let mut args = vec![];
    if lines { args.push("-l"); }
    if words { args.push("-w"); }
    if !lines && !words { args.push("-l"); }
    for p in paths { args.push(p); }
    run_cmd("/usr/bin/wc", &args)
}

pub fn cmd_cp(recursive: bool, src: &str, dest: &str) -> (Vec<u8>, Option<i32>) {
    let mut args = vec![];
    if recursive { args.push("-r"); }
    args.push(src);
    args.push(dest);
    run_cmd("/usr/bin/cp", &args)
}

pub fn cmd_mv(src: &str, dest: &str) -> (Vec<u8>, Option<i32>) {
    run_cmd("/usr/bin/mv", &[src, dest])
}

pub fn cmd_rm(recursive: bool, force: bool, paths: &[String]) -> (Vec<u8>, Option<i32>) {
    let mut args = vec![];
    if recursive { args.push("-r"); }
    if force { args.push("-f"); }
    for p in paths { args.push(p); }
    run_cmd("/usr/bin/rm", &args)
}

pub fn cmd_mkdir(recursive: bool, paths: &[String]) -> (Vec<u8>, Option<i32>) {
    let mut args = vec![];
    if recursive { args.push("-p"); }
    for p in paths { args.push(p); }
    run_cmd("/usr/bin/mkdir", &args)
}

pub fn cmd_chmod(mode: &str, paths: &[String]) -> (Vec<u8>, Option<i32>) {
    let mut args = vec![mode];
    for p in paths { args.push(p); }
    run_cmd("/usr/bin/chmod", &args)
}

pub fn cmd_chown(owner: &str, paths: &[String]) -> (Vec<u8>, Option<i32>) {
    let mut args = vec![owner];
    for p in paths { args.push(p); }
    run_cmd("/usr/bin/chown", &args)
}

pub fn cmd_diff(file1: &str, file2: &str) -> (Vec<u8>, Option<i32>) {
    run_cmd("/usr/bin/diff", &["-u", file1, file2])
}

pub fn cmd_touch(path: &str) -> (Vec<u8>, Option<i32>) {
    match std::fs::write(path, "") {
        Ok(_) => (b"ok\n".to_vec(), Some(0)),
        Err(e) => (format!("touch error: {}\n", e).into_bytes(), Some(1)),
    }
}

pub fn cmd_write_file(path: &str, data: &[u8], append: bool) -> (Vec<u8>, Option<i32>) {
    let mut opts = std::fs::OpenOptions::new();
    if append { opts.append(true); } else { opts.write(true).create(true).truncate(true); }
    match opts.open(path) {
        Ok(mut f) => {
            match f.write_all(data) {
                Ok(_) => (b"ok\n".to_vec(), Some(0)),
                Err(e) => (format!("write error: {}\n", e).into_bytes(), Some(1)),
            }
        }
        Err(e) => (format!("open error: {}\n", e).into_bytes(), Some(1)),
    }
}

pub fn cmd_read_file(path: &str) -> (Vec<u8>, Option<i32>) {
    match std::fs::read_to_string(path) {
        Ok(c) => (c.into_bytes(), Some(0)),
        Err(e) => (format!("read error: {}\n", e).into_bytes(), Some(1)),
    }
}
