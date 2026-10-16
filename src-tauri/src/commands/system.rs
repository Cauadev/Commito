//! App-level commands: environment diagnostics, settings, shell integration.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::State;

use crate::errors::{AppError, AppResult, ErrorCode};
use crate::git::runner::{discover_git, GitRunner, RunOpts};
use crate::settings::Settings;
use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub git_found: bool,
    pub git_path: Option<String>,
    pub git_version: Option<String>,
    pub git_version_ok: bool,
    pub gcm_version: Option<String>,
    pub credential_helper: Option<String>,
    pub lfs_version: Option<String>,
    pub platform: String,
    pub settings_path: String,
}

pub const MIN_GIT: (u32, u32) = (2, 36);

fn parse_git_version(s: &str) -> Option<(u32, u32)> {
    let v = s.trim().strip_prefix("git version ")?;
    let mut it = v.split('.');
    Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?))
}

async fn opt_text(git: &GitRunner, args: &[&str]) -> Option<String> {
    let cwd = std::env::temp_dir();
    let o = git.run(&cwd, args, RunOpts::read()).await.ok()?;
    o.ok().then(|| o.text().trim().to_string()).filter(|s| !s.is_empty())
}

#[tauri::command]
pub async fn app_info(state: State<'_, AppState>) -> AppResult<AppInfo> {
    let git = state.git.read().unwrap().clone();
    let mut info = AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        git_found: git.is_some(),
        git_path: git.as_ref().map(|g| g.git_path.display().to_string()),
        git_version: None,
        git_version_ok: false,
        gcm_version: None,
        credential_helper: None,
        lfs_version: None,
        platform: std::env::consts::OS.to_string(),
        settings_path: state.settings_path.display().to_string(),
    };
    if let Some(g) = git {
        info.git_version = opt_text(&g, &["--version"]).await;
        info.git_version_ok = info.git_version.as_deref().and_then(parse_git_version).map(|v| v >= MIN_GIT).unwrap_or(false);
        info.credential_helper = opt_text(&g, &["config", "--get-all", "credential.helper"]).await;
        info.gcm_version = opt_text(&g, &["credential-manager", "--version"]).await;
        info.lfs_version = opt_text(&g, &["lfs", "version"]).await;
    }
    Ok(info)
}

#[tauri::command]
pub async fn set_git_path(state: State<'_, AppState>, path: Option<String>) -> AppResult<()> {
    let found = discover_git(path.as_deref());
    if path.as_deref().is_some_and(|p| !p.trim().is_empty()) && found.as_ref().map(|f| f.display().to_string()) != path {
        return Err(AppError::invalid("Executável do Git não encontrado nesse caminho."));
    }
    *state.git.write().unwrap() = found.map(GitRunner::new);
    state.settings.lock().unwrap().git_path = path.filter(|p| !p.trim().is_empty());
    state.save_settings()
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn save_ui_settings(state: State<'_, AppState>, ui: serde_json::Value) -> AppResult<()> {
    if !ui.is_object() {
        return Err(AppError::invalid("Preferências inválidas."));
    }
    state.settings.lock().unwrap().ui = ui;
    state.save_settings()
}

#[tauri::command]
pub fn save_session(state: State<'_, AppState>, tabs: Vec<String>, active: Option<String>) -> AppResult<()> {
    {
        let mut s = state.settings.lock().unwrap();
        s.open_tabs = tabs;
        s.active_tab = active;
    }
    state.save_settings()
}

#[tauri::command]
pub fn set_preferences(state: State<'_, AppState>, clone_dir: Option<String>, terminal: Option<String>) -> AppResult<()> {
    {
        let mut s = state.settings.lock().unwrap();
        s.clone_dir = clone_dir.filter(|c| !c.trim().is_empty());
        s.terminal = terminal.filter(|c| !c.trim().is_empty());
    }
    state.save_settings()
}

#[tauri::command]
pub fn remove_recent(state: State<'_, AppState>, path: String) -> AppResult<()> {
    state.settings.lock().unwrap().recent_repos.retain(|r| !crate::settings::same_path(&r.path, &path));
    state.save_settings()
}

#[tauri::command]
pub fn default_clone_dir(state: State<'_, AppState>) -> String {
    if let Some(d) = state.settings.lock().unwrap().clone_dir.clone() {
        return d;
    }
    dirs::home_dir().map(|h| h.join("source").join("repos").display().to_string()).unwrap_or_default()
}

#[tauri::command]
pub fn cancel_operation(op_id: String) -> bool {
    crate::git::runner::cancel_operation(&op_id)
}

fn ensure_known_path(state: &AppState, path: &str) -> AppResult<PathBuf> {
    // Shell integrations only act on folders inside repositories opened in this session.
    let key = crate::state::norm_key(path);
    let ok = state.authorized.lock().unwrap().iter().any(|r| key == *r || key.starts_with(&format!("{r}/")));
    if !ok {
        return Err(AppError::new(ErrorCode::RepoNotAuthorized, "Caminho fora dos repositórios abertos."));
    }
    Ok(PathBuf::from(path))
}

#[tauri::command]
pub fn open_in_explorer(state: State<'_, AppState>, path: String) -> AppResult<()> {
    let p = ensure_known_path(&state, &path)?;
    if p.is_file() {
        tauri_plugin_opener::reveal_item_in_dir(&p).map_err(|e| AppError::new(ErrorCode::Io, "Não foi possível abrir o Explorer.").details(e.to_string()))
    } else {
        tauri_plugin_opener::open_path(p.display().to_string(), None::<&str>).map_err(|e| AppError::new(ErrorCode::Io, "Não foi possível abrir o Explorer.").details(e.to_string()))
    }
}

#[tauri::command]
pub fn open_repo_file(state: State<'_, AppState>, repo: String, path: String) -> AppResult<()> {
    crate::git::read::validate_path(&path)?;
    let root = ensure_known_path(&state, &repo)?;
    let full = root.join(&path);
    if !full.exists() {
        return Err(AppError::new(ErrorCode::NotFound, "Arquivo não existe no working directory."));
    }
    tauri_plugin_opener::open_path(full.display().to_string(), None::<&str>).map_err(|e| AppError::new(ErrorCode::Io, "Não foi possível abrir o arquivo.").details(e.to_string()))
}

#[tauri::command]
pub fn open_url(url: String) -> AppResult<()> {
    let parsed = url::Url::parse(&url).map_err(|_| AppError::invalid("URL inválida."))?;
    if parsed.scheme() != "https" && parsed.scheme() != "http" {
        return Err(AppError::invalid("Apenas URLs http(s) podem ser abertas."));
    }
    tauri_plugin_opener::open_url(parsed.as_str(), None::<&str>).map_err(|e| AppError::new(ErrorCode::Io, "Não foi possível abrir o navegador.").details(e.to_string()))
}

#[tauri::command]
pub fn open_terminal(state: State<'_, AppState>, path: String) -> AppResult<()> {
    let p = ensure_known_path(&state, &path)?;
    let pref = state.settings.lock().unwrap().terminal.clone().unwrap_or_else(|| "auto".into());
    spawn_terminal(&p, &pref, state.git.read().unwrap().as_ref().map(|g| g.git_path.clone()))
}

#[cfg(windows)]
fn spawn_terminal(dir: &Path, pref: &str, git_path: Option<PathBuf>) -> AppResult<()> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
    let which = |exe: &str| crate::git::runner::discover_in_path(exe);
    let git_bash = git_path.and_then(|g| {
        // ...\Git\cmd\git.exe -> ...\Git\git-bash.exe
        g.parent()?.parent().map(|root| root.join("git-bash.exe")).filter(|p| p.is_file())
    });
    let res = match pref {
        "git-bash" if git_bash.is_some() => Command::new(git_bash.unwrap()).arg(format!("--cd={}", dir.display())).spawn(),
        "cmd" => Command::new("cmd.exe").current_dir(dir).creation_flags(CREATE_NEW_CONSOLE).spawn(),
        "powershell" => Command::new("powershell.exe").args(["-NoExit"]).current_dir(dir).creation_flags(CREATE_NEW_CONSOLE).spawn(),
        _ => {
            if let Some(wt) = which("wt.exe") {
                Command::new(wt).arg("-d").arg(dir).spawn()
            } else if let Some(pwsh) = which("pwsh.exe") {
                Command::new(pwsh).args(["-NoExit"]).current_dir(dir).creation_flags(CREATE_NEW_CONSOLE).spawn()
            } else {
                Command::new("powershell.exe").args(["-NoExit"]).current_dir(dir).creation_flags(CREATE_NEW_CONSOLE).spawn()
            }
        }
    };
    res.map(|_| ()).map_err(|e| AppError::new(ErrorCode::Io, "Não foi possível abrir o terminal.").details(e.to_string()))
}

