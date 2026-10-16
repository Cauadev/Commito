//! Mutating commands. Every command takes the per-repository write lock and records an undo
//! entry only when the operation is provably reversible.

use tauri::{AppHandle, State};

use super::locked;
use super::repo::account_by_id;
use crate::errors::AppResult;
use crate::events::ProgressSink;
use crate::git::diff::HunkSelection;
use crate::git::ops::{self, CheckoutTarget, CommitRequest, ConflictResolution, LineAction, MergeMode, OperationResult, ResetMode, SeqAction, StashRequest, TodoItem, WorktreeRequest};
use crate::git::read;
use crate::git::remote::{self, PullStrategy, PushRequest};
use crate::git::undo::{UndoAction, UndoEntry};
use crate::settings::ProviderAccount;
use crate::state::{AppState, Repo};

fn push_undo(state: &AppState, r: &Repo, entry: UndoEntry) {
    let mut map = state.undo.lock().unwrap();
    let v = map.entry(r.key.clone()).or_default();
    v.push(entry);
    if v.len() > 30 {
        v.remove(0);
    }
}

async fn head_oid(r: &Repo) -> Option<String> {
    read::head_ref(r).await.oid
}

async fn ref_oid(r: &Repo, name: &str) -> Option<String> {
    let o = r.git.run(&r.path, &["rev-parse", "--verify", "-q", name], crate::git::runner::RunOpts::read()).await.ok()?;
    o.ok().then(|| o.text().trim().to_string())
}

/// Records a "move HEAD back" undo when an operation completed and changed HEAD.
fn record_head_move(state: &AppState, r: &Repo, res: &OperationResult, before: Option<String>, after: Option<String>, label: String, soft: bool) {
    if !res.ok || res.conflicts {
        return;
    }
    if let (Some(b), Some(a)) = (before, after) {
        if a != b {
            let action = if soft { UndoAction::ResetSoft { to: b, expect_head: a } } else { UndoAction::ResetKeep { to: b, expect_head: a } };
            push_undo(state, r, UndoEntry::new(label, action));
        }
    }
}

fn repo_account(state: &AppState, r: &Repo) -> Option<ProviderAccount> {
    let s = state.settings.lock().unwrap();
    let id = s.repo_auth.get(&r.key)?;
    s.accounts.iter().find(|a| &a.id == id).cloned()
}

// ------------------------------------------------------------------ index & commit

#[tauri::command]
pub async fn stage_files(state: State<'_, AppState>, repo: String, paths: Vec<String>) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    locked(&state, &r, || ops::stage(&r, &paths)).await
}

#[tauri::command]
pub async fn unstage_files(state: State<'_, AppState>, repo: String, paths: Vec<String>) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    locked(&state, &r, || ops::unstage(&r, &paths)).await
}

#[tauri::command]
pub async fn apply_lines(state: State<'_, AppState>, repo: String, path: String, action: LineAction, selections: Vec<HunkSelection>, context: Option<u32>) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    locked(&state, &r, || ops::apply_lines(&r, &path, action, &selections, context.unwrap_or(3))).await
}

#[tauri::command]
pub async fn discard_changes(state: State<'_, AppState>, repo: String, tracked: Vec<String>, untracked: Vec<String>) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    locked(&state, &r, || ops::discard(&r, &tracked, &untracked)).await
}

#[tauri::command]
pub async fn commit(state: State<'_, AppState>, repo: String, request: CommitRequest) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    let before = head_oid(&r).await;
    let res = locked(&state, &r, || ops::commit(&r, &request)).await?;
    let after = head_oid(&r).await;
    let label = if request.amend { "Amend do último commit".to_string() } else { format!("Commit \"{}\"", request.message.lines().next().unwrap_or("").chars().take(50).collect::<String>()) };
    record_head_move(&state, &r, &res, before, after, label, true);
    Ok(res)
}

#[tauri::command]
pub async fn last_commit_message(state: State<'_, AppState>, repo: String) -> AppResult<String> {
    let r = state.repo(&repo)?;
    ops::last_commit_message(&r).await
}

