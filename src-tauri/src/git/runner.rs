//! Centralised git process execution.
//!
//! * Arguments are always passed as a vector (never through a shell), so branch names,
//!   paths or messages cannot inject commands.
//! * stdout/stderr are captured separately; stderr can be streamed as progress events.
//! * Long operations can be cancelled by op id (kills the whole process tree on Windows).

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::oneshot;

use crate::errors::{classify_git_failure, AppError, AppResult, ErrorCode};
use crate::events::ProgressSink;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Global options prepended to every invocation: stable, machine friendly output.
const BASE_ARGS: &[&str] = &[
    "--no-pager",
    "-c",
    "core.quotepath=false",
    "-c",
    "color.ui=never",
    "-c",
    "i18n.logOutputEncoding=UTF-8",
    "-c",
    "advice.detachedHead=false",
];

const MAX_STDERR: usize = 512 * 1024;

static CANCELLERS: LazyLock<Mutex<HashMap<String, oneshot::Sender<()>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Requests cancellation of a running operation. Returns false when nothing was running.
pub fn cancel_operation(op_id: &str) -> bool {
    let tx = CANCELLERS.lock().unwrap().remove(op_id);
    match tx {
        Some(tx) => tx.send(()).is_ok(),
        None => false,
    }
}

#[derive(Debug)]
pub struct GitOutput {
    pub code: i32,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

impl GitOutput {
    pub fn ok(&self) -> bool {
        self.code == 0
    }
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }
}

#[derive(Default)]
pub struct RunOpts {
    pub stdin: Option<Vec<u8>>,
    pub env: Vec<(String, OsString)>,
    pub timeout: Option<Duration>,
    pub progress: Option<ProgressSink>,
    /// Allow the operation to talk to remotes interactively (credential manager UI).
    pub network: bool,
    /// Read-only commands skip optional locks so background refreshes never block writers.
    pub read_only: bool,
}

impl RunOpts {
    pub fn read() -> Self {
        Self { read_only: true, timeout: Some(Duration::from_secs(120)), ..Default::default() }
    }
    pub fn write() -> Self {
        Self { timeout: Some(Duration::from_secs(600)), ..Default::default() }
    }
    pub fn network(progress: Option<ProgressSink>) -> Self {
        Self { network: true, progress, ..Default::default() }
    }
    pub fn stdin(mut self, data: Vec<u8>) -> Self {
        self.stdin = Some(data);
        self
    }
    pub fn env(mut self, key: &str, value: impl Into<OsString>) -> Self {
        self.env.push((key.to_string(), value.into()));
        self
    }
}

#[derive(Clone, Debug)]
pub struct GitRunner {
    pub git_path: PathBuf,
}

impl GitRunner {
    pub fn new(git_path: PathBuf) -> Self {
        Self { git_path }
    }

