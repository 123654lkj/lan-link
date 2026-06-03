use crate::native_cmd::helper::{read_proc, hfmt};

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

pub fn cmd_hostname() -> (Vec<u8>, Option<i32>) {
    (read_proc("/proc/sys/kernel/hostname").trim().to_string().into_bytes(), Some(0))
}

pub fn cmd_free(human: bool) -> (Vec<u8>, Option<i32>) {
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

pub fn cmd_cpu() -> (Vec<u8>, Option<i32>) {
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

pub fn cmd_uname(all: bool, release: bool, machine: bool) -> (Vec<u8>, Option<i32>) {
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

pub fn cmd_whoami() -> (Vec<u8>, Option<i32>) {
    match std::env::var("USER").or_else(|_| std::env::var("LOGNAME")) {
        Ok(u) if !u.is_empty() => (format!("{}\n", u).into_bytes(), Some(0)),
        _ => (format!("unknown\n").into_bytes(), Some(0)),
    }
}

pub fn cmd_ps(full: bool, user: Option<String>, tree: bool) -> (Vec<u8>, Option<i32>) {
    if tree {
        return cmd_top_snapshot();
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

    // Collect PIDs first
    let pids: Vec<u32> = dir
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_string_lossy().parse::<u32>().ok())
        .collect();

    // Parallel process using std::thread::scope (available since Rust 1.63)
    let mut results: Vec<(u32, String)> = Vec::with_capacity(pids.len());
    std::thread::scope(|s| {
        let chunk_size = (pids.len() + 3) / 4; // 4 parallel chunks
        let mut handles = Vec::new();
        for chunk in pids.chunks(chunk_size) {
            let chunk = chunk.to_vec();
            let user = user.clone();
            handles.push(s.spawn(move || {
                let mut local = Vec::new();
                for &pid in &chunk {
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
                    if full {
                        if let Some(ref u) = user {
                            if pn != u.as_str() {
                                continue;
                            }
                        }
                        let stat = read_proc(&format!("/proc/{}/stat", pid));
                        let f: Vec<&str> = stat.split_whitespace().collect();
                        local.push((pid, format!(
                            "{:<8} {:>5} {:>5} {:>5} {:>7} {:>7} {:<8} {:<9} {:<10} {}\n",
                            "?", pid, "0.0", "0.0",
                            f.get(22).unwrap_or(&"0"),
                            f.get(23).unwrap_or(&"0"),
                            "?", st, "00:00:00",
                            cmd.trim()
                        )));
                    } else {
                        local.push((pid, format!("{:>5} {:<8} {} {}\n", pid, "?", "00:00:00", cmd.trim())));
                    }
                }
                local
            }));
        }
        for handle in handles {
            results.extend(handle.join().unwrap());
        }
    });

    // Sort by PID for stable output
    results.sort_by_key(|(pid, _)| *pid);

    let mut out = String::from(h);
    for (_, line) in results {
        out += &line;
    }
    (out.into_bytes(), Some(0))
}

pub fn cmd_info() -> (Vec<u8>, Option<i32>) {
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

pub fn cmd_top_snapshot() -> (Vec<u8>, Option<i32>) {
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

    (out.into_bytes(), Some(0))
}

pub fn cmd_kill(pid: u32, signal: u32) -> (Vec<u8>, Option<i32>) {
    match std::process::Command::new("/usr/bin/kill")
        .arg(format!("-{}", signal))
        .arg(format!("{}", pid))
        .output()
    {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("kill error: {}\n", e).into_bytes(), Some(1)),
    }
}

pub fn cmd_pgrep(name: &str, full: bool, count: bool) -> (Vec<u8>, Option<i32>) {
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

pub fn cmd_mount() -> (Vec<u8>, Option<i32>) {
    match std::process::Command::new("/usr/bin/mount").output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("mount error: {}\n", e).into_bytes(), Some(1)),
    }
}

pub fn cmd_who() -> (Vec<u8>, Option<i32>) {
    match std::process::Command::new("/usr/bin/who").output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("who error: {}\n", e).into_bytes(), Some(1)),
    }
}

pub fn cmd_last(lines: u32) -> (Vec<u8>, Option<i32>) {
    let mut cmd = std::process::Command::new("/usr/bin/last");
    if lines > 0 { cmd.arg(format!("-{}", lines)); }
    match cmd.output() {
        Ok(o) => (o.stdout, o.status.code()),
        Err(e) => (format!("last error: {}\n", e).into_bytes(), Some(1)),
    }
}

pub fn cmd_dmesg(lines: u32) -> (Vec<u8>, Option<i32>) {
    let mut text = String::new();
    if let Ok(mut f) = std::fs::File::open("/dev/kmsg") {
        use std::io::Read;
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            let mut total = 0usize;
            let mut local = String::new();
            loop {
                match f.read(&mut buf) {
                    Ok(n) if n > 0 => {
                        total += n;
                        local.push_str(&String::from_utf8_lossy(&buf[..n]));
                        if total >= 65536 {
                            break; // 64KB cap
                        }
                    }
                    _ => break,
                }
            }
            let _ = tx.send(local);
        });
        if let Ok(s) = rx.recv_timeout(std::time::Duration::from_secs(2)) {
            text = s;
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
    if lines > 0 {
        let lv: Vec<&str> = text.lines().collect();
        if lv.len() > lines as usize {
            text = lv[lv.len() - lines as usize..].join("\n") + "\n";
        }
    }
    if text.is_empty() {
        (b"dmesg: kernel log not available\n".to_vec(), Some(1))
    } else {
        (text.into_bytes(), Some(0))
    }
}

pub fn cmd_lsblk() -> (Vec<u8>, Option<i32>) {
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

pub fn cmd_ip() -> (Vec<u8>, Option<i32>) {
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
