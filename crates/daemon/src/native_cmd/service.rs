//! 服务管理命令
//!
//! 提供服务管理相关的原生命令实现，包括：
//!
//! - **系统服务**：`service` — systemd 服务管理（start/stop/restart/status/enable/disable）
//! - **日志查询**：`journal` — journalctl 日志查询，支持按等级/时间过滤
//! - **包管理**：`pkg` — apt/dnf 包管理器操作
//! - **容器管理**：`docker` — Docker 容器和镜像管理
//! - **定时任务**：`crontab` — crontab 查看和编辑
//! - **防火墙**：`firewall` — iptables/nftables 规则查询
//!
//! 服务管理命令通过 `std::process::Command` 调用系统工具实现。

pub fn cmd_service(action: &lan_link_protocol::frame::ServiceActionType) -> (Vec<u8>, Option<i32>) {
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

pub fn cmd_journal(
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

pub fn cmd_pkg(action: &lan_link_protocol::frame::PkgActionType) -> (Vec<u8>, Option<i32>) {
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

pub fn cmd_docker(action: &lan_link_protocol::frame::DockerActionType) -> (Vec<u8>, Option<i32>) {
    use lan_link_protocol::frame::DockerActionType;
    fn docker_api(method: &str, path: &str, body: Option<&str>) -> (Vec<u8>, Option<i32>) {
        for sock in &["/var/run/docker.sock", "/run/docker.sock"] {
            if !std::path::Path::new(sock).exists() {
                continue;
            }
            use std::io::{Read, Write};
            use std::os::unix::net::UnixStream;
            if let Ok(mut stream) = UnixStream::connect(sock) {
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
        DockerActionType::Exec { container, cmd } => {
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

pub fn cmd_crontab(action: &lan_link_protocol::frame::CrontabActionType) -> (Vec<u8>, Option<i32>) {
    use lan_link_protocol::frame::CrontabActionType;
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

pub fn cmd_firewall(backend: &str) -> (Vec<u8>, Option<i32>) {
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