#[cfg(not(windows))]
fn spawn_terminal(dir: &Path, _pref: &str, _git: Option<PathBuf>) -> AppResult<()> {
    std::process::Command::new("x-terminal-emulator").current_dir(dir).spawn().map(|_| ()).map_err(AppError::from)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshDiagnostics {
    pub ssh_found: bool,
    pub agent_keys: Vec<String>,
    pub agent_error: Option<String>,
    pub config_exists: bool,
    pub known_hosts_exists: bool,
    pub public_keys: Vec<String>,
}

#[tauri::command]
pub async fn ssh_diagnostics() -> AppResult<SshDiagnostics> {
    let home = dirs::home_dir().unwrap_or_default();
    let ssh_dir = home.join(".ssh");
    let mut d = SshDiagnostics {
        ssh_found: crate::git::runner::discover_in_path(if cfg!(windows) { "ssh.exe" } else { "ssh" }).is_some(),
        agent_keys: vec![],
        agent_error: None,
        config_exists: ssh_dir.join("config").is_file(),
        known_hosts_exists: ssh_dir.join("known_hosts").is_file(),
        public_keys: std::fs::read_dir(&ssh_dir)
            .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".pub")).collect())
            .unwrap_or_default(),
    };
    let mut cmd = tokio::process::Command::new(if cfg!(windows) { "ssh-add.exe" } else { "ssh-add" });
    cmd.arg("-l").stdin(std::process::Stdio::null()).kill_on_drop(true);
    #[cfg(windows)]
    {
        cmd.creation_flags(0x0800_0000);
    }
    match tokio::time::timeout(std::time::Duration::from_secs(5), cmd.output()).await {
        Ok(Ok(out)) => {
            let text = String::from_utf8_lossy(&out.stdout).into_owned();
            if out.status.success() {
                // "256 SHA256:xxxx comment (ED25519)" — fingerprints only, never key material.
                d.agent_keys = text.lines().map(str::to_string).collect();
            } else {
                let err = format!("{}{}", text, String::from_utf8_lossy(&out.stderr)).trim().to_string();
                d.agent_error = Some(if err.contains("no identities") { "O agente SSH está ativo, mas não tem chaves carregadas (ssh-add).".into() } else { err });
            }
        }
        _ => d.agent_error = Some("ssh-add não disponível ou o serviço ssh-agent não está em execução.".into()),
    }
    Ok(d)
}

#[cfg(test)]
mod tests {
    #[test]
    fn git_version_parse() {
        assert_eq!(super::parse_git_version("git version 2.45.2.windows.1"), Some((2, 45)));
        assert!(super::parse_git_version("git version 2.30.0").unwrap() < super::MIN_GIT);
    }
}

