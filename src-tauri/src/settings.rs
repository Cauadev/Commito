//! Non-secret persisted configuration (JSON in the app config dir).
//! Secrets (tokens) never live here — only opaque account ids that key the OS vault.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::errors::AppResult;

pub const SETTINGS_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct RecentRepo {
    pub path: String,
    pub name: String,
    pub last_opened: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Github,
    Gitlab,
    Bitbucket,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAccount {
    pub id: String,
    pub provider: Provider,
    /// API base URL (GitHub Enterprise / GitLab self-managed); None = public cloud.
    pub base_url: Option<String>,
    pub username: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    /// Atlassian account e-mail used for Bitbucket REST basic auth.
    pub email: Option<String>,
    pub label: Option<String>,
    pub created_at: i64,
    pub scopes: Option<Vec<String>>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub version: u32,
    pub git_path: Option<String>,
    pub recent_repos: Vec<RecentRepo>,
    pub open_tabs: Vec<String>,
    pub active_tab: Option<String>,
    pub accounts: Vec<ProviderAccount>,
    /// repo path -> account id used to authenticate Git HTTPS for that repository.
    pub repo_auth: HashMap<String, String>,
    pub clone_dir: Option<String>,
    pub terminal: Option<String>,
    /// UI-owned preferences (panel sizes, graph columns, theme...). Opaque to Rust.
    pub ui: serde_json::Value,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            git_path: None,
            recent_repos: Vec::new(),
            open_tabs: Vec::new(),
            active_tab: None,
            accounts: Vec::new(),
            repo_auth: HashMap::new(),
            clone_dir: None,
            terminal: None,
            ui: serde_json::Value::Object(Default::default()),
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => match serde_json::from_str::<Settings>(&text) {
                Ok(mut s) => {
                    s.migrate();
                    s
                }
                Err(_) => {
                    // Keep the broken file for inspection instead of silently losing it.
                    let _ = std::fs::copy(path, path.with_extension("json.bak"));
                    Settings::default()
                }
            },
            Err(_) => Settings::default(),
        }
    }

    fn migrate(&mut self) {
        if self.version < SETTINGS_VERSION {
            self.version = SETTINGS_VERSION;
        }
        if !self.ui.is_object() {
            self.ui = serde_json::Value::Object(Default::default());
        }
    }

    pub fn save(&self, path: &Path) -> AppResult<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp: PathBuf = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn touch_recent(&mut self, path: &str, name: &str) {
        let now = now_secs();
        self.recent_repos.retain(|r| !same_path(&r.path, path));
        self.recent_repos.insert(0, RecentRepo { path: path.to_string(), name: name.to_string(), last_opened: now });
        self.recent_repos.truncate(30);
    }
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn same_path(a: &str, b: &str) -> bool {
    let n = |s: &str| s.replace('\\', "/").trim_end_matches('/').to_lowercase();
    n(a) == n(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_recent_dedupe() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        let mut s = Settings::load(&p);
        s.touch_recent("C:\\a\\repo", "repo");
        s.touch_recent("c:/a/repo/", "repo");
        assert_eq!(s.recent_repos.len(), 1);
        s.save(&p).unwrap();
        let s2 = Settings::load(&p);
        assert_eq!(s2.recent_repos.len(), 1);
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(!text.to_lowercase().contains("token"));
    }

    #[test]
    fn corrupt_file_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        std::fs::write(&p, "{not json").unwrap();
        let s = Settings::load(&p);
        assert!(s.recent_repos.is_empty());
        assert!(p.with_extension("json.bak").exists());
    }
}
