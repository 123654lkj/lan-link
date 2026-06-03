//! Native command implementations

//!

//! Each command is executed on the daemon using controlled std::process::Command.

//! No shell wrappers (sh -c). Truly native commands read /proc directly.

//! Results returned as (output_bytes, exit_code).


use lan_link_protocol::frame::NativeCmdType;

use std::io::Write;

use std::net::TcpStream;

use std::process::Command;
fn read_proc(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

// ─── Truly native commands (Linux /proc) ─────────────────────────

fn cmd_uptime() -> (Vec<u8>, Option<i32>) {
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

fn cmd_hostname() -> (Vec<u8>, Option<i32>) {
    (read_proc("/proc/sys/kernel/hostname").trim().to_string().into_bytes(), Some(0))
}

fn cmd_free(human: bool) -> (Vec<u8>, Option<i32>) {
    let mem = read_proc("/proc/meminfo");

    let kv = |k: &str| -> u64 {
        mem.lines()
            .find(|l| l.starts_with(k))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    };

    let mt = kv("MemTotal:");
    let mf = kv("MemFree:");
    let ma = kv("MemAvailable:");
    // /proc/meminfo values are in kB, convert to bytes for hfmt
    let mt = mt * 1024;
    let mf = mf * 1024;
    let ma = ma * 1024;

    let mu = mt.saturating_sub(ma);

    let st = kv("SwapTotal:");
    let sf = kv("SwapFree:");
    let st = st * 1024;
    let sf = sf * 1024;
    let su = st.saturating_sub(sf);

    let mut out = String::from("              Mem:\n");

    if human {
        out += &format!(
            "{:>8} {:>8} {:>8} {:>8}\n",
            "total", "used", "free", "avail"
        );

        out += &format!(
            "{:>8} {:>8} {:>8} {:>8}\n",
            hfmt(mt),
            hfmt(mu),
            hfmt(mf),
            hfmt(ma)
        );

        out += &format!(
            "              Swap: {:>8} {:>8} {:>8}\n",
            hfmt(st),
            hfmt(su),
            hfmt(sf)
        );
    } else {
        out += &format!(
            "{:>12} {:>12} {:>12} {:>12}\n",
            "total", "used", "free", "avail"
        );

        out += &format!("{:>12} {:>12} {:>12} {:>12}\n", mt, mu, mf, ma);

        out += &format!("              Swap: {:>12} {:>12} {:>12}\n", st, su, sf);
    }

    (out.into_bytes(), Some(0))
}

fn hfmt(s: u64) -> String {
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

fn cmd_cpu() -> (Vec<u8>, Option<i32>) {
    let info = read_proc("/proc/cpuinfo");

    let mut out = String::new();

    for line in info.lines() {
        if line.starts_with("processor")
            || line.starts_with("model name")
            || line.starts_with("cpu MHz")
            || line.starts_with("cache size")
        {
            out += line;
            out += "\n";
        }
    }

    (out.into_bytes(), Some(0))
}

fn cmd_uname(all: bool, release: bool, machine: bool) -> (Vec<u8>, Option<i32>) {
    if all {
        let p = [
            read_proc("/proc/sys/kernel/ostype").trim().to_string(),
            read_proc("/proc/sys/kernel/hostname").trim().to_string(),
            read_proc("/proc/sys/kernel/osrelease").trim().to_string(),
            read_proc("/proc/sys/kernel/version").trim().to_string(),
            read_proc("/proc/sys/kernel/arch").trim().to_string(),
        ];

        return (
            format!("{} {} {} {} {}\n", p[0], p[1], p[2], p[3], p[4]).into_bytes(),
            Some(0),
        );
    }

    if release {
        return (
            read_proc("/proc/sys/kernel/osrelease").into_bytes(),
            Some(0),
        );
    }

    if machine {
        return (read_proc("/proc/sys/kernel/arch").into_bytes(), Some(0));
    }

    (read_proc("/proc/sys/kernel/ostype").into_bytes(), Some(0))
}

fn cmd_whoami() -> (Vec<u8>, Option<i32>) {
    match std::env::var("USER").or_else(|_| std::env::var("LOGNAME")) {
        Ok(u) if !u.is_empty() => (format!("{}\n", u).into_bytes(), Some(0)),
        _ => (format!("unknown\n").into_bytes(), Some(0)),
    }
}

fn cmd_ps(full: bool, user: Option<String>, tree: bool) -> (Vec<u8>, Option<i32>) {
    if tree {
        let t = cmd_top_snapshot();
        return (t.into_bytes(), Some(0));
    }

    let dir = match std::fs::read_dir("/proc") {
        Ok(d) => d,
        Err(_) => return (b"ps: /proc not available\n".to_vec(), Some(1)),
    };

    let h = if full {
        "USER       PID %CPU %MEM    VSZ    RSS TTY      STAT   START   TIME COMMAND\n"
    } else {
        "  PID TTY          TIME CMD\n"
    };

    let mut out = String::from(h);

    for e in dir.filter_map(|e| e.ok()) {
        let pid: u32 = match e.file_name().to_string_lossy().parse() {
            Ok(p) => p,
            _ => continue,
        };

        let status = read_proc(&format!("/proc/{}/status", pid));

        let cl = read_proc(&format!("/proc/{}/cmdline", pid));

        let st = status
            .lines()
            .find(|l| l.starts_with("State:"))
            .and_then(|l| l.split_whitespace().nth(1))
            .unwrap_or("?");

        let pn = status
            .lines()
            .find(|l| l.starts_with("Name:"))
            .and_then(|l| l.split('\t').nth(1))
            .unwrap_or("")
            .trim();

        let cmd = if cl.is_empty() {
            format!("[{}]", pn)
        } else {
            cl.replace('\0', " ")
        };

        if !full {
            out += &format!("{:>5} {:<8} {} {}\n", pid, "?", "00:00:00", cmd.trim());
        } else {
            let stat = read_proc(&format!("/proc/{}/stat", pid));

            let f: Vec<&str> = stat.split_whitespace().collect();

            if let Some(ref u) = user {
                if pn != u.as_str() {
                    continue;
                }
            }

            out += &format!(
                "{:<8} {:>5} {:>5} {:>5} {:>7} {:>7} {:<8} {:<9} {:<10} {}\n",
                "?",
                pid,
                "0.0",
                "0.0",
                f.get(22).unwrap_or(&"0"),
                f.get(23).unwrap_or(&"0"),
                "?",
                st,
                "00:00:00",
                cmd.trim()
            );
        }
    }

    (out.into_bytes(), Some(0))
}

fn cmd_info() -> (Vec<u8>, Option<i32>) {
    let mut out = String::new();

    let (u, _) = cmd_uname(true, false, false);
    out += &String::from_utf8_lossy(&u);
    out += "---\n";

    let cpu = read_proc("/proc/cpuinfo");

    if let Some(l) = cpu.lines().find(|l| l.starts_with("model name")) {
        out += l;
        out += "\n";
    }

    out += "---\n";

    let (f, _) = cmd_free(true);
    out += &String::from_utf8_lossy(&f);
    out += "---\n";

    let (uf, _) = cmd_uptime();
    out += &String::from_utf8_lossy(&uf);

    (out.into_bytes(), Some(0))
}

fn cmd_ls(path: &str, long: bool, all: bool) -> (Vec<u8>, Option<i32>) {
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

fn cmd_cat(path: &str) -> (Vec<u8>, Option<i32>) {
    match std::fs::read(path) {
        Ok(d) => (d, Some(0)),
        Err(e) => (format!("cat: {}: {}\n", path, e).into_bytes(), Some(1)),
    }
}

fn cmd_head(path: &str, lines: u32) -> (Vec<u8>, Option<i32>) {
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

fn cmd_tail(path: &str, n: u32) -> (Vec<u8>, Option<i32>) {
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

fn cmd_stat(path: &str) -> (Vec<u8>, Option<i32>) {
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

fn cmd_grep(
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

fn cmd_arp() -> (Vec<u8>, Option<i32>) {
    (read_proc("/proc/net/arp").into_bytes(), Some(0))
}

fn cmd_netstat(tcp: bool, udp: bool, listening: bool) -> (Vec<u8>, Option<i32>) {
    let mut out = String::new();

    if tcp || (!tcp && !udp) {
        out += "Proto Recv-Q Send-Q Local Address           Foreign Address         State\n";

        for line in read_proc("/proc/net/tcp").lines().skip(1) {
            let p: Vec<&str> = line.split_whitespace().collect();
            if p.len() < 4 {
                continue;
            }

            let s = match u8::from_str_radix(p[3], 16).unwrap_or(0) {
                10 => "LISTEN",
                1 => "ESTABLISHED",
                _ => "OTHER",
            };

            if listening && s != "LISTEN" {
                continue;
            }

            out += &format!(
                "tcp    0      0 {:<22} {:<22} {}\n",
                hex2addr(p[1]),
                hex2addr(p[2]),
                s
            );
        }
    }

    if udp {
        out += "\nProto Recv-Q Send-Q Local Address           Foreign Address         State\n";

        for line in read_proc("/proc/net/udp").lines().skip(1) {
            let p: Vec<&str> = line.split_whitespace().collect();
            if p.len() < 4 {
                continue;
            }

            out += &format!(
                "udp    0      0 {:<22} {:<22} {}\n",
                hex2addr(p[1]),
                hex2addr(p[2]),
                "ESTABLISHED"
            );
        }
    }

    (out.into_bytes(), Some(0))
}

fn hex2addr(s: &str) -> String {
    let p: Vec<&str> = s.split(':').collect();

    if p.len() < 2 {
        return s.to_string();
    }

    let port = u16::from_str_radix(p[1], 16).unwrap_or(0);

    if p[0].len() == 8 {
        let b: Vec<u8> = (0..4)
            .map(|i| u8::from_str_radix(&p[0][i * 2..i * 2 + 2], 16).unwrap_or(0))
            .collect();

        format!("{}.{}.{}.{}:{}", b[3], b[2], b[1], b[0], port)
    } else {
        format!("[{}]:{}", p[0], port)
    }
}

fn cmd_top_snapshot() -> String {
    let mut out = String::from("top - native\n\n");

    let load = read_proc("/proc/loadavg");

    out += &format!(
        "load average: {}\n\n",
        load.split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ")
    );

    let mut total = 0u32;
    let mut running = 0u32;

    if let Ok(dir) = std::fs::read_dir("/proc") {
        for e in dir.filter_map(|e| e.ok()) {
            if e.file_name().to_string_lossy().parse::<u32>().is_ok() {
                total += 1;

                let s = read_proc(&format!("/proc/{}/status", e.file_name().to_string_lossy()));

                if s.lines()
                    .find(|l| l.starts_with("State:"))
                    .map(|l| l.contains("running"))
                    .unwrap_or(false)
                {
                    running += 1;
                }
            }
        }
    }

    out += &format!("Tasks: {} total, {} running\n", total, running);

    let mem = read_proc("/proc/meminfo");

    let kv = |k: &str| -> u64 {
        mem.lines()
            .find(|l| l.starts_with(k))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    };

    let mt = kv("MemTotal:") / 1024;
    let mf = kv("MemFree:") / 1024;
    let ma = kv("MemAvailable:") / 1024;

    let mu = mt.saturating_sub(ma);

    out += &format!(
        "MiB Mem: {:>8.1} total, {:>8.1} free, {:>8.1} used, {:>8.1} avail\n",
        mt as f64 / 1024.0,
        mf as f64 / 1024.0,
        mu as f64 / 1024.0,
        ma as f64 / 1024.0
    );

    out += "\n  PID USER      PR  NI    VIRT    RES    SHR S  %CPU  %MEM     TIME+ COMMAND\n";

    let mut ps = Vec::new();

    if let Ok(dir) = std::fs::read_dir("/proc") {
        for e in dir.filter_map(|e| e.ok()) {
            let pid: u32 = match e.file_name().to_string_lossy().parse() {
                Ok(p) => p,
                _ => continue,
            };

            let stat = read_proc(&format!("/proc/{}/stat", pid));

            let f: Vec<&str> = stat.split_whitespace().collect();

            if f.len() < 24 {
                continue;
            }

            let ut: u64 = f[13].parse().unwrap_or(0);
            let st: u64 = f[14].parse().unwrap_or(0);

            let vsz: u64 = f[22].parse().unwrap_or(0);
            let rss: u64 = f[23].parse().unwrap_or(0);

            ps.push((pid, (ut + st) as f64, vsz, rss * 4));
        }
    }

    ps.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    for (pid, _, vsz, rss) in ps.iter().take(20) {
        let cl = read_proc(&format!("/proc/{}/cmdline", pid));

        let cmd = if cl.is_empty() {
            format!("[{}]", pid)
        } else {
            cl.replace('\0', " ")
                .split_whitespace()
                .next()
                .unwrap_or("?")
                .to_string()
        };

        out += &format!(
            "{:>5} ?        20   0 {:>7} {:>7} ?    S   0.0  0.0   0:00.00 {}\n",
            pid,
            vsz / 1024,
            rss / 1024,
            cmd
        );
    }

    out
}

fn cmd_portscan(host: &str, start: u16, end: u16) -> (Vec<u8>, Option<i32>) {
    let mut out = format!("Scanning ports {}-{} on {}\n", start, end, host);

    for port in start..=end {
        if let Ok(addr) = format!("{}:{}", host, port).parse::<std::net::SocketAddr>() {
            if TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(200)).is_ok() {
                out += &format!("Port {} is open\n", port);
            }
        }
    }

    (out.into_bytes(), Some(0))
}

// ─── Main dispatch ──────────────────────────────────────────────

pub fn run_native_cmd(cmd: &NativeCmdType) -> (Vec<u8>, Option<i32>) {
    match cmd {
        NativeCmdType::Ls { path, long, all } => cmd_ls(path, *long, *all),

        NativeCmdType::Cat { path } => cmd_cat(path),

        NativeCmdType::Head { path, lines } => cmd_head(path, *lines),

        NativeCmdType::Tail { path, lines, .. } => cmd_tail(path, *lines),

        NativeCmdType::Stat { path } => cmd_stat(path),

        NativeCmdType::Uptime => cmd_uptime(),

        NativeCmdType::Hostname => cmd_hostname(),

        NativeCmdType::Free { human } => cmd_free(*human),

        NativeCmdType::Cpu => cmd_cpu(),

        NativeCmdType::Uname {
            all,
            release,
            machine,
        } => cmd_uname(*all, *release, *machine),

        NativeCmdType::Whoami => cmd_whoami(),

        NativeCmdType::Ps { full, user, tree } => cmd_ps(*full, user.clone(), *tree),

        NativeCmdType::Info => cmd_info(),

        NativeCmdType::Arp => cmd_arp(),

        NativeCmdType::Netstat {
            tcp,
            udp,
            listening,
            ..
        } => cmd_netstat(*tcp, *udp, *listening),

        NativeCmdType::PortScan {
            host,
            start_port,
            end_port,
            ..
        } => cmd_portscan(host, *start_port, *end_port),

        NativeCmdType::Top { .. } => (cmd_top_snapshot().into_bytes(), Some(0)),

        NativeCmdType::Grep {
            pattern,
            path,
            recursive,
            line_number,
            count,
        } => cmd_grep(pattern, path, *recursive, *line_number, *count),

        // ---- Management commands (native Rust) ----
        NativeCmdType::Dmesg { lines, .. } => cmd_dmesg(*lines),
        NativeCmdType::Lsblk => cmd_lsblk(),
        NativeCmdType::Mount => cmd_mount(),
        NativeCmdType::Who => cmd_who(),
        NativeCmdType::Kill { pid, signal } => cmd_kill(*pid, *signal),

        NativeCmdType::Pgrep { name, full, count } => cmd_pgrep(name, *full, *count),

        NativeCmdType::Dns { hostname, .. } => cmd_dns(hostname),
        NativeCmdType::Ssh => cmd_ssh_check(),
        NativeCmdType::Mkdir { recursive, paths } => cmd_mkdir(*recursive, paths),

        NativeCmdType::Rm {
            recursive,
            force,
            paths,
        } => cmd_rm(*recursive, *force, paths),

        NativeCmdType::Mv { src, dest } => cmd_mv(src, dest),
        NativeCmdType::Cp {
            recursive,
            src,
            dest,
        } => cmd_cp(*recursive, src, dest),
        NativeCmdType::Chmod { mode, paths } => cmd_chmod(mode, paths),

        NativeCmdType::Chown { owner, paths } => cmd_chown(owner, paths),

        NativeCmdType::Diff { file1, file2 } => cmd_diff(file1, file2),
        NativeCmdType::Wc {
            lines,
            words,
            paths,
        } => cmd_wc(*lines, *words, paths),

        NativeCmdType::Find {
            path,
            name,
            type_,
            maxdepth,
        } => cmd_find(path, &name, &type_, *maxdepth),

        NativeCmdType::Tree {
            path,
            depth,
            dirs_only,
        } => cmd_tree(path, *depth, *dirs_only),

        NativeCmdType::Du {
            path,
            summarize,
            maxdepth,
        } => cmd_du(path, *summarize, *maxdepth),

        NativeCmdType::Df { human, .. } => cmd_df(*human),
        NativeCmdType::Last { lines } => cmd_last(*lines),
        NativeCmdType::Ip { .. } => cmd_ip(),
        // ── Management ──
        NativeCmdType::Service { action } => cmd_service(action),
        NativeCmdType::Journal {
            unit,
            lines,
            priority,
            since,
            follow,
        } => cmd_journal(
            unit.as_deref(),
            *lines,
            priority.as_deref(),
            since.as_deref(),
            *follow,
        ),
        NativeCmdType::Pkg { action } => cmd_pkg(action),
        NativeCmdType::Docker { action } => cmd_docker(action),
        NativeCmdType::Crontab { action } => cmd_crontab(action),
        NativeCmdType::Firewall { backend } => cmd_firewall(backend),
        NativeCmdType::Checksum { path, algorithm } => cmd_checksum(path, algorithm),

        NativeCmdType::BatchContent { lines, .. } => (lines.join("\n").into_bytes(), Some(0)),

        NativeCmdType::Watch { interval_secs, cmd } => cmd_watch_fn(*interval_secs, cmd),
        NativeCmdType::WriteFile { path, data, append } => {
            let mut opts = std::fs::OpenOptions::new();
            if *append { opts.append(true); } else { opts.write(true).create(true).truncate(true); }
            match opts.open(path) {
                Ok(mut f) => {
                    use std::io::Write;
                    match f.write_all(data) {
                        Ok(_) => (b"ok\n".to_vec(), Some(0)),
                        Err(e) => (format!("write error: {}\n", e).into_bytes(), Some(1)),
                    }
                }
                Err(e) => (format!("open error: {}\n", e).into_bytes(), Some(1)),
            }
        }
        NativeCmdType::Sed { path, pattern, replacement, .. } => {
            let new_content = match std::fs::read_to_string(path) {
                Ok(c) => c.replace(pattern.as_str(), replacement.as_str()),
                Err(e) => return (format!("read error: {}\n", e).into_bytes(), Some(1)),
            };
            match std::fs::write(path, &new_content) {
                Ok(_) => (b"ok\n".to_vec(), Some(0)),
                Err(e) => (format!("write error: {}\n", e).into_bytes(), Some(1)),
            }

        }
        NativeCmdType::ReadFile { path } => {
            match std::fs::read_to_string(path) {
                Ok(c) => (c.into_bytes(), Some(0)),
                Err(e) => (format!("read error: {}\n", e).into_bytes(), Some(1)),
            }
        }
        NativeCmdType::Touch { path } => {
            match std::fs::write(path, "") {
                Ok(_) => (b"ok
".to_vec(), Some(0)),
                Err(e) => (format!("touch error: {}
", e).into_bytes(), Some(1)),
            }
        }
        NativeCmdType::Pkill { name, signal } => {
            match std::process::Command::new("/usr/bin/pkill")
                .arg("-f").arg(signal.to_string()).arg(name)
                .output()
            {
                Ok(o) => (o.stdout, o.status.code()),
                Err(e) => (format!("pkill error: {}
", e).into_bytes(), Some(1)),
            }
        }
        NativeCmdType::ShellExec { cmd, .. } => {
            match std::process::Command::new("/bin/sh")
                .arg("-c").arg(cmd)
                .output()
            {
                Ok(o) => (o.stdout, o.status.code()),
                Err(e) => (format!("shell error: {}
", e).into_bytes(), Some(1)),
            }
        }
    }
}

// ---- Native implementations for management commands ----

fn cmd_checksum(path: &str, algorithm: &str) -> (Vec<u8>, Option<i32>) {
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

fn cmd_dmesg(_lines: u32) -> (Vec<u8>, Option<i32>) {
    let mut text = String::new();
    if let Ok(mut f) = std::fs::File::open("/dev/kmsg") {
        use std::io::Read;
        let mut buf = [0u8; 8192];
        for _ in 0..128 {
            match f.read(&mut buf) {
                Ok(n) if n > 0 => {
                    text.push_str(&String::from_utf8_lossy(&buf[..n]));
                }
                _ => break,
            }
        }
    }
    if text.is_empty() {
        if let Ok(log) = std::fs::read_to_string("/var/log/kern.log") {
            text = log;
        }
    }
    if text.is_empty() {
        if let Ok(syslog) = std::fs::read_to_string("/var/log/syslog") {
            let mut krn = String::new();
            for line in syslog.lines() {
                if line.contains("kernel:") {
                    krn.push_str(line);
                    krn.push('\n');
                }
            }
            text = krn;
        }
    }
    if _lines > 0 {
        let lv: Vec<&str> = text.lines().collect();
        if lv.len() > _lines as usize {
            text = lv[lv.len() - _lines as usize..].join("\n") + "\n";
        }
    }
    if text.is_empty() {
        (b"dmesg: kernel log not available\n".to_vec(), Some(1))
    } else {
        (text.into_bytes(), Some(0))
    }
}

fn cmd_lsblk() -> (Vec<u8>, Option<i32>) {
    let mut out = String::from("NAME    MAJ:MIN RM   SIZE RO TYPE MOUNTPOINT\n");
    if let Ok(blocks) = std::fs::read_dir("/sys/block") {
        for entry in blocks.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();
            let size = std::fs::read_to_string(entry.path().join("size"))
                .unwrap_or_default()
                .trim()
                .parse::<u64>()
                .unwrap_or(0);
            let ro = std::fs::read_to_string(entry.path().join("ro"))
                .unwrap_or_default()
                .trim()
                .to_string();
            let ss = if size == 0 {
                "0".into()
            } else if size >= 2097152 {
                format!("{:.1}G", size as f64 * 512.0 / 1073741824.0)
            } else if size >= 2048 {
                format!("{:.1}M", size as f64 * 512.0 / 1048576.0)
            } else {
                format!("{:.1}K", size as f64 * 512.0 / 1024.0)
            };
            out.push_str(&format!(
                "{:<8} 0:0    {} {:>7} {} disk\n",
                name, "0", ss, ro
            ));
        }
    }
    (out.into(), Some(0))
}

fn cmd_ip() -> (Vec<u8>, Option<i32>) {
    let mut out = "1: lo: <LOOPBACK> mtu 65536\n    inet 127.0.0.1/8 scope host lo\n".to_string();
    if let Ok(nets) = std::fs::read_dir("/sys/class/net") {
        for entry in nets.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "lo" {
                continue;
            }
            let addr = std::fs::read_to_string(entry.path().join("address"))
                .unwrap_or_default()
                .trim()
                .to_string();
            let mtu = std::fs::read_to_string(entry.path().join("mtu"))
                .unwrap_or_default()
                .trim()
                .to_string();
            let st = std::fs::read_to_string(entry.path().join("operstate"))
                .unwrap_or_default()
                .trim()
                .to_string();
            let fl = if st == "up" { "UP,LOWER_UP" } else { "DOWN" };
            out.push_str(&format!(
                "2: {}: <{}> mtu {}\n    link/ether {}\n",
                name, fl, mtu, addr
            ));
        }
    }
    (out.into(), Some(0))
}

fn cmd_watch_fn(_interval_secs: u64, cmds: &[String]) -> (Vec<u8>, Option<i32>) {
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

fn cmd_service(action: &lan_link_protocol::frame::ServiceActionType) -> (Vec<u8>, Option<i32>) {
    use lan_link_protocol::frame::ServiceActionType;
    match action {
        ServiceActionType::List { active, failed } => {
            let mut units = Vec::new();
            for dir in [
                "/etc/systemd/system",
                "/lib/systemd/system",
                "/run/systemd/system",
            ] {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.filter_map(|e| e.ok()) {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.ends_with(".service") {
                            units.push(name);
                        }
                    }
                }
            }
            units.sort();
            units.dedup();
            let mut out = "  UNIT                    LOAD   ACTIVE SUB\n".to_string();
            for svc in &units {
                let n = svc.trim_end_matches(".service");
                let run = std::fs::read_dir("/proc")
                    .ok()
                    .map(|d| {
                        d.filter_map(|e| e.ok()).any(|e| {
                            if let Ok(p) = e.file_name().to_string_lossy().parse::<u32>() {
                                std::fs::read_to_string(format!("/proc/{}/cmdline", p))
                                    .ok()
                                    .map(|c| c.contains(n))
                                    .unwrap_or(false)
                            } else {
                                false
                            }
                        })
                    })
                    .unwrap_or(false);
                if *failed {
                    continue;
                } else if *active {
                    if run {
                        out.push_str(&format!("  {:<20} loaded active running\n", svc));
                    }
                } else {
                    if run {
                        out.push_str(&format!("  {:<20} loaded active running\n", svc));
                    } else {
                        out.push_str(&format!("  {:<20} loaded inactive dead\n", svc));
                    }
                }
            }
            if *failed && out.lines().count() <= 1 {
                out.push_str("0 units listed.\n");
            }
            (out.into_bytes(), Some(0))
        }
        ServiceActionType::Status { name } => {
            let c = name.trim_end_matches(".service");
            let sf = format!("{}.service", c);
            let mut out = String::new();
            let mut found = false;
            for p in &[
                format!("/etc/systemd/system/{}", sf),
                format!("/lib/systemd/system/{}", sf),
            ] {
                if let Ok(content) = std::fs::read_to_string(p) {
                    out.push_str(&format!("* {} - ", sf));
                    for line in content.lines() {
                        if let Some(d) = line.strip_prefix("Description=") {
                            out.push_str(d);
                            break;
                        }
                    }
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                    out.push_str(&format!("     Loaded: loaded ({})\n", p));
                    found = true;
                    break;
                }
            }
            if !found {
                out.push_str(&format!("Unit {}.service could not be found.\n", c));
                return (out.into_bytes(), Some(1));
            }
            let run = std::fs::read_dir("/proc")
                .ok()
                .map(|d| {
                    d.filter_map(|e| e.ok()).any(|e| {
                        if let Ok(p) = e.file_name().to_string_lossy().parse::<u32>() {
                            std::fs::read_to_string(format!("/proc/{}/cmdline", p))
                                .ok()
                                .map(|cmdline| cmdline.contains(c))
                                .unwrap_or(false)
                        } else {
                            false
                        }
                    })
                })
                .unwrap_or(false);
            if run {
                out.push_str("     Active: active (running)\n");
            } else {
                out.push_str("     Active: inactive (dead)\n");
            }
            if let Ok(dir) = std::fs::read_dir("/proc") {
                for entry in dir.filter_map(|e| e.ok()) {
                    if let Ok(p) = entry.file_name().to_string_lossy().parse::<u32>() {
                        if let Ok(cl) = std::fs::read_to_string(format!("/proc/{}/cmdline", p)) {
                            if cl.contains(c) {
                                out.push_str(&format!("   Main PID: {}\n", p));
                                break;
                            }
                        }
                    }
                }
            }
            (out.into_bytes(), Some(0))
        }
        ServiceActionType::Start { name } => {
            match std::process::Command::new("/usr/bin/systemctl")
                .arg("start")
                .arg(name)
                .output()
            {
                Ok(o) => (o.stdout, o.status.code()),
                Err(e) => (format!("systemctl error: {}\n", e).into_bytes(), Some(1)),
            }
        }
        ServiceActionType::Stop { name } => {
            match std::process::Command::new("/usr/bin/systemctl")
                .arg("stop")
                .arg(name)
                .output()
            {
                Ok(o) => (o.stdout, o.status.code()),
                Err(e) => (format!("systemctl error: {}\n", e).into_bytes(), Some(1)),
            }
        }
        ServiceActionType::Restart { name } => {
            match std::process::Command::new("/usr/bin/systemctl")
                .arg("restart")
                .arg(name)
                .output()
            {
                Ok(o) => (o.stdout, o.status.code()),
                Err(e) => (format!("systemctl error: {}\n", e).into_bytes(), Some(1)),
            }
        }
        ServiceActionType::Reload { name } => {
            match std::process::Command::new("/usr/bin/systemctl")
                .arg("reload")
                .arg(name)
                .output()
            {
                Ok(o) => (o.stdout, o.status.code()),
                Err(e) => (format!("systemctl error: {}\n", e).into_bytes(), Some(1)),
            }
        }
        ServiceActionType::Enable { name } => {
            let w = "/etc/systemd/system/multi-user.target.wants";
            let s = [
                format!("/etc/systemd/system/{}", name),
                format!("/lib/systemd/system/{}", name),
            ]
            .iter()
            .find(|p| std::path::Path::new(p).exists())
            .cloned();
            match s {
                Some(src) => {
                    let link = format!("{}/{}", w, name);
                    #[cfg(target_os = "linux")]
                    {
                        let _ = std::fs::create_dir_all(w);
                        let _ = std::os::unix::fs::symlink(&src, &link);
                    }
                    (
                        format!("Created symlink {} -> {}\n", link, src).into_bytes(),
                        Some(0),
                    )
                }
                None => (
                    format!("Unit file for {} not found\n", name).into_bytes(),
                    Some(1),
                ),
            }
        }
        ServiceActionType::Disable { name } => {
            let link = format!("/etc/systemd/system/multi-user.target.wants/{}", name);
            match std::fs::remove_file(&link) {
                Ok(_) => (format!("Removed symlink {}\n", link).into_bytes(), Some(0)),
                Err(e) => (
                    format!("Failed to remove symlink: {}\n", e).into_bytes(),
                    Some(1),
                ),
            }
        }
    }
}

fn cmd_journal(
    unit: Option<&str>,
    lines: u32,
    priority: Option<&str>,
    _since: Option<&str>,
    _follow: bool,
) -> (Vec<u8>, Option<i32>) {
    let mut out = String::new();
    for path in &["/var/log/syslog", "/var/log/messages"] {
        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines() {
                if let Some(u) = unit {
                    if !line.contains(u.trim_end_matches(".service")) {
                        continue;
                    }
                }
                if let Some(mp) = priority.and_then(|p| match p {
                    "0" | "emerg" => Some(0),
                    "1" | "alert" => Some(1),
                    "2" | "crit" => Some(2),
                    "3" | "err" => Some(3),
                    "4" | "warning" => Some(4),
                    "5" | "notice" => Some(5),
                    "6" | "info" => Some(6),
                    "7" | "debug" => Some(7),
                    _ => None,
                }) {
                    let hp = if mp <= 3 {
                        line.contains("error") || line.contains("emerg") || line.contains("crit")
                    } else if mp <= 5 {
                        line.contains("warn") || line.contains("notice")
                    } else {
                        true
                    };
                    if !hp {
                        continue;
                    }
                }
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    if lines > 0 {
        let lv: Vec<&str> = out.lines().collect();
        if lv.len() > lines as usize {
            out = lv[lv.len() - lines as usize..].join("\n") + "\n";
        }
    }
    if out.is_empty() {
        (b"No journal/log data available\n".to_vec(), Some(0))
    } else {
        (out.into_bytes(), Some(0))
    }
}

fn cmd_pkg(action: &lan_link_protocol::frame::PkgActionType) -> (Vec<u8>, Option<i32>) {
    use lan_link_protocol::frame::PkgActionType;
    match action {
        PkgActionType::List { .. } => {
            let mut out = String::from(
                "Desired=Unknown/Install/Remove/Purge/Hold\n| Status=Not/Inst/Conf-files/Unpacked/halF-conf/Half-inst/trig-aWait/Trig-pend\n|/ Err?=(none)/Reinst-required (Status,Err: uppercase=bad)\n||/ Name                 Version          Architecture Description\n+++-====================-================-============-============================================\n",
            );
            if let Ok(status) = std::fs::read_to_string("/var/lib/dpkg/status") {
                let (mut pn, mut pv, mut pa, mut pd, mut ps) = (
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                );
                for line in status.lines() {
                    if line.starts_with("Package: ") {
                        pn = line[9..].to_string();
                    } else if line.starts_with("Version: ") {
                        pv = line[9..].to_string();
                    } else if line.starts_with("Architecture: ") {
                        pa = line[14..].to_string();
                    } else if line.starts_with("Description: ") {
                        pd = line[13..].to_string();
                    } else if line.starts_with("Status: ") {
                        ps = line[8..].to_string();
                    } else if line.is_empty() && !pn.is_empty() {
                        let flag = if ps.starts_with("install ok installed") {
                            "ii"
                        } else if ps.contains("not-installed") {
                            "un"
                        } else {
                            "rc"
                        };
                        out.push_str(&format!(
                            "{:<5} {:<20} {:<16} {:<12} {}\n",
                            flag, pn, pv, pa, pd
                        ));
                        pn.clear();
                    }
                }
            } else {
                out.push_str("(dpkg database not found: /var/lib/dpkg/status)\n");
            }
            (out.into_bytes(), Some(0))
        }
        PkgActionType::Search { query } => {
            let mut out = String::new();
            if let Ok(status) = std::fs::read_to_string("/var/lib/dpkg/status") {
                let (mut pn, mut pd) = (String::new(), String::new());
                for line in status.lines() {
                    if line.starts_with("Package: ") {
                        pn = line[9..].to_string();
                    } else if line.starts_with("Description: ") {
                        pd = line[13..].to_string();
                    } else if line.is_empty() && !pn.is_empty() {
                        if pn.contains(query) || pd.contains(query) {
                            out.push_str(&format!("{} - {}\n", pn, pd));
                        }
                        pn.clear();
                    }
                }
            }
            if out.is_empty() {
                out = format!("No packages found matching '{}'\n", query);
            }
            (out.into_bytes(), Some(0))
        }
        PkgActionType::Install { name } => {
            match std::process::Command::new("/usr/bin/apt-get")
                .args(&["install", "-y", name])
                .output()
            {
                Ok(o) => {
                    let mut b = o.stdout;
                    if !o.stderr.is_empty() {
                        b.extend_from_slice(&o.stderr);
                    }
                    (b, o.status.code())
                }
                Err(e) => (format!("apt-get error: {}\n", e).into_bytes(), Some(1)),
            }
        }
        PkgActionType::Remove { name } => {
            match std::process::Command::new("/usr/bin/apt-get")
                .args(&["remove", "-y", name])
                .output()
            {
                Ok(o) => {
                    let mut b = o.stdout;
                    if !o.stderr.is_empty() {
                        b.extend_from_slice(&o.stderr);
                    }
                    (b, o.status.code())
                }
                Err(e) => (format!("apt-get error: {}\n", e).into_bytes(), Some(1)),
            }
        }
        PkgActionType::Update => {
            match std::process::Command::new("/usr/bin/apt-get")
                .arg("update")
                .output()
            {
                Ok(o) => {
                    let mut b = o.stdout;
                    if !o.stderr.is_empty() {
                        b.extend_from_slice(&o.stderr);
                    }
                    (b, o.status.code())
                }
                Err(e) => (format!("apt-get error: {}\n", e).into_bytes(), Some(1)),
            }
        }
        PkgActionType::Upgrade => {
            match std::process::Command::new("/usr/bin/apt-get")
                .args(&["upgrade", "-y"])
                .output()
            {
                Ok(o) => {
                    let mut b = o.stdout;
                    if !o.stderr.is_empty() {
                        b.extend_from_slice(&o.stderr);
                    }
                    (b, o.status.code())
                }
                Err(e) => (format!("apt-get error: {}\n", e).into_bytes(), Some(1)),
            }
        }
    }
}

fn cmd_docker(action: &lan_link_protocol::frame::DockerActionType) -> (Vec<u8>, Option<i32>) {
    use lan_link_protocol::frame::DockerActionType;
    fn docker_api(method: &str, path: &str, body: Option<&str>) -> (Vec<u8>, Option<i32>) {
        for sock in &["/var/run/docker.sock", "/run/docker.sock"] {
            if !std::path::Path::new(sock).exists() {
                continue;
            }
            use std::io::{Read, Write};
            #[cfg(target_os = "linux")] if let Ok(mut stream) = std::os::unix::net::UnixStream::connect(sock) {
            #[cfg(not(target_os = "linux"))] { let _ = sock; continue; }
                let req = if let Some(b) = body {
                    format!(
                        "{} {} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                        method,
                        path,
                        b.len(),
                        b
                    )
                } else {
                    format!("{} {} HTTP/1.1\r\nHost: localhost\r\n\r\n", method, path)
                };
                if stream.write_all(req.as_bytes()).is_err() {
                    continue;
                }
                let (mut resp, mut buf) = (Vec::new(), [0u8; 4096]);
                loop {
                    match stream.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => resp.extend_from_slice(&buf[..n]),
                        Err(_) => break,
                    }
                }
                let s = String::from_utf8_lossy(&resp);
                if let Some(bs) = s.find("\r\n\r\n") {
                    let code = s
                        .lines()
                        .next()
                        .unwrap_or("")
                        .split_whitespace()
                        .nth(1)
                        .and_then(|c| c.parse::<u32>().ok())
                        .unwrap_or(500);
                    return (
                        format!("HTTP {}: {}\n", code, &s[bs + 4..]).into_bytes(),
                        Some(if code < 400 { 0 } else { 1 }),
                    );
                }
                return (resp, Some(0));
            }
        }
        (
            b"Docker socket not found at /var/run/docker.sock\n".to_vec(),
            Some(1),
        )
    }
    match action {
        DockerActionType::Ps { all, .. } => docker_api(
            "GET",
            if *all {
                "/containers/json?all=true"
            } else {
                "/containers/json"
            },
            None,
        ),
        DockerActionType::Logs { name, tail, .. } => docker_api(
            "GET",
            &format!(
                "/containers/{}/logs?stderr=true&stdout=true&tail={}",
                name, tail
            ),
            None,
        ),
        DockerActionType::Stats { .. } => docker_api("GET", "/containers/json?all=true", None),
        DockerActionType::Exec { container, cmd, .. } => {
            let body = format!(r#"{{\"AttachStdin\":false,\"AttachStdout\":true,\"AttachStderr\":true,\"Cmd\":{:?}}}"#, cmd).to_string();
            let (cr, _) = docker_api(
                "POST",
                &format!("/containers/{}/exec", container),
                Some(&body),
            );
            let s = String::from_utf8_lossy(&cr);
            if let Some(is) = s.find("\"Id\":\"") {
                let id_str = &s[is + 6..];
                if let Some(ie) = id_str.find('\"') {
                    return docker_api(
                        "POST",
                        &format!("/exec/{}/start", &id_str[..ie]),
                        Some("{\"Detach\":false,\"Tty\":false}"),
                    );
                }
            }
            (cr, Some(1))
        }
        DockerActionType::Info => docker_api("GET", "/info", None),
        DockerActionType::Images => docker_api("GET", "/images/json", None),
        DockerActionType::Rm { container, force } => docker_api(
            "DELETE",
            &if *force {
                format!("/containers/{}?force=true", container)
            } else {
                format!("/containers/{}", container)
            },
            None,
        ),
        DockerActionType::Control { container, action } => docker_api(
            "POST",
            &format!("/containers/{}/{}/", container, action),
            None,
        ),
    }
}

fn cmd_crontab(action: &lan_link_protocol::frame::CrontabActionType) -> (Vec<u8>, Option<i32>) {
    use lan_link_protocol::frame::CrontabActionType;
    use std::process::Command;
    match action {
        CrontabActionType::List => {
            let user = std::env::var("USER").unwrap_or_else(|_| "root".to_string());
            match std::fs::read_to_string(&format!("/var/spool/cron/crontabs/{}", user)) {
                Ok(c) => (c.into_bytes(), Some(0)),
                Err(_) => match std::fs::read_to_string("/etc/crontab") {
                    Ok(c) => {
                        let mut out = "# (from /etc/crontab, user crontab not found)\n".to_string();
                        out.push_str(&c);
                        (out.into_bytes(), Some(0))
                    }
                    Err(_) => (b"no crontab for this user\n".to_vec(), Some(1)),
                },
            }
        }
        CrontabActionType::Edit => (b"Use 'iexec crontab -e' for editing\n".to_vec(), Some(0)),
        CrontabActionType::Remove => {
            let user = std::env::var("USER").unwrap_or_else(|_| "root".to_string());
            match std::fs::remove_file(&format!("/var/spool/cron/crontabs/{}", user)) {
                Ok(_) => (b"crontab removed\n".to_vec(), Some(0)),
                Err(_) => (b"no crontab to remove\n".to_vec(), Some(1)),
            }
        }
    }
}

fn cmd_firewall(backend: &str) -> (Vec<u8>, Option<i32>) {
    match backend {
        "ufw" => {
            let mut out = "Status: ".to_string();
            if std::path::Path::new("/etc/ufw").exists() {
                out.push_str("active\n");
                for p in &["/etc/ufw/user.rules", "/etc/ufw/before.rules"] {
                    if let Ok(content) = std::fs::read_to_string(p) {
                        out.push_str(&format!("--- {} ---\n", p));
                        for line in content
                            .lines()
                            .filter(|l| !l.trim().starts_with("#") && !l.trim().is_empty())
                        {
                            out.push_str(line);
                            out.push_str("\n");
                        }
                    }
                }
            }
            if out.is_empty() {
                out = "(no iptables/nftables rules found via /proc)\n".to_string();
            }
            (out.into_bytes(), Some(0))
        }
        _ => {
            let mut out = String::new();
            for p in &["/proc/net/ip_tables_names", "/proc/net/ip6_tables_names"] {
                if let Ok(names) = std::fs::read_to_string(p) {
                    for table in names.lines() {
                        out.push_str(&format!("Table: {}\n", table));
                    }
                }
            }
            if std::path::Path::new("/etc/nftables.conf").exists() {
                if let Ok(conf) = std::fs::read_to_string("/etc/nftables.conf") {
                    out.push_str("--- /etc/nftables.conf ---\n");
                    for line in conf.lines().filter(|l| !l.trim().starts_with("#") && !l.trim().is_empty()) {
                        out.push_str(line);
                        out.push_str("\n");
                    }
                }
            }
            if out.is_empty() {
                out = "(no iptables/nftables rules found via /proc)\n".to_string();
            }
            (out.into_bytes(), Some(0))
        }
    }
}

// -- Native wrappers for sys commands (std::process::Command, not shell) --
fn cmd_mount() -> (Vec<u8>, Option<i32>) {
    match std::process::Command::new("/usr/bin/mount").output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("mount error: {}\n", e).into_bytes(), Some(1)),
    }
}

fn cmd_who() -> (Vec<u8>, Option<i32>) {
    match std::process::Command::new("/usr/bin/who").output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("who error: {}\n", e).into_bytes(), Some(1)),
    }
}

fn cmd_kill(pid: u32, signal: u32) -> (Vec<u8>, Option<i32>) {
    match std::process::Command::new("/usr/bin/kill")
        .arg(format!("-{}", signal))
        .arg(format!("{}", pid))
        .output()
    {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("kill error: {}\n", e).into_bytes(), Some(1)),
    }
}

fn cmd_pgrep(name: &str, full: bool, count: bool) -> (Vec<u8>, Option<i32>) {
    let mut cmd = std::process::Command::new("/usr/bin/pgrep");
    if full { cmd.arg("-f"); }
    if count { cmd.arg("-c"); }
    cmd.arg(name);
    match cmd.output() {
        Ok(o) => {
            let out = if o.status.success() || o.status.code() == Some(1) { o.stdout } else { o.stderr };
            (out, o.status.code())
        },
        Err(e) => (format!("pgrep error: {}\n", e).into_bytes(), Some(1)),
    }
}

fn cmd_dns(hostname: &str) -> (Vec<u8>, Option<i32>) {
    match std::process::Command::new("/usr/bin/getent")
        .arg("hosts").arg(hostname)
        .output()
    {
        Ok(o) => (o.stdout, o.status.code()),
        Err(_) => {
            match std::process::Command::new("/usr/bin/host").arg(hostname).output() {
                Ok(o2) => (o2.stdout, o2.status.code()),
                Err(e) => (format!("dns lookup error: {}\n", e).into_bytes(), Some(1)),
            }
        }
    }
}

fn cmd_ssh_check() -> (Vec<u8>, Option<i32>) {
    match std::process::Command::new("/usr/bin/ssh").arg("-V").output() {
        Ok(o) => {
            let ver = String::from_utf8_lossy(&o.stderr);
            (format!("SSH available: {}\n", ver.lines().next().unwrap_or("unknown")).into_bytes(), Some(0))
        },
        Err(e) => (format!("SSH not available: {}\n", e).into_bytes(), Some(1)),
    }
}

fn cmd_mkdir(recursive: bool, paths: &[String]) -> (Vec<u8>, Option<i32>) {
    let mut cmd = std::process::Command::new("/usr/bin/mkdir");
    if recursive { cmd.arg("-p"); }
    for p in paths { cmd.arg(p); }
    match cmd.output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("mkdir error: {}\n", e).into_bytes(), Some(1)),
    }
}

fn cmd_rm(recursive: bool, force: bool, paths: &[String]) -> (Vec<u8>, Option<i32>) {
    let mut cmd = std::process::Command::new("/usr/bin/rm");
    if recursive { cmd.arg("-r"); }
    if force { cmd.arg("-f"); }
    for p in paths { cmd.arg(p); }
    match cmd.output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("rm error: {}\n", e).into_bytes(), Some(1)),
    }
}

fn cmd_mv(src: &str, dest: &str) -> (Vec<u8>, Option<i32>) {
    match std::process::Command::new("/usr/bin/mv").arg(src).arg(dest).output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("mv error: {}\n", e).into_bytes(), Some(1)),
    }
}

fn cmd_cp(recursive: bool, src: &str, dest: &str) -> (Vec<u8>, Option<i32>) {
    let mut cmd = std::process::Command::new("/usr/bin/cp");
    if recursive { cmd.arg("-r"); }
    cmd.arg(src).arg(dest);
    match cmd.output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("cp error: {}\n", e).into_bytes(), Some(1)),
    }
}

fn cmd_chmod(mode: &str, paths: &[String]) -> (Vec<u8>, Option<i32>) {
    let mut cmd = std::process::Command::new("/usr/bin/chmod");
    cmd.arg(mode);
    for p in paths { cmd.arg(p); }
    match cmd.output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("chmod error: {}\n", e).into_bytes(), Some(1)),
    }
}

