//! Privileged session: one `pkexec` prompt per app session.
//!
//! The first privileged action starts `packaging/linux/valet-helper --broker` through pkexec.
//! The broker stays alive while the app runs and executes whitelisted valet commands sent over
//! its stdin; it is reachable only through the pipes this process holds and exits when they
//! close (app closed, crashed or session ended by the user).

use std::io::{self, BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use thiserror::Error;

use super::env::{HostEnv, is_executable};
use super::runner::CommandOutput;

/// The broker script, embedded so AppImage/dev builds can run it with `pkexec /bin/bash -c`
/// (root can't read the AppImage FUSE mount, and nothing is installed under /usr there).
pub const HELPER_SCRIPT: &str = include_str!("../../../packaging/linux/valet-helper");

/// Upper bound for the stderr kept to explain a failed start.
const STDERR_CAP: usize = 16 * 1024;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum BrokerError {
    #[error("authentication was cancelled")]
    AuthCancelled,
    #[error("not authorized to run privileged Valet commands")]
    NotAuthorized,
    #[error("could not start the privileged helper: {0}")]
    Start(String),
    /// The broker was gone before the request was sent: nothing ran, safe to retry.
    #[error("the privileged session ended")]
    Closed,
    #[error("the privileged helper stopped while running the command")]
    Died,
    #[error("the privileged command did not finish within {0}s")]
    Timeout(u64),
    #[error("the privileged helper rejected the command: {0}")]
    Rejected(String),
    #[error("unexpected reply from the privileged helper: {0}")]
    Protocol(String),
}

#[derive(Debug)]
enum Frame {
    Ready,
    Result(CommandOutput),
    Error(String),
}

/// Program and arguments that start the broker for `valet`.
pub fn broker_command(env: &HostEnv, valet: &str) -> (String, Vec<String>) {
    if is_executable(&env.helper) {
        // Installed with the polkit policy: nicer prompt and auth_admin_keep.
        let helper = env.helper.to_string_lossy().into_owned();
        return ("pkexec".into(), vec![helper, "--broker".into(), valet.into()]);
    }
    (
        "pkexec".into(),
        vec![
            "/bin/bash".into(),
            "-c".into(),
            HELPER_SCRIPT.into(),
            "valet-helper".into(),
            "--broker".into(),
            valet.into(),
        ],
    )
}

/// A running broker process.
pub struct Broker {
    valet: String,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    frames: Receiver<io::Result<Frame>>,
    stderr: Arc<Mutex<String>>,
}

impl Broker {
    /// Spawns the broker and waits until it is authenticated and ready (the user may take a
    /// while to type the password, hence `timeout`).
    pub fn start(
        program: &str,
        args: &[String],
        extra_env: &[(&str, &str)],
        valet: &str,
        timeout: Duration,
    ) -> Result<Self, BrokerError> {
        let mut child = Command::new(program)
            .args(args)
            .envs(extra_env.iter().copied())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| BrokerError::Start(format!("{program}: {e}")))?;

        let stdout = child.stdout.take().expect("piped stdout");
        let stderr_pipe = child.stderr.take().expect("piped stderr");
        let (tx, frames) = mpsc::channel();
        thread::spawn(move || read_frames(stdout, &tx));
        let stderr = Arc::new(Mutex::new(String::new()));
        let sink = Arc::clone(&stderr);
        thread::spawn(move || collect_stderr(stderr_pipe, &sink));

        let mut broker =
            Self { valet: valet.to_owned(), stdin: child.stdin.take(), child: Some(child), frames, stderr };

        match broker.frames.recv_timeout(timeout) {
            Ok(Ok(Frame::Ready)) => Ok(broker),
            Ok(Ok(other)) => Err(BrokerError::Protocol(format!("{other:?}"))),
            Ok(Err(e)) => Err(BrokerError::Protocol(e.to_string())),
            Err(RecvTimeoutError::Timeout) => Err(BrokerError::Timeout(timeout.as_secs())),
            Err(RecvTimeoutError::Disconnected) => Err(broker.start_failure()),
        }
    }

    /// Why the broker exited before READY: pkexec's exit codes or the helper's own message.
    fn start_failure(&mut self) -> BrokerError {
        let code = self.child.as_mut().and_then(|c| c.wait().ok()).and_then(|s| s.code());
        // Give the stderr thread a moment to drain the pipe after the exit.
        thread::sleep(Duration::from_millis(20));
        let stderr = self.stderr.lock().map(|s| s.trim().to_owned()).unwrap_or_default();
        match code {
            Some(126) => BrokerError::AuthCancelled,
            Some(127) => BrokerError::NotAuthorized,
            _ if !stderr.is_empty() => BrokerError::Start(stderr),
            _ => BrokerError::Start(format!("exit code {code:?}")),
        }
    }

    pub fn valet(&self) -> &str {
        &self.valet
    }

    pub fn is_alive(&mut self) -> bool {
        self.stdin.is_some() && self.child.as_mut().is_some_and(|c| matches!(c.try_wait(), Ok(None)))
    }

    /// Sends one valet command and waits for its result.
    pub fn run(&mut self, args: &[String], timeout: Duration) -> Result<CommandOutput, BrokerError> {
        if !self.is_alive() {
            return Err(BrokerError::Closed);
        }
        let stdin = self.stdin.as_mut().ok_or(BrokerError::Closed)?;

        let mut request = Vec::new();
        for arg in args {
            if arg.is_empty() || arg.contains('\0') {
                return Err(BrokerError::Rejected(format!("invalid argument {arg:?}")));
            }
            request.extend_from_slice(arg.as_bytes());
            request.push(0);
        }
        request.push(0);
        if stdin.write_all(&request).and_then(|()| stdin.flush()).is_err() {
            self.stdin = None;
            return Err(BrokerError::Closed);
        }

        match self.frames.recv_timeout(timeout) {
            Ok(Ok(Frame::Result(out))) => Ok(out),
            Ok(Ok(Frame::Error(message))) => Err(BrokerError::Rejected(message)),
            Ok(Ok(Frame::Ready)) => Err(BrokerError::Protocol("unexpected READY".into())),
            Ok(Err(e)) => Err(BrokerError::Protocol(e.to_string())),
            Err(RecvTimeoutError::Timeout) => {
                // A root process can't be killed from here: close its stdin so it exits once the
                // running command finishes, and start a fresh session next time.
                self.stdin = None;
                Err(BrokerError::Timeout(timeout.as_secs()))
            }
            Err(RecvTimeoutError::Disconnected) => {
                self.stdin = None;
                Err(BrokerError::Died)
            }
        }
    }
}

