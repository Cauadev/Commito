//! Honest undo: only operations performed by this app, that are provably reversible, and only
//! while the repository is still in the exact state the operation left it in.

use serde::Serialize;

use super::read;
use super::runner::RunOpts;
use crate::errors::{AppError, AppResult, ErrorCode};
use crate::state::Repo;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum UndoAction {
    /// Undo commit / amend: move the branch back, keeping the changes staged.
    ResetSoft { to: String, expect_head: String },
    /// Undo merge / cherry-pick / revert / pull / reset: move back, refusing to lose local changes.
    ResetKeep { to: String, expect_head: String },
    /// Undo checkout: switch back to the previous branch or commit.
    Checkout { branch: Option<String>, sha: String, expect_head: String },
    /// Undo branch creation.
    DeleteBranch { name: String, expect_sha: String },
    /// Undo branch deletion.
    RestoreBranch { name: String, sha: String },
    RenameBranch { from: String, to: String },
    DeleteTag { name: String, expect_target: String },
    RestoreTag { name: String, object: String },
    /// Undo stash push.
    StashPop { expect_hash: String },
    /// Undo stash drop.
    RestoreStash { hash: String, message: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoEntry {
    pub id: String,
    pub label: String,
    pub created_at: i64,
    pub warning: Option<String>,
    pub action: UndoAction,
}

impl UndoEntry {
    pub fn new(label: impl Into<String>, action: UndoAction) -> Self {
        Self { id: uuid::Uuid::new_v4().to_string(), label: label.into(), created_at: crate::settings::now_secs(), warning: None, action }
    }
    pub fn warn(mut self, w: impl Into<String>) -> Self {
        self.warning = Some(w.into());
        self
    }
}

async fn rev(repo: &Repo, r: &str) -> Option<String> {
    let o = repo.git.run(&repo.path, &["rev-parse", "--verify", "-q", r], RunOpts::read()).await.ok()?;
    o.ok().then(|| o.text().trim().to_string())
}

/// Checks whether the entry can still be applied safely. Returns a reason when it cannot.
pub async fn check(repo: &Repo, entry: &UndoEntry) -> Option<String> {
    let head = read::head_ref(repo).await;
    let head_oid = head.oid.clone().unwrap_or_default();
    let stale = || Some("O repositório mudou depois desta operação; desfazer não é mais seguro.".to_string());
    match &entry.action {
        UndoAction::ResetSoft { expect_head, .. } | UndoAction::ResetKeep { expect_head, .. } | UndoAction::Checkout { expect_head, .. } => {
            if &head_oid != expect_head {
                return stale();
            }
        }
        UndoAction::DeleteBranch { name, expect_sha } => {
            if rev(repo, &format!("refs/heads/{name}")).await.as_deref() != Some(expect_sha.as_str()) {
                return stale();
            }
            if head.branch.as_deref() == Some(name.as_str()) {
                return Some("A branch está em uso (checkout). Troque de branch antes de desfazer.".into());
            }
        }
        UndoAction::RestoreBranch { name, .. } => {
            if rev(repo, &format!("refs/heads/{name}")).await.is_some() {
                return Some("Já existe uma branch com esse nome.".into());
            }
        }
        UndoAction::RenameBranch { to, from } => {
            if rev(repo, &format!("refs/heads/{to}")).await.is_none() || rev(repo, &format!("refs/heads/{from}")).await.is_some() {
                return stale();
            }
        }
        UndoAction::DeleteTag { name, expect_target } => {
            if rev(repo, &format!("refs/tags/{name}")).await.as_deref() != Some(expect_target.as_str()) {
                return stale();
            }
        }
        UndoAction::RestoreTag { name, .. } => {
            if rev(repo, &format!("refs/tags/{name}")).await.is_some() {
                return Some("Já existe uma tag com esse nome.".into());
            }
        }
        UndoAction::StashPop { expect_hash } => {
            if rev(repo, "refs/stash").await.as_deref() != Some(expect_hash.as_str()) {
                return stale();
            }
        }
        UndoAction::RestoreStash { .. } => {}
    }
    None
}

pub async fn apply(repo: &Repo, entry: &UndoEntry) -> AppResult<String> {
    if let Some(reason) = check(repo, entry).await {
        return Err(AppError::new(ErrorCode::StaleState, reason));
    }
    let g = &repo.git;
    let p = &repo.path;
    let w = RunOpts::write;
    match &entry.action {
        UndoAction::ResetSoft { to, .. } => {
            g.run_ok(p, &["reset", "--soft", to], w()).await?;
        }
        UndoAction::ResetKeep { to, .. } => {
            g.run_ok(p, &["reset", "--keep", to], w()).await?;
        }
        UndoAction::Checkout { branch, sha, .. } => match branch {
            Some(b) => {
                g.run_ok(p, &["switch", b], w()).await?;
            }
            None => {
                g.run_ok(p, &["switch", "--detach", sha], w()).await?;
            }
        },
        UndoAction::DeleteBranch { name, .. } => {
            g.run_ok(p, &["branch", "-D", name], w()).await?;
        }
        UndoAction::RestoreBranch { name, sha } => {
            g.run_ok(p, &["branch", name, sha], w()).await?;
        }
        UndoAction::RenameBranch { from, to } => {
            g.run_ok(p, &["branch", "-m", to, from], w()).await?;
        }
        UndoAction::DeleteTag { name, .. } => {
            g.run_ok(p, &["tag", "-d", name], w()).await?;
        }
        UndoAction::RestoreTag { name, object } => {
            g.run_ok(p, &["tag", name, object], w()).await?;
        }
        UndoAction::StashPop { .. } => {
            let out = g.run(p, &["stash", "pop", "--index"], w()).await?;
            if !out.ok() {
                return Err(AppError::new(ErrorCode::Conflict, "Não foi possível reaplicar o stash sem conflitos; ele continua na lista.").details(out.stderr));
            }
        }
        UndoAction::RestoreStash { hash, message } => {
            g.run_ok(p, &["stash", "store", "-m", message, hash], w()).await?;
        }
    }
    Ok(format!("Desfeito: {}", entry.label))
}
