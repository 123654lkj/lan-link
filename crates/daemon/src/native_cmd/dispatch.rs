use super::exec::*;
use super::system::*;
use super::network::*;
use super::fs::*;
use super::service::*;
use super::pkg::*;
use super::docker::*;
use super::cron::*;
use super::firewall::*;
use lan_link_protocol::frame::NativeCmdType;

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
                Ok(_) => (b"ok\n".to_vec(), Some(0)),
                Err(e) => (format!("touch error: {}\n", e).into_bytes(), Some(1)),
            }
        }
        NativeCmdType::Pkill { name, signal } => {
            match std::process::Command::new("/usr/bin/pkill")
                .arg("-f").arg(signal.to_string()).arg(name)
                .output()
            {
                Ok(o) => (o.stdout, o.status.code()),
                Err(e) => (format!("pkill error: {}\n", e).into_bytes(), Some(1)),
            }
        }
        NativeCmdType::ShellExec { cmd, .. } => {
            match std::process::Command::new("/bin/sh")
                .env("PATH", "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin")
                .arg("-c").arg(cmd)
                .output()
            {
                Ok(o) => (o.stdout, o.status.code()),
                Err(e) => (format!("shell error: {}\n", e).into_bytes(), Some(1)),
            }
        }
    }
}