#[tauri::command]
pub async fn set_identity(state: State<'_, AppState>, repo: String, global: bool, name: String, email: String) -> AppResult<()> {
    let r = state.repo(&repo)?;
    locked(&state, &r, || ops::set_identity(&r, global, &name, &email)).await
}

#[tauri::command]
pub async fn set_pull_strategy(state: State<'_, AppState>, repo: String, strategy: String, global: bool) -> AppResult<()> {
    let r = state.repo(&repo)?;
    let scope = if global { "--global" } else { "--local" };
    let (rebase, ff): (Option<&str>, Option<&str>) = match strategy.as_str() {
        "merge" => (Some("false"), None),
        "rebase" => (Some("true"), None),
        "ff_only" => (None, Some("only")),
        _ => (None, None),
    };
    let w = crate::git::runner::RunOpts::write;
    locked(&state, &r, || async {
        let _ = r.git.run(&r.path, &["config", scope, "--unset", "pull.rebase"], w()).await;
        let _ = r.git.run(&r.path, &["config", scope, "--unset", "pull.ff"], w()).await;
        if let Some(v) = rebase {
            r.git.run_ok(&r.path, &["config", scope, "pull.rebase", v], w()).await?;
        }
        if let Some(v) = ff {
            r.git.run_ok(&r.path, &["config", scope, "pull.ff", v], w()).await?;
        }
        Ok(())
    })
    .await
}

// ------------------------------------------------------------------ branches

#[tauri::command]
pub async fn create_branch(state: State<'_, AppState>, repo: String, name: String, start: Option<String>, checkout: bool) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    let before_head = read::head_ref(&r).await;
    let res = locked(&state, &r, || ops::create_branch(&r, &name, start.as_deref(), checkout)).await?;
    if let Some(sha) = ref_oid(&r, &format!("refs/heads/{name}")).await {
        if checkout {
            if let Some(oid) = before_head.oid.clone() {
                push_undo(&state, &r, UndoEntry::new(format!("Criar e trocar para '{name}'"), UndoAction::Checkout { branch: before_head.branch.clone(), sha: oid, expect_head: sha }));
            }
        } else {
            push_undo(&state, &r, UndoEntry::new(format!("Criar branch '{name}'"), UndoAction::DeleteBranch { name: name.clone(), expect_sha: sha }));
        }
    }
    Ok(res)
}

#[tauri::command]
pub async fn checkout(state: State<'_, AppState>, repo: String, target: CheckoutTarget, auto_stash: Option<bool>) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    let before = read::head_ref(&r).await;
    let res = locked(&state, &r, || ops::checkout(&r, &target, auto_stash.unwrap_or(false))).await?;
    if let (Some(prev), Some(now)) = (before.oid.clone(), head_oid(&r).await) {
        if !res.conflicts && !res.message.contains("ficaram no stash") {
            let label = match &before.branch {
                Some(b) => format!("Checkout (voltar para '{b}')"),
                None => "Checkout (voltar ao HEAD anterior)".into(),
            };
            push_undo(&state, &r, UndoEntry::new(label, UndoAction::Checkout { branch: before.branch.clone(), sha: prev, expect_head: now }));
        }
    }
    Ok(res)
}

#[tauri::command]
pub async fn rename_branch(state: State<'_, AppState>, repo: String, old: String, new: String) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    let res = locked(&state, &r, || ops::rename_branch(&r, &old, &new)).await?;
    push_undo(&state, &r, UndoEntry::new(format!("Renomear '{old}' → '{new}'"), UndoAction::RenameBranch { from: old, to: new }));
    Ok(res)
}

#[tauri::command]
pub async fn delete_branch(state: State<'_, AppState>, repo: String, name: String, force: bool) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    let sha = ref_oid(&r, &format!("refs/heads/{name}")).await;
    let res = locked(&state, &r, || ops::delete_branch(&r, &name, force)).await?;
    if let Some(sha) = sha {
        push_undo(&state, &r, UndoEntry::new(format!("Excluir branch '{name}'"), UndoAction::RestoreBranch { name, sha }));
    }
    Ok(res)
}

