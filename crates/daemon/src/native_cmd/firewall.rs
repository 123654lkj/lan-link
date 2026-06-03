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
