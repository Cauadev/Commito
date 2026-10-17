//! Auto-update (tauri-plugin-updater). The update feed URL is chosen at build time through
//! `COMMITO_UPDATE_URL` (e.g. https://github.com/<owner>/<repo>/releases/latest/download/latest.json).
//! Builds without it never contact any update server. Every package is verified against the
//! public key in `tauri.conf.json` before installing.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::errors::{AppError, AppResult, ErrorCode};

pub const UPDATE_PROGRESS_EVENT: &str = "fg://update-progress";

#[derive(Default)]
pub struct PendingUpdate(pub Mutex<Option<Update>>);

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    pub date: Option<String>,
    pub notes: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct UpdateProgress {
    downloaded: u64,
    total: Option<u64>,
    finished: bool,
}

pub fn update_url() -> Option<String> {
    option_env!("COMMITO_UPDATE_URL")
        .map(str::to_string)
        .or_else(|| std::env::var("COMMITO_UPDATE_URL").ok())
        // Plain http is only accepted for localhost in debug builds (end-to-end testing).
        .filter(|u| u.starts_with("https://") || (cfg!(debug_assertions) && u.starts_with("http://localhost")))
}

fn err(e: impl std::fmt::Display) -> AppError {
    AppError::new(ErrorCode::Network, "Não foi possível verificar ou baixar a atualização.").details(e.to_string())
}

#[tauri::command]
pub fn update_configured() -> bool {
    update_url().is_some()
}

#[tauri::command]
pub async fn update_check(app: AppHandle, pending: State<'_, PendingUpdate>) -> AppResult<Option<UpdateInfo>> {
    let Some(url) = update_url() else { return Ok(None) };
    let url = url::Url::parse(&url).map_err(err)?;
    // Debug builds accept http://localhost (warning only); release builds require https.
    let updater = app.updater_builder().endpoints(vec![url]).map_err(err)?.build().map_err(err)?;
    let update = updater.check().await.map_err(err)?;
    let info = update.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        current_version: u.current_version.clone(),
        date: u.date.map(|d| d.to_string()),
        notes: u.body.clone(),
    });
    *pending.0.lock().unwrap() = update;
    Ok(info)
}

/// Downloads, verifies the signature and installs the pending update, then restarts the app.
#[tauri::command]
pub async fn update_install(app: AppHandle, pending: State<'_, PendingUpdate>) -> AppResult<()> {
    let update = pending.0.lock().unwrap().take().ok_or_else(|| AppError::new(ErrorCode::StaleState, "Nenhuma atualização pendente. Verifique novamente."))?;
    let mut downloaded: u64 = 0;
    let app2 = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = app2.emit(UPDATE_PROGRESS_EVENT, UpdateProgress { downloaded, total, finished: false });
            },
            || {},
        )
        .await
        .map_err(err)?;
    let _ = app.emit(UPDATE_PROGRESS_EVENT, UpdateProgress { downloaded: 0, total: None, finished: true });
    app.restart();
}

/// Debug-only: downloads the pending update and verifies its signature without installing.
/// Used by end-to-end tests; not registered in release builds.
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn update_verify_only(pending: State<'_, PendingUpdate>) -> AppResult<usize> {
    let update = pending.0.lock().unwrap().take().ok_or_else(|| AppError::new(ErrorCode::StaleState, "Nenhuma atualização pendente."))?;
    let bytes = update.download(|_, _| {}, || {}).await.map_err(err)?;
    Ok(bytes.len())
}