    /// Runs git and returns its output regardless of exit code.
    pub async fn run<S: AsRef<str>>(&self, cwd: &Path, args: &[S], opts: RunOpts) -> AppResult<GitOutput> {
        let args: Vec<String> = args.iter().map(|a| a.as_ref().to_string()).collect();
        let mut cmd = tokio::process::Command::new(&self.git_path);
        cmd.current_dir(cwd)
            .args(BASE_ARGS)
            .args(&args)
            .stdin(if opts.stdin.is_some() { Stdio::piped() } else { Stdio::null() })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            // Never open an interactive editor: commit messages are always provided explicitly.
            .env("GIT_EDITOR", "true")
            .env("GIT_MERGE_AUTOEDIT", "no")
            // English messages make the (secondary) stderr classification reliable.
            .env("LANGUAGE", "en")
            .env("LC_MESSAGES", "C")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE");
        if opts.read_only {
            cmd.env("GIT_OPTIONAL_LOCKS", "0");
        }
        if !opts.network {
            // Local commands must never wait on a credential prompt.
            cmd.env("GIT_TERMINAL_PROMPT", "0");
        }
        for (k, v) in &opts.env {
            cmd.env(k, v);
        }
        #[cfg(windows)]
        {
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AppError::new(ErrorCode::GitNotFound, "Git não encontrado.")
                    .recovery("Instale o Git for Windows (https://git-scm.com/download/win) ou configure o caminho do executável nas Configurações.")
            } else {
                AppError::from(e)
            }
        })?;

        if let Some(data) = opts.stdin {
            if let Some(mut stdin) = child.stdin.take() {
                tokio::spawn(async move {
                    let _ = stdin.write_all(&data).await;
                    let _ = stdin.shutdown().await;
                });
            }
        }

        let mut stdout = child.stdout.take().expect("stdout piped");
        let mut stderr = child.stderr.take().expect("stderr piped");
        let out_task = tokio::spawn(async move {
            let mut buf = Vec::new();
            let _ = stdout.read_to_end(&mut buf).await;
            buf
        });
        let progress = opts.progress.clone();
        let err_task = tokio::spawn(async move {
            let mut collected = Vec::new();
            let mut chunk = [0u8; 4096];
            let mut pending = Vec::new();
            loop {
                match stderr.read(&mut chunk).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if collected.len() < MAX_STDERR {
                            collected.extend_from_slice(&chunk[..n]);
                        }
                        if let Some(p) = &progress {
                            pending.extend_from_slice(&chunk[..n]);
                            while let Some(pos) = pending.iter().position(|b| *b == b'\r' || *b == b'\n') {
                                let line: Vec<u8> = pending.drain(..=pos).collect();
                                let line = String::from_utf8_lossy(&line).trim().to_string();
                                if !line.is_empty() {
                                    p.line(&line);
                                }
                            }
                        }
                    }
                }
            }
            String::from_utf8_lossy(&collected).into_owned()
        });

        let op_id = opts.progress.as_ref().map(|p| p.op_id.clone());
        let cancel_rx = op_id.as_ref().map(|id| {
            let (tx, rx) = oneshot::channel::<()>();
            CANCELLERS.lock().unwrap().insert(id.clone(), tx);
            rx
        });

        let pid = child.id();
        let wait = async {
            match cancel_rx {
                Some(rx) => {
                    tokio::select! {
                        status = child.wait() => Ok(status),
                        Ok(()) = rx => Err(()),
                    }
                }
                None => Ok(child.wait().await),
            }
        };
        let result = match opts.timeout {
            Some(t) => match tokio::time::timeout(t, wait).await {
                Ok(r) => r.map(Some),
                Err(_) => Ok(None),
            },
            None => wait.await.map(Some),
        };
        if let Some(id) = &op_id {
            CANCELLERS.lock().unwrap().remove(id);
        }

        let status = match result {
            Ok(Some(status)) => status?,
            Ok(None) => {
                kill_tree(pid);
                let _ = child.kill().await;
                return Err(AppError::new(ErrorCode::Timeout, format!("git {} excedeu o tempo limite.", args.first().cloned().unwrap_or_default())));
            }
            Err(()) => {
                kill_tree(pid);
                let _ = child.kill().await;
                return Err(AppError::new(ErrorCode::Cancelled, "Operação cancelada."));
            }
        };
        let stdout = out_task.await.unwrap_or_default();
        let stderr = err_task.await.unwrap_or_default();
        Ok(GitOutput { code: status.code().unwrap_or(-1), stdout, stderr })
    }

    /// Runs git and converts a non-zero exit code into a classified [`AppError`].
    pub async fn run_ok<S: AsRef<str>>(&self, cwd: &Path, args: &[S], opts: RunOpts) -> AppResult<GitOutput> {
        let out = self.run(cwd, args, opts).await?;
        if out.ok() {
            Ok(out)
        } else {
            let args: Vec<String> = args.iter().map(|a| a.as_ref().to_string()).collect();
            let mut details = out.stderr.clone();
            if details.trim().is_empty() {
                details = out.text();
            }
            Err(classify_git_failure(&args, &details))
        }
    }

    /// Convenience: read-only command returning stdout as text.
    pub async fn read<S: AsRef<str>>(&self, cwd: &Path, args: &[S]) -> AppResult<String> {
        Ok(self.run_ok(cwd, args, RunOpts::read()).await?.text())
    }
}

#[cfg(windows)]
fn kill_tree(pid: Option<u32>) {
    use std::os::windows::process::CommandExt;
    if let Some(pid) = pid {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

#[cfg(not(windows))]
fn kill_tree(_pid: Option<u32>) {}

/// Locate a git executable: explicit setting, PATH, then the default Git for Windows locations.
pub fn discover_git(configured: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = configured.filter(|p| !p.trim().is_empty()) {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Some(p) = discover_in_path(if cfg!(windows) { "git.exe" } else { "git" }) {
        return Some(p);
    }
    #[cfg(windows)]
    {
        let mut roots = vec![];
        for var in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)", "LOCALAPPDATA"] {
            if let Some(v) = std::env::var_os(var) {
                roots.push(PathBuf::from(v));
            }
        }
        for root in roots {
            for sub in ["Git\\cmd\\git.exe", "Programs\\Git\\cmd\\git.exe"] {
                let c = root.join(sub);
                if c.is_file() {
                    return Some(c);
                }
            }
        }
    }
    None
}

pub fn discover_in_path(exe: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths).map(|d| d.join(exe)).find(|c| c.is_file())
}