fn cmd_chown(owner: &str, paths: &[String]) -> (Vec<u8>, Option<i32>) {
    let mut cmd = std::process::Command::new("/usr/bin/chown");
    cmd.arg(owner);
    for p in paths { cmd.arg(p); }
    match cmd.output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("chown error: {}\n", e).into_bytes(), Some(1)),
    }
}

fn cmd_diff(file1: &str, file2: &str) -> (Vec<u8>, Option<i32>) {
    match std::process::Command::new("/usr/bin/diff").arg("-u").arg(file1).arg(file2).output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("diff error: {}\n", e).into_bytes(), Some(1)),
    }
}

fn cmd_wc(lines: bool, words: bool, paths: &[String]) -> (Vec<u8>, Option<i32>) {
    let mut cmd = std::process::Command::new("/usr/bin/wc");
    if lines { cmd.arg("-l"); }
    if words { cmd.arg("-w"); }
    if !lines && !words { cmd.arg("-l"); }
    for p in paths { cmd.arg(p); }
    match cmd.output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("wc error: {}\n", e).into_bytes(), Some(1)),
    }
}

fn cmd_find(path: &str, name: &Option<String>, type_: &Option<String>, maxdepth: u32) -> (Vec<u8>, Option<i32>) {
    let mut cmd = std::process::Command::new("/usr/bin/find");
    cmd.arg(path);
    if maxdepth > 0 { cmd.arg("-maxdepth").arg(format!("{}", maxdepth)); }
    if let Some(n) = name { cmd.arg("-name").arg(n); }
    if let Some(t) = type_ { cmd.arg("-type").arg(t); }
    match cmd.output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("find error: {}\n", e).into_bytes(), Some(1)),
    }
}

