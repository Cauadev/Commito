//! Repository lifecycle and read-only queries.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::errors::{AppError, AppResult};
use crate::events::ProgressSink;
use crate::git::diff::FileDiff;
use crate::git::log::{CommitDetail, CommitInfo};
use crate::git::read::{self, Blame, DiffSource, GitStatus, GraphPayload, Identity, Remote, ResolvedRepo, Submodule};
use crate::git::refs::{GitRef, StashEntry, Worktree};
use crate::git::remote::{self, CloneRequest};
use crate::git::status::FileChange;
use crate::git::undo;
use crate::settings::ProviderAccount;
use crate::state::AppState;

async fn open_resolved(app: &AppHandle, state: &AppState, resolved: &ResolvedRepo) {
    state.authorize(&resolved.path);
    {
        let mut s = state.settings.lock().unwrap();
        s.touch_recent(&resolved.path, &resolved.name);
    }
    let _ = state.save_settings();
    let key = crate::state::norm_key(&resolved.path);
    let has_watch = state.watchers.lock().unwrap().contains_key(&key);
    if !has_watch && !resolved.bare {
        // Other worktrees nested inside this one (e.g. `.worktrees/*`) must not refresh this tab.
        let nested: Vec<std::path::PathBuf> = match state.git() {
            Ok(git) => git
                .read(Path::new(&resolved.path), &["worktree", "list", "--porcelain"])
                .await
                .map(|raw| crate::git::refs::parse_worktrees(&raw).into_iter().map(|w| std::path::PathBuf::from(crate::state::native_path(&w.path))).collect())
                .unwrap_or_default(),
            Err(_) => vec![],
        };
        match crate::watcher::watch(app, &resolved.path, Path::new(&resolved.path), Path::new(&resolved.git_dir), Path::new(&resolved.common_dir), &nested) {
            Ok(h) => {
                state.watchers.lock().unwrap().insert(key, h);
            }
            Err(e) => eprintln!("watcher failed for {}: {e}", resolved.path),
        }
    }
}

#[tauri::command]
pub async fn open_repository(app: AppHandle, state: State<'_, AppState>, path: String) -> AppResult<ResolvedRepo> {
    let git = state.git()?;
    let resolved = read::resolve(&git, Path::new(&path)).await?;
    open_resolved(&app, &state, &resolved).await;
    Ok(resolved)
}

#[tauri::command]
pub async fn close_repository(state: State<'_, AppState>, path: String) -> AppResult<()> {
    let key = crate::state::norm_key(&path);
    state.watchers.lock().unwrap().remove(&key);
    let prefix = format!("{key}|");
    state.graph_cache.lock().await.retain(|k, _| !k.starts_with(&prefix));
    Ok(())
}

#[tauri::command]
pub async fn init_repository(app: AppHandle, state: State<'_, AppState>, path: String, initial_branch: Option<String>) -> AppResult<ResolvedRepo> {
    let git = state.git()?;
    let p = PathBuf::from(&path);
    if !p.is_absolute() {
        return Err(AppError::invalid("Escolha uma pasta."));
    }
    if p.join(".git").exists() {
        return Err(AppError::invalid("Esta pasta já contém um repositório Git.").recovery("Use \"Abrir repositório\"."));
    }
    crate::git::ops::init(&git, &p, initial_branch.as_deref()).await?;
    let resolved = read::resolve(&git, &p).await?;
    open_resolved(&app, &state, &resolved).await;
    Ok(resolved)
}

pub fn account_by_id(state: &AppState, id: Option<&str>) -> AppResult<Option<ProviderAccount>> {
    match id.filter(|i| !i.is_empty()) {
        None => Ok(None),
        Some(id) => state
            .settings
            .lock()
            .unwrap()
            .accounts
            .iter()
            .find(|a| a.id == id)
            .cloned()
            .map(Some)
            .ok_or_else(|| AppError::invalid("Conta não encontrada.")),
    }
}

#[tauri::command]
pub async fn clone_repository(
    app: AppHandle,
    state: State<'_, AppState>,
    request: CloneRequest,
    op_id: Option<String>,
    account_id: Option<String>,
) -> AppResult<ResolvedRepo> {
    let git = state.git()?;
    let account = account_by_id(&state, account_id.as_deref())?;
    let progress = ProgressSink::new(&app, op_id, None);
    let dest = remote::clone(&git, &request, progress, account.as_ref()).await?;
    let resolved = read::resolve(&git, Path::new(&dest)).await?;
    if let Some(acc) = account.as_ref().filter(|a| remote::account_for_url(Some(a), &request.url).is_some()) {
        state.settings.lock().unwrap().repo_auth.insert(crate::state::norm_key(&resolved.path), acc.id.clone());
    }
    open_resolved(&app, &state, &resolved).await;
    Ok(resolved)
}