#[tauri::command]
pub async fn set_upstream(state: State<'_, AppState>, repo: String, branch: String, upstream: Option<String>) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    locked(&state, &r, || ops::set_upstream(&r, &branch, upstream.as_deref())).await
}

#[tauri::command]
pub async fn merge(state: State<'_, AppState>, repo: String, target: String, mode: MergeMode) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    let before = head_oid(&r).await;
    let res = locked(&state, &r, || ops::merge(&r, &target, mode)).await?;
    record_head_move(&state, &r, &res, before, head_oid(&r).await, format!("Merge de '{target}'"), false);
    Ok(res)
}

#[tauri::command]
pub async fn rebase(state: State<'_, AppState>, repo: String, onto: String) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    let before = head_oid(&r).await;
    let res = locked(&state, &r, || ops::rebase(&r, &onto)).await?;
    record_head_move(&state, &r, &res, before, head_oid(&r).await, format!("Rebase sobre '{onto}'"), false);
    Ok(res)
}

#[tauri::command]
pub async fn rebase_interactive(state: State<'_, AppState>, repo: String, base: String, todo: Vec<TodoItem>) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    let before = head_oid(&r).await;
    let res = locked(&state, &r, || ops::rebase_interactive(&r, &base, &todo)).await?;
    record_head_move(&state, &r, &res, before, head_oid(&r).await, "Rebase interativo".into(), false);
    Ok(res)
}

#[tauri::command]
pub async fn sequencer(state: State<'_, AppState>, repo: String, action: SeqAction) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    locked(&state, &r, || ops::sequencer(&r, action)).await
}

#[tauri::command]
pub async fn resolve_conflicts(state: State<'_, AppState>, repo: String, paths: Vec<String>, resolution: ConflictResolution) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    locked(&state, &r, || ops::resolve_conflicts(&r, &paths, resolution)).await
}

#[tauri::command]
pub async fn cherry_pick(state: State<'_, AppState>, repo: String, shas: Vec<String>, no_commit: Option<bool>) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    let before = head_oid(&r).await;
    let res = locked(&state, &r, || ops::cherry_pick(&r, &shas, no_commit.unwrap_or(false))).await?;
    record_head_move(&state, &r, &res, before, head_oid(&r).await, format!("Cherry-pick de {} commit(s)", shas.len()), false);
    Ok(res)
}

#[tauri::command]
pub async fn revert_commit(state: State<'_, AppState>, repo: String, sha: String, no_commit: Option<bool>) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    let before = head_oid(&r).await;
    let res = locked(&state, &r, || ops::revert(&r, &sha, no_commit.unwrap_or(false))).await?;
    record_head_move(&state, &r, &res, before, head_oid(&r).await, format!("Revert de {}", &sha[..sha.len().min(8)]), false);
    Ok(res)
}

#[tauri::command]
pub async fn reset(state: State<'_, AppState>, repo: String, rev: String, mode: ResetMode, confirmed: Option<bool>) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    let before = head_oid(&r).await;
    let res = locked(&state, &r, || ops::reset(&r, &rev, mode, confirmed.unwrap_or(false))).await?;
    let after = head_oid(&r).await;
    if let (Some(b), Some(a)) = (before, after) {
        if a != b {
            let mut e = UndoEntry::new(format!("Reset para {}", &rev[..rev.len().min(8)]), UndoAction::ResetKeep { to: b, expect_head: a });
            if mode == ResetMode::Hard {
                e = e.warn("Restaura apenas a posição da branch. Alterações não commitadas descartadas pelo reset hard não podem ser recuperadas.");
            }
            push_undo(&state, &r, e);
        }
    }
    Ok(res)
}

// ------------------------------------------------------------------ tags

#[tauri::command]
pub async fn create_tag(state: State<'_, AppState>, repo: String, name: String, target: String, message: Option<String>) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    let res = locked(&state, &r, || ops::create_tag(&r, &name, &target, message.as_deref())).await?;
    if let Some(obj) = ref_oid(&r, &format!("refs/tags/{name}")).await {
        push_undo(&state, &r, UndoEntry::new(format!("Criar tag '{name}'"), UndoAction::DeleteTag { name, expect_target: obj }));
    }
    Ok(res)
}

