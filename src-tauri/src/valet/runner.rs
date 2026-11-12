//! Process execution on the host.
//!
//! Every external command goes through [`HostRunner`], always as a program plus an argument
//! list (never through a shell). Today only [`NativeRunner`] exists; a Flatpak build will add a
//! runner that prefixes `flatpak-spawn --host` without touching the callers.

use std::io::{self, Read};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Captured result of a finished process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    /// Exit code, `None` when the process was killed by a signal (or by the timeout).
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl CommandOutput {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

pub trait HostRunner: Send + Sync {
    /// Runs `program` with `args` in `cwd` (or the current directory) and waits for it,
    /// returning its captured output.
    fn run_in(&self, program: &str, args: &[&str], cwd: Option<&Path>) -> io::Result<CommandOutput>;

    /// Runs `program` with `args` in the current directory.
    fn run(&self, program: &str, args: &[&str]) -> io::Result<CommandOutput> {
        self.run_in(program, args, None)
    }
}

/// Runs processes directly on the host with `std::process::Command`.
#[derive(Debug, Clone)]
pub struct NativeRunner {
    timeout: Duration,
}

impl NativeRunner {
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }
}

impl Default for NativeRunner {
    fn default() -> Self {
        Self::new(Duration::from_secs(15))
    }
}

impl HostRunner for NativeRunner {
    fn run_in(&self, program: &str, args: &[&str], cwd: Option<&Path>) -> io::Result<CommandOutput> {
        let mut command = Command::new(program);
        command.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }
        let mut child = command.spawn()?;

        // Drain both pipes in background threads so a chatty process can't block on a full pipe
        // while we wait for it.
        let stdout = spawn_reader(child.stdout.take());
        let stderr = spawn_reader(child.stderr.take());

        let code = wait_with_timeout(&mut child, self.timeout)?;

        Ok(CommandOutput { code, stdout: stdout.join().unwrap_or_default(), stderr: stderr.join().unwrap_or_default() })
    }
}

fn spawn_reader<R: Read + Send + 'static>(pipe: Option<R>) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut buf);
        }
        String::from_utf8_lossy(&buf).into_owned()
    })
}

/// Time a timed-out process gets to exit after `SIGTERM` before it is killed with `SIGKILL`.
const TERMINATE_GRACE: Duration = Duration::from_secs(3);

fn wait_with_timeout(child: &mut Child, timeout: Duration) -> io::Result<Option<i32>> {
    if let Some(code) = wait_until(child, Instant::now() + timeout)? {
        return Ok(code);
    }

    // Ask first: a process that handles SIGTERM can finish what it is writing and exit cleanly.
    terminate(child);
    if wait_until(child, Instant::now() + TERMINATE_GRACE)?.is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }

    Err(io::Error::new(io::ErrorKind::TimedOut, format!("process did not finish within {}s", timeout.as_secs())))
}

/// Polls `child` until it exits (`Some(exit code)`) or `deadline` passes (`None`).
fn wait_until(child: &mut Child, deadline: Instant) -> io::Result<Option<Option<i32>>> {
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status.code()));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(unix)]
fn terminate(child: &Child) {
    if let Ok(pid) = libc::pid_t::try_from(child.id()) {
        // SAFETY: `pid` belongs to a child we spawned and have not reaped yet (`try_wait` returned
        // `None`), so it cannot have been recycled for another process.
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }
    }
}

#[cfg(not(unix))]
fn terminate(_child: &Child) {}

#[cfg(test)]
pub mod fake {
    use super::{CommandOutput, HostRunner};
    use std::io;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    /// Test double answering by the exact command line (`program arg1 arg2`).
    #[derive(Default)]
    pub struct FakeRunner {
        responses: Vec<(String, io::Result<CommandOutput>)>,
        pub calls: Mutex<Vec<String>>,
        pub cwds: Mutex<Vec<Option<PathBuf>>>,
    }

    impl FakeRunner {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn ok(self, command: &str, stdout: &str) -> Self {
            self.respond(command, 0, stdout)
        }

        pub fn respond(self, command: &str, code: i32, stdout: &str) -> Self {
            self.respond_full(command, code, stdout, "")
        }

        pub fn respond_full(mut self, command: &str, code: i32, stdout: &str, stderr: &str) -> Self {
            self.responses.push((
                command.to_owned(),
                Ok(CommandOutput { code: Some(code), stdout: stdout.to_owned(), stderr: stderr.to_owned() }),
            ));
            self
        }

        pub fn last_call(&self) -> Option<String> {
            self.calls.lock().unwrap().last().cloned()
        }

        pub fn last_cwd(&self) -> Option<PathBuf> {
            self.cwds.lock().unwrap().last().cloned().flatten()
        }

        pub fn called(&self, command: &str) -> bool {
            self.calls.lock().unwrap().iter().any(|c| c == command)
        }
    }

    impl HostRunner for FakeRunner {
        fn run_in(&self, program: &str, args: &[&str], cwd: Option<&Path>) -> io::Result<CommandOutput> {
            let line = std::iter::once(program).chain(args.iter().copied()).collect::<Vec<_>>().join(" ");
            self.calls.lock().unwrap().push(line.clone());
            self.cwds.lock().unwrap().push(cwd.map(Path::to_path_buf));
            match self.responses.iter().find(|(cmd, _)| *cmd == line) {
                Some((_, Ok(out))) => Ok(out.clone()),
                Some((_, Err(e))) => Err(io::Error::new(e.kind(), e.to_string())),
                None => Err(io::Error::new(io::ErrorKind::NotFound, format!("unexpected command: {line}"))),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_runner_passes_arguments_without_a_shell() {
        let out = NativeRunner::default().run("echo", &["$HOME", "a;b"]).unwrap();
        assert!(out.success());
        assert_eq!(out.stdout, "$HOME a;b\n");
    }

    #[test]
    fn native_runner_honours_cwd() {
        let tmp = tempfile::tempdir().unwrap();
        let out = NativeRunner::default().run_in("pwd", &[], Some(tmp.path())).unwrap();
        assert_eq!(out.stdout.trim(), std::fs::canonicalize(tmp.path()).unwrap().to_string_lossy());
    }

    #[test]
    fn native_runner_reports_exit_code() {
        let out = NativeRunner::default().run("false", &[]).unwrap();
        assert_eq!(out.code, Some(1));
    }

    #[test]
    fn native_runner_lets_timed_out_process_handle_sigterm() {
        let tmp = tempfile::tempdir().unwrap();
        let marker = tmp.path().join("terminated");
        let script = format!("trap 'touch {}; exit 0' TERM; while :; do sleep 0.05; done", marker.display());

        let err = NativeRunner::new(Duration::from_millis(200)).run("sh", &["-c", &script]).unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
        assert!(marker.exists(), "the SIGTERM handler should have run before the kill");
    }

    #[test]
    fn native_runner_kills_process_that_ignores_sigterm() {
        let started = Instant::now();
        let err = NativeRunner::new(Duration::from_millis(100))
            .run("sh", &["-c", "trap '' TERM; while :; do sleep 0.05; done"])
            .unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
        assert!(started.elapsed() < TERMINATE_GRACE + Duration::from_secs(2));
    }

    #[test]
    fn native_runner_times_out() {
        let err = NativeRunner::new(Duration::from_millis(100)).run("sleep", &["5"]).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
    }
}
