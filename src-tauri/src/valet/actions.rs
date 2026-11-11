//! Mutations, always delegated to the valet CLI.
//!
//! [`Action`] is a closed set: the frontend can only ask for these commands, every argument is
//! validated here and passed with `Command::args()`. Commands listed in the CLI's
//! `SUDO_COMMANDS` go to the privileged session broker ([`super::privileged`]), started with
//! `pkexec` once per app session, which impersonates `sudo` (`HOME` + `SUDO_USER`) so the CLI
//! keeps writing `~/.config/valet` as the user.

use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::env::HostEnv;
use super::locate::locate_valet;
use super::privileged::{BrokerError, PrivilegedExec, SessionBroker};
use super::runner::{CommandOutput, HostRunner, NativeRunner};

/// Privileged commands may install packages (`use`, `isolate`) and wait for a password.
const ACTION_TIMEOUT: Duration = Duration::from_secs(600);

static SITE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,252}$").expect("valid regex"));
static PHP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d\.\d{1,2}$").expect("valid regex"));

/// Service keys accepted by `valet start|stop|restart [services]*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValetService {
    Nginx,
    Php,
    Dnsmasq,
    Mysql,
    Redis,
    Mailpit,
}

impl ValetService {
    fn as_arg(self) -> &'static str {
        match self {
            Self::Nginx => "nginx",
            Self::Php => "php",
            Self::Dnsmasq => "dnsmasq",
            Self::Mysql => "mysql",
            Self::Redis => "redis",
            Self::Mailpit => "mailpit",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    /// `None` = every Valet service.
    Start {
        service: Option<ValetService>,
    },
    Stop {
        service: Option<ValetService>,
    },
    Restart {
        service: Option<ValetService>,
    },
    Park {
        path: String,
    },
    Forget {
        path: String,
    },
    Link {
        path: String,
        name: String,
    },
    Unlink {
        name: String,
    },
    Secure {
        site: String,
    },
    Unsecure {
        site: String,
    },
    Isolate {
        site: String,
        php: String,
    },
    Unisolate {
        site: String,
    },
    Use {
        php: String,
        update_cli: bool,
    },
    /// Switches only the CLI `php` (broker built-in, see `valet-helper`).
    UseCli {
        php: String,
    },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ActionError {
    #[error("invalid {field}: {reason}")]
    Invalid { field: &'static str, reason: String },

    #[error("valet binary not found in PATH or Composer bin-dir")]
    ValetNotFound,

    #[error(transparent)]
    Privileged(#[from] BrokerError),

    #[error("could not run {program}: {message}")]
    Spawn { program: String, message: String },
}

/// What the frontend shows after an action: the CLI's own output, untouched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActionResult {
    pub success: bool,
    pub code: Option<i32>,
    /// Human-readable command line (`valet secure shop`), for display only.
    pub command: String,
    pub privileged: bool,
    pub stdout: String,
    pub stderr: String,
}

impl Action {
    /// Mirrors `SUDO_COMMANDS` in the CLI's `valet` script.
    pub fn privileged(&self) -> bool {
        matches!(
            self,
            Self::Start { .. }
                | Self::Stop { .. }
                | Self::Restart { .. }
                | Self::Secure { .. }
                | Self::Unsecure { .. }
                | Self::Isolate { .. }
                | Self::Unisolate { .. }
                | Self::Use { .. }
                | Self::UseCli { .. }
        )
    }

    /// Run by the broker itself instead of valet: the CLI has no command for it.
    fn helper_builtin(&self) -> bool {
        matches!(self, Self::UseCli { .. })
    }

    /// Validates the arguments and returns the valet arguments plus the working directory.
    pub fn to_args(&self) -> Result<(Vec<String>, Option<PathBuf>), ActionError> {
        let service_args = |command: &str, service: &Option<ValetService>| {
            let mut args = vec![command.to_owned()];
            args.extend(service.map(|s| s.as_arg().to_owned()));
            args
        };

        let args = match self {
            Self::Start { service } => service_args("start", service),
            Self::Restart { service } => service_args("restart", service),
            Self::Stop { service } => {
                if *service == Some(ValetService::Dnsmasq) {
                    // `valet stop dnsmasq` is silently ignored by the CLI.
                    return Err(invalid("service", "dnsmasq can't be stopped through valet"));
                }
                service_args("stop", service)
            }
            Self::Park { path } => vec!["park".into(), existing_dir(path)?],
            Self::Forget { path } => vec!["forget".into(), absolute_path(path)?],
            Self::Link { path, name } => {
                let dir = existing_dir(path)?;
                return Ok((vec!["link".into(), site_name(name)?], Some(PathBuf::from(dir))));
            }
            Self::Unlink { name } => vec!["unlink".into(), site_name(name)?],
            Self::Secure { site } => vec!["secure".into(), site_name(site)?],
            Self::Unsecure { site } => vec!["unsecure".into(), site_name(site)?],
            Self::Isolate { site, php } => {
                vec!["isolate".into(), php_version(php)?, format!("--site={}", site_name(site)?)]
            }
            Self::Unisolate { site } => vec!["unisolate".into(), format!("--site={}", site_name(site)?)],
            Self::UseCli { php } => vec!["cli-php".into(), php_version(php)?],
            Self::Use { php, update_cli } => {
                let mut args = vec!["use".into(), php_version(php)?];
                if *update_cli {
                    args.push("--update-cli".into());
                }
                args
            }
        };
        Ok((args, None))
    }
}

fn invalid(field: &'static str, reason: impl Into<String>) -> ActionError {
    ActionError::Invalid { field, reason: reason.into() }
}

fn site_name(value: &str) -> Result<String, ActionError> {
    if SITE_RE.is_match(value) {
        Ok(value.to_owned())
    } else {
        Err(invalid("site", format!("'{value}' is not a valid site name")))
    }
}

fn php_version(value: &str) -> Result<String, ActionError> {
    if PHP_RE.is_match(value) {
        Ok(value.to_owned())
    } else {
        Err(invalid("php", format!("'{value}' is not a PHP version like 8.3")))
    }
}

fn absolute_path(value: &str) -> Result<String, ActionError> {
    if value.contains('\0') || !Path::new(value).is_absolute() {
        return Err(invalid("path", format!("'{value}' is not an absolute path")));
    }
    Ok(value.to_owned())
}

fn existing_dir(value: &str) -> Result<String, ActionError> {
    let path = absolute_path(value)?;
    if !Path::new(&path).is_dir() {
        return Err(invalid("path", format!("'{value}' is not a directory")));
    }
    Ok(path)
}

/// Runs `action` through the valet CLI: as the user, or through the privileged session for
/// the commands the CLI itself runs with sudo.
pub fn run_action(
    runner: &dyn HostRunner,
    privileged: &mut dyn PrivilegedExec,
    env: &HostEnv,
    action: &Action,
) -> Result<ActionResult, ActionError> {
    let (mut args, cwd) = action.to_args()?;
    let valet = locate_valet(runner, env).ok_or(ActionError::ValetNotFound)?;
    let valet = valet.to_string_lossy().into_owned();
    let command = match action {
        Action::UseCli { php } => format!("update-alternatives --set php /usr/bin/php{php}"),
        _ => format!("valet {}", args.join(" ")),
    };

    // Never block on a confirmation prompt: there is no terminal to answer it.
    if !action.helper_builtin() {
        args.push("--no-interaction".into());
    }

    let out = if action.privileged() {
        privileged.exec(env, &valet, &args)?
    } else {
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();
        runner
            .run_in(&valet, &argv, cwd.as_deref())
            .map_err(|e| ActionError::Spawn { program: valet.clone(), message: e.to_string() })?
    };

    Ok(result(command, action.privileged(), out))
}

fn result(command: String, privileged: bool, out: CommandOutput) -> ActionResult {
    ActionResult {
        success: out.success(),
        code: out.code,
        command,
        privileged,
        stdout: strip_ansi(&out.stdout),
        stderr: strip_ansi(&out.stderr),
    }
}

/// The CLI colours its output; the UI shows plain text.
fn strip_ansi(text: &str) -> String {
    static ANSI_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]").expect("valid regex"));
    ANSI_RE.replace_all(text, "").trim().to_owned()
}

/// The session broker; its lock also serializes actions, so two pkexec prompts or two valet
/// writes to config.json never race.
static SESSION: LazyLock<Mutex<SessionBroker>> = LazyLock::new(|| Mutex::new(SessionBroker::new(ACTION_TIMEOUT)));

fn session() -> std::sync::MutexGuard<'static, SessionBroker> {
    SESSION.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[tauri::command]
pub async fn run_valet_action(action: Action) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        run_action(&NativeRunner::new(ACTION_TIMEOUT), &mut *session(), &HostEnv::from_process(), &action)
    })
    .await
    .map_err(|e| format!("action task failed: {e}"))?
    .map_err(|e| e.to_string())
}

