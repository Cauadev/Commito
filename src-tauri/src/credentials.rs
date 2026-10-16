//! OS credential vault (Windows Credential Manager) and the Git askpass bridge.
//!
//! Tokens are written/read only here. They never reach the WebView, settings, logs,
//! command lines or remote URLs. For Git over HTTPS with a connected account, git is
//! pointed at this very executable as `GIT_ASKPASS`; the helper process reads the token
//! from the vault and prints it on stdout to git.

use std::ffi::OsString;

use crate::errors::{AppError, AppResult, ErrorCode};
use crate::git::runner::RunOpts;
use crate::settings::{Provider, ProviderAccount};

const SERVICE: &str = "ForgeGraph";
pub const ASKPASS_ACCOUNT_ENV: &str = "FORGEGRAPH_ASKPASS_ACCOUNT";
pub const ASKPASS_USER_ENV: &str = "FORGEGRAPH_ASKPASS_USER";

fn entry(account_id: &str) -> AppResult<keyring::Entry> {
    keyring::Entry::new(SERVICE, &format!("account:{account_id}")).map_err(vault_err)
}

fn vault_err(e: keyring::Error) -> AppError {
    AppError::new(ErrorCode::Credential, "Falha ao acessar o cofre de credenciais do sistema.").details(e.to_string())
}

pub fn store_token(account_id: &str, token: &str) -> AppResult<()> {
    entry(account_id)?.set_password(token).map_err(vault_err)
}

pub fn read_token(account_id: &str) -> AppResult<String> {
    match entry(account_id)?.get_password() {
        Ok(t) => Ok(t),
        Err(keyring::Error::NoEntry) => Err(AppError::new(ErrorCode::Credential, "Token da conta não encontrado no cofre do sistema.")
            .recovery("Reconecte a conta em Contas conectadas.")),
        Err(e) => Err(vault_err(e)),
    }
}

/// Deletes only this app's entry for the account; other apps' credentials are untouched.
pub fn delete_token(account_id: &str) -> AppResult<()> {
    match entry(account_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(vault_err(e)),
    }
}

/// Username git should send over HTTPS together with the account token.
pub fn git_username(account: &ProviderAccount) -> String {
    match account.provider {
        Provider::Github => "x-access-token".to_string(),
        Provider::Gitlab => "oauth2".to_string(),
        Provider::Bitbucket => "x-bitbucket-api-token-auth".to_string(),
    }
}

/// Extra git `-c` arguments + env so that HTTPS auth for this invocation uses the account token
/// through the askpass bridge instead of the credential helper chain.
pub fn askpass_args() -> Vec<String> {
    vec!["-c".into(), "credential.helper=".into(), "-c".into(), "credential.interactive=never".into()]
}

pub fn with_askpass(mut opts: RunOpts, account: &ProviderAccount) -> AppResult<RunOpts> {
    let exe = std::env::current_exe()?;
    opts.env.push(("GIT_ASKPASS".into(), OsString::from(exe)));
    opts.env.push((ASKPASS_ACCOUNT_ENV.into(), OsString::from(&account.id)));
    opts.env.push((ASKPASS_USER_ENV.into(), OsString::from(git_username(account))));
    opts.env.push(("GIT_TERMINAL_PROMPT".into(), OsString::from("0")));
    Ok(opts)
}

/// Entry point used when this executable is launched by git as an askpass helper.
/// Returns Some(exit_code) when running in helper mode.
pub fn maybe_run_askpass() -> Option<i32> {
    let account = std::env::var(ASKPASS_ACCOUNT_ENV).ok()?;
    let prompt = std::env::args().nth(1).unwrap_or_default().to_lowercase();
    let answer = if prompt.starts_with("username") {
        std::env::var(ASKPASS_USER_ENV).ok()
    } else if prompt.starts_with("password") {
        read_token(&account).ok()
    } else {
        None
    };
    match answer {
        Some(a) => {
            use std::io::Write;
            let mut out = std::io::stdout();
            let _ = writeln!(out, "{a}");
            let _ = out.flush();
            Some(0)
        }
        None => Some(1),
    }
}