#[tauri::command]
pub async fn delete_tag(state: State<'_, AppState>, repo: String, name: String) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    let obj = ref_oid(&r, &format!("refs/tags/{name}")).await;
    let res = locked(&state, &r, || ops::delete_tag(&r, &name)).await?;
    if let Some(object) = obj {
        push_undo(&state, &r, UndoEntry::new(format!("Excluir tag '{name}'"), UndoAction::RestoreTag { name, object }));
    }
    Ok(res)
}

#[tauri::command]
pub async fn rename_tag(state: State<'_, AppState>, repo: String, old: String, new: String) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    locked(&state, &r, || ops::rename_tag(&r, &old, &new)).await
}

// ------------------------------------------------------------------ stash

#[tauri::command]
pub async fn stash_push(state: State<'_, AppState>, repo: String, request: StashRequest) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    let res = locked(&state, &r, || ops::stash_push(&r, &request)).await?;
    if let Some(h) = ref_oid(&r, "refs/stash").await {
        push_undo(&state, &r, UndoEntry::new("Stash das alterações", UndoAction::StashPop { expect_hash: h }));
    }
    Ok(res)
}

#[tauri::command]
pub async fn stash_apply(state: State<'_, AppState>, repo: String, index: u32, hash: String, pop: bool, restore_index: Option<bool>) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    locked(&state, &r, || ops::stash_apply(&r, index, &hash, pop, restore_index.unwrap_or(false))).await
}

#[tauri::command]
pub async fn stash_drop(state: State<'_, AppState>, repo: String, index: u32, hash: String) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    let msg = read::list_stashes(&r).await?.into_iter().find(|s| s.hash == hash).map(|s| s.message).unwrap_or_default();
    let res = locked(&state, &r, || ops::stash_drop(&r, index, &hash)).await?;
    push_undo(&state, &r, UndoEntry::new("Descartar stash", UndoAction::RestoreStash { hash, message: msg }));
    Ok(res)
}

#[tauri::command]
pub async fn stash_clear(state: State<'_, AppState>, repo: String) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    locked(&state, &r, || ops::stash_clear(&r)).await
}

#[tauri::command]
pub async fn stash_branch(state: State<'_, AppState>, repo: String, index: u32, hash: String, name: String) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    locked(&state, &r, || ops::stash_branch(&r, index, &hash, &name)).await
}

// ------------------------------------------------------------------ worktrees

#[tauri::command]
pub async fn worktree_add(state: State<'_, AppState>, repo: String, request: WorktreeRequest) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    locked(&state, &r, || ops::worktree_add(&r, &request)).await
}

#[tauri::command]
pub async fn worktree_remove(state: State<'_, AppState>, repo: String, path: String, force: bool) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    if crate::settings::same_path(&path, &r.display) {
        return Err(crate::errors::AppError::invalid("Não é possível remover a worktree aberta nesta aba."));
    }
    locked(&state, &r, || ops::worktree_remove(&r, &path, force)).await
}

#[tauri::command]
pub async fn worktree_prune(state: State<'_, AppState>, repo: String) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    locked(&state, &r, || ops::worktree_prune(&r)).await
}

#[tauri::command]
pub async fn worktree_lock(state: State<'_, AppState>, repo: String, path: String, lock: bool, reason: Option<String>) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    locked(&state, &r, || ops::worktree_lock(&r, &path, lock, reason.as_deref())).await
}

// ------------------------------------------------------------------ remotes

#[tauri::command]
pub async fn remote_add(state: State<'_, AppState>, repo: String, name: String, url: String) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    locked(&state, &r, || ops::remote_add(&r, &name, &url)).await
}

#[tauri::command]
pub async fn remote_set_url(state: State<'_, AppState>, repo: String, name: String, url: String, push: bool) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    locked(&state, &r, || ops::remote_set_url(&r, &name, &url, push)).await
}

#[tauri::command]
pub async fn remote_rename(state: State<'_, AppState>, repo: String, old: String, new: String) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    locked(&state, &r, || ops::remote_rename(&r, &old, &new)).await
}