impl Drop for Broker {
    fn drop(&mut self) {
        // Closing stdin ends the broker loop; reap it in the background to avoid a zombie.
        self.stdin = None;
        if let Some(mut child) = self.child.take() {
            thread::spawn(move || {
                let _ = child.wait();
            });
        }
    }
}

fn read_frames(stdout: impl Read, tx: &mpsc::Sender<io::Result<Frame>>) {
    let mut reader = BufReader::new(stdout);
    loop {
        let frame = match read_frame(&mut reader) {
            Ok(Some(frame)) => Ok(frame),
            Ok(None) => return, // EOF: dropping `tx` signals Disconnected
            Err(e) => Err(e),
        };
        let failed = frame.is_err();
        if tx.send(frame).is_err() || failed {
            return;
        }
    }
}

fn read_frame(reader: &mut impl BufRead) -> io::Result<Option<Frame>> {
    let mut header = String::new();
    if reader.read_line(&mut header)? == 0 {
        return Ok(None);
    }
    let bad = || io::Error::new(io::ErrorKind::InvalidData, format!("bad header {header:?}"));
    let fields: Vec<&str> = header.split_whitespace().collect();

    let frame = match fields.as_slice() {
        ["READY"] => Frame::Ready,
        ["RESULT", code, out_len, err_len] => {
            let code: i32 = code.parse().map_err(|_| bad())?;
            let stdout = read_exact_string(reader, out_len.parse().map_err(|_| bad())?)?;
            let stderr = read_exact_string(reader, err_len.parse().map_err(|_| bad())?)?;
            Frame::Result(CommandOutput { code: Some(code), stdout, stderr })
        }
        ["ERROR", len] => Frame::Error(read_exact_string(reader, len.parse().map_err(|_| bad())?)?),
        _ => return Err(bad()),
    };
    Ok(Some(frame))
}

