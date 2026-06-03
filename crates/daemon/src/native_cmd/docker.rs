use lan_link_protocol::frame::DockerActionType;

pub fn cmd_docker(action: &DockerActionType) -> (Vec<u8>, Option<i32>) {
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
