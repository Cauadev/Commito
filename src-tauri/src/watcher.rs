//! File system watcher: notifies the UI (debounced) when the working tree or git metadata
//! change outside of the app (IDE, terminal, other tools).

use std::path::{Path, PathBuf};
use std::time::Duration;

use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use tauri::{AppHandle, Emitter};

use crate::events::{RepoChangedPayload, REPO_CHANGED_EVENT};

pub struct WatchHandle {
    _inner: Box<dyn std::any::Any + Send>,
}

/// Classifies a changed path: `None` = ignore, `Some(true)` = git metadata, `Some(false)` = worktree.
pub fn classify(path: &Path, git_dirs: &[PathBuf]) -> Option<bool> {
    let lower = path.to_string_lossy().replace('\\', "/").to_lowercase();
    for gd in git_dirs {
        let g = gd.to_string_lossy().replace('\\', "/").to_lowercase();
        if let Some(rest) = lower.strip_prefix(&g) {
            let rest = rest.trim_start_matches('/');
            if rest.starts_with("objects") || rest.starts_with("logs") || rest.ends_with(".lock") || rest.starts_with("fsmonitor") || rest.is_empty() {
                return None;
            }
            return Some(true);
        }
    }
    if lower.contains("/.git/") || lower.ends_with("/.git") {
        return None;
    }
    Some(false)
}

pub fn watch(app: &AppHandle, repo: &str, root: &Path, git_dir: &Path, common_dir: &Path) -> notify::Result<WatchHandle> {
    let app = app.clone();
    let repo_id = repo.to_string();
    let git_dirs = vec![git_dir.to_path_buf(), common_dir.to_path_buf(), root.join(".git")];
    let dirs_for_handler = git_dirs.clone();
    let mut debouncer = new_debouncer(Duration::from_millis(350), None, move |res: DebounceEventResult| {
        let Ok(events) = res else { return };
        let mut git = false;
        let mut worktree = false;
        for ev in events {
            for p in &ev.paths {
                match classify(p, &dirs_for_handler) {
                    Some(true) => git = true,
                    Some(false) => worktree = true,
                    None => {}
                }
            }
        }
        if git || worktree {
            let _ = app.emit(REPO_CHANGED_EVENT, RepoChangedPayload { repo: repo_id.clone(), kind: if git { "git".into() } else { "worktree".into() } });
        }
    })?;
    debouncer.watch(root, RecursiveMode::Recursive)?;
    // Linked worktrees keep their git dir (and refs in the common dir) outside the root.
    for d in [git_dir, common_dir] {
        if d.is_dir() && !d.starts_with(root) {
            let _ = debouncer.watch(d, RecursiveMode::Recursive);
        }
    }
    Ok(WatchHandle { _inner: Box::new(debouncer) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_paths() {
        let gd = vec![PathBuf::from("C:/r/.git")];
        assert_eq!(classify(Path::new("C:/r/src/a.rs"), &gd), Some(false));
        assert_eq!(classify(Path::new("C:\\r\\.git\\refs\\heads\\main"), &gd), Some(true));
        assert_eq!(classify(Path::new("C:/r/.git/HEAD"), &gd), Some(true));
        assert_eq!(classify(Path::new("C:/r/.git/index.lock"), &gd), None);
        assert_eq!(classify(Path::new("C:/r/.git/objects/ab/cd"), &gd), None);
    }
}
