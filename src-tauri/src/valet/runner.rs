//! Process execution on the host.
//!
//! Every external command goes through [`HostRunner`], always as a program plus an argument
//! list (never through a shell). Today only [`NativeRunner`] exists; a Flatpak build will add a
//! runner that prefixes `flatpak-spawn --host` without touching the callers.

use std::io::{self, Read};
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
    /// Runs `program` with `args` and waits for it, returning its captured output.
    fn run(&self, program: &str, args: &[&str]) -> io::Result<CommandOutput>;
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
    fn run(&self, program: &str, args: &[&str]) -> io::Result<CommandOutput> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

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

fn wait_with_timeout(child: &mut Child, timeout: Duration) -> io::Result<Option<i32>> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status.code());
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("process did not finish within {}s", timeout.as_secs()),
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(test)]
pub mod fake {
    use super::{CommandOutput, HostRunner};
    use std::io;
    use std::sync::Mutex;

    /// Test double answering by the exact command line (`program arg1 arg2`).
    #[derive(Default)]
    pub struct FakeRunner {
        responses: Vec<(String, io::Result<CommandOutput>)>,
        pub calls: Mutex<Vec<String>>,
    }

    impl FakeRunner {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn ok(self, command: &str, stdout: &str) -> Self {
            self.respond(command, 0, stdout)
        }

        pub fn respond(mut self, command: &str, code: i32, stdout: &str) -> Self {
            self.responses.push((
                command.to_owned(),
                Ok(CommandOutput { code: Some(code), stdout: stdout.to_owned(), stderr: String::new() }),
            ));
            self
        }

        pub fn called(&self, command: &str) -> bool {
            self.calls.lock().unwrap().iter().any(|c| c == command)
        }
    }

    impl HostRunner for FakeRunner {
        fn run(&self, program: &str, args: &[&str]) -> io::Result<CommandOutput> {
            let line = std::iter::once(program).chain(args.iter().copied()).collect::<Vec<_>>().join(" ");
            self.calls.lock().unwrap().push(line.clone());
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
    fn native_runner_reports_exit_code() {
        let out = NativeRunner::default().run("false", &[]).unwrap();
        assert_eq!(out.code, Some(1));
    }

    #[test]
    fn native_runner_times_out() {
        let err = NativeRunner::new(Duration::from_millis(100)).run("sleep", &["5"]).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
    }
}