#[tauri::command]
pub async fn get_status(state: State<'_, AppState>, repo: String, include_ignored: Option<bool>) -> AppResult<GitStatus> {
    let r = state.repo(&repo)?;
    read::status(&r, include_ignored.unwrap_or(false)).await
}

#[tauri::command]
pub async fn get_graph(
    state: State<'_, AppState>,
    repo: String,
    scope: String,
    limit: usize,
    include_wip: bool,
    reset: Option<bool>,
) -> AppResult<GraphPayload> {
    let r = state.repo(&repo)?;
    read::graph(&r, &state.graph_cache, &scope, limit, include_wip, reset.unwrap_or(false)).await
}

#[tauri::command]
pub async fn get_refs(state: State<'_, AppState>, repo: String) -> AppResult<Vec<GitRef>> {
    let r = state.repo(&repo)?;
    read::list_refs(&r).await
}

#[tauri::command]
pub async fn search_commits(state: State<'_, AppState>, repo: String, query: String, scope: String) -> AppResult<Vec<String>> {
    let r = state.repo(&repo)?;
    read::search_commits(&r, &query, &scope, 500).await
}

#[tauri::command]
pub async fn get_commit(state: State<'_, AppState>, repo: String, sha: String) -> AppResult<CommitDetail> {
    let r = state.repo(&repo)?;
    read::commit_detail(&r, &sha).await
}

#[tauri::command]
pub async fn get_stash_untracked(state: State<'_, AppState>, repo: String, hash: String) -> AppResult<Vec<FileChange>> {
    let r = state.repo(&repo)?;
    read::stash_untracked_files(&r, &hash).await
}

#[tauri::command]
pub async fn get_working_diff(state: State<'_, AppState>, repo: String, path: String, source: DiffSource, context: Option<u32>) -> AppResult<FileDiff> {
    let r = state.repo(&repo)?;
    read::working_diff(&r, &path, source, context.unwrap_or(3)).await
}

#[tauri::command]
pub async fn get_commit_diff(state: State<'_, AppState>, repo: String, base: Option<String>, sha: String, path: String, context: Option<u32>) -> AppResult<FileDiff> {
    let r = state.repo(&repo)?;
    read::commit_file_diff(&r, base.as_deref(), &sha, &path, context.unwrap_or(3)).await
}

#[tauri::command]
pub async fn compare_refs(state: State<'_, AppState>, repo: String, base: String, target: String) -> AppResult<Vec<FileChange>> {
    let r = state.repo(&repo)?;
    read::validate_rev(&base)?;
    read::validate_rev(&target)?;
    read::commit_files(&r, &target, Some(&base)).await
}

#[tauri::command]
pub async fn get_file_history(state: State<'_, AppState>, repo: String, path: String) -> AppResult<Vec<HistoryEntry>> {
    let r = state.repo(&repo)?;
    Ok(read::file_history(&r, &path, 500).await?.into_iter().map(HistoryEntry::from).collect())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub hash: String,
    pub parents: Vec<String>,
    pub subject: String,
    pub author_name: String,
    pub author_email: String,
    pub author_time: i64,
}

impl From<CommitInfo> for HistoryEntry {
    fn from(c: CommitInfo) -> Self {
        Self { hash: c.hash, parents: c.parents, subject: c.subject, author_name: c.author_name, author_email: c.author_email, author_time: c.author_time }
    }
}

