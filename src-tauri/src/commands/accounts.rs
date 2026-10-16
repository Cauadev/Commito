//! Connected provider accounts (GitHub, GitLab, Bitbucket Cloud).

use serde::Serialize;
use tauri::State;

use crate::credentials;
use crate::errors::{AppError, AppResult};
use crate::providers::{self, AccountProfile, RepoPage, Workspace};
use crate::settings::{now_secs, Provider, ProviderAccount};
use crate::state::AppState;

fn find(state: &AppState, id: &str) -> AppResult<ProviderAccount> {
    state.settings.lock().unwrap().accounts.iter().find(|a| a.id == id).cloned().ok_or_else(|| AppError::invalid("Conta não encontrada."))
}

#[tauri::command]
pub fn list_accounts(state: State<'_, AppState>) -> Vec<ProviderAccount> {
    state.settings.lock().unwrap().accounts.clone()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddAccountResult {
    pub account: ProviderAccount,
    pub warnings: Vec<String>,
}

/// Validates the token against the provider API, then stores it in the OS vault.
/// The token is never persisted in settings nor returned to the WebView.
#[tauri::command]
pub async fn add_account(
    state: State<'_, AppState>,
    provider: Provider,
    token: String,
    base_url: Option<String>,
    email: Option<String>,
    label: Option<String>,
) -> AppResult<AddAccountResult> {
    let token = token.trim().to_string();
    if token.is_empty() {
        return Err(AppError::invalid("Informe o token."));
    }
    let base = providers::normalize_base_url(provider, base_url.as_deref())?;
    let email = email.map(|e| e.trim().to_string()).filter(|e| !e.is_empty());
    let profile: AccountProfile = providers::validate(&state.http, provider, base.as_deref(), email.as_deref(), &token).await?;
    let id = uuid::Uuid::new_v4().to_string();
    credentials::store_token(&id, &token)?;
    let account = ProviderAccount {
        id,
        provider,
        base_url: base,
        username: profile.username.clone(),
        display_name: profile.display_name.clone(),
        avatar_url: profile.avatar_url.clone(),
        email,
        label: label.filter(|l| !l.trim().is_empty()),
        created_at: now_secs(),
        scopes: profile.scopes.clone(),
        expires_at: profile.expires_at.clone(),
    };
    {
        let mut s = state.settings.lock().unwrap();
        s.accounts.push(account.clone());
    }
    if let Err(e) = state.save_settings() {
        let _ = credentials::delete_token(&account.id);
        return Err(e);
    }
    Ok(AddAccountResult { account, warnings: profile.warnings })
}

#[tauri::command]
pub async fn test_account(state: State<'_, AppState>, id: String) -> AppResult<AccountProfile> {
    let acc = find(&state, &id)?;
    let token = credentials::read_token(&id)?;
    let profile = providers::validate(&state.http, acc.provider, acc.base_url.as_deref(), acc.email.as_deref(), &token).await?;
    {
        let mut s = state.settings.lock().unwrap();
        if let Some(a) = s.accounts.iter_mut().find(|a| a.id == id) {
            a.display_name = profile.display_name.clone();
            a.avatar_url = profile.avatar_url.clone();
            a.scopes = profile.scopes.clone();
            a.expires_at = profile.expires_at.clone();
        }
    }
    state.save_settings()?;
    Ok(profile)
}

#[tauri::command]
pub fn remove_account(state: State<'_, AppState>, id: String) -> AppResult<()> {
    credentials::delete_token(&id)?;
    {
        let mut s = state.settings.lock().unwrap();
        s.accounts.retain(|a| a.id != id);
        s.repo_auth.retain(|_, v| *v != id);
    }
    state.save_settings()
}

#[tauri::command]
pub async fn list_remote_repos(state: State<'_, AppState>, id: String, page: u32, query: Option<String>, workspace: Option<String>) -> AppResult<RepoPage> {
    let acc = find(&state, &id)?;
    let token = credentials::read_token(&id)?;
    providers::list_repos(&state.http, &acc, &token, page, query.as_deref(), workspace.as_deref()).await
}

#[tauri::command]
pub async fn list_workspaces(state: State<'_, AppState>, id: String) -> AppResult<Vec<Workspace>> {
    let acc = find(&state, &id)?;
    let token = credentials::read_token(&id)?;
    providers::list_workspaces(&state.http, &acc, &token).await
}

#[tauri::command]
pub fn token_help(provider: Provider) -> String {
    providers::token_help(provider).to_string()
}
