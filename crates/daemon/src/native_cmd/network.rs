use crate::native_cmd::helper::read_proc;
use std::net::TcpStream;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Mutex;

pub fn hex2addr(s: &str) -> String {
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

pub fn cmd_dns(hostname: &str) -> (Vec<u8>, Option<i32>) {
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

pub fn cmd_ssh_check() -> (Vec<u8>, Option<i32>) {
    match std::process::Command::new("/usr/bin/ssh").arg("-V").output() {
        Ok(o) => {
            let ver = String::from_utf8_lossy(&o.stderr);
            (format!("SSH available: {}\n", ver.lines().next().unwrap_or("unknown")).into_bytes(), Some(0))
        },
        Err(e) => (format!("SSH not available: {}\n", e).into_bytes(), Some(1)),
    }
}

pub fn cmd_portscan(host: &str, start: u16, end: u16) -> (Vec<u8>, Option<i32>) {
    let mut out = format!("Scanning ports {}-{} on {}\n", start, end, host);
    let results = Mutex::new(String::new());
    let next_port = AtomicU16::new(start);
    let num_ports = (end - start + 1) as usize;
    let max_threads = std::cmp::min(num_ports, 100usize);

    std::thread::scope(|s| {
        for _ in 0..max_threads {
            s.spawn(|| {
                loop {
                    let port = next_port.fetch_add(1, Ordering::Relaxed);
                    if port > end {
                        break;
                    }
                    if let Ok(addr) = format!("{}:{}", host, port).parse::<std::net::SocketAddr>() {
                        if TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(200)).is_ok() {
                            let mut r = results.lock().unwrap();
                            r.push_str(&format!("Port {} is open\n", port));
                        }
                    }
                }
            });
        }
    });

    out += &results.into_inner().unwrap();
    (out.into_bytes(), Some(0))
}

pub fn cmd_netstat(tcp: bool, udp: bool, listening: bool) -> (Vec<u8>, Option<i32>) {
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

pub fn cmd_arp() -> (Vec<u8>, Option<i32>) {
    (read_proc("/proc/net/arp").into_bytes(), Some(0))
}
