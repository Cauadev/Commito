//! Application state shared by all commands.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use crate::errors::{AppError, AppResult, ErrorCode};
use crate::git::cache::GraphCache;
use crate::git::runner::{discover_git, GitRunner};
use crate::git::undo::UndoEntry;
use crate::settings::Settings;
use crate::watcher::WatchHandle;

pub struct AppState {
    pub git: RwLock<Option<GitRunner>>,
    pub settings: Mutex<Settings>,
    pub settings_path: PathBuf,
    /// Repository roots the user explicitly opened (normalised keys). Commands refuse other paths.
    pub authorized: Mutex<HashSet<String>>,
    pub locks: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    pub graph_cache: tokio::sync::Mutex<HashMap<String, GraphCache>>,
    pub http: reqwest::Client,
    pub watchers: Mutex<HashMap<String, WatchHandle>>,
    pub undo: Mutex<HashMap<String, Vec<UndoEntry>>>,
}

/// Resolved repository handle passed to git functions.
#[derive(Clone)]
pub struct Repo {
    pub git: GitRunner,
    pub path: PathBuf,
    pub key: String,
    pub display: String,
}

pub fn norm_key(path: &str) -> String {
    let mut s = path.replace('\\', "/");
    while s.ends_with('/') && s.len() > 3 {
        s.pop();
    }
    s.to_lowercase()
}

/// Native display form of a path returned by git (`C:/x/y` -> `C:\x\y` on Windows).
pub fn native_path(path: &str) -> String {
    if cfg!(windows) {
        path.replace('/', "\\")
    } else {
        path.to_string()
    }
}

impl AppState {
    pub fn new(settings_path: PathBuf) -> Self {
        let settings = Settings::load(&settings_path);
        let git = discover_git(settings.git_path.as_deref()).map(GitRunner::new);
        Self {
            git: RwLock::new(git),
            settings: Mutex::new(settings),
            settings_path,
            authorized: Mutex::new(HashSet::new()),
            locks: Mutex::new(HashMap::new()),
            graph_cache: tokio::sync::Mutex::new(HashMap::new()),
            http: crate::providers::http_client(),
            watchers: Mutex::new(HashMap::new()),
            undo: Mutex::new(HashMap::new()),
        }
    }

    pub fn git(&self) -> AppResult<GitRunner> {
        self.git.read().unwrap().clone().ok_or_else(|| {
            AppError::new(ErrorCode::GitNotFound, "Git não encontrado neste computador.")
                .recovery("Instale o Git for Windows (inclui o Git Credential Manager) e reinicie o Commito, ou informe o caminho do git.exe nas Configurações.")
        })
    }

    pub fn authorize(&self, path: &str) {
        self.authorized.lock().unwrap().insert(norm_key(path));
    }

    pub fn repo(&self, path: &str) -> AppResult<Repo> {
        let key = norm_key(path);
        if !self.authorized.lock().unwrap().contains(&key) {
            return Err(AppError::new(ErrorCode::RepoNotAuthorized, "Repositório não foi aberto nesta sessão.")
                .recovery("Abra o repositório novamente pela interface.")
                .repo(path));
        }
        Ok(Repo { git: self.git()?, path: PathBuf::from(path), key, display: path.to_string() })
    }

    pub fn lock(&self, repo: &Repo) -> Arc<tokio::sync::Mutex<()>> {
        self.locks.lock().unwrap().entry(repo.key.clone()).or_default().clone()
    }

    pub fn save_settings(&self) -> AppResult<()> {
        let s = self.settings.lock().unwrap().clone();
        s.save(&self.settings_path)
    }

    pub async fn invalidate_graph(&self, repo: &Repo) {
        let prefix = format!("{}|", repo.key);
        self.graph_cache.lock().await.retain(|k, _| !k.starts_with(&prefix));
    }
}
