use lan_link_protocol::frame::PkgActionType;

pub fn cmd_pkg(action: &PkgActionType) -> (Vec<u8>, Option<i32>) {
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
