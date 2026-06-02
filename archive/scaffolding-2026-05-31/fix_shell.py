from pathlib import Path

# Fix shell Cargo.toml - add anyhow
Path(r"G:\codex-AI-tools\lan-link\crates\shell\Cargo.toml").write_text("""\
[package]
name = "lan-link-shell"
version = "0.1.0"
edition = "2024"

[dependencies]
lan-link-protocol = { path = "../protocol" }
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
anyhow = "1"
""", encoding="utf-8")

# Rewrite shell lib.rs without portable-pty (cmake dependency issue on Windows)
# Use std::process::Command for now, add pty later when cmake is available
shell_lib = """//! Shell engine: command execution.
//!
//! Uses std::process::Command for cross-platform execution.
//! Pty support (portable-pty) requires cmake on Windows; add later.

use std::process::{Command, Stdio};
use std::io::{Read, Write};

/// Execute a command and capture stdout + stderr.
pub fn exec(cmd: &str, args: &[&str]) -> anyhow::Result<ExecResult> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stdout = String::new();
    let mut stderr = String::new();

    if let Some(mut out) = child.stdout.take() {
        out.read_to_string(&mut stdout)?;
    }
    if let Some(mut err) = child.stderr.take() {
        err.read_to_string(&mut stderr)?;
    }

    let status = child.wait()?;

    Ok(ExecResult {
        exit_code: status.code().unwrap_or(-1),
        stdout,
        stderr,
    })
}

/// Execute a command with stdin input.
pub fn exec_with_input(cmd: &str, args: &[&str], stdin_data: &[u8]) -> anyhow::Result<ExecResult> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(stdin_data)?;
    }

    let mut stdout = String::new();
    let mut stderr = String::new();

    if let Some(mut out) = child.stdout.take() {
        out.read_to_string(&mut stdout)?;
    }
    if let Some(mut err) = child.stderr.take() {
        err.read_to_string(&mut stderr)?;
    }

    let status = child.wait()?;

    Ok(ExecResult {
        exit_code: status.code().unwrap_or(-1),
        stdout,
        stderr,
    })
}

#[derive(Debug, Clone)]
pub struct ExecResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_echo() {
        let r = exec("echo", &["hello"]).unwrap();
        assert_eq!(r.exit_code, 0);
        assert!(r.stdout.contains("hello"));
    }
}
"""

Path(r"G:\codex-AI-tools\lan-link\crates\shell\src\lib.rs").write_text(shell_lib, encoding="utf-8")

print("fixed")