#[tauri::command]
pub async fn remote_remove(state: State<'_, AppState>, repo: String, name: String) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    locked(&state, &r, || ops::remote_remove(&r, &name)).await
}

#[tauri::command]
pub async fn fetch(app: AppHandle, state: State<'_, AppState>, repo: String, remote: Option<String>, prune: Option<bool>, op_id: Option<String>) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    let acc = repo_account(&state, &r);
    let p = ProgressSink::new(&app, op_id, Some(repo.clone()));
    // Fetch only touches remote-tracking refs: it does not need the worktree write lock.
    let res = remote::fetch(&r, remote.as_deref(), prune.unwrap_or(false), false, p, acc.as_ref()).await;
    state.invalidate_graph(&r).await;
    res
}

#[tauri::command]
pub async fn pull(app: AppHandle, state: State<'_, AppState>, repo: String, strategy: PullStrategy, op_id: Option<String>) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    let acc = repo_account(&state, &r);
    let p = ProgressSink::new(&app, op_id, Some(repo.clone()));
    let before = head_oid(&r).await;
    let res = locked(&state, &r, || remote::pull(&r, strategy, p, acc.as_ref())).await?;
    record_head_move(&state, &r, &res, before, head_oid(&r).await, "Pull".into(), false);
    Ok(res)
}

#[tauri::command]
pub async fn push(app: AppHandle, state: State<'_, AppState>, repo: String, request: PushRequest, op_id: Option<String>) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    let acc = repo_account(&state, &r);
    let p = ProgressSink::new(&app, op_id, Some(repo.clone()));
    locked(&state, &r, || remote::push(&r, &request, p, acc.as_ref())).await
}

#[tauri::command]
pub async fn delete_remote_branch(app: AppHandle, state: State<'_, AppState>, repo: String, remote: String, branch: String, op_id: Option<String>) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    let acc = repo_account(&state, &r);
    let p = ProgressSink::new(&app, op_id, Some(repo.clone()));
    locked(&state, &r, || remote::delete_remote_branch(&r, &remote, &branch, p, acc.as_ref())).await
}

#[tauri::command]
pub async fn push_tag(app: AppHandle, state: State<'_, AppState>, repo: String, remote: String, tag: Option<String>, op_id: Option<String>) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    let acc = repo_account(&state, &r);
    let p = ProgressSink::new(&app, op_id, Some(repo.clone()));
    locked(&state, &r, || remote::push_tag(&r, &remote, tag.as_deref(), p, acc.as_ref())).await
}

#[tauri::command]
pub async fn delete_remote_tag(app: AppHandle, state: State<'_, AppState>, repo: String, remote: String, tag: String, op_id: Option<String>) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    let acc = repo_account(&state, &r);
    let p = ProgressSink::new(&app, op_id, Some(repo.clone()));
    locked(&state, &r, || remote::delete_remote_tag(&r, &remote, &tag, p, acc.as_ref())).await
}

#[tauri::command]
pub async fn prune_remote(app: AppHandle, state: State<'_, AppState>, repo: String, remote: String, op_id: Option<String>) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    let acc = repo_account(&state, &r);
    let p = ProgressSink::new(&app, op_id, Some(repo.clone()));
    locked(&state, &r, || remote::prune_remote(&r, &remote, p, acc.as_ref())).await
}

#[tauri::command]
pub async fn test_remote(state: State<'_, AppState>, repo: String, remote: String, account_id: Option<String>) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    let acc = match account_id {
        Some(id) => account_by_id(&state, Some(&id))?,
        None => repo_account(&state, &r),
    };
    remote::test_remote(&r, &remote, acc.as_ref()).await
}

#[tauri::command]
pub async fn submodule_update(app: AppHandle, state: State<'_, AppState>, repo: String, op_id: Option<String>) -> AppResult<OperationResult> {
    let r = state.repo(&repo)?;
    let p = ProgressSink::new(&app, op_id, Some(repo.clone()));
    locked(&state, &r, || remote::submodule_update(&r, p)).await
}
