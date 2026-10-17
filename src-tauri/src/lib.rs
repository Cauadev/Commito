//! Commito — desktop Git client (Tauri 2 backend).

pub mod commands;
pub mod credentials;
pub mod errors;
pub mod events;
pub mod git;
pub mod providers;
pub mod redact;
pub mod settings;
pub mod state;
pub mod updater;
pub mod watcher;

#[cfg(test)]
mod tests;

use tauri::Manager;

use crate::commands::{accounts, ops, repo, system};
use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updater::PendingUpdate::default())
        .setup(|app| {
            // COMMITO_CONFIG_DIR isolates settings (tests / a second profile) from the user's normal config.
            let dir = std::env::var_os("COMMITO_CONFIG_DIR")
                .map(std::path::PathBuf::from)
                .or_else(|| app.path().app_config_dir().ok())
                .unwrap_or_else(|| std::env::temp_dir().join("commito"));
            app.manage(AppState::new(dir.join("settings.json")));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            system::app_info,
            system::set_git_path,
            system::get_settings,
            system::save_ui_settings,
            system::save_session,
            system::set_preferences,
            system::remove_recent,
            system::default_clone_dir,
            system::cancel_operation,
            system::open_in_explorer,
            system::open_repo_file,
            system::open_url,
            system::open_terminal,
            system::ssh_diagnostics,
            system::donation_available,
            system::donation_pix,
            updater::update_configured,
            updater::update_check,
            updater::update_install,
            #[cfg(debug_assertions)]
            updater::update_verify_only,
            repo::open_repository,
            repo::close_repository,
            repo::init_repository,
            repo::clone_repository,
            repo::get_status,
            repo::get_graph,
            repo::get_refs,
            repo::search_commits,
            repo::get_commit,
            repo::get_stash_untracked,
            repo::get_working_diff,
            repo::get_commit_diff,
            repo::compare_refs,
            repo::get_file_history,
            repo::get_blame,
            repo::rebase_plan,
            repo::read_worktree_file,
            repo::get_remotes,
            repo::get_worktrees,
            repo::get_stashes,
            repo::get_identity,
            repo::get_submodules,
            repo::get_repo_auth,
            repo::set_repo_auth,
            repo::list_undo,
            repo::undo_last,
            ops::stage_files,
            ops::unstage_files,
            ops::apply_lines,
            ops::discard_changes,
            ops::commit,
            ops::last_commit_message,
            ops::set_identity,
            ops::set_pull_strategy,
            ops::create_branch,
            ops::checkout,
            ops::rename_branch,
            ops::delete_branch,
            ops::set_upstream,
            ops::merge,
            ops::rebase,
            ops::rebase_interactive,
            ops::sequencer,
            ops::resolve_conflicts,
            ops::cherry_pick,
            ops::revert_commit,
            ops::reset,
            ops::create_tag,
            ops::delete_tag,
            ops::rename_tag,
            ops::stash_push,
            ops::stash_apply,
            ops::stash_drop,
            ops::stash_clear,
            ops::stash_branch,
            ops::worktree_add,
            ops::worktree_remove,
            ops::worktree_prune,
            ops::worktree_lock,
            ops::remote_add,
            ops::remote_set_url,
            ops::remote_rename,
            ops::remote_remove,
            ops::fetch,
            ops::pull,
            ops::push,
            ops::delete_remote_branch,
            ops::push_tag,
            ops::delete_remote_tag,
            ops::prune_remote,
            ops::test_remote,
            ops::submodule_update,
            accounts::list_accounts,
            accounts::add_account,
            accounts::test_account,
            accounts::remove_account,
            accounts::list_remote_repos,
            accounts::list_workspaces,
            accounts::token_help,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Commito");
}