fn read_exact_string(reader: &mut impl Read, len: usize) -> io::Result<String> {
    let mut buf = vec![0; len];
    reader.read_exact(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn collect_stderr(pipe: impl Read, sink: &Mutex<String>) {
    let mut reader = BufReader::new(pipe);
    let mut line = String::new();
    while reader.read_line(&mut line).unwrap_or(0) > 0 {
        if let Ok(mut buf) = sink.lock()
            && buf.len() < STDERR_CAP
        {
            buf.push_str(&line);
        }
        line.clear();
    }
}

/// Runs privileged valet commands. Abstracted so actions can be tested without pkexec.
pub trait PrivilegedExec: Send {
    fn exec(&mut self, env: &HostEnv, valet: &str, args: &[String]) -> Result<CommandOutput, BrokerError>;
    fn is_active(&mut self) -> bool;
    fn end(&mut self);
}

/// Lazily started broker, reused for the whole app session.
pub struct SessionBroker {
    broker: Option<Broker>,
    timeout: Duration,
}

impl SessionBroker {
    pub fn new(timeout: Duration) -> Self {
        Self { broker: None, timeout }
    }
}

impl PrivilegedExec for SessionBroker {
    fn exec(&mut self, env: &HostEnv, valet: &str, args: &[String]) -> Result<CommandOutput, BrokerError> {
        // At most one restart, and only when nothing ran (the session ended or valet moved).
        for attempt in 0..2 {
            let reusable = self.broker.as_mut().is_some_and(|b| b.is_alive() && b.valet() == valet);
            if !reusable {
                self.broker = None;
                let (program, argv) = broker_command(env, valet);
                self.broker = Some(Broker::start(&program, &argv, &[], valet, self.timeout)?);
            }
            match self.broker.as_mut().expect("started above").run(args, self.timeout) {
                Err(BrokerError::Closed) if attempt == 0 => self.broker = None,
                Err(e @ (BrokerError::Died | BrokerError::Timeout(_) | BrokerError::Closed)) => {
                    self.broker = None;
                    return Err(e);
                }
                other => return other,
            }
        }
        Err(BrokerError::Closed)
    }

    fn is_active(&mut self) -> bool {
        self.broker.as_mut().is_some_and(Broker::is_alive)
    }

    fn end(&mut self) {
        self.broker = None;
    }
}

/// Starts the real helper script as the current user (test mode), for protocol tests.
#[cfg(test)]
pub fn start_test_broker(valet: &std::path::Path) -> Result<Broker, BrokerError> {
    use std::os::unix::fs::MetadataExt;
    let uid = std::fs::metadata(valet).unwrap().uid().to_string();
    let args: Vec<String> = ["-c", HELPER_SCRIPT, "valet-helper", "--broker", &valet.to_string_lossy()]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    Broker::start(
        "bash",
        &args,
        &[("PKEXEC_UID", &uid), ("VALET_HELPER_ALLOW_NON_ROOT", "1")],
        &valet.to_string_lossy(),
        Duration::from_secs(10),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    const SECS: Duration = Duration::from_secs(10);

    /// Fake `valet` that echoes its arguments, HOME and SUDO_USER, and fails on `stop`.
    fn fake_valet(dir: &Path) -> PathBuf {
        let valet = dir.join("valet");
        fs::write(
            &valet,
            "#!/bin/sh\n\
             printf 'args:'; for a in \"$@\"; do printf ' [%s]' \"$a\"; done; echo\n\
             echo \"home=$HOME user=$SUDO_USER\"\n\
             echo warn >&2\n\
             [ \"$1\" = stop ] && exit 3\n\
             exit 0\n",
        )
        .unwrap();
        fs::set_permissions(&valet, fs::Permissions::from_mode(0o755)).unwrap();
        valet
    }

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn runs_many_commands_in_one_session() {
        let tmp = tempfile::tempdir().unwrap();
        let mut broker = start_test_broker(&fake_valet(tmp.path())).unwrap();

        let out = broker.run(&args(&["restart", "nginx", "--no-interaction"]), SECS).unwrap();
        assert_eq!(out.code, Some(0));
        assert!(out.stdout.starts_with("args: [restart] [nginx] [--no-interaction]\n"), "{}", out.stdout);
        assert_eq!(out.stderr, "warn\n");

        let out = broker.run(&args(&["stop", "php"]), SECS).unwrap();
        assert_eq!(out.code, Some(3));

        // Arguments keep spaces, quotes and newlines intact (no shell involved).
        let out = broker.run(&args(&["secure", "a b'\"\n$(id)"]), SECS).unwrap();
        assert!(out.stdout.starts_with("args: [secure] [a b'\"\n$(id)]\n"), "{}", out.stdout);
        assert!(broker.is_alive());
    }

    #[test]
    fn impersonates_the_calling_user() {
        let tmp = tempfile::tempdir().unwrap();
        let mut broker = start_test_broker(&fake_valet(tmp.path())).unwrap();

        let out = broker.run(&args(&["restart"]), SECS).unwrap();
        let home = std::env::var("HOME").unwrap();
        assert!(out.stdout.contains(&format!("home={home} user=")), "{}", out.stdout);
        assert!(!out.stdout.contains("user=\n"), "SUDO_USER must be set: {}", out.stdout);
    }

    #[test]
    fn rejects_commands_outside_the_whitelist_and_keeps_running() {
        let tmp = tempfile::tempdir().unwrap();
        let mut broker = start_test_broker(&fake_valet(tmp.path())).unwrap();

        for bad in [["uninstall"].as_slice(), &["park", "/tmp"], &["--help"]] {
            let err = broker.run(&args(bad), SECS).unwrap_err();
            assert!(matches!(err, BrokerError::Rejected(ref m) if m.contains("not allowed")), "{err:?}");
        }
        assert_eq!(broker.run(&args(&["restart"]), SECS).unwrap().code, Some(0));
    }

    #[test]
    fn cli_php_builtin_only_accepts_installed_versions() {
        let tmp = tempfile::tempdir().unwrap();
        let mut broker = start_test_broker(&fake_valet(tmp.path())).unwrap();

        for bad in [["cli-php", "9.9"], ["cli-php", "../../bin/sh"], ["cli-php", "8.3; id"]] {
            let err = broker.run(&args(&bad), SECS).unwrap_err();
            assert!(matches!(err, BrokerError::Rejected(ref m) if m.contains("not installed")), "{err:?}");
        }
        // With extra arguments it is not the built-in, and not a valet command either.
        let err = broker.run(&args(&["cli-php", "8.3", "--x"]), SECS).unwrap_err();
        assert!(matches!(err, BrokerError::Rejected(ref m) if m.contains("not allowed")), "{err:?}");
    }

    #[test]
    fn refuses_a_binary_that_is_not_valet() {
        let tmp = tempfile::tempdir().unwrap();
        let other = tmp.path().join("evil");
        fs::copy(fake_valet(tmp.path()), &other).unwrap();

        let err = start_test_broker(&other).err().unwrap();
        assert!(matches!(err, BrokerError::Start(ref m) if m.contains("not a valet executable")), "{err:?}");
    }

    #[test]
    fn exits_when_the_app_closes_the_pipe() {
        let tmp = tempfile::tempdir().unwrap();
        let mut broker = start_test_broker(&fake_valet(tmp.path())).unwrap();
        broker.stdin = None;

        let status = broker.child.as_mut().unwrap().wait().unwrap();
        assert_eq!(status.code(), Some(0));
        assert_eq!(broker.run(&args(&["restart"]), SECS), Err(BrokerError::Closed));
    }

    #[test]
    fn maps_pkexec_exit_codes() {
        let cancelled = Broker::start("sh", &args(&["-c", "exit 126"]), &[], "/v", SECS).err();
        assert_eq!(cancelled, Some(BrokerError::AuthCancelled));
        let denied = Broker::start("sh", &args(&["-c", "exit 127"]), &[], "/v", SECS).err();
        assert_eq!(denied, Some(BrokerError::NotAuthorized));
    }

    #[test]
    fn uses_the_installed_helper_when_present() {
        let tmp = tempfile::tempdir().unwrap();
        let helper = tmp.path().join("valet-helper");
        fs::write(&helper, "").unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
        let mut env = HostEnv::from_process();

        env.helper = helper.clone();
        let (program, argv) = broker_command(&env, "/v/valet");
        assert_eq!(program, "pkexec");
        assert_eq!(argv, [helper.to_string_lossy().as_ref(), "--broker", "/v/valet"]);

        env.helper = tmp.path().join("missing");
        let (_, argv) = broker_command(&env, "/v/valet");
        assert_eq!(argv[..2], ["/bin/bash", "-c"]);
        assert_eq!(argv[2], HELPER_SCRIPT);
        assert_eq!(argv[3..], ["valet-helper", "--broker", "/v/valet"]);
    }
}
