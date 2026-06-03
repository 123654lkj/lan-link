//! Native command implementations
//!
//! Each command is executed on the daemon using controlled std::process::Command.
//! No shell wrappers (sh -c). Truly native commands read /proc directly.
//! Results returned as (output_bytes, exit_code).

mod helper;
mod fs;
mod system;
mod network;
mod service;
mod exec;

use lan_link_protocol::frame::NativeCmdType;

pub fn run_native_cmd(cmd: &NativeCmdType) -> (Vec<u8>, Option<i32>) {
    match cmd {
        // -- Filesystem --
        NativeCmdType::Ls { path, long, all } => fs::cmd_ls(path, *long, *all),
        NativeCmdType::Cat { path } => fs::cmd_cat(path),
        NativeCmdType::Head { path, lines } => fs::cmd_head(path, *lines),
        NativeCmdType::Tail { path, lines, .. } => fs::cmd_tail(path, *lines),
        NativeCmdType::Stat { path } => fs::cmd_stat(path),
        NativeCmdType::Grep { pattern, path, recursive, line_number, count } => {
            fs::cmd_grep(pattern, path, *recursive, *line_number, *count)
        }
        NativeCmdType::Find { path, name, type_, maxdepth } => {
            fs::cmd_find(path, &name, &type_, *maxdepth)
        }
        NativeCmdType::Du { path, summarize, maxdepth } => fs::cmd_du(path, *summarize, *maxdepth),
        NativeCmdType::Df { human, .. } => fs::cmd_df(*human),
        NativeCmdType::Tree { path, depth, dirs_only } => fs::cmd_tree(path, *depth, *dirs_only),
        NativeCmdType::Mkdir { recursive, paths } => fs::cmd_mkdir(*recursive, paths),
        NativeCmdType::Rm { recursive, force, paths } => fs::cmd_rm(*recursive, *force, paths),
        NativeCmdType::Mv { src, dest } => fs::cmd_mv(src, dest),
        NativeCmdType::Cp { recursive, src, dest } => fs::cmd_cp(*recursive, src, dest),
        NativeCmdType::Chmod { mode, paths } => fs::cmd_chmod(mode, paths),
        NativeCmdType::Chown { owner, paths } => fs::cmd_chown(owner, paths),
        NativeCmdType::Diff { file1, file2 } => fs::cmd_diff(file1, file2),
        NativeCmdType::Wc { lines, words, paths } => fs::cmd_wc(*lines, *words, paths),
        NativeCmdType::WriteFile { path, data, append } => fs::cmd_write_file(path, data, *append),
        NativeCmdType::ReadFile { path } => fs::cmd_read_file(path),
        NativeCmdType::Touch { path } => fs::cmd_touch(path),

        // -- System --
        NativeCmdType::Uptime => system::cmd_uptime(),
        NativeCmdType::Hostname => system::cmd_hostname(),
        NativeCmdType::Free { human } => system::cmd_free(*human),
        NativeCmdType::Cpu => system::cmd_cpu(),
        NativeCmdType::Uname { all, release, machine } => system::cmd_uname(*all, *release, *machine),
        NativeCmdType::Whoami => system::cmd_whoami(),
        NativeCmdType::Ps { full, user, tree } => system::cmd_ps(*full, user.clone(), *tree),
        NativeCmdType::Info => system::cmd_info(),
        NativeCmdType::Top { .. } => system::cmd_top_snapshot(),
        NativeCmdType::Kill { pid, signal } => system::cmd_kill(*pid, *signal),
        NativeCmdType::Pgrep { name, full, count } => system::cmd_pgrep(name, *full, *count),
        NativeCmdType::Dmesg { lines, .. } => system::cmd_dmesg(*lines),
        NativeCmdType::Lsblk => system::cmd_lsblk(),
        NativeCmdType::Mount => system::cmd_mount(),
        NativeCmdType::Who => system::cmd_who(),
        NativeCmdType::Last { lines } => system::cmd_last(*lines),
        NativeCmdType::Ip { .. } => system::cmd_ip(),
        NativeCmdType::Checksum { path, algorithm } => system::cmd_checksum(path, algorithm),

        // -- Network --
        NativeCmdType::Dns { hostname, .. } => network::cmd_dns(hostname),
        NativeCmdType::Ssh => network::cmd_ssh_check(),
        NativeCmdType::PortScan { host, start_port, end_port, .. } => {
            network::cmd_portscan(host, *start_port, *end_port)
        }
        NativeCmdType::Netstat { tcp, udp, listening, .. } => {
            network::cmd_netstat(*tcp, *udp, *listening)
        }
        NativeCmdType::Arp => network::cmd_arp(),

        // -- Management --
        NativeCmdType::Service { action } => service::cmd_service(action),
        NativeCmdType::Journal { unit, lines, priority, since, follow } => {
            service::cmd_journal(unit.as_deref(), *lines, priority.as_deref(), since.as_deref(), *follow)
        }
        NativeCmdType::Pkg { action } => service::cmd_pkg(action),
        NativeCmdType::Docker { action } => service::cmd_docker(action),
        NativeCmdType::Crontab { action } => service::cmd_crontab(action),
        NativeCmdType::Firewall { backend } => service::cmd_firewall(backend),

        // -- Exec / Inline --
        NativeCmdType::BatchContent { lines, .. } => exec::cmd_batch_content(lines),
        NativeCmdType::Watch { cmd, .. } => exec::cmd_watch_fn(cmd),
        NativeCmdType::Sed { path, pattern, replacement, .. } => exec::cmd_sed(path, pattern, replacement),
        NativeCmdType::Pkill { name, signal } => exec::cmd_pkill(name, *signal),
        NativeCmdType::ShellExec { cmd, .. } => exec::cmd_shell_exec(cmd),
    }
}
