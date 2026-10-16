//! Structured errors shared by every Tauri command.
//!
//! Every error carries a stable `code` the UI can switch on, a human message in
//! pt-BR, optional technical details (already redacted) and an optional recovery hint.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    GitNotFound,
    NotARepository,
    RepoNotAuthorized,
    GitFailed,
    Conflict,
    AuthFailed,
    HostKey,
    Network,
    NonFastForward,
    ProtectedBranch,
    DirtyWorktree,
    LockFile,
    NoUpstream,
    Cancelled,
    Timeout,
    InvalidInput,
    Io,
    Provider,
    RateLimited,
    Credential,
    NotFound,
    StaleState,
    Internal,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    pub details: Option<String>,
    pub recovery: Option<String>,
    pub repo: Option<String>,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), details: None, recovery: None, repo: None }
    }
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidInput, message)
    }
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }
    pub fn details(mut self, details: impl Into<String>) -> Self {
        let d = details.into();
        if !d.trim().is_empty() {
            self.details = Some(crate::redact::redact(&d));
        }
        self
    }
    pub fn recovery(mut self, recovery: impl Into<String>) -> Self {
        self.recovery = Some(recovery.into());
        self
    }
    pub fn repo(mut self, repo: impl Into<String>) -> Self {
        self.repo = Some(repo.into());
        self
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::new(ErrorCode::Io, "Falha de entrada/saída no sistema de arquivos.").details(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::internal("Falha ao serializar/deserializar dados.").details(e.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        let code = if e.is_timeout() || e.is_connect() { ErrorCode::Network } else { ErrorCode::Provider };
        // reqwest errors can include the URL; URLs never carry tokens in this app, but redact anyway.
        AppError::new(code, "Falha de comunicação com o provedor.")
            .details(e.without_url().to_string())
            .recovery("Verifique sua conexão com a internet, proxy e a URL do provedor.")
    }
}

/// Classify a failed git invocation from its stderr. Exit code is the source of truth
/// for failure; this only picks the best human explanation.
pub fn classify_git_failure(args: &[String], stderr: &str) -> AppError {
    let s = stderr.to_lowercase();
    let first = stderr
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("fatal:") || l.starts_with("error:"))
        .or_else(|| stderr.lines().map(str::trim).find(|l| !l.is_empty()))
        .unwrap_or("o Git retornou um erro sem mensagem")
        .to_string();
    let cmd = args.iter().find(|a| !a.starts_with('-')).cloned().unwrap_or_default();

    let (code, msg, rec): (ErrorCode, String, Option<&str>) = if s.contains("host key verification failed") {
        (ErrorCode::HostKey, "A chave do host SSH não é confiável ou mudou.".into(),
         Some("Conecte-se uma vez pelo terminal (ssh -T git@host) para aceitar a chave do host, ou verifique ~/.ssh/known_hosts."))
    } else if s.contains("authentication failed")
        || s.contains("could not read username")
        || s.contains("could not read password")
        || s.contains("permission denied (publickey")
        || s.contains("terminal prompts disabled")
        || s.contains("invalid username or password")
        || s.contains("http basic: access denied")
        || s.contains("the requested url returned error: 401")
        || s.contains("the requested url returned error: 403")
    {
        (ErrorCode::AuthFailed, "Falha de autenticação no remote.".into(),
         Some("HTTPS: confirme a credencial no Git Credential Manager ou use uma conta conectada. SSH: verifique se a chave está carregada no ssh-agent e cadastrada no provedor."))
    } else if s.contains("protected branch") || s.contains("gh006") || s.contains("pre-receive hook declined") {
        (ErrorCode::ProtectedBranch, "O remote recusou o push (branch protegida ou hook do servidor).".into(),
         Some("Abra um Pull/Merge Request ou ajuste as regras de proteção no provedor."))
    } else if (s.contains("[rejected]") || s.contains("! [remote rejected]") || s.contains("failed to push"))
        && (s.contains("non-fast-forward") || s.contains("fetch first") || s.contains("stale info"))
    {
        (ErrorCode::NonFastForward, "O push foi rejeitado porque o remote tem commits que você ainda não possui.".into(),
         Some("Faça Pull (ou Fetch + rebase/merge) e tente novamente. Force push só com confirmação explícita."))
    } else if s.contains("could not resolve host")
        || s.contains("failed to connect")
        || s.contains("connection timed out")
        || s.contains("connection refused")
        || s.contains("network is unreachable")
        || s.contains("could not resolve hostname")
        || s.contains("operation timed out")
        || s.contains("ssl certificate problem")
        || s.contains("unable to access")
    {
        (ErrorCode::Network, "Não foi possível alcançar o remote.".into(),
         Some("Verifique a conexão, proxy, VPN e a URL do remote. As operações locais continuam disponíveis."))
    } else if s.contains("could not read from remote repository") {
        (ErrorCode::AuthFailed, "Não foi possível ler o repositório remoto (acesso negado ou repositório inexistente).".into(),
         Some("Confira a URL, suas permissões no provedor e a credencial SSH/HTTPS."))
    } else if s.contains("index.lock") || (s.contains("unable to create") && s.contains(".lock")) {
        (ErrorCode::LockFile, "Outro processo Git está usando este repositório (arquivo .lock).".into(),
         Some("Aguarde o outro processo terminar. Se nenhum estiver ativo, remova o arquivo .lock indicado nos detalhes."))
    } else if s.contains("would be overwritten by") || s.contains("please commit your changes or stash them") || s.contains("your local changes") {
        (ErrorCode::DirtyWorktree, "Há alterações locais que seriam sobrescritas por esta operação.".into(),
         Some("Faça commit ou stash das alterações e tente novamente."))
    } else if s.contains("conflict") || s.contains("could not apply") || s.contains("after resolving the conflicts") || s.contains("fix conflicts") {
        (ErrorCode::Conflict, "A operação parou com conflitos.".into(),
         Some("Resolva os arquivos em conflito, marque-os como resolvidos e escolha Continuar, ou Abortar."))
    } else if s.contains("no upstream") || s.contains("has no upstream branch") || s.contains("no tracking information") {
        (ErrorCode::NoUpstream, "A branch atual não tem upstream configurado.".into(),
         Some("Faça Push com a opção de definir upstream (-u) ou configure o tracking da branch."))
    } else if s.contains("not a git repository") {
        (ErrorCode::NotARepository, "A pasta não é um repositório Git.".into(), None)
    } else {
        (ErrorCode::GitFailed, format!("git {cmd} falhou: {}", first.trim_start_matches("fatal: ").trim_start_matches("error: ")), None)
    };
    let mut err = AppError::new(code, msg).details(stderr.to_string());
    if let Some(r) = rec {
        err = err.recovery(r);
    }
    err
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_common_failures() {
        let a = vec!["push".to_string()];
        assert_eq!(classify_git_failure(&a, "To x\n ! [rejected] main -> main (fetch first)\nerror: failed to push").code, ErrorCode::NonFastForward);
        assert_eq!(classify_git_failure(&a, "fatal: Authentication failed for 'https://x'").code, ErrorCode::AuthFailed);
        assert_eq!(classify_git_failure(&a, "fatal: unable to access 'https://x/': Could not resolve host: x").code, ErrorCode::Network);
        assert_eq!(classify_git_failure(&a, "fatal: Unable to create 'C:/r/.git/index.lock': File exists.").code, ErrorCode::LockFile);
        assert_eq!(classify_git_failure(&a, "CONFLICT (content): Merge conflict in a.txt").code, ErrorCode::Conflict);
        assert_eq!(classify_git_failure(&a, "Host key verification failed.").code, ErrorCode::HostKey);
    }
}