/// Reads a working tree file as text (used to display conflict markers). Limited to 4 MB.
#[tauri::command]
pub async fn read_worktree_file(state: State<'_, AppState>, repo: String, path: String) -> AppResult<String> {
    let r = state.repo(&repo)?;
    read::validate_path(&path)?;
    let full = r.path.join(&path);
    let meta = std::fs::metadata(&full)?;
    if meta.len() > 4 * 1024 * 1024 {
        return Err(AppError::invalid("Arquivo grande demais para exibir (> 4 MB)."));
    }
    let bytes = std::fs::read(&full)?;
    if bytes.iter().take(8000).any(|b| *b == 0) {
        return Err(AppError::invalid("Arquivo binário."));
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Commits that `git rebase -i <base>` would pick (oldest first, merges excluded).
#[tauri::command]
pub async fn rebase_plan(state: State<'_, AppState>, repo: String, base: String) -> AppResult<Vec<HistoryEntry>> {
    let r = state.repo(&repo)?;
    read::validate_rev(&base)?;
    let range = format!("{base}..HEAD");
    let out = r
        .git
        .run_ok(&r.path, &["log", "--reverse", "--no-merges", "-z", crate::git::log::LOG_FORMAT, "--max-count=1000", &range, "--"], crate::git::runner::RunOpts::read())
        .await?;
    Ok(crate::git::log::parse_log(&out.stdout).into_iter().map(HistoryEntry::from).collect())
}

#[tauri::command]
pub async fn get_blame(state: State<'_, AppState>, repo: String, path: String, rev: Option<String>) -> AppResult<Blame> {
    let r = state.repo(&repo)?;
    read::blame(&r, &path, rev.as_deref()).await
}

#[tauri::command]
pub async fn get_remotes(state: State<'_, AppState>, repo: String) -> AppResult<Vec<Remote>> {
    let r = state.repo(&repo)?;
    read::list_remotes(&r).await
}

#[tauri::command]
pub async fn get_worktrees(state: State<'_, AppState>, repo: String) -> AppResult<Vec<Worktree>> {
    let r = state.repo(&repo)?;
    read::list_worktrees(&r).await
}

#[tauri::command]
pub async fn get_stashes(state: State<'_, AppState>, repo: String) -> AppResult<Vec<StashEntry>> {
    let r = state.repo(&repo)?;
    read::list_stashes(&r).await
}

#[tauri::command]
pub async fn get_identity(state: State<'_, AppState>, repo: String) -> AppResult<Identity> {
    let r = state.repo(&repo)?;
    Ok(read::identity(&r).await)
}

#[tauri::command]
pub async fn get_submodules(state: State<'_, AppState>, repo: String) -> AppResult<Vec<Submodule>> {
    let r = state.repo(&repo)?;
    read::submodules(&r).await
}

#[tauri::command]
pub fn get_repo_auth(state: State<'_, AppState>, repo: String) -> AppResult<Option<String>> {
    let r = state.repo(&repo)?;
    Ok(state.settings.lock().unwrap().repo_auth.get(&r.key).cloned())
}

#[tauri::command]
pub fn set_repo_auth(state: State<'_, AppState>, repo: String, account_id: Option<String>) -> AppResult<()> {
    let r = state.repo(&repo)?;
    {
        let mut s = state.settings.lock().unwrap();
        match account_id.filter(|a| !a.is_empty()) {
            Some(id) => {
                if !s.accounts.iter().any(|a| a.id == id) {
                    return Err(AppError::invalid("Conta não encontrada."));
                }
                s.repo_auth.insert(r.key.clone(), id);
            }
            None => {
                s.repo_auth.remove(&r.key);
            }
        }
    }
    state.save_settings()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoView {
    pub id: String,
    pub label: String,
    pub created_at: i64,
    pub warning: Option<String>,
    pub available: bool,
    pub reason: Option<String>,
}

#[tauri::command]
pub async fn list_undo(state: State<'_, AppState>, repo: String) -> AppResult<Vec<UndoView>> {
    let r = state.repo(&repo)?;
    let entries = state.undo.lock().unwrap().get(&r.key).cloned().unwrap_or_default();
    let mut out = Vec::new();
    for (i, e) in entries.iter().rev().enumerate() {
        // Only the most recent entry can be undone (stack semantics).
        let reason = if i == 0 { undo::check(&r, e).await } else { Some("Desfaça primeiro as operações mais recentes.".into()) };
        out.push(UndoView { id: e.id.clone(), label: e.label.clone(), created_at: e.created_at, warning: e.warning.clone(), available: reason.is_none(), reason });
    }
    Ok(out)
}

#[tauri::command]
pub async fn undo_last(state: State<'_, AppState>, repo: String, id: String) -> AppResult<String> {
    let r = state.repo(&repo)?;
    let entry = {
        let map = state.undo.lock().unwrap();
        map.get(&r.key).and_then(|v| v.last().cloned())
    };
    let Some(entry) = entry.filter(|e| e.id == id) else {
        return Err(AppError::new(crate::errors::ErrorCode::StaleState, "Esta ação não é mais a última desfazível."));
    };
    let msg = super::locked(&state, &r, || undo::apply(&r, &entry)).await?;
    if let Some(v) = state.undo.lock().unwrap().get_mut(&r.key) {
        v.pop();
    }
    Ok(msg)
}
