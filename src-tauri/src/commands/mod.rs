pub mod accounts;
pub mod ops;
pub mod repo;
pub mod system;

use std::future::Future;

use crate::errors::AppResult;
use crate::state::{AppState, Repo};

/// Serialises write operations per repository/worktree to avoid index/ref lock races.
pub async fn locked<T, F, Fut>(state: &AppState, repo: &Repo, f: F) -> AppResult<T>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = AppResult<T>>,
{
    let lock = state.lock(repo);
    let _guard = lock.lock().await;
    let res = f().await;
    state.invalidate_graph(repo).await;
    res
}
