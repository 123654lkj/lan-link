use std::io::Write;
use std::process::Command;

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

fn grep_walk(p: &std::path::Path, pat: &str, out: &mut String, t: &mut usize, ln: bool, cnt: bool) {
    if let Ok(entries) = std::fs::read_dir(p) {
        for entry in entries.filter_map(|e| e.ok()) {
            if entry.path().is_dir() {
                grep_walk(&entry.path(), pat, out, t, ln, cnt);
            } else if let Ok(c) = std::fs::read_to_string(&entry.path()) {
                let mut fm = 0usize;

                for (i, l) in c.lines().enumerate() {
                    if l.contains(pat) {
                        fm += 1;
                        if !cnt {
                            let _line = if ln {
                                format!("{}:{}:{}\n", entry.path().display(), i + 1, l)
                            } else {
                                format!("{}:{}\n", entry.path().display(), l)
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

pub fn cmd_find(path: &str, name: &Option<String>, type_: &Option<String>, maxdepth: u32) -> (Vec<u8>, Option<i32>) {
    let mut cmd = Command::new("/usr/bin/find");
    cmd.arg(path);
    if maxdepth > 0 { cmd.arg("-maxdepth").arg(format!("{}", maxdepth)); }
    if let Some(n) = name { cmd.arg("-name").arg(n); }
    if let Some(t) = type_ { cmd.arg("-type").arg(t); }
    match cmd.output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("find error: {}\n", e).into_bytes(), Some(1)),
    }
}

pub fn cmd_tree(path: &str, depth: u32, dirs_only: bool) -> (Vec<u8>, Option<i32>) {
    let mut cmd = Command::new("/usr/bin/tree");
    if depth > 0 { cmd.arg("-L").arg(format!("{}", depth)); }
    if dirs_only { cmd.arg("-d"); }
    cmd.arg(path);
    match cmd.output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("tree error: {}\n", e).into_bytes(), Some(1)),
    }
}

pub fn cmd_du(path: &str, summarize: bool, maxdepth: u32) -> (Vec<u8>, Option<i32>) {
    let mut cmd = Command::new("/usr/bin/du");
    cmd.arg("-h");
    if summarize { cmd.arg("-s"); }
    if maxdepth > 0 { cmd.arg("--max-depth").arg(format!("{}", maxdepth)); }
    cmd.arg(path);
    match cmd.output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("du error: {}\n", e).into_bytes(), Some(1)),
    }
}

pub fn cmd_df(human: bool) -> (Vec<u8>, Option<i32>) {
    let mut cmd = Command::new("/usr/bin/df");
    if human { cmd.arg("-h"); }
    match cmd.output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("df error: {}\n", e).into_bytes(), Some(1)),
    }
}

pub fn cmd_mount() -> (Vec<u8>, Option<i32>) {
    match Command::new("/usr/bin/mount").output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("mount error: {}\n", e).into_bytes(), Some(1)),
    }
}

pub fn cmd_mkdir(recursive: bool, paths: &[String]) -> (Vec<u8>, Option<i32>) {
    let mut cmd = Command::new("/usr/bin/mkdir");
    if recursive { cmd.arg("-p"); }
    for p in paths { cmd.arg(p); }
    match cmd.output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("mkdir error: {}\n", e).into_bytes(), Some(1)),
    }
}

pub fn cmd_rm(recursive: bool, force: bool, paths: &[String]) -> (Vec<u8>, Option<i32>) {
    let mut cmd = Command::new("/usr/bin/rm");
    if recursive { cmd.arg("-r"); }
    if force { cmd.arg("-f"); }
    for p in paths { cmd.arg(p); }
    match cmd.output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("rm error: {}\n", e).into_bytes(), Some(1)),
    }
}

pub fn cmd_mv(src: &str, dest: &str) -> (Vec<u8>, Option<i32>) {
    match Command::new("/usr/bin/mv").arg(src).arg(dest).output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("mv error: {}\n", e).into_bytes(), Some(1)),
    }
}

pub fn cmd_cp(recursive: bool, src: &str, dest: &str) -> (Vec<u8>, Option<i32>) {
    let mut cmd = Command::new("/usr/bin/cp");
    if recursive { cmd.arg("-r"); }
    cmd.arg(src).arg(dest);
    match cmd.output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("cp error: {}\n", e).into_bytes(), Some(1)),
    }
}

pub fn cmd_chmod(mode: &str, paths: &[String]) -> (Vec<u8>, Option<i32>) {
    let mut cmd = Command::new("/usr/bin/chmod");
    cmd.arg(mode);
    for p in paths { cmd.arg(p); }
    match cmd.output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("chmod error: {}\n", e).into_bytes(), Some(1)),
    }
}

pub fn cmd_chown(owner: &str, paths: &[String]) -> (Vec<u8>, Option<i32>) {
    let mut cmd = Command::new("/usr/bin/chown");
    cmd.arg(owner);
    for p in paths { cmd.arg(p); }
    match cmd.output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("chown error: {}\n", e).into_bytes(), Some(1)),
    }
}

pub fn cmd_diff(file1: &str, file2: &str) -> (Vec<u8>, Option<i32>) {
    match Command::new("/usr/bin/diff").arg("-u").arg(file1).arg(file2).output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("diff error: {}\n", e).into_bytes(), Some(1)),
    }
}

pub fn cmd_wc(lines: bool, words: bool, paths: &[String]) -> (Vec<u8>, Option<i32>) {
    let mut cmd = Command::new("/usr/bin/wc");
    if lines { cmd.arg("-l"); }
    if words { cmd.arg("-w"); }
    if !lines && !words { cmd.arg("-l"); }
    for p in paths { cmd.arg(p); }
    match cmd.output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("wc error: {}\n", e).into_bytes(), Some(1)),
    }
}

pub fn cmd_checksum(path: &str, algorithm: &str) -> (Vec<u8>, Option<i32>) {
    use sha2::Digest;
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(e) => return (format!("{}: {}\n", path, e).into_bytes(), Some(1)),
    };
    match algorithm {
        "md5" | "md5sum" => {
            let hex_str = format!("{:x}", md5::Md5::digest(&data));
            (format!("{}  {}\n", hex_str, path).into_bytes(), Some(0))
        }
        _ => {
            let mut h = sha2::Sha256::new();
            h.update(&data);
            (
                format!("{:x}  {}\n", h.finalize(), path).into_bytes(),
                Some(0),
            )
        }
    }
}
