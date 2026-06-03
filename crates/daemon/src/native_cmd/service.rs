use lan_link_protocol::frame::ServiceActionType;
use std::process::Command;

pub fn cmd_service(action: &ServiceActionType) -> (Vec<u8>, Option<i32>) {
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
            match Command::new("/usr/bin/systemctl")
                .arg("start")
                .arg(name)
                .output()
            {
                Ok(o) => (o.stdout, o.status.code()),
                Err(e) => (format!("systemctl error: {}\n", e).into_bytes(), Some(1)),
            }
        }
        ServiceActionType::Stop { name } => {
            match Command::new("/usr/bin/systemctl")
                .arg("stop")
                .arg(name)
                .output()
            {
                Ok(o) => (o.stdout, o.status.code()),
                Err(e) => (format!("systemctl error: {}\n", e).into_bytes(), Some(1)),
            }
        }
        ServiceActionType::Restart { name } => {
            match Command::new("/usr/bin/systemctl")
                .arg("restart")
                .arg(name)
                .output()
            {
                Ok(o) => (o.stdout, o.status.code()),
                Err(e) => (format!("systemctl error: {}\n", e).into_bytes(), Some(1)),
            }
        }
        ServiceActionType::Reload { name } => {
            match Command::new("/usr/bin/systemctl")
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
