//! File system watcher: notifies the UI (debounced) when the working tree or git metadata
//! change outside of the app (IDE, terminal, other tools).
//!
//! Uses the raw OS watcher (ReadDirectoryChangesW on Windows), which starts in O(1) even on
//! huge trees, plus our own debounce thread. Paths that never matter for `git status` noise
//! (`node_modules`, other worktrees nested inside this one, git object/log churn) are dropped
//! before they can trigger refreshes.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{AppHandle, Emitter};

use crate::events::{RepoChangedPayload, REPO_CHANGED_EVENT};

const DEBOUNCE: Duration = Duration::from_millis(400);

pub struct WatchHandle {
    _watcher: RecommendedWatcher,
}

fn norm(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/").to_lowercase()
}

/// Classifies a changed path: `None` = ignore, `Some(true)` = git metadata, `Some(false)` = worktree.
pub fn classify(path: &Path, git_dirs: &[String], ignored_roots: &[String]) -> Option<bool> {
    let lower = norm(path);
    for gd in git_dirs {
        if let Some(rest) = lower.strip_prefix(gd.as_str()) {
            let rest = rest.trim_start_matches('/');
            if rest.starts_with("objects") || rest.starts_with("logs") || rest.ends_with(".lock") || rest.starts_with("fsmonitor") || rest.is_empty() {
                return None;
            }
            // Other worktrees' private state (…/worktrees/<name>/…) also lives here.
            return Some(true);
        }
    }
    if ignored_roots.iter().any(|r| lower.starts_with(r.as_str())) {
        return None;
    }
    if lower.contains("/node_modules/") || lower.ends_with("/node_modules") || lower.contains("/.git/") || lower.ends_with("/.git") {
        return None;
    }
    Some(false)
}

/// `nested_worktrees`: other worktrees located inside `root` (their edits must not refresh this tab).
pub fn watch(app: &AppHandle, repo: &str, root: &Path, git_dir: &Path, common_dir: &Path, nested_worktrees: &[PathBuf]) -> notify::Result<WatchHandle> {
    let (tx, rx) = mpsc::channel::<notify::Result<notify::Event>>();
    let mut watcher = notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    })?;
    watcher.watch(root, RecursiveMode::Recursive)?;
    for d in [git_dir, common_dir] {
        if d.is_dir() && !d.starts_with(root) {
            let _ = watcher.watch(d, RecursiveMode::Recursive);
        }
    }

    let git_dirs: Vec<String> = [git_dir.to_path_buf(), common_dir.to_path_buf(), root.join(".git")].iter().map(|p| norm(p)).collect();
    let ignored: Vec<String> = nested_worktrees.iter().map(|p| norm(p)).filter(|p| p != &norm(root)).collect();
    let app = app.clone();
    let repo_id = repo.to_string();
    std::thread::Builder::new()
        .name("fg-watcher".into())
        .spawn(move || {
            let mut pending: Option<(Instant, bool)> = None;
            loop {
                let timeout = match pending {
                    Some((since, _)) => DEBOUNCE.saturating_sub(since.elapsed()),
                    None => Duration::from_secs(3600),
                };
                match rx.recv_timeout(timeout) {
                    Ok(Ok(ev)) => {
                        for p in &ev.paths {
                            if let Some(is_git) = classify(p, &git_dirs, &ignored) {
                                let git = pending.map(|(_, g)| g).unwrap_or(false) || is_git;
                                let since = pending.map(|(s, _)| s).unwrap_or_else(Instant::now);
                                pending = Some((since, git));
                            }
                        }
                    }
                    Ok(Err(_)) => {}
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if let Some((_, git)) = pending.take() {
                            let _ = app.emit(REPO_CHANGED_EVENT, RepoChangedPayload { repo: repo_id.clone(), kind: if git { "git".into() } else { "worktree".into() } });
                        }
                    }
                    // Watcher dropped (tab closed): stop the thread.
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        })
        .map_err(|e| notify::Error::generic(&e.to_string()))?;
    Ok(WatchHandle { _watcher: watcher })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_paths() {
        let gd = vec!["c:/r/.git".to_string()];
        let ign = vec!["c:/r/.worktrees/a".to_string()];
        assert_eq!(classify(Path::new("C:/r/src/a.rs"), &gd, &ign), Some(false));
        assert_eq!(classify(Path::new("C:\\r\\.git\\refs\\heads\\main"), &gd, &ign), Some(true));
        assert_eq!(classify(Path::new("C:/r/.git/HEAD"), &gd, &ign), Some(true));
        assert_eq!(classify(Path::new("C:/r/.git/index.lock"), &gd, &ign), None);
        assert_eq!(classify(Path::new("C:/r/.git/objects/ab/cd"), &gd, &ign), None);
        assert_eq!(classify(Path::new("C:/r/web/node_modules/x/y.js"), &gd, &ign), None);
        assert_eq!(classify(Path::new("C:\\r\\.worktrees\\a\\src\\main.ts"), &gd, &ign), None);
    }
}
