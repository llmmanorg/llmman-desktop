//! The `llmman serve` behind the window: one already answering, or our child.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

/// llmman's default bind. The child is pinned to it (`LLMMAN_HOST`) so a
/// value in the environment cannot send it somewhere the window isn't.
pub const HOST: &str = "127.0.0.1:17434";
pub const URL: &str = "http://127.0.0.1:17434/";

pub enum Status {
    /// The daemon's latest log line while it starts: on a first run it
    /// fetches llama.cpp before it binds, which can take minutes.
    Starting(String),
    Ready,
    /// Our child exited; its exit status and log tail.
    Failed(String),
}

#[derive(Default)]
pub struct Daemon {
    child: Option<Child>,
}

impl Daemon {
    /// Reuses a running daemon or spawns one, reports progress through
    /// `send`, then watches the child until it exits or [`Daemon::stop`]
    /// takes it. Blocks; run it on its own thread.
    pub fn start(this: &Mutex<Daemon>, send: impl Fn(Status)) {
        if this.lock().unwrap().child.is_some() {
            return; // already started and watched
        }
        if alive() {
            return send(Status::Ready);
        }
        let log = log_path();
        match spawn(&log) {
            Ok(child) => this.lock().unwrap().child = Some(child),
            Err(e) => return send(Status::Failed(e)),
        }
        let (mut ready, mut shown) = (false, String::new());
        loop {
            std::thread::sleep(Duration::from_millis(if ready { 1000 } else { 250 }));
            {
                let mut d = this.lock().unwrap();
                let Some(child) = d.child.as_mut() else {
                    return; // stopped
                };
                if let Ok(Some(status)) = child.try_wait() {
                    d.child = None;
                    let tail = tail(&read(&log), 40);
                    return send(Status::Failed(format!(
                        "{status}\n\n{tail}\n\n{}",
                        log.display()
                    )));
                }
            }
            if ready {
                continue;
            }
            if alive() {
                ready = true;
                send(Status::Ready);
            } else {
                let line = tail(&read(&log), 1);
                if line != shown {
                    shown = line.clone();
                    send(Status::Starting(line));
                }
            }
        }
    }

    /// Stops our child, if any: SIGTERM lets `serve` unload its models and
    /// their engines; on Windows, which has no such signal, the whole tree.
    pub fn stop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        #[cfg(unix)]
        // SAFETY: kill(2) has no memory-safety preconditions; the pid is
        // our unreaped child, so it cannot have been reused.
        unsafe {
            libc::kill(child.id() as libc::pid_t, libc::SIGTERM);
        }
        #[cfg(windows)]
        {
            let mut cmd = Command::new("taskkill");
            no_window(&mut cmd);
            let _ = cmd
                .args(["/PID", &child.id().to_string(), "/T", "/F"])
                .status();
        }
        for _ in 0..50 {
            if let Ok(Some(_)) = child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// `GET /api/version` answers (401 from a daemon with API keys counts: the
/// page asks for the key itself): the router is up, not merely the socket.
fn alive() -> bool {
    let Ok(mut s) = TcpStream::connect_timeout(&HOST.parse().unwrap(), Duration::from_secs(1))
    else {
        return false;
    };
    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
    let req = format!("GET /api/version HTTP/1.1\r\nHost: {HOST}\r\nConnection: close\r\n\r\n");
    let mut head = [0u8; 12];
    s.write_all(req.as_bytes()).is_ok() && s.read_exact(&mut head).is_ok() && answered(&head)
}

fn answered(head: &[u8; 12]) -> bool {
    head.starts_with(b"HTTP/1.") && matches!(&head[9..], b"200" | b"401")
}

/// The `llmman` shipped next to this executable, else the one on `PATH`.
fn llmman() -> PathBuf {
    let name = if cfg!(windows) {
        "llmman.exe"
    } else {
        "llmman"
    };
    std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.join(name)))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| name.into())
}

fn log_path() -> PathBuf {
    std::env::temp_dir().join("llmman-desktop-serve.log")
}

fn spawn(log: &PathBuf) -> Result<Child, String> {
    let exe = llmman();
    let out = std::fs::File::create(log).map_err(|e| format!("{}: {e}", log.display()))?;
    let err = out.try_clone().map_err(|e| e.to_string())?;
    let mut cmd = Command::new(&exe);
    cmd.arg("serve")
        .env("LLMMAN_HOST", HOST)
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err);
    #[cfg(windows)]
    no_window(&mut cmd);
    cmd.spawn().map_err(|e| {
        format!(
            "could not run {}: {e}\n\nInstall llmman (https://github.com/llmmanorg/llmman#install) \
             or put it next to this app.",
            exe.display()
        )
    })
}

/// llmman is a console program; without this each spawn opens a console.
#[cfg(windows)]
fn no_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

fn read(log: &PathBuf) -> String {
    std::fs::read(log)
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default()
}

/// The last `n` non-blank lines; `\r` counts as a line end, for progress bars.
fn tail(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text
        .split(['\n', '\r'])
        .map(str::trim_end)
        .filter(|l| !l.is_empty())
        .collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_skips_blanks_and_splits_progress() {
        assert_eq!(tail("a\n\nb\r 50%\r100%\n\n", 1), "100%");
        assert_eq!(tail("a\nb\nc\n", 2), "b\nc");
        assert_eq!(tail("", 3), "");
    }

    #[test]
    fn ok_and_unauthorized_count_as_up() {
        assert!(answered(b"HTTP/1.1 200"));
        assert!(answered(b"HTTP/1.1 401"));
        assert!(!answered(b"HTTP/1.1 404"));
        assert!(!answered(b"SSH-2.0-Open"));
    }
}
