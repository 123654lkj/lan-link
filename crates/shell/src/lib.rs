//! Shell 执行引擎
//!
//! 跨平台命令执行。Unix 用 `sh -c`，Windows 用 `cmd /C`。
//! 提供同步（exec/exec_with_input）和异步流式（StreamingExec）两种模式。

//! Shell engine: command execution.
//!
//! Cross-platform streaming exec. On unix, `cmd` is passed to `sh -c`.
//! On windows, `cmd` is passed to `cmd /C`. Two reader threads drain
//! stdout and stderr into an mpsc channel, and a waiter thread sends the
//! exit code on a done channel. Stdin writes are serialised through an
//! Arc<Mutex<Option<ChildStdin>>>.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

#[derive(Debug, Clone)]
pub struct ExecResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub stream: u8, // 0 = stdout, 1 = stderr
    pub data: Vec<u8>,
}

pub fn exec(cmd: &str, args: &[&str]) -> anyhow::Result<ExecResult> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut out) = child.stdout.take() { out.read_to_string(&mut stdout)?; }
    if let Some(mut err) = child.stderr.take() { err.read_to_string(&mut stderr)?; }
    let status = child.wait()?;
    Ok(ExecResult { exit_code: status.code().unwrap_or(-1), stdout, stderr })
}

pub fn exec_with_input(cmd: &str, args: &[&str], stdin_data: &[u8]) -> anyhow::Result<ExecResult> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() { stdin.write_all(stdin_data)?; }
    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut out) = child.stdout.take() { out.read_to_string(&mut stdout)?; }
    if let Some(mut err) = child.stderr.take() { err.read_to_string(&mut stderr)?; }
    let status = child.wait()?;
    Ok(ExecResult { exit_code: status.code().unwrap_or(-1), stdout, stderr })
}

pub struct StreamingExec {
    child: Arc<Mutex<Option<std::process::Child>>>,
    chunks_rx: mpsc::Receiver<StreamChunk>,
    done_rx: mpsc::Receiver<Option<i32>>,
    stdin_arc: Arc<Mutex<Option<std::process::ChildStdin>>>,
}

impl StreamingExec {
    pub fn spawn(cmd: &str) -> anyhow::Result<Self> {
        let mut command = if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.arg("/C").arg(cmd);
            c
        } else {
            let mut c = Command::new("sh");
            c.arg("-c").arg(cmd);
            c
        };
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command.spawn()?;
        let mut stdout = child.stdout.take().expect("piped stdout");
        let mut stderr = child.stderr.take().expect("piped stderr");
        let stdin = child.stdin.take().expect("piped stdin");

        let (chunks_tx, chunks_rx) = mpsc::channel::<StreamChunk>();
        let chunks_tx = Arc::new(Mutex::new(chunks_tx));
        let (done_tx, done_rx) = mpsc::channel::<Option<i32>>();

        let chunks_tx_a = chunks_tx.clone();
        thread::Builder::new().name("ll-shell-stdout".into()).spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match stdout.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let tx = chunks_tx_a.lock().unwrap();
                        if tx.send(StreamChunk { stream: 0, data: buf[..n].to_vec() }).is_err() { break; }
                    }
                    Err(_) => break,
                }
            }
        })?;

        let chunks_tx_b = chunks_tx.clone();
        thread::Builder::new().name("ll-shell-stderr".into()).spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match stderr.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let tx = chunks_tx_b.lock().unwrap();
                        if tx.send(StreamChunk { stream: 1, data: buf[..n].to_vec() }).is_err() { break; }
                    }
                    Err(_) => break,
                }
            }
        })?;

        let child_arc: Arc<Mutex<Option<std::process::Child>>> = Arc::new(Mutex::new(Some(child)));
        let child_arc2 = child_arc.clone();
        thread::Builder::new().name("ll-shell-wait".into()).spawn(move || {
            let code = {
                let mut guard = child_arc2.lock().unwrap();
                if let Some(mut c) = guard.take() {
                    c.wait().ok().and_then(|s| s.code())
                } else { None }
            };
            let _ = done_tx.send(code);
        })?;

        Ok(StreamingExec {
            child: child_arc,
            chunks_rx,
            done_rx,
            stdin_arc: Arc::new(Mutex::new(Some(stdin))),
        })
    }

    pub fn try_poll_chunk(&self) -> Option<StreamChunk> {
        self.chunks_rx.try_recv().ok()
    }

    pub fn try_wait(&self) -> Option<Option<i32>> {
        match self.done_rx.try_recv() {
            Ok(code) => Some(code),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(None),
        }
    }

    pub fn wait(&self) -> Option<i32> {
        self.done_rx.recv().ok().flatten()
    }

    pub fn kill(&self) -> anyhow::Result<()> {
        let mut guard = self.child.lock().unwrap();
        if let Some(c) = guard.as_mut() { c.kill()?; }
        Ok(())
    }

    pub fn write_stdin(&self, data: &[u8], close: bool) -> anyhow::Result<()> {
        let mut guard = self.stdin_arc.lock().unwrap();
        if let Some(stdin) = guard.as_mut() {
            stdin.write_all(data)?;
            if close { *guard = None; }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_echo() {
        let r = exec("echo", &["hello"]).unwrap();
        assert_eq!(r.exit_code, 0);
        assert!(r.stdout.contains("hello"));
    }

    #[test]
    fn test_streaming_echo() {
        let s = StreamingExec::spawn("echo hello world").unwrap();
        let mut acc = Vec::new();
        loop {
            if let Some(c) = s.try_poll_chunk() { acc.extend_from_slice(&c.data); }
            else if let Some(code) = s.try_wait() { assert_eq!(code, Some(0)); break; }
            else { thread::sleep(Duration::from_millis(10)); }
        }
        let text = String::from_utf8_lossy(&acc);
        assert!(text.contains("hello world"), "got: {}", text);
    }

    #[test]
    fn test_streaming_stdin() {
        // cat reads stdin until EOF
        let s = StreamingExec::spawn("cat").unwrap();
        s.write_stdin(b"line1
line2
", false).unwrap();
        s.write_stdin(b"", true).unwrap();
        let mut acc = Vec::new();
        loop {
            if let Some(c) = s.try_poll_chunk() { acc.extend_from_slice(&c.data); }
            else if let Some(_code) = s.try_wait() { break; }
            else { thread::sleep(Duration::from_millis(10)); }
        }
        let text = String::from_utf8_lossy(&acc);
        assert!(text.contains("line1"));
        assert!(text.contains("line2"));
    }
}