fn cmd_tree(path: &str, depth: u32, dirs_only: bool) -> (Vec<u8>, Option<i32>) {
    let mut cmd = std::process::Command::new("/usr/bin/tree");
    if depth > 0 { cmd.arg("-L").arg(format!("{}", depth)); }
    if dirs_only { cmd.arg("-d"); }
    cmd.arg(path);
    match cmd.output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("tree error: {}\n", e).into_bytes(), Some(1)),
    }
}

fn cmd_du(path: &str, summarize: bool, maxdepth: u32) -> (Vec<u8>, Option<i32>) {
    let mut cmd = std::process::Command::new("/usr/bin/du");
    cmd.arg("-h");
    if summarize { cmd.arg("-s"); }
    if maxdepth > 0 { cmd.arg("--max-depth").arg(format!("{}", maxdepth)); }
    cmd.arg(path);
    match cmd.output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("du error: {}\n", e).into_bytes(), Some(1)),
    }
}

fn cmd_df(human: bool) -> (Vec<u8>, Option<i32>) {
    let mut cmd = std::process::Command::new("/usr/bin/df");
    if human { cmd.arg("-h"); }
    match cmd.output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("df error: {}\n", e).into_bytes(), Some(1)),
    }
}

fn cmd_last(lines: u32) -> (Vec<u8>, Option<i32>) {
    let mut cmd = std::process::Command::new("/usr/bin/last");
    if lines > 0 { cmd.arg(format!("-{}", lines)); }
    match cmd.output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("last error: {}\n", e).into_bytes(), Some(1)),
    }
}