/// Whether privileged actions currently run without asking for the password again.
/// `try_lock`: while an action runs, the UI just keeps showing the previous value.
#[tauri::command]
pub fn privileged_session_active() -> Option<bool> {
    SESSION.try_lock().ok().map(|mut s| s.is_active())
}

/// Ends the privileged session: the next privileged action asks for the password again.
#[tauri::command]
pub async fn end_privileged_session() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| session().end()).await.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::valet::runner::fake::FakeRunner;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    struct Fx {
        _tmp: tempfile::TempDir,
        env: HostEnv,
        valet: String,
        root: PathBuf,
    }

    fn executable(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "#!/bin/sh\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn fx() -> Fx {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        let valet = root.join("bin/valet");
        executable(&valet);
        let env = HostEnv {
            home: root.join("home"),
            path: Some(std::env::join_paths([root.join("bin")]).unwrap()),
            usr_bin: root.join("usr/bin"),
            force_files: false,
            helper: root.join("libexec/valet-helper"),
        };
        Fx { valet: valet.to_string_lossy().into_owned(), _tmp: tmp, env, root }
    }

    fn args(action: Action) -> Vec<String> {
        action.to_args().unwrap().0
    }

    #[test]
    fn builds_cli_arguments() {
        assert_eq!(args(Action::Restart { service: None }), ["restart"]);
        assert_eq!(args(Action::Stop { service: Some(ValetService::Php) }), ["stop", "php"]);
        assert_eq!(args(Action::Secure { site: "shop".into() }), ["secure", "shop"]);
        assert_eq!(args(Action::Unsecure { site: "app.trice".into() }), ["unsecure", "app.trice"]);
        assert_eq!(args(Action::Isolate { site: "shop".into(), php: "7.4".into() }), ["isolate", "7.4", "--site=shop"]);
        assert_eq!(args(Action::Unisolate { site: "shop".into() }), ["unisolate", "--site=shop"]);
        assert_eq!(args(Action::Use { php: "8.3".into(), update_cli: true }), ["use", "8.3", "--update-cli"]);
        assert_eq!(args(Action::Unlink { name: "shop".into() }), ["unlink", "shop"]);
        assert_eq!(args(Action::Forget { path: "/gone/dir".into() }), ["forget", "/gone/dir"]);
    }

    #[test]
    fn link_runs_inside_the_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().to_string_lossy().into_owned();
        let (args, cwd) = Action::Link { path: path.clone(), name: "shop".into() }.to_args().unwrap();
        assert_eq!(args, ["link", "shop"]);
        assert_eq!(cwd, Some(PathBuf::from(path)));
    }

    #[test]
    fn rejects_unsafe_arguments() {
        let bad = [
            Action::Secure { site: "--help".into() },
            Action::Secure { site: "shop; rm -rf ~".into() },
            Action::Secure { site: "../etc".into() },
            Action::Unlink { name: String::new() },
            Action::Isolate { site: "shop".into(), php: "latest".into() },
            Action::Use { php: "8.3 --ignore-ext".into(), update_cli: false },
            Action::Park { path: "relative/dir".into() },
            Action::Park { path: "/definitely/not/here".into() },
            Action::Forget { path: "-rf".into() },
            Action::Stop { service: Some(ValetService::Dnsmasq) },
        ];
        for action in bad {
            assert!(matches!(action.to_args(), Err(ActionError::Invalid { .. })), "{action:?}");
        }
    }

    #[test]
    fn deserializes_tagged_actions_from_the_frontend() {
        let action: Action = serde_json::from_str(r#"{"kind":"restart","service":"nginx"}"#).unwrap();
        assert_eq!(action, Action::Restart { service: Some(ValetService::Nginx) });
        let action: Action = serde_json::from_str(r#"{"kind":"use","php":"8.4","update_cli":false}"#).unwrap();
        assert_eq!(action, Action::Use { php: "8.4".into(), update_cli: false });
        assert!(serde_json::from_str::<Action>(r#"{"kind":"uninstall"}"#).is_err());
        assert!(serde_json::from_str::<Action>(r#"{"kind":"restart","service":"sshd"}"#).is_err());
    }

    #[test]
    fn privileged_set_matches_sudo_commands() {
        assert!(Action::Restart { service: None }.privileged());
        assert!(Action::Use { php: "8.3".into(), update_cli: false }.privileged());
        assert!(Action::Isolate { site: "a".into(), php: "8.3".into() }.privileged());
        assert!(!Action::Park { path: "/x".into() }.privileged());
        assert!(!Action::Link { path: "/x".into(), name: "a".into() }.privileged());
        assert!(!Action::Unlink { name: "a".into() }.privileged());
    }

    /// Records privileged calls instead of running pkexec.
    #[derive(Default)]
    struct FakePrivileged {
        calls: Vec<(String, Vec<String>)>,
        reply: Option<Result<CommandOutput, BrokerError>>,
    }

    impl PrivilegedExec for FakePrivileged {
        fn exec(&mut self, _env: &HostEnv, valet: &str, args: &[String]) -> Result<CommandOutput, BrokerError> {
            self.calls.push((valet.to_owned(), args.to_vec()));
            self.reply
                .clone()
                .unwrap_or_else(|| Ok(CommandOutput { code: Some(0), stdout: "done".into(), stderr: String::new() }))
        }
        fn is_active(&mut self) -> bool {
            !self.calls.is_empty()
        }
        fn end(&mut self) {}
    }

    #[test]
    fn runs_unprivileged_actions_as_the_user() {
        let fx = fx();
        let dir = fx.root.join("sites");
        fs::create_dir_all(&dir).unwrap();
        let cmd = format!("{} park {} --no-interaction", fx.valet, dir.display());
        let runner = FakeRunner::new().ok(&cmd, "\x1b[32mThe [x] directory has been added.\x1b[0m\n");
        let mut privileged = FakePrivileged::default();

        let action = Action::Park { path: dir.to_string_lossy().into() };
        let result = run_action(&runner, &mut privileged, &fx.env, &action).unwrap();

        assert!(result.success);
        assert!(!result.privileged);
        assert!(privileged.calls.is_empty());
        assert_eq!(result.stdout, "The [x] directory has been added.");
        assert_eq!(result.command, format!("valet park {}", dir.display()));
    }

    #[test]
    fn link_passes_the_folder_as_cwd() {
        let fx = fx();
        let dir = fx.root.join("projects/shop");
        fs::create_dir_all(&dir).unwrap();
        let runner = FakeRunner::new().ok(&format!("{} link shop --no-interaction", fx.valet), "");

        let action = Action::Link { path: dir.to_string_lossy().into(), name: "shop".into() };
        run_action(&runner, &mut FakePrivileged::default(), &fx.env, &action).unwrap();

        assert_eq!(runner.last_cwd(), Some(dir));
    }

    #[test]
    fn privileged_actions_go_to_the_session_broker() {
        let fx = fx();
        let runner = FakeRunner::new();
        let mut privileged = FakePrivileged::default();

        let action = Action::Restart { service: Some(ValetService::Nginx) };
        let result = run_action(&runner, &mut privileged, &fx.env, &action).unwrap();

        assert!(result.success && result.privileged);
        assert_eq!(result.command, "valet restart nginx");
        assert_eq!(
            privileged.calls,
            [(fx.valet.clone(), vec!["restart".into(), "nginx".into(), "--no-interaction".into()])]
        );
        assert!(!runner.called(&format!("{} restart nginx --no-interaction", fx.valet)), "never runs unprivileged");
    }

    #[test]
    fn switches_only_the_cli_php_through_the_broker() {
        let fx = fx();
        let mut privileged = FakePrivileged::default();

        let result =
            run_action(&FakeRunner::new(), &mut privileged, &fx.env, &Action::UseCli { php: "8.4".into() }).unwrap();

        assert!(result.privileged);
        assert_eq!(result.command, "update-alternatives --set php /usr/bin/php8.4");
        assert_eq!(privileged.calls[0].1, ["cli-php", "8.4"]);
        assert!(matches!(Action::UseCli { php: "8.4 x".into() }.to_args(), Err(ActionError::Invalid { .. })));
        let action: Action = serde_json::from_str(r#"{"kind":"use_cli","php":"8.3"}"#).unwrap();
        assert_eq!(action, Action::UseCli { php: "8.3".into() });
    }

    #[test]
    fn surfaces_broker_failures_and_cli_errors() {
        let fx = fx();
        let mut cancelled = FakePrivileged { reply: Some(Err(BrokerError::AuthCancelled)), ..Default::default() };
        assert_eq!(
            run_action(&FakeRunner::new(), &mut cancelled, &fx.env, &Action::Restart { service: None }),
            Err(ActionError::Privileged(BrokerError::AuthCancelled))
        );

        let failed =
            CommandOutput { code: Some(1), stdout: String::new(), stderr: "Job for nginx.service failed".into() };
        let mut privileged = FakePrivileged { reply: Some(Ok(failed)), ..Default::default() };
        let result =
            run_action(&FakeRunner::new(), &mut privileged, &fx.env, &Action::Restart { service: None }).unwrap();
        assert!(!result.success);
        assert_eq!(result.stderr, "Job for nginx.service failed");
    }

    #[test]
    fn validates_before_asking_for_privileges() {
        let fx = fx();
        let mut privileged = FakePrivileged::default();
        let action = Action::Secure { site: "--help".into() };
        assert!(matches!(
            run_action(&FakeRunner::new(), &mut privileged, &fx.env, &action),
            Err(ActionError::Invalid { .. })
        ));
        assert!(privileged.calls.is_empty());
    }

    #[test]
    fn fails_without_valet() {
        let mut fx = fx();
        fx.env.path = None;
        assert_eq!(
            run_action(&FakeRunner::new(), &mut FakePrivileged::default(), &fx.env, &Action::Restart { service: None }),
            Err(ActionError::ValetNotFound)
        );
    }

    /// Parks a temporary folder through the real CLI, checks `valet state`, then forgets it:
    /// `cargo test -- --ignored real_machine`.
    #[test]
    #[ignore = "changes the real Valet config (reverted at the end)"]
    fn real_machine_park_and_forget() {
        use crate::valet::state::load_state;

        let runner = NativeRunner::new(Duration::from_secs(60));
        let env = HostEnv { force_files: false, ..HostEnv::from_process() };
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("ui-e2e-site")).unwrap();
        let path = tmp.path().to_string_lossy().into_owned();

        let mut privileged = SessionBroker::new(ACTION_TIMEOUT);
        let parked = run_action(&runner, &mut privileged, &env, &Action::Park { path: path.clone() }).unwrap();
        let state = load_state(&runner, &env).unwrap();
        let forgot = run_action(&runner, &mut privileged, &env, &Action::Forget { path: path.clone() }).unwrap();
        let after = load_state(&runner, &env).unwrap();

        assert!(parked.success, "{parked:?}");
        assert!(state.paths.contains(&path));
        assert!(state.sites.iter().any(|s| s.name == "ui-e2e-site"));
        assert!(forgot.success, "{forgot:?}");
        assert!(!after.paths.contains(&path));
        assert!(!after.sites.iter().any(|s| s.name == "ui-e2e-site"));
    }
}
